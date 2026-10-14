// Vung xem truoc KET QUA (anh/video) dung CHUNG cho cac node tao anh/video qua API (Veo3/Google/
// Seedance/Grok/MuseAI...) thay vi moi node tu ve rieng (truoc day CHI `VideoNode`/Dola co preview,
// cac node API moi chi la form tham so tho — thieu nhat quan UI). Doc `useRunStore().nodeVideos[id]`:
// ten map ke thua tu Dola nhung THUC CHAT la "duong dan output HOAN TAT gan nhat cua node nay" bat
// ke video hay anh (xem `store/run.ts::reconcileTasksFromDb` gan tu BAT KY `TaskDto.localPath` nao,
// khong rieng video) — dung lai duoc cho ca anh, khong can map rieng.
//
// Video: RE CHUOT DE XEM NHANH (hover -> play muted/loop, roi -> dung), giong HET `VideoNode.tsx`
// (Dola) qua `useHoverPlay` — video giu `pointer-events-none` nen KEO CA NODE qua vung nay van muot
// (khong xung dot keo-tha cua React Flow). Xem DAY DU (am thanh, tua, toan man hinh) la viec cua
// nut "Phóng to" tren thanh cong cu noi (`NodeFloatingToolbar`) mo `Lightbox` — node GOI component
// nay tu quan ly state `zoom`/`<Lightbox>`, KHONG lam trong `OutputPreview` (moi node can bo nut
// toolbar KHAC nhau — xem `VideoNode.tsx` lam mau: expand/trim/frame/audio-extract/audio-split/
// download — review nguoi dung 2026-10-13: "muse, veo3 đều giống như dola chứ", cac node API
// (Veo3/MuseAI...) truoc day THIEU toan bo toolbar nay, chi co nut Xoa).
import { assetUrl } from "../ipc/tauri";
import { useHoverPlay } from "./useHoverPlay";

export default function OutputPreview({
  kind,
  path,
  label,
}: {
  kind: "image" | "video" | "audio";
  path: string | undefined;
  label: string;
}) {
  const src = assetUrl(path);
  const hp = useHoverPlay();
  return (
    <div
      className="relative flex aspect-video w-full items-center justify-center bg-gradient-to-br from-[#1f1f26] to-[#111114]"
      {...(src && kind === "video" ? hp.hover : {})}
      title={src ? (kind === "video" ? "Rê chuột để xem; Phóng to để phát đầy đủ" : undefined) : "Chưa có kết quả (chạy để tạo)"}
    >
      {src ? (
        kind === "image" ? (
          // eslint-disable-next-line @typescript-eslint/no-explicit-any -- the <img> ket qua tao tu asset:// cuc bo, khong phai noi dung nguoi dung nhap
          <img src={src} className="absolute inset-0 h-full w-full bg-black object-contain" alt="" />
        ) : kind === "audio" ? (
          <audio src={src} className="absolute inset-x-2 bottom-2 w-[calc(100%-1rem)]" controls preload="metadata" />
        ) : (
          <video
            ref={hp.videoRef}
            src={src}
            className="pointer-events-none absolute inset-0 h-full w-full bg-black object-contain"
            muted
            loop
            playsInline
            preload="metadata"
          />
        )
      ) : (
        <span className="flex h-9 w-9 items-center justify-center rounded-full bg-white/10 text-white">
          {kind === "image" ? (
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <circle cx="8.5" cy="8.5" r="1.5" />
              <path d="M21 15l-5-5L5 21" />
            </svg>
          ) : kind === "audio" ? (
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M9 18V5l12-2v13" />
              <circle cx="6" cy="18" r="3" />
              <circle cx="18" cy="16" r="3" />
            </svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          )}
        </span>
      )}
      <span className="pointer-events-none absolute bottom-1.5 left-1.5 rounded bg-black/55 px-1.5 py-0.5 text-[9px] text-slate-200 backdrop-blur-sm">
        {label}
      </span>
    </div>
  );
}
