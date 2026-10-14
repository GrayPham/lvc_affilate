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
