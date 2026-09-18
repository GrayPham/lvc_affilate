// Dinh dang tieng Viet dung cho man "Tác vụ" (Task 13): nhan trang thai/mode, nhan ma loi
// (FailureCode, crates/lvc-core/src/failure.rs — 15 gia tri, xem doc module do), nhan stage
// (fixtures/stages.json, sinh tu STAGE_LAUNCHING..STAGE_DONE cua lvc-engine), dieu kien "tiep tuc
// duoc khong" (mirror dieu kien chan trong `task_resume_impl`,
// crates/lvc-app/src/commands/tasks.rs), va dinh dang thoi gian da troi qua (ms). Moi bang tra cuu
// o day CHI mang tinh hien thi — khong phai nguon su that (server la nguon su that, xem cung mot
// quy uoc voi `graph/rules.ts`/`graph/errors.ts` cua Task 12): ma/stage KHONG nam trong bang van
// hien thi duoc, chi khong co nhan tieng Viet (fallback ve chuoi tho).
import type { TaskDto } from "../ipc/types";

/**
 * Nhan tieng Viet cho `FailureCode::as_str()` (crates/lvc-core/src/failure.rs) — DUNG 15 gia tri
 * (xem `ALL` trong test Rust `round_trip_parse_as_str_cho_ca_muoi_lam_gia_tri`). `null` (task
 * chua/khong loi) -> chuoi rong; ma la nhung khong nam trong 15 gia tri (khong nen xay ra, server
 * la nguon su that) -> tra nguyen ma tho thay vi nem loi, de UI khong vo vi mot ma moi Rust vua
 * them ma UI chua kip cap nhat.
 */
const FAILURE_LABEL_VI: Record<string, string> = {
  cancelled: "Đã huỷ",
  LOGGED_OUT: "Mất đăng nhập",
  "429": "Bị giới hạn tốc độ (429)",
  ACCOUNT_UNAVAILABLE: "Không có tài khoản khả dụng",
  PROMPT_VIOLATION: "Prompt vi phạm chính sách",
  REF_UPLOAD: "Lỗi tải ảnh tham chiếu",
  PROFILE_BUSY: "Hồ sơ trình duyệt đang bận",
  PROXY_DOWN: "Proxy không hoạt động",
  DURATION_UNAVAILABLE: "Thời lượng không khả dụng",
  AGENT_QUESTION: "Agent hỏi lại, cần xử lý thủ công",
  MODE_UNAVAILABLE: "Chế độ không khả dụng",
  NO_CONVERSATION: "Không có hội thoại để tiếp tục",
  RESTART: "Cần khởi động lại",
  RATIO_UNAVAILABLE: "Tỉ lệ khung hình không khả dụng",
  AGENT_NO_VIDEO: "Agent không trả về video",
};

export function failureLabel(code: string | null): string {
  if (code === null) {
    return "";
  }
  return FAILURE_LABEL_VI[code] ?? code;
}

/** Nhan tieng Viet cho tung buoc trong `fixtures/stages.json` (11 gia tri, da khu trung). */
const STAGE_LABEL_VI: Record<string, string> = {
  launching: "Khởi động",
  page: "Mở trang",
  mode: "Chọn chế độ",
  composer: "Mở khung soạn",
  refs: "Tải ảnh tham chiếu",
  prompt: "Nhập prompt",
  sending: "Đang gửi",
  generating: "Đang tạo video",
  confirm: "Xác nhận",
  downloading: "Đang tải xuống",
  done: "Hoàn tất",
};

export function stageLabel(stage: string): string {
  return STAGE_LABEL_VI[stage] ?? stage;
}

/** Nhan tieng Viet cho `TaskDto.status` (`queued`/`processing`/`completed`/`failed`). */
const STATUS_LABEL_VI: Record<string, string> = {
  queued: "Đang chờ",
  processing: "Đang xử lý",
  completed: "Hoàn tất",
  failed: "Thất bại",
};

export function statusLabel(status: string): string {
  return STATUS_LABEL_VI[status] ?? status;
}

/** Nhan tieng Viet ngan cho `TaskDto.mode` (`fast`/`pro`/`pro_max`/`agent`) dung trong bang. */
const MODE_LABEL_VI: Record<string, string> = {
  fast: "Nhanh",
  pro: "Pro",
  pro_max: "Pro Max",
  agent: "Agent",
};

export function modeLabel(mode: string): string {
  return MODE_LABEL_VI[mode] ?? mode;
}

/**
 * `true` khi task co the bam "Tiếp tục" (`task_resume`) — mirror CHINH XAC hai dieu kien dau cua
 * `task_resume_impl` (crates/lvc-app/src/commands/tasks.rs, TRUOC khi cham DB that): trang thai
 * phai `failed` hoac `queued` (khac -> server tra `NOT_RESUMABLE`), VA phai co hoi thoai
 * (`hasConversation`). Day la mirror hien thi (an/tat nut) — server van la nguon su that cuoi
 * cung, mot task co the doi trang thai giua luc UI hien va luc nguoi dung bam nut.
 */
export function isResumable(task: TaskDto): boolean {
  return (task.status === "failed" || task.status === "queued") && task.hasConversation;
}

/** `true` khi task co the bam "Huỷ" (`task_cancel`) — chi con y nghia khi chua ket thuc. */
export function isCancellable(task: TaskDto): boolean {
  return task.status === "queued" || task.status === "processing";
}

/** Dinh dang so mili-giay da troi qua thanh chuoi ngan kieu "1p05s"/"42s" (0 hoac am -> "0s"). */
export function formatElapsed(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const mm = Math.floor(totalSeconds / 60);
  const ss = totalSeconds % 60;
  return mm > 0 ? `${mm}p${ss.toString().padStart(2, "0")}s` : `${ss}s`;
}
