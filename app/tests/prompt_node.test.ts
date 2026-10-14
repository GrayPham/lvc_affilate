// I2 + M7 (fix-round 1, final-review): `countScenes` (src/nodes/PromptNode.tsx) phai bo qua dong
// TRONG va dong CHU THICH (`#`, sau trim) — dung PHAN TICH nhu `lvc_core::scene_txt::parse_txt`
// (crates/lvc-core/src/scene_txt.rs: "dong trong hoac bat dau bang `#`" deu bi bo qua, khong tinh
// la mot canh) de so "N cảnh" hien tren node Prompt khop dung so canh THAT SU se duoc gui di khi
// bam "Kiem tra"/"Chay", khong dem thua dong chu thich.
import { describe, expect, it } from "vitest";

import { countScenes } from "../src/graph/sceneCount";

describe("countScenes (PromptNode) — khop scene_txt::parse_txt", () => {
  it("dem so dong khong rong", () => {
    expect(countScenes("cảnh 1\ncảnh 2\ncảnh 3")).toBe(3);
  });

  it("bo qua dong trong (kem dong chi co khoang trang)", () => {
    expect(countScenes("cảnh 1\n\ncảnh 2\n   \ncảnh 3")).toBe(3);
  });

  it("bo qua dong bat dau bang # (chu thich, khop scene_txt::parse_txt bo qua)", () => {
    expect(countScenes("# ghi chu dau file\ncảnh 1\n# một chú thích khác\ncảnh 2")).toBe(2);
  });

  it("dong # sau khi trim khoang trang dau dong cung bi bo qua", () => {
    expect(countScenes("   # thut le van la chu thich\ncảnh 1")).toBe(1);
  });

  it("hint sau dau | KHONG anh huong so dem (chi dem theo dong)", () => {
    expect(countScenes("cảnh 1 | ratio=16:9 | dur=10\ncảnh 2")).toBe(2);
  });

  it("chuoi rong -> 0 canh", () => {
    expect(countScenes("")).toBe(0);
  });

  it("chi toan dong chu thich/trong -> 0 canh", () => {
    expect(countScenes("# a\n\n   \n#b")).toBe(0);
  });
});

// Bao dam MOT nguon: hop thoai "Thêm workflow" va node Prompt phai dem GIONG HET nhau. Truoc
// 2026-10-14 moi ben mot ham, ban trong hop thoai khong bo qua dong `#` nen bao "5 cảnh" trong khi
// node chay dung 3 — nguoi dung khong co cach nao biet ben nao dung.
describe("mot nguon duy nhat", () => {
  it("dong `#` khong duoc tinh la canh — o BAT KY cho nao goi", () => {
    const txt = "# ghi chu\ncanh 1\ncanh 2\n# ghi chu nua\n";
    expect(countScenes(txt)).toBe(2);
  });
});
