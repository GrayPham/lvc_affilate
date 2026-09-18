// Mot dong bang trong man Tai khoan (Task 14): ten, badge dang nhap, toggle lich, o proxy inline
// (`ProxyEditor`), luot dung/han muc gon, han cooldown/rate/quota con lai, nut Kiem tra/Xoa.
// Nhan cac HAM HANH DONG tu `AccountsPage` (goi thang store `useAccountsStore`) thay vi tu doc
// store o day — de component nay thuan tuy la trinh bay + trang thai UI cuc bo (loi inline, xac
// nhan xoa 2 buoc), de test/tai su dung.
import { useState } from "react";

import {
  accountStatusLabel,
  describeAccountError,
  formatRemaining,
  formatUsageCompact,
} from "../format/account";
import type { AccountStatus } from "../ipc/types";
import ProxyEditor from "./ProxyEditor";

interface AccountRowProps {
  row: AccountStatus;
  /** `true` khi MOT lenh IPC nao do dang chay cho tai khoan nay (tu `store.busy[row.name]`). */
  busy: boolean;
  /** Epoch giay dung chung cho ca bang — tranh moi dong tinh "bay gio" khac nhau chut it. */
  now: number;
  onVerify: (name: string) => Promise<unknown>;
  onToggleScheduling: (name: string, on: boolean) => Promise<void>;
  onSaveProxy: (name: string, proxy: string) => Promise<void>;
  onDelete: (name: string) => Promise<boolean>;
}

const LOGIN_BADGE_CLASS: Record<ReturnType<typeof accountStatusLabel>, string> = {
  OK: "bg-emerald-100 text-emerald-700",
  "Hết phiên": "bg-red-100 text-red-700",
  "Chưa rõ": "bg-slate-100 text-slate-600",
};

export default function AccountRow({
  row,
  busy,
  now,
  onVerify,
  onToggleScheduling,
  onSaveProxy,
  onDelete,
}: AccountRowProps) {
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const [schedulingError, setSchedulingError] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const loginLabel = accountStatusLabel(row);

  async function handleVerify(): Promise<void> {
    setVerifyError(null);
    try {
      await onVerify(row.name);
    } catch (e) {
      setVerifyError(describeAccountError(e));
    }
  }

  async function handleToggleScheduling(on: boolean): Promise<void> {
    setSchedulingError(null);
    try {
      await onToggleScheduling(row.name, on);
    } catch (e) {
      setSchedulingError(describeAccountError(e));
    }
  }

  async function handleDeleteConfirmed(): Promise<void> {
    setDeleteError(null);
    try {
      await onDelete(row.name);
      setConfirmingDelete(false);
    } catch (e) {
      setDeleteError(describeAccountError(e));
      setConfirmingDelete(false);
    }
  }

  return (
    <tr className="border-b border-slate-100 align-top">
      <td className="px-2 py-1.5 font-medium text-slate-800">{row.name}</td>
      <td className="px-2 py-1.5">
        <span className={`rounded px-1.5 py-0.5 text-xs font-medium ${LOGIN_BADGE_CLASS[loginLabel]}`}>
          {loginLabel}
        </span>
        {verifyError ? <p className="mt-0.5 text-xs text-red-600">{verifyError}</p> : null}
      </td>
      <td className="px-2 py-1.5 text-center">
        <input
          type="checkbox"
          checked={row.scheduling}
          disabled={busy}
          onChange={(e) => void handleToggleScheduling(e.target.checked)}
          aria-label={`Lịch tài khoản ${row.name}`}
        />
        {schedulingError ? <p className="mt-0.5 text-xs text-red-600">{schedulingError}</p> : null}
      </td>
      <td className="px-2 py-1.5">
        <ProxyEditor name={row.name} value={row.proxy} busy={busy} onSave={onSaveProxy} />
      </td>
      <td className="whitespace-nowrap px-2 py-1.5 font-mono text-xs text-slate-600">
        {formatUsageCompact(row)}
      </td>
      <td className="whitespace-nowrap px-2 py-1.5 text-xs text-slate-600">
        {formatRemaining(row, now)}
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
        {deleteError ? <p className="mt-0.5 text-xs text-red-600">{deleteError}</p> : null}
      </td>
    </tr>
  );
}
