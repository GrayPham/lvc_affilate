// Gan proxy HANG LOAT cho nhieu tai khoan da chon — dung theo
// `references/lvc-ui-design/screens/AssignProxy.dc.html` (ban "Lưu cho N tài khoản"). Chon MOT
// "Kiểu kết nối" + proxy roi ap cho moi dong da chon bang chinh cac lenh san co (Dola:
// `account_set_proxy` qua `useAccountsStore.setProxy`; nen tang khac: cap nhat proxy credential qua
// `useCredentialsStore.update`). Chay TUAN TU, bao ket qua tung dong; mot dong loi KHONG dung ca lo.
import { useState } from "react";

import ConnectionModeFields from "./ConnectionModeFields";
import Dialog from "./Dialog";
import type { EditTarget } from "./EditAccountDialog";
import { BTN_NEUTRAL_SM, BTN_PRIMARY_SM } from "../ui/buttonStyles";
import { useAccountsStore } from "../store/accounts";
import { useCredentialsStore } from "../store/credentials";
import { describeAccountError, validateProxy } from "../format/account";
import { describeCredentialError } from "../format/credential";
import { modeToProxy, type ConnMode } from "../format/proxyMode";

export interface AssignProxyRow {
  key: string;
  label: string;
  target: EditTarget;
}

interface RowResult {
  key: string;
  label: string;
  ok: boolean;
  error: string | null;
}

export default function AssignProxyDialog({
  rows,
  onClose,
}: {
  rows: AssignProxyRow[];
  onClose: () => void;
}) {
  const setProxyDola = useAccountsStore((s) => s.setProxy);
  const updateCredential = useCredentialsStore((s) => s.update);

  // Mac dinh "Proxy cố định" (truong hop gan hang loat hay gap nhat).
  const [mode, setMode] = useState<ConnMode>("fixed");
  const [proxyInput, setProxyInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<RowResult[] | null>(null);

  const targetProxy = modeToProxy(mode, proxyInput);
  // "fixed" phai nhap proxy hop le (rong -> ro rang la nham voi "Proxy chung").
  const fixedOk = mode !== "fixed" || (proxyInput.trim() !== "" && validateProxy(proxyInput) === null);
  const canSave = !busy && rows.length > 0 && fixedOk;

  async function applyOne(row: AssignProxyRow): Promise<RowResult> {
    try {
      if (row.target.source === "dola") {
        await setProxyDola(row.target.row.name, targetProxy);
      } else {
        await updateCredential(row.target.row.id, { proxy: targetProxy });
      }
      return { key: row.key, label: row.label, ok: true, error: null };
    } catch (e) {
      const msg =
        row.target.source === "dola" ? describeAccountError(e) : describeCredentialError(e);
      return { key: row.key, label: row.label, ok: false, error: msg };
    }
  }

  async function saveAll(): Promise<void> {
    setBusy(true);
    const out: RowResult[] = [];
    setResults([]);
    for (const row of rows) {
      // eslint-disable-next-line no-await-in-loop -- co y chay TUAN TU (tranh N lenh ghi song song).
      const r = await applyOne(row);
      out.push(r);
      setResults([...out]);
    }
    setBusy(false);
  }

  const doneCount = results?.length ?? 0;
  const okCount = results?.filter((r) => r.ok).length ?? 0;
  const allDone = results !== null && !busy && doneCount === rows.length;

  return (
    <Dialog title="Proxy & trình duyệt" onClose={onClose} width="34rem">
      <div className="flex flex-col gap-4">
        <p className="text-xs text-muted">
          Áp dụng cho <span className="font-semibold text-fg">{rows.length} tài khoản</span>:{" "}
          <span className="text-fg">
            {rows
              .slice(0, 6)
              .map((r) => r.label)
              .join(", ")}
            {rows.length > 6 ? `, +${rows.length - 6}…` : ""}
          </span>
        </p>

        <ConnectionModeFields
          mode={mode}
          onModeChange={setMode}
          proxyValue={proxyInput}
          onProxyChange={setProxyInput}
          disabled={busy}
        />

        {results !== null ? (
          <div className="flex flex-col gap-1.5">
            <div className="text-xs text-muted">
              {busy ? `Đang lưu ${doneCount}/${rows.length}…` : `Xong: ${okCount}/${rows.length} thành công`}
            </div>
            <ul className="flex max-h-40 flex-col gap-1 overflow-auto">
              {results.map((r) => (
                <li
                  key={r.key}
                  className="flex items-center gap-2 rounded-sm bg-fill px-2.5 py-1.5 text-[12.5px]"
                >
                  <span className={r.ok ? "text-ok" : "text-rose"}>{r.ok ? "✓" : "✕"}</span>
                  <span className="font-medium text-fg">{r.label}</span>
                  {r.error ? <span className="text-ink3">{r.error}</span> : null}
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        <div className="flex justify-end gap-2 border-t border-line pt-3">
          <button type="button" className={BTN_NEUTRAL_SM} onClick={onClose}>
            {allDone ? "Đóng" : "Huỷ"}
          </button>
          {!allDone ? (
            <button
              type="button"
              disabled={!canSave}
              className={BTN_PRIMARY_SM}
              onClick={() => void saveAll()}
            >
              {busy ? "Đang lưu…" : `Lưu cho ${rows.length} tài khoản`}
            </button>
          ) : null}
        </div>
      </div>
    </Dialog>
  );
}
