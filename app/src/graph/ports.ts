// Bang cong/kieu/mau/nhan tieng Viet theo `NodeKind` — ghim boi test cau truc voi
// `src/ipc/fixtures/ports.json` (sinh tu `NodeKind::inputs()/outputs()` that o
// crates/lvc-core/src/graph/node.rs, xem `tests/ipc_fixtures.test.ts`). Doi Rust ma khong doi o
// day -> test do fail, day chinh la muc dich (khong de UI va Rust lech cong ngam).
import { NODE_KIND_ORDER, type NodeKindTag, type PortSpec, type PortType } from "../ipc/types";

/** Cong vao/ra tinh cua tung `NodeKind` — khop dung `fixtures/ports.json`. */
export const PORTS: Record<NodeKindTag, { inputs: PortSpec[]; outputs: PortSpec[] }> = {
  prompt: {
    inputs: [],
    outputs: [{ name: "out", type: "text" }],
  },
  image_upload: {
    inputs: [],
    outputs: [{ name: "out", type: "image" }],
  },
  image_from_url: {
    inputs: [{ name: "url", type: "text" }],
    outputs: [{ name: "out", type: "image" }],
  },
  image_gen: {
    inputs: [
      { name: "prompt", type: "text" },
      { name: "ref", type: "image" },
    ],
    outputs: [{ name: "out", type: "image" }],
  },
  video: {
    inputs: [
      { name: "prompt", type: "text" },
      { name: "refs", type: "image" },
    ],
    outputs: [{ name: "out", type: "video" }],
  },
  remove_logo: {
    inputs: [{ name: "in", type: "video" }],
    outputs: [{ name: "out", type: "video" }],
  },
  ensure_h264: {
    inputs: [{ name: "in", type: "video" }],
    outputs: [{ name: "out", type: "video" }],
  },
  concat: {
    inputs: [{ name: "in", type: "video" }],
    outputs: [{ name: "out", type: "video" }],
  },
  save: {
    inputs: [
      { name: "video", type: "video" },
      { name: "image", type: "image" },
    ],
    outputs: [],
  },
};

/** Mau moi kieu cong (tay cam React Flow, duong noi) — dung nhat quan cho ca 3 kieu du lieu. */
export const PORT_TYPE_COLOR: Record<PortType, string> = {
  text: "#0ea5e9", // sky-500 — Text
  image: "#a855f7", // purple-500 — Image
  video: "#f97316", // orange-500 — Video
};

/**
 * Nhan tieng Viet hien tren node — khop CHINH XAC `NodeKind::label_vi()`
 * (crates/lvc-core/src/graph/node.rs), khong phai ban nhan cu cua brief Task 11 (xem Concerns
 * trong task-11-report.md: hai nguon tung lech nhau 5/9 chuoi). Task 12 ruling: Rust la nguon
 * su that vi `label_vi()` la ham cong khai duy nhat that su dinh nghia nhan node — pin boi
 * `tests/labels.test.ts`.
 */
export const NODE_LABEL_VI: Record<NodeKindTag, string> = {
  prompt: "Prompt",
  image_upload: "Tải ảnh lên",
  image_from_url: "Lấy ảnh từ URL",
  image_gen: "Tạo ảnh",
  video: "Tạo video",
  remove_logo: "Xoá logo",
  ensure_h264: "Chuẩn H.264",
  concat: "Ghép phân cảnh",
  save: "Lưu",
};

/** `NodeKind` duoc P3 dung tren canvas (spec §4.2: 4 loai; phan con lai doi sang P4). */
export const SUPPORTED_IN_P3: ReadonlySet<NodeKindTag> = new Set<NodeKindTag>([
  "prompt",
  "image_upload",
  "video",
  "save",
]);

/** Kieu hep — 4 `NodeKindTag` co the tha vao canvas o P3 (Task 12: Palette/`store/graph.ts`). */
export type AddableNodeKind = "prompt" | "image_upload" | "video" | "save";

export { NODE_KIND_ORDER };
