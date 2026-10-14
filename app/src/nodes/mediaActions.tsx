// Bo HANH DONG dung chung cho thanh cong cu noi (`NodeFloatingToolbar`) cua moi node TAO video/anh
// qua API (Veo3/Google/Seedance/Grok/MuseAI...) — truoc day CHI `VideoNode.tsx` (Dola) co day du bo
// nay (Phóng to/Cắt/Chụp khung hình/Tách audio/Tách audio↔video/Tải xuống), cac node API khac chi
// co nut Xoá (review nguoi dung 2026-10-13: "muse, veo3 đều giống như dola chứ" — node video nao
// cung nen co CUNG bo hanh dong, khong rieng Dola). Tach icon + ham dung o day thay vi lap lai trong
// tung file node (da thay trung het trong `VideoNode.tsx` khi lam MuseAI/Veo3).
//
// Trim/frame-capture/audio-extract/audio-split la NODE/BANG CHUNG (khong rieng Dola) nen dung lai
// duoc NGUYEN VAN cho bat ky nguon video nao — chi can `src`/`path` that.
import type { AddableNodeKind } from "../graph/ports";
import type { PendingConnect } from "../graph/connect";
import type { CaptureTarget, TrimTarget } from "../store/workflowUi";
import type { ToolbarAction } from "./NodeFloatingToolbar";

const ExpandIcon = (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
  </svg>
);
const TrimIcon = (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="6" cy="6" r="3" />
    <circle cx="6" cy="18" r="3" />
    <path d="M20 4L8.12 15.88M14.47 14.48L20 20M8.12 8.12L12 12" />
  </svg>
);
const FrameIcon = (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
    <circle cx="12" cy="13" r="4" />
  </svg>
);
const AudioExtractIcon = (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M9 18V5l12-2v13" />
    <circle cx="6" cy="18" r="3" />
    <circle cx="18" cy="16" r="3" />
  </svg>
);
const AudioSplitIcon = (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 4v4" />
    <path d="M12 8L6 14" />
    <path d="M12 8l6 6" />
    <path d="M6 14v4" />
    <path d="M18 14v4" />
  </svg>
);
const DownloadIcon = (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
    <polyline points="7 10 12 15 17 10" />
    <line x1="12" y1="15" x2="12" y2="3" />
  </svg>
);

interface MediaResultActionsParams {
  nodeId: string;
  nodeLabel: string;
  /** `assetUrl(path)` — rong khi chua co ket qua (cac nut lien quan se `disabled`). */
  src: string;
  /** Vi tri node (them node moi canh ben phai) — `undefined` tru luc chua do duoc (hiem). */
  selfPos: { x: number; y: number } | undefined;
  addNodeConnected: (
    kind: AddableNodeKind,
    position: { x: number; y: number },
    from: Omit<PendingConnect, "kind">,
  ) => void;
  onExpand: () => void;
}

/** `expand` (mo Lightbox) + `download` (them node Lưu nối sẵn) — dung CHUNG cho MOI ket qua co file
 * that (video lan anh), khong rieng loai nao. */
function baseActions(p: MediaResultActionsParams): ToolbarAction[] {
  const base = p.selfPos ?? { x: 0, y: 0 };
  return [
    {
      key: "expand",
      title: p.src ? "Phóng to" : "Chưa có kết quả (chạy để tạo)",
      disabled: !p.src,
      onClick: p.onExpand,
      icon: ExpandIcon,
    },
    {
      key: "download",
      title: p.src ? "Tải xuống (thêm node Lưu, nối từ node này)" : "Chưa có kết quả (chạy để tạo)",
      disabled: !p.src,
      onClick: () => {
        p.addNodeConnected(
          "save",
          { x: base.x + 340, y: base.y + 270 },
          { nodeId: p.nodeId, handleId: "out", handleType: "source" },
        );
      },
      icon: DownloadIcon,
    },
  ];
}

interface VideoResultActionsParams extends MediaResultActionsParams {
  /** Duong dan THAT (khong phai `asset://`) — can cho bang Cắt (ffmpeg doc file truc tiep). */
  path: string | undefined;
  openTrim: (t: TrimTarget) => void;
  openCapture: (t: CaptureTarget) => void;
}

/** Bo hanh dong day du cho node KET QUA VIDEO: Phóng to, Cắt, Chụp khung hình, Tách audio, Tách
 * audio↔video, Tải xuống — khop CHINH XAC `VideoNode.tsx` (Dola), dung lai cho moi node video API
 * khac (Veo3/Google/Seedance/MuseAI...). */
export function buildVideoResultActions(p: VideoResultActionsParams): ToolbarAction[] {
  const base = p.selfPos ?? { x: 0, y: 0 };
  const [expand, download] = baseActions(p);
  return [
    expand!,
    {
      key: "trim",
      title: p.src ? "Cắt video (chọn đoạn → thêm vào canvas)" : "Chưa có video (chạy để tạo)",
      disabled: !p.src,
      onClick: () => {
        p.openTrim({ nodeId: p.nodeId, src: p.src, path: p.path ?? "", label: p.nodeLabel, x: base.x, y: base.y });
      },
      icon: TrimIcon,
    },
    {
      key: "frame",
      title: p.src ? "Chụp khung hình (bắt khung → thêm ảnh vào canvas)" : "Chưa có video (chạy để tạo)",
      disabled: !p.src,
      onClick: () => {
        p.openCapture({ nodeId: p.nodeId, src: p.src, label: p.nodeLabel, x: base.x, y: base.y });
      },
      icon: FrameIcon,
    },
    {
      key: "audio-extract",
      title: "Tách audio (thêm node Tách audio, nối từ video này)",
      onClick: () => {
        p.addNodeConnected(
          "audio_extract",
          { x: base.x + 340, y: base.y - 90 },
          { nodeId: p.nodeId, handleId: "out", handleType: "source" },
        );
      },
      icon: AudioExtractIcon,
    },
    {
      key: "audio-split",
      title: "Tách audio↔video (thêm node Tách audio↔video, nối từ video này)",
      onClick: () => {
        p.addNodeConnected(
          "audio_split",
          { x: base.x + 340, y: base.y + 90 },
          { nodeId: p.nodeId, handleId: "out", handleType: "source" },
        );
      },
      icon: AudioSplitIcon,
    },
    download!,
  ];
}

/** Bo hanh dong cho node KET QUA ANH: Phóng to, Tải xuống — Cắt/Chụp khung hình/Tách audio KHONG
 * ap dung cho anh tinh nen khong co trong bo nay. */
export function buildImageResultActions(p: MediaResultActionsParams): ToolbarAction[] {
  return baseActions(p);
}

/** Bo hanh dong cho node KET QUA AUDIO (node `tts`): Phóng to (nghe thu qua Lightbox), Tải xuống —
 * Cắt/Chụp khung hình/Tách audio KHONG ap dung vi nguon DA LA audio, khong phai video. */
export function buildAudioResultActions(p: MediaResultActionsParams): ToolbarAction[] {
  return baseActions(p);
}
