// "Workflow của tôi" — kho workflow da luu (`workflow_list`), chon mot cai de mo
// (`workflow_get` -> `loadGraph`).
//
// Dang LUOI THE giong Kho mau / Kho kich ban (khong phai danh sach doc hep nhu truoc): ca ba kho
// trong app dung chung mot phong cach de nguoi dung khong phai hoc lai cach doc o moi man.
//
// Day cung la noi thay workflow nao co ban moi tren kho cong dong: the co ban cap nhat dang cho se
// co nhan "↑ 1.0.1" bam duoc, mo `UpdateWorkflowDialog`.
import { useCallback, useEffect, useMemo, useState } from "react";

import { workflowDelete, workflowGet, workflowList, workflowUpdateList } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import ConfirmDialog from "./ConfirmDialog";
import UpdateWorkflowDialog from "./UpdateWorkflowDialog";
import { Button, SearchInput } from "../ui/primitives";
import type { WorkflowDoc, WorkflowMeta, WorkflowUpdate } from "../ipc/types";

interface WorkflowListModalProps {
  onClose: () => void;
  onPicked: (doc: WorkflowDoc) => void;
}

function formatUpdatedAt(unixSeconds: number): string {
  try {
    return new Date(unixSeconds * 1000).toLocaleString("vi-VN");
  } catch {
    return String(unixSeconds);
  }
}

export default function WorkflowListModal({ onClose, onPicked }: WorkflowListModalProps) {
  const [items, setItems] = useState<WorkflowMeta[] | null>(null);
  const [updates, setUpdates] = useState<WorkflowUpdate[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loadingId, setLoadingId] = useState<string | null>(null);
  const [showUpdate, setShowUpdate] = useState<WorkflowUpdate | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<WorkflowMeta | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async (): Promise<void> => {
    setItems(await workflowList());
    // Khong co ban cap nhat nao la trang thai THUONG GAP nhat — loi o day (vd chua tung nap cache
    // kho cong dong) khong duoc chan danh sach workflow hien ra.
    try {
      setUpdates(await workflowUpdateList());
    } catch {
      setUpdates([]);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    refresh().catch((e: unknown) => {
      if (!cancelled) {
        setError(errorText(e));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  const updateByWorkflow = useMemo(
    () => new Map(updates.map((u) => [u.workflowId, u])),
    [updates],
  );

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (q === "") {
      return items ?? [];
    }
    return (items ?? []).filter((m) => m.name.toLowerCase().includes(q));
  }, [items, query]);

  async function handlePick(meta: WorkflowMeta): Promise<void> {
    setLoadingId(meta.id);
    setError(null);
    try {
      const doc = await workflowGet(meta.id);
      onPicked(doc);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoadingId(null);
    }
  }

  async function handleDelete(meta: WorkflowMeta): Promise<void> {
    setConfirmDelete(null);
    try {
      await workflowDelete(meta.id);
      setMessage(`Đã xoá "${meta.name}".`);
      await refresh();
    } catch (e) {
      setError(errorText(e));
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center overflow-auto bg-black/60 p-8"
      role="dialog"
      aria-modal="true"
      aria-labelledby="wfLibTitle"
    >
      <div className="flex max-h-full w-[760px] max-w-full flex-col overflow-hidden rounded-2xl border border-line2 bg-panel shadow-[var(--dialog-shadow)]">
        <header className="flex flex-wrap items-center gap-2 border-b border-line px-5 py-4">
          <div className="min-w-[200px] flex-1">
            <h2 id="wfLibTitle" className="text-[16px] font-bold text-ink">
              Workflow của tôi
            </h2>
            <p className="mt-0.5 text-[12.5px] text-ink3">
              {items === null ? "Đang tải…" : `${items.length} workflow đã lưu`}
            </p>
          </div>
          <button
            type="button"
            aria-label="Đóng"
            onClick={onClose}
            className="grid h-9 w-9 place-items-center rounded-[9px] text-[16px] text-ink3 hover:text-ink"
          >
            ✕
          </button>
        </header>

        <div className="flex flex-col gap-3.5 overflow-auto px-5 py-4">
          <div className="flex">
            <SearchInput
              value={query}
              onChange={setQuery}
              label="Tìm trong Workflow của tôi"
              placeholder="Tìm theo tên workflow…"
            />
          </div>

          {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}
          {message ? <p className="text-[12.5px] text-ok">{message}</p> : null}

          {items === null ? (
            <p className="text-[12.5px] text-ink3">Đang tải…</p>
          ) : visible.length === 0 ? (
            <p className="text-[12.5px] leading-[1.5] text-ink3">
              {items.length === 0
                ? "Chưa lưu workflow nào. Dựng trên canvas rồi bấm Lưu (hoặc Ctrl+S) để cất vào đây."
                : "Không có workflow nào khớp."}
            </p>
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3.5">
              {visible.map((meta) => {
                const update = updateByWorkflow.get(meta.id);
                return (
                  <div
                    key={meta.id}
                    className="flex flex-col gap-2 rounded-xl border border-line bg-fill p-3"
                  >
                    <div className="flex items-start gap-2">
                      <span className="min-w-0 flex-1 truncate text-[13.5px] font-semibold text-ink">
                        {meta.name}
                      </span>
                      {update ? (
                        <button
                          type="button"
                          title={`Kho có bản ${update.newVersion} (đang dùng ${update.currentVersion})`}
                          onClick={() => setShowUpdate(update)}
                          className="shrink-0 rounded-sm border border-accent/40 bg-accent/10 px-1.5 py-0.5 text-[10.5px] font-bold text-accent-text hover:bg-accent/20"
                        >
                          ↑ {update.newVersion}
                        </button>
                      ) : null}
                    </div>
                    <span className="text-[11.5px] text-ink3">
                      Sửa lần cuối {formatUpdatedAt(meta.updatedAt)}
                    </span>
                    <div className="mt-auto flex gap-1.5 pt-1">
                      <Button
                        small
                        variant="primary"
                        disabled={loadingId !== null}
                        onClick={() => void handlePick(meta)}
                      >
                        {loadingId === meta.id ? "Đang mở…" : "Mở"}
                      </Button>
                      <Button
                        small
                        variant="danger"
                        disabled={loadingId !== null}
                        onClick={() => setConfirmDelete(meta)}
                      >
                        Xoá
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {showUpdate ? (
        <UpdateWorkflowDialog
          update={showUpdate}
          onClose={() => setShowUpdate(null)}
          onApplied={(_id, mode) => {
            setMessage(
              mode === "keep_both"
                ? `Đã thêm bản ${showUpdate.newVersion} thành workflow mới.`
                : `Đã cập nhật lên bản ${showUpdate.newVersion}.`,
            );
            void refresh();
          }}
        />
      ) : null}

      {confirmDelete ? (
        <ConfirmDialog
          title="Xoá workflow?"
          body={`"${confirmDelete.name}" sẽ bị xoá khỏi Workflow của tôi.`}
          note="Video đã tạo và các mục đang trong hàng chờ KHÔNG bị ảnh hưởng — hàng chờ giữ bản chụp riêng của đồ thị."
          okLabel="Xoá"
          onCancel={() => setConfirmDelete(null)}
          onConfirm={() => void handleDelete(confirmDelete)}
        />
      ) : null}
    </div>
  );
}
