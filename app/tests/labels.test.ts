// Task 12 ruling (xem Concerns task-11-report.md): `NODE_LABEL_VI` phai khop CHINH XAC
// `NodeKind::label_vi()` that (crates/lvc-core/src/graph/node.rs) — khong dung bang nhan cu cua
// brief Task 11 (tung lech 5/9 chuoi). Test nay ghim danh sach do tai bien gioi TS, doc lap voi
// Rust (khong the chay cargo test tu vitest) nhung sao chep dung tung ky tu tu doc comment/test
// `label_vi_tieng_viet_dung_bang_spec` cua node.rs.
import { describe, expect, it } from "vitest";

import { NODE_LABEL_VI } from "../src/graph/ports";
import { NODE_KIND_ORDER } from "../src/ipc/types";

/** Sao chep dung tung ky tu tu `NodeKind::label_vi()` (node.rs). */
const RUST_LABEL_VI: Record<string, string> = {
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

describe("NODE_LABEL_VI khop dung NodeKind::label_vi() (Rust)", () => {
  it("bao phu du 9 NodeKindTag, khong sot khong thua", () => {
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
