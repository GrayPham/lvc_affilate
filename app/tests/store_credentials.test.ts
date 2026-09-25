// TDD P4c — store `store/credentials.ts` (kho tai khoan AI). Mock DUY NHAT lop `src/ipc/tauri.ts`
// (seam da tai lieu hoa) — `commands.ts` (zod parse that) va `format/credential.ts`
// (`describeCredentialError`/`isIpcError` that) khong bi mock, nen test di qua toan bo duong That
// tru `invoke()` cuoi cung.
//
// AN TOAN BI MAT: kiem tra `add`/`update` gui `secret` GHI-MOT-CHIEU xuong IPC nhung KHONG luu vao
// state (`rows` chi co `secretHint` da che) va moi hanh dong GHI goi `credential_list` lai sau do.
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CredentialView } from "../src/ipc/types";

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

import { useCredentialsStore } from "../src/store/credentials";

function makeRow(overrides: Partial<CredentialView> = {}): CredentialView {
  return {
    id: "cred_1",
    provider: "openai",
    kind: "api_key",
    label: "OpenAI chinh",
    secretHint: "••••1234",
    meta: { base_url: "" },
    proxyMasked: "",
    status: "ok",
    enabled: true,
    lastUsedAt: 0,
    lastVerifiedAt: 0,
    ...overrides,
  };
}

beforeEach(() => {
  mockInvoke.mockReset();
  useCredentialsStore.setState({ rows: [], loading: false, error: null, busy: {} });
});

describe("load", () => {
  it("nap danh sach va parse qua zod that", async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "credential_list") {
        return Promise.resolve([makeRow(), makeRow({ id: "cred_2", provider: "dola", kind: "session" })]);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });

    await useCredentialsStore.getState().load();
    const rows = useCredentialsStore.getState().rows;
    expect(rows).toHaveLength(2);
    expect(rows[1]?.provider).toBe("dola");
    expect(useCredentialsStore.getState().error).toBeNull();
  });

  it("loi IPC -> `error` la thong diep tieng Viet (khong nem)", async () => {
    mockInvoke.mockImplementation(() =>
      Promise.reject({ code: "STORE", message: "raw", details: null }),
    );
    await useCredentialsStore.getState().load();
    expect(useCredentialsStore.getState().error).toBe("Lỗi lưu dữ liệu.");
  });
});

describe("add", () => {
  it("gui secret ghi-mot-chieu, KHONG luu vao state, va load() lai sau do", async () => {
    let addArgs: Record<string, unknown> | undefined;
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === "credential_add") {
        addArgs = args;
        return Promise.resolve(makeRow({ id: "cred_new", label: "moi" }));
      }
      if (cmd === "credential_list") {
        return Promise.resolve([makeRow({ id: "cred_new", label: "moi" })]);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });

    const view = await useCredentialsStore.getState().add({
      provider: "openai",
      kind: "api_key",
      label: "moi",
      secret: "sk-super-secret",
      meta: { base_url: "https://x" },
      proxy: "1.2.3.4:8080",
    });

    // secret duoc gui xuong IPC ...
    expect(addArgs?.secret).toBe("sk-super-secret");
    expect(addArgs?.provider).toBe("openai");
    expect(addArgs?.meta).toEqual({ base_url: "https://x" });
    // ... nhung KHONG lo o ket qua tra ve (chi secretHint) va KHONG nam trong state.
    expect(view).not.toHaveProperty("secret");
    const serialized = JSON.stringify(useCredentialsStore.getState().rows);
    expect(serialized).not.toContain("sk-super-secret");
    // load() da chay lai.
    expect(useCredentialsStore.getState().rows).toHaveLength(1);
  });
});

describe("verify", () => {
  it("tra ve VerifyOutcome, busy>0 trong luc chay roi ve 0, load() lai sau do", async () => {
    useCredentialsStore.setState({ rows: [makeRow()] });
    const verifyD = deferred<unknown>();
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "credential_verify") {
        return verifyD.promise;
      }
      if (cmd === "credential_list") {
        return Promise.resolve([makeRow({ status: "ok" })]);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });

    const p = useCredentialsStore.getState().verify("cred_1");
    expect((useCredentialsStore.getState().busy["cred_1"] ?? 0)).toBeGreaterThan(0);

    verifyD.resolve({ status: "ok", detail: "key hop le" });
    const outcome = await p;
    expect(outcome).toEqual({ status: "ok", detail: "key hop le" });
    expect(useCredentialsStore.getState().busy["cred_1"] ?? 0).toBe(0);
  });
});

describe("remove", () => {
  it("goi credential_delete + load() lai, tra ve boolean", async () => {
    useCredentialsStore.setState({ rows: [makeRow()] });
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "credential_delete") {
        return Promise.resolve(true);
      }
      if (cmd === "credential_list") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });

    const deleted = await useCredentialsStore.getState().remove("cred_1");
    expect(deleted).toBe(true);
    expect(useCredentialsStore.getState().rows).toHaveLength(0);
    expect(useCredentialsStore.getState().busy["cred_1"] ?? 0).toBe(0);
  });
});
