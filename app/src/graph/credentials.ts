// Kiem tra mot do thi co DU tai khoan/API key de chay hay khong — THUAN (khong I/O), nhan san
// `catalog` + danh sach credential de test duoc truc tiep.
//
// Vi sao kiem o day thay vi de engine bao loi luc chay: mot workflow trong hang cho co the toi luot
// sau nhieu gio; luc do bao "chua co tai khoan Gemini" thi nguoi dung da di ngu. Canh bao NGAY luc
// them vao hang cho moi cuu duoc dem chay.
//
// Chi kiem "co ton tai mot credential dang BAT cho (provider, kind) nay hay khong". KHONG kiem han
// muc/het han: node tu lay ngau nhien tu kho luc chay nen biet truoc cung khong chac dung — hua
// chac chan o day se thanh loi bao sai.
import type { CredentialView, Graph, NodeManifest } from "../ipc/types";

/** Mot nhu cau tai khoan chua duoc dap ung. */
export interface MissingCredential {
  /** `kind` cua node can (vd `"google_video"`). */
  nodeKind: string;
  /** Nhan tieng Viet cua node do, de hien cho nguoi dung. */
  nodeLabel: string;
  provider: string;
  /** Cac loai bi mat node chap nhan (vd `["api_key"]`). */
  kinds: string[];
  /** So node trong do thi dang can dung nhu cau nay. */
  nodeCount: number;
}

/**
 * Nhung nhu cau tai khoan ma do thi can nhung kho KHONG co.
 *
 * Gop theo `nodeKind`: ba node Veo3 cung thieu mot tai khoan Google thi bao MOT dong "Veo3 ×3",
 * khong phai ba dong giong nhau.
 *
 * Node khong co trong `catalog` (vd do thi cu dung kind da bo) bi bo qua — khong the biet no can gi
 * thi khong bao gi; `workflow_validate` phia Rust moi la cho bat loi do thi sai.
 */
export function missingCredentials(
  graph: Graph,
  catalog: readonly NodeManifest[],
  credentials: readonly CredentialView[],
): MissingCredential[] {
  const byKind = new Map(catalog.map((m) => [m.kind, m]));
  // Chi credential dang BAT moi tinh: tat mot tai khoan la cach nguoi dung noi "dung dung cai nay".
  const have = new Set(
    credentials.filter((c) => c.enabled).map((c) => `${c.provider}/${c.kind}`),
  );

  // `Node.kind` la mot union co the (`{ kind: "prompt", ... }`) chu khong phai chuoi — the nam
  // ben trong moi la `kind` dung de tra manifest.
  const counts = new Map<string, number>();
  for (const node of graph.nodes) {
    const tag = node.kind.kind;
    counts.set(tag, (counts.get(tag) ?? 0) + 1);
  }

  const out: MissingCredential[] = [];
  for (const [nodeKind, nodeCount] of counts) {
    const manifest = byKind.get(nodeKind);
    const spec = manifest?.credential;
    if (!manifest || !spec) {
      continue;
    }
    // Du MOT loai duoc chap nhan la coi nhu co — node tu chon loai nao dung duoc luc chay.
    const satisfied = spec.kinds.some((k) => have.has(`${spec.provider}/${k}`));
    if (!satisfied) {
      out.push({
        nodeKind,
        nodeLabel: manifest.labelVi,
        provider: spec.provider,
        kinds: spec.kinds,
        nodeCount,
      });
    }
  }
  // Thu tu on dinh (theo nhan) de danh sach khong nhay khi re-render.
  return out.sort((a, b) => a.nodeLabel.localeCompare(b.nodeLabel, "vi"));
}
