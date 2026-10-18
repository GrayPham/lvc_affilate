// Dialog "Proxy & trình duyệt" cho MOT dong trong bang tai khoan hop nhat (AccountsPage). Dung theo
// `references/lvc-ui-design/screens/AssignProxy.dc.html` (ban mot-tai-khoan): chon "Kiểu kết nối"
// (IP máy / Proxy cố định / Proxy chung) + kiem proxy + luu. Cac chuc nang khac (Kiểm tra/Mở trình
// duyệt/Bật-Tắt/Xoá) da tach ra nut rieng tren moi hang + menu chuot phai o `AccountsPage.tsx`.
//
// Engine Firefox Portable (credential co `meta.engine === "firefox_portable"`): hien BADGE + canh
// bao "chi dung tay" — tai khoan nay KHONG verify/chay workflow tu dong (khong co driver CDP cho
// Firefox, xem docs/superpowers/plans/2026-10-18-firefox-portable-cookie-profiles.md).
import { useState } from "react";

import ConnectionModeFields from "./ConnectionModeFields";
import Dialog from "./Dialog";
import { BTN_NEUTRAL_SM, BTN_PRIMARY_SM } from "../ui/buttonStyles";
import { useAccountsStore } from "../store/accounts";
import { useCredentialsStore } from "../store/credentials";
import { credentialProviderLabel, describeCredentialError } from "../format/credential";
import { describeAccountError, validateProxy } from "../format/account";
import { modeToProxy, proxyToMode } from "../format/proxyMode";
import type { AccountStatus, CredentialView } from "../ipc/types";

export type EditTarget =
  | { source: "dola"; row: AccountStatus }
  | { source: "credential"; row: CredentialView };

/** `true` khi credential gan engine Firefox Portable (`meta.engine`). Dola khong co khai niem nay. */
function isFirefoxEngine(target: EditTarget): boolean {
  return target.source === "credential" && target.row.meta.engine === "firefox_portable";
}

export default function EditAccountDialog({
  target,
  onClose,
}: {
  target: EditTarget;
  onClose: () => void;
}) {
  const setProxyDola = useAccountsStore((s) => s.setProxy);
  const updateCredential = useCredentialsStore((s) => s.update);

  const initialProxy = target.source === "dola" ? target.row.proxy : target.row.proxyMasked;
  const [mode, setMode] = useState(() => proxyToMode(initialProxy));
  // Chuoi proxy o che do "fixed" — khoi tao bang gia tri da luu neu no la proxy co dinh.
  const [proxyInput, setProxyInput] = useState(() =>
    proxyToMode(initialProxy) === "fixed" ? initialProxy : "",
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);

  const name = target.source === "dola" ? target.row.name : target.row.label;
  const platformLabel =
    target.source === "dola" ? "Dola" : credentialProviderLabel(target.row.provider);
  const firefox = isFirefoxEngine(target);

  const targetProxy = modeToProxy(mode, proxyInput);
  // Save bat khi: gia tri se luu khac gia tri ban dau. O che do "fixed" ma input con dang che `••••`
  // (chua sua) thi targetProxy === initialProxy -> khong "dirty" -> khong gui chuoi che di (dung).
  const dirty = targetProxy !== initialProxy;
  // Chan save khi proxy co dinh RO RANG sai dinh dang (bo qua chuoi con che `••••` — khong tu validate
  // duoc, nhung luc do khong "dirty" nen cung khong save).
  const proxyInvalid =
    mode === "fixed" && !proxyInput.includes("•") && validateProxy(proxyInput) !== null;
  const canSave = !busy && dirty && !proxyInvalid;

  async function saveProxy(): Promise<void> {
    setBusy(true);
    setError(null);
    setOkMsg(null);
    try {
      if (target.source === "dola") {
        await setProxyDola(target.row.name, targetProxy);
      } else {
        await updateCredential(target.row.id, { proxy: targetProxy });
      }
      setOkMsg("Đã lưu proxy.");
    } catch (e) {
      setError(target.source === "dola" ? describeAccountError(e) : describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title={`Proxy & trình duyệt — ${name}`} onClose={onClose} width="30rem">
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between text-xs text-muted">
          <span>Nền tảng</span>
          <span className="font-medium text-fg">{platformLabel}</span>
        </div>

        {firefox ? (
          <div className="flex flex-col gap-1 rounded-md border border-accent/30 bg-accent/5 px-3 py-2 text-xs">
            <span className="flex items-center gap-2 font-semibold text-accent">
              <span className="rounded-sm bg-accent/15 px-1.5 py-0.5 text-[11px]">
                🦊 Firefox Portable
              </span>
            </span>
            <span className="leading-[1.5] text-ink3">
              Tài khoản này chỉ dùng tay: mở trình duyệt ra đã đăng nhập sẵn. KHÔNG verify/chạy
              workflow tự động (Firefox không có driver tự động hoá). Proxy sẽ ghi vào profile; proxy
              có mật khẩu thì Firefox hỏi lúc chạy.
            </span>
          </div>
        ) : null}

        <ConnectionModeFields
          mode={mode}
          onModeChange={setMode}
          proxyValue={proxyInput}
          onProxyChange={setProxyInput}
          disabled={busy}
        />

        <div className="flex items-start gap-2 rounded-md border border-line px-3 py-2 text-[12px] leading-[1.5] text-ink3">
          <span aria-hidden>ⓘ</span>
          <span>
            Đổi proxy có thể đăng xuất phiên ở một số nền tảng. Hãy kiểm tra lại đăng nhập sau khi
            lưu.
          </span>
        </div>

        {error ? <p className="text-xs text-rose">{error}</p> : null}
        {okMsg ? <p className="text-xs text-ok">{okMsg}</p> : null}

        <div className="flex justify-end gap-2 border-t border-line pt-3">
          <button type="button" className={BTN_NEUTRAL_SM} onClick={onClose}>
            Đóng
          </button>
          <button
            type="button"
            disabled={!canSave}
            className={BTN_PRIMARY_SM}
            onClick={() => void saveProxy()}
          >
            {busy ? "Đang lưu…" : "Lưu proxy"}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
