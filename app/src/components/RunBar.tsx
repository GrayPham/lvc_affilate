// Thanh dieu khien run (Task 12 brief): Kiểm tra (`workflow_validate`), Chạy (`run_start`), Huỷ
// (`run_cancel`), Lưu/Mở/Mới (`workflow_save`/`workflow_list`+`workflow_get`/canvas trống). Nam
// gon logic goi IPC + orchestrator trang thai run o day thay vi rai ra Workflow.tsx.
import { useEffect, useState } from "react";
import { z } from "zod";

import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { useWorkflowUi } from "../store/workflowUi";
import {
  isIpcError,
  runCancel,
  runStart,
  templateSavePrivate,
  workflowSave,
  workflowValidate,
} from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { confirmDialog, promptDialog } from "../store/dialog";
import { describeTemplateError } from "../format/template";
import { GraphErrorSchema, type PlanSummary, type WorkflowDoc } from "../ipc/types";
import { formatGraphError } from "../graph/errors";
import WorkflowListModal from "./WorkflowListModal";
import TemplateGallery from "./TemplateGallery";

function newEphemeralId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `wf-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export default function RunBar() {
  const nodes = useGraphStore((s) => s.nodes);
  const edges = useGraphStore((s) => s.edges);
  const globalErrors = useGraphStore((s) => s.globalErrors);
  const nodeErrors = useGraphStore((s) => s.nodeErrors);
  const setValidation = useGraphStore((s) => s.setValidation);
  const loadGraph = useGraphStore((s) => s.loadGraph);
  const resetGraph = useGraphStore((s) => s.resetGraph);
  const toGraph = useGraphStore((s) => s.toGraph);

  const runId = useRunStore((s) => s.runId);
  const runStatus = useRunStore((s) => s.status);
  const beginRun = useRunStore((s) => s.beginRun);
  const clearRun = useRunStore((s) => s.clearRun);

  // "Kho mẫu" (TemplateGallery) chia trang thai mo voi thanh cong cu trai (CanvasToolbar) qua
  // `store/workflowUi.ts` — RunBar van la noi render gallery, CanvasToolbar chi mo.
  const showTemplates = useWorkflowUi((s) => s.templatesOpen);
  const openTemplates = useWorkflowUi((s) => s.openTemplates);
  const closeTemplates = useWorkflowUi((s) => s.closeTemplates);

  const [workflowId, setWorkflowId] = useState<string>(() => newEphemeralId());
  const [workflowName, setWorkflowName] = useState("Workflow chưa đặt tên");
  const [planSummary, setPlanSummary] = useState<PlanSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [showOpen, setShowOpen] = useState(false);

  const nodeErrorCount = Object.keys(nodeErrors).length;
  const running = runId !== null && runStatus === null;

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

  async function handleRun(): Promise<void> {
    const ok = await handleValidate();
    if (!ok) {
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      // Xoa TRUOC khi goi run_start — tranh khoang trong su kien cua run moi den roi bi lan reset
      // sau xoa mat (xem doc `clearRun`/`beginRun` trong store/run.ts).
      clearRun();
      const graph = toGraph();
      const info = await runStart(workflowId, graph);
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

  async function handleSave(): Promise<void> {
    const name = await promptDialog({ title: "Lưu workflow", message: "Tên workflow:", defaultValue: workflowName, okText: "Lưu" });
    if (name === null || name.trim() === "") {
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const graph = toGraph();
      const meta = await workflowSave(workflowId, name.trim(), graph);
      setWorkflowId(meta.id);
      setWorkflowName(meta.name);
      setMessage(`Đã lưu "${meta.name}".`);
    } catch (e) {
      setMessage(errorText(e));
    } finally {
      setBusy(false);
    }
  }

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
    setWorkflowId(newEphemeralId());
    setWorkflowName("Workflow chưa đặt tên");
    setPlanSummary(null);
    setMessage(null);
  }

  async function handlePicked(doc: WorkflowDoc): Promise<void> {
    if (!(await confirmCancelIfRunning())) {
      return;
    }
    setShowOpen(false);
    loadGraph(doc.graph);
    setWorkflowId(doc.meta.id);
    setWorkflowName(doc.meta.name);
    setPlanSummary(null);
    setMessage(null);
    if (!running) {
      clearRun();
    }
  }

  return (
    <div className="flex flex-col gap-2 border-b border-white/10 bg-[#121216] p-2 text-slate-200">
      <div className="flex flex-wrap items-center gap-2">
        <span className="mr-2 text-xs font-medium text-slate-300" title={workflowId}>
          {workflowName}
        </span>
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-200 hover:bg-white/10 disabled:opacity-50"
          onClick={() => void handleValidate()}
        >
          Kiểm tra
        </button>
        <button
          type="button"
          disabled={busy || running}
          className="rounded-md bg-sky-500 px-3 py-1 text-xs font-medium text-white hover:bg-sky-400 disabled:opacity-50"
          onClick={() => void handleRun()}
        >
          Chạy
        </button>
        <button
          type="button"
          disabled={busy || !running}
          className="rounded-md border border-red-500/40 px-3 py-1 text-xs text-red-300 hover:bg-red-500/10 disabled:opacity-50"
          onClick={() => void handleCancel()}
        >
          Huỷ
        </button>
        <span className="mx-1 h-4 w-px bg-white/10" />
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-200 hover:bg-white/10 disabled:opacity-50"
          onClick={() => void handleSave()}
        >
          Lưu
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-200 hover:bg-white/10 disabled:opacity-50"
          onClick={() => setShowOpen(true)}
        >
          Mở
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-200 hover:bg-white/10 disabled:opacity-50"
          onClick={() => void handleNew()}
        >
          Mới
        </button>
        <span className="mx-1 h-4 w-px bg-white/10" />
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-200 hover:bg-white/10 disabled:opacity-50"
          onClick={openTemplates}
        >
          Kho mẫu
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-md border border-white/10 bg-white/5 px-3 py-1 text-xs text-slate-200 hover:bg-white/10 disabled:opacity-50"
          onClick={() => void handleSaveTemplate()}
        >
          Lưu thành mẫu
        </button>
        {running ? (
          <span className="text-xs text-sky-400">Đang chạy (run {runId})…</span>
        ) : runStatus ? (
          <span className="text-xs text-slate-400">Lần chạy trước: {runStatus}</span>
        ) : null}
      </div>

      {planSummary ? (
        <p className="text-xs text-emerald-400">
          Hợp lệ — sẽ tạo {planSummary.videoTotal} video.
          {planSummary.warnings.length > 0 ? ` (${planSummary.warnings.join("; ")})` : ""}
        </p>
      ) : null}

      {message ? <p className="text-xs text-slate-300">{message}</p> : null}

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
    </div>
  );
}
