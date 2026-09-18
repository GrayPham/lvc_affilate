// Thanh dieu khien run (Task 12 brief): Kiểm tra (`workflow_validate`), Chạy (`run_start`), Huỷ
// (`run_cancel`), Lưu/Mở/Mới (`workflow_save`/`workflow_list`+`workflow_get`/canvas trống). Nam
// gon logic goi IPC + orchestrator trang thai run o day thay vi rai ra Workflow.tsx.
import { useEffect, useState } from "react";
import { z } from "zod";

import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { isIpcError, runCancel, runStart, workflowSave, workflowValidate } from "../ipc/commands";
import { GraphErrorSchema, type PlanSummary, type WorkflowDoc } from "../ipc/types";
import { formatGraphError } from "../graph/errors";
import WorkflowListModal from "./WorkflowListModal";

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
      setMessage(e instanceof Error ? e.message : String(e));
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
      setMessage(e instanceof Error ? e.message : String(e));
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
      setMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleSave(): Promise<void> {
    const name = window.prompt("Tên workflow:", workflowName);
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
      setMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  function handleNew(): void {
    if (nodes.length > 0 && !window.confirm("Tạo workflow mới? Canvas hiện tại sẽ bị xoá (nếu chưa lưu sẽ mất).")) {
      return;
    }
    resetGraph();
    clearRun();
    setWorkflowId(newEphemeralId());
    setWorkflowName("Workflow chưa đặt tên");
    setPlanSummary(null);
    setMessage(null);
  }

  function handlePicked(doc: WorkflowDoc): void {
    setShowOpen(false);
    loadGraph(doc.graph);
    setWorkflowId(doc.meta.id);
    setWorkflowName(doc.meta.name);
    setPlanSummary(null);
    setMessage(null);
    clearRun();
  }

  return (
    <div className="flex flex-col gap-2 border-b border-slate-200 bg-white p-2">
      <div className="flex flex-wrap items-center gap-2">
        <span className="mr-2 text-xs font-medium text-slate-500" title={workflowId}>
          {workflowName}
        </span>
        <button
          type="button"
          disabled={busy}
          className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
          onClick={() => void handleValidate()}
        >
          Kiểm tra
        </button>
        <button
          type="button"
          disabled={busy || running}
          className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700 disabled:opacity-50"
          onClick={() => void handleRun()}
        >
          Chạy
        </button>
        <button
          type="button"
          disabled={busy || !running}
          className="rounded border border-red-300 px-3 py-1 text-xs text-red-700 hover:bg-red-50 disabled:opacity-50"
          onClick={() => void handleCancel()}
        >
          Huỷ
        </button>
        <span className="mx-1 h-4 w-px bg-slate-200" />
        <button
          type="button"
          disabled={busy}
          className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
          onClick={() => void handleSave()}
        >
          Lưu
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
          onClick={() => setShowOpen(true)}
        >
          Mở
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
          onClick={handleNew}
        >
          Mới
        </button>
        {running ? (
          <span className="text-xs text-sky-600">Đang chạy (run {runId})…</span>
        ) : runStatus ? (
          <span className="text-xs text-slate-500">Lần chạy trước: {runStatus}</span>
        ) : null}
      </div>

      {planSummary ? (
        <p className="text-xs text-emerald-700">
          Hợp lệ — sẽ tạo {planSummary.videoTotal} video.
          {planSummary.warnings.length > 0 ? ` (${planSummary.warnings.join("; ")})` : ""}
        </p>
      ) : null}

      {message ? <p className="text-xs text-slate-600">{message}</p> : null}

      {globalErrors.length > 0 || nodeErrorCount > 0 ? (
        <ul className="max-h-32 overflow-auto rounded border border-red-200 bg-red-50 p-2 text-xs text-red-700">
          {globalErrors.map((err, i) => (
            <li key={`g${i}`}>{formatGraphError(err)}</li>
          ))}
          {Object.entries(nodeErrors).flatMap(([nodeId, errs]) =>
            errs.map((err, i) => <li key={`${nodeId}-${i}`}>{formatGraphError(err)}</li>),
          )}
        </ul>
      ) : null}

      {showOpen ? <WorkflowListModal onClose={() => setShowOpen(false)} onPicked={handlePicked} /> : null}
    </div>
  );
}
