// Hop thoai "Mở" cua RunBar — liet ke workflow da luu (`workflow_list`), chon mot cai de tai
// (`workflow_get` -> `loadGraph`). Tach rieng file vi RunBar da kha day du nut/trang thai.
import { useEffect, useState } from "react";

import { workflowGet, workflowList } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { WorkflowDoc, WorkflowMeta } from "../ipc/types";

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
  const [error, setError] = useState<string | null>(null);
  const [loadingId, setLoadingId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    workflowList()
      .then((list) => {
        if (!cancelled) {
          setItems(list);
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setError(errorText(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="w-96 max-w-full rounded-lg bg-white p-4 shadow-lg">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold">Mở workflow</h2>
          <button type="button" className="text-slate-400 hover:text-slate-700" onClick={onClose}>
            Đóng
          </button>
        </div>
        {error ? <p className="mb-2 text-xs text-red-600">{error}</p> : null}
        {items === null ? (
          <p className="text-xs text-slate-500">Đang tải…</p>
        ) : items.length === 0 ? (
          <p className="text-xs text-slate-500">Chưa có workflow nào được lưu.</p>
        ) : (
          <ul className="max-h-72 divide-y divide-slate-100 overflow-auto">
            {items.map((meta) => (
              <li key={meta.id}>
                <button
                  type="button"
                  className="flex w-full flex-col items-start px-1 py-2 text-left hover:bg-slate-50 disabled:opacity-50"
                  disabled={loadingId !== null}
                  onClick={() => void handlePick(meta)}
                >
                  <span className="text-xs font-medium text-slate-800">{meta.name}</span>
                  <span className="text-[10px] text-slate-400">{formatUpdatedAt(meta.updatedAt)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
