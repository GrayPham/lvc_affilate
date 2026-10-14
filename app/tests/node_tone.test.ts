// `toneForKind` (src/nodes/nodeCardStyle.ts) — mau tren the node PHAI mang thong tin: cung mot
// loai viec thi cung mot mau, trong CA app (bang Tai khoan, the workflow, node tren canvas).
import { describe, expect, it } from "vitest";

import { toneChipClass, toneEdgeClass, toneForKind } from "../src/nodes/nodeCardStyle";

describe("toneForKind", () => {
  it("moi node tao VIDEO deu cung mot tong", () => {
    for (const k of ["dola_video", "veo3_video", "google_video", "seedance_video", "museai_video"]) {
      expect(toneForKind(k)).toBe("video");
    }
  });

  it("moi node tao ANH deu cung mot tong", () => {
    for (const k of ["veo3_image", "google_image", "grok_image", "seedance_image", "museai_image", "image_upload"]) {
      expect(toneForKind(k)).toBe("image");
    }
  });

  it("node CHU (prompt/kich ban/ghi chu) cung tong", () => {
    for (const k of ["prompt", "script_gen", "script_agent", "note"]) {
      expect(toneForKind(k)).toBe("script");
    }
  });

  it("node AM THANH cung tong", () => {
    for (const k of ["tts", "audio_extract", "audio_split"]) {
      expect(toneForKind(k)).toBe("audio");
    }
  });

  it("node vao/ra va xu ly file cung tong trung tinh hon", () => {
    for (const k of ["save", "load_video", "concat", "trim", "frame_capture"]) {
      expect(toneForKind(k)).toBe("io");
    }
  });

  it("`load_video` la VAO/RA chu khong phai node tao video", () => {
    expect(toneForKind("load_video")).toBe("io");
    expect(toneForKind("load_video")).not.toBe("video");
  });

  it("kind la (node moi chua khai) -> trung tinh, khong doan bua", () => {
    expect(toneForKind("kind_chua_biet")).toBe("neutral");
    expect(toneForKind("")).toBe("neutral");
  });
});

describe("lop mau", () => {
  it("moi tong ra mot lop khac nhau, khong tong nao rong", () => {
    const tones = ["video", "image", "script", "audio", "io", "neutral"] as const;
    const edges = tones.map(toneEdgeClass);
    const chips = tones.map(toneChipClass);
    for (const c of [...edges, ...chips]) {
      expect(c.trim()).not.toBe("");
    }
    // 5 tong dau phai phan biet duoc voi nhau o vien tren.
    expect(new Set(edges.slice(0, 5)).size).toBe(5);
  });
});
