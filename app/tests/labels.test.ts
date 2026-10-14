// `NODE_LABEL_VI` phai khop CHINH XAC `labelVi` cua manifest node (P4a, lenh `node_catalog` /
// `src/ipc/fixtures/catalog.json`). Test nay ghim danh sach do tai bien gioi TS; doi chieu truc
// tiep voi catalog.json nam o `tests/ipc_fixtures.test.ts`.
import { describe, expect, it } from "vitest";

import { NODE_LABEL_VI } from "../src/graph/ports";
import { NODE_KIND_ORDER } from "../src/ipc/types";

/** Sao chep dung tung ky tu tu `labelVi` cua manifest node (P4a, `fixtures/catalog.json`). */
const RUST_LABEL_VI: Record<string, string> = {
  prompt: "Prompt",
  note: "Ghi chú",
  image_upload: "Tải ảnh lên",
  load_video: "Tải video lên",
  dola_video: "Tạo video Dola",
  concat: "Ghép phân cảnh",
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

describe("NODE_LABEL_VI khop dung labelVi cua manifest node (Rust, P4a)", () => {
  it("bao phu du NodeKindTag, khong sot khong thua", () => {
    expect(Object.keys(NODE_LABEL_VI).sort()).toEqual([...NODE_KIND_ORDER].sort());
    expect(Object.keys(RUST_LABEL_VI).sort()).toEqual([...NODE_KIND_ORDER].sort());
  });

  it.each(NODE_KIND_ORDER.map((kind) => [kind, RUST_LABEL_VI[kind]] as const))(
    "%s -> %s",
    (kind, want) => {
      expect(NODE_LABEL_VI[kind]).toBe(want);
    },
  );
});
