//! `lvc-github` — GHI lên kho cộng đồng GitHub (đối xứng với `lvc-template-registry` chỉ ĐỌC).
//!
//! Dùng cho "Publish workflow lên GitHub": thêm/sửa một mục trong `index.json` của repo registry
//! rồi mở **Pull Request** — KHÔNG đẩy thẳng vào nhánh chính, để chủ repo duyệt trước.
//!
//! ## Bí mật
//! Token do NGƯỜI DÙNG dán (personal access token của chính họ), lưu mã hoá trong `lvc-vault` như
//! API key. Ở crate này token chỉ đi qua THAM SỐ hàm — không lưu vào struct, không log, không bao
//! giờ nằm trong thông điệp lỗi ([`GithubError`] cắt ngắn thân phản hồi và không in header).
//!
//! Chia module: [`error`] · [`model`] (kiểu thuần) · [`client`] (REST + điều phối tạo PR).

pub mod client;
pub mod error;
pub mod model;

pub use client::{GithubClient, RepoRef};
pub use error::GithubError;
pub use model::{PublishRequest, PublishResult};
