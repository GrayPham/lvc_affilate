// Kiểm hợp lệ phía client cho form Cài đặt — THUẦN, không React, để test trực tiếp.
//
// Đây là BẢN GƯƠNG của hai điều kiện server CHẮC CHẮN sẽ bác (`lvc_core::settings::parse_threads`
// và `parse_duration`), chỉ để UX mượt: chặn tự lưu và hiện lỗi ngay thay vì chờ server trả về.
// Nguồn sự thật cuối cùng vẫn là `AppSettings::from_map` phía Rust — nó tự chuẩn hoá mọi giá trị
// sai rồi trả `warnings`, nên hàm này KHÔNG cần (và không nên) kiểm lại mọi trường.
//
// Hai thư mục (tài khoản/đầu ra) KHÔNG bị bắt buộc có giá trị: rỗng là HỢP LỆ, nghĩa là "để app tự
// chọn" (`lvc_engine::resolve_dir` — rỗng sau trim → `app_data_dir/accounts` hoặc
// `app_data_dir/downloads`), không phải lỗi.
import { allowedDurations } from "../../graph/rules";
import { THREADS_MAX, THREADS_MIN } from "../../store/settings";
import type { AppSettings } from "../../ipc/types";

/** Danh sách lỗi hiển thị; rỗng = đủ điều kiện gọi `settingsSet`. */
export function validateSettings(draft: AppSettings): string[] {
  const errors: string[] = [];
  if (
    !Number.isInteger(draft.threads) ||
    draft.threads < THREADS_MIN ||
    draft.threads > THREADS_MAX
  ) {
    errors.push(`Số luồng phải là số nguyên từ ${THREADS_MIN} đến ${THREADS_MAX}.`);
  }
  const allowed = allowedDurations(draft.defaultMode, draft.defaultModel);
  if (!allowed.includes(draft.defaultDuration)) {
    errors.push(
      `Thời lượng mặc định ${draft.defaultDuration}s không hợp lệ với chế độ/model đã chọn (cho phép ${allowed.join(", ")}s).`,
    );
  }
  return errors;
}
