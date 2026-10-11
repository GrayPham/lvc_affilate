// Kho tài khoản AI (P4c + mở rộng). Gộp UI theo yêu cầu người dùng:
// - `LoginCookieManager`: mọi tài khoản ĐĂNG NHẬP/COOKIE (mọi nền tảng) — nhúng vào tab "Tài khoản"
//   (cùng chỗ tài khoản Dola) qua `AccountsPage`.
// - `ApiKeyManager` (default export của trang này): chỉ API KEY (OpenAI/Gemini, key chính thức/
//   endpoint tuỳ chỉnh/SDK) — là tab "API key".
//
// AN TOAN BI MAT: ô nhập bí mật GHI-MỘT-CHIỀU — không điền sẵn/hiện lại; xoá NGAY khi bấm "Thêm".
import { useEffect, useMemo, useState, type FormEvent } from "react";

import { validateProxy } from "../format/account";
import {
  CREDENTIAL_STATUS_BADGE_CLASS,
  credentialModeLabel,
  credentialProviderLabel,
  credentialStatusLabel,
  describeCredentialError,
  rowBaseUrl,
  rowMode,
  type CredentialMode,
} from "../format/credential";
import { useCredentialsStore } from "../store/credentials";
import {
  loginImportCookieFiles,
  loginOpenInteractive,
  pickFiles,
  type InteractiveLoginResult,
  type LoginImportItem,
} from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { CredentialKind, CredentialProvider, CredentialView } from "../ipc/types";

// Loai bi mat cho phep theo provider (P4c contract) — dung suy `kind` tu che do.
const KINDS_BY_PROVIDER: Record<CredentialProvider, readonly CredentialKind[]> = {
  dola: ["session"],
  openai: ["api_key"],
  gemini: ["api_key", "session"],
  google: ["login", "session"],
  chatgpt_web: ["login", "session"],
  facebook: ["login", "session"],
  apple: ["login", "session"],
  seedance: ["api_key"],
  grok: ["api_key"],
};

// Che do API hop le theo provider.
const MODES_BY_PROVIDER: Record<CredentialProvider, readonly CredentialMode[]> = {
  dola: ["cookie"],
  google: ["cookie"],
  chatgpt_web: ["cookie"],
  openai: ["official", "custom", "sdk"],
  gemini: ["official", "custom", "sdk", "cookie"],
  facebook: ["cookie"],
  apple: ["cookie"],
  seedance: ["official", "custom"],
  grok: ["official", "custom"],
};

/** Nền tảng có API key (tab "API key"). */
const API_PROVIDERS: readonly CredentialProvider[] = ["openai", "gemini", "seedance", "grok"];

/** Nền tảng dùng cookie/đăng nhập (tab "Tài khoản"). */
const COOKIE_PROVIDERS: readonly CredentialProvider[] = [
  "dola",
  "google",
  "gemini",
  "facebook",
  "apple",
  "chatgpt_web",
];

/** Che do -> loai bi mat (kind) luu xuong backend. */
function modeToKind(provider: CredentialProvider, mode: CredentialMode): CredentialKind {
  return mode === "cookie" ? KINDS_BY_PROVIDER[provider][0]! : "api_key";
}

// ---------------------------------------------------------------------------------------------
// Tab "API key" (default export) — chỉ entry api_key.
// ---------------------------------------------------------------------------------------------

export default function ApiKeyManager() {
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

  const apiRows = rows.filter((r) => r.kind === "api_key");

  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-3">
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-semibold text-slate-800">API key</h2>
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
      {error ? <p className="text-xs text-red-600">Không tải được: {error}</p> : null}

      <CredentialArea
        title="Kho API (khoá / endpoint)"
        hint="OpenAI, Gemini — key chính thức, endpoint bên thứ 3, hoặc SDK."
        rows={apiRows}
        busy={busy}
        onVerify={verify}
        onToggle={(id, enabled) => update(id, { enabled })}
        onDelete={remove}
      />
      {apiRows.length === 0 && !loading ? (
        <p className="text-sm text-slate-400">Chưa có API key nào — thêm ở form bên dưới.</p>
      ) : null}

      <AddCredentialForm scope="api" />
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// Quản lý tài khoản ĐĂNG NHẬP/COOKIE — nhúng vào tab "Tài khoản" (AccountsPage).
// ---------------------------------------------------------------------------------------------

export function LoginCookieManager() {
  const rows = useCredentialsStore((s) => s.rows);
  const busy = useCredentialsStore((s) => s.busy);
  const load = useCredentialsStore((s) => s.load);
  const verify = useCredentialsStore((s) => s.verify);
  const update = useCredentialsStore((s) => s.update);
  const remove = useCredentialsStore((s) => s.remove);

  useEffect(() => {
    void load();
  }, [load]);

  const loginRows = rows.filter((r) => r.kind !== "api_key");

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-sm font-semibold text-slate-800">Tài khoản đăng nhập / Cookie (nền tảng khác)</h2>
      <InteractiveLogin onDone={() => void load()} />
      <BulkCookieLogin onDone={() => void load()} />
      <CredentialArea
        title="Kho đăng nhập / Cookie"
        hint="Dola, Google, Gemini, Facebook, Apple, ChatGPT — phiên/đăng nhập (cookie)."
        rows={loginRows}
        busy={busy}
        onVerify={verify}
        onToggle={(id, enabled) => update(id, { enabled })}
        onDelete={remove}
      />
      <AddCredentialForm scope="cookie" />
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// (B) Đăng nhập THỦ CÔNG có hỗ trợ — mở cửa sổ trình duyệt THẬT cho người dùng tự đăng nhập.
// ĐƯỜNG HỢP LỆ: không tự điền email/mật khẩu, không tính TOTP/2FA, không né bot.
// ---------------------------------------------------------------------------------------------

function InteractiveLogin({ onDone }: { onDone: () => void }) {
  const [provider, setProvider] = useState<CredentialProvider>("google");
  const [label, setLabel] = useState("");
  const [proxy, setProxy] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<InteractiveLoginResult | null>(null);

  const canSubmit = !busy && label.trim() !== "";

  async function open(): Promise<void> {
    if (!canSubmit) {
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const res = await loginOpenInteractive(provider, label.trim(), proxy.trim());
      setResult(res);
      onDone();
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="rounded border border-dashed border-sky-300 bg-sky-50 p-3">
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-sky-700">
        Đăng nhập thủ công (mở trình duyệt)
      </h3>
      <div className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col gap-1 text-xs">
          Nền tảng
          <select
            className="rounded border border-slate-300 px-2 py-1 text-sm"
            value={provider}
            disabled={busy}
            onChange={(e) => setProvider(e.target.value as CredentialProvider)}
          >
            {COOKIE_PROVIDERS.map((p) => (
              <option key={p} value={p}>
                {credentialProviderLabel(p)}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          Nhãn
          <input
            className="rounded border border-slate-300 px-2 py-1 text-sm"
            value={label}
            disabled={busy}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="vd Gmail acc1"
          />
        </label>
        <label className="flex flex-1 flex-col gap-1 text-xs">
          Proxy (tuỳ chọn)
          <input
            className="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
            value={proxy}
            disabled={busy}
            onChange={(e) => setProxy(e.target.value)}
            placeholder="để trống, hoặc ip:port"
          />
        </label>
        <button
          type="button"
          disabled={!canSubmit}
          className="rounded bg-sky-700 px-3 py-1 text-xs text-white hover:bg-sky-600 disabled:opacity-50"
          onClick={() => void open()}
        >
          {busy ? "Đang mở…" : "Mở trình duyệt đăng nhập"}
        </button>
      </div>
      <p className="mt-1 text-[11px] text-slate-500">
        Cửa sổ trình duyệt sẽ mở tới trang đăng nhập. Bạn TỰ đăng nhập trong cửa sổ đó; phiên được lưu
        vào profile để tái dùng. Ứng dụng không tự điền thông tin đăng nhập.
      </p>
      {error ? <p className="mt-1 text-xs text-red-600">{error}</p> : null}
      {result ? (
        <p className="mt-1 text-xs text-emerald-700">
          Đã mở &amp; lưu &quot;{result.label}&quot; ({credentialProviderLabel(result.provider as CredentialProvider)}).
          Hãy đăng nhập trong cửa sổ vừa mở.
        </p>
      ) : null}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// (A) Nạp cookie HÀNG LOẠT.
// ---------------------------------------------------------------------------------------------

function BulkCookieLogin({ onDone }: { onDone: () => void }) {
  const [provider, setProvider] = useState<CredentialProvider>("dola");
  const [proxy, setProxy] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [items, setItems] = useState<LoginImportItem[] | null>(null);

  async function pick(): Promise<void> {
    setError(null);
    let paths: string[] = [];
    try {
      paths = await pickFiles(false);
    } catch (e) {
      setError(errorText(e));
      return;
    }
    if (paths.length === 0) {
      return;
    }
    setBusy(true);
    try {
      const res = await loginImportCookieFiles(provider, paths, proxy.trim());
      setItems(res);
      onDone();
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="rounded border border-dashed border-slate-300 bg-slate-50 p-3">
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
        Nạp cookie hàng loạt (nhiều file)
      </h3>
      <div className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col gap-1 text-xs">
          Nền tảng
          <select
            className="rounded border border-slate-300 px-2 py-1 text-sm"
            value={provider}
            disabled={busy}
            onChange={(e) => setProvider(e.target.value as CredentialProvider)}
          >
            {COOKIE_PROVIDERS.map((p) => (
              <option key={p} value={p}>
                {credentialProviderLabel(p)}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-1 flex-col gap-1 text-xs">
          Proxy chung (tuỳ chọn)
          <input
            className="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
            value={proxy}
            disabled={busy}
            onChange={(e) => setProxy(e.target.value)}
            placeholder="để trống, hoặc ip:port"
          />
        </label>
        <button
          type="button"
          disabled={busy}
          className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700 disabled:opacity-50"
          onClick={() => void pick()}
        >
          {busy ? "Đang nạp…" : "Chọn nhiều file cookie…"}
        </button>
      </div>
      <p className="mt-1 text-[11px] text-slate-400">
        Mỗi file = một tài khoản (tên = tên file), lưu dạng phiên (cookie). Đường hợp lệ — bạn tự xuất
        cookie đã đăng nhập.
      </p>
      {error ? <p className="mt-1 text-xs text-red-600">{error}</p> : null}
      {items ? (
        <div className="mt-2 max-h-40 overflow-auto rounded border border-slate-200 bg-white">
          {items.map((b, i) => (
            <div key={`${b.label}-${i}`} className="flex items-start gap-2 border-b border-slate-100 px-2 py-1 text-xs last:border-b-0">
              <span className={b.ok ? "text-emerald-600" : "text-red-600"}>{b.ok ? "✓" : "✗"}</span>
              <span className="font-medium text-slate-800">{b.label}</span>
              {b.error ? <span className="text-red-600">— {b.error}</span> : null}
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// Một khu bảng — nhóm theo provider.
// ---------------------------------------------------------------------------------------------

interface AreaProps {
  title: string;
  hint: string;
  rows: CredentialView[];
  busy: Record<string, number>;
  onVerify: (id: string) => Promise<unknown>;
  onToggle: (id: string, enabled: boolean) => Promise<void>;
  onDelete: (id: string) => Promise<boolean>;
}

function CredentialArea({ title, hint, rows, busy, onVerify, onToggle, onDelete }: AreaProps) {
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null);
  const [bulkBusy, setBulkBusy] = useState(false);

  // Đóng menu chuột phải khi bấm ra ngoài.
  useEffect(() => {
    if (!menu) {
      return;
    }
    const close = (): void => setMenu(null);
    window.addEventListener("click", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [menu]);

  if (rows.length === 0) {
    return null;
  }
  const providers = COOKIE_PROVIDERS.concat(API_PROVIDERS).filter(
    (p, i, a) => a.indexOf(p) === i && rows.some((r) => r.provider === p),
  );
  const ordered = providers.flatMap((p) => rows.filter((r) => r.provider === p));
  const selCount = ordered.filter((r) => selected.has(r.id)).length;
  const allSel = selCount === ordered.length && ordered.length > 0;

  function toggleOne(id: string, on: boolean): void {
    setSelected((prev) => {
      const next = new Set(prev);
      if (on) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  }
  function toggleAll(on: boolean): void {
    setSelected(on ? new Set(ordered.map((r) => r.id)) : new Set());
  }

  async function bulkDelete(): Promise<void> {
    setBulkBusy(true);
    try {
      for (const r of ordered.filter((x) => selected.has(x.id))) {
        await onDelete(r.id);
      }
      setSelected(new Set());
    } finally {
      setBulkBusy(false);
    }
  }
  async function bulkEnable(on: boolean): Promise<void> {
    setBulkBusy(true);
    try {
      for (const r of ordered.filter((x) => selected.has(x.id))) {
        await onToggle(r.id, on);
      }
    } finally {
      setBulkBusy(false);
    }
  }

  const menuRow = menu ? ordered.find((r) => r.id === menu.id) : undefined;

  return (
    <section className="flex flex-col gap-1">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-600">{title}</h3>
      <p className="text-[11px] text-slate-400">{hint}</p>

      {/* Thanh thao tác hàng loạt — hiện khi có dòng được chọn */}
      {selCount > 0 ? (
        <div className="flex items-center gap-2 rounded bg-slate-100 px-2 py-1 text-xs">
          <span className="text-slate-600">Đã chọn {selCount}</span>
          <button type="button" disabled={bulkBusy} className="rounded border border-slate-300 bg-white px-2 py-0.5 hover:bg-slate-50 disabled:opacity-50" onClick={() => void bulkEnable(true)}>
            Bật
          </button>
          <button type="button" disabled={bulkBusy} className="rounded border border-slate-300 bg-white px-2 py-0.5 hover:bg-slate-50 disabled:opacity-50" onClick={() => void bulkEnable(false)}>
            Tắt
          </button>
          <button type="button" disabled={bulkBusy} className="rounded border border-red-300 bg-white px-2 py-0.5 text-red-700 hover:bg-red-50 disabled:opacity-50" onClick={() => void bulkDelete()}>
            {bulkBusy ? "Đang xoá…" : "Xoá đã chọn"}
          </button>
          <button type="button" className="ml-auto text-slate-400 hover:text-slate-700" onClick={() => setSelected(new Set())}>
            Bỏ chọn
          </button>
        </div>
      ) : null}

      <div className="overflow-auto rounded border border-slate-200">
        <table className="w-full min-w-[860px] border-collapse text-left text-sm">
          <thead className="bg-slate-50 text-xs text-slate-500">
            <tr>
              <th className="w-8 px-2 py-1.5">
                <input type="checkbox" checked={allSel} onChange={(e) => toggleAll(e.target.checked)} aria-label="Chọn tất cả" />
              </th>
              <th className="px-2 py-1.5 font-medium">Nhãn</th>
              <th className="px-2 py-1.5 font-medium">Nền tảng</th>
              <th className="px-2 py-1.5 font-medium">Chế độ</th>
              <th className="px-2 py-1.5 font-medium">Bí mật (đã che)</th>
              <th className="px-2 py-1.5 font-medium">Trạng thái</th>
              <th className="px-2 py-1.5 font-medium">Bật</th>
              <th className="px-2 py-1.5 font-medium">Proxy</th>
              <th className="px-2 py-1.5 font-medium">Hành động</th>
            </tr>
          </thead>
          <tbody>
            {ordered.map((row) => (
              <CredentialRow
                key={row.id}
                row={row}
                busy={(busy[row.id] ?? 0) > 0}
                selected={selected.has(row.id)}
                onSelect={toggleOne}
                onContext={(id, x, y) => setMenu({ id, x, y })}
                onVerify={onVerify}
                onToggleEnabled={onToggle}
                onDelete={onDelete}
              />
            ))}
          </tbody>
        </table>
      </div>

      {/* Menu chuột phải */}
      {menu && menuRow ? (
        <div
          className="fixed z-50 min-w-[140px] rounded-md border border-slate-200 bg-white py-1 text-xs shadow-lg"
          style={{ left: menu.x, top: menu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button type="button" className="block w-full px-3 py-1.5 text-left hover:bg-slate-100" onClick={() => { setMenu(null); void onVerify(menuRow.id); }}>
            Kiểm tra
          </button>
          <button type="button" className="block w-full px-3 py-1.5 text-left hover:bg-slate-100" onClick={() => { setMenu(null); void onToggle(menuRow.id, !menuRow.enabled); }}>
            {menuRow.enabled ? "Tắt" : "Bật"}
          </button>
          <button type="button" className="block w-full px-3 py-1.5 text-left text-red-700 hover:bg-red-50" onClick={() => { setMenu(null); void onDelete(menuRow.id); }}>
            Xoá
          </button>
        </div>
      ) : null}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// Một dòng.
// ---------------------------------------------------------------------------------------------

interface CredentialRowProps {
  row: CredentialView;
  busy: boolean;
  selected: boolean;
  onSelect: (id: string, on: boolean) => void;
  onContext: (id: string, x: number, y: number) => void;
  onVerify: (id: string) => Promise<unknown>;
  onToggleEnabled: (id: string, enabled: boolean) => Promise<void>;
  onDelete: (id: string) => Promise<boolean>;
}

function CredentialRow({ row, busy, selected, onSelect, onContext, onVerify, onToggleEnabled, onDelete }: CredentialRowProps) {
  const [verifyMsg, setVerifyMsg] = useState<string | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const mode = rowMode(row.meta, row.kind);
  const baseUrl = rowBaseUrl(row.meta);

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
    <tr
      className={"border-b border-slate-100 align-top " + (selected ? "bg-sky-50" : "")}
      onContextMenu={(e) => {
        e.preventDefault();
        onContext(row.id, e.clientX, e.clientY);
      }}
    >
      <td className="px-2 py-1.5">
        <input type="checkbox" checked={selected} onChange={(e) => onSelect(row.id, e.target.checked)} aria-label={`Chọn ${row.label}`} />
      </td>
      <td className="px-2 py-1.5 font-medium text-slate-800">{row.label}</td>
      <td className="px-2 py-1.5 text-slate-600">{credentialProviderLabel(row.provider)}</td>
      <td className="px-2 py-1.5 text-slate-600">
        {credentialModeLabel(mode)}
        {baseUrl ? (
          <span className="block max-w-[220px] truncate font-mono text-[10px] text-slate-400" title={baseUrl}>
            {baseUrl}
          </span>
        ) : null}
      </td>
      <td className="px-2 py-1.5 font-mono text-xs text-slate-600">{row.secretHint}</td>
      <td className="px-2 py-1.5">
        <span className={`rounded px-1.5 py-0.5 text-xs font-medium ${CREDENTIAL_STATUS_BADGE_CLASS[row.status]}`}>
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
              <button type="button" disabled={busy} className="rounded border border-red-300 px-1.5 py-0.5 text-red-700 hover:bg-red-50 disabled:opacity-50" onClick={() => void handleDeleteConfirmed()}>
                Có
              </button>
              <button type="button" disabled={busy} className="rounded border border-slate-300 px-1.5 py-0.5 hover:bg-slate-50 disabled:opacity-50" onClick={() => setConfirmingDelete(false)}>
                Không
              </button>
            </span>
          ) : (
            <button type="button" disabled={busy} className="rounded border border-red-300 px-2 py-0.5 text-xs text-red-700 hover:bg-red-50 disabled:opacity-50" onClick={() => setConfirmingDelete(true)}>
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
// Form "Thêm" — `scope` quyết định danh sách nền tảng + chế độ.
// ---------------------------------------------------------------------------------------------

function AddCredentialForm({ scope }: { scope: "api" | "cookie" }) {
  const add = useCredentialsStore((s) => s.add);
  const providers = scope === "api" ? API_PROVIDERS : COOKIE_PROVIDERS;

  const [provider, setProvider] = useState<CredentialProvider>(providers[0]!);
  const modesFor = (p: CredentialProvider): readonly CredentialMode[] =>
    MODES_BY_PROVIDER[p].filter((m) => (scope === "api" ? m !== "cookie" : m === "cookie"));
  const [mode, setMode] = useState<CredentialMode>(modesFor(providers[0]!)[0] ?? "cookie");
  const [label, setLabel] = useState("");
  const [secret, setSecret] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [proxy, setProxy] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);

  const allowedModes = modesFor(provider);
  const isCookie = mode === "cookie";
  const needsBaseUrl = mode === "custom";
  const proxyError = useMemo(() => (proxy === "" ? null : validateProxy(proxy)), [proxy]);

  function handleProviderChange(next: CredentialProvider): void {
    setProvider(next);
    setMode(modesFor(next)[0] ?? "cookie");
  }

  const canSubmit =
    !busy &&
    label.trim() !== "" &&
    secret !== "" &&
    (!needsBaseUrl || baseUrl.trim() !== "") &&
    (proxy === "" || proxyError === null);

  async function handleSubmit(e: FormEvent<HTMLFormElement>): Promise<void> {
    e.preventDefault();
    if (!canSubmit) {
      return;
    }
    const secretToSend = secret;
    setSecret("");
    setBusy(true);
    setError(null);
    setOkMsg(null);
    const meta: Record<string, unknown> = { mode };
    if (baseUrl.trim() !== "" && (mode === "custom" || mode === "sdk")) {
      meta.base_url = baseUrl.trim();
    }
    try {
      const view = await add({
        provider,
        kind: modeToKind(provider, mode),
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
        {scope === "api" ? "Thêm API key" : "Thêm tài khoản đăng nhập / cookie"}
      </h3>
      <form className="flex flex-col gap-3" onSubmit={(e) => void handleSubmit(e)}>
        <div className="flex flex-wrap gap-3">
          <label className="flex flex-col gap-1 text-xs">
            Nền tảng
            <select className="rounded border border-slate-300 px-2 py-1 text-sm" value={provider} disabled={busy} onChange={(e) => handleProviderChange(e.target.value as CredentialProvider)}>
              {providers.map((p) => (
                <option key={p} value={p}>
                  {credentialProviderLabel(p)}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">
            Chế độ
            <select className="rounded border border-slate-300 px-2 py-1 text-sm" value={mode} disabled={busy || allowedModes.length <= 1} onChange={(e) => setMode(e.target.value as CredentialMode)}>
              {allowedModes.map((m) => (
                <option key={m} value={m}>
                  {credentialModeLabel(m)}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs">
            Nhãn
            <input className="rounded border border-slate-300 px-2 py-1 text-sm" value={label} disabled={busy} onChange={(e) => setLabel(e.target.value)} placeholder={scope === "api" ? "vd Gemini chính chủ" : "vd Gmail acc1"} />
          </label>
        </div>

        <label className="flex flex-col gap-1 text-xs">
          {isCookie ? "Bí mật (cookie/đăng nhập)" : "API key"}
          {isCookie ? (
            <textarea className="h-28 resize-none rounded border border-slate-300 px-2 py-1 font-mono text-xs" value={secret} disabled={busy} onChange={(e) => setSecret(e.target.value)} placeholder="Dán cookie/nội dung đăng nhập vào đây" />
          ) : (
            <input type="password" autoComplete="off" className="rounded border border-slate-300 px-2 py-1 font-mono text-sm" value={secret} disabled={busy} onChange={(e) => setSecret(e.target.value)} placeholder="sk-… / API key" />
          )}
          <span className="text-slate-400">Bí mật chỉ ghi một chiều — không hiển thị lại, xoá ngay khi bấm "Thêm".</span>
        </label>

        {mode === "custom" || mode === "sdk" ? (
          <label className="flex flex-col gap-1 text-xs">
            Base URL {mode === "custom" ? "(bắt buộc — endpoint bên thứ 3)" : "(tuỳ chọn)"}
            <input className="rounded border border-slate-300 px-2 py-1 font-mono text-sm" value={baseUrl} disabled={busy} onChange={(e) => setBaseUrl(e.target.value)} placeholder="vd https://api.nhacungcap-thu-3.com/v1" />
          </label>
        ) : null}

        <label className="flex flex-col gap-1 text-xs">
          Proxy (tuỳ chọn)
          <input className="rounded border border-slate-300 px-2 py-1 font-mono text-sm" value={proxy} disabled={busy} onChange={(e) => setProxy(e.target.value)} placeholder="để trống, hoặc ip:port, hoặc direct" />
          {proxyError ? <span className="text-red-600">{proxyError}</span> : null}
        </label>

        {error ? <p className="text-xs text-red-600">{error}</p> : null}
        {okMsg ? <p className="text-xs text-emerald-700">{okMsg}</p> : null}

        <div className="flex justify-end">
          <button type="submit" disabled={!canSubmit} className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700 disabled:opacity-50">
            {busy ? "Đang thêm…" : "Thêm"}
          </button>
        </div>
      </form>
    </section>
  );
}
