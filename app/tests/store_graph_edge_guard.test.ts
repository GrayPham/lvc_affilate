// Hồi quy: React Flow 12 đôi khi TỰ phát lệnh `onEdgesChange` type "remove" cho một cạnh nó chưa
// neo được vào handle NGAY SAU khi nạp đồ thị (vd `ghep->save` tới cổng `video` của node Lưu).
// Trước đây store áp dụng mù -> cạnh hợp lệ biến mất -> workflow báo "save thiếu đầu vào" dù
// template đúng. Store PHẢI bỏ qua 'remove' tự phát (cạnh chưa được người dùng chọn), nhưng vẫn
// cho xoá khi người dùng CHỌN cạnh rồi Delete.
import { describe, expect, it } from "vitest";

import { createGraphStore } from "../src/store/graph";
import type { Graph } from "../src/ipc/types";

function graphConcatSave(): Graph {
  return {
    version: 2,
    nodes: [
      { id: "ghep", kind: { kind: "concat" }, x: 0, y: 0 },
      { id: "save", kind: { kind: "save", out_dir: "" }, x: 300, y: 0 },
    ],
    edges: [{ id: "e1", from: "ghep", from_port: "out", to: "save", to_port: "video" }],
  } as unknown as Graph;
}

describe("onEdgesChange: chống React Flow tự xoá cạnh hợp lệ khi nạp", () => {
  it("bỏ qua 'remove' tự phát cho cạnh CHƯA được chọn -> ghep->save giữ nguyên", () => {
    const store = createGraphStore({ workflowId: "t", workflowName: "t" });
    store.getState().loadGraph(graphConcatSave());
    expect(store.getState().edges.map((e) => e.id)).toEqual(["e1"]);

    // React Flow tự phát remove ngay sau nạp (cạnh chưa được chọn).
    store.getState().onEdgesChange([{ type: "remove", id: "e1" }]);
    expect(store.getState().edges.map((e) => e.id)).toEqual(["e1"]);
  });

  it("người dùng CHỌN cạnh rồi xoá -> vẫn xoá được (không chặn nhầm thao tác thật)", () => {
    const store = createGraphStore({ workflowId: "t", workflowName: "t" });
    store.getState().loadGraph(graphConcatSave());

    store.getState().onEdgesChange([{ type: "select", id: "e1", selected: true }]);
    store.getState().onEdgesChange([{ type: "remove", id: "e1" }]);
    expect(store.getState().edges).toEqual([]);
  });
});
