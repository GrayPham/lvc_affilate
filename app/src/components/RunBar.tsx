// Thanh dieu khien run (Task 12 brief): Kiểm tra (`workflow_validate`), Chạy (`run_start`), Huỷ
// (`run_cancel`), Lưu/Mở/Mới (`workflow_save`/`workflow_list`+`workflow_get`/canvas trống). Nam
// gon logic goi IPC + orchestrator trang thai run o day thay vi rai ra Workflow.tsx.
import { useEffect, useRef, useState } from "react";
import { z } from "zod";

import { newEphemeralWorkflowId, useGraphStore, useGraphStoreApi } from "../store/graph";
import { useQueueStore } from "../store/queue";
import { useRunStore, useRunStoreApi } from "../store/run";
import { registerRunOwner, unregisterRunOwner } from "../store/runRegistry";
import { useWorkflowUi } from "../store/workflowUi";
import {
  isIpcError,
  runCancel,
  runStart,
  templateSavePrivate,
  workflowSave,
  workflowSetOrigin,
  workflowValidate,
} from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { confirmDialog, promptDialog } from "../store/dialog";
import { describeTemplateError } from "../format/template";
import { GraphErrorSchema, type PlanSummary, type WorkflowDoc } from "../ipc/types";
import { formatGraphError } from "../graph/errors";
import WorkflowListModal from "./WorkflowListModal";
import TemplateGallery from "./TemplateGallery";
import PublishWorkflowDialog from "./PublishWorkflowDialog";

export default function RunBar() {
  const nodes = useGraphStore((s) => s.nodes);
  const edges = useGraphStore((s) => s.edges);
  const globalErrors = useGraphStore((s) => s.globalErrors);
  const nodeErrors = useGraphStore((s) => s.nodeErrors);
  const setValidation = useGraphStore((s) => s.setValidation);
  const loadGraph = useGraphStore((s) => s.loadGraph);
  const resetGraph = useGraphStore((s) => s.resetGraph);
  const toGraph = useGraphStore((s) => s.toGraph);
  const graphApi = useGraphStoreApi();
  const enqueue = useQueueStore((s) => s.add);
  // P-tabs: id/ten workflow cua TAB nay chuyen vao graph store (xem doc `GraphStoreState`) de thanh
  // tab (`WorkflowTabs.tsx`) doc duoc ten tab theo thoi gian thuc.
  const workflowId = useGraphStore((s) => s.workflowId);
  const workflowName = useGraphStore((s) => s.workflowName);
  const workflowSaved = useGraphStore((s) => s.workflowSaved);
  const setWorkflowMeta = useGraphStore((s) => s.setWorkflowMeta);

  const runId = useRunStore((s) => s.runId);
  const runStatus = useRunStore((s) => s.status);
  const beginRun = useRunStore((s) => s.beginRun);
  const clearRun = useRunStore((s) => s.clearRun);
  // P-tabs: instance store run cua TAB nay — dang ky lam "chu" cua runId vua tao de
  // `engine_bridge.ts` dinh tuyen dung su kien toi day (xem `store/runRegistry.ts`).
  const runStoreApi = useRunStoreApi();

  // "Kho mẫu" (TemplateGallery) chia trang thai mo voi thanh cong cu trai (CanvasToolbar) qua
  // `store/workflowUi.ts` — RunBar van la noi render gallery, CanvasToolbar chi mo.
  const showTemplates = useWorkflowUi((s) => s.templatesOpen);
  const openTemplates = useWorkflowUi((s) => s.openTemplates);
  const closeTemplates = useWorkflowUi((s) => s.closeTemplates);

  const [planSummary, setPlanSummary] = useState<PlanSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [showPublish, setShowPublish] = useState(false);
  /** Ten dang sua tai cho; `null` = khong sua. */
  const [renaming, setRenaming] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [showOpen, setShowOpen] = useState(false);

  const nodeErrorCount = Object.keys(nodeErrors).length;
  const running = runId !== null && runStatus === null;

  // P-tabs: tab dong (component nay unmount) -> go dang ky run khoi runRegistry — CHI anh huong
  // dinh tuyen su kien SONG toi tab nay (da khong con render de thay nua), KHONG huy run that su
  // (engine chay tiep o server, van xem lai duoc qua trang "Tác vụ" doc tu DB). Dung ref vi gia tri
  // `runId` moi nhat can o THOI DIEM unmount, khong phai luc effect dang ky (chay dung 1 lan).
  const runIdRef = useRef(runId);
  runIdRef.current = runId;
  useEffect(() => {
    return () => {
      if (runIdRef.current) {
        unregisterRunOwner(runIdRef.current);
      }
    };
  }, []);

  // Do thi doi (them/xoa/noi node, sua field) sau lan "Kiểm tra" gan nhat -> ket qua cu (video
  // total) khong con dang tin cay, an di thay vi de nguoi dung tuong da kiem lai roi.
  useEffect(() => {
    setPlanSummary(null);
  }, [nodes, edges]);

  async function handleValidate(): Promise<boolean> {
    setBusy(true);
    setMessage(null);
    try {
      const graph = toGraph();
      const summary = await workflowValidate(graph);
      setPlanSummary(summary);
      setValidation(null);
      return true;
    } catch (e) {
      setPlanSummary(null);
      if (isIpcError(e) && e.code === "GRAPH_INVALID") {
        const parsed = z.array(GraphErrorSchema).safeParse(e.details);
        if (parsed.success) {
          setValidation(parsed.data);
          return false;
        }
      }
      setValidation(null);
      setMessage(errorText(e));
      return false;
    } finally {
      setBusy(false);
    }
  }

  /** "Thêm vào hàng chờ" (màn Tác vụ) — xếp workflow hiện tại vào cuối hàng để chạy lần lượt,
   * KHÁC "Chạy" (chạy ngay lập tức). Đồ thị được backend chụp lại nên sửa canvas sau đó không
   * làm đổi mục đã xếp. */
  async function handleEnqueue(): Promise<void> {
    const ok = await handleValidate();
    if (!ok) {
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      await enqueue(workflowId, workflowName, toGraph());
      setMessage(`Đã thêm "${workflowName}" vào hàng chờ — xem ở tab Tác vụ.`);
    } catch (e) {
      setMessage(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleRun(): Promise<void> {
    const ok = await handleValidate();
    if (!ok) {
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      // Xoa TRUOC khi goi run_start — tranh khoang trong su kien cua run moi den roi bi lan reset
      // sau xoa mat (xem doc `clearRun`/`beginRun` trong store/run.ts). Go dang ky run CU (neu co)
      // khoi so dang ky (store/runRegistry.ts) — tranh map giu rac mai mot runId da xong.
      if (runId) {
        unregisterRunOwner(runId);
      }
      clearRun();
      const graph = toGraph();
      const info = await runStart(workflowId, graph);
      // Dang ky TAB NAY la chu cua runId vua tao TRUOC `beginRun` — flush dung trinh tu neu su kien
      // "queued" da toi truoc do bi dem lai trong runRegistry (xem doc o do).
      registerRunOwner(info.runId, runStoreApi);
      beginRun(info.runId);
    } catch (e) {
      setMessage(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleCancel(): Promise<void> {
    if (!runId) {
      return;
    }
    setBusy(true);
    try {
      await runCancel(runId);
    } catch (e) {
      setMessage(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  /** Doi ten tai cho. Workflow DA LUU thi ghi lai DB luon — khong thi mo lai se ve ten cu, nguoi
   * dung tuong app nuot mat thao tac. Chua luu thi chi doi tren canvas (chua co gi trong DB de ghi). */
  async function commitRename(next: string): Promise<void> {
    const name = next.trim();
    setRenaming(null);
    if (name === "" || name === workflowName) {
      return;
    }
    setWorkflowMeta(workflowId, name);
    if (!workflowSaved) {
      return;
    }
    try {
      await workflowSave(workflowId, name, toGraph());
    } catch (e) {
      setMessage(errorText(e));
    }
  }

  /** Nut "Lưu" — luon hoi ten (de nguoi dung doi ten nhan tien), roi ghi. */
  async function handleSave(): Promise<void> {
    const name = await promptDialog({
      title: "Lưu workflow",
      message: "Tên workflow:",
      defaultValue: workflowName,
      okText: "Lưu",
    });
    if (name === null || name.trim() === "") {
      return;
    }
    await saveAs(name);
  }

  /** Ghi THAT vao bang `workflows`. Tach khoi `handleSave` de Ctrl+S dung lai duoc ma khong phai
   * hoi ten lan nua. */
  async function saveAs(name: string): Promise<void> {
    setBusy(true);
    setMessage(null);
    try {
      const graph = toGraph();
      const meta = await workflowSave(workflowId, name.trim(), graph);
      setWorkflowMeta(meta.id, meta.name, true);
      // Đồ thị này dựng từ một mẫu CỘNG ĐỒNG có phiên bản -> ghi nguồn gốc để sau này app biết
      // mẫu gốc có bản mới hay chưa. Ghi thất bại KHÔNG được làm người dùng tưởng là chưa lưu
      // được workflow (đã lưu xong ở trên rồi) — chỉ mất tính năng nhắc cập nhật.
      const origin = graphApi.getState().templateOrigin;
      if (origin !== null && origin.sourceVersion !== "") {
        try {
          await workflowSetOrigin({
            workflowId: meta.id,
            sourceId: origin.sourceId,
            sourceVersion: origin.sourceVersion,
            graph,
          });
        } catch {
          // Bỏ qua có chủ ý: xem ghi chú ngay trên.
        }
      }
      setMessage(`Đã lưu "${meta.name}" vào Workflow của tôi.`);
    } catch (e) {
      setMessage(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  // Ctrl+S (Workflow.tsx phat CustomEvent "lvc:save") -> luu workflow. Dung ref de tranh closure cu.
  //
  // Workflow DA LUU thi ghi de NGAY, khong hoi lai ten — dung nghia "Ctrl+S" ma ai cung quen.
  // Chua luu lan nao thi van phai hoi ten mot lan dau.
  const quickSaveRef = useRef<() => void>(() => undefined);
  quickSaveRef.current = () => {
    if (busy) {
      return;
    }
    if (workflowSaved) {
      void saveAs(workflowName);
    } else {
      void handleSave();
    }
  };
  useEffect(() => {
    const h = (): void => quickSaveRef.current();
    window.addEventListener("lvc:save", h);
    return () => window.removeEventListener("lvc:save", h);
  }, []);

  /** "Lưu thành mẫu" — hoi ten (+ mo ta tuy chon) roi luu canvas hien tai thanh mau private moi.
   * `thumbnail` de rong ("") — chua chup anh canvas. Dung cung `toGraph` nhu duong luu workflow. */
  async function handleSaveTemplate(): Promise<void> {
    if (nodes.length === 0) {
      setMessage("Canvas trống — chưa có gì để lưu thành mẫu.");
      return;
    }
    const name = await promptDialog({ title: "Lưu thành mẫu", message: "Tên mẫu:", defaultValue: workflowName, okText: "Tiếp" });
    if (name === null || name.trim() === "") {
      return;
    }
    const description = await promptDialog({ title: "Lưu thành mẫu", message: "Mô tả (tuỳ chọn):", defaultValue: "", multiline: true, okText: "Lưu" });
    if (description === null) {
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const graph = toGraph();
      const meta = await templateSavePrivate(null, name.trim(), graph, description.trim(), "");
      setMessage(`Đã lưu mẫu "${meta.name}".`);
    } catch (e) {
      setMessage(describeTemplateError(e));
    } finally {
      setBusy(false);
    }
  }

  /**
   * Fix-round 1 (review C1): "Mới"/"Mở" KHONG duoc tu clearRun() ngam khi dang co run active —
   * run mo coi tu do se van tiep tuc chay/phat su kien tren engine (mot process-wide broadcast
   * channel dung chung, khong tach rieng theo run), va khong con nut "Huỷ" nao con lai de goi
   * `run_cancel`. Hoi truoc, cho phep Huỷ; neu nguoi dung dong y HOAC tu choi Huỷ nhung van muon
   * tiep tuc, chi KHONG clearRun() — giu `runId` de con theo doi/Huỷ duoc toi khi that su nhan
   * duoc `run_done` (store/run.ts, `applyEvent` fix-round 1 C1 da chan su kien cua run khac hijack
   * lai `runId` nay). Tra `false` = nguoi dung huy thao tac (khong lam gi them).
   */
  async function confirmCancelIfRunning(): Promise<boolean> {
    if (!running || !runId) {
      return true;
    }
    const proceed = await confirmDialog({
      title: "Đang chạy",
      message: `Đang có lần chạy (run ${runId}) chưa xong. Huỷ lần chạy này để tiếp tục?`,
      okText: "Huỷ lần chạy",
      cancelText: "Để nguyên",
      danger: true,
    });
    if (!proceed) {
      return false;
    }
    try {
      await runCancel(runId);
    } catch (e) {
      setMessage(errorText(e));
    }
    return true;
  }

  async function handleNew(): Promise<void> {
    if (!(await confirmCancelIfRunning())) {
      return;
    }
    if (
      nodes.length > 0 &&
      !(await confirmDialog({
        title: "Workflow mới",
        message: "Tạo workflow mới? Canvas hiện tại sẽ bị xoá (nếu chưa lưu sẽ mất).",
        okText: "Tạo mới",
        danger: true,
      }))
    ) {
      return;
    }
    resetGraph();
    if (!running) {
      clearRun();
    }
    setWorkflowMeta(newEphemeralWorkflowId(), "Workflow chưa đặt tên");
    setPlanSummary(null);
    setMessage(null);
  }

  async function handlePicked(doc: WorkflowDoc): Promise<void> {
    if (!(await confirmCancelIfRunning())) {
      return;
    }
    setShowOpen(false);
    loadGraph(doc.graph);
    setWorkflowMeta(doc.meta.id, doc.meta.name, true);
    setPlanSummary(null);
    setMessage(null);
    if (!running) {
      clearRun();
    }
  }

  return (
    <div className="flex flex-col gap-2.5 border-b border-line bg-rail px-3 py-2.5 text-ink">
      <div className="flex items-center gap-1.5">
        <svg viewBox="0 0 24 24" className="h-4 w-4 shrink-0 text-accent-text" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
          <path d="M6 3h9l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z" />
          <path d="M14 3v4a1 1 0 0 0 1 1h4" />
        </svg>
        {renaming === null ? (
          <button
            type="button"
            title="Bấm để đổi tên"
            onClick={() => setRenaming(workflowName)}
            className="rounded px-1 text-sm font-semibold text-ink hover:bg-fill2"
          >
            {workflowName}
          </button>
        ) : (
          <input
            autoFocus
            value={renaming}
            onChange={(e) => setRenaming(e.target.value)}
            onBlur={() => void commitRename(renaming)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                void commitRename(renaming);
              } else if (e.key === "Escape") {
                setRenaming(null);
              }
            }}
            className="min-w-0 flex-1 rounded border border-amber-400/60 bg-fill px-1.5 py-0.5 text-sm font-semibold text-ink outline-none"
          />
        )}
        {workflowSaved ? null : (
          <span className="text-[10px] text-ink3">chưa lưu</span>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={busy}
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => void handleValidate()}
        >
          Kiểm tra
        </button>
        <button
          type="button"
          disabled={busy || running}
          className="rounded-lg bg-accent px-4 py-2 text-xs font-semibold text-accent-ink shadow-md shadow-amber-500/30 hover:bg-amber-400 hover:shadow-lg hover:shadow-amber-500/40 disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none"
          onClick={() => void handleRun()}
        >
          ▶ Chạy
        </button>
        <button
          type="button"
          disabled={busy || !running}
          className="rounded-lg border border-red-500/40 px-4 py-2 text-xs font-medium text-rose hover:bg-red-500/10 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => void handleCancel()}
        >
          Huỷ
        </button>
        <span className="mx-1 h-5 w-px bg-fill2" />
        <button
          type="button"
          disabled={busy}
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => void handleSave()}
        >
          Lưu <span className="ml-1 text-[10px] text-ink3">Ctrl+S</span>
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => setShowOpen(true)}
        >
          Workflow của tôi
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => void handleNew()}
        >
          Mới
        </button>
        <span className="mx-1 h-5 w-px bg-fill2" />
        <button
          type="button"
          disabled={busy}
          title="Xếp workflow này vào hàng chờ ở tab Tác vụ (chạy lần lượt)"
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => void handleEnqueue()}
        >
          + Hàng chờ
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={openTemplates}
        >
          Kho mẫu
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => void handleSaveTemplate()}
        >
          Lưu thành mẫu
        </button>
        <button
          type="button"
          disabled={busy}
          title="Mở Pull Request đưa workflow này lên kho cộng đồng"
          className="rounded-lg border border-line bg-fill px-4 py-2 text-xs font-medium text-ink hover:bg-fill2 disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => setShowPublish(true)}
        >
          ↑ Publish
        </button>
        {running ? (
          <span className="text-xs text-accent-text">Đang chạy (run {runId})…</span>
        ) : runStatus ? (
          <span className="text-xs text-ink2">Lần chạy trước: {runStatus}</span>
        ) : null}
      </div>

      {planSummary ? (
        <p className="text-xs text-ok">
          Hợp lệ — sẽ tạo {planSummary.videoTotal} video.
          {planSummary.warnings.length > 0 ? ` (${planSummary.warnings.join("; ")})` : ""}
        </p>
      ) : null}

      {message ? <p className="text-xs text-ink2">{message}</p> : null}

      {globalErrors.length > 0 || nodeErrorCount > 0 ? (
        <ul className="max-h-32 overflow-auto rounded-md border border-red-500/30 bg-red-950/40 p-2 text-xs text-red-200">
          {globalErrors.map((err, i) => (
            <li key={`g${i}`}>{formatGraphError(err)}</li>
          ))}
          {Object.entries(nodeErrors).flatMap(([nodeId, errs]) =>
            errs.map((err, i) => <li key={`${nodeId}-${i}`}>{formatGraphError(err)}</li>),
          )}
        </ul>
      ) : null}

      {showOpen ? (
        <WorkflowListModal onClose={() => setShowOpen(false)} onPicked={(doc) => void handlePicked(doc)} />
      ) : null}

      {showTemplates ? (
        <TemplateGallery
          onClose={closeTemplates}
          onApplied={(name) => setMessage(`Đã dùng mẫu "${name}".`)}
        />
      ) : null}

      {showPublish ? (
        <PublishWorkflowDialog
          defaultId={workflowId}
          defaultName={workflowName}
          graph={toGraph()}
          onClose={() => setShowPublish(false)}
        />
      ) : null}
    </div>
  );
}
