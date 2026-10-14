// Node "Prompt" (NodeKind::Prompt, spec §4.2) — textarea nhieu dong, dinh dang TXT P0: moi dong
// khong rong la mot canh, hint `<prompt> | ratio=16:9 | dur=10 | ...` (P0) chi hien nhu
// placeholder, KHONG parse o day (Rust `scene_txt::parse_txt`/`parse_line` moi la nguon phan tich
// that qua `workflow_validate`).
//
// I2 (fix-round 1, final-review): `scene_txt::parse_line` (crates/lvc-core/src/scene_txt.rs) coi
// PHAN 0 (truoc dau `|` DAU TIEN) la TOAN BO prompt, cac hint theo sau PHAI dung `|` rieng — vi du
// cu `"cảnh 2 ratio=16:9|dur=10"` (KHONG co `|` truoc `ratio=`) gui nguyen ca chuoi
// `"cảnh 2 ratio=16:9"` lam prompt, `ratio=16:9` khong duoc hieu la override (chi `dur=10` co tac
// dung) — vua sai y nguoi dung vua ton credit gui prompt bi "nhiem" chuoi hint. Placeholder duoi
// day sua thanh `| ratio=16:9 | dur=10` (`|` truoc CA HAI hint).
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type PromptKind = Extract<NodeKind, { kind: "prompt" }>;
export type PromptRFNode = RFNodeBase<PromptKind, "prompt">;

/**
 * So canh se duoc gui (uoc luong hien thi, KHONG phai nguon su that — `workflow_validate` moi la
 * nguon that khi bam "Kiem tra"/"Chay"). Bo qua dong trong VA dong bat dau bang `#` (sau `trim`) —
 * khop dung `scene_txt::parse_txt` (crates/lvc-core/src/scene_txt.rs): "dong trong hoac bat dau
 * bang `#`" deu KHONG phai canh (M7, fix-round 1 final-review) — truoc ban sua nay ham chi bo qua
 * dong trong nen mot workflow co dong chu thich se hien "N canh" nhieu hon so canh THAT gui di.
 */
export function countScenes(text: string): number {
  return text
    .split("\n")
    .filter((line) => {
      const trimmed = line.trim();
      return trimmed.length > 0 && !trimmed.startsWith("#");
    }).length;
}

export default function PromptNode({ id, data, selected }: NodeProps<PromptRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <NodeShell label={NODE_LABEL_VI.prompt} kind="prompt" selected={selected} errors={errors} runState={runState}>
        <textarea
          className="nodrag h-24 w-full resize-none rounded-md border border-line bg-fill p-1.5 text-xs text-ink placeholder-slate-500 outline-none focus:border-sky-400/60"
          value={data.text}
          placeholder={"cảnh 1\ncảnh 2 | ratio=16:9 | dur=10\ncảnh 3"}
          onChange={(e) => updateNodeData(id, { ...data, text: e.target.value })}
        />
        <p className="text-[10px] text-ink2">{countScenes(data.text)} cảnh</p>
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
