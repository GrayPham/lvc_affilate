// TDD P4d — ghim (pin) cac ma loi TEMPLATE_* -> nhan tieng Viet cua `src/format/template.ts`.
// Neu backend them/bo/doi mot ma loi, danh sach ghim o day lac hau va test do — do la CHU DICH
// (giong `format_credential.test.ts`).
import { describe, expect, it } from "vitest";

import {
  TEMPLATE_ERROR_VI,
  describeTemplateError,
  templateErrorMessage,
} from "../src/format/template";

describe("TEMPLATE_ERROR_VI — pin dung 5 ma loi TEMPLATE_* + STORE (P4d contract)", () => {
  const TEMPLATE_ERROR_CODES: readonly string[] = [
    "TEMPLATE_NOT_FOUND",
    "TEMPLATE_CORRUPT",
    "TEMPLATE_READONLY",
    "GRAPH_ENCODE",
    "STORE",
  ];

  it("dung 5 ma, khong sot khong thua", () => {
    expect(TEMPLATE_ERROR_CODES).toHaveLength(5);
    expect(Object.keys(TEMPLATE_ERROR_VI).sort()).toEqual([...TEMPLATE_ERROR_CODES].sort());
  });

  it.each(TEMPLATE_ERROR_CODES)("%s co nhan tieng Viet rieng", (code) => {
    expect(TEMPLATE_ERROR_VI).toHaveProperty(code);
    expect(TEMPLATE_ERROR_VI[code]!.length).toBeGreaterThan(0);
  });
});

describe("templateErrorMessage / describeTemplateError", () => {
  it("IpcError co ma -> nhan tieng Viet theo ma", () => {
    expect(templateErrorMessage({ code: "TEMPLATE_READONLY", message: "raw", details: null })).toBe(
      "Mẫu công khai chỉ đọc — không thể lưu đè hoặc xoá.",
    );
  });

  it("describeTemplateError: IpcError (khong phai Error JS that) -> nhan tieng Viet, khong '[object Object]'", () => {
    const ipcErr = { code: "TEMPLATE_NOT_FOUND", message: "raw", details: null };
    expect(describeTemplateError(ipcErr)).not.toContain("[object Object]");
    expect(describeTemplateError(ipcErr)).toBe("Không tìm thấy mẫu.");
  });

  it("ma khong co trong bang -> roi ve message goc", () => {
    expect(describeTemplateError({ code: "WAT", message: "chi tiet", details: null })).toBe(
      "chi tiet",
    );
  });

  it("Error JS that -> .message; gia tri la -> String()", () => {
    expect(describeTemplateError(new Error("boom"))).toBe("boom");
    expect(describeTemplateError("tho")).toBe("tho");
  });
});
