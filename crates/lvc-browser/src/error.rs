//! Loi cua lvc-browser (ADR-R002): bao boc loi CDP/I-O/JSON thanh thong diep tieng Viet.

#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("khong khoi dong duoc trinh duyet: {0}")]
    Launch(String),
    /// I-1: `CdpError::LaunchExit` tach duoc ma thoat + stderr co cau truc (khac `Launch`
    /// von chi con mot chuoi `Display` da render san) khi tien trinh Chrome thoat truoc khi
    /// websocket URL duoc doc xong, MA KHONG phai loi `ProfileBusy` (ma 21/24 — xem `ProfileBusy`).
    #[error("trinh duyet thoat truoc khi ket noi CDP (ma thoat {code:?}): {stderr}")]
    LaunchExit { code: Option<i32>, stderr: String },
    #[error("loi CDP: {0}")]
    Cdp(#[from] chromiumoxide::error::CdpError),
    #[error("profile dang bi tien trinh khac dung: {0}")]
    ProfileBusy(String),
    #[error("loi I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("loi JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("dieu huong that bai toi {url}: {reason}")]
    Navigation { url: String, reason: String },
    /// M-6: het thoi gian cho (vd `goto` khong "load" xong trong 60s).
    #[error("qua thoi gian cho: {0}")]
    Timeout(String),
}
