//! `lvc-scripts` — hợp đồng dịch vụ LƯU kịch bản (không phải provider AI).
//!
//! Node `script_gen` sinh kịch bản nhiều cảnh trong MỘT LẦN CHẠY workflow (kết quả chỉ tồn tại
//! tạm trong `Outputs`, trôi xuống node kế tiếp rồi mất — không phải một dòng trong kho "Kịch bản"
//! trừ khi người dùng tự tạo/dán tay). Khi tham số `save_as_script` của node được bật, `run()`
//! lấy dịch vụ [`ScriptSinkHost`] qua `ctx.services` để lưu kết quả đó thành một kịch bản mới
//! trong kho (bảng `scripts`/`scenes`, xem `lvc-store::scripts`), KHÔNG tự phụ thuộc `lvc-store`/
//! `sqlx` (vi phạm luật "node độc lập", ADR-R014).
//!
//! Provider KHÔNG biết khái niệm node. Engine impl [`ScriptSinkHost`] (xem
//! `crates/lvc-engine/src/scriptsink.rs`) dùng `lvc_store::ScriptRepo`.

use async_trait::async_trait;

/// Kịch bản nháp cần lưu — `scenes[i]` là MỘT dòng/cảnh, cùng thứ tự node đã sinh ra.
#[derive(Debug, Clone)]
pub struct ScriptDraft {
    pub name: String,
    pub scenes: Vec<String>,
}

/// Lỗi lưu kịch bản — KHÔNG phải lỗi sinh text (đã thành công trước đó), chỉ là lỗi lưu kho.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ScriptSinkError {
    #[error("không lưu được kịch bản: {0}")]
    Save(String),
}

/// Dịch vụ lưu kịch bản do host (engine) cấp cho node qua `NodeCtx::services`. Node khai kiểu
/// `Arc<dyn ScriptSinkHost>` mà không phụ thuộc engine/store.
#[async_trait]
pub trait ScriptSinkHost: Send + Sync {
    /// Lưu `draft` thành một kịch bản MỚI; trả về id kịch bản vừa tạo.
    ///
    /// # Errors
    /// [`ScriptSinkError::Save`] khi ghi kho thất bại.
    async fn save_script(&self, draft: ScriptDraft) -> Result<String, ScriptSinkError>;
}
