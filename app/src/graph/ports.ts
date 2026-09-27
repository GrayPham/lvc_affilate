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
  load_video: {
    inputs: [],
    outputs: [{ name: "out", type: "video" }],
  },
  dola_video: {
    inputs: [
      { name: "prompt", type: "text" },
      { name: "refs", type: "image" },
    ],
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
      { name: "audio", type: "audio" },
    ],
    outputs: [],
  },
  trim: {
    inputs: [{ name: "in", type: "video" }],
    outputs: [{ name: "out", type: "video" }],
  },
  frame_capture: {
    inputs: [{ name: "in", type: "video" }],
    outputs: [{ name: "out", type: "image" }],
  },
  audio_extract: {
    inputs: [{ name: "in", type: "video" }],
    outputs: [{ name: "out", type: "audio" }],
  },
  audio_split: {
    inputs: [{ name: "in", type: "video" }],
    outputs: [
      { name: "video", type: "video" },
      { name: "audio", type: "audio" },
    ],
  },
};

/** Mau moi kieu cong (tay cam React Flow, duong noi) — dung nhat quan cho moi kieu du lieu. */
export const PORT_TYPE_COLOR: Record<PortType, string> = {
  text: "#0ea5e9", // sky-500 — Text
  image: "#a855f7", // purple-500 — Image
  video: "#f97316", // orange-500 — Video
  audio: "#22c55e", // green-500 — Audio (audio_extract/audio_split/save)
};

/**
 * Nhan tieng Viet hien tren node — khop CHINH XAC `labelVi` cua manifest node (P4a,
 * `fixtures/catalog.json` / lenh `node_catalog`). Rust la nguon su that — pin boi
 * `tests/labels.test.ts` + `tests/ipc_fixtures.test.ts`.
 */
export const NODE_LABEL_VI: Record<NodeKindTag, string> = {
  prompt: "Prompt",
  image_upload: "Tải ảnh lên",
  load_video: "Tải video lên",
  dola_video: "Tạo video Dola",
  concat: "Ghép phân cảnh",
  save: "Lưu",
  trim: "Cắt video",
  frame_capture: "Chụp khung hình",
  audio_extract: "Tách audio",
  audio_split: "Tách audio↔video",
};

/** `NodeKind` co the them vao canvas tu Palette — P4a: ca 4 kind con lai deu them duoc. */
export const ADDABLE_KINDS: ReadonlySet<NodeKindTag> = new Set<NodeKindTag>(NODE_KIND_ORDER);

/** Kieu hep — cac `NodeKindTag` co the tha vao canvas (Palette/`store/graph.ts`). */
export type AddableNodeKind = NodeKindTag;

export { NODE_KIND_ORDER };
