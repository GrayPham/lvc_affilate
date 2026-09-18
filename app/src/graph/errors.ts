// Doi mot `GraphError` (crates/lvc-core/src/graph/validate.rs, xem `src/ipc/types.ts`) sang mot
// dong tieng Viet ngan de hien trong RunBar (danh sach loi sau "Kiểm tra") va tooltip badge loi
// tren tung node (NodeShell.tsx).
import type { GraphError } from "../ipc/types";

export function formatGraphError(err: GraphError): string {
  switch (err.type) {
    case "duplicate_node_id":
      return `Trùng id node: ${err.id}`;
    case "unknown_node":
      return `Có cạnh trỏ tới node không tồn tại: ${err.id}`;
    case "unknown_port":
      return `Node ${err.node} không có cổng "${err.port}"`;
    case "type_mismatch":
      return `Cạnh ${err.edge} sai kiểu: ${err.from} → ${err.to}`;
    case "missing_required_input":
      return `Node ${err.node} thiếu đầu vào bắt buộc ở cổng "${err.port}"`;
    case "cycle":
      return `Chu trình khép kín: ${err.nodes.join(" → ")}`;
    case "unsupported":
      return `Node ${err.node} (loại "${err.kind}") chưa hỗ trợ ở P3`;
    case "empty":
      return "Canvas trống — hãy thêm node";
    case "multiple_inputs_on_single_port":
      return `Cổng "${err.port}" trên node ${err.node} chỉ nhận 1 cạnh, đang có nhiều hơn`;
    case "scene":
      return `Node ${err.node}${err.line !== null ? ` (dòng ${err.line})` : ""}: ${err.message}`;
    case "too_many_refs":
      return `Node ${err.node}, cảnh ${err.sceneIndex}: quá nhiều ảnh tham chiếu (${err.count} > ${err.max})`;
    default:
      return err satisfies never;
  }
}
