// Thông tin liên hệ hỗ trợ hiện ở màn khoá bản quyền (và dùng lại được ở Cài đặt sau này).
//
// Nơi DUY NHẤT giữ các kênh hỗ trợ — đổi nhóm/đổi link thì sửa ở đây, không rải vào component.
// Mục `url: null` chỉ hiện chữ, không bấm được (dùng khi chỉ có số điện thoại, chưa có link).
export type ContactLink = {
  /** Tên kênh, hiện nguyên văn (không dịch — "Zalo", "Telegram"…). */
  readonly label: string;
  /** Mô tả ngắn, hiện khi không có link và làm tooltip khi có link. */
  readonly value: string;
  /** Mở bằng trình duyệt ngoài; `null` = chưa có liên kết. */
  readonly url: string | null;
};

/** Trang đăng ký tài khoản (người dùng cung cấp 2026-10-21). */
export const REGISTER_URL = "https://marketmmoai.com/register";

/** Chân trang pháp lý hiện ở màn khoá. `labelKey` là khoá i18n (xem `i18n/vi.ts`). */
export const LEGAL_LINKS = [
  { labelKey: "legal.terms", url: "https://marketmmoai.com/terms-of-service" },
  { labelKey: "legal.privacy", url: "https://marketmmoai.com/privacy-policy" },
  { labelKey: "legal.support", url: "https://marketmmoai.com/support" },
] as const;

export const SUPPORT_CONTACTS: readonly ContactLink[] = [
  { label: "Zalo", value: "Nhóm hỗ trợ Zalo", url: "https://zalo.me/g/ovvcol530" },
  { label: "Telegram", value: "Nhóm hỗ trợ Telegram", url: "https://t.me/+vA2tu15gJPA3MmU1" },
];
