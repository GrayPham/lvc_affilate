// Kho template (P4d) — hop thoai gallery: hai tab Public/Private, o tim theo ten/mo ta, luoi the
// (thumbnail neu co, khong thi placeholder ten mau). Moi the co "Dùng mẫu" (nap do thi len canvas);
// the private con co xoa 2-buoc. "Dùng mẫu" lay `template_get` roi goi `loadGraph` (dung duong nhu
// mo mot workflow da luu, xem `RunBar.handlePicked`) — neu canvas dang co node thi hoi truoc.
import { useEffect, useMemo, useState } from "react";

import { useGraphStore } from "../store/graph";
import { useTemplatesStore } from "../store/templates";
import { describeTemplateError } from "../format/template";
import type { TemplateMeta, TemplateScope } from "../ipc/types";

interface TemplateGalleryProps {
  onClose: () => void;
  /** Bao cho RunBar hien thong bao "Đã dùng mẫu ..." sau khi nap thanh cong. */
  onApplied?: (name: string) => void;
}

export default function TemplateGallery({ onClose, onApplied }: TemplateGalleryProps) {
  const items = useTemplatesStore((s) => s.items);
  const loading = useTemplatesStore((s) => s.loading);
  const storeError = useTemplatesStore((s) => s.error);
  const load = useTemplatesStore((s) => s.load);
  const getDoc = useTemplatesStore((s) => s.getDoc);
  const remove = useTemplatesStore((s) => s.remove);

  const loadGraph = useGraphStore((s) => s.loadGraph);

  const [scope, setScope] = useState<TemplateScope>("public");
  const [query, setQuery] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);

  // Tai lai moi khi doi tab — `items` chi giu MOT pham vi (xem store/templates.ts).
  useEffect(() => {
    void load(scope);
    setConfirmDeleteId(null);
  }, [scope, load]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (q === "") {
      return items;
    }
    return items.filter(
      (t) => t.name.toLowerCase().includes(q) || t.description.toLowerCase().includes(q),
    );
  }, [items, query]);

  async function handleUse(meta: TemplateMeta): Promise<void> {
    setBusyId(meta.id);
    setLocalError(null);
    try {
      const doc = await getDoc(meta.id);
      // Kiem tra TRUC TIEP tu store (khong subscribe node vao component nay cho re-render vo ich).
      const hasNodes = useGraphStore.getState().nodes.length > 0;
      if (hasNodes && !window.confirm("Thay đồ thị hiện tại?")) {
        setBusyId(null);
        return;
      }
      loadGraph(doc.graph);
      onApplied?.(meta.name);
      onClose();
    } catch (e) {
      setLocalError(describeTemplateError(e));
      setBusyId(null);
    }
  }

  async function handleDelete(meta: TemplateMeta): Promise<void> {
    if (confirmDeleteId !== meta.id) {
      setConfirmDeleteId(meta.id);
      return;
    }
    setBusyId(meta.id);
    setLocalError(null);
    try {
      await remove(meta.id);
      setConfirmDeleteId(null);
    } catch (e) {
      setLocalError(describeTemplateError(e));
    } finally {
      setBusyId(null);
    }
  }

  const error = localError ?? storeError;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4">
      <div className="flex max-h-[80vh] w-[42rem] max-w-full flex-col rounded-lg bg-white p-4 shadow-lg">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold">Kho mẫu</h2>
          <button type="button" className="text-slate-400 hover:text-slate-700" onClick={onClose}>
            Đóng
          </button>
        </div>

        <div className="mb-2 flex items-center gap-2">
          <div className="flex rounded border border-slate-200 p-0.5 text-xs">
            <button
              type="button"
              className={`rounded px-3 py-1 ${scope === "public" ? "bg-slate-900 text-white" : "text-slate-600 hover:bg-slate-100"}`}
              onClick={() => setScope("public")}
            >
              Công khai
            </button>
            <button
              type="button"
              className={`rounded px-3 py-1 ${scope === "private" ? "bg-slate-900 text-white" : "text-slate-600 hover:bg-slate-100"}`}
              onClick={() => setScope("private")}
            >
              Của tôi
            </button>
          </div>
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Tìm theo tên hoặc mô tả…"
            className="flex-1 rounded border border-slate-300 px-2 py-1 text-xs"
          />
        </div>

        {error ? <p className="mb-2 text-xs text-red-600">{error}</p> : null}

        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <p className="text-xs text-slate-500">Đang tải…</p>
          ) : filtered.length === 0 ? (
            <p className="text-xs text-slate-500">
              {query.trim() !== "" ? "Không có mẫu nào khớp." : "Chưa có mẫu nào."}
            </p>
          ) : (
            <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3">
              {filtered.map((meta) => {
                const canDelete = meta.scope === "private" && meta.id.startsWith("tpl_");
                const busy = busyId !== null;
                return (
                  <li
                    key={meta.id}
                    className="flex flex-col overflow-hidden rounded border border-slate-200"
                  >
                    <div className="flex aspect-video items-center justify-center bg-slate-50 p-2 text-center">
                      {meta.thumbnail ? (
                        <img
                          src={meta.thumbnail}
                          alt={meta.name}
                          className="h-full w-full object-cover"
                        />
                      ) : (
                        <span className="text-xs font-medium text-slate-400">{meta.name}</span>
                      )}
                    </div>
                    <div className="flex flex-1 flex-col gap-1 p-2">
                      <span className="text-xs font-medium text-slate-800">{meta.name}</span>
                      {meta.description ? (
                        <span className="line-clamp-2 text-[10px] text-slate-500">
                          {meta.description}
                        </span>
                      ) : null}
                      <div className="mt-auto flex items-center gap-1 pt-1">
                        <button
                          type="button"
                          disabled={busy}
                          className="rounded bg-slate-900 px-2 py-1 text-[11px] text-white hover:bg-slate-700 disabled:opacity-50"
                          onClick={() => void handleUse(meta)}
                        >
                          Dùng mẫu
                        </button>
                        {canDelete ? (
                          <button
                            type="button"
                            disabled={busy}
                            className={`rounded border px-2 py-1 text-[11px] disabled:opacity-50 ${
                              confirmDeleteId === meta.id
                                ? "border-red-500 bg-red-600 text-white hover:bg-red-500"
                                : "border-red-300 text-red-700 hover:bg-red-50"
                            }`}
                            onClick={() => void handleDelete(meta)}
                          >
                            {confirmDeleteId === meta.id ? "Xác nhận xoá?" : "Xoá"}
                          </button>
                        ) : null}
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
