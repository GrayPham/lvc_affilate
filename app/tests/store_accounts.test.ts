// TDD Task 14 fix-round 1 (review task-14-review.md, Important #1 + #2) — invariants cua
// `store/accounts.ts` khi HAI hanh dong chay chong lan:
// 1. Loi cua `setScheduling` chi duoc hoan tac (revert) DONG bi anh huong, KHONG duoc ghi de toan
//    bo `rows` bang mot snapshot cu — snapshot do co the da lac hau neu MOT dong KHAC vua duoc
//    `load()` cap nhat thanh cong trong luc cho phan hoi cua dong nay.
// 2. `busy` la dem tham chieu (ref-count) theo ten — hai hanh dong chong lan tren CUNG mot tai
//    khoan phai giu `busy[name] > 0` cho toi khi CA HAI ket thuc, khong phai hanh dong xong TRUOC
//    xoa busy mat cho hanh dong con lai dang chay.
//
// Mock DUY NHAT lop `src/ipc/tauri.ts` (seam da tai lieu hoa ngay trong file do) — `commands.ts`
// (zod parse that) va `format/account.ts` (`describeAccountError`/`isIpcError` that) khong bi mock,
// nen test nay di qua toan bo duong That tru phan `invoke()` cuoi cung.
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AccountStatus } from "../src/ipc/types";

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (v: T) => void;
  reject: (e: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const mockInvoke = vi.fn(
  (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> =>
    Promise.reject(new Error("mockInvoke: chua cau hinh cho lenh nay")),
);

vi.mock("../src/ipc/tauri", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args),
  listen: vi.fn(),
}));

import { useAccountsStore } from "../src/store/accounts";

function makeRow(overrides: Partial<AccountStatus> = {}): AccountStatus {
  return {
    name: "acc1",
    scheduling: false,
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

beforeEach(() => {
  mockInvoke.mockReset();
  useAccountsStore.setState({ rows: [], loading: false, error: null, busy: {} });
});

describe("setScheduling — revert CHI dong bi loi (Important #1)", () => {
  it("mot dong loi KHONG duoc xoa mat cap nhat da thanh cong cua dong khac trong luc cho", async () => {
    const acc1Initial = makeRow({ name: "acc1", scheduling: false });
    const acc2Initial = makeRow({ name: "acc2", scheduling: false });
    useAccountsStore.setState({ rows: [acc1Initial, acc2Initial] });

    const setSchedAcc1 = deferred<unknown>();
    const setSchedAcc2 = deferred<unknown>();

    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === "account_set_scheduling") {
        const name = (args as { name: string }).name;
        if (name === "acc1") {
          return setSchedAcc1.promise;
        }
        if (name === "acc2") {
          return setSchedAcc2.promise;
        }
      }
      if (cmd === "account_list") {
        // "Server" sau khi acc2 da ghi thanh cong (con acc1 CHUA — dang cho phan hoi):
        // acc1 giu nguyen gia tri CU (scheduling:false), acc2 da doi.
        return Promise.resolve([
          makeRow({ name: "acc1", scheduling: false }),
          makeRow({ name: "acc2", scheduling: true }),
        ]);
      }
      return Promise.reject(new Error(`lenh khong mong doi trong test nay: ${cmd}`));
    });

    // Bam bat lich ca hai gan nhu cung luc — ca hai optimistic-update ngay lap tuc.
    const p1 = useAccountsStore.getState().setScheduling("acc1", true).catch(() => undefined);
    const p2 = useAccountsStore.getState().setScheduling("acc2", true).catch(() => undefined);

    // acc2 thanh cong TRUOC -> load() chay -> rows duoc thay bang snapshot server o tren.
    setSchedAcc2.resolve(null);
    await p2;
    expect(useAccountsStore.getState().rows.find((r) => r.name === "acc2")?.scheduling).toBe(true);

    // acc1 THAT BAI SAU KHI acc2 da cap nhat xong — chi dong acc1 duoc hoan tac, KHONG duoc ghi
    // de ca mang bang snapshot truoc do (se xoa mat acc2.scheduling=true vua load).
    setSchedAcc1.reject({ code: "STORE", message: "loi gia lap", details: null });
    await p1;

    const rows = useAccountsStore.getState().rows;
    expect(rows.find((r) => r.name === "acc1")?.scheduling).toBe(false); // hoan tac dung dong minh
    expect(rows.find((r) => r.name === "acc2")?.scheduling).toBe(true); // KHONG bi clobber
  });
});

describe("busy — dem tham chieu theo ten (Important #2)", () => {
  it("hai hanh dong chong lan tren CUNG mot ten giu busy>0 cho toi khi CA HAI xong", async () => {
    useAccountsStore.setState({ rows: [makeRow({ name: "acc1" })] });

    const verifyDeferred = deferred<unknown>();
    const setProxyDeferred = deferred<unknown>();

    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "account_verify") {
        return verifyDeferred.promise;
      }
      if (cmd === "account_set_proxy") {
        return setProxyDeferred.promise;
      }
      if (cmd === "account_list") {
        return Promise.resolve([makeRow({ name: "acc1" })]);
      }
      return Promise.reject(new Error(`lenh khong mong doi trong test nay: ${cmd}`));
    });

    expect(useAccountsStore.getState().busy["acc1"] ?? 0).toBe(0);

    const pVerify = useAccountsStore.getState().verify("acc1");
    const pProxy = useAccountsStore.getState().setProxy("acc1", "1.2.3.4:8080");

    // Ca hai dang chay -> busy phai duong.
    expect(useAccountsStore.getState().busy["acc1"] ?? 0).toBeGreaterThan(0);

    verifyDeferred.resolve({ state: "logged_in", loginOk: true, checkedAt: 1_700_000_500 });
    await pVerify;

    // MOT trong hai xong — hanh dong con lai (setProxy) VAN dang chay -> busy KHONG duoc ve 0.
    expect(useAccountsStore.getState().busy["acc1"] ?? 0).toBeGreaterThan(0);

    setProxyDeferred.resolve(null);
    await pProxy;

    // Ca hai da xong -> busy phai ve 0/khong con key.
    expect(useAccountsStore.getState().busy["acc1"] ?? 0).toBe(0);
  });
});
