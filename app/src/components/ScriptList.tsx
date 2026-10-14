// Danh sach kich ban (trai) cua tab "Kịch bản" (P3c Task 2) — tao moi (hoi ten qua `window.
// prompt`, giong quy uoc don gian cua `WorkflowListModal`), doi ten/thu muc (sua inline), xoa
// (xac nhan 2 buoc, giong `AccountRow`), va "Nhập TXT…" (modal dan noi dung TXT -> thay TOAN BO
// canh cua kich ban DANG MO — vo hieu khi chua chon kich ban nao). Doc THANG `store/scripts.ts`
// (giong `TaskTable` doc thang `store/tasks.ts`).
import { useState } from "react";
import { promptDialog } from "../store/dialog";

import { useScriptsStore } from "../store/scripts";
import { pickDirectory } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { ScriptMeta } from "../ipc/types";

const BTN = "rounded border px-2 py-1 text-xs disabled:opacity-50";
const BTN_NEUTRAL = `${BTN} border-line2 hover:bg-fill2`;
const BTN_PRIMARY = `${BTN} border-line2 bg-panel text-ink hover:bg-fill2`;

function CountsLine({ meta }: { meta: ScriptMeta }) {
  return (
    <span className="block text-[10px] text-ink2">
      {meta.total} tổng · {meta.done} xong · {meta.failed} lỗi · {meta.running} đang chạy
    </span>
  );
}

interface EditFormState {
  name: string;
  dir: string;
}

/** Modal "Nhập TXT…" (P3c Task 2 brief) — dan noi dung TXT (moi dong mot canh, hint sau `|`) ->
 * `scriptImportTxt` -> thay TOAN BO canh cua kich ban `scriptId`. */
function ImportTxtModal({
  scriptName,
  onClose,
  onImport,
}: {
  scriptName: string;
  onClose: () => void;
  onImport: (text: string) => Promise<number | null>;
}) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<number | null>(null);

  async function handleImport(): Promise<void> {
    setBusy(true);
    try {
      const n = await onImport(text);
      setResult(n);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="w-[32rem] max-w-full rounded-lg bg-panel p-4 shadow-lg">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold">Nhập TXT vào "{scriptName}"</h2>
          <button type="button" className="text-ink2 hover:text-ink" onClick={onClose}>
            Đóng
          </button>
        </div>

        {result !== null ? (
          <div className="flex flex-col gap-3">
            <p className="text-sm text-emerald-700">Đã thay toàn bộ cảnh bằng {result} cảnh mới.</p>
            <button
              type="button"
              className="self-end rounded bg-panel px-3 py-1 text-xs text-ink hover:bg-fill2"
              onClick={onClose}
            >
              Đóng
            </button>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <label className="flex flex-col gap-1 text-xs">
              Dán nội dung TXT, mỗi dòng một cảnh, hint sau "|"
              <textarea
                className="h-40 resize-none rounded border border-line2 px-2 py-1 font-mono text-xs"
                value={text}
                disabled={busy}
                onChange={(e) => setText(e.target.value)}
                placeholder={"cảnh 1\ncảnh 2 | ratio=16:9 | dur=10"}
              />
            </label>
            <p className="text-[11px] text-amber-600">
              Sẽ thay TOÀN BỘ cảnh hiện có của kịch bản này bằng nội dung dán ở trên.
            </p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                disabled={busy}
                className="rounded border border-line2 px-3 py-1 text-xs hover:bg-fill2 disabled:opacity-50"
                onClick={onClose}
              >
                Huỷ
              </button>
              <button
                type="button"
                disabled={busy || text.trim() === ""}
                className="rounded bg-panel px-3 py-1 text-xs text-ink hover:bg-fill2 disabled:opacity-50"
                onClick={() => void handleImport()}
              >
                {busy ? "Đang nhập…" : "Nhập"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default function ScriptList() {
  const scripts = useScriptsStore((s) => s.scripts);
  const loadingList = useScriptsStore((s) => s.loadingList);
  const listError = useScriptsStore((s) => s.listError);
  const selectedScriptId = useScriptsStore((s) => s.selectedScriptId);
  const busy = useScriptsStore((s) => s.busy);
  const error = useScriptsStore((s) => s.error);
  const selectScript = useScriptsStore((s) => s.selectScript);
  const createScript = useScriptsStore((s) => s.createScript);
  const renameScript = useScriptsStore((s) => s.renameScript);
  const deleteScript = useScriptsStore((s) => s.deleteScript);
  const importTxt = useScriptsStore((s) => s.importTxt);

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editForm, setEditForm] = useState<EditFormState>({ name: "", dir: "" });
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null);
  const [showImport, setShowImport] = useState(false);
  const [pickDirError, setPickDirError] = useState<string | null>(null);

  const selectedMeta = scripts.find((s) => s.id === selectedScriptId) ?? null;

  function startEdit(meta: ScriptMeta): void {
    setEditingId(meta.id);
    setEditForm({ name: meta.name, dir: meta.dir });
    setPickDirError(null);
  }

  async function saveEdit(id: string): Promise<void> {
    const ok = await renameScript(id, editForm.name, editForm.dir);
    if (ok) {
      setEditingId(null);
    }
  }

  async function pickEditDir(): Promise<void> {
    setPickDirError(null);
    try {
      const dir = await pickDirectory();
      if (dir !== null) {
        setEditForm((f) => ({ ...f, dir }));
      }
    } catch (e) {
      setPickDirError(errorText(e));
    }
  }

  async function handleCreate(): Promise<void> {
    const name = await promptDialog({ title: "Kịch bản mới", message: "Tên kịch bản:", defaultValue: "", okText: "Tạo" });
    if (name === null) {
      return;
    }
    await createScript(name, "");
  }

  async function handleDeleteConfirmed(id: string): Promise<void> {
    const ok = await deleteScript(id);
    if (ok) {
      setConfirmingDeleteId(null);
    }
  }

  return (
    <div className="flex w-72 shrink-0 flex-col gap-2 border-r border-line2 p-2">
      <div className="flex flex-wrap items-center gap-1">
        <h2 className="mr-auto text-sm font-semibold text-ink">Kịch bản</h2>
        <button type="button" disabled={busy} className={BTN_PRIMARY} onClick={() => void handleCreate()}>
          Mới
        </button>
        <button
          type="button"
          disabled={busy || selectedScriptId === null}
          className={BTN_NEUTRAL}
          title={selectedScriptId === null ? "Chọn một kịch bản trước" : undefined}
          onClick={() => setShowImport(true)}
        >
          Nhập TXT…
        </button>
      </div>

      {loadingList ? <p className="text-[11px] text-ink2">Đang tải…</p> : null}
      {listError ? <p className="text-xs text-red-600">{listError}</p> : null}
      {error ? <p className="text-xs text-red-600">{error}</p> : null}

      {scripts.length === 0 && !loadingList ? (
        <p className="text-xs text-ink2">Chưa có kịch bản nào — bấm "Mới" để tạo.</p>
      ) : (
        <ul className="flex flex-1 flex-col gap-1 overflow-auto">
          {scripts.map((meta) => (
            <li
              key={meta.id}
              className={
                "rounded border " +
                (meta.id === selectedScriptId
                  ? "border-line2 bg-fill2"
                  : "border-line2 hover:bg-fill2")
              }
            >
              {editingId === meta.id ? (
                <div className="flex flex-col gap-1 p-1.5">
                  <input
                    className="rounded border border-line2 px-1.5 py-0.5 text-xs"
                    value={editForm.name}
                    onChange={(e) => setEditForm((f) => ({ ...f, name: e.target.value }))}
                    placeholder="Tên kịch bản"
                  />
                  <div className="flex items-center gap-1">
                    <input
                      className="flex-1 rounded border border-line2 px-1.5 py-0.5 text-xs"
                      value={editForm.dir}
                      onChange={(e) => setEditForm((f) => ({ ...f, dir: e.target.value }))}
                      placeholder="(mặc định trong thư mục đầu ra)"
                    />
                    <button type="button" className={BTN_NEUTRAL} onClick={() => void pickEditDir()}>
                      Chọn…
                    </button>
                  </div>
                  {pickDirError ? <p className="text-[11px] text-red-600">{pickDirError}</p> : null}
                  <div className="flex justify-end gap-1">
                    <button type="button" className={BTN_NEUTRAL} onClick={() => setEditingId(null)}>
                      Huỷ
                    </button>
                    <button type="button" className={BTN_PRIMARY} onClick={() => void saveEdit(meta.id)}>
                      Lưu
                    </button>
                  </div>
                </div>
              ) : (
                <>
                  <button
                    type="button"
                    className="w-full px-2 py-1.5 text-left"
                    onClick={() => void selectScript(meta.id)}
                  >
                    <span className="block text-sm font-medium text-ink">{meta.name}</span>
                    <CountsLine meta={meta} />
                  </button>
                  <div className="flex gap-1 px-2 pb-1.5 text-[11px]">
                    <button type="button" className="text-sky-600 hover:underline" onClick={() => startEdit(meta)}>
                      Đổi tên/Thư mục
                    </button>
                    {confirmingDeleteId === meta.id ? (
                      <span className="ml-auto flex items-center gap-1">
                        Xoá thật?
                        <button
                          type="button"
                          disabled={busy}
                          className="text-red-700 hover:underline disabled:opacity-50"
                          onClick={() => void handleDeleteConfirmed(meta.id)}
                        >
                          Có
                        </button>
                        <button type="button" className="hover:underline" onClick={() => setConfirmingDeleteId(null)}>
                          Không
                        </button>
                      </span>
                    ) : (
                      <button
                        type="button"
                        className="ml-auto text-red-700 hover:underline"
                        onClick={() => setConfirmingDeleteId(meta.id)}
                      >
                        Xoá
                      </button>
                    )}
                  </div>
                </>
              )}
            </li>
          ))}
        </ul>
      )}

      {showImport && selectedScriptId !== null && selectedMeta ? (
        <ImportTxtModal
          scriptName={selectedMeta.name}
          onClose={() => setShowImport(false)}
          onImport={(text) => importTxt(selectedScriptId, text)}
        />
      ) : null}
    </div>
  );
}
