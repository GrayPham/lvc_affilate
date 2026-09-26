// Man Tai khoan (Task 14) — bang danh sach + toolbar "Nạp cookie"/"Làm mới". Logic IPC/optimistic
// nam trong `store/accounts.ts`; dong bang la `components/AccountRow.tsx` (proxy inline qua
// `ProxyEditor`); hop thoai nap cookie la `components/CookieImport.tsx`.
import { useEffect, useState } from "react";

import AccountRow from "../components/AccountRow";
import CookieImport from "../components/CookieImport";
import { LoginCookieManager } from "./CredentialsPage";
import { useAccountsStore } from "../store/accounts";

const NOW_REFRESH_MS = 30_000;

export default function AccountsPage() {
  const rows = useAccountsStore((s) => s.rows);
  const loading = useAccountsStore((s) => s.loading);
  const error = useAccountsStore((s) => s.error);
  const busy = useAccountsStore((s) => s.busy);
  const load = useAccountsStore((s) => s.load);
  const verify = useAccountsStore((s) => s.verify);
  const setScheduling = useAccountsStore((s) => s.setScheduling);
  const setProxy = useAccountsStore((s) => s.setProxy);
  const remove = useAccountsStore((s) => s.remove);

  const [showImport, setShowImport] = useState(false);
  // Epoch giay dung chung cho tinh "con lai bao nhieu phut" cua ca bang — lam moi dinh ky de
  // dong ho khong dung im, khong can chinh xac tung giay cho muc dich nay.
  const [now, setNow] = useState(() => Date.now() / 1000);

  // Chon nhieu dong theo `row.name` + menu chuot phai (theo mau CredentialArea).
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [menu, setMenu] = useState<{ name: string; x: number; y: number } | null>(null);
  const [bulkBusy, setBulkBusy] = useState(false);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const id = setInterval(() => setNow(Date.now() / 1000), NOW_REFRESH_MS);
    return () => clearInterval(id);
  }, []);

  // Bo chon nhung ten khong con trong danh sach (vd sau khi xoa/lam moi).
  useEffect(() => {
    setSelected((prev) => {
      const names = new Set(rows.map((r) => r.name));
      const next = new Set([...prev].filter((n) => names.has(n)));
      return next.size === prev.size ? prev : next;
    });
  }, [rows]);

  // Dong menu chuot phai khi bam ra ngoai / cuon.
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

  const selCount = rows.filter((r) => selected.has(r.name)).length;
  const allSel = selCount === rows.length && rows.length > 0;

  function toggleOne(name: string, on: boolean): void {
    setSelected((prev) => {
      const next = new Set(prev);
      if (on) {
        next.add(name);
      } else {
        next.delete(name);
      }
      return next;
    });
  }
  function toggleAll(on: boolean): void {
    setSelected(on ? new Set(rows.map((r) => r.name)) : new Set());
  }

  async function bulkSchedule(on: boolean): Promise<void> {
    setBulkBusy(true);
    try {
      for (const r of rows.filter((x) => selected.has(x.name))) {
        await setScheduling(r.name, on);
      }
    } finally {
      setBulkBusy(false);
    }
  }
  async function bulkDelete(): Promise<void> {
    setBulkBusy(true);
    try {
      for (const r of rows.filter((x) => selected.has(x.name))) {
        await remove(r.name);
      }
      setSelected(new Set());
    } finally {
      setBulkBusy(false);
    }
  }

  const menuRow = menu ? rows.find((r) => r.name === menu.name) : undefined;

  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-3">
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-semibold text-slate-800">Tài khoản</h2>
        <button
          type="button"
          className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700"
          onClick={() => setShowImport(true)}
        >
          Nạp cookie Dola
        </button>
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
        <p className="text-xs text-red-600">Không tải được danh sách tài khoản: {error}</p>
      ) : null}

      {selCount > 0 ? (
        <div className="flex items-center gap-2 rounded bg-slate-100 px-2 py-1 text-xs">
          <span className="text-slate-600">Đã chọn {selCount}</span>
          <button
            type="button"
            disabled={bulkBusy}
            className="rounded border border-slate-300 bg-white px-2 py-0.5 hover:bg-slate-50 disabled:opacity-50"
            onClick={() => void bulkSchedule(true)}
          >
            Bật lịch
          </button>
          <button
            type="button"
            disabled={bulkBusy}
            className="rounded border border-slate-300 bg-white px-2 py-0.5 hover:bg-slate-50 disabled:opacity-50"
            onClick={() => void bulkSchedule(false)}
          >
            Tắt lịch
          </button>
          <button
            type="button"
            disabled={bulkBusy}
            className="rounded border border-red-300 bg-white px-2 py-0.5 text-red-700 hover:bg-red-50 disabled:opacity-50"
            onClick={() => void bulkDelete()}
          >
            {bulkBusy ? "Đang xoá…" : "Xoá đã chọn"}
          </button>
          <button
            type="button"
            className="ml-auto text-slate-400 hover:text-slate-700"
            onClick={() => setSelected(new Set())}
          >
            Bỏ chọn
          </button>
        </div>
      ) : null}

      {rows.length === 0 && !loading ? (
        <p className="text-sm text-slate-400">Chưa có tài khoản nào — bấm "Nạp cookie" để thêm.</p>
      ) : (
        <div className="overflow-auto rounded border border-slate-200">
          <table className="w-full min-w-[720px] border-collapse text-left text-sm">
            <thead className="bg-slate-50 text-xs text-slate-500">
              <tr>
                <th className="w-8 px-2 py-1.5">
                  <input
                    type="checkbox"
                    checked={allSel}
                    onChange={(e) => toggleAll(e.target.checked)}
                    aria-label="Chọn tất cả"
                  />
                </th>
                <th className="px-2 py-1.5 font-medium">Tên</th>
                <th className="px-2 py-1.5 font-medium">Đăng nhập</th>
                <th className="px-2 py-1.5 font-medium">Lịch</th>
                <th className="px-2 py-1.5 font-medium">Proxy</th>
                <th className="px-2 py-1.5 font-medium">Dùng hôm nay/hạn mức</th>
                <th className="px-2 py-1.5 font-medium">Còn lại</th>
                <th className="px-2 py-1.5 font-medium">Hành động</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <AccountRow
                  key={row.name}
                  row={row}
                  busy={(busy[row.name] ?? 0) > 0}
                  now={now}
                  selected={selected.has(row.name)}
                  onSelect={toggleOne}
                  onContext={(name, x, y) => setMenu({ name, x, y })}
                  onVerify={verify}
                  onToggleScheduling={setScheduling}
                  onSaveProxy={setProxy}
                  onDelete={remove}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Menu chuot phai cho mot dong Dola */}
      {menu && menuRow ? (
        <div
          className="fixed z-50 min-w-[140px] rounded-md border border-slate-200 bg-white py-1 text-xs shadow-lg"
          style={{ left: menu.x, top: menu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="block w-full px-3 py-1.5 text-left hover:bg-slate-100"
            onClick={() => {
              setMenu(null);
              void verify(menuRow.name);
            }}
          >
            Kiểm tra
          </button>
          <button
            type="button"
            className="block w-full px-3 py-1.5 text-left hover:bg-slate-100"
            onClick={() => {
              setMenu(null);
              void setScheduling(menuRow.name, !menuRow.scheduling);
            }}
          >
            {menuRow.scheduling ? "Tắt lịch" : "Bật lịch"}
          </button>
          <button
            type="button"
            className="block w-full px-3 py-1.5 text-left text-red-700 hover:bg-red-50"
            onClick={() => {
              setMenu(null);
              void remove(menuRow.name);
            }}
          >
            Xoá
          </button>
        </div>
      ) : null}

      {/* Gộp: tài khoản đăng nhập/cookie các nền tảng khác (Google/Facebook/Apple/Gemini/ChatGPT)
          vào CÙNG tab "Tài khoản" — API key ở tab riêng. */}
      <div className="mt-4 border-t border-slate-200 pt-4">
        <LoginCookieManager />
      </div>

      {showImport ? <CookieImport onClose={() => setShowImport(false)} /> : null}
    </div>
  );
}
