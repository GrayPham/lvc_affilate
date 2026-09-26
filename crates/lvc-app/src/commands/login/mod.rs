//! Miền "đăng nhập nền tảng" (A + B, đường HỢP LỆ) — chia module theo LUẬT (CLAUDE.md): mỗi việc/nền
//! tảng một file nhỏ, `mod.rs` chỉ ghép lại.
//!
//! - [`platform`]: bảng nền tảng + URL đăng nhập (thuần, test được) — dùng cho B.
//! - [`cookie`]: (A) nạp cookie NHIỀU FILE vào kho theo nền tảng — người dùng mang phiên đã đăng nhập.
//! - [`interactive`]: (B) đăng nhập THỦ CÔNG — mở cửa sổ trình duyệt THẬT tới trang đăng nhập để
//!   NGƯỜI DÙNG tự đăng nhập, rồi lưu credential `session` tham chiếu profile bền vững.
//!
//! RANH GIỚI (xem MEMORY `p4b-no-antibot-signing` + `module-split-rule`): KHÔNG tự gõ mật khẩu, KHÔNG
//! tự tính TOTP/2FA, KHÔNG né phát hiện bot. B (đăng nhập thủ công) CHỈ mở cửa sổ trình duyệt để
//! NGƯỜI DÙNG tự đăng nhập rồi lưu profile — không port flow tự động của bản Python.

pub mod cookie;
pub mod interactive;
pub mod platform;
