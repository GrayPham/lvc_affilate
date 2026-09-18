// TDD Task 14 buoc 1 — cac ham thuan cua man Tai khoan (`src/format/account.ts`). Bang doi
// chieu `validateProxy` duoc rut thang tu cac test cua `crates/lvc-core/src/proxy.rs::parse_proxy`
// (cung oracle, chi port sang TS) de dam bao khop hanh vi server that.
import { describe, expect, it } from "vitest";

import {
  ACCOUNT_ERROR_VI,
  accountStatusLabel,
  describeAccountError,
  formatRemaining,
  formatUsageCompact,
  remainingMinutes,
  validateName,
  validateProxy,
  type LoginLabel,
} from "../src/format/account";
import type { AccountStatus } from "../src/ipc/types";

function baseRow(overrides: Partial<AccountStatus> = {}): AccountStatus {
  return {
    name: "acc1",
    scheduling: true,
    note: "",
    email: "",
    createdAt: 0,
    lastUsedAt: 0,
    loginOk: true,
    loginCheckedAt: 1_700_000_000,
    cooldownUntil: 0,
    cooling: false,
    rateLimitedUntil: 0,
    rateLimited: false,
    limitReason: "",
    quotaBlockedUntil: 0,
    quotaBlocked: false,
    quotaReason: "",
    creditBalance: null,
    creditCheckedAt: 0,
    proxy: "",
    browser: "chromium",
    proxySet: false,
    loggedOutAt: 0,
    autoVerifyState: "",
    autoVerifyTries: 0,
    cookieExpiresAt: 0,
    usedToday: 0,
    limit: 10,
    remaining: 10,
    usedPro: 0,
    limitPro: 2,
    usedProMax: 0,
    limitProMax: 2,
    usedAgent: 0,
    limitAgent: 10,
    ...overrides,
  };
}

describe("accountStatusLabel", () => {
  it("loginCheckedAt <= 0 -> Chưa rõ, BAT KE loginOk/cooling/rateLimited/quotaBlocked/scheduling", () => {
    for (const loginOk of [true, false]) {
      for (const cooling of [true, false]) {
        for (const rateLimited of [true, false]) {
          for (const quotaBlocked of [true, false]) {
            for (const scheduling of [true, false]) {
              const row = baseRow({
                loginOk,
                loginCheckedAt: 0,
                cooling,
                rateLimited,
                quotaBlocked,
                scheduling,
              });
              expect(accountStatusLabel(row)).toBe("Chưa rõ");
            }
          }
        }
      }
    }
  });

  it("loginCheckedAt > 0 -> OK/Hết phiên theo loginOk, KHONG phu thuoc cooling/rateLimited/quotaBlocked/scheduling", () => {
    for (const cooling of [true, false]) {
      for (const rateLimited of [true, false]) {
        for (const quotaBlocked of [true, false]) {
          for (const scheduling of [true, false]) {
            const okRow = baseRow({
              loginOk: true,
              loginCheckedAt: 1_700_000_000,
              cooling,
              rateLimited,
              quotaBlocked,
              scheduling,
            });
            const outRow = baseRow({
              loginOk: false,
              loginCheckedAt: 1_700_000_000,
              cooling,
              rateLimited,
              quotaBlocked,
              scheduling,
            });
            expect(accountStatusLabel(okRow)).toBe("OK");
            expect(accountStatusLabel(outRow)).toBe("Hết phiên");
          }
        }
      }
    }
  });

  it("gia tri am cua loginCheckedAt cung tinh la chua kiem (phong thu)", () => {
    expect(accountStatusLabel(baseRow({ loginCheckedAt: -1 }))).toBe("Chưa rõ" satisfies LoginLabel);
  });
});

describe("validateName", () => {
  const VALID: readonly string[] = ["a", "acc1", "acc_1-2", "A".repeat(32)];
  const INVALID: readonly string[] = [
    "",
    "A".repeat(33),
    "acc 1",
    "acc/1",
    "acc.1",
    "acc@1",
    "tài khoản",
  ];

  it.each(VALID)("%s -> hop le", (name) => {
    expect(validateName(name)).toBeNull();
  });

  it.each(INVALID)("%s -> khong hop le", (name) => {
    expect(validateName(name)).not.toBeNull();
  });
});

describe("validateProxy — mirror crates/lvc-core/src/proxy.rs::parse_proxy", () => {
  const VALID: readonly string[] = [
    "",
    "   ",
    "direct",
    "DIRECT",
    "  Direct  ",
    "1.2.3.4:8080",
    "1.2.3.4:8080:user:pass",
    "socks5://u:p@host.example.com:1080",
    "http://host:1080",
    "http://[::1]:1080",
  ];

  const INVALID: readonly string[] = [
    "1.2.3.4:8080:user", // 3 manh
    "1.2.3.4:8080:user:", // pass rong
    "1.2.3.4:8080::pass", // user rong
    "1.2.3.4", // 1 manh
    "1.2.3.4:8080:8081:8082:8083", // 5 manh
    "1.2.3.4:99999", // port qua 65535 (dang plain)
    "1.2.3.4:abc", // port khong phai so
    "http://u@host:1080", // co user, thieu pass
    "http://:p@host:1080", // co pass, thieu user
    "http://host:1080/path", // co path
    "http://host:1080?x=1", // co query
    "ftp://host:1080", // scheme sai
    "abc", // rac hoan toan
    "http:// host:1080", // co khoang trang
  ];

  it.each(VALID)("%s -> hop le", (proxy) => {
    expect(validateProxy(proxy)).toBeNull();
  });

  it.each(INVALID)("%s -> khong hop le", (proxy) => {
    expect(validateProxy(proxy)).not.toBeNull();
  });

  it("thong diep loi khop nguyen van ban Rust (proxies.py)", () => {
    expect(validateProxy("abc")).toBe(
      "proxy must be ip:port[:user:pass] or scheme://user:pass@host:port " +
        "(scheme http/https/socks5), or 'direct'",
    );
  });
});

describe("remainingMinutes", () => {
  const NOW = 1_700_000_000;

  it("until <= 0 -> 0 (khong ap dung)", () => {
    expect(remainingMinutes(0, NOW)).toBe(0);
    expect(remainingMinutes(-1, NOW)).toBe(0);
  });

  it("until da qua -> 0 (khong am)", () => {
    expect(remainingMinutes(NOW - 100, NOW)).toBe(0);
  });

  it("until = now -> 0", () => {
    expect(remainingMinutes(NOW, NOW)).toBe(0);
  });

  it("lam tron LEN theo phut", () => {
    expect(remainingMinutes(NOW + 1, NOW)).toBe(1);
    expect(remainingMinutes(NOW + 59, NOW)).toBe(1);
    expect(remainingMinutes(NOW + 60, NOW)).toBe(1);
    expect(remainingMinutes(NOW + 61, NOW)).toBe(2);
    expect(remainingMinutes(NOW + 600, NOW)).toBe(10);
  });
});

describe("formatUsageCompact", () => {
  it("gop 4 mode theo thu tu fast/pro/pro_max/agent", () => {
    const row = baseRow({
      usedToday: 3,
      limit: 10,
      usedPro: 0,
      limitPro: 2,
      usedProMax: 1,
      limitProMax: 2,
      usedAgent: 5,
      limitAgent: 30,
    });
    expect(formatUsageCompact(row)).toBe("3/10 · pro 0/2 · pro_max 1/2 · agent 5/30");
  });
});

describe("formatRemaining", () => {
  const NOW = 1_700_000_000;

  it("khong co han nao dang ap dung -> gach ngang", () => {
    expect(formatRemaining(baseRow(), NOW)).toBe("—");
  });

  it("chi hien cac co dang BAT (cooling/rateLimited/quotaBlocked doc lap)", () => {
    const row = baseRow({
      cooling: true,
      cooldownUntil: NOW + 300,
      rateLimited: false,
      rateLimitedUntil: NOW + 999, // co gia tri nhung co rateLimited=false -> KHONG hien
      quotaBlocked: true,
      quotaBlockedUntil: NOW + 60,
    });
    expect(formatRemaining(row, NOW)).toBe("Nghỉ 5p · Quota 1p");
  });
});

describe("describeAccountError — khong bao gio ra '[object Object]' cho loi IPC", () => {
  it("IpcError (khong phai Error JS that) -> thong diep tieng Viet theo ma", () => {
    const ipcErr = { code: "COOKIE_NOT_LOGGED_IN", message: "raw", details: null };
    expect(describeAccountError(ipcErr)).not.toContain("[object Object]");
    expect(describeAccountError(ipcErr)).toBe(
      "Cookie không đăng nhập được (phiên đã hết hạn hoặc sai).",
    );
  });

  it("ma khong nam trong bang tra cuu -> roi ve message goc", () => {
    const ipcErr = { code: "SOME_UNKNOWN_CODE", message: "chi tiet la", details: null };
    expect(describeAccountError(ipcErr)).toBe("chi tiet la");
  });

  it("Error JS that -> dung .message", () => {
    expect(describeAccountError(new Error("boom"))).toBe("boom");
  });

  it("gia tri la khac -> String()", () => {
    expect(describeAccountError("chuoi loi tho")).toBe("chuoi loi tho");
  });
});

describe("ACCOUNT_ERROR_VI — pin dung 13 ma cua AccountError::code() (review, Minor #1)", () => {
  // Copy TAY tu `crates/lvc-engine/src/accounts.rs` — ham `AccountError::code()`, nhanh `match`
  // liet ke moi bien the (~dong 140-155 tai thoi diem viet test nay, sau khi them NOT_A_PROFILE o
  // fix-round 1 final-review I5). Neu Rust them/bo/doi ten mot bien the, danh sach nay lac hau va
  // test duoi day se do — do la CHU DICH (khong phai fixture sinh tu Rust nhu
  // `ipc_fixtures.test.ts`, vi `AccountError` khong di qua IPC nhu mot DTO co `Serialize` — chi ma
  // chuoi `code()` cua no moi sang UI).
  const RUST_ACCOUNT_ERROR_CODES: readonly string[] = [
    "ACCOUNT_EXISTS",
    "ACCOUNT_NOT_FOUND",
    "INVALID_NAME",
    "COOKIE_INVALID",
    "COOKIE_NOT_LOGGED_IN",
    "INVALID_PROXY",
    "PROFILE_BUSY",
    "ACCOUNT_BUSY",
    "NOT_A_PROFILE",
    "BROWSER",
    "STORE",
    "IO",
    "ENGINE",
  ];

  it("dung 13 ma, khong sot khong thua", () => {
    expect(RUST_ACCOUNT_ERROR_CODES).toHaveLength(13);
  });

  it.each(RUST_ACCOUNT_ERROR_CODES)("%s co nhan tieng Viet rieng trong ACCOUNT_ERROR_VI", (code) => {
    expect(ACCOUNT_ERROR_VI).toHaveProperty(code);
    expect(typeof ACCOUNT_ERROR_VI[code]).toBe("string");
    expect(ACCOUNT_ERROR_VI[code]!.length).toBeGreaterThan(0);
  });

  it("ACCOUNT_ERROR_VI khong co key thua ngoai danh sach 13 ma tren", () => {
    expect(Object.keys(ACCOUNT_ERROR_VI).sort()).toEqual([...RUST_ACCOUNT_ERROR_CODES].sort());
  });
});
