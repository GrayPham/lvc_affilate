//! Hình dạng một mục trong `index.json` của registry — xem `CLAUDE.md` mục hợp đồng dữ liệu.
//! `graph` giữ nguyên `serde_json::Value` (provider KHÔNG phụ thuộc `lvc-core`); `lvc-app` tự parse
//! thành `Graph` + `migrate` sau khi nhận từ client.
//!
//! MỘT `index.json` chứa CẢ HAI loại mục (workflow template VÀ kịch bản affiliate) — phân biệt
//! bằng [`RemoteTemplate::kind`] (`"workflow"` mặc định khi thiếu, giữ tương thích ngược với
//! `index.json` đã publish trước khi trường này tồn tại; `"script"` cho kịch bản). Script dùng
//! `scenes` (mỗi dòng một cảnh) thay cho `graph` — caller (`lvc-app::commands`) tự lọc đúng `kind`
//! mình cần, bỏ qua mục còn lại.

use serde::{Deserialize, Serialize};

fn default_kind() -> String {
    "workflow".to_string()
}

/// Một mục cộng đồng (template HOẶC kịch bản), như trả về từ `index.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RemoteTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    /// URL tuyệt đối tới ảnh thumbnail (hoặc rỗng nếu chưa có). Script không dùng trường này.
    #[serde(default)]
    pub thumbnail: String,
    #[serde(default)]
    pub updated_at: f64,
    /// `"workflow"` (mặc định) | `"script"`.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Bắt buộc khi `kind="workflow"`, vắng mặt khi `kind="script"`.
    #[serde(default)]
    pub graph: Option<serde_json::Value>,
    /// Bắt buộc khi `kind="script"` (mỗi phần tử một cảnh), vắng mặt khi `kind="workflow"`.
    #[serde(default)]
    pub scenes: Option<Vec<String>>,
    /// Phiên bản (`"1.0.1"`). RỖNG với mục đã publish trước khi trường này tồn tại — khi đó app
    /// không thể biết có bản mới hay không, nên coi như "không có cập nhật" (xem
    /// `lvc-app::commands::updates`).
    #[serde(default)]
    pub version: String,
    /// Tên tác giả để hiện trên hộp thoại cập nhật; rỗng = không hiện.
    #[serde(default)]
    pub author: String,
    /// Những gì đổi ở phiên bản này. Rỗng = tác giả không ghi gì (hộp thoại vẫn cập nhật được,
    /// chỉ không có danh sách thay đổi để đọc).
    #[serde(default)]
    pub changelog: Vec<ChangeEntry>,
}

/// Một dòng trong danh sách thay đổi.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChangeEntry {
    /// Loại thay đổi, tự do nhưng thực tế là `"Thêm"`/`"Sửa"`/`"Bỏ"` — hiện thành nhãn nhỏ.
    pub kind: String,
    /// Mô tả một câu.
    pub text: String,
}
