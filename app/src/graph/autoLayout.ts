// Tự sắp xếp node trên canvas — THUẦN (không React, không store), nên test được trực tiếp.
//
// Cách xếp: theo TẦNG từ trái sang phải đúng chiều dữ liệu chảy. Node không có đầu vào nằm tầng 0;
// mỗi node còn lại nằm ở tầng = (tầng sâu nhất trong các node cấp nguồn) + 1. Trong một tầng, các
// node xếp dọc theo thứ tự xuất hiện.
//
// Vì sao không dùng thư viện (dagre/elk): đồ thị ở đây nhỏ (vài chục node) và LUÔN chảy một chiều
// trái→phải, nên xếp theo tầng là đủ và đọc được. Kéo thêm một thư viện layout ~100KB cho việc này
// là cái giá không đáng.

import type { RFEdge, RFNode } from "./adapter";

/** Khoảng cách giữa hai tầng (ngang) và giữa hai node cùng tầng (dọc). */
const GAP_X = 320;
const GAP_Y = 190;

/** Góc trên trái của toàn bộ bố cục. */
const ORIGIN = { x: 80, y: 80 };

/**
 * Tầng của từng node.
 *
 * Đồ thị có CHU TRÌNH thì không có thứ tự tô-pô; thay vì bỏ cuộc (hoặc lặp vô hạn), node nào chưa
 * xếp được sau khi đã duyệt hết sẽ bị dồn vào tầng cuối. Người dùng vẫn nhận được một bố cục dùng
 * được thay vì một nút bấm không làm gì.
 */
export function layerOf(nodes: readonly RFNode[], edges: readonly RFEdge[]): Map<string, number> {
  const ids = new Set(nodes.map((n) => n.id));
  // Chỉ giữ cạnh mà CẢ HAI đầu còn tồn tại — cạnh mồ côi làm lệch bậc vào, xếp sai cả hình.
  const real = edges.filter((e) => ids.has(e.source) && ids.has(e.target));

  const incoming = new Map<string, string[]>();
  const outgoing = new Map<string, string[]>();
  for (const id of ids) {
    incoming.set(id, []);
    outgoing.set(id, []);
  }
  for (const e of real) {
    incoming.get(e.target)!.push(e.source);
    outgoing.get(e.source)!.push(e.target);
  }

  const layer = new Map<string, number>();
  // Kahn: node đã hết cạnh vào chưa xử lý thì xếp được.
  const remaining = new Map<string, number>();
  const queue: string[] = [];
  for (const n of nodes) {
    const deg = incoming.get(n.id)!.length;
    remaining.set(n.id, deg);
    if (deg === 0) {
      layer.set(n.id, 0);
      queue.push(n.id);
    }
  }

  while (queue.length > 0) {
    const id = queue.shift()!;
    const here = layer.get(id) ?? 0;
    for (const next of outgoing.get(id)!) {
      layer.set(next, Math.max(layer.get(next) ?? 0, here + 1));
      const left = (remaining.get(next) ?? 1) - 1;
      remaining.set(next, left);
      if (left === 0) {
        queue.push(next);
      }
    }
  }

  // Còn sót = nằm trong chu trình. Dồn ra tầng sau cùng để vẫn nhìn thấy được.
  const maxLayer = layer.size === 0 ? 0 : Math.max(...layer.values());
  for (const n of nodes) {
    if (!layer.has(n.id)) {
      layer.set(n.id, maxLayer + 1);
    }
  }
  return layer;
}

/**
 * Vị trí mới cho MỌI node. Trả bản SAO — không sửa mảng đầu vào.
 *
 * Giữ nguyên mọi thứ khác của node (kích thước, dữ liệu, trạng thái chọn); chỉ đổi `position`.
 */
export function autoLayout(nodes: readonly RFNode[], edges: readonly RFEdge[]): RFNode[] {
  if (nodes.length === 0) {
    return [];
  }
  const layer = layerOf(nodes, edges);

  // Đếm chỗ đã dùng của từng tầng, theo THỨ TỰ XUẤT HIỆN trong mảng — bố cục ổn định: bấm hai lần
  // liên tiếp cho ra đúng một kết quả.
  const usedInLayer = new Map<number, number>();
  return nodes.map((n) => {
    const col = layer.get(n.id) ?? 0;
    const row = usedInLayer.get(col) ?? 0;
    usedInLayer.set(col, row + 1);
    return {
      ...n,
      position: { x: ORIGIN.x + col * GAP_X, y: ORIGIN.y + row * GAP_Y },
    };
  });
}
