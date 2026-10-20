//! `lvc-license`: cổng bản quyền LVC Shop (port 1:1 từ LVC Veo3). Đăng nhập tài khoản → kích hoạt
//! license; verify phản hồi server bằng ECDSA P-256 (4 lớp guard); ECDH/HKDF/Fernet cho payload gửi
//! lên; session cache mã hoá để auto-login offline tới khi hết hạn.
//!
//! Chia module theo trách nhiệm (luật CLAUDE.md): [`verify`] (THUẦN: canonical JSON + chữ ký + 4 lớp
//! guard), `crypto` (ECDH/HKDF/Fernet), `device` (`device_id`/info), `client` (HTTP), `session` (cache).
//!
//! LUẬT: KHÔNG log bí mật (password, license key, JWT, session plaintext, Fernet key).

pub mod client;
pub mod crypto;
pub mod device;
pub mod session;
pub mod verify;
