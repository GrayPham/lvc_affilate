// Từ điển TIẾNG VIỆT — NGUỒN CHÂN LÝ của tập khoá i18n. Thêm khoá ở đây trước, các ngôn ngữ khác
// kiểu `Record<DictKey, string>` nên thiếu khoá là LỖI BIÊN DỊCH (xem `en.ts`).
export const vi = {
  "app.name": "LVC Shop",
  "app.tagline": "Studio video bán hàng",
  "theme.toggle": "Đổi sáng/tối",

  // ── Cổng bản quyền ──
  "gate.title": "Kích hoạt LVC Shop",
  "gate.close": "Đóng",
  "gate.checking": "Đang kiểm tra bản quyền…",
  "gate.step.login": "Đăng nhập",
  "gate.step.activate": "Kích hoạt",
  "gate.login.subtitle": "Đăng nhập tài khoản LVC để tiếp tục",
  "gate.login.username": "Tên đăng nhập",
  "gate.login.password": "Mật khẩu",
  "gate.login.submit": "Đăng nhập",
  "gate.login.submitting": "Đang đăng nhập…",
  "gate.login.remember": "Ghi nhớ tài khoản",
  "gate.login.rememberHint": "Bỏ chọn thì lần mở sau phải đăng nhập lại.",
  "gate.password.show": "Hiện mật khẩu",
  "gate.password.hide": "Ẩn mật khẩu",

  "gate.activate.subtitle": "Nhập mã bản quyền để kích hoạt",
  "gate.activate.key": "Mã giấy phép",
  "gate.activate.placeholder": "XXXX-XXXX-XXXX-XXXX",
  "gate.activate.submit": "Kích hoạt",
  "gate.activate.submitting": "Đang kích hoạt…",
  "gate.activate.hello": "Xin chào {ten}",
  "gate.back": "Quay lại đăng nhập",

  "gate.exit": "Thoát",
  "gate.logout": "Đăng xuất",
  "gate.buy": "Mua giấy phép",
  "gate.contact": "Cần hỗ trợ?",
  "gate.register.prompt": "Chưa có tài khoản?",
  "gate.register.link": "Tạo tài khoản miễn phí",
  "legal.terms": "Điều khoản",
  "legal.privacy": "Bảo mật",
  "legal.support": "Hỗ trợ",
  "legal.version": "Phiên bản {v}",

  // ── Khối minh bạch: máy chủ nhận được gì ──
  "gate.device.label": "Mã máy này",
  "gate.device.unknown": "chưa đọc được",

  // ── Lỗi (khớp mã lỗi IPC từ `commands/license.rs`) ──
  "gate.err.NETWORK": "Không kết nối được máy chủ. Kiểm tra mạng rồi thử lại.",
  "gate.err.BAD_CREDENTIALS": "Sai tên đăng nhập hoặc mật khẩu.",
  "gate.err.VERIFY_FAILED": "Mã bản quyền không hợp lệ hoặc đã dùng trên máy khác.",
  "gate.err.NOT_LOGGED_IN": "Phiên đăng nhập đã hết, vui lòng đăng nhập lại.",
  "gate.err.SESSION_SAVE": "Không ghi được phiên làm việc xuống máy.",
  "gate.err.unknown": "Có lỗi xảy ra, vui lòng thử lại.",
  "gate.err.noBridge": "Đây là bản xem giao diện trong trình duyệt, chưa nối được với app nên không đăng nhập được. Hãy mở app LVC Shop thật.",
  "gate.err.notActivated": "Kích hoạt chưa thành công, kiểm tra lại mã.",

  // ── Chọn ngôn ngữ ──
  "lang.switch": "Ngôn ngữ",
} as const;

/** Tập khoá hợp lệ — suy từ từ điển tiếng Việt. */
export type DictKey = keyof typeof vi;
