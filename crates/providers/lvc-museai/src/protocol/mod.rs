//! Giao thức Muse Hatch/Noise — port từ `E:\ProjectAILVC\Lvc_veo3_automation\src\core\museai\
//! protocol\` (tham khảo, CHỈ ĐỌC). Chia theo đúng cách file gốc chia (một file = một trách
//! nhiệm): [`noise`] (bắt tay mã hoá, crypto chuẩn công khai) · [`wire`] (codec protobuf tối
//! giản) · [`errors`] (kiểu lỗi) · [`transport`] (kết nối RPC qua WebSocket + Noise, KHÔNG giả
//! mạo TLS fingerprint — xem `CLAUDE.md` "bối cảnh rủi ro") · [`auth`] (struct `MuseBootstrap`,
//! THUẦN) · [`media`] (trích xuất `VideoRef`/session id từ JSON, THUẦN) · [`filesystem`] (tải file
//! qua `fs.*`/`idea-cards`).

pub mod attachments;
pub mod auth;
pub mod client;
pub mod errors;
pub mod filesystem;
pub mod media;
pub mod noise;
pub mod transport;
pub mod wire;
