// Hop thoai "Thêm hàng loạt" — dung theo `references/lvc-ui-design/screens/BulkAdd.dc.html`.
//
// Chon nhieu workflow mot luot, moi dong hien trang thai THAT (do thi hop le chua, du tai khoan
// chua), roi them het vao hang cho theo dung thu tu da chon.
//
// Hai nut o cuoi co y nghia khac nhau that su:
// - "Thêm N sẵn sàng": bo qua dong thieu/hong — dung khi muon chay dem ma khong canh.
// - "Thêm cả N": them het, dong thieu tai khoan se LOI dung o node do khi toi luot. Nut nay KHONG
//   them dong co do thi HONG: engine tu choi ngay luc them (`queue_add` kiem truoc khi luu), nen
//   bam vao chi nhan mot loi vo nghia.
import { useEffect, useMemo, useState } from "react";

import { nodeCatalog, workflowGet, workflowList, workflowValidate } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { missingCredentials } from "../graph/credentials";
import type { MissingCredential } from "../graph/credentials";
import { useCredentialsStore } from "../store/credentials";
import { useQueueStore } from "../store/queue";
import { Button, Dialog, DialogFooter } from "../ui/primitives";
import type { Graph, NodeManifest, WorkflowMeta } from "../ipc/types";

/** Mot dong trong danh sach, kem ket qua kiem tra. */
interface Row {
  meta: WorkflowMeta;
  graph: Graph | null;
  /** `true` khi `workflow_validate` bac do thi. */
  broken: boolean;
  missing: MissingCredential[];
  checked: boolean;
}

/** Nhan nhom node cho the mau — dung `category` cua manifest (video/image/script). */
const CATEGORY_LABEL: Record<string, string> = {
  video: "Video",
  image: "Hình ảnh",
  script: "Kịch bản",
};

const CATEGORY_CLASS: Record<string, string> = {
  video: "text-tag-video border-tag-video/35 bg-tag-video/[0.09]",
  image: "text-tag-image border-tag-image/40 bg-tag-image/[0.09]",
  script: "text-tag-script border-tag-script/38 bg-tag-script/[0.10]",
};

/**
 * Cac nhom node co mat trong do thi (video/image/script), giu thu tu co dinh de the mau khong nhay.
 * Export de test: day la thong tin duy nhat moi dong hien ve "workflow nay lam gi".
 */
export function graphCategories(
  graph: Graph,
  catalog: readonly NodeManifest[],
): string[] {
  const byKind = new Map(catalog.map((m) => [m.kind, m.category]));
  const found = new Set<string>();
  for (const n of graph.nodes) {
    const cat = byKind.get(n.kind.kind);
    if (cat !== undefined && cat in CATEGORY_LABEL) {
      found.add(cat);
    }
  }
  return ["video", "image", "script"].filter((c) => found.has(c));
}

interface BulkAddDialogProps {
  onClose: () => void;
  /** Goi sau khi them xong, kem so workflow da them. */
  onAdded?: (count: number) => void;
}

export default function BulkAddDialog({ onClose, onAdded }: BulkAddDialogProps) {
  const enqueue = useQueueStore((s) => s.add);
  const credentials = useCredentialsStore((s) => s.rows);
  const loadCredentials = useCredentialsStore((s) => s.load);

  const [catalog, setCatalog] = useState<NodeManifest[]>([]);
  const [rows, setRows] = useState<Row[] | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void loadCredentials();
    nodeCatalog()
      .then(setCatalog)
      .catch(() => setCatalog([]));
  }, [loadCredentials]);

  // Nap danh sach roi kiem TUNG workflow. Kiem lan luot (khong song song) de khong ban hang chuc
  // lenh IPC cung luc lam treo giao dien khi nguoi dung co nhieu workflow.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await workflowList();
        if (cancelled) {
          return;
        }
        setRows(list.map((meta) => ({ meta, graph: null, broken: false, missing: [], checked: false })));
        setPicked(new Set(list.map((m) => m.id)));

        for (const meta of list) {
          if (cancelled) {
            return;
          }
          let graph: Graph | null = null;
          let broken = false;
          try {
            const doc = await workflowGet(meta.id);
            graph = doc.graph;
            try {
              await workflowValidate(doc.graph);
            } catch {
              broken = true;
            }
          } catch {
            broken = true;
          }
          if (cancelled) {
            return;
          }
          setRows((prev) =>
            (prev ?? []).map((r) =>
              r.meta.id === meta.id ? { ...r, graph, broken, checked: true } : r,
            ),
          );
        }
      } catch (e) {
        if (!cancelled) {
          setError(errorText(e));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Tinh "thieu tai khoan" o day (khong trong vong kiem tra tren) de doi credential/catalog la
  // danh sach cap nhat ngay, khong phai mo lai hop thoai.
  const withMissing = useMemo(
    () =>
      (rows ?? []).map((r) => ({
        ...r,
        missing: r.graph ? missingCredentials(r.graph, catalog, credentials) : [],
      })),
    [rows, catalog, credentials],
  );

  const pickedRows = withMissing.filter((r) => picked.has(r.meta.id));
  const checking = withMissing.some((r) => !r.checked);
  const ready = pickedRows.filter((r) => !r.broken && r.missing.length === 0);
  const brokenCount = pickedRows.filter((r) => r.broken).length;
  const missingCount = pickedRows.filter((r) => !r.broken && r.missing.length > 0).length;
  /** "Thêm cả": moi dong TRU dong co do thi hong (engine se tu choi). */
  const addable = pickedRows.filter((r) => !r.broken);

  function toggle(id: string): void {
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  async function addAll(list: readonly Row[]): Promise<void> {
    setBusy(true);
    setError(null);
    let done = 0;
    for (const r of list) {
      if (!r.graph) {
        continue;
      }
      setProgress(`Đang thêm ${done + 1}/${list.length}…`);
      try {
        await enqueue(r.meta.id, r.meta.name, r.graph);
        done += 1;
      } catch (e) {
        // Mot dong loi KHONG duoc dung ca lo — bao o cuoi, cac dong con lai van vao hang cho.
        setError(`"${r.meta.name}": ${errorText(e)}`);
      }
    }
    setProgress(null);
    setBusy(false);
    onAdded?.(done);
    if (done === list.length) {
      onClose();
    }
  }

  const uniqueMissing = useMemo(() => {
    const byProvider = new Map<string, MissingCredential>();
    for (const r of pickedRows) {
      for (const m of r.missing) {
        byProvider.set(`${m.provider}/${m.kinds.join(",")}`, m);
      }
    }
    return [...byProvider.values()];
  }, [pickedRows]);

  return (
    <Dialog
      title={`Thêm hàng loạt · ${picked.size} workflow`}
      width={620}
      onClose={onClose}
      footer={
        <DialogFooter
          hint={
            missingCount > 0
              ? `${missingCount} workflow thiếu tài khoản sẽ lỗi ở đúng node đó khi tới lượt.`
              : brokenCount > 0
                ? `${brokenCount} workflow có đồ thị chưa chạy được — không thêm được.`
                : undefined
          }
        >
          <Button
            disabled={busy || checking || addable.length === 0 || missingCount === 0}
            onClick={() => void addAll(addable)}
          >
            Thêm cả {addable.length}
          </Button>
          <Button
            variant="primary"
            disabled={busy || checking || ready.length === 0}
            onClick={() => void addAll(ready)}
          >
            {busy ? (progress ?? "Đang thêm…") : `Thêm ${ready.length} sẵn sàng`}
          </Button>
        </DialogFooter>
      }
    >
      {uniqueMissing.length > 0 ? (
        <div role="alert" className="flex gap-2.5 rounded-xl border border-rose/45 bg-rose/[0.08] p-3.5">
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
            aria-hidden="true"
            className="mt-px flex-none text-rose"
          >
            <path d="M12 3 2 20h20L12 3Z" />
            <path d="M12 10v4M12 17v.5" strokeLinecap="round" />
          </svg>
          <div className="flex flex-1 flex-col gap-1.5">
            <span className="text-[13.5px] font-bold text-rose">
              {missingCount} workflow cần tài khoản mà kho chưa có
            </span>
            <ul className="flex flex-col gap-1 text-[12.5px] leading-[1.5] text-ink2">
              {uniqueMissing.map((m) => (
                <li key={`${m.provider}-${m.kinds.join(",")}`}>
                  <strong className="text-ink">{m.provider}</strong> ({m.kinds.join(" hoặc ")}) —
                  cần cho {m.nodeLabel}
                </li>
              ))}
            </ul>
            <span className="text-xs text-ink3">
              Thêm ở tab Tài khoản hoặc API key rồi mở lại hộp thoại này.
            </span>
          </div>
        </div>
      ) : null}

      <h3 className="flex items-center gap-2 text-[11px] font-semibold tracking-[0.1em] text-ink3 uppercase">
        Workflow đã chọn
        <span className="tracking-normal normal-case text-accent-text">
          {checking
            ? "đang kiểm tra…"
            : `${ready.length} sẵn sàng${missingCount > 0 ? ` · ${missingCount} thiếu tài khoản` : ""}${brokenCount > 0 ? ` · ${brokenCount} đồ thị lỗi` : ""}`}
        </span>
        <span className="h-px flex-1 bg-line" />
      </h3>

      {rows === null ? (
        <p className="text-[12.5px] text-ink3">Đang tải danh sách workflow…</p>
      ) : rows.length === 0 ? (
        <p className="text-[12.5px] text-ink3">Chưa có workflow nào được lưu.</p>
      ) : (
        <div className="max-h-[340px] overflow-auto rounded-xl border border-line">
          {withMissing.map((r, i) => {
            const on = picked.has(r.meta.id);
            const status = r.broken
              ? { text: "Đồ thị lỗi", cls: "text-rose" }
              : r.missing.length > 0
                ? { text: "Thiếu tài khoản", cls: "text-rose" }
                : { text: "Sẵn sàng", cls: "text-ok" };
            const cats = r.graph ? graphCategories(r.graph, catalog) : [];
            return (
              <label
                key={r.meta.id}
                className={
                  "flex items-center gap-2.5 border-b border-line px-3 py-[11px] last:border-b-0 " +
                  (r.broken || r.missing.length > 0 ? "bg-rose/[0.04]" : "")
                }
              >
                <input
                  type="checkbox"
                  className="h-4 w-4 flex-none accent-accent"
                  checked={on}
                  disabled={busy}
                  onChange={() => toggle(r.meta.id)}
                  aria-label={`Chọn ${r.meta.name}`}
                />
                <span className="inline-grid h-6 w-6 flex-none place-items-center rounded-sm bg-fill2 text-[11.5px] font-bold text-ink2">
                  {i + 1}
                </span>
                <span className="flex min-w-0 flex-1 flex-col gap-1.5">
                  <span className="truncate text-[13px] font-semibold text-ink">{r.meta.name}</span>
                  <span className="flex flex-wrap gap-1">
                    {cats.map((c) => (
                      <span
                        key={c}
                        className={
                          "rounded-sm border px-[7px] py-px text-[11px] font-semibold " +
                          CATEGORY_CLASS[c]
                        }
                      >
                        {CATEGORY_LABEL[c]}
                      </span>
                    ))}
                  </span>
                </span>
                <span className="text-right whitespace-nowrap">
                  <span className={"block text-[12.5px] font-semibold " + status.cls}>
                    {r.checked ? status.text : "Đang kiểm…"}
                  </span>
                  <span className="block text-[11.5px] text-ink3">
                    {r.missing.length > 0
                      ? r.missing.map((m) => m.provider).join(", ")
                      : r.graph
                        ? `${r.graph.nodes.length} node`
                        : ""}
                  </span>
                </span>
              </label>
            );
          })}
        </div>
      )}

      {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}
    </Dialog>
  );
}
