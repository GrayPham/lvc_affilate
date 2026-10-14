// `timeAgo` (src/components/UpdateWorkflowDialog.tsx) — doi moc giay Unix cua mot muc kho cong
// dong thanh chuoi "5 ngay truoc" nhu ban ban giao. THUAN: `nowMs` truyen vao nen test khong phu
// thuoc dong ho that.
import { describe, expect, it } from "vitest";

import { timeAgo } from "../src/components/UpdateWorkflowDialog";

const NOW_MS = 1_760_000_000_000; // moc gia dinh
const NOW_S = NOW_MS / 1000;

describe("timeAgo", () => {
  it("moc 0 (khong biet) -> chuoi rong, KHONG phai '56 nam truoc'", () => {
    expect(timeAgo(0, NOW_MS)).toBe("");
    expect(timeAgo(-5, NOW_MS)).toBe("");
  });

  it("duoi mot phut -> vua xong", () => {
    expect(timeAgo(NOW_S, NOW_MS)).toBe("vừa xong");
    expect(timeAgo(NOW_S - 59, NOW_MS)).toBe("vừa xong");
  });

  it("phut", () => {
    expect(timeAgo(NOW_S - 60, NOW_MS)).toBe("1 phút trước");
    expect(timeAgo(NOW_S - 59 * 60, NOW_MS)).toBe("59 phút trước");
  });

  it("gio", () => {
    expect(timeAgo(NOW_S - 3600, NOW_MS)).toBe("1 giờ trước");
    expect(timeAgo(NOW_S - 5 * 3600, NOW_MS)).toBe("5 giờ trước");
  });

  it("ngay — dung voi vi du cua ban thiet ke", () => {
    expect(timeAgo(NOW_S - 5 * 86400, NOW_MS)).toBe("5 ngày trước");
    expect(timeAgo(NOW_S - 29 * 86400, NOW_MS)).toBe("29 ngày trước");
  });

  it("thang", () => {
    expect(timeAgo(NOW_S - 30 * 86400, NOW_MS)).toBe("1 tháng trước");
  });

  it("nam", () => {
    expect(timeAgo(NOW_S - 400 * 86400, NOW_MS)).toBe("1 năm trước");
  });

  it("moc o TUONG LAI (lech dong ho) -> vua xong, khong ra so am", () => {
    const out = timeAgo(NOW_S + 10_000, NOW_MS);
    expect(out).toBe("vừa xong");
    expect(out).not.toContain("-");
  });
});
