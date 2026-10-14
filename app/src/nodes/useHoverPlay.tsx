// Hover-để-phát cho video xem trước trong node (kiểu Lumina): rê chuột vào -> phát, rời ra -> tạm
// dừng + tua về đầu. Video để `pointer-events-none` nên KÉO cả card vẫn mượt; handler gắn ở DIV
// bọc (nhận hover thay cho video).
//
// Tiếng: mặc định TẮT vì trình duyệt chặn tự phát khi có tiếng (rê chuột qua mà máy rục tiếng
// cũng làm người dùng giật mình). Nút loa `<MuteButton>` bật lên, và lựa chọn đó được nhớ cho mọi
// node ở mọi lần mở app sau (`uiStore.previewMuted`).
//
// Dùng:
//   const hp = useHoverPlay();
//   <div className="relative" {...hp.hover}>
//     <video ref={hp.videoRef} muted={hp.muted} … />
//     <MuteButton />
//   </div>
import { useRef } from "react";

import { useUiStore } from "../store/uiStore";

export function useHoverPlay() {
  const videoRef = useRef<HTMLVideoElement>(null);
  const muted = useUiStore((s) => s.previewMuted);
  return {
    videoRef,
    muted,
    hover: {
      onMouseEnter: (): void => {
        const v = videoRef.current;
        if (v) {
          void v.play().catch(() => {
            /* autoplay bị chặn / chưa sẵn sàng — bỏ qua */
          });
        }
      },
      onMouseLeave: (): void => {
        videoRef.current?.pause();
      },
    },
  };
}

/** Nút bật/tắt tiếng cho video xem trước. Đặt trong vùng xem trước (`position: relative`). */
export function MuteButton() {
  const muted = useUiStore((s) => s.previewMuted);
  const toggle = useUiStore((s) => s.togglePreviewMuted);
  return (
    <button
      type="button"
      // `nodrag` để bấm nút không kéo cả node; `stopPropagation` để không mở lightbox.
      className="nodrag absolute right-1.5 bottom-1.5 z-10 grid h-7 w-7 place-items-center rounded-full bg-black/55 text-white hover:bg-black/75"
      title={muted ? "Bật tiếng khi xem trước" : "Tắt tiếng khi xem trước"}
      aria-label={muted ? "Bật tiếng" : "Tắt tiếng"}
      aria-pressed={!muted}
      onClick={(e) => {
        e.stopPropagation();
        toggle();
      }}
    >
      {muted ? (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M11 5 6 9H2v6h4l5 4V5Z" />
          <path d="m23 9-6 6M17 9l6 6" />
        </svg>
      ) : (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M11 5 6 9H2v6h4l5 4V5Z" />
          <path d="M15.5 8.5a5 5 0 0 1 0 7M19 5a9 9 0 0 1 0 14" />
        </svg>
      )}
    </button>
  );
}
