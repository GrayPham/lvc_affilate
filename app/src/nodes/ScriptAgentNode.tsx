// Node "Agent kịch bản affiliate" (kind `script_agent`) — giống `ScriptGenNode` ở khung/hợp đồng
// (chủ đề/brief -> AI viết N phân cảnh -> cổng `out`), khác ở 2 điểm theo yêu cầu người dùng
// (2026-10-13):
// 1. Chỉ dẫn hệ thống LUÔN chuyên cho kịch bản affiliate (hook/lợi ích/CTA) — backend khoá cứng,
//    ô "Yêu cầu thêm" ở đây chỉ NỐI THÊM, không thay thế (xem CLAUDE.md node-script-agent).
// 2. Ô chọn nhà cung cấp AI KHÔNG còn là dropdown tĩnh gemini/openai + ô gõ tay tên API key — thay
//    bằng MỘT dropdown liệt kê ĐỘNG đúng các API key (Gemini/OpenAI) ĐANG CÓ trong kho Tài khoản AI
//    (`useCredentialsStore`) — nền tảng nào chưa có API key thì không hiện, đúng yêu cầu "trong kho
//    có api key của nền tảng nào thì hiện nền tảng đó" (Claude bị loại khỏi phạm vi — quyết định
//    người dùng 2026-10-13, chỉ Gemini/OpenAI qua `kind=api_key`).
import { useEffect } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { useCredentialsStore } from "../store/credentials";
import { credentialProviderLabel } from "../format/credential";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type ScriptAgentKind = Extract<NodeKind, { kind: "script_agent" }>;
export type ScriptAgentRFNode = RFNodeBase<ScriptAgentKind, "script_agent">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 placeholder-slate-500 outline-none focus:border-sky-400/60";

/** Nhà cung cấp AI text mà `TextGenHost` (backend) hỗ trợ — xem `crates/providers/lvc-genai`. Chỉ
 * 2 nền tảng này được liệt kê động từ kho credential (đã loại Claude theo quyết định người dùng). */
const TEXT_GEN_PROVIDERS = new Set(["gemini", "openai"]);

export default function ScriptAgentNode({ id, data, selected }: NodeProps<ScriptAgentRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  const credRows = useCredentialsStore((s) => s.rows);
  const loadCred = useCredentialsStore((s) => s.load);
  useEffect(() => {
    void loadCred();
  }, [loadCred]);

  // Chỉ API key (kind=api_key), con dung (enabled), nha cung cap sinh TEXT ho tro (gemini/openai).
  const usable = credRows.filter(
    (r) => r.enabled && r.kind === "api_key" && TEXT_GEN_PROVIDERS.has(r.provider),
  );
  const currentValue = `${data.provider}::${data.credential}`;
  const hasCurrent = usable.some((r) => `${r.provider}::${r.label}` === currentValue);

  function onPickCredential(value: string): void {
    const idx = value.indexOf("::");
    if (idx < 0) {
      return;
    }
    const provider = value.slice(0, idx);
    const credential = value.slice(idx + 2);
    if (provider === "gemini" || provider === "openai") {
      updateNodeData(id, { ...data, provider, credential });
    }
  }

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle
        type="target"
        position={Position.Left}
        id="brief"
        style={{ background: PORT_TYPE_COLOR.text }}
      />
      <NodeShell label={NODE_LABEL_VI.script_agent} selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <textarea
            className={`${FIELD_CLASS} h-16 resize-none`}
            value={data.topic}
            placeholder="Sản phẩm / chương trình affiliate (bỏ trống nếu nối cổng brief)"
            onChange={(e) => updateNodeData(id, { ...data, topic: e.target.value })}
          />
          <div className="flex items-center gap-2 text-[11px] text-slate-400">
            <label className="flex items-center gap-1">
              Số cảnh
              <input
                type="number"
                min={1}
                className={`${FIELD_CLASS} w-14`}
                value={data.num_scenes}
                onChange={(e) =>
                  updateNodeData(id, { ...data, num_scenes: Math.max(1, Number(e.target.value) || 1) })
                }
              />
            </label>
          </div>
          {usable.length > 0 ? (
            <select
              className={FIELD_CLASS}
              title="Model AI (chỉ hiện nền tảng đã có API key trong kho)"
              value={hasCurrent ? currentValue : ""}
              onChange={(e) => onPickCredential(e.target.value)}
            >
              {!hasCurrent ? (
                <option value="" disabled>
                  — Chọn model AI —
                </option>
              ) : null}
              {usable.map((r) => (
                <option key={r.id} value={`${r.provider}::${r.label}`} className="bg-neutral-900 text-neutral-100">
                  {credentialProviderLabel(r.provider)} — {r.label}
                </option>
              ))}
            </select>
          ) : (
            <p className="text-[11px] text-amber-400">
              Chưa có API key Gemini/OpenAI nào trong kho — vào trang "Tài khoản AI" để thêm trước.
            </p>
          )}
          <textarea
            className={`${FIELD_CLASS} h-10 resize-none`}
            value={data.extra_instructions}
            placeholder="Yêu cầu thêm (tuỳ chọn — nối vào chỉ dẫn affiliate có sẵn, không thay thế)"
            onChange={(e) => updateNodeData(id, { ...data, extra_instructions: e.target.value })}
          />
          <label className="flex items-center gap-1.5 text-[11px] text-slate-400">
            <input
              type="checkbox"
              className="nodrag"
              checked={data.save_as_script}
              onChange={(e) => updateNodeData(id, { ...data, save_as_script: e.target.checked })}
            />
            Lưu thành kịch bản khi chạy
          </label>
          {data.save_as_script ? (
            <input
              className={FIELD_CLASS}
              value={data.script_name}
              placeholder="Tên kịch bản (bỏ trống để tự đặt tên)"
              onChange={(e) => updateNodeData(id, { ...data, script_name: e.target.value })}
            />
          ) : null}
        </div>
      </NodeShell>
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        style={{ background: PORT_TYPE_COLOR.text }}
      />
    </>
  );
}
