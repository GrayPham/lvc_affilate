//! Tạo VIDEO qua kênh API app: gửi lệnh action-bar "Nhanh" rồi đọc SSE + poll chat tới khi có
//! video — port `send_video`/`send_payload`/`poll_video` (`dola_app.py:702-708`, `:1004-1035`,
//! `:1182-...`) và vòng đợi video (`video_worker_ui.py:2979-3014`).
//!
//! Dùng lại khối dựng body ([`crate::payload`]) và bộ đọc ([`crate::response`]). Một lần chạy = một
//! chat mới (không nối tiếp chat cũ). Tôn trọng [`CancellationToken`].

use std::time::{Duration, Instant};

use lvc_core::{Limits, LogoMode, Mode, Model, Ratio, video_timeout};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::error::ApiError;
use crate::payload::{
    ChatRef, RealIds, attachment_block, batch_get_body, poll_chain_body, video_payload,
};
use crate::response::{
    ChatStream, SseParser, VIDEO_STATUS_DONE, VideoUrls, explain_error, extract_urls, parse_chain,
    walk_video_models,
};
use crate::session::{AppSession, POLL_TIMEOUT, SSE_TIMEOUT, net_error};
use crate::upload::UploadedRef;

/// Khoảng nghỉ giữa hai lần poll chat (`video_worker_ui.py:2992` — 10s).
const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Tham số tạo video (đúng các trường cảnh trong `lvc_core::Scene`).
#[derive(Debug, Clone)]
pub struct VideoOpts {
    /// Chế độ Dola. Hiện dùng đường action-bar "Nhanh"; `mode` giữ để engine quyết trần thời gian
    /// đợi (pro-like đợi lâu hơn) và ghi lại cấu hình.
    pub mode: Mode,
    pub model: Model,
    pub ratio: Ratio,
    pub duration: u16,
    /// Xử lý watermark do engine lo; ở đây chỉ ảnh hưởng ưu tiên URL trả về.
    pub logo: LogoMode,
}

impl Default for VideoOpts {
    fn default() -> Self {
        Self {
            mode: Mode::Fast,
            model: Model::default(),
            ratio: Ratio::default(),
            duration: 10,
            logo: LogoMode::default(),
        }
    }
}

/// Video đã tạo xong (chưa tải file — engine tải).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoOut {
    /// URL ưu tiên bản KHÔNG watermark; nếu không có thì `download_url` (có watermark).
    pub video_url: String,
    pub conversation_id: String,
    /// `true` nếu URL trả về là bản có watermark (không lấy được bản sạch).
    pub watermarked: bool,
}

/// Video coi là XONG khi status = 3 và đã có URL tải (`download_url` hoặc `video_model` bản sạch)
/// (`video_worker_ui.py:2989`).
fn video_ready(v: Option<&Value>) -> bool {
    let Some(v) = v else { return false };
    if v.get("status").and_then(Value::as_i64) != Some(VIDEO_STATUS_DONE) {
        return false;
    }
    let has_dl = v
        .get("download_url")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    let has_vm = v
        .get("video_model")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    has_dl || has_vm
}

/// Chọn URL cuối: ưu tiên bản không watermark (`nowm_url`), nếu rỗng thì `download_url`.
fn pick_url(urls: &VideoUrls) -> Option<(String, bool)> {
    if !urls.nowm_url.is_empty() {
        Some((urls.nowm_url.clone(), false))
    } else if !urls.download_url.is_empty() {
        Some((urls.download_url.clone(), true))
    } else {
        None
    }
}

/// Tạo một video từ `prompt` (+ ảnh tham chiếu đã upload). Trả [`VideoOut`] khi có URL.
///
/// # Errors
/// `ApiError`: mạng/proxy, `LoggedOut`, `Stream` (Dola trả `STREAM_ERROR`), `NoConversation`,
/// `Timeout` (đợi quá trần), `Cancelled`, `Protocol` (thiếu URL).
pub async fn generate_video(
    session: &AppSession,
    prompt: &str,
    refs: &[UploadedRef],
    opts: &VideoOpts,
    cancel: &CancellationToken,
) -> Result<VideoOut, ApiError> {
    generate_video_with(session, prompt, refs, opts, cancel, POLL_INTERVAL).await
}

/// Như [`generate_video`] nhưng cho tuỳ chỉnh khoảng poll (test dùng khoảng ngắn để chạy nhanh).
///
/// # Errors
/// Như [`generate_video`].
pub async fn generate_video_with(
    session: &AppSession,
    prompt: &str,
    refs: &[UploadedRef],
    opts: &VideoOpts,
    cancel: &CancellationToken,
    poll_interval: Duration,
) -> Result<VideoOut, ApiError> {
    let mut ids = RealIds;
    let who = session.identity();
    let blocks: Vec<Value> = refs
        .iter()
        .map(|r| attachment_block(&r.attachment(), &mut ids))
        .collect();
    let payload = video_payload(
        &who,
        prompt,
        opts.model,
        opts.ratio,
        opts.duration,
        &ChatRef::default(),
        &blocks,
        &mut ids,
    );
    let body = serde_json::to_vec(&payload).map_err(|e| ApiError::Protocol(e.to_string()))?;

    // --- Gửi + đọc SSE tới hết (hoặc STREAM_ERROR) ---
    let mut stream = read_sse(session, body, cancel).await?;
    if !stream.error.is_empty() {
        return Err(ApiError::Stream {
            code: stream.error_code.clone(),
            message: stream_message(&stream.error),
        });
    }
    let conv = std::mem::take(&mut stream.conversation_id);
    if conv.is_empty() {
        return Err(ApiError::NoConversation(format!(
            "Dola khong tra conversation_id (bot: {})",
            snippet(&stream.text)
        )));
    }

    // --- Đợi video: dùng luôn kết quả trong SSE, nếu chưa xong thì poll ---
    let mut video = stream.video.take();
    let wait = Duration::from_secs(video_timeout(
        opts.mode,
        opts.duration,
        !refs.is_empty(),
        &Limits::default(),
    ));
    let deadline = Instant::now() + wait;
    while !video_ready(video.as_ref()) {
        if Instant::now() >= deadline {
            return Err(ApiError::Timeout(format!(
                "khong co video sau {}s (conversation_id={conv})",
                wait.as_secs()
            )));
        }
        sleep_or_cancel(poll_interval, cancel).await?;
        let poll = session
            .post_json(
                "im/chain/single",
                &session.params_im,
                &poll_chain_body(&conv),
                POLL_TIMEOUT,
            )
            .await?;
        let chain = parse_chain(&poll);
        if let Some(v) = chain.video {
            video = Some(v);
        }
    }

    let mut video = video.unwrap_or(Value::Null);
    let mut urls = extract_urls(&video);
    // Fallback: co video nhung chua co ban sach -> hoi batch_get lay video_model sach
    // (conv_video_model, dola_app.py:1107-1121).
    if urls.nowm_url.is_empty() && urls.video_model.is_empty() && !urls.vid.is_empty() {
        if let Some(vm) = fetch_clean_model(session, &conv).await {
            if let Value::Object(map) = &mut video {
                map.insert("video_model".into(), Value::String(vm));
            }
            urls = extract_urls(&video);
        }
    }

    let (video_url, watermarked) = pick_url(&urls).ok_or_else(|| {
        ApiError::Protocol(format!(
            "video xong nhung khong co URL (conversation_id={conv})"
        ))
    })?;
    Ok(VideoOut {
        video_url,
        conversation_id: conv,
        watermarked,
    })
}

/// Gửi `chat/completion` (stream) và đọc toàn bộ SSE vào [`ChatStream`].
async fn read_sse(
    session: &AppSession,
    body: Vec<u8>,
    cancel: &CancellationToken,
) -> Result<ChatStream, ApiError> {
    let mut resp = session
        .post(
            "chat/completion",
            &session.params_chat,
            body,
            SSE_TIMEOUT,
            &[],
        )
        .await?;
    let mut parser = SseParser::new();
    let mut stream = ChatStream::new("");
    loop {
        let chunk = tokio::select! {
            () = cancel.cancelled() => return Err(ApiError::Cancelled),
            c = resp.chunk() => c.map_err(|e| net_error(e, session.proxy()))?,
        };
        match chunk {
            Some(bytes) => {
                for (ev, data) in parser.push(&bytes) {
                    stream.feed(ev.as_deref(), &data);
                }
                if stream.stopped {
                    break;
                }
            }
            None => break,
        }
    }
    for (ev, data) in parser.finish() {
        stream.feed(ev.as_deref(), &data);
    }
    Ok(stream)
}

/// Nghỉ `dur`, nhưng huỷ ngay nếu `cancel` được kích.
async fn sleep_or_cancel(dur: Duration, cancel: &CancellationToken) -> Result<(), ApiError> {
    tokio::select! {
        () = cancel.cancelled() => Err(ApiError::Cancelled),
        () = tokio::time::sleep(dur) => Ok(()),
    }
}

/// Lấy `video_model` bản sạch qua `im/conversation/batch_get` (cmd 1111). Lỗi → `None`.
async fn fetch_clean_model(session: &AppSession, conv: &str) -> Option<String> {
    let resp = session
        .post_json(
            "im/conversation/batch_get",
            &session.params_im,
            &batch_get_body(conv),
            POLL_TIMEOUT,
        )
        .await
        .ok()?;
    let mut out = Vec::new();
    walk_video_models(&resp, &mut out);
    out.into_iter().find(|s| !s.is_empty())
}

/// Giải thích tiếng Việt nếu biết mã lỗi; nếu không thì trả JSON lỗi gọn.
fn stream_message(err: &str) -> String {
    let why = explain_error(err);
    if why.is_empty() {
        snippet(err)
    } else {
        format!("{why} · {}", snippet(err))
    }
}

fn snippet(s: &str) -> String {
    s.chars().take(200).collect()
}
