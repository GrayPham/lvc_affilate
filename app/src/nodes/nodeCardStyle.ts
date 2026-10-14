// Lop viền dùng chung cho node trên canvas (MediaCard + NodeShell) — phản ánh KẾT QUẢ CHẠY để
// debug bằng mắt: node lỗi -> viền ĐỎ, xong -> viền XANH LÁ, đang chạy -> viền XANH DƯƠNG, đã huỷ ->
// viền HỔ PHÁCH. Lỗi kiểm hợp lệ (nodeErrors) cũng viền đỏ. Ưu tiên: lỗi kiểm/lỗi chạy > xong >
// đang chạy > huỷ > đang chọn > mặc định — kết quả chạy luôn thấy được, "đang chọn" chỉ tô khi node
// chưa từng chạy (khi đã chạy, node vẫn nhận biết đang chọn qua thanh công cụ + bảng nhật ký).
import type { NodeRunState } from "../store/run";

export function nodeBorderClass(opts: {
  hasErrors: boolean;
  selected: boolean;
  runState?: NodeRunState;
}): string {
  const st = opts.runState?.state;
  if (opts.hasErrors || st === "failed") {
    return "border-red-500/80 ring-1 ring-red-500/40";
  }
  if (st === "done") {
    return "border-emerald-500/70 ring-1 ring-emerald-500/30";
  }
  if (st === "running") {
    return "border-sky-500/70 ring-1 ring-sky-500/30";
  }
  if (st === "cancelled") {
    return "border-amber-500/70 ring-1 ring-amber-500/30";
  }
  if (opts.selected) {
    return "border-sky-400/80 ring-1 ring-sky-400/40";
  }
  return "border-line";
}


// ---------------------------------------------------------------------------------------------
// Màu theo LOẠI node — để nhìn canvas là biết mỗi node làm gì, không phải đọc từng nhãn.
//
// Thân thẻ giữ trung tính (chữ phải dễ đọc); màu chỉ nằm ở VIỀN TRÊN + chip nhãn. Dùng đúng bộ
// `--tag-*` mà bảng Tài khoản/thẻ workflow đang dùng, nên một loại node chỉ có MỘT màu trong cả
// app — học một lần dùng mọi nơi.
// ---------------------------------------------------------------------------------------------

export type NodeTone = "video" | "image" | "script" | "audio" | "io" | "neutral";

/** `kind` của node -> tông màu. Kind lạ (node mới chưa khai) -> `neutral`, không đoán bừa. */
export function toneForKind(kind: string): NodeTone {
  if (kind.includes("video") && kind !== "load_video") {
    return "video";
  }
  if (kind.includes("image")) {
    return "image";
  }
  if (kind.startsWith("script") || kind === "prompt" || kind === "note") {
    return "script";
  }
  if (kind.startsWith("audio") || kind === "tts") {
    return "audio";
  }
  if (kind === "save" || kind === "load_video" || kind === "concat" || kind === "trim" || kind === "frame_capture") {
    return "io";
  }
  return "neutral";
}

/** Lớp cho VIỀN TRÊN của thẻ (dải màu 3px). */
export function toneEdgeClass(tone: NodeTone): string {
  switch (tone) {
    case "video":
      return "bg-tag-video";
    case "image":
      return "bg-tag-image";
    case "script":
      return "bg-tag-script";
    case "audio":
      return "bg-rest";
    case "io":
      return "bg-ink3";
    default:
      return "bg-line2";
  }
}

/** Lớp cho CHIP nhãn ở đầu thẻ (nền mờ + chữ cùng tông). */
export function toneChipClass(tone: NodeTone): string {
  switch (tone) {
    case "video":
      return "bg-tag-video/12 text-tag-video";
    case "image":
      return "bg-tag-image/12 text-tag-image";
    case "script":
      return "bg-tag-script/12 text-tag-script";
    case "audio":
      return "bg-rest/12 text-rest";
    case "io":
      return "bg-fill text-ink2";
    default:
      return "bg-fill text-ink2";
  }
}
