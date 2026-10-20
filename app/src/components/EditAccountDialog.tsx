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

/** Tag loai node da luu trong `meta.node_types` (mang chuoi). */
function readNodeTypes(meta: Record<string, unknown>): string[] {
  const nt = meta.node_types;
  return Array.isArray(nt) ? nt.filter((x): x is string => typeof x === "string") : [];
}

/** Lua chon tag loai node (khop `Vault::set_node_types`: video/image/script). */
const NODE_OPTIONS: ReadonlyArray<{ value: string; label: string }> = [
  { value: "video", label: "Video" },
  { value: "image", label: "Hình ảnh" },
  { value: "script", label: "Tạo kịch bản" },
];

export default function EditAccountDialog({
  target,
  onClose,
}: {
  target: EditTarget;
  onClose: () => void;
}) {
  const setProxyDola = useAccountsStore((s) => s.setProxy);
  const updateCredential = useCredentialsStore((s) => s.update);
  const setNodeTypesStore = useCredentialsStore((s) => s.setNodeTypes);
  const reloginStore = useCredentialsStore((s) => s.relogin);

  // Chi credential (khong phai Dola) moi co id/node_types/dang-nhap-lai.
  const credId = target.source === "credential" ? target.row.id : null;
  const credKind = target.source === "credential" ? target.row.kind : "";
  // "Dang nhap lai" chi cho tai khoan dang COOKIE (session/login), khong cho api_key.
  const canRelogin = credId !== null && (credKind === "session" || credKind === "login");
  const [nodeTypes, setNodeTypes] = useState<string[]>(() =>
    target.source === "credential" ? readNodeTypes(target.row.meta) : [],
  );
  const [cookieText, setCookieText] = useState("");

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

  function toggleNode(value: string): void {
    setNodeTypes((cur) =>
      cur.includes(value) ? cur.filter((x) => x !== value) : [...cur, value],
    );
  }

  async function saveNodeTypes(): Promise<void> {
    if (credId === null) return;
    setBusy(true);
    setError(null);
    setOkMsg(null);
    try {
      await setNodeTypesStore(credId, nodeTypes);
      setOkMsg("Đã lưu node sử dụng.");
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  async function doRelogin(): Promise<void> {
    if (credId === null || cookieText.trim() === "") return;
    setBusy(true);
    setError(null);
    setOkMsg(null);
    try {
      await reloginStore(credId, cookieText);
      setCookieText("");
      setOkMsg("Đã đăng nhập lại (tiêm cookie mới vào profile). Bấm Kiểm tra để xác nhận.");
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      title={`${target.source === "credential" ? "Cài đặt tài khoản" : "Proxy & trình duyệt"} — ${name}`}
      onClose={onClose}
      width="30rem"
    >
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

        {credId !== null ? (
          <div className="flex flex-col gap-2 border-t border-line pt-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-fg">Dùng cho node</span>
              <button
                type="button"
                disabled={busy}
                className={BTN_NEUTRAL_SM}
                onClick={() => void saveNodeTypes()}
              >
                Lưu node
              </button>
            </div>
            <div className="flex flex-wrap gap-2">
              {NODE_OPTIONS.map((opt) => {
                const on = nodeTypes.includes(opt.value);
                return (
                  <button
                    key={opt.value}
                    type="button"
                    disabled={busy}
                    onClick={() => toggleNode(opt.value)}
                    className={`rounded-full border px-3 py-1 text-xs ${
                      on
                        ? "border-accent bg-accent/15 text-accent"
                        : "border-line text-ink3 hover:border-accent/40"
                    }`}
                  >
                    {on ? "✓ " : ""}
                    {opt.label}
                  </button>
                );
              })}
            </div>
            <span className="text-[11px] leading-[1.4] text-muted">
              Node AI chỉ lấy tài khoản có đúng loại node tương ứng.
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

        {canRelogin ? (
          <div className="flex flex-col gap-2 border-t border-line pt-3">
            <span className="text-xs font-medium text-fg">Đăng nhập lại</span>
            <span className="text-[11px] leading-[1.4] text-muted">
              Dán cookie MỚI (đã đăng nhập) để tiêm lại vào đúng tài khoản này khi phiên hết hạn.
            </span>
            <textarea
              value={cookieText}
              onChange={(e) => setCookieText(e.target.value)}
              disabled={busy}
              rows={3}
              placeholder="name=value; name2=value2; …  (hoặc cookie Netscape)"
              className="w-full resize-y rounded-md border border-line bg-bg2 px-2 py-1.5 font-mono text-[11px] text-fg placeholder:text-muted"
            />
            <div className="flex justify-end">
              <button
                type="button"
                disabled={busy || cookieText.trim() === ""}
                className={BTN_NEUTRAL_SM}
                onClick={() => void doRelogin()}
              >
                {busy ? "Đang tiêm…" : "Đăng nhập lại"}
              </button>
            </div>
          </div>
        ) : null}

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
