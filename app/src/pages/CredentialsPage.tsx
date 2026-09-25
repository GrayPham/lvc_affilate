// Man "Tài khoản AI" (P4c) — quan ly kho tai khoan AI (credentials) qua 5 lenh `credential_*`.
// Nhom theo provider: Dola / Google / ChatGPT / API key (OpenAI/Gemini). Moi dong hien nhan, loai,
// `secretHint` (BIEU DIEN DUY NHAT cua bi mat — da che), badge trang thai, toggle bat/tat, proxy da
// che, nut "Kiểm tra" va xoa 2 buoc. Form "Thêm" o cuoi.
//
// AN TOAN BI MAT: o nhap bi mat la GHI-MOT-CHIEU — khong bao gio dien san / hien lai gia tri da
// nhap; xoa NGAY khi bam "Thêm" (truoc ca khi cho IPC). Khong `console.*` nao cham vao bi mat. UI
// chi bao gio nhan `secretHint` da che, khong bao gio nhan bi mat nguyen van tu backend.
import { useEffect, useMemo, useState, type FormEvent } from "react";

import { validateProxy } from "../format/account";
import {
  CREDENTIAL_STATUS_BADGE_CLASS,
  credentialKindLabel,
  credentialProviderLabel,
  credentialStatusLabel,
  describeCredentialError,
} from "../format/credential";
import { useCredentialsStore } from "../store/credentials";
import type { CredentialKind, CredentialProvider, CredentialView } from "../ipc/types";

// Loai bi mat cho phep theo provider (P4c contract). Thu tu = thu tu hien tren dropdown.
const KINDS_BY_PROVIDER: Record<CredentialProvider, readonly CredentialKind[]> = {
  dola: ["session"],
  openai: ["api_key"],
  gemini: ["api_key"],
  google: ["login", "session"],
  chatgpt_web: ["login", "session"],
};

// Nhom hien thi — moi nhom gom mot hoac nhieu provider.
const GROUPS: ReadonlyArray<{ title: string; providers: readonly CredentialProvider[] }> = [
  { title: "Dola", providers: ["dola"] },
  { title: "Google", providers: ["google"] },
  { title: "ChatGPT", providers: ["chatgpt_web"] },
  { title: "API key (OpenAI/Gemini)", providers: ["openai", "gemini"] },
];

const PROVIDER_ORDER: readonly CredentialProvider[] = [
  "dola",
  "google",
  "chatgpt_web",
  "openai",
  "gemini",
];

/** `base_url` chi co y nghia voi provider goi API (openai/gemini). */
function providerUsesBaseUrl(provider: CredentialProvider): boolean {
  return provider === "openai" || provider === "gemini";
}

export default function CredentialsPage() {
  const rows = useCredentialsStore((s) => s.rows);
  const loading = useCredentialsStore((s) => s.loading);
  const error = useCredentialsStore((s) => s.error);
  const busy = useCredentialsStore((s) => s.busy);
  const load = useCredentialsStore((s) => s.load);
  const verify = useCredentialsStore((s) => s.verify);
  const update = useCredentialsStore((s) => s.update);
  const remove = useCredentialsStore((s) => s.remove);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-3">
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-semibold text-slate-800">Tài khoản AI</h2>
        <button
          type="button"
          disabled={loading}
          className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
          onClick={() => void load()}
        >
          Làm mới
        </button>
        {loading ? <span className="text-xs text-slate-400">Đang tải…</span> : null}
      </div>

      {error ? (
        <p className="text-xs text-red-600">Không tải được danh sách tài khoản AI: {error}</p>
      ) : null}

      {rows.length === 0 && !loading ? (
        <p className="text-sm text-slate-400">Chưa có tài khoản AI nào — thêm ở form bên dưới.</p>
      ) : (
        GROUPS.map((group) => {
          const groupRows = rows.filter((r) => group.providers.includes(r.provider));
          if (groupRows.length === 0) {
            return null;
          }
          return (
            <section key={group.title} className="flex flex-col gap-1">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500">
                {group.title}
              </h3>
              <div className="overflow-auto rounded border border-slate-200">
                <table className="w-full min-w-[760px] border-collapse text-left text-sm">
                  <thead className="bg-slate-50 text-xs text-slate-500">
                    <tr>
                      <th className="px-2 py-1.5 font-medium">Nhãn</th>
                      <th className="px-2 py-1.5 font-medium">Nhà cung cấp</th>
                      <th className="px-2 py-1.5 font-medium">Loại</th>
                      <th className="px-2 py-1.5 font-medium">Bí mật (đã che)</th>
                      <th className="px-2 py-1.5 font-medium">Trạng thái</th>
                      <th className="px-2 py-1.5 font-medium">Bật</th>
                      <th className="px-2 py-1.5 font-medium">Proxy</th>
                      <th className="px-2 py-1.5 font-medium">Hành động</th>
                    </tr>
                  </thead>
                  <tbody>
                    {groupRows.map((row) => (
                      <CredentialRow
                        key={row.id}
                        row={row}
                        busy={(busy[row.id] ?? 0) > 0}
                        onVerify={verify}
                        onToggleEnabled={(id, enabled) => update(id, { enabled })}
                        onDelete={remove}
                      />
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          );
        })
      )}

      <AddCredentialForm />
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// Mot dong tai khoan AI.
// ---------------------------------------------------------------------------------------------

interface CredentialRowProps {
  row: CredentialView;
  busy: boolean;
  onVerify: (id: string) => Promise<unknown>;
  onToggleEnabled: (id: string, enabled: boolean) => Promise<void>;
  onDelete: (id: string) => Promise<boolean>;
}

function CredentialRow({ row, busy, onVerify, onToggleEnabled, onDelete }: CredentialRowProps) {
  const [verifyMsg, setVerifyMsg] = useState<string | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  async function handleVerify(): Promise<void> {
    setVerifyMsg(null);
    setRowError(null);
    try {
      await onVerify(row.id);
    } catch (e) {
      setRowError(describeCredentialError(e));
    }
  }

  async function handleToggle(enabled: boolean): Promise<void> {
    setRowError(null);
    try {
      await onToggleEnabled(row.id, enabled);
    } catch (e) {
      setRowError(describeCredentialError(e));
    }
  }

  async function handleDeleteConfirmed(): Promise<void> {
    setRowError(null);
    try {
      await onDelete(row.id);
      setConfirmingDelete(false);
    } catch (e) {
      setRowError(describeCredentialError(e));
      setConfirmingDelete(false);
    }
  }

  return (
    <tr className="border-b border-slate-100 align-top">
      <td className="px-2 py-1.5 font-medium text-slate-800">{row.label}</td>
      <td className="px-2 py-1.5 text-slate-600">{credentialProviderLabel(row.provider)}</td>
      <td className="px-2 py-1.5 text-slate-600">{credentialKindLabel(row.kind)}</td>
      <td className="px-2 py-1.5 font-mono text-xs text-slate-600">{row.secretHint}</td>
      <td className="px-2 py-1.5">
        <span
          className={`rounded px-1.5 py-0.5 text-xs font-medium ${CREDENTIAL_STATUS_BADGE_CLASS[row.status]}`}
        >
          {credentialStatusLabel(row.status)}
        </span>
        {verifyMsg ? <p className="mt-0.5 text-xs text-slate-500">{verifyMsg}</p> : null}
      </td>
      <td className="px-2 py-1.5 text-center">
        <input
          type="checkbox"
          checked={row.enabled}
          disabled={busy}
          onChange={(e) => void handleToggle(e.target.checked)}
          aria-label={`Bật tài khoản ${row.label}`}
        />
      </td>
      <td className="whitespace-nowrap px-2 py-1.5 font-mono text-xs text-slate-600">
        {row.proxyMasked === "" ? "—" : row.proxyMasked}
      </td>
      <td className="whitespace-nowrap px-2 py-1.5">
        <div className="flex items-center gap-2">
          <button
            type="button"
            disabled={busy}
            className="rounded border border-slate-300 px-2 py-0.5 text-xs hover:bg-slate-50 disabled:opacity-50"
            onClick={() => void handleVerify()}
          >
            {busy ? "…" : "Kiểm tra"}
          </button>
          {confirmingDelete ? (
            <span className="flex items-center gap-1 text-xs">
              Xoá thật?
              <button
                type="button"
                disabled={busy}
                className="rounded border border-red-300 px-1.5 py-0.5 text-red-700 hover:bg-red-50 disabled:opacity-50"
                onClick={() => void handleDeleteConfirmed()}
              >
                Có
              </button>
              <button
                type="button"
                disabled={busy}
                className="rounded border border-slate-300 px-1.5 py-0.5 hover:bg-slate-50 disabled:opacity-50"
                onClick={() => setConfirmingDelete(false)}
              >
                Không
              </button>
            </span>
          ) : (
            <button
              type="button"
              disabled={busy}
              className="rounded border border-red-300 px-2 py-0.5 text-xs text-red-700 hover:bg-red-50 disabled:opacity-50"
              onClick={() => setConfirmingDelete(true)}
            >
              Xoá
            </button>
          )}
        </div>
        {rowError ? <p className="mt-0.5 text-xs text-red-600">{rowError}</p> : null}
      </td>
    </tr>
  );
}

// ---------------------------------------------------------------------------------------------
// Form "Thêm tài khoản AI".
// ---------------------------------------------------------------------------------------------

function AddCredentialForm() {
  const add = useCredentialsStore((s) => s.add);

  const [provider, setProvider] = useState<CredentialProvider>("dola");
  const allowedKinds = KINDS_BY_PROVIDER[provider];
  const [kind, setKind] = useState<CredentialKind>(allowedKinds[0]!);
  const [label, setLabel] = useState("");
  const [secret, setSecret] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [proxy, setProxy] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);

  // api_key -> o mot dong kieu mat khau; session/login -> textarea (cookie/noi dung dai).
  const secretIsApiKey = kind === "api_key";
  const usesBaseUrl = providerUsesBaseUrl(provider);
  const proxyError = useMemo(() => (proxy === "" ? null : validateProxy(proxy)), [proxy]);

  function handleProviderChange(next: CredentialProvider): void {
    setProvider(next);
    // Doi kind ve gia tri hop le dau tien cua provider moi.
    setKind(KINDS_BY_PROVIDER[next][0]!);
  }

  const canSubmit =
    !busy && label.trim() !== "" && secret !== "" && (proxy === "" || proxyError === null);

  async function handleSubmit(e: FormEvent<HTMLFormElement>): Promise<void> {
    e.preventDefault();
    if (!canSubmit) {
      return;
    }
    // Doc bi mat ra bien cuc bo roi XOA NGAY o (truoc ca khi cho IPC) — xem ghi chu AN TOAN.
    const secretToSend = secret;
    setSecret("");
    setBusy(true);
    setError(null);
    setOkMsg(null);
    const meta =
      usesBaseUrl && baseUrl.trim() !== "" ? { base_url: baseUrl.trim() } : undefined;
    try {
      const view = await add({
        provider,
        kind,
        label: label.trim(),
        secret: secretToSend,
        meta,
        proxy: proxy.trim() === "" ? undefined : proxy.trim(),
      });
      setOkMsg(`Đã thêm "${view.label}".`);
      setLabel("");
      setBaseUrl("");
      setProxy("");
    } catch (err) {
      setError(describeCredentialError(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="mt-2 rounded border border-slate-200 p-3">
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
        Thêm tài khoản AI
      </h3>
      <form className="flex flex-col gap-3" onSubmit={(e) => void handleSubmit(e)}>
        <div className="flex flex-wrap gap-3">
          <label className="flex flex-col gap-1 text-xs">
            Nhà cung cấp
            <select
              className="rounded border border-slate-300 px-2 py-1 text-sm"
              value={provider}
              disabled={busy}
              onChange={(e) => handleProviderChange(e.target.value as CredentialProvider)}
            >
              {PROVIDER_ORDER.map((p) => (
                <option key={p} value={p}>
                  {credentialProviderLabel(p)}
                </option>
              ))}
            </select>
          </label>

          <label className="flex flex-col gap-1 text-xs">
            Loại
            <select
              className="rounded border border-slate-300 px-2 py-1 text-sm"
              value={kind}
              disabled={busy || allowedKinds.length === 1}
              onChange={(e) => setKind(e.target.value as CredentialKind)}
            >
              {allowedKinds.map((k) => (
                <option key={k} value={k}>
                  {credentialKindLabel(k)}
                </option>
              ))}
            </select>
          </label>

          <label className="flex flex-1 flex-col gap-1 text-xs">
            Nhãn
            <input
              className="rounded border border-slate-300 px-2 py-1 text-sm"
              value={label}
              disabled={busy}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="vd OpenAI chính"
            />
          </label>
        </div>

        <label className="flex flex-col gap-1 text-xs">
          {secretIsApiKey ? "API key" : "Bí mật (cookie/đăng nhập)"}
          {secretIsApiKey ? (
            <input
              type="password"
              autoComplete="off"
              className="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
              value={secret}
              disabled={busy}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="sk-…"
            />
          ) : (
            <textarea
              className="h-28 resize-none rounded border border-slate-300 px-2 py-1 font-mono text-xs"
              value={secret}
              disabled={busy}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="Dán cookie/nội dung đăng nhập vào đây"
            />
          )}
          <span className="text-slate-400">
            Bí mật chỉ ghi một chiều — không hiển thị lại, xoá ngay khi bấm "Thêm".
          </span>
        </label>

        {usesBaseUrl ? (
          <label className="flex flex-col gap-1 text-xs">
            Base URL (tuỳ chọn)
            <input
              className="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
              value={baseUrl}
              disabled={busy}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="để trống = mặc định của nhà cung cấp"
            />
          </label>
        ) : null}

        <label className="flex flex-col gap-1 text-xs">
          Proxy (tuỳ chọn)
          <input
            className="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
            value={proxy}
            disabled={busy}
            onChange={(e) => setProxy(e.target.value)}
            placeholder="để trống, hoặc ip:port, hoặc direct"
          />
          {proxyError ? <span className="text-red-600">{proxyError}</span> : null}
        </label>

        {error ? <p className="text-xs text-red-600">{error}</p> : null}
        {okMsg ? <p className="text-xs text-emerald-700">{okMsg}</p> : null}

        <div className="flex justify-end">
          <button
            type="submit"
            disabled={!canSubmit}
            className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700 disabled:opacity-50"
          >
            {busy ? "Đang thêm…" : "Thêm"}
          </button>
        </div>
      </form>
    </section>
  );
}
