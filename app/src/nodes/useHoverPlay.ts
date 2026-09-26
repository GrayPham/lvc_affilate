// Hover-để-phát cho video xem trước trong node (kiểu Lumina): rê chuột vào -> phát (muted, loop),
// rời ra -> tạm dừng. Video để `pointer-events-none` nên KÉO cả card vẫn mượt; handler gắn ở DIV
// bọc (nhận hover thay cho video). Dùng: const hp = useHoverPlay(); <div {...hp.hover}><video ref=
// {hp.videoRef} .../></div>.
import { useRef } from "react";

export function useHoverPlay() {
  const videoRef = useRef<HTMLVideoElement>(null);
  return {
    videoRef,
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
