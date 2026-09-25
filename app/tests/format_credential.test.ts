// TDD P4c — ghim (pin) cac record nhan tieng Viet cua `src/format/credential.ts`:
// provider/kind/status + ma loi CRED_*. Neu backend them/bo/doi mot gia tri enum hoac ma loi, danh
// sach ghim o day lac hau va test do — do la CHU DICH (giong `format_account.test.ts`).
import { describe, expect, it } from "vitest";

import {
  CREDENTIAL_ERROR_VI,
  CREDENTIAL_KIND_VI,
  CREDENTIAL_PROVIDER_VI,
  CREDENTIAL_STATUS_BADGE_CLASS,
  CREDENTIAL_STATUS_VI,
  credentialProviderLabel,
  credentialStatusLabel,
  describeCredentialError,
} from "../src/format/credential";
import {
  CredentialKindSchema,
  CredentialProviderSchema,
  CredentialStatusSchema,
} from "../src/ipc/types";

describe("CREDENTIAL_PROVIDER_VI — pin dung 5 provider cua CredentialProviderSchema", () => {
  it("khop chinh xac cac gia tri enum, khong sot khong thua", () => {
    expect(Object.keys(CREDENTIAL_PROVIDER_VI).sort()).toEqual(
      [...CredentialProviderSchema.options].sort(),
    );
  });

  it.each(CredentialProviderSchema.options)("%s co nhan khong rong", (p) => {
    expect(credentialProviderLabel(p).length).toBeGreaterThan(0);
  });

  it("nhan cu the", () => {
    expect(CREDENTIAL_PROVIDER_VI.dola).toBe("Dola");
    expect(CREDENTIAL_PROVIDER_VI.chatgpt_web).toBe("ChatGPT (web)");
  });
});

describe("CREDENTIAL_KIND_VI — pin dung 3 kind cua CredentialKindSchema", () => {
  it("khop chinh xac cac gia tri enum, khong sot khong thua", () => {
    expect(Object.keys(CREDENTIAL_KIND_VI).sort()).toEqual([...CredentialKindSchema.options].sort());
  });
});

describe("CREDENTIAL_STATUS_VI + badge — pin dung 5 status cua CredentialStatusSchema", () => {
  it("nhan khop chinh xac cac gia tri enum, khong sot khong thua", () => {
    expect(Object.keys(CREDENTIAL_STATUS_VI).sort()).toEqual(
      [...CredentialStatusSchema.options].sort(),
    );
  });

  it("badge co lop mau rieng cho tung status, khong sot khong thua", () => {
    expect(Object.keys(CREDENTIAL_STATUS_BADGE_CLASS).sort()).toEqual(
      [...CredentialStatusSchema.options].sort(),
    );
    // Cac lop mau phai phan biet (khong hai status trung mot lop).
    const classes = Object.values(CREDENTIAL_STATUS_BADGE_CLASS);
    expect(new Set(classes).size).toBe(classes.length);
  });

  it("nhan cu the", () => {
    expect(credentialStatusLabel("ok")).toBe("OK");
    expect(credentialStatusLabel("banned")).toBe("Bị khoá");
  });
});

describe("CREDENTIAL_ERROR_VI — pin dung 7 ma loi CRED_* + STORE (P4c contract)", () => {
  const CRED_ERROR_CODES: readonly string[] = [
    "CRED_BAD_PROVIDER",
    "CRED_BAD_KIND",
    "CRED_BAD_LABEL",
    "CRED_EMPTY_SECRET",
    "CRED_NOT_FOUND",
    "CRED_CRYPTO",
    "STORE",
  ];

  it("dung 7 ma, khong sot khong thua", () => {
    expect(CRED_ERROR_CODES).toHaveLength(7);
    expect(Object.keys(CREDENTIAL_ERROR_VI).sort()).toEqual([...CRED_ERROR_CODES].sort());
  });

  it.each(CRED_ERROR_CODES)("%s co nhan tieng Viet rieng", (code) => {
    expect(CREDENTIAL_ERROR_VI).toHaveProperty(code);
    expect(CREDENTIAL_ERROR_VI[code]!.length).toBeGreaterThan(0);
  });
});

describe("describeCredentialError — khong bao gio ra '[object Object]' cho loi IPC", () => {
  it("IpcError (khong phai Error JS that) -> thong diep tieng Viet theo ma", () => {
    const ipcErr = { code: "CRED_EMPTY_SECRET", message: "raw", details: null };
    expect(describeCredentialError(ipcErr)).not.toContain("[object Object]");
    expect(describeCredentialError(ipcErr)).toBe("Chưa nhập bí mật (API key hoặc cookie/đăng nhập).");
  });

  it("ma khong co trong bang -> roi ve message goc", () => {
    expect(describeCredentialError({ code: "WAT", message: "chi tiet", details: null })).toBe(
      "chi tiet",
    );
  });

  it("Error JS that -> .message; gia tri la -> String()", () => {
    expect(describeCredentialError(new Error("boom"))).toBe("boom");
    expect(describeCredentialError("tho")).toBe("tho");
  });
});
