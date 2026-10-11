//! P6 — Smoke test TRUNG TÍNH: Rust điều khiển `Camoufox` qua `WebDriver BiDi` để kiểm độ ổn định
//! của việc điều khiển, CHỈ mở một trang VÔ HẠI (`example.com`).
//!
//! RANH GIỚI CỨNG (cố ý): test này KHÔNG mở `flow.google.com`, KHÔNG mint `reCAPTCHA`, KHÔNG nạp
//! cookie/tài khoản, KHÔNG qua mặt chống bot. Nó chỉ trả lời đúng MỘT câu hỏi: điều khiển `Camoufox`
//! bằng Rust (qua `BiDi`) có ổn định không — đo trên một trang tĩnh công khai. Lái `Camoufox` vào
//! Flow là ranh giới cứng của repo (xem `p4b-no-antibot-signing`, `login.rs`) nên KHÔNG làm ở đây.
//!
//! Cách chạy (TAY — cần `Camoufox` đang chạy + mạng; `#[ignore]` nên KHÔNG vào CI hay `check.ps1`):
//!
//! - Mở `Camoufox` bật `WebDriver BiDi` (ví dụ cờ `--remote-debugging-port=9333`); nó in ra dòng
//!   `WebDriver BiDi listening on ...` kèm địa chỉ `ws`.
//! - Nếu địa chỉ khác mặc định, đặt biến môi trường `CAMOUFOX_BIDI_WS` cho khớp.
//! - Chạy: `cargo test -p lvc-browser --test camoufox_bidi_smoke -- --ignored --nocapture`.

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Endpoint `BiDi` mặc định (ghi đè qua env `CAMOUFOX_BIDI_WS`). Camoufox để lộ `BiDi` ở
/// `ws://127.0.0.1:9333` — thêm `/session` để vào chế độ `BiDi`-only (gọi `session.new` trực tiếp).
fn bidi_ws_url() -> String {
    std::env::var("CAMOUFOX_BIDI_WS").unwrap_or_else(|_| "ws://127.0.0.1:9333/session".to_string())
}

#[tokio::test]
#[ignore = "can Camoufox THAT dang chay + mang; KHONG chay trong CI/check.ps1. Chay tay: \
            cargo test -p lvc-browser --test camoufox_bidi_smoke -- --ignored --nocapture"]
async fn dieu_khien_camoufox_mo_example_com_qua_bidi() {
    let url = bidi_ws_url();
    eprintln!("Ket noi BiDi toi {url} …");
    let (mut ws, _) = connect_async(&url)
        .await
        .expect("ket noi WebSocket BiDi (Camoufox da chay + dung endpoint chua?)");

    // 1) Mo phien BiDi.
    let session = cmd(&mut ws, 1, "session.new", json!({ "capabilities": {} })).await;
    eprintln!(
        "session.new OK (sessionId = {})",
        session
            .pointer("/result/sessionId")
            .and_then(Value::as_str)
            .unwrap_or("?")
    );

    // 2) Lay context (tab) dau tien.
    let tree = cmd(&mut ws, 2, "browsingContext.getTree", json!({})).await;
    let context = tree
        .pointer("/result/contexts/0/context")
        .and_then(Value::as_str)
        .expect("co it nhat 1 browsing context")
        .to_string();

    // 3) Dieu huong toi trang TRUNG TINH (example.com), cho tai xong.
    cmd(
        &mut ws,
        3,
        "browsingContext.navigate",
        json!({ "context": context, "url": "https://example.com/", "wait": "complete" }),
    )
    .await;

    // 4) Doc document.title de xac nhan dieu khien thuc su chay (khong chi ket noi duoc).
    let evald = cmd(
        &mut ws,
        4,
        "script.evaluate",
        json!({
            "expression": "document.title",
            "target": { "context": context },
            "awaitPromise": false
        }),
    )
    .await;
    let title = evald
        .pointer("/result/result/value")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    eprintln!("document.title = {title:?}");

    // 5) Dong phien (don dep). ws tu dong dong khi `ws` bi drop cuoi test.
    send(&mut ws, 5, "session.end", json!({})).await;

    assert!(
        title.contains("Example"),
        "mong tieu de chua 'Example' (example.com), nhan duoc: {title:?} \
         — co the build Camoufox khac giao thuc; kiem endpoint/BiDi"
    );
    eprintln!("OK: Rust dieu khien Camoufox mo example.com + doc title thanh cong.");
}

/// Gửi một lệnh `BiDi` rồi đọc tới khi nhận message có `id` khớp (bỏ qua event xen giữa). Panic nếu
/// phản hồi mang `error` hoặc hết giờ.
async fn cmd<S>(ws: &mut S, id: u64, method: &str, params: Value) -> Value
where
    S: SinkExt<Message>
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
    <S as futures::Sink<Message>>::Error: std::fmt::Debug,
{
    send(ws, id, method, params).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let next = tokio::time::timeout_at(deadline, ws.next())
            .await
            .unwrap_or_else(|_| panic!("het gio cho phan hoi BiDi cho id={id} ({method})"));
        let Some(Ok(msg)) = next else {
            panic!("ket noi BiDi dong truoc khi co phan hoi id={id} ({method})");
        };
        let text = match msg {
            Message::Text(t) => t.as_str().to_owned(),
            Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
            Message::Close(_) => panic!("BiDi dong ket noi (id={id}, {method})"),
            _ => continue,
        };
        let v: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Event (khong co `id`) hoac phan hoi cua lenh khac -> bo qua, doc tiep.
        if v.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(err) = v.get("error") {
            panic!("BiDi loi cho {method}: {err}");
        }
        return v;
    }
}

async fn send<S>(ws: &mut S, id: u64, method: &str, params: Value)
where
    S: SinkExt<Message> + Unpin,
    <S as futures::Sink<Message>>::Error: std::fmt::Debug,
{
    let payload = json!({ "id": id, "method": method, "params": params }).to_string();
    ws.send(Message::text(payload))
        .await
        .expect("gui lenh BiDi");
}
