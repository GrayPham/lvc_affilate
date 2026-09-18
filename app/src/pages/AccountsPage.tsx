// Man Tai khoan (Task 14) — bang danh sach + toolbar "Nạp cookie"/"Làm mới". Logic IPC/optimistic
// nam trong `store/accounts.ts`; dong bang la `components/AccountRow.tsx` (proxy inline qua
// `ProxyEditor`); hop thoai nap cookie la `components/CookieImport.tsx`.
import { useEffect, useState } from "react";

import AccountRow from "../components/AccountRow";
import CookieImport from "../components/CookieImport";
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

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const id = setInterval(() => setNow(Date.now() / 1000), NOW_REFRESH_MS);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-3">
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-semibold text-slate-800">Tài khoản</h2>
        <button
          type="button"
          className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700"
          onClick={() => setShowImport(true)}
        >
          Nạp cookie
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

      {rows.length === 0 && !loading ? (
        <p className="text-sm text-slate-400">Chưa có tài khoản nào — bấm "Nạp cookie" để thêm.</p>
      ) : (
        <div className="overflow-auto rounded border border-slate-200">
          <table className="w-full min-w-[720px] border-collapse text-left text-sm">
            <thead className="bg-slate-50 text-xs text-slate-500">
              <tr>
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

      {showImport ? <CookieImport onClose={() => setShowImport(false)} /> : null}
    </div>
  );
}
