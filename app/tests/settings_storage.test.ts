// `slug`/`renderFilename` phia client (components/settings/StorageSettings.tsx) la BAN SAO cua
// `lvc_core::settings::storage::{slug, render_filename}` — chi dung de XEM TRUOC tren form; gia tri
// that khi chay do Rust tinh.
//
// Cac test duoi day co y dung CHINH cac vi du da ghim trong test Rust
// (`crates/lvc-core/src/settings/storage.rs`): mot ben doi ma ben kia khong doi se do ngay o day,
// de nguoi dung khong thay xem truoc khac voi ten file thuc te.
import { describe, expect, it } from "vitest";

import { formatBytes, renderFilename, slug } from "../src/components/settings/StorageSettings";

const VARS = {
  workflow: "Storyboard ảnh từng cảnh",
  date: "2026-10-14",
  time: "1432",
  index: 3,
  account: "Nguyễn Văn A",
};

describe("slug — khop ban Rust", () => {
  it("bo dau tieng Viet", () => {
    expect(slug("Storyboard ảnh từng cảnh")).toBe("storyboard-anh-tung-canh");
    expect(slug("Đường Hồ Chí Minh")).toBe("duong-ho-chi-minh");
    expect(slug("Nguyễn Văn A")).toBe("nguyen-van-a");
  });

  it("gop dau gach va bo o hai dau", () => {
    expect(slug("  a   b  ")).toBe("a-b");
    expect(slug("--a--b--")).toBe("a-b");
    expect(slug("a/b:c")).toBe("a-b-c");
  });

  it("rong ra khong-ten", () => {
    expect(slug("")).toBe("khong-ten");
    expect(slug("///")).toBe("khong-ten");
  });
});

describe("renderFilename — khop ban Rust", () => {
  it("mau mac dinh ra dung vi du cua ban thiet ke", () => {
    expect(renderFilename("{workflow}_{gio}_{stt}", VARS)).toBe(
      "storyboard-anh-tung-canh_1432_003",
    );
  });

  it("stt dem 0 thanh 3 chu so, so lon hon khong bi cat", () => {
    expect(renderFilename("{stt}", { ...VARS, index: 7 })).toBe("007");
    expect(renderFilename("{stt}", { ...VARS, index: 1234 })).toBe("1234");
  });

  it("du 5 bien", () => {
    expect(renderFilename("{workflow}-{ngay}-{gio}-{stt}-{tai_khoan}", VARS)).toBe(
      "storyboard-anh-tung-canh-2026-10-14-1432-003-nguyen-van-a",
    );
  });

  it("bien la giu nguyen de nguoi dung thay minh go sai", () => {
    expect(renderFilename("a_{khong_co}_b", VARS)).toBe("a_{khong_co}_b");
  });

  it("khong tao duoc thu muc con", () => {
    const out = renderFilename("..\\..\\{gio}", VARS);
    expect(out).not.toContain("\\");
    expect(out).not.toContain("/");
  });

  it("loai ky tu Windows cam", () => {
    const out = renderFilename('a:b*c?d"e<f>g|h', VARS);
    for (const bad of [":", "*", "?", '"', "<", ">", "|"]) {
      expect(out).not.toContain(bad);
    }
  });

  it("mau rong ra video", () => {
    expect(renderFilename("", VARS)).toBe("video");
    expect(renderFilename("...", VARS)).toBe("video");
  });
});

describe("formatBytes", () => {
  it("duoi 1 KB thi ghi byte", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
  });

  it("dung dau phay thap phan kieu Viet", () => {
    expect(formatBytes(1536)).toBe("1,5 KB");
    expect(formatBytes(3 * 1024 ** 3)).toBe("3,0 GB");
  });

  it("tu 100 tro len thi lam tron, khong con phan thap phan", () => {
    expect(formatBytes(840 * 1024)).toBe("840 KB");
  });
});
