//! `lvc-template-registry` — provider đọc kho template CỘNG ĐỒNG (P7): một file tĩnh `index.json`
//! lưu trên GitHub (public, không token), host miễn phí qua `raw.githubusercontent.com`.
//!
//! Kiến trúc (ADR-R014 style, xem `docs/superpowers/specs/2026-10-11-remote-template-registry-design.md`):
//! client HTTP thật ([`client::RegistryClient`]) KHÔNG biết khái niệm node/scope của app — `lvc-app`
//! gọi trực tiếp (không qua `ctx.services`/host trait như provider AI, vì đây không phải node chạy
//! trong DAG, chỉ là một lệnh IPC đơn giản giống cách `lvc-app` gọi `lvc-store::TemplateRepo`).
//!
//! Chia module: [`error`] · [`model`] (kiểu dữ liệu thuần, `graph` giữ `serde_json::Value`) ·
//! [`client`] (fetch + điều phối).

pub mod client;
pub mod error;
pub mod model;

pub use client::{DEFAULT_INDEX_URL, RegistryClient};
pub use error::RegistryError;
pub use model::{ChangeEntry, RemoteTemplate};
