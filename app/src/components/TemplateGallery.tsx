// Kho template (P4d) — hop thoai gallery: hai tab Public/Private, o tim theo ten/mo ta, luoi the
// (thumbnail neu co, khong thi placeholder ten mau). Moi the co "Dùng mẫu" (nap do thi len canvas);
// the private con co xoa 2-buoc. "Dùng mẫu" lay `template_get` roi goi `loadGraph` (dung duong nhu
// mo mot workflow da luu, xem `RunBar.handlePicked`) — neu canvas dang co node thi hoi truoc.
import { useEffect, useMemo, useState } from "react";
import { confirmDialog } from "../store/dialog";

import { useGraphStore, useGraphStoreApi } from "../store/graph";
import { useTemplatesStore } from "../store/templates";
import { describeTemplateError } from "../format/template";
import type { TemplateMeta, TemplateScope } from "../ipc/types";
import Lightbox from "./Lightbox";

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
  const graphApi = useGraphStoreApi();

  const [scope, setScope] = useState<TemplateScope>("public");
  // Ảnh thumbnail đang phóng to (lightbox) — null = không mở.
  const [zoomImg, setZoomImg] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string>("all");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);

  // Tai lai moi khi doi tab — `items` chi giu MOT pham vi (xem store/templates.ts).
  useEffect(() => {
    void load(scope);
    setConfirmDeleteId(null);
    setCategory("all");
  }, [scope, load]);

  // Danh sach nhom (giu thu tu xuat hien) — dung dung chip loc.
  const categories = useMemo(() => {
    const seen: string[] = [];
    for (const t of items) {
      if (!seen.includes(t.category)) {
        seen.push(t.category);
      }
    }
    return seen;
  }, [items]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items.filter((t) => {
      if (category !== "all" && t.category !== category) {
        return false;
      }
      if (q === "") {
        return true;
      }
      return t.name.toLowerCase().includes(q) || t.description.toLowerCase().includes(q);
    });
  }, [items, query, category]);

  // Gom the theo nhom de hien tieu de nhom (giu thu tu `categories`).
  const groups = useMemo(() => {
    const order = category === "all" ? categories : [category];
    return order
      .map((c) => ({ category: c, rows: filtered.filter((t) => t.category === c) }))
      .filter((g) => g.rows.length > 0);
  }, [filtered, categories, category]);

  async function handleUse(meta: TemplateMeta): Promise<void> {
    setBusyId(meta.id);
    setLocalError(null);
    try {
      const doc = await getDoc(meta.id);
      // Kiem tra TRUC TIEP tu store (khong subscribe node vao component nay cho re-render vo ich).
      const hasNodes = graphApi.getState().nodes.length > 0;
      if (hasNodes && !(await confirmDialog({ title: "Dùng mẫu", message: "Thay đồ thị hiện tại bằng mẫu này?", okText: "Thay", danger: true }))) {
        setBusyId(null);
        return;
      }
      loadGraph(doc.graph);
      // Mẫu CỘNG ĐỒNG có phiên bản -> nhớ nguồn gốc để lúc lưu workflow còn ghi lại được (xem
      // `GraphStoreState.templateOrigin`). `loadGraph` đã tự xoá nguồn gốc cũ nên mẫu dựng sẵn/
      // riêng (không có `version`) không cần làm gì thêm.
      if (meta.scope === "remote" && meta.version !== "") {
        graphApi.getState().setTemplateOrigin({
          sourceId: meta.id,
          sourceVersion: meta.version,
        });
      }
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="flex max-h-[80vh] w-[42rem] max-w-full flex-col rounded-lg border border-line bg-panel p-4 shadow-2xl">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold text-fg">Kho mẫu</h2>
          <button type="button" className="text-ink3 hover:text-fg" onClick={onClose}>
            Đóng
          </button>
        </div>

        <div className="mb-2 flex items-center gap-2">
          <div className="flex rounded border border-line p-0.5 text-xs">
            <button
              type="button"
              className={`rounded px-3 py-1 ${scope === "public" ? "bg-primary text-primary-fg shadow-sm" : "text-muted hover:bg-fill2"}`}
              onClick={() => setScope("public")}
            >
              Công khai
            </button>
            <button
              type="button"
              className={`rounded px-3 py-1 ${scope === "private" ? "bg-primary text-primary-fg shadow-sm" : "text-muted hover:bg-fill2"}`}
              onClick={() => setScope("private")}
            >
              Của tôi
            </button>
            <button
              type="button"
              className={`rounded px-3 py-1 ${scope === "remote" ? "bg-primary text-primary-fg shadow-sm" : "text-muted hover:bg-fill2"}`}
              onClick={() => setScope("remote")}
            >
              Miễn phí
            </button>
          </div>
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Tìm theo tên hoặc mô tả…"
            className="flex-1 rounded border border-line bg-fill px-2 py-1 text-xs text-fg placeholder:text-ink3"
          />
        </div>

        {categories.length > 1 ? (
          <div className="mb-2 flex flex-wrap items-center gap-1">
            <button
              type="button"
              className={`rounded-full border px-2.5 py-0.5 text-[11px] ${category === "all" ? "border-amber-600 bg-primary text-primary-fg shadow-sm" : "border-line text-muted hover:bg-fill2"}`}
              onClick={() => setCategory("all")}
            >
              Tất cả
            </button>
            {categories.map((c) => (
              <button
                key={c}
                type="button"
                className={`rounded-full border px-2.5 py-0.5 text-[11px] ${category === c ? "border-amber-600 bg-primary text-primary-fg shadow-sm" : "border-line text-muted hover:bg-fill2"}`}
                onClick={() => setCategory(c)}
              >
                {c}
              </button>
            ))}
          </div>
        ) : null}

        {error ? <p className="mb-2 text-xs text-rose">{error}</p> : null}

        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <p className="text-xs text-ink3">Đang tải…</p>
          ) : filtered.length === 0 ? (
            <p className="text-xs text-ink3">
              {query.trim() !== "" ? "Không có mẫu nào khớp." : "Chưa có mẫu nào."}
            </p>
          ) : (
            <div className="flex flex-col gap-3">
              {groups.map((group) => (
                <section key={group.category} className="flex flex-col gap-1.5">
                  <h3 className="text-[11px] font-semibold uppercase tracking-wide text-muted">
                    {group.category}
                  </h3>
                  <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3">
                    {group.rows.map((meta) => {
                      const canDelete = meta.scope === "private" && meta.id.startsWith("tpl_");
                      const busy = busyId !== null;
                      return (
                        <li
                          key={meta.id}
                          className="flex flex-col overflow-hidden rounded border border-line bg-fill"
                        >
                    <div className="flex aspect-[9/16] items-center justify-center overflow-hidden bg-black text-center">
                      {meta.thumbnail ? (
                        <img
                          src={meta.thumbnail}
                          alt={meta.name}
                          title="Bấm để xem ảnh lớn"
                          onClick={() => setZoomImg(meta.thumbnail)}
                          className="h-full w-full cursor-zoom-in object-cover"
                        />
                      ) : (
                        <span className="p-2 text-xs font-medium text-muted">{meta.name}</span>
                      )}
                    </div>
                    <div className="flex flex-1 flex-col gap-1 p-2">
                      <span className="text-xs font-medium text-fg">{meta.name}</span>
                      {meta.description ? (
                        <span className="line-clamp-2 text-[10px] text-ink3">
                          {meta.description}
                        </span>
                      ) : null}
                      <div className="mt-auto flex items-center gap-1 pt-1">
                        <button
                          type="button"
                          disabled={busy}
                          className="rounded-lg bg-amber-600 px-2.5 py-1 text-[11px] font-semibold text-primary-fg shadow-sm shadow-amber-600/25 hover:bg-amber-500 hover:shadow-md disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none"
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
                                ? "border-red-500 bg-red-600 text-fg hover:bg-red-500"
                                : "border-rose/35 text-rose hover:bg-rose/10"
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
                </section>
              ))}
            </div>
          )}
        </div>
      </div>
      {zoomImg ? (
        <Lightbox src={zoomImg} kind="image" onClose={() => setZoomImg(null)} />
      ) : null}
    </div>
  );
}
