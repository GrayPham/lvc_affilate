// Bang canh (phai) cua tab "Kịch bản" (P3c Task 2) — toolbar (Cố định style / + Dòng / Lưu cảnh /
// Chạy tất cả (chưa xong) / Dừng tất cả / Đặt lại trạng thái) + cac dong `SceneRow`. Doc THANG
// `store/scripts.ts` (giong `TaskTable` doc thang `store/tasks.ts`) — `ScriptsPage` chi ghep
// `ScriptList` + `SceneTable` canh nhau, khong truyen props qua lai.
import { Fragment } from "react";

import { useScriptsStore } from "../store/scripts";
import SceneRow from "./SceneRow";
import { scriptErrorLabel } from "../format/scene";

const BTN = "rounded border px-2 py-1 text-xs disabled:opacity-50";
const BTN_NEUTRAL = `${BTN} border-line2 hover:bg-fill2`;
const BTN_PRIMARY = `${BTN} border-line2 bg-panel text-fg hover:bg-fill2`;

const COLS = 9;

export default function SceneTable() {
  const doc = useScriptsStore((s) => s.doc);
  const draft = useScriptsStore((s) => s.draft);
  const dirty = useScriptsStore((s) => s.dirty);
  const loadingDoc = useScriptsStore((s) => s.loadingDoc);
  const style = useScriptsStore((s) => s.style);
  const busy = useScriptsStore((s) => s.busy);
  const error = useScriptsStore((s) => s.error);
  const lastRunId = useScriptsStore((s) => s.lastRunId);
  const lastReport = useScriptsStore((s) => s.lastReport);
  const pendingIds = useScriptsStore((s) => s.pendingIds);
  const selectedIds = useScriptsStore((s) => s.selectedIds);

  const setStyle = useScriptsStore((s) => s.setStyle);
  const addRow = useScriptsStore((s) => s.addRow);
  const removeRow = useScriptsStore((s) => s.removeRow);
  const setDraftField = useScriptsStore((s) => s.setDraftField);
  const saveScenes = useScriptsStore((s) => s.saveScenes);
  const runScene = useScriptsStore((s) => s.runScene);
  const runAllPending = useScriptsStore((s) => s.runAllPending);
  const stopScene = useScriptsStore((s) => s.stopScene);
  const stopAll = useScriptsStore((s) => s.stopAll);
  const openSceneFile = useScriptsStore((s) => s.openSceneFile);
  const resetSelectedOrFailed = useScriptsStore((s) => s.resetSelectedOrFailed);
  const toggleSelected = useScriptsStore((s) => s.toggleSelected);

  if (loadingDoc) {
    return <p className="p-4 text-xs text-muted">Đang tải kịch bản…</p>;
  }
  if (!doc) {
    return (
      <p className="flex-1 p-4 text-xs text-muted">
        Chọn một kịch bản ở bên trái để xem các cảnh.
      </p>
    );
  }

  const errorsBySceneId = new Map((lastReport?.errors ?? []).map((e) => [e.sceneId, e.code]));

  return (
    <div className="flex flex-1 flex-col gap-2 overflow-auto p-2">
      <div className="flex flex-wrap items-center gap-2 border-b border-line2 pb-2">
        <h2 className="text-sm font-semibold text-fg">{doc.meta.name}</h2>

        <label className="flex items-center gap-1 text-xs text-ink3">
          Cố định style
          <input
            type="text"
            className="w-40 rounded border border-line2 px-1.5 py-0.5 text-xs"
            value={style}
            onChange={(e) => setStyle(e.target.value)}
            placeholder="vd Cinematic"
          />
        </label>

        <button type="button" disabled={busy} className={BTN_NEUTRAL} onClick={() => addRow()}>
          + Dòng
        </button>
        <button
          type="button"
          disabled={busy || !dirty}
          className={BTN_PRIMARY}
          onClick={() => void saveScenes()}
        >
          Lưu cảnh
        </button>
        <button type="button" disabled={busy} className={BTN_NEUTRAL} onClick={() => void runAllPending()}>
          Chạy tất cả (chưa xong)
        </button>
        <button
          type="button"
          disabled={busy || lastRunId === null}
          className={BTN_NEUTRAL}
          onClick={() => void stopAll()}
        >
          Dừng tất cả
        </button>
        <button
          type="button"
          disabled={busy}
          className={BTN_NEUTRAL}
          onClick={() => void resetSelectedOrFailed()}
        >
          Đặt lại trạng thái
        </button>

        {dirty ? <span className="text-[11px] text-amber-600">Có thay đổi chưa lưu</span> : null}
      </div>

      {error ? <p className="text-xs text-red-600">{error}</p> : null}

      {draft.length === 0 ? (
        <p className="p-4 text-center text-xs text-muted">
          Chưa có cảnh nào — bấm "+ Dòng" hoặc "Nhập TXT…" ở danh sách kịch bản.
        </p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-line2 text-[10px] uppercase tracking-wide text-muted">
              <th className="px-2 py-1 font-medium">#</th>
              <th className="px-2 py-1 font-medium">Prompt</th>
              <th className="px-2 py-1 font-medium">Tỉ lệ</th>
              <th className="px-2 py-1 font-medium">Thời lượng</th>
              <th className="px-2 py-1 font-medium">Model</th>
              <th className="px-2 py-1 font-medium">Mode</th>
              <th className="px-2 py-1 font-medium">Tài khoản</th>
              <th className="px-2 py-1 font-medium">Trạng thái</th>
              <th className="px-2 py-1 font-medium">Thao tác</th>
            </tr>
          </thead>
          <tbody>
            {draft.map((d, i) => {
              const docScene = doc.scenes.find((s) => s.id === d.id);
              const errCode = docScene ? errorsBySceneId.get(docScene.id) : undefined;
              const taskId = docScene?.taskId ?? null;
              return (
                <Fragment key={d.key}>
                  <SceneRow
                    index={i + 1}
                    draft={d}
                    docScene={docScene}
                    pendingStop={taskId !== null && pendingIds.has(taskId)}
                    pendingOpen={taskId !== null && pendingIds.has(taskId)}
                    disabled={busy}
                    selected={docScene !== undefined && selectedIds.has(docScene.id)}
                    onToggleSelect={() => docScene && toggleSelected(docScene.id)}
                    onPromptChange={(prompt) => setDraftField(d.key, { prompt })}
                    onFieldChange={(patch) => setDraftField(d.key, patch)}
                    onRun={() => void runScene(d.key)}
                    onStop={() => taskId !== null && void stopScene(taskId)}
                    onOpenFile={() => taskId !== null && void openSceneFile(taskId)}
                    onRemove={() => removeRow(d.key)}
                  />
                  {errCode ? (
                    <tr>
                      <td colSpan={COLS} className="px-2 pb-1.5 text-[11px] text-red-600">
                        Lỗi lần chạy gần nhất: {scriptErrorLabel(errCode)}
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
