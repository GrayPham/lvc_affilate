// Khoi truong "Kiểu kết nối" (proxy) dung CHUNG cho `EditAccountDialog` (mot tai khoan) va
// `AssignProxyDialog` (gan hang loat) — dung theo `references/lvc-ui-design/screens/AssignProxy.dc.html`.
//
// 3 che do THUC (xem `format/proxyMode.ts` giai thich quyet dinh "Xoay vòng" -> "Proxy chung"):
//   - IP máy      : khong proxy (luu "direct").
//   - Proxy cố định: nhap 1 proxy (luu nguyen chuoi).
//   - Proxy chung : dung global_proxy trong Cai dat (luu "").
//
// Controlled: cha giu `mode` + `proxyValue`, component nay lo hien thi + nut "Kiểm tra proxy".
import { useState } from "react";

import { BTN_NEUTRAL_SM } from "../ui/buttonStyles";
import { validateProxy } from "../format/account";
import { CONN_MODES, type ConnMode } from "../format/proxyMode";
import { proxyTest } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { ProxyProbe } from "../ipc/types";

export default function ConnectionModeFields({
  mode,
  onModeChange,
  proxyValue,
  onProxyChange,
  disabled = false,
}: {
  mode: ConnMode;
  onModeChange: (m: ConnMode) => void;
  proxyValue: string;
  onProxyChange: (v: string) => void;
  disabled?: boolean;
}) {
  const [testing, setTesting] = useState(false);
  const [probe, setProbe] = useState<ProxyProbe | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);

  function clearProbe(): void {
    setProbe(null);
    setProbeError(null);
  }

  function pickMode(m: ConnMode): void {
    clearProbe();
    onModeChange(m);
  }

  const proxyError = mode === "fixed" && proxyValue !== "" ? validateProxy(proxyValue) : null;
  // Proxy da luu dang che `••••` — khong gui chuoi che di kiem (se sai mat khau). Phai nhap lai.
  const proxyMasked = proxyValue.includes("•");
  // Chuoi dem di kiem: fixed -> chuoi nhap; machine -> "" (xem IP may). shared khong kiem (khong biet
  // proxy chung o client — da che phia server).
  const testTarget = mode === "fixed" ? proxyValue : "";
  const canTest =
    !testing && !disabled && mode !== "shared" && proxyError === null && !proxyMasked;

  async function runTest(): Promise<void> {
    setTesting(true);
    clearProbe();
    try {
      setProbe(await proxyTest(testTarget));
    } catch (e) {
      setProbeError(errorText(e));
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <fieldset className="m-0 border-0 p-0">
        <legend className="mb-2 p-0 text-xs font-semibold text-muted">Kiểu kết nối</legend>
        <div className="grid grid-cols-3 gap-2">
          {CONN_MODES.map((m) => {
            const on = m.mode === mode;
            return (
              <button
                key={m.mode}
                type="button"
                aria-pressed={on}
                disabled={disabled}
                onClick={() => pickMode(m.mode)}
                className="flex min-h-[60px] flex-col items-start gap-0.5 rounded-lg border px-3 py-2.5 text-left disabled:cursor-not-allowed disabled:opacity-50"
                style={{
                  borderColor: on ? "rgb(245 165 36 / .65)" : "var(--line2)",
                  background: on ? "rgb(245 165 36 / .12)" : "transparent",
                }}
              >
                <span className="text-[13px] font-semibold text-fg">{m.label}</span>
                <span className="text-[11.5px] leading-[1.4] text-ink3">{m.sub}</span>
              </button>
            );
          })}
        </div>
      </fieldset>

      {mode === "fixed" ? (
        <label className="flex flex-col gap-1 text-xs text-muted">
          Proxy <span className="text-ink3">host:port hoặc host:port:user:pass</span>
          <input
            className="rounded-md border border-line bg-fill px-2.5 py-1.5 font-mono text-sm text-fg placeholder:text-ink3"
            value={proxyValue}
            disabled={disabled}
            onChange={(e) => {
              clearProbe();
              onProxyChange(e.target.value);
            }}
            placeholder="vd 113.161.4.22:8080:user01:••••••"
          />
          {proxyError ? <span className="text-rose">{proxyError}</span> : null}
        </label>
      ) : mode === "shared" ? (
        <p className="rounded-md border border-line px-3 py-2 text-[12px] leading-[1.5] text-ink3">
          Dùng proxy chung cấu hình trong Cài đặt cho tài khoản này. Xoay vòng theo danh sách proxy
          riêng cho từng tài khoản — sắp có.
        </p>
      ) : (
        <p className="rounded-md border border-line px-3 py-2 text-[12px] leading-[1.5] text-ink3">
          Không dùng proxy — đi thẳng bằng IP máy.
        </p>
      )}

      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <button
            type="button"
            disabled={!canTest}
            className={BTN_NEUTRAL_SM}
            onClick={() => void runTest()}
            title={
              mode === "shared"
                ? "Proxy chung được ẩn ở client — không kiểm trực tiếp tại đây"
                : proxyMasked
                  ? "Nhập lại proxy để kiểm tra — mật khẩu đã lưu đang được ẩn"
                  : "Nối thử để xem IP ra, vị trí, độ trễ"
            }
          >
            {testing ? "Đang kiểm tra…" : "Kiểm tra proxy"}
          </button>
          {mode === "machine" ? (
            <span className="text-[11px] text-ink3">Kiểm tra IP máy hiện tại (không proxy)</span>
          ) : proxyMasked ? (
            <span className="text-[11px] text-ink3">Nhập lại proxy để kiểm tra (mật khẩu đã ẩn)</span>
          ) : null}
        </div>

        {probe ? (
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-md border border-ok/30 bg-ok/10 px-3 py-2 text-xs">
            <span className="font-semibold text-ok">✓ Kết nối được</span>
            <span className="text-muted">
              IP ra: <span className="font-mono text-fg">{probe.outboundIp || "—"}</span>
            </span>
            <span className="text-muted">
              Vị trí: <span className="text-fg">{probe.location || "—"}</span>
            </span>
            <span className="text-muted">
              Độ trễ: <span className="text-fg">{probe.latencyMs} ms</span>
            </span>
          </div>
        ) : null}
        {probeError ? (
          <div className="rounded-md border border-rose/30 bg-rose/10 px-3 py-2 text-xs text-rose">
            ✕ {probeError}
          </div>
        ) : null}
      </div>
    </div>
  );
}
