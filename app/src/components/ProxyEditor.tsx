// O sua proxy inline cho MOT dong `AccountRow` (Task 14). `value` la chuoi DA CHE (`mask_proxy`
// phia Rust) — KHONG BAO GIO dung no de doi sua (khong the "un-mask"): vao che do sua luon bat
// dau tu o TRONG, nguoi dung go proxy MOI hoan toan (hoac de trong = bo proxy rieng, dung chung;
// "direct" = chan han proxy chung). Validate truoc phia client (`validateProxy`) de bao loi ngay
// khong doi round-trip; loi server (`INVALID_PROXY`) van duoc hien y nguyen qua `onSave` throw.
import { useState } from "react";

import { describeAccountError, validateProxy } from "../format/account";

interface ProxyEditorProps {
  name: string;
  /** Gia tri DA CHE tra ve tu `account_list` (`AccountStatus.proxy`). */
  value: string;
  onSave: (name: string, proxy: string) => Promise<void>;
}

export default function ProxyEditor({ name, value, onSave }: ProxyEditorProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function startEdit(): void {
    setDraft("");
    setError(null);
    setEditing(true);
  }

  function cancelEdit(): void {
    setEditing(false);
    setError(null);
  }

  async function handleSave(): Promise<void> {
    const clientError = validateProxy(draft);
    if (clientError) {
      setError(clientError);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await onSave(name, draft);
      setEditing(false);
    } catch (e) {
      setError(describeAccountError(e));
    } finally {
      setBusy(false);
    }
  }

  if (!editing) {
    return (
      <div className="flex items-center gap-1">
        <span className="font-mono text-xs text-slate-700">
          {value === "" ? "(chưa đặt)" : value}
        </span>
        <button
          type="button"
          className="text-xs text-sky-600 hover:underline"
          onClick={startEdit}
        >
          Sửa
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-1">
        <input
          autoFocus
          className="w-40 rounded border border-slate-300 px-1 py-0.5 font-mono text-xs"
          value={draft}
          disabled={busy}
          placeholder="ip:port hoặc direct"
          onChange={(e) => setDraft(e.target.value)}
        />
        <button
          type="button"
          disabled={busy}
          className="text-xs text-emerald-700 hover:underline disabled:opacity-50"
          onClick={() => void handleSave()}
        >
          Lưu
        </button>
        <button
          type="button"
          disabled={busy}
          className="text-xs text-slate-500 hover:underline disabled:opacity-50"
          onClick={cancelEdit}
        >
          Huỷ
        </button>
      </div>
      {error ? <span className="text-xs text-red-600">{error}</span> : null}
    </div>
  );
}
