// Review fix-round 1 (I1): Tauri `invoke()` reject bang gia tri Rust `Err` da giai ma THANG
// (`IpcError` — mot object `{code,message,details}`), KHONG phai instance `Error`. Truoc fix nay
// moi noi dung `e instanceof Error ? e.message : String(e)` deu roi ve "[object Object]" cho
// dung truong hop hay gap nhat. `errorText` la diem DUY NHAT xu ly ca ba dang: `IpcError`, `Error`
// JS thuan, va bat ky gia tri nao khac.
import { describe, expect, it } from "vitest";

import { errorText } from "../src/ipc/error_text";
import type { IpcError } from "../src/ipc/types";

function ipcError(overrides: Partial<IpcError> = {}): IpcError {
  return { code: "STORE", message: "khong ghi duoc CSDL", details: null, ...overrides };
}

describe("errorText — chuan hoa loi IPC/Error/khac ve mot dong (review fix-round 1 I1)", () => {
  it("IpcError thuong -> \"<code>: <message>\"", () => {
    expect(errorText(ipcError())).toBe("STORE: khong ghi duoc CSDL");
  });

  it("IpcError GRAPH_INVALID voi details la mang -> them so luong loi", () => {
    const e = ipcError({
      code: "GRAPH_INVALID",
      message: "do thi khong hop le",
      details: [{ type: "empty" }, { type: "cycle", nodes: ["n1"] }],
    });
    expect(errorText(e)).toBe("GRAPH_INVALID: do thi khong hop le (2 lỗi)");
  });

  it("IpcError GRAPH_INVALID nhung details khong phai mang -> khong them so luong (van co code+message)", () => {
    const e = ipcError({ code: "GRAPH_INVALID", message: "do thi khong hop le", details: null });
    expect(errorText(e)).toBe("GRAPH_INVALID: do thi khong hop le");
  });

  it("Error JS thuan -> .message", () => {
    expect(errorText(new Error("mang loi"))).toBe("mang loi");
  });

  it("chuoi/gia tri khac -> String(e)", () => {
    expect(errorText("chi la mot chuoi")).toBe("chi la mot chuoi");
    expect(errorText(42)).toBe("42");
    expect(errorText(undefined)).toBe("undefined");
  });

});
