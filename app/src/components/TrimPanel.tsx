// Bảng "Cắt video" kiểu Lumina — mở từ nút trên node video. Xem video + THANH THỜI GIAN kéo hai tay
// cầm (bắt đầu/kết thúc) để chọn đoạn; "Cắt & thêm vào canvas" gọi ffmpeg (trim_video) tạo file mới
// rồi thêm node "Tải video lên". `path` (đường dẫn thật) để cắt, `src` để xem.
import { useCallback, useRef, useState } from "react";

import { useWorkflowUi } from "../store/workflowUi";
import { useGraphStore } from "../store/graph";
import { trimVideo } from "../ipc/commands";
import { errorText } from "../ipc/error_text";

function fmt(t: number): string {
  const safe = Number.isFinite(t) && t > 0 ? t : 0;
  const m = Math.floor(safe / 60);
  const s = safe - m * 60;
  return `${String(m).padStart(2, "0")}:${s.toFixed(2).padStart(5, "0")}`;
}

export default function TrimPanel() {
  const target = useWorkflowUi((s) => s.trimTarget);
  const close = useWorkflowUi((s) => s.closeTrim);
  const addVideoNode = useGraphStore((s) => s.addVideoNode);

  const videoRef = useRef<HTMLVideoElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const [duration, setDuration] = useState(0);
  const [start, setStart] = useState(0);
  const [end, setEnd] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onLoaded = useCallback(() => {
    const d = videoRef.current?.duration ?? 0;
    const dur = Number.isFinite(d) && d > 0 ? d : 0;
    setDuration(dur);
    setStart(0);
    setEnd(dur);
  }, []);

  const timeAtClientX = useCallback(
    (clientX: number): number => {
      const rect = trackRef.current?.getBoundingClientRect();
      if (!rect || rect.width === 0 || duration <= 0) {
        return 0;
      }
      const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      return ratio * duration;
    },
    [duration],
  );

  // Kéo tay cầm start/end bằng pointer (đáng tin hơn hai <input range> chồng nhau).
  const drag = useCallback(
    (which: "start" | "end") =>
      (e: React.PointerEvent): void => {
        e.preventDefault();
        e.stopPropagation();
        const move = (ev: PointerEvent): void => {
          const t = timeAtClientX(ev.clientX);
          if (which === "start") {
            setStart((prevStart) => {
              void prevStart;
              const s = Math.max(0, Math.min(t, end - 0.05));
              if (videoRef.current) {
                videoRef.current.currentTime = s;
              }
              return s;
            });
          } else {
            setEnd(() => {
              const en = Math.min(duration, Math.max(t, start + 0.05));
              if (videoRef.current) {
                videoRef.current.currentTime = en;
              }
              return en;
            });
          }
        };
        const up = (): void => {
          window.removeEventListener("pointermove", move);
          window.removeEventListener("pointerup", up);
        };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
      },
    [timeAtClientX, duration, start, end],
  );

  const seekTo = useCallback(
    (e: React.MouseEvent): void => {
      const t = timeAtClientX(e.clientX);
      if (videoRef.current) {
        videoRef.current.currentTime = t;
      }
    },
    [timeAtClientX],
  );

  const confirm = useCallback(async (): Promise<void> => {
    if (!target) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const newPath = await trimVideo(target.path, start, end, `${target.label} cat`);
      addVideoNode(newPath, `${target.label} · cắt ${fmt(start)}–${fmt(end)}`, { x: target.x, y: target.y });
      close();
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  }, [target, start, end, addVideoNode, close]);

  if (!target) {
    return null;
  }

  const pct = (t: number): number => (duration > 0 ? (t / duration) * 100 : 0);
  const selDur = Math.max(0, end - start);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" onClick={close}>
      <div
        className="flex max-h-[92vh] w-[760px] max-w-[calc(100vw-2rem)] flex-col gap-3 overflow-hidden rounded-2xl border border-line bg-panel p-3 text-fg shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold">Cắt video · {target.label}</h3>
          <button type="button" className="rounded-md px-2 py-1 text-muted hover:bg-fill2 hover:text-fg" onClick={close} aria-label="Đóng">
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
          onLoadedMetadata={onLoaded}
          onDurationChange={onLoaded}
          className="max-h-[52vh] w-full rounded-lg bg-black object-contain"
        />

        {/* Thanh thời gian: nền + đoạn chọn tô sáng + hai tay cầm kéo được */}
        <div className="px-1">
          <div ref={trackRef} className="relative h-9 cursor-pointer select-none rounded-md bg-fill2" onClick={seekTo}>
            <div
              className="absolute top-0 h-full rounded-md bg-sky-500/35"
              style={{ left: `${pct(start)}%`, width: `${pct(selDur)}%` }}
            />
            {/* Tay cầm bắt đầu */}
            <div
              className="absolute top-0 z-10 h-full w-3 -translate-x-1/2 cursor-ew-resize rounded bg-sky-400 shadow"
              style={{ left: `${pct(start)}%` }}
              onPointerDown={drag("start")}
              title="Kéo: điểm bắt đầu"
            />
            {/* Tay cầm kết thúc */}
            <div
              className="absolute top-0 z-10 h-full w-3 -translate-x-1/2 cursor-ew-resize rounded bg-sky-400 shadow"
              style={{ left: `${pct(end)}%` }}
              onPointerDown={drag("end")}
              title="Kéo: điểm kết thúc"
            />
          </div>
          <div className="mt-1 flex items-center justify-between text-[11px] text-muted">
            <span>Bắt đầu <span className="font-mono text-fg">{fmt(start)}</span></span>
            <span>Đoạn <span className="font-mono text-rest">{fmt(selDur)}</span></span>
            <span>Kết thúc <span className="font-mono text-fg">{fmt(end)}</span></span>
          </div>
          {duration <= 0 ? <p className="mt-1 text-[11px] text-ink3">Đang tải video…</p> : null}
        </div>

        {error ? <p className="text-xs text-rose">{error}</p> : null}

        <div className="flex justify-end gap-2">
          <button type="button" className="rounded-md border border-line2 bg-fill px-3 py-1 text-xs hover:bg-fill2" onClick={close}>
            Huỷ
          </button>
          <button
            type="button"
            disabled={busy || selDur <= 0 || duration <= 0}
            className="rounded-md bg-sky-600 px-3 py-1 text-xs font-medium text-fg hover:bg-sky-500 disabled:opacity-40"
            onClick={() => void confirm()}
          >
            {busy ? "Đang cắt…" : "Cắt & thêm vào canvas"}
          </button>
        </div>
      </div>
    </div>
  );
}
