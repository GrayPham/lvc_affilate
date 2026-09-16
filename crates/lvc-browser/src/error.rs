//! Loi cua lvc-browser (ADR-R002): bao boc loi CDP/I-O/JSON thanh thong diep tieng Viet.

#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("khong khoi dong duoc trinh duyet: {0}")]
    Launch(String),
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
}
