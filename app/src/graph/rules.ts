// Mirror phia client cua hai quy tac Rust (crates/lvc-core/src/{mode.rs,graph/validate.rs}) can
// phan hoi NGAY khi nguoi dung thao tac tren canvas (keo tha, chon duration, noi canh) — khong
// doi round-trip `workflow_validate`. Server la nguon su that cuoi cung; mirror nay chi de UX
// muot, sai lech se van bi server bat lai o `workflow_validate`/`run_start`.
import type { Mode, Model, NodeKindTag } from "../ipc/types";
import { PORTS } from "./ports";
import type { RFEdge, RFNode } from "./adapter";

/**
 * `Mode::allowed_durations` (crates/lvc-core/src/mode.rs:53, ADR-044, thay ADR-033/034): pro-like
 * (`pro`/`pro_max`/`agent`) luon kep (10,15) bat ke model; `fast` + `seedance-2.5` duoc them 30;
 * `fast` + model khac van (10,15). Bang tinh, khop dung 6 truong hop test Rust
 * `thoi_luong_cho_phep_theo_model_adr_044`.
 */
const PRO_LIKE_MODES: ReadonlySet<Mode> = new Set<Mode>(["pro", "pro_max", "agent"]);

export function allowedDurations(mode: Mode, model: Model): readonly number[] {
  if (PRO_LIKE_MODES.has(mode)) {
    return [10, 15];
  }
  if (model === "seedance-2.5") {
    return [10, 15, 30];
  }
  return [10, 15];
}

/**
 * `many` cua tung cong VAO theo `NodeKind` — mirror truong `many` cua `inputs` trong manifest
 * node (P4a, `fixtures/catalog.json`). KHONG lay tu `PORTS` (`src/graph/ports.ts`): bang do
 * co chu dich chi mang `{name,type}` khop `fixtures/ports.json`, khong mang co `many`/`required`/
 * `collect` cua Rust (xem concern task-11-report.md muc cuoi). Chi can cong VAO vi
 * `check_multiple_inputs` (validate.rs) chi xet `node.kind.inputs()`, khong xet cong ra.
 */
const INPUT_PORT_MANY: Record<NodeKindTag, Record<string, boolean>> = {
  prompt: {},
  image_upload: {},
  dola_video: { prompt: true, refs: true },
  concat: { in: true },
  save: { video: true, image: true },
};

/**
 * Phan toi thieu chung giua `Connection` (`@xyflow/system`, handle luon `string | null`) va
 * `RFEdge`/`Edge` (handle `string | null | undefined` — optional field trong `EdgeBase`) — du de
 * kiem tra, nhan ca hai.
 */
export interface ConnectionLike {
  source: string;
  sourceHandle?: string | null | undefined;
  target: string;
  targetHandle?: string | null | undefined;
}

/**
 * Kiem hop le MOT canh sap noi truoc khi cho vao canvas — mirror hai buoc dau cua `validate()`
 * phia server (`crates/lvc-core/src/graph/validate.rs`): kieu cong phai khop (tuong duong
 * `GraphError::TypeMismatch`) va cong vao `many=false` khong duoc nhan qua 1 canh (tuong duong
 * `GraphError::MultipleInputsOnSinglePort`). Dung lam prop `isValidConnection` cua `<ReactFlow>`
 * (chan ve canh sai truoc ca khi tha chuot) va lai o `onConnect` cua store (chan lan nua phong
 * khi co noi goi thang khong qua UI keo-tha).
 */
export function isValidConnection(
  connection: ConnectionLike,
  nodes: readonly RFNode[],
  edges: readonly RFEdge[],
): boolean {
  const { source, sourceHandle, target, targetHandle } = connection;
  if (!sourceHandle || !targetHandle) {
    return false;
  }
  // Fix-round 1 (review I3): tu noi mot node vao chinh no (node co cong vao va cong ra cung kieu)
  // tao mot canh 1-node — Rust van bat duoc (nhu mot Cycle 1 node qua topo_order, xem
  // validate.rs) nhung chi tren vong tron `workflow_validate`, khong o lop phan hoi tuc thi nay.
  // Hom nay chua node nao nhu vay, nhung node module sau nay co the co — chan luon o day.
  if (source === target) {
    return false;
  }

  const sourceNode = nodes.find((n) => n.id === source);
  const targetNode = nodes.find((n) => n.id === target);
  if (!sourceNode || !targetNode) {
    return false;
  }

  const sourceKind = sourceNode.type;
  const targetKind = targetNode.type;
  if (!sourceKind || !targetKind) {
    return false;
  }

  const outPort = PORTS[sourceKind].outputs.find((p) => p.name === sourceHandle);
  const inPort = PORTS[targetKind].inputs.find((p) => p.name === targetHandle);
  if (!outPort || !inPort) {
    return false;
  }
  if (outPort.type !== inPort.type) {
    return false;
  }

  const many = INPUT_PORT_MANY[targetKind][targetHandle] ?? true;
  if (!many) {
    const already = edges.some((e) => e.target === target && e.targetHandle === targetHandle);
    if (already) {
      return false;
    }
  }

  return true;
}
