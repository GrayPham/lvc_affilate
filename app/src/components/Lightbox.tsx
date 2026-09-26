// Lightbox (P-lumina): overlay xem to anh/video. Bam nen hoac × de dong. `src` rong -> khong hien.
import { createPortal } from "react-dom";

interface LightboxProps {
  src: string;
  kind: "image" | "video";
  onClose: () => void;
}

export default function Lightbox({ src, kind, onClose }: LightboxProps) {
  if (!src) {
    return null;
  }
  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-8"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
    >
      <button
        type="button"
        className="absolute right-4 top-4 flex h-9 w-9 items-center justify-center rounded-full bg-white/10 text-white hover:bg-white/20"
        onClick={onClose}
        aria-label="Đóng"
        title="Đóng"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
      </button>
      {kind === "video" ? (
        <video
          src={src}
          className="max-h-full max-w-full rounded-lg bg-black"
          controls
          autoPlay
          onClick={(e) => e.stopPropagation()}
        />
      ) : (
        <img
          src={src}
          alt=""
          className="max-h-full max-w-full rounded-lg object-contain"
          onClick={(e) => e.stopPropagation()}
        />
      )}
    </div>,
    document.body,
  );
}
