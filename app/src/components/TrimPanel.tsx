// Bảng "Cắt video" kiểu Lumina — mở từ nút trên node video. Phát/xem video, kéo 2 tay cầm chọn đoạn
// [bắt đầu, kết thúc] trên thanh thời gian; "Cắt & thêm vào canvas" gọi ffmpeg (trim_video) tạo file
// mới rồi thêm node "Tải video lên" cho đoạn đã cắt. Dùng đường dẫn thật (`path`) để cắt, `src` để xem.
import { useRef, useState } from "react";

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
  const [duration, setDuration] = useState(0);
  const [start, setStart] = useState(0);
  const [end, setEnd] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!target) {
    return null;
  }

  function onLoaded(): void {
    const d = videoRef.current?.duration ?? 0;
    const dur = Number.isFinite(d) ? d : 0;
    setDuration(dur);
    setStart(0);
    setEnd(dur);
  }

  function seek(t: number): void {
    const v = videoRef.current;
    if (v) {
      v.currentTime = Math.max(0, Math.min(t, duration || 0));
    }
  }

  function setStartClamped(t: number): void {
    const s = Math.max(0, Math.min(t, end - 0.05));
    setStart(s);
    seek(s);
  }
  function setEndClamped(t: number): void {
    const e = Math.min(duration, Math.max(t, start + 0.05));
    setEnd(e);
    seek(e);
  }

  async function confirm(): Promise<void> {
    if (!target) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const newPath = await trimVideo(target.path, start, end, `${target.label} cat`);
      addVideoNode(newPath, `${target.label} · cắt ${fmt(start)}–${fmt(end)}`, { x: target.x, y: target.y });
      close();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  const selDur = Math.max(0, end - start);
  const pct = (t: number): number => (duration > 0 ? (t / duration) * 100 : 0);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" onClick={close}>
      <div
        className="flex max-h-[90vh] w-[720px] max-w-[calc(100vw-2rem)] flex-col gap-3 overflow-hidden rounded-2xl border border-white/10 bg-[#17171a] p-3 text-slate-200 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold">Cắt video · {target.label}</h3>
          <button type="button" className="rounded-md px-2 py-1 text-slate-400 hover:bg-white/10 hover:text-white" onClick={close} aria-label="Đóng">
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
          className="max-h-[44vh] w-full rounded-lg bg-black object-contain"
        />

        {/* Thanh thời gian: đoạn chọn tô sáng giữa hai tay cầm start/end */}
        <div className="relative h-6 rounded bg-white/10">
          <div
            className="absolute top-0 h-full rounded bg-sky-500/40"
            style={{ left: `${pct(start)}%`, width: `${pct(selDur)}%` }}
          />
        </div>

        <div className="flex flex-wrap items-center gap-3 text-xs text-slate-400">
          <label className="flex items-center gap-1">
            Bắt đầu
            <input
              type="range"
              min={0}
              max={duration || 0}
              step={0.05}
              value={start}
              onChange={(e) => setStartClamped(Number(e.target.value))}
              className="w-40"
            />
            <span className="w-14 font-mono text-slate-200">{fmt(start)}</span>
          </label>
          <label className="flex items-center gap-1">
            Kết thúc
            <input
              type="range"
              min={0}
              max={duration || 0}
              step={0.05}
              value={end}
              onChange={(e) => setEndClamped(Number(e.target.value))}
              className="w-40"
            />
            <span className="w-14 font-mono text-slate-200">{fmt(end)}</span>
          </label>
          <span className="ml-auto">Đoạn: <span className="font-mono text-sky-300">{fmt(selDur)}</span></span>
        </div>

        {error ? <p className="text-xs text-red-400">{error}</p> : null}

        <div className="flex justify-end gap-2">
          <button type="button" className="rounded-md border border-white/15 bg-white/5 px-3 py-1 text-xs hover:bg-white/10" onClick={close}>
            Huỷ
          </button>
          <button
            type="button"
            disabled={busy || selDur <= 0}
            className="rounded-md bg-sky-600 px-3 py-1 text-xs font-medium text-white hover:bg-sky-500 disabled:opacity-40"
            onClick={() => void confirm()}
          >
            {busy ? "Đang cắt…" : "Cắt & thêm vào canvas"}
          </button>
        </div>
      </div>
    </div>
  );
}
