// Banner auto-update (P8) — mount MOT LAN cho ca app (App.tsx, cung cap `DialogHost`). Tu check
// ngam luc app mo (useEffect mount); CHI hien UI khi co ban moi/dang tai/loi — khong lam phien nguoi
// dung khi da la ban moi nhat (ADR-R026, xem docs/superpowers/specs/2026-10-13-auto-update-design.md).
import { useEffect } from "react";
import { useUpdaterStore } from "../store/updater";

function formatBytes(n: number): string {
  if (n < 1024 * 1024) {
    return `${(n / 1024).toFixed(0)} KB`;
  }
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export default function UpdateBanner() {
  const phase = useUpdaterStore((s) => s.phase);
  const update = useUpdaterStore((s) => s.update);
  const notes = useUpdaterStore((s) => s.notes);
  const downloadedBytes = useUpdaterStore((s) => s.downloadedBytes);
  const totalBytes = useUpdaterStore((s) => s.totalBytes);
  const error = useUpdaterStore((s) => s.error);
  const dismissedVersion = useUpdaterStore((s) => s.dismissedVersion);
  const checkForUpdate = useUpdaterStore((s) => s.checkForUpdate);
  const install = useUpdaterStore((s) => s.install);
  const dismiss = useUpdaterStore((s) => s.dismiss);

  useEffect(() => {
    // `true` = im lang khi that bai — xem doc `checkForUpdate` trong store.
    void checkForUpdate(true);
    // Chi check 1 lan luc mount (store la singleton toan app — khong phu thuoc props/deps nao).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (phase === "available" && update && update.version === dismissedVersion) {
    return null;
  }
  if (phase !== "available" && phase !== "downloading" && phase !== "error") {
    return null;
  }

  return (
    <div className="fixed bottom-4 right-4 z-50 w-80 rounded-xl border border-line bg-panel p-3 shadow-2xl">
      {phase === "error" ? (
        <>
          <p className="text-sm font-semibold text-rose">Kiểm tra cập nhật thất bại</p>
          <p className="mt-1 text-xs text-ink2">{error}</p>
          <div className="mt-2 flex justify-end gap-2">
            <button
              type="button"
              className="rounded-md px-2 py-1 text-xs text-ink2 hover:bg-fill2 hover:text-ink"
              onClick={dismiss}
            >
              Đóng
            </button>
            <button
              type="button"
              className="rounded-md bg-amber-400/90 px-2 py-1 text-xs font-medium text-ink hover:bg-amber-400"
              onClick={() => void checkForUpdate()}
            >
              Thử lại
            </button>
          </div>
        </>
      ) : null}

      {phase === "available" && update ? (
        <>
          <p className="text-sm font-semibold text-ink">Có bản cập nhật v{update.version}</p>
          {notes ? (
            <p className="mt-1 max-h-24 overflow-auto whitespace-pre-wrap text-xs text-ink2">{notes}</p>
          ) : null}
          <div className="mt-2 flex justify-end gap-2">
            <button
              type="button"
              className="rounded-md px-2 py-1 text-xs text-ink2 hover:bg-fill2 hover:text-ink"
              onClick={dismiss}
            >
              Để sau
            </button>
            <button
              type="button"
              className="rounded-md bg-amber-400/90 px-2 py-1 text-xs font-medium text-ink hover:bg-amber-400"
              onClick={() => void install()}
            >
              Cài đặt
            </button>
          </div>
        </>
      ) : null}

      {phase === "downloading" ? (
        <>
          <p className="text-sm font-semibold text-ink">Đang tải bản cập nhật…</p>
          <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-fill2">
            <div
              className="h-full bg-amber-400 transition-all"
              style={{
                width: totalBytes ? `${Math.min(100, (downloadedBytes / totalBytes) * 100)}%` : "30%",
              }}
            />
          </div>
          <p className="mt-1 text-[11px] text-ink3">
            {formatBytes(downloadedBytes)}
            {totalBytes ? ` / ${formatBytes(totalBytes)}` : ""} — ứng dụng sẽ tự khởi động lại sau khi cài xong
          </p>
        </>
      ) : null}
    </div>
  );
}
