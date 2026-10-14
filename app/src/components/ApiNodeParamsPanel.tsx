// Bang tham so DUOI (P-lumina, cung mot cho dung VideoParamsPanel/dola_video) cho 7 node goi API
// tao anh/video (Veo3/Google/Seedance/Grok) — TRUOC day cac node nay nhet toan bo truong tham so
// (model/ti le/tai khoan…) thang vao the node nho tren canvas (chat, kho doc — nguoi dung phan anh
// "quá tệ hại" so voi node Dola). Nay moi node chi con nhan + OutputPreview tren canvas
// (`nodes/<Kind>Node.tsx`), tham so day du chuyen xuong day — hien khi node do duoc chon, dung mo
// hinh/chrome y het `VideoParamsPanel` (chip tham chieu + o prompt tu node Prompt da noi + hang
// tham so). Mot FILE CHUNG thay vi 7 file gan giong nhau — khac nhau o DANH SACH TRUONG
// (`FIELD_SETS` duoi), khong phai o chrome.
import { referenceChips } from "../graph/refs";
import { useGraphStore } from "../store/graph";
import type { NodeKind, NodeKindTag } from "../ipc/types";
import PromptField from "./PromptField";

const FIELD_CLASS =
  "rounded-md border border-line bg-fill px-2 py-1 text-xs text-ink outline-none focus:border-sky-400/60";

const ASPECT_PL: ReadonlyArray<readonly [string, string]> = [
  ["portrait", "Dọc"],
  ["landscape", "Ngang"],
];
const ASPECT_PLS: ReadonlyArray<readonly [string, string]> = [
  ["portrait", "Dọc"],
  ["landscape", "Ngang"],
  ["square", "Vuông"],
];
const VEO3_IMAGE_MODELS: ReadonlyArray<readonly [string, string]> = [
  ["GEM_PIX_2", "Banana Pro (GEM_PIX_2)"],
  ["NARWHAL", "Banana 2 (NARWHAL)"],
  ["HARBOR_SEAL", "Lite (HARBOR_SEAL)"],
];
// muse.ai KHONG co tham so API that cho ty le/thoi luong (xac nhan qua GET /model, xem
// node-museai-video/CLAUDE.md) — day chi la CHUOI CHEN VAO PROMPT, khong dam bao AI tuan theo.
const MUSEAI_ASPECT: ReadonlyArray<readonly [string, string]> = [
  ["", "Không chỉ định"],
  ["16:9", "16:9 (ngang)"],
  ["9:16", "9:16 (dọc)"],
  ["1:1", "1:1 (vuông)"],
];

type ApiNodeData = Extract<
  NodeKind,
  { kind: "veo3_image" | "veo3_video" | "google_image" | "google_video" | "seedance_image" | "seedance_video" | "grok_image" | "museai_video" | "museai_image" | "tts" }
>;

/** Cong nhan ANH tham chieu cua tung kind — `undefined` = khong co (vd `grok_image`). */
const REF_PORT: Partial<Record<NodeKindTag, string>> = {
  veo3_image: "refs",
  veo3_video: "refs",
  google_video: "ref",
  seedance_video: "ref",
};

const API_KINDS = new Set<NodeKindTag>([
  "veo3_image",
  "veo3_video",
  "google_image",
  "google_video",
  "seedance_image",
  "seedance_video",
  "grok_image",
  "museai_video",
  "museai_image",
  "tts",
]);

const TTS_PROVIDER: ReadonlyArray<readonly [string, string]> = [
  ["elevenlabs", "ElevenLabs"],
  ["minimax", "MiniMax"],
];

function Select({
  value,
  title,
  options,
  onChange,
  className = "",
}: {
  value: string;
  title: string;
  options: ReadonlyArray<readonly [string, string]>;
  onChange: (v: string) => void;
  className?: string;
}) {
  return (
    <select className={`${FIELD_CLASS} ${className}`} value={value} title={title} onChange={(e) => onChange(e.target.value)}>
      {options.map(([v, label]) => (
        <option key={v} value={v} className="bg-panel text-ink">
          {label}
        </option>
      ))}
    </select>
  );
}

function Text({
  value,
  placeholder,
  onChange,
  className = "flex-1",
  type = "text",
}: {
  value: string | number;
  placeholder: string;
  onChange: (v: string) => void;
  className?: string;
  type?: "text" | "number";
}) {
  return (
    <input
      type={type}
      className={`${FIELD_CLASS} ${className}`}
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

/** Mot "hang tham so" rieng theo kind — tach ham THUAN nhan (data, update) de de doc/sua tung kind
 * doc lap, khong phai sua chung mot ham khong lo re nhanh theo kind. */
function fieldsFor(data: ApiNodeData, update: (patch: Partial<ApiNodeData>) => void) {
  switch (data.kind) {
    case "veo3_image":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Select value={data.model} title="Model tạo ảnh" options={VEO3_IMAGE_MODELS} onChange={(v) => update({ model: v })} className="flex-1" />
            <Select value={data.aspect} title="Tỉ lệ khung" options={ASPECT_PL} onChange={(v) => update({ aspect: v })} />
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <label className="flex flex-1 items-center gap-1.5 text-[11px] text-ink2">
              Seed
              <Text
                type="number"
                value={data.seed ?? ""}
                placeholder="rỗng = ngẫu nhiên"
                onChange={(v) => update({ seed: v === "" ? null : Number(v) })}
              />
            </label>
          </div>
          <Text value={data.account} placeholder="Tài khoản (tên/label credential Google; rỗng = tự chọn)" onChange={(v) => update({ account: v })} />
        </>
      );
    case "veo3_video":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={data.mode}
              title="Chế độ"
              options={[
                ["t2v", "Chữ → video (t2v)"],
                ["r2v", "Ảnh tham chiếu → video (r2v, tối đa 6 ảnh)"],
              ]}
              onChange={(v) => update({ mode: v })}
              className="flex-1"
            />
            <Select value={data.aspect} title="Tỉ lệ khung" options={ASPECT_PL} onChange={(v) => update({ aspect: v })} />
          </div>
          <Text value={data.model} placeholder="Model (rỗng = mặc định theo mode)" onChange={(v) => update({ model: v })} />
          <div className="flex flex-wrap items-center gap-2">
            <label className="flex items-center gap-1.5 text-[11px] text-ink2">
              Số biến thể
              <Text type="number" value={data.seed_count} placeholder="1" className="w-16" onChange={(v) => update({ seed_count: Math.max(1, Number(v) || 1) })} />
            </label>
            <Text value={data.account} placeholder="Tài khoản (tên/label credential Google; rỗng = tự chọn)" onChange={(v) => update({ account: v })} />
          </div>
          <p className="flex items-center gap-1.5 text-[10px] text-ink3">
            <span className="rounded bg-fill2 px-1 py-0.5 text-[9px] uppercase tracking-wide text-ink2">Sắp có</span>
            Chế độ trình duyệt chống phát hiện (Camoufox)
          </p>
        </>
      );
    case "google_image":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Text value={data.model} placeholder="Model (rỗng = gemini-3.1-flash-image)" onChange={(v) => update({ model: v })} />
            <Select value={data.aspect} title="Tỉ lệ khung" options={ASPECT_PLS} onChange={(v) => update({ aspect: v })} />
          </div>
          <Text value={data.credential} placeholder="Tên API key Google (bắt buộc)" onChange={(v) => update({ credential: v })} />
        </>
      );
    case "google_video":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={data.mode}
              title="Chế độ"
              options={[
                ["t2v", "Chữ → video (t2v)"],
                ["i2v", "Ảnh → video (i2v)"],
              ]}
              onChange={(v) => update({ mode: v })}
              className="flex-1"
            />
            <Select value={data.aspect} title="Tỉ lệ khung" options={ASPECT_PLS} onChange={(v) => update({ aspect: v })} />
          </div>
          <Text value={data.model} placeholder="Model (rỗng = veo-3.1-generate-preview)" onChange={(v) => update({ model: v })} />
          <Text value={data.credential} placeholder="Tên API key Google (bắt buộc)" onChange={(v) => update({ credential: v })} />
        </>
      );
    case "seedance_image":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Text value={data.model} placeholder="Model / endpoint id (rỗng = mặc định)" onChange={(v) => update({ model: v })} />
            <Select value={data.aspect} title="Tỉ lệ khung" options={ASPECT_PLS} onChange={(v) => update({ aspect: v })} />
          </div>
          <Text value={data.credential} placeholder="Tên API key Seedance (bắt buộc)" onChange={(v) => update({ credential: v })} />
        </>
      );
    case "seedance_video":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={data.mode}
              title="Chế độ"
              options={[
                ["t2v", "Chữ → video (t2v)"],
                ["i2v", "Ảnh → video (i2v)"],
              ]}
              onChange={(v) => update({ mode: v })}
              className="flex-1"
            />
            <Select value={data.aspect} title="Tỉ lệ khung" options={ASPECT_PLS} onChange={(v) => update({ aspect: v })} />
          </div>
          <Text value={data.model} placeholder="Model / endpoint id (rỗng = mặc định)" onChange={(v) => update({ model: v })} />
          <Text value={data.credential} placeholder="Tên API key Seedance (bắt buộc)" onChange={(v) => update({ credential: v })} />
        </>
      );
    case "grok_image":
      return (
        <>
          <Text value={data.model} placeholder="Model (rỗng = grok-2-image-1212)" onChange={(v) => update({ model: v })} />
          <Text value={data.credential} placeholder="Tên API key Grok (bắt buộc)" onChange={(v) => update({ credential: v })} />
        </>
      );
    case "museai_video":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Select value={data.aspect} title="Tỉ lệ khung hình (chỉ chèn vào prompt, không đảm bảo)" options={MUSEAI_ASPECT} onChange={(v) => update({ aspect: v })} className="flex-1" />
            <Text
              type="number"
              value={data.duration ?? ""}
              placeholder="Giây (không đảm bảo)"
              className="w-28"
              onChange={(v) => update({ duration: v === "" ? null : Number(v) })}
            />
          </div>
          <p className="text-[10px] text-ink3">
            muse.ai không có API tỉ lệ/thời lượng — 2 ô trên chỉ CHÈN THÊM vào câu prompt, AI có thể không tuân theo.
          </p>
          <Text value={data.account} placeholder="Tài khoản (tên/label credential MuseAI; rỗng = tự chọn)" onChange={(v) => update({ account: v })} />
        </>
      );
    case "museai_image":
      return (
        <Text value={data.account} placeholder="Tài khoản (tên/label credential MuseAI; rỗng = tự chọn)" onChange={(v) => update({ account: v })} />
      );
    case "tts":
      return (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={data.provider}
              title="Nhà cung cấp"
              options={TTS_PROVIDER}
              onChange={(v) => update({ provider: v as "elevenlabs" | "minimax" })}
              className="flex-1"
            />
            <Text
              type="number"
              value={data.speed}
              placeholder="Tốc độ (1.0)"
              className="w-28"
              onChange={(v) => update({ speed: v === "" ? 1 : Number(v) })}
            />
          </div>
          <Text value={data.voice_id} placeholder="Id giọng đọc (bắt buộc)" onChange={(v) => update({ voice_id: v })} />
          <Text value={data.model} placeholder="Model (rỗng = mặc định)" onChange={(v) => update({ model: v })} />
          <Text value={data.credential} placeholder="Tên API key (rỗng = tự chọn)" onChange={(v) => update({ credential: v })} />
        </>
      );
  }
}

export default function ApiNodeParamsPanel() {
  const selectedNodeId = useGraphStore((s) => s.selectedNodeId);
  const nodes = useGraphStore((s) => s.nodes);
  const edges = useGraphStore((s) => s.edges);
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const onEdgesChange = useGraphStore((s) => s.onEdgesChange);

  const node = selectedNodeId ? nodes.find((n) => n.id === selectedNodeId) : undefined;
  if (!node || !API_KINDS.has(node.data.kind)) {
    return null;
  }
  const id = node.id;
  const data = node.data as ApiNodeData;
  const refPort = REF_PORT[data.kind];
  const chips = refPort ? referenceChips(id, nodes, edges, refPort) : [];

  function update(patch: Partial<ApiNodeData>): void {
    updateNodeData(id, { ...data, ...patch } as NodeKind);
  }

  return (
    <div className="pointer-events-auto flex w-[26rem] max-w-[calc(100%-2rem)] flex-col gap-2 rounded-2xl border border-line bg-[#17171a]/95 px-3 py-2.5 text-ink shadow-2xl backdrop-blur-md">
      {chips.length > 0 ? (
        <div className="flex flex-wrap items-center gap-1.5 border-b border-line pb-2">
          <span className="text-[11px] text-ink2">Tham chiếu:</span>
          {chips.map((chip) => (
            <span key={chip.edgeId} className="flex items-center gap-1 rounded-md bg-fill2 py-0.5 pl-2 pr-1 text-[11px] text-ink" title={chip.label}>
              <span className="max-w-[9rem] truncate">{chip.label}</span>
              <button
                type="button"
                title="Bỏ tham chiếu"
                aria-label={`Bỏ tham chiếu ${chip.label}`}
                className="flex h-4 w-4 items-center justify-center rounded text-ink2 hover:bg-fill2 hover:text-ink"
                onClick={() => onEdgesChange([{ type: "remove", id: chip.edgeId }])}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      ) : null}

      <PromptField id={id} />

      <div className="flex flex-col gap-2 border-t border-line pt-2">{fieldsFor(data, update)}</div>
    </div>
  );
}
