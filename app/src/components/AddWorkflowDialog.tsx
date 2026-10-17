// Hop thoai "Thêm workflow" (vao hang cho) — dung theo
// `references/lvc-ui-design/screens/AddWorkflow.dc.html`.
//
// ## Khac ban thiet ke mot cho quan trọng, co chu dich
// Ban thiet ke ve mot form SUA tham so tung node ngay trong hop thoai (chon tai khoan, go prompt
// tung canh, chon thu muc luu). O day KHONG lam vay: bang tham so cua node da co san tren canvas
// (thanh kiem tra ben phai moi node), dung lai lan hai trong hop thoai nay se thanh HAI trinh sua
// cung mot du lieu — chung se troi nhau ngay lan them node moi dau tien.
//
// Thay vao do hop thoai lam dung ba viec no lam tot nhat:
//  1. kiem do thi bang CHINH bo kiem cua engine (`workflowValidate`) -> bao dung node nao thieu gi;
//  2. canh bao thieu tai khoan TRUOC khi vao hang cho (`missingCredentials`) — toi luot chay la 3
//     gio sang, luc do bao thi khong ai sua duoc;
//  3. cho doi thu muc luu RIENG cho lan chay nay, khong doi cai dat chung.
import { useEffect, useMemo, useState } from "react";

import {
  nodeCatalog,
  pickDirectory,
  pickFiles,
  workflowGet,
  workflowList,
  workflowValidate,
} from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { formatGraphError } from "../graph/errors";
import { missingCredentials } from "../graph/credentials";
import { countScenes } from "../graph/sceneCount";
import { useCredentialsStore } from "../store/credentials";
import { useQueueStore } from "../store/queue";
import { useSettingsStore } from "../store/settings";
import { Button, Dialog, DialogFooter, Field, INPUT_CLASS } from "../ui/primitives";
import { GraphErrorSchema } from "../ipc/types";
import type { Graph, NodeManifest, WorkflowMeta } from "../ipc/types";

/** Kind cua node "Lưu video" — noi duy nhat `out_dir` co y nghia. */
const SAVE_KIND = "save";

/** Kind cua node "Prompt" — noi nhan du lieu dau vao (moi DONG mot canh). */
const PROMPT_KIND = "prompt";

/** Kind cua node "Tải ảnh lên" — noi nhan anh tham chieu (cong `refs` cua node video). */
const IMAGE_KIND = "image_upload";

/**
 * Tra ve ban SAO cua do thi voi moi node `save` doi `out_dir` thanh `dir`. `dir` rong -> giu nguyen.
 *
 * THUAN, export de test: day la cho duy nhat "thu muc luu rieng cho lan chay nay" tro thanh su
 * that — hang cho luu ANH CHUP do thi, nen phai sua anh chup chu khong phai sua workflow goc (sua
 * workflow goc se doi ca nhung lan chay sau, khong phai y nguoi dung).
 */
export function withOutDir(graph: Graph, dir: string): Graph {
  if (dir.trim() === "") {
    return graph;
  }
  return {
    ...graph,
    nodes: graph.nodes.map((n) =>
      n.kind.kind === SAVE_KIND
        ? { ...n, kind: { ...n.kind, out_dir: dir.trim() } }
        : n,
    ),
  };
}

/** So node `save` trong do thi — de biet co nen hien o "Thư mục lưu" hay khong. */
export function countSaveNodes(graph: Graph): number {
  return graph.nodes.filter((n) => n.kind.kind === SAVE_KIND).length;
}

/** So node `prompt` — do thi khong co node nao thi khong co cho nhan dau vao. */
export function countPromptNodes(graph: Graph): number {
  return graph.nodes.filter((n) => n.kind.kind === PROMPT_KIND).length;
}

/** So node `image_upload` — do thi co thi hien o chon anh, khong thi an. */
export function countImageNodes(graph: Graph): number {
  return graph.nodes.filter((n) => n.kind.kind === IMAGE_KIND).length;
}

/**
 * Tra ban SAO cua do thi voi moi node `image_upload` thay `paths` bang danh sach anh moi. Rong ->
 * giu nguyen (dung anh da cau hinh san trong workflow, neu co).
 *
 * THUAN, export de test. Cung ly do voi `withPromptText`: hang cho luu ANH CHUP, nen sua anh chup
 * — workflow goc tren canvas KHONG doi.
 */
export function withImages(graph: Graph, paths: readonly string[]): Graph {
  if (paths.length === 0) {
    return graph;
  }
  return {
    ...graph,
    nodes: graph.nodes.map((n) =>
      n.kind.kind === IMAGE_KIND ? { ...n, kind: { ...n.kind, paths: [...paths] } } : n,
    ),
  };
}

/**
 * Tra ban SAO cua do thi voi moi node `prompt` thay `text` bang `text` moi. Rong -> giu nguyen.
 *
 * THUAN, export de test. Day la cho "nhap du lieu dau vao de chay workflow" tro thanh su that:
 * hang cho luu ANH CHUP do thi, nen sua anh chup — workflow goc tren canvas KHONG doi, lan chay
 * sau van dung prompt cu cua no.
 */
export function withPromptText(graph: Graph, text: string): Graph {
  if (text.trim() === "") {
    return graph;
  }
  const clean = text
    .split("\n")
    .filter((l) => l.trim() !== "")
    .join("\n");
  return {
    ...graph,
    nodes: graph.nodes.map((n) =>
      n.kind.kind === PROMPT_KIND ? { ...n, kind: { ...n.kind, text: clean } } : n,
    ),
  };
}

interface AddWorkflowDialogProps {
  onClose: () => void;
  /** Goi sau khi them thanh cong, kem ten workflow da them. */
  onAdded?: (name: string) => void;
}

export default function AddWorkflowDialog({ onClose, onAdded }: AddWorkflowDialogProps) {
  const enqueue = useQueueStore((s) => s.add);
  const [catalog, setCatalog] = useState<NodeManifest[]>([]);
  const credentials = useCredentialsStore((s) => s.rows);
  const loadCredentials = useCredentialsStore((s) => s.load);
  const settings = useSettingsStore((s) => s.saved);
  const loadSettings = useSettingsStore((s) => s.load);

  const [list, setList] = useState<WorkflowMeta[] | null>(null);
  const [pickedId, setPickedId] = useState("");
  const [graph, setGraph] = useState<Graph | null>(null);
  const [outDir, setOutDir] = useState("");
  const [promptText, setPromptText] = useState("");
  /** Anh tham chieu nguoi dung chon — tiem vao node `image_upload` cua anh chup do thi. */
  const [images, setImages] = useState<string[]>([]);
  /** So ban dong nhat xep vao hang cho. AI sinh moi lan mot khac, nen chay N lan la cach "tao
   * nhieu phien ban de chon cai dep nhat". */
  const [count, setCount] = useState(1);
  const [videoTotal, setVideoTotal] = useState<number | null>(null);
  const [graphErrors, setGraphErrors] = useState<string[]>([]);
  const [checking, setChecking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Khong co store rieng cho danh muc node — cac component khac (`ScriptVideoConfig`) cung goi
    // thang `nodeCatalog()`, giu dung quy uoc do. Loi o day chi lam mat canh bao "thieu tai
    // khoan", khong duoc chan ca hop thoai.
    nodeCatalog()
      .then(setCatalog)
      .catch(() => setCatalog([]));
    void loadCredentials();
    void loadSettings();
    workflowList()
      .then((rows) => {
        setList(rows);
        if (rows.length > 0) {
          setPickedId(rows[0]!.id);
        }
      })
      .catch((e: unknown) => setError(errorText(e)));
  }, [loadCredentials, loadSettings]);

  // Doi workflow -> nap do thi roi kiem NGAY, khong cho nguoi dung bam "Thêm" moi phat hien sai.
  useEffect(() => {
    if (pickedId === "") {
      return;
    }
    let cancelled = false;
    setChecking(true);
    setGraph(null);
    setGraphErrors([]);
    setVideoTotal(null);
    workflowGet(pickedId)
      .then(async (doc) => {
        if (cancelled) {
          return;
        }
        setGraph(doc.graph);
        try {
          const plan = await workflowValidate(doc.graph);
          if (!cancelled) {
            setVideoTotal(plan.videoTotal);
          }
        } catch (e) {
          if (cancelled) {
            return;
          }
          // `workflow_validate` tra `details` = danh sach `GraphError` -> doi thanh cau tieng Viet
          // bang CHINH bo dich dung tren canvas, de hai cho khong mo ta cung loi theo hai kieu.
          const details = (e as { details?: unknown })?.details;
          const parsed = GraphErrorSchema.array().safeParse(details);
          setGraphErrors(
            parsed.success ? parsed.data.map(formatGraphError) : [errorText(e)],
          );
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setError(errorText(e));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setChecking(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [pickedId]);

  const missing = useMemo(
    () => (graph ? missingCredentials(graph, catalog, credentials) : []),
    [graph, catalog, credentials],
  );
  const saveCount = useMemo(() => (graph ? countSaveNodes(graph) : 0), [graph]);
  const promptCount = useMemo(() => (graph ? countPromptNodes(graph) : 0), [graph]);
  const imageCount = useMemo(() => (graph ? countImageNodes(graph) : 0), [graph]);
  const picked = list?.find((w) => w.id === pickedId);

  async function pickOutDir(): Promise<void> {
    setError(null);
    try {
      const dir = await pickDirectory();
      if (dir !== null) {
        setOutDir(dir);
      }
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function handleAdd(): Promise<void> {
    if (!graph || !picked) {
      return;
    }
    setBusy(true);
    setError(null);
    // Tiem text + anh + thu muc vao ANH CHUP; workflow goc khong doi.
    const snapshot = withOutDir(withImages(withPromptText(graph, promptText), images), outDir);
    const n = Math.max(1, Math.min(count, 50));
    try {
      for (let i = 0; i < n; i += 1) {
        // Ten kem so thu tu khi xep nhieu ban, de phan biet trong hang cho.
        await enqueue(picked.id, n > 1 ? `${picked.name} (${i + 1}/${n})` : picked.name, snapshot);
      }
      onAdded?.(n > 1 ? `${picked.name} ×${n}` : picked.name);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  }

  async function pickImages(): Promise<void> {
    setError(null);
    try {
      const picked = await pickFiles(true);
      if (picked.length > 0) {
        setImages(picked);
      }
    } catch (e) {
      setError(errorText(e));
    }
  }

  const blocked = graphErrors.length > 0;
  const canAdd = graph !== null && !blocked && !busy && !checking;

  return (
    <Dialog
      title="Thêm workflow vào hàng chờ"
      width={620}
      onClose={onClose}
      footer={
        <DialogFooter
          hint={
            blocked
              ? "Sửa đồ thị trên canvas rồi lưu lại trước khi thêm."
              : missing.length > 0
                ? "Vẫn thêm được — workflow sẽ lỗi ở node thiếu tài khoản."
                : videoTotal === null
                  ? undefined
                  : `Sẽ tạo ${videoTotal} video.`
          }
        >
          <Button onClick={onClose} disabled={busy}>
            Huỷ
          </Button>
          <Button variant="primary" disabled={!canAdd} onClick={() => void handleAdd()}>
            {busy ? "Đang thêm…" : "Thêm vào hàng chờ"}
          </Button>
        </DialogFooter>
      }
    >
      {list === null ? (
        <p className="text-[12.5px] text-ink3">Đang tải danh sách workflow…</p>
      ) : list.length === 0 ? (
        <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2.5 text-[12.5px] leading-[1.5] text-muted">
          Chưa có workflow nào được lưu. Dựng một workflow trên canvas rồi bấm{" "}
          <strong className="text-fg">Lưu</strong> trước đã.
        </p>
      ) : (
        <Field label="Workflow" required>
          <select
            className={INPUT_CLASS}
            value={pickedId}
            onChange={(e) => setPickedId(e.target.value)}
          >
            {list.map((w) => (
              <option key={w.id} value={w.id} className="bg-panel text-fg">
                {w.name}
              </option>
            ))}
          </select>
        </Field>
      )}

      {checking ? <p className="text-[12.5px] text-ink3">Đang kiểm tra đồ thị…</p> : null}

      {blocked ? (
        <div className="flex flex-col gap-1.5 rounded-lg border border-rose/45 bg-rose/[0.08] px-3 py-2.5">
          <span className="text-[13px] font-semibold text-rose">
            Đồ thị chưa chạy được — thiếu dữ liệu đầu vào
          </span>
          <ul className="flex flex-col gap-1 text-[12.5px] leading-[1.5] text-muted">
            {graphErrors.map((msg) => (
              <li key={msg}>• {msg}</li>
            ))}
          </ul>
        </div>
      ) : null}

      {missing.length > 0 ? (
        <div className="flex gap-2.5 rounded-lg border border-primary/40 bg-primary/[0.07] p-3">
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
            aria-hidden="true"
            className="mt-px flex-none text-accent"
          >
            <path d="M12 3 2 20h20L12 3Z" />
            <path d="M12 10v4M12 17v.5" strokeLinecap="round" />
          </svg>
          <div className="flex flex-1 flex-col gap-1.5">
            <span className="text-[13px] font-semibold text-fg">
              Kho chưa có tài khoản cho {missing.length} loại node
            </span>
            <ul className="flex flex-col gap-1 text-[12.5px] leading-[1.5] text-muted">
              {missing.map((m) => (
                <li key={m.nodeKind}>
                  <strong className="text-fg">{m.nodeLabel}</strong>
                  {m.nodeCount > 1 ? ` ×${m.nodeCount}` : ""} cần{" "}
                  <strong className="text-fg">{m.provider}</strong> ({m.kinds.join(" hoặc ")})
                </li>
              ))}
            </ul>
            <span className="text-xs text-ink3">
              Thêm ở tab Tài khoản hoặc API key. Thiếu thì workflow vẫn vào hàng chờ nhưng sẽ lỗi
              đúng ở node đó.
            </span>
          </div>
        </div>
      ) : null}

      {promptCount > 0 ? (
        <Field
          label="Dữ liệu đầu vào"
          hint={
            promptText.trim() === ""
              ? "để trống = dùng prompt đã lưu trong workflow"
              : `${countScenes(promptText)} cảnh · mỗi dòng một cảnh`
          }
        >
          <textarea
            rows={5}
            className={`${INPUT_CLASS} resize-y py-2 font-mono text-[12.5px]`}
            placeholder={"Mỗi dòng một cảnh — dán hoặc nhập danh sách prompt ở đây.\nCô gái mở hộp sản phẩm, ánh sáng cửa sổ\nCận tay cầm sản phẩm, xoay chậm"}
            value={promptText}
            onChange={(e) => setPromptText(e.target.value)}
          />
          <span className="text-xs leading-[1.5] text-ink3">
            Thay prompt của {promptCount > 1 ? `${promptCount} node Prompt` : "node Prompt"} cho
            riêng lần chạy này. Workflow đã lưu không bị đổi.
          </span>
        </Field>
      ) : null}

      {imageCount > 0 ? (
        <Field
          label="Ảnh tham chiếu"
          hint={images.length === 0 ? "để trống = dùng ảnh đã có trong workflow" : `${images.length} ảnh`}
        >
          <div className="flex flex-wrap items-center gap-1.5">
            <Button onClick={() => void pickImages()}>Chọn ảnh…</Button>
            {images.length > 0 ? (
              <Button variant="danger" onClick={() => setImages([])}>
                Bỏ ảnh
              </Button>
            ) : null}
          </div>
          {images.length > 0 ? (
            <div className="flex flex-wrap gap-1.5">
              {images.map((p) => (
                <span
                  key={p}
                  title={p}
                  className="max-w-[160px] truncate rounded-sm bg-fill2 px-2 py-0.5 font-mono text-[11px] text-ink2"
                >
                  {p.split(/[\/]/).pop()}
                </span>
              ))}
            </div>
          ) : null}
          <span className="text-xs leading-[1.5] text-ink3">
            Tiêm vào {imageCount > 1 ? `${imageCount} node Tải ảnh` : "node Tải ảnh"} cho riêng lần
            chạy này. Dùng chung cho mọi cảnh.
          </span>
        </Field>
      ) : null}

      {!blocked && graph !== null ? (
        <Field label="Số lượng" hint="xếp bao nhiêu bản vào hàng chờ">
          <div className="flex flex-wrap items-center gap-2">
            <input
              type="number"
              min={1}
              max={50}
              className={`w-24 ${INPUT_CLASS} text-right font-mono`}
              value={count}
              onChange={(e) => setCount(Math.max(1, Math.min(50, Number(e.target.value) || 1)))}
            />
            <span className="text-xs leading-[1.5] text-ink3">
              Mỗi bản là một lần chạy riêng. AI sinh mỗi lần một khác — chạy nhiều để chọn bản đẹp
              nhất. Hàng chờ chạy lần lượt.
            </span>
          </div>
        </Field>
      ) : null}

      {saveCount > 0 ? (
        <Field
          label="Thư mục lưu"
          hint={
            saveCount > 1
              ? `áp cho cả ${saveCount} node Lưu video; để trống = dùng cài đặt chung`
              : "để trống = dùng cài đặt chung"
          }
        >
          <div className="flex flex-wrap gap-1.5">
            <input
              className={`flex-[1_1_220px] ${INPUT_CLASS} font-mono text-[12.5px]`}
              placeholder={settings?.outputDir === "" ? "downloads" : (settings?.outputDir ?? "")}
              value={outDir}
              onChange={(e) => setOutDir(e.target.value)}
            />
            <Button onClick={() => void pickOutDir()}>Chọn…</Button>
          </div>
          <span className="text-xs leading-[1.5] text-ink3">
            Chỉ áp dụng cho lần thêm này — không đổi cài đặt chung, không đổi workflow đã lưu.
          </span>
        </Field>
      ) : null}

      {graph !== null && !blocked ? (
        <p className="text-xs text-ink3">
          {graph.nodes.length} node
          {videoTotal === null ? "" : ` · ${videoTotal} video`}
          {missing.length === 0 ? " · đủ tài khoản" : ""}
        </p>
      ) : null}

      {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}
    </Dialog>
  );
}
