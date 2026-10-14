// Bảng "Chụp khung hình" kiểu Lumina — mở từ nút trên node video (Tạo video Dola / Tải video lên).
// Phát/tua video nguồn, bắt Khung đầu / Khung cuối / Khung hiện tại; các khung gom lại thành thumbnail;
// "Thêm vào canvas" lưu từng khung thành file (frame_save_png) rồi tạo node "Tải ảnh lên" để dùng tiếp
// (vd tạo video từ ảnh). Vẽ khung từ thẻ <video> qua <canvas> — không cần ffmpeg.
import { useRef, useState } from "react";

import { useWorkflowUi } from "../store/workflowUi";
import { useGraphStore } from "../store/graph";
import { frameSavePng } from "../ipc/commands";
import { errorText } from "../ipc/error_text";

interface Frame {
  dataUrl: string;
  label: string; // vd "00:15.08s"
}

function fmt(t: number): string {
  const safe = Number.isFinite(t) && t > 0 ? t : 0;
  const m = Math.floor(safe / 60);
  const s = safe - m * 60;
  return `${String(m).padStart(2, "0")}:${s.toFixed(2).padStart(5, "0")}s`;
}

/** Vẽ khung hiện tại của <video> ra PNG dataURL. Ném nếu chưa có kích thước/canvas bị nhiễm. */
function grab(video: HTMLVideoElement): string {
  const w = video.videoWidth;
  const h = video.videoHeight;
  if (w === 0 || h === 0) {
    throw new Error("Video chưa sẵn sàng (chưa có khung hình).");
  }
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Không tạo được canvas.");
  }
  ctx.drawImage(video, 0, 0, w, h);
  return canvas.toDataURL("image/png");
}

function seekTo(video: HTMLVideoElement, time: number): Promise<void> {
  return new Promise((resolve) => {
    const done = (): void => {
      video.removeEventListener("seeked", done);
      resolve();
    };
    video.addEventListener("seeked", done);
    video.currentTime = Math.max(0, Math.min(time, video.duration || 0));
  });
}

export default function CaptureFramesPanel() {
  const target = useWorkflowUi((s) => s.captureTarget);
  const close = useWorkflowUi((s) => s.closeCapture);
  const addImageNodes = useGraphStore((s) => s.addImageNodes);

  const videoRef = useRef<HTMLVideoElement>(null);
  const [frames, setFrames] = useState<Frame[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (!target) {
    return null;
  }

  function capture(kind: "first" | "last" | "current"): void {
    const video = videoRef.current;
    if (!video) {
      return;
    }
    setError(null);
    const run = async (): Promise<void> => {
      try {
        if (kind === "first") {
          await seekTo(video, 0);
        } else if (kind === "last") {
          await seekTo(video, Math.max(0, (video.duration || 0) - 0.05));
        }
        const dataUrl = grab(video);
        setFrames((f) => [...f, { dataUrl, label: fmt(video.currentTime) }]);
      } catch (e) {
        setError(errorText(e));
      }
    };
    void run();
  }

  async function addToCanvas(): Promise<void> {
    if (frames.length === 0 || !target) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const items: { path: string; label: string }[] = [];
      for (const fr of frames) {
        const path = await frameSavePng(fr.dataUrl, `${target.label} ${fr.label}`);
        items.push({ path, label: `${target.label} · ${fr.label}` });
      }
      addImageNodes(items, { x: target.x, y: target.y });
      close();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" onClick={close}>
      <div
        className="flex max-h-[90vh] w-[720px] max-w-[calc(100vw-2rem)] flex-col gap-3 overflow-hidden rounded-2xl border border-line bg-panel p-3 text-ink shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold">Chụp khung hình · {target.label}</h3>
          <button type="button" className="rounded-md px-2 py-1 text-ink2 hover:bg-fill2 hover:text-ink" onClick={close} aria-label="Đóng">
            ✕
          </button>
        </div>

        {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
        <video
          ref={videoRef}
          src={target.src}
          crossOrigin="anonymous"
          controls
          playsInline
          preload="auto"
          className="max-h-[46vh] w-full rounded-lg bg-black object-contain"
        />

        <div className="flex flex-wrap items-center gap-2">
          <button type="button" className="rounded-md border border-line2 bg-fill px-2.5 py-1 text-xs hover:bg-fill2" onClick={() => capture("first")}>
            Khung đầu
          </button>
          <button type="button" className="rounded-md border border-line2 bg-fill px-2.5 py-1 text-xs hover:bg-fill2" onClick={() => capture("last")}>
            Khung cuối
          </button>
          <button type="button" className="rounded-md border border-sky-500/40 bg-sky-500/10 px-2.5 py-1 text-xs text-sky-200 hover:bg-sky-500/20" onClick={() => capture("current")}>
            Khung hiện tại
          </button>
          <button type="button" disabled={frames.length === 0} className="rounded-md border border-line2 bg-fill px-2.5 py-1 text-xs hover:bg-fill2 disabled:opacity-40" onClick={() => setFrames([])}>
            Xoá hết
          </button>
          <button
            type="button"
            disabled={frames.length === 0 || busy}
            className="ml-auto rounded-md bg-sky-600 px-3 py-1 text-xs font-medium text-ink hover:bg-sky-500 disabled:opacity-40"
            onClick={() => void addToCanvas()}
          >
            {busy ? "Đang thêm…" : `Thêm vào canvas (${frames.length})`}
          </button>
        </div>

        {error ? <p className="text-xs text-rose">{error}</p> : null}

        <div className="flex min-h-[92px] flex-wrap gap-2 overflow-y-auto rounded-lg border border-line bg-black/20 p-2">
          {frames.length === 0 ? (
            <p className="m-auto text-xs text-ink3">Chưa có khung nào — tua video rồi bấm "Khung hiện tại".</p>
          ) : (
            frames.map((fr, i) => (
              <div key={`${fr.label}-${i}`} className="relative">
                <img src={fr.dataUrl} alt="" className="h-20 w-auto rounded border border-line object-cover" />
                <span className="absolute bottom-0 left-0 rounded-tr bg-black/70 px-1 text-[9px] text-ink">{fr.label}</span>
                <button
                  type="button"
                  className="absolute right-0 top-0 flex h-4 w-4 items-center justify-center rounded-bl bg-black/70 text-[10px] text-ink hover:bg-red-600 hover:text-ink"
                  onClick={() => setFrames((f) => f.filter((_, j) => j !== i))}
                  aria-label="Bỏ khung"
                >
                  ×
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
