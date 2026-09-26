//! Miền "đăng nhập nền tảng" (A + B, đường HỢP LỆ) — chia module theo LUẬT (CLAUDE.md): mỗi việc/nền
//! tảng một file nhỏ, `mod.rs` chỉ ghép lại.
//!
//! - [`platform`]: bảng nền tảng + URL đăng nhập (thuần, test được) — dùng cho B.
//! - [`cookie`]: (A) nạp cookie NHIỀU FILE vào kho theo nền tảng — người dùng mang phiên đã đăng nhập.
//!
//! RANH GIỚI (xem MEMORY `p4b-no-antibot-signing` + `module-split-rule`): KHÔNG tự gõ mật khẩu, KHÔNG
//! tự tính TOTP/2FA, KHÔNG né phát hiện bot. B (đăng nhập thủ công) sẽ mở cửa sổ trình duyệt để NGƯỜI
//! DÙNG tự đăng nhập rồi lưu profile — thêm ở bước sau (cần phương thức mở trình duyệt của engine).

pub mod cookie;
pub mod platform;
