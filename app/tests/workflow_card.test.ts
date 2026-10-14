// `coverSrc`/`coverFor` (src/components/WorkflowCard.tsx) — ba nguon khai anh bia theo ba dang
// khac nhau; noi nham dang nao cung lam the thanh mot O DEN (loi that, 2026-10-14: muc kho cong
// dong khai `https://...` bi noi thanh `/covers/https://....svg`).
import { describe, expect, it } from "vitest";

import { coverFor, coverSrc } from "../src/components/WorkflowCard";

describe("coverSrc", () => {
  it("ten bia dung san -> file trong /covers", () => {
    expect(coverSrc("g_food")).toBe("/covers/g_food.svg");
  });

  it("URL https cua kho cong dong -> GIU NGUYEN", () => {
    const url = "https://raw.githubusercontent.com/o/r/master/thumbs/ugc.png";
    expect(coverSrc(url)).toBe(url);
  });

  it("URL http -> giu nguyen", () => {
    expect(coverSrc("http://a.b/c.png")).toBe("http://a.b/c.png");
  });

  it("data URL (mau rieng tu chup canvas) -> giu nguyen", () => {
    expect(coverSrc("data:image/png;base64,AAA")).toBe("data:image/png;base64,AAA");
  });

  it("duong dan tuyet doi -> giu nguyen", () => {
    expect(coverSrc("/covers/basic.svg")).toBe("/covers/basic.svg");
  });
});

describe("coverFor", () => {
  it("co bia khai san -> dung dung cai do", () => {
    expect(coverFor("bat-ky", "https://a.b/c.png")).toBe("https://a.b/c.png");
    expect(coverFor("bat-ky", "g_tech")).toBe("g_tech");
  });

  it("khong khai -> chon theo khoa, va ON DINH qua moi lan goi", () => {
    const a = coverFor("wf-123");
    const b = coverFor("wf-123");
    expect(a).toBe(b);
    expect(a).not.toBe("");
  });

  it("hai khoa khac nhau thuong ra bia khac nhau", () => {
    const ids = ["a", "b", "c", "d", "e", "f", "g", "h"].map((k) => coverFor(k));
    expect(new Set(ids).size).toBeGreaterThan(1);
  });

  it("chuoi rong van ra mot bia hop le, khong ra rong", () => {
    expect(coverSrc(coverFor(""))).toMatch(/^\/covers\/.+\.svg$/);
  });
});
