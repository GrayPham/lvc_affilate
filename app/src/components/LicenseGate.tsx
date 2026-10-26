// Cổng bản quyền: màn khoá 2 bước (đăng nhập tài khoản → kích hoạt license) bọc toàn app. Chưa
// kích hoạt hợp lệ thì KHÔNG render app chính.
//
// Hình dáng bám bản thiết kế chuẩn công ty `DlgLicense.dc.html` (canvas "LVC Fox — Thiết kế UI P3"):
// hộp thoại có DẢI ĐẦU (logo + tiêu đề + nút ×) và DẢI CHÂN nền `panel-2` với nút dồn phải, ô nhập
// có NHÃN ở trên (không chỉ placeholder), và khối minh bạch "máy chủ nhận được gì".
//
// ĐỒNG BỘ VỚI MÀN CHÍNH: màn khoá nằm trong đúng bộ khung của `App.tsx` — nền gradient `lvc-frame`,
// panel bo 20px nền `bg`, thanh trên cao 68px có vùng kéo + bộ nút cửa sổ dùng chung
// (`ui/WindowButtons`). Không được tự vẽ nút đóng riêng: hai màn phải trông như MỘT app.
//
// Năm điểm dễ làm hỏng khi sửa tiếp:
//  - Cửa sổ đặt `decorations: false` (tauri.conf.json) nên Windows không vẽ nút nào. Bỏ
//    `<WindowButtons />` là người dùng hết đường đóng app. Thanh trên phải giữ
//    `data-tauri-drag-region` để kéo được cửa sổ.
//  - "Ghi nhớ tài khoản" KHÔNG tích → backend không ghi session, lần mở sau phải đăng nhập lại.
//  - Ngoài Tauri (xem thử bằng trình duyệt) KHÔNG có cầu IPC: báo đúng `gate.err.noBridge` thay vì
//    thông báo lỗi chung chung.
//  - Chữ đẳng khoảng (`font-mono`) CHỈ dùng cho mã máy thật. Câu tiếng Việt đặt vào font mono sẽ
//    giãn chữ loang lổ, trông như lỗi hiển thị.
//  - Bí mật (mật khẩu/mã license) chỉ gửi xuống backend, không lưu ở store/localStorage/log.
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { AlertCircle, Eye, EyeOff, Loader2, LogOut, Moon, Sun } from "lucide-react";

import { LEGAL_LINKS, REGISTER_URL, SUPPORT_CONTACTS } from "../config/contact";
import { LanguageSwitcher, useT } from "../i18n";
import { maskDeviceId } from "../format/device";
import {
  deviceId,
  licenseActivate,
  licenseLogin,
  licenseLogout,
  licenseStatus,
} from "../ipc/commands";
import { useUiStore } from "../store/uiStore";
import { BTN_ICON_ROUND } from "../ui/buttonStyles";
import { WindowButtons } from "../ui/WindowButtons";
import { licenseErrorKey } from "./licenseGateError";

type Phase = "loading" | "login" | "activate" | "activated";

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Chu kỳ hỏi lại backend xem quyền còn hiệu lực không (5 phút). */
const RECHECK_MS = 5 * 60 * 1000;

/** Trang bán giấy phép LVC Shop (người dùng cung cấp 2026-10-21). */
const BUY_URL = "https://marketmmoai.com/tools/lvc-shop";

/** Quầng sáng thương hiệu sau hộp thoại — nền phẳng tuyệt đối làm màn khoá trông thô. */
const GLOW =
  "radial-gradient(58% 44% at 50% 16%, rgb(246 185 15 / 0.14) 0%, rgb(246 185 15 / 0.04) 45%, transparent 72%)";

// Hình dáng lấy từ `DlgLicense.dc.html`: ô nhập cao 40px bo 10px, nút cao 38px bo 11px. Viền giữ
// 1px và dùng `ring` khi focus — đổi độ dày viền sẽ làm nội dung nhảy 1px.
const INPUT =
  "h-10 w-full rounded-[10px] border border-line bg-panel-2 px-3 text-sm text-fg outline-none transition-[border-color,box-shadow] placeholder:text-muted focus:border-primary focus:bg-panel focus:ring-1 focus:ring-primary";
const BTN = "inline-flex h-[38px] items-center justify-center gap-1.5 rounded-[11px] text-[13px]";
const BTN_SECONDARY = `${BTN} border border-line bg-panel px-4 text-fg transition-colors hover:bg-hover disabled:opacity-50`;
const BTN_MAIN = `${BTN} bg-primary px-[18px] font-bold text-primary-fg transition-colors hover:bg-primary-hover disabled:cursor-not-allowed disabled:opacity-40`;

/** Ô nhập có nhãn ở trên — đúng kiểu `DlgLicense` (nhãn 12px màu muted). */
function Labeled({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5 text-xs font-medium text-muted">
      {label}
      {children}
    </label>
  );
}

/** Chỉ báo bước 1 → 2. */
function Steps({ phase }: { phase: "login" | "activate" }) {
  const t = useT();
  const items = [
    { n: 1, label: t("gate.step.login"), on: true },
    { n: 2, label: t("gate.step.activate"), on: phase === "activate" },
  ];
  return (
    <div className="flex items-center gap-2">
      {items.map((it, i) => (
        <div key={it.n} className="flex items-center gap-2">
          {i > 0 ? <span className="h-px w-6 bg-line" /> : null}
          <span
            className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors ${
              it.on ? "bg-sel text-accent" : "text-muted"
            }`}
          >
            <span
              className={`inline-flex h-[18px] w-[18px] items-center justify-center rounded-full text-[10px] font-bold ${
                it.on ? "bg-primary text-primary-fg" : "bg-line text-muted"
              }`}
            >
              {it.n}
            </span>
            {it.label}
          </span>
        </div>
      ))}
    </div>
  );
}

/** Hàng liên hệ hỗ trợ ở dải chân. Giá trị lấy từ `config/contact.ts`. */
function Support({ onOpen }: { onOpen: (url: string) => void }) {
  const t = useT();
  return (
    <div className="mr-auto flex flex-col gap-0.5">
      <span className="text-[11px] text-muted">{t("gate.contact")}</span>
      <div className="flex flex-wrap items-center gap-1.5 text-[11.5px]">
        {SUPPORT_CONTACTS.map((c) =>
          c.url === null ? (
            <span key={c.label} className="text-muted">
              <span className="text-fg/70">{c.label}</span> {c.value}
            </span>
          ) : (
            <button
              key={c.label}
              type="button"
              title={c.value}
              className="rounded-full border border-line bg-panel px-2.5 py-0.5 font-medium text-accent transition-colors hover:border-primary hover:bg-hover"
              onClick={() => onOpen(c.url as string)}
            >
              {c.label}
            </button>
          ),
        )}
      </div>
    </div>
  );
}

export default function LicenseGate({ children }: { children: React.ReactNode }) {
  const t = useT();
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  const [phase, setPhase] = useState<Phase>("loading");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [remember, setRemember] = useState(true);
  const [licenseKey, setLicenseKey] = useState("");
  const [device, setDevice] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<ReturnType<typeof licenseErrorKey> | null>(null);

  useEffect(() => {
    licenseStatus()
      .then((s) => setPhase(s.activated ? "activated" : "login"))
      .catch(() => setPhase("login"));
    deviceId()
      .then(setDevice)
      .catch(() => setDevice(null));
    // Ngoài Tauri không có API app → không hiện số phiên bản, không phải lỗi.
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  // Đã vào app vẫn hỏi lại backend định kỳ: license hết hạn giữa chừng (hoặc backend thu hồi quyền)
  // thì quay về màn khoá ngay, không đợi mở lại app. Nguồn sự thật là backend; giao diện chỉ hiển thị.
  useEffect(() => {
    if (phase !== "activated") {
      return undefined;
    }
    const id = window.setInterval(() => {
      licenseStatus()
        .then((s) => {
          if (!s.activated) {
            setPhase("login");
          }
        })
        .catch(() => {
          // Lỗi tạm thời (IPC) không đủ lý do đá người dùng ra; lần kiểm tra sau sẽ thử lại.
        });
    }, RECHECK_MS);
    return () => window.clearInterval(id);
  }, [phase]);

  /** Ngoài Tauri thì mọi `invoke` đều ném — nói thẳng là bản xem thử, đừng đổ lỗi mơ hồ. */
  function fail(e: unknown): void {
    setErrorKey(IN_TAURI ? licenseErrorKey(e) : "gate.err.noBridge");
  }

  async function doLogin(): Promise<void> {
    setBusy(true);
    setErrorKey(null);
    try {
      await licenseLogin(username.trim(), password);
      setPhase("activate");
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  async function doActivate(): Promise<void> {
    setBusy(true);
    setErrorKey(null);
    try {
      const s = await licenseActivate(licenseKey.trim(), remember);
      setPhase(s.activated ? "activated" : "activate");
      if (!s.activated) {
        setErrorKey("gate.err.notActivated");
      }
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  /** Đăng xuất: xoá session + token, xoá mật khẩu/mã khỏi bộ nhớ màn hình, quay về bước 1. */
  async function doLogout(): Promise<void> {
    setBusy(true);
    try {
      await licenseLogout();
    } catch {
      // Đăng xuất là best-effort; dù lỗi vẫn đưa người dùng về màn đăng nhập.
    } finally {
      setPassword("");
      setLicenseKey("");
      setErrorKey(null);
      setPhase("login");
      setBusy(false);
    }
  }

  function openUrl(url: string): void {
    if (!IN_TAURI) {
      return;
    }
    openExternal(url).catch(() => {});
  }

  if (phase === "activated") {
    return <>{children}</>;
  }

  const onLogin = phase === "login";
  const submit = onLogin ? doLogin : doActivate;
  const canSubmit = onLogin ? username.trim() !== "" && password !== "" : licenseKey.trim() !== "";

  return (
    <div className="lvc-frame flex h-screen w-screen p-2.5 text-fg">
      <div className="relative flex min-w-0 flex-1 flex-col overflow-hidden rounded-[20px] bg-bg shadow-panel">
        {/* Thanh trên y hệt màn chính (`App.tsx` + `ui/TopBar`): vùng kéo, thương hiệu, nút cửa sổ. */}
        <header
          data-tauri-drag-region
          className="flex h-[68px] shrink-0 items-center gap-2.5 pl-5 pr-3"
        >
          <img src="/icon.png" alt="" className="h-8 w-8 shrink-0 rounded-md object-cover" />
          <div className="flex min-w-0 flex-col leading-[1.25]">
            <span className="truncate text-[13px] font-bold">{t("app.name")}</span>
            <span className="truncate text-[10.5px] text-muted">{t("app.tagline")}</span>
          </div>
          <div data-tauri-drag-region className="h-full min-w-4 flex-1" />
          <button
            type="button"
            title={t("theme.toggle")}
            aria-label={t("theme.toggle")}
            className={BTN_ICON_ROUND}
            onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
          >
            {theme === "dark" ? <Sun size={17} /> : <Moon size={17} />}
          </button>
          <LanguageSwitcher />
          <WindowButtons />
        </header>

        <div className="relative grid flex-1 place-items-center px-4 pb-8">
          <div
            aria-hidden
            className="pointer-events-none absolute inset-0"
            style={{ background: GLOW }}
          />

          <div className="relative flex w-full max-w-[560px] flex-col gap-4">
          <form
            className="flex w-full flex-col overflow-hidden rounded-[18px] border border-line bg-panel shadow-panel"
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
          >
            <div className="flex items-center gap-3 border-b border-line px-6 py-4">
              <h2 className="flex-1 text-[17px] font-bold tracking-tight">{t("gate.title")}</h2>
            </div>

            <div className="flex flex-col gap-4 px-6 py-5">
          {phase === "loading" ? (
            <p className="flex items-center justify-center gap-2 py-10 text-sm text-muted">
              <Loader2 size={15} className="animate-spin" />
              {t("gate.checking")}
            </p>
          ) : (
            <>
              <div className="flex flex-col gap-1.5">
                <Steps phase={phase} />
                <p className="text-xs text-muted">
                  {onLogin ? t("gate.login.subtitle") : t("gate.activate.subtitle")}
                </p>
              </div>

              {onLogin ? (
                <div className="flex flex-col gap-3.5">
                  <Labeled label={t("gate.login.username")}>
                    <input
                      className={INPUT}
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      disabled={busy}
                      autoFocus
                    />
                  </Labeled>

                  <Labeled label={t("gate.login.password")}>
                    <div className="relative">
                      <input
                        className={`${INPUT} pr-11`}
                        type={showPassword ? "text" : "password"}
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        disabled={busy}
                      />
                      <button
                        type="button"
                        title={showPassword ? t("gate.password.hide") : t("gate.password.show")}
                        aria-label={showPassword ? t("gate.password.hide") : t("gate.password.show")}
                        className="absolute right-1.5 top-1/2 inline-flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-lg text-muted transition-colors hover:bg-hover hover:text-fg"
                        onClick={() => setShowPassword((v) => !v)}
                        tabIndex={-1}
                      >
                        {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                      </button>
                    </div>
                  </Labeled>

                  <label className="flex cursor-pointer items-start gap-2.5 text-xs text-fg">
                    <input
                      type="checkbox"
                      className="mt-px h-[15px] w-[15px] shrink-0 accent-[var(--primary)]"
                      checked={remember}
                      onChange={(e) => setRemember(e.target.checked)}
                      disabled={busy}
                    />
                    <span>
                      <span className="font-medium">{t("gate.login.remember")}</span>
                      <span className="mt-0.5 block text-[11px] leading-snug text-muted">
                        {t("gate.login.rememberHint")}
                      </span>
                    </span>
                  </label>
                </div>
              ) : (
                <div className="flex flex-col gap-3.5">
                  {username.trim() === "" ? null : (
                    <p className="text-xs text-muted">
                      {t("gate.activate.hello", { ten: username.trim() })}
                    </p>
                  )}
                  <Labeled label={t("gate.activate.key")}>
                    <input
                      className={`${INPUT} font-mono tracking-[0.08em]`}
                      placeholder={t("gate.activate.placeholder")}
                      value={licenseKey}
                      onChange={(e) => setLicenseKey(e.target.value)}
                      disabled={busy}
                      autoFocus
                    />
                  </Labeled>
                </div>
              )}

              {/* Mã máy: người dùng đọc cho nhóm hỗ trợ khi cần cấp/đổi máy. */}
              <div className="grid grid-cols-[112px_1fr] gap-x-3 rounded-xl border border-line bg-panel-2 px-3.5 py-3 text-[12.5px] leading-snug">
                <span className="text-muted">{t("gate.device.label")}</span>
                <span className={device === null ? "text-muted" : "font-mono"} title={device ?? undefined}>
                  {device === null ? t("gate.device.unknown") : maskDeviceId(device)}
                </span>
              </div>

              {errorKey ? (
                <p className="flex items-start gap-2 rounded-xl border border-danger/30 bg-danger/5 px-3.5 py-2.5 text-xs leading-snug text-danger">
                  <AlertCircle size={14} className="mt-px shrink-0" />
                  {t(errorKey)}
                </p>
              ) : null}
            </>
          )}
        </div>

        {/* Dải chân — liên hệ bên trái, hành động dồn phải, đúng bản thiết kế. */}
        {phase === "loading" ? null : (
          <div className="flex flex-wrap items-center justify-end gap-2 border-t border-line bg-panel-2 px-6 py-3.5">
            <Support onOpen={openUrl} />
            {onLogin ? null : (
              <button
                type="button"
                className="inline-flex items-center gap-1.5 px-2 text-xs text-muted transition-colors hover:text-fg"
                onClick={() => void doLogout()}
                disabled={busy}
              >
                <LogOut size={13} />
                {t("gate.logout")}
              </button>
            )}
            <button
              type="button"
              className={BTN_SECONDARY}
              onClick={() => openUrl(BUY_URL)}
              disabled={busy}
            >
              {t("gate.buy")}
            </button>
            <button type="submit" className={BTN_MAIN} disabled={busy || !canSubmit}>
              {busy ? <Loader2 size={14} className="animate-spin" /> : null}
              {busy
                ? onLogin
                  ? t("gate.login.submitting")
                  : t("gate.activate.submitting")
                : onLogin
                  ? t("gate.login.submit")
                  : t("gate.activate.submit")}
                </button>
              </div>
            )}
          </form>

          {onLogin ? (
            <p className="text-center text-xs text-muted">
              {t("gate.register.prompt")}{" "}
              <button
                type="button"
                className="font-bold text-accent underline decoration-primary underline-offset-2 transition-colors hover:text-primary"
                onClick={() => openUrl(REGISTER_URL)}
              >
                {t("gate.register.link")}
              </button>
            </p>
          ) : null}
          </div>

          {/* Chân trang: điều khoản · bảo mật · hỗ trợ + số phiên bản của app. */}
          <footer className="absolute inset-x-0 bottom-3 flex flex-wrap items-center justify-center gap-x-3 gap-y-1 px-4 text-[11px] text-muted">
            {LEGAL_LINKS.map((l) => (
              <button
                key={l.labelKey}
                type="button"
                className="underline underline-offset-2 transition-colors hover:text-fg"
                onClick={() => openUrl(l.url)}
              >
                {t(l.labelKey)}
              </button>
            ))}
            {version === null ? null : (
              <span className="tabular-nums">{t("legal.version", { v: version })}</span>
            )}
          </footer>
        </div>
      </div>
    </div>
  );
}
