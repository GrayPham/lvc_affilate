//! Vận chuyển RPC qua WebSocket + Noise — port từ `transport.py` (tham khảo, CHỈ ĐỌC).
//!
//! Dùng `tokio-tungstenite` CHUẨN (TLS qua `rustls`, không giả mạo fingerprint TLS/JA3) — khác hẳn
//! `curl_cffi` của `auth.py` bản Python (CỐ Ý không port phần đó, xem `CLAUDE.md` "bối cảnh rủi
//! ro"). Nguồn tham khảo tự xác nhận cổng Hatch (`wss://hatch.metaaivm.com`) dùng thư viện
//! `websockets` thường, KHÔNG dùng `curl_cffi` — việc chống-bot chỉ nằm ở các endpoint HTTP của
//! `muse.ai` (bootstrap/token), không phải cổng WebSocket này.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex as AsyncMutex, mpsc};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::errors::MuseError;
use super::noise::{CipherState, NoiseXxInitiator, make_message1_payload};
use super::wire::{
    ApplicationRequest, DecodedServiceFrame, Header, NoiseReassembler, SERVICE_AUTHD,
    SERVICE_DAEMON, SERVICE_SENTINEL, SERVICE_VAULT, ServiceFramePayload, decode_service_frame,
    decode_service_response, encode_service_frame_request, encode_service_request,
    split_noise_payload,
};

type Result<T> = std::result::Result<T, MuseError>;
type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsWrite = SplitSink<WsStream, Message>;
type WsRead = SplitStream<WsStream>;
/// `None` = tín hiệu "đã đóng kết nối" (khớp `queue.put(None)` bên Python).
type FrameTx = mpsc::UnboundedSender<Option<DecodedServiceFrame>>;
type FrameRx = mpsc::UnboundedReceiver<Option<DecodedServiceFrame>>;

const EMPTY_AD: &[u8] = b"";

fn service_id(service: &str) -> Option<u64> {
    Some(match service {
        "daemon" => SERVICE_DAEMON,
        "sentinel" => SERVICE_SENTINEL,
        "vault" => SERVICE_VAULT,
        "authd" => SERVICE_AUTHD,
        _ => return None,
    })
}

#[derive(Debug, Clone)]
pub struct RpcResponse {
    pub status: i64,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

impl RpcResponse {
    /// # Errors
    /// Lỗi parse JSON khi `body` không rỗng nhưng không phải JSON hợp lệ.
    pub fn json<T: serde::de::DeserializeOwned>(
        &self,
    ) -> std::result::Result<Option<T>, serde_json::Error> {
        if self.body.is_empty() {
            return Ok(None);
        }
        serde_json::from_slice(&self.body).map(Some)
    }

    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// # Errors
    /// [`MuseError::Rpc`] khi `status >= 400`.
    pub fn raise_for_status(self) -> Result<Self> {
        if self.status >= 400 {
            let snippet: String = self.text().chars().take(500).collect();
            let status = u16::try_from(self.status).ok();
            return Err(MuseError::Rpc {
                message: format!("Hatch RPC tra ve HTTP {}: {snippet}", self.status),
                status,
                body: self.body,
            });
        }
        Ok(self)
    }
}

struct SendSide {
    tx: CipherState,
    next_stream_id: i64,
}

impl SendSide {
    fn encrypt_request(
        &mut self,
        method: &str,
        path: &str,
        headers: Vec<Header>,
        body: Vec<u8>,
        service: &str,
    ) -> Result<(i64, Vec<Vec<u8>>)> {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 1;
        let app = ApplicationRequest {
            verb: method.to_string(),
            path: path.to_string(),
            headers,
            body,
            end_body: true,
        };
        let service_frame = encode_service_frame_request(stream_id, &app);
        let sid = service_id(service)
            .ok_or_else(|| MuseError::Protocol(format!("dich vu Hatch khong ro: {service}")))?;
        let envelope = encode_service_request(sid, &service_frame);
        let chunks = split_noise_payload(&envelope, super::wire::MAX_NOISE_CHUNK_PAYLOAD)
            .map_err(|e| MuseError::Protocol(e.to_string()))?;
        let mut encrypted = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            encrypted.push(
                self.tx
                    .encrypt_with_ad(EMPTY_AD, &chunk)
                    .map_err(|e| MuseError::Protocol(e.to_string()))?,
            );
        }
        Ok((stream_id, encrypted))
    }
}

fn decrypt_ws_frame(
    rx: &mut CipherState,
    reassembler: &mut NoiseReassembler,
    encrypted: &[u8],
) -> Result<Option<DecodedServiceFrame>> {
    let raw_chunk = rx
        .decrypt_with_ad(EMPTY_AD, encrypted)
        .map_err(|e| MuseError::Protocol(e.to_string()))?;
    let Some(assembled) = reassembler
        .feed(&raw_chunk)
        .map_err(|e| MuseError::Protocol(e.to_string()))?
    else {
        return Ok(None);
    };
    let service_payload =
        decode_service_response(&assembled).map_err(|e| MuseError::Protocol(e.to_string()))?;
    let frame =
        decode_service_frame(&service_payload).map_err(|e| MuseError::Protocol(e.to_string()))?;
    Ok(Some(frame))
}

async fn receiver_loop(
    mut read: WsRead,
    mut rx: CipherState,
    streams: Arc<SyncMutex<HashMap<i64, FrameTx>>>,
    closed: Arc<AtomicBool>,
    fatal: Arc<SyncMutex<Option<String>>>,
) {
    let mut reassembler = NoiseReassembler::new();
    while let Some(item) = read.next().await {
        let message = match item {
            Ok(m) => m,
            Err(e) => {
                *fatal
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(e.to_string());
                break;
            }
        };
        let Message::Binary(bytes) = message else {
            continue;
        };
        match decrypt_ws_frame(&mut rx, &mut reassembler, &bytes) {
            Ok(Some(frame)) => {
                let sender = streams
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&frame.stream_id)
                    .cloned();
                if let Some(sender) = sender {
                    let _ = sender.send(Some(frame));
                }
            }
            Ok(None) => {}
            Err(e) => {
                *fatal
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(e.to_string());
                break;
            }
        }
    }
    closed.store(true, Ordering::SeqCst);
    let mut map = streams
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for (_, sender) in map.drain() {
        let _ = sender.send(None);
    }
}

/// Kết nối RPC tới cổng Hatch của Muse — bắt tay Noise XX rồi gửi/nhận request qua các khung mã
/// hoá (xem doc đầu file). Clone được (các trường đều `Arc`) — nhiều tác vụ có thể cùng gọi
/// [`HatchConnection::request`] qua CÙNG một kết nối, phía gửi khoá bằng [`AsyncMutex`].
#[derive(Clone)]
pub struct HatchConnection {
    write: Arc<AsyncMutex<WsWrite>>,
    send: Arc<AsyncMutex<SendSide>>,
    streams: Arc<SyncMutex<HashMap<i64, FrameTx>>>,
    locale: String,
    closed: Arc<AtomicBool>,
    fatal: Arc<SyncMutex<Option<String>>>,
}

impl HatchConnection {
    /// Bắt tay Noise XX qua WebSocket rồi mở kết nối RPC sẵn sàng dùng.
    ///
    /// # Errors
    /// [`MuseError::Protocol`] khi kết nối/bắt tay thất bại hoặc quá hạn `timeout`.
    pub async fn connect(ws_url: &str, locale: &str, timeout: Duration) -> Result<Self> {
        let (ws_stream, _response) =
            tokio::time::timeout(timeout, tokio_tungstenite::connect_async(ws_url))
                .await
                .map_err(|_| {
                    MuseError::Protocol("ket noi WebSocket Hatch qua thoi gian cho".into())
                })?
                .map_err(|e| {
                    MuseError::Protocol(format!("ket noi WebSocket Hatch that bai: {e}"))
                })?;
        let (mut write, mut read) = ws_stream.split();

        let mut initiator = NoiseXxInitiator::new();
        let (_nonce, payload1) = make_message1_payload();
        let msg1 = initiator
            .write_message1(&payload1)
            .map_err(|e| MuseError::Protocol(e.to_string()))?;
        write
            .send(Message::Binary(msg1))
            .await
            .map_err(|e| MuseError::Protocol(format!("gui Noise message1 that bai: {e}")))?;

        let message2 = tokio::time::timeout(timeout, read.next())
            .await
            .map_err(|_| MuseError::Protocol("cho Noise message2 qua thoi gian".into()))?
            .ok_or_else(|| MuseError::Protocol("ket noi dong truoc khi nhan message2".into()))?
            .map_err(|e| MuseError::Protocol(format!("loi doc Noise message2: {e}")))?;
        let Message::Binary(message2_bytes) = message2 else {
            return Err(MuseError::Protocol(
                "mong doi Noise message2 dang nhi phan".into(),
            ));
        };
        initiator
            .read_message2(&message2_bytes)
            .map_err(|e| MuseError::Protocol(e.to_string()))?;

        let msg3 = initiator
            .write_message3(b"")
            .map_err(|e| MuseError::Protocol(e.to_string()))?;
        write
            .send(Message::Binary(msg3))
            .await
            .map_err(|e| MuseError::Protocol(format!("gui Noise message3 that bai: {e}")))?;

        let (tx, rx) = initiator
            .split_ciphers()
            .map_err(|e| MuseError::Protocol(e.to_string()))?;

        let streams: Arc<SyncMutex<HashMap<i64, FrameTx>>> = Arc::default();
        let closed = Arc::new(AtomicBool::new(false));
        let fatal = Arc::new(SyncMutex::new(None));
        tokio::spawn(receiver_loop(
            read,
            rx,
            streams.clone(),
            closed.clone(),
            fatal.clone(),
        ));

        Ok(Self {
            write: Arc::new(AsyncMutex::new(write)),
            send: Arc::new(AsyncMutex::new(SendSide {
                tx,
                next_stream_id: 1,
            })),
            streams,
            locale: locale.to_string(),
            closed,
            fatal,
        })
    }

    fn headers(&self, json_body: bool, extra: Vec<Header>) -> Vec<Header> {
        let mut headers = Vec::new();
        if json_body {
            headers.push(Header {
                key: "Content-Type".into(),
                value: "application/json".into(),
            });
        }
        headers.push(Header {
            key: "x-request-id".into(),
            value: uuid::Uuid::new_v4().to_string(),
        });
        headers.push(Header {
            key: "x-app-id".into(),
            value: "hatch-web".into(),
        });
        headers.push(Header {
            key: "Accept-Language".into(),
            value: self.locale.clone(),
        });
        headers.extend(extra);
        headers
    }

    async fn open(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
        json_body: bool,
        service: &str,
        extra_headers: Vec<Header>,
    ) -> Result<(i64, FrameRx)> {
        if self.closed.load(Ordering::SeqCst) {
            let reason = self
                .fatal
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            return Err(MuseError::Protocol(format!(
                "ket noi Noise da dong: {reason:?}"
            )));
        }
        let headers = self.headers(json_body, extra_headers);
        let (stream_id, frames) = {
            let mut send = self.send.lock().await;
            send.encrypt_request(method, path, headers, body, service)?
        };
        let (tx_chan, rx_chan) = mpsc::unbounded_channel();
        self.streams
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(stream_id, tx_chan);
        let mut write = self.write.lock().await;
        for frame in frames {
            if let Err(e) = write.send(Message::Binary(frame)).await {
                self.streams
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&stream_id);
                return Err(MuseError::Protocol(format!(
                    "gui khung Hatch that bai: {e}"
                )));
            }
        }
        Ok((stream_id, rx_chan))
    }

    /// Request/response một lần (`GET`/`POST` không-stream) — `params` mã hoá JSON nếu khác
    /// `GET`, nếu không gửi thẳng không query string (chưa cần — các điểm gọi hiện tại đều `GET`
    /// không tham số).
    ///
    /// # Errors
    /// [`MuseError::Protocol`] khi kết nối lỗi/quá hạn; [`MuseError::Rpc`] khi server trả `reset`.
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        params: Option<&serde_json::Value>,
        timeout: Duration,
    ) -> Result<RpcResponse> {
        self.request_with_headers(method, path, params, Vec::new(), timeout)
            .await
    }

    /// Như [`HatchConnection::request`] nhưng cho thêm header (vd `Range` khi tải một phần file).
    ///
    /// # Errors
    /// Như [`HatchConnection::request`].
    pub async fn request_with_headers(
        &self,
        method: &str,
        path: &str,
        params: Option<&serde_json::Value>,
        extra_headers: Vec<Header>,
        timeout: Duration,
    ) -> Result<RpcResponse> {
        let (path_full, body, json_body) = encode_request(method, path, params);
        let (stream_id, mut rx_chan) = self
            .open(method, &path_full, body, json_body, "daemon", extra_headers)
            .await?;
        let out = tokio::time::timeout(timeout, collect_response(&mut rx_chan, &self.fatal)).await;
        self.streams
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&stream_id);
        out.map_err(|_| MuseError::Protocol("yeu cau RPC Hatch qua thoi gian cho".into()))?
    }

    /// Như `chat_stream` bên Python: mở một subscription, thu mọi dòng JSON (phân cách `\n`) tới
    /// khi `end_body` hoặc hết `timeout` — quá hạn KHÔNG phải lỗi (endpoint xác nhận, có thể đóng
    /// trễ hơn tiến trình sinh thật), trả những gì đã thu được.
    ///
    /// # Errors
    /// [`MuseError::Protocol`]/[`MuseError::Rpc`] khi kết nối lỗi hoặc server trả lỗi rõ ràng
    /// (KHÁC với việc hết hạn `timeout`, vốn trả `Ok` với những gì đã thu được).
    pub async fn subscribe_collect(
        &self,
        method: &str,
        path: &str,
        params: Option<&serde_json::Value>,
        timeout: Duration,
    ) -> Result<Vec<serde_json::Value>> {
        let (path_full, body, json_body) = encode_request(method, path, params);
        let (stream_id, mut rx_chan) = self
            .open(method, &path_full, body, json_body, "daemon", Vec::new())
            .await?;
        let out = tokio::time::timeout(timeout, collect_subscription(&mut rx_chan)).await;
        self.streams
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&stream_id);
        match out {
            Ok(result) => result,
            Err(_timeout) => Ok(Vec::new()),
        }
    }

    pub async fn close(&self) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            let mut write = self.write.lock().await;
            let _ = write.close().await;
        }
    }
}

async fn collect_response(
    rx_chan: &mut FrameRx,
    fatal: &Arc<SyncMutex<Option<String>>>,
) -> Result<RpcResponse> {
    let mut status = 0i64;
    let mut headers = Vec::new();
    let mut chunks = Vec::new();
    loop {
        let Some(frame) = rx_chan.recv().await.flatten() else {
            let reason = fatal
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            return Err(MuseError::Protocol(format!(
                "ket noi Noise dong giua luc cho RPC: {reason:?}"
            )));
        };
        match frame.payload {
            ServiceFramePayload::Reset { code, reason } => {
                return Err(MuseError::Rpc {
                    message: format!("Hatch stream reset code={code}: {reason}"),
                    status: None,
                    body: Vec::new(),
                });
            }
            ServiceFramePayload::Response(resp) => {
                status = resp.status;
                headers = resp.headers;
                chunks.extend(resp.body);
                if resp.end_body {
                    break;
                }
            }
            ServiceFramePayload::BodyChunk { data, end_body } => {
                chunks.extend(data);
                if end_body {
                    break;
                }
            }
            ServiceFramePayload::Unknown => {}
        }
    }
    Ok(RpcResponse {
        status,
        headers,
        body: chunks,
    })
}

async fn collect_subscription(rx_chan: &mut FrameRx) -> Result<Vec<serde_json::Value>> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = Vec::new();
    loop {
        let Some(frame) = rx_chan.recv().await.flatten() else {
            return Ok(events);
        };
        let end_body = match frame.payload {
            ServiceFramePayload::Reset { code, reason } => {
                return Err(MuseError::Rpc {
                    message: format!("Hatch subscription reset code={code}: {reason}"),
                    status: None,
                    body: Vec::new(),
                });
            }
            ServiceFramePayload::Response(resp) => {
                if resp.status >= 400 {
                    return Err(MuseError::Rpc {
                        message: format!("Hatch subscription HTTP {}", resp.status),
                        status: u16::try_from(resp.status).ok(),
                        body: resp.body,
                    });
                }
                buffer.extend(resp.body);
                resp.end_body
            }
            ServiceFramePayload::BodyChunk { data, end_body } => {
                buffer.extend(data);
                end_body
            }
            ServiceFramePayload::Unknown => continue,
        };
        drain_json_lines(&mut buffer, &mut events)?;
        if end_body {
            if !buffer.iter().all(u8::is_ascii_whitespace) {
                events.push(
                    serde_json::from_slice(&buffer)
                        .map_err(|e| MuseError::Protocol(format!("JSON subscription loi: {e}")))?,
                );
            }
            return Ok(events);
        }
    }
}

fn drain_json_lines(buffer: &mut Vec<u8>, events: &mut Vec<serde_json::Value>) -> Result<()> {
    while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
        let line: Vec<u8> = buffer.drain(..=pos).collect();
        let line = &line[..line.len() - 1];
        if !line.iter().all(u8::is_ascii_whitespace) && !line.is_empty() {
            events.push(
                serde_json::from_slice(line)
                    .map_err(|e| MuseError::Protocol(format!("JSON subscription loi: {e}")))?,
            );
        }
    }
    Ok(())
}

/// `params` chỉ hỗ trợ mã hoá JSON body cho verb khác `GET` — các điểm gọi `GET` hiện tại
/// (`/model`, `/api/session/list`) đều không tham số nên KHÔNG cần codec query-string ở đây.
fn encode_request(
    method: &str,
    path: &str,
    params: Option<&serde_json::Value>,
) -> (String, Vec<u8>, bool) {
    if method.eq_ignore_ascii_case("GET") {
        return (path.to_string(), Vec::new(), false);
    }
    let body = params
        .map(|p| serde_json::to_vec(p).unwrap_or_default())
        .unwrap_or_default();
    (path.to_string(), body, true)
}
