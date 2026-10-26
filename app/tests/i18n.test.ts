// i18n: ban dich phai phu KIN tap khoa goc (tieng Viet la nguon chan ly), khong duoc de chuoi rong,
// va cho giu `{ten}` phai duoc thay. Neu khong co test nay thi mot khoa thieu chi lo ra luc nguoi
// dung doi ngon ngu va thay o trong.
import { describe, expect, it } from "vitest";

import { en } from "../src/i18n/en";
import { translate } from "../src/i18n/translate";
import { LANGS } from "../src/i18n/types";
import { vi } from "../src/i18n/vi";
import { licenseErrorKey } from "../src/components/licenseGateError";
import type { IpcError } from "../src/ipc/types";

function ipcError(code: string): IpcError {
  return { code, message: "chi tiet ky thuat", details: null };
}

describe("tu dien i18n", () => {
  it("English phu kin moi khoa cua tieng Viet", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(vi).sort());
  });

  it("khong ban dich nao de chuoi rong", () => {
    for (const [key, value] of Object.entries(en)) {
      expect(value.trim(), `khoa ${key} rong ben English`).not.toBe("");
    }
    for (const [key, value] of Object.entries(vi)) {
      expect(value.trim(), `khoa ${key} rong ben tieng Viet`).not.toBe("");
    }
  });

  it("moi ngon ngu khai bao deu tra ve chuoi that", () => {
    for (const lang of LANGS) {
      expect(translate(lang, "gate.exit")).not.toBe("");
    }
  });
});

describe("translate — cho giu {ten}", () => {
  it("thay cho giu bang gia tri truyen vao", () => {
    expect(translate("vi", "gate.activate.hello", { ten: "alice" })).toBe("Xin chào alice");
    expect(translate("en", "gate.activate.hello", { ten: "alice" })).toBe("Hello alice");
  });

  it("thieu bien thi giu nguyen cho giu, khong in 'undefined'", () => {
    expect(translate("vi", "gate.activate.hello", {})).toContain("{ten}");
  });

  it("khong truyen vars thi tra nguyen van", () => {
    expect(translate("en", "gate.login.submit")).toBe("Sign in");
  });
});

describe("licenseErrorKey — ma loi IPC -> khoa i18n", () => {
  it("ma loi biet truoc -> khoa rieng, dich duoc", () => {
    for (const code of ["NETWORK", "BAD_CREDENTIALS", "VERIFY_FAILED", "NOT_LOGGED_IN", "SESSION_SAVE"]) {
      const key = licenseErrorKey(ipcError(code));
      expect(key).toBe(`gate.err.${code}`);
      expect(translate("vi", key)).not.toBe("");
      expect(translate("en", key)).not.toBe("");
    }
  });

  it("ma loi la -> thong bao chung, KHONG lo message ky thuat", () => {
    expect(licenseErrorKey(ipcError("MOT_MA_LA"))).toBe("gate.err.unknown");
    expect(licenseErrorKey(new Error("vo tinh"))).toBe("gate.err.unknown");
    expect(licenseErrorKey("chuoi tho")).toBe("gate.err.unknown");
  });
});
