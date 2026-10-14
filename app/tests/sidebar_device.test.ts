// `maskDeviceId` (src/format/device.ts) — rut gon ma thiet bi cho vua thanh ben
// nhung van du de doi chieu khi bao loi. Ma KHONG phai bi mat; rut gon chi de gon mat.
import { describe, expect, it } from "vitest";

import { maskDeviceId } from "../src/format/device";

describe("maskDeviceId", () => {
  it("ma dai (uuid 32 ky tu) -> 4 dau + 6 cuoi", () => {
    expect(maskDeviceId("7B2F1A2B3C4D5E6F7A8B9C0D9E422A12")).toBe("7B2F···422A12");
  });

  it("giu dung 4 ky tu dau va 6 ky tu cuoi", () => {
    const out = maskDeviceId("ABCDEFGHIJKLMNOP");
    expect(out.startsWith("ABCD")).toBe(true);
    expect(out.endsWith("KLMNOP")).toBe(true);
    expect(out).toContain("···");
  });

  it("ma NGAN (<=10) giu nguyen — rut gon chi lam mat thong tin", () => {
    expect(maskDeviceId("ABC123")).toBe("ABC123");
    expect(maskDeviceId("ABCDEFGHIJ")).toBe("ABCDEFGHIJ");
  });

  it("chuoi rong khong lam vo gi", () => {
    expect(maskDeviceId("")).toBe("");
  });
});
