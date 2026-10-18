// Logic thuan cua CheckProgressDialog (src/components/check_progress.ts).
import { describe, expect, it } from "vitest";

import { etaText, progressPercent, progressText, rowView } from "../src/components/check_progress";

describe("rowView", () => {
  it("cho / dang kiem / loi co nhan + mau rieng", () => {
    expect(rowView("waiting").label).toBe("Chờ");
    expect(rowView("checking").label).toBe("Đang kiểm tra");
    expect(rowView("checking").icon).toBe("…");
    expect(rowView("failed").label).toBe("Lỗi kiểm tra");
    expect(rowView("failed").colorClass).toContain("rose");
  });

  it("xong + OK -> Đạt, dau tick, mau ok", () => {
    const v = rowView("done", "ok");
    expect(v.label).toBe("Đạt");
    expect(v.icon).toBe("✓");
    expect(v.colorClass).toContain("ok");
  });

  it("xong nhung trang thai xau -> nhan dung trang thai do", () => {
    expect(rowView("done", "expired").label).toBe("Hết hạn");
    expect(rowView("done", "banned").label).toBe("Bị khoá");
    expect(rowView("done", "quota").label).toBe("Hết hạn mức");
    expect(rowView("done", "expired").icon).toBe("!");
  });

  it("done khong co status -> Chưa rõ, khong nem", () => {
    expect(rowView("done").label).toBe("Chưa rõ");
  });
});

describe("etaText", () => {
  it("chua xong hang nao -> null (chua doan duoc)", () => {
    expect(etaText(0, 3, 0)).toBeNull();
  });

  it("xong het -> null", () => {
    expect(etaText(3, 3, 9000)).toBeNull();
  });

  it("xong 1/3 het 10s -> con ~20 giay", () => {
    expect(etaText(1, 3, 10_000)).toBe("khoảng 20 giây nữa");
  });

  it("con lau thi doi sang phut", () => {
    // xong 1/5, moi hang 30s -> con 4*30=120s = 2 phut
    expect(etaText(1, 5, 30_000)).toBe("khoảng 2 phút nữa");
  });

  it("khong bao gio ra 0 giay", () => {
    expect(etaText(2, 3, 100)).toBe("khoảng 1 giây nữa");
  });
});

describe("progress", () => {
  it("progressText", () => {
    expect(progressText(2, 3)).toBe("2/3 xong");
  });

  it("progressPercent", () => {
    expect(progressPercent(2, 4)).toBe(50);
    expect(progressPercent(0, 0)).toBe(0);
    expect(progressPercent(3, 3)).toBe(100);
  });
});
