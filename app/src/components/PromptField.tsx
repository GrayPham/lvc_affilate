// O nhap PROMPT dung chung trong bang tham so duoi (`VideoParamsPanel`/`ApiNodeParamsPanel`) — neu
// node dang chon DA co mot node Prompt noi vao cong `prompt`, sua THANG node do; neu CHUA co, go
// chu vao day se TU TAO mot node Prompt + TU NOI day (qua `addNodeConnected` voi `dataOverride` de
// nhet luon chu vua go vao node moi, khong can nguoi dung tu keo day tay — review nguoi dung: "nếu
// Prompt không nối thì có thể cho nhập chứ"). Tach rieng component de dung chung (truoc day moi
// panel tu chep logic nay).
import { useGraphStore } from "../store/graph";

export default function PromptField({ id }: { id: string }) {
  const nodes = useGraphStore((s) => s.nodes);
  const edges = useGraphStore((s) => s.edges);
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const addNodeConnected = useGraphStore((s) => s.addNodeConnected);

  const selfNode = nodes.find((n) => n.id === id);
  const promptEdge = edges.find((e) => e.target === id && e.targetHandle === "prompt");
  const promptNode = promptEdge
    ? nodes.find((n) => n.id === promptEdge.source && n.data.kind === "prompt")
    : undefined;
  const promptText = promptNode && promptNode.data.kind === "prompt" ? promptNode.data.text : "";

  function handleChange(value: string): void {
    if (promptNode && promptNode.data.kind === "prompt") {
      updateNodeData(promptNode.id, { ...promptNode.data, text: value });
      return;
    }
    const basePos = selfNode?.position ?? { x: 0, y: 0 };
    addNodeConnected(
      "prompt",
      { x: basePos.x - 320, y: basePos.y },
      { nodeId: id, handleId: "prompt", handleType: "target" },
      { text: value },
    );
  }

  return (
    <textarea
      className="nodrag min-h-[64px] w-full resize-y rounded-md border border-line bg-fill px-2 py-1.5 text-xs leading-relaxed text-fg outline-none focus:border-sky-400/60"
      value={promptText}
      placeholder="Mô tả nội dung… (gõ vào đây sẽ tự tạo + nối node Prompt)"
      onChange={(e) => handleChange(e.target.value)}
    />
  );
}
