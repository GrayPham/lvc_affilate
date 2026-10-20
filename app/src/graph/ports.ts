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
  note: {
    inputs: [],
    outputs: [],
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
  script_gen: {
    inputs: [{ name: "brief", type: "text" }],
    outputs: [{ name: "out", type: "text" }],
  },
  script_agent: {
    inputs: [{ name: "brief", type: "text" }],
    outputs: [{ name: "out", type: "text" }],
  },
  veo3_image: {
    inputs: [
      { name: "prompt", type: "text" },
      { name: "refs", type: "image" },
    ],
    outputs: [{ name: "out", type: "image" }],
  },
  veo3_video: {
    inputs: [
      { name: "prompt", type: "text" },
      { name: "refs", type: "image" },
    ],
    outputs: [{ name: "out", type: "video" }],
  },
  google_image: {
    inputs: [{ name: "prompt", type: "text" }],
    outputs: [{ name: "out", type: "image" }],
  },
  google_video: {
    inputs: [
      { name: "prompt", type: "text" },
      { name: "ref", type: "image" },
    ],
    outputs: [{ name: "out", type: "video" }],
  },
  seedance_image: {
    inputs: [{ name: "prompt", type: "text" }],
    outputs: [{ name: "out", type: "image" }],
  },
  seedance_video: {
    inputs: [
      { name: "prompt", type: "text" },
      { name: "ref", type: "image" },
    ],
    outputs: [{ name: "out", type: "video" }],
  },
  grok_image: {
    inputs: [{ name: "prompt", type: "text" }],
    outputs: [{ name: "out", type: "image" }],
  },
  museai_video: {
    inputs: [
      { name: "prompt", type: "text" },
      // Ảnh tham chiếu dùng chung cho mọi cảnh — giống cổng `refs` của `veo3_video`.
      { name: "refs", type: "image" },
    ],
    outputs: [{ name: "out", type: "video" }],
  },
  museai_image: {
    inputs: [{ name: "prompt", type: "text" }],
    outputs: [{ name: "out", type: "image" }],
  },
  tts: {
    inputs: [{ name: "prompt", type: "text" }],
    outputs: [{ name: "out", type: "audio" }],
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
  note: "Ghi chú",
  image_upload: "Tải ảnh lên",
  load_video: "Tải video lên",
  dola_video: "Tạo video Dola",
  concat: "Nối video (ghép cảnh)",
  save: "Lưu",
  trim: "Cắt video",
  frame_capture: "Chụp khung hình",
  audio_extract: "Tách audio",
  audio_split: "Tách audio↔video",
  script_gen: "Kịch bản AI",
  script_agent: "Agent kịch bản affiliate",
  veo3_image: "Tạo ảnh Veo3 (Google Labs Flow)",
  veo3_video: "Tạo video Veo3",
  google_image: "Tạo ảnh Google (API Imagen)",
  google_video: "Tạo video Google (API Veo)",
  seedance_image: "Tạo ảnh Seedance (API Seedream)",
  seedance_video: "Tạo video Seedance (API)",
  grok_image: "Tạo ảnh Grok (API xAI)",
  museai_video: "Tạo video MuseAI",
  museai_image: "Tạo ảnh MuseAI",
  tts: "Tạo giọng đọc AI (TTS)",
};

/**
 * Nhom (category) cua tung `NodeKind` — khop CHINH XAC `category` cua manifest node (P4a,
 * `fixtures/catalog.json` / lenh `node_catalog`), dung de GOM NHOM danh sach "Thêm node" (Palette)
 * thay vi liet ke phang mot danh sach dai (kho chon khi so luong kind tang theo thoi gian).
 */
export const NODE_CATEGORY_VI: Record<NodeKindTag, string> = {
  prompt: "text",
  note: "text",
  image_upload: "input",
  load_video: "input",
  dola_video: "video",
  concat: "video",
  save: "output",
  trim: "video",
  frame_capture: "image",
  audio_extract: "audio",
  audio_split: "video",
  script_gen: "text",
  script_agent: "text",
  veo3_image: "image",
  veo3_video: "video",
  google_image: "image",
  google_video: "video",
  seedance_image: "image",
  seedance_video: "video",
  grok_image: "image",
  museai_video: "video",
  museai_image: "image",
  tts: "audio",
};

/** Thu tu + nhan tieng Viet cua tung nhom — thu tu nay quyet dinh thu tu hien thi trong Palette
 * (theo mach luong lam viec tu nhien: dau vao -> van ban -> anh -> video -> am thanh -> xuat). */
export const CATEGORY_ORDER: readonly string[] = ["input", "text", "image", "video", "audio", "output"];
export const CATEGORY_LABEL_VI: Record<string, string> = {
  input: "Đầu vào",
  text: "Văn bản",
  image: "Tạo ảnh",
  video: "Tạo / xử lý video",
  audio: "Âm thanh",
  output: "Lưu / xuất",
};

/** Kind co trong catalog nhung CHUA trien khai that (khung rong scripts/new-node.ps1) — khong cho
 * them vao canvas de nguoi dung khong keo phai mot node luon bao "chua cai dat". Bo khoi tap nay
 * ngay khi node co logic that (xem crate CLAUDE.md). */
const NOT_YET_IMPLEMENTED: ReadonlySet<NodeKindTag> = new Set<NodeKindTag>([]);

/** `NodeKind` co the them vao canvas tu Palette. */
export const ADDABLE_KINDS: ReadonlySet<NodeKindTag> = new Set<NodeKindTag>(
  NODE_KIND_ORDER.filter((k) => !NOT_YET_IMPLEMENTED.has(k)),
);

/** Kieu hep — cac `NodeKindTag` co the tha vao canvas (Palette/`store/graph.ts`). */
export type AddableNodeKind = NodeKindTag;

export { NODE_KIND_ORDER };
