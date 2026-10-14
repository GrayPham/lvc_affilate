// `missingCredentials` (src/graph/credentials.ts) — canh bao "thieu tai khoan" TRUOC khi them mot
// workflow vao hang cho, thay vi de nguoi dung phat hien luc 3 gio sang.
import { describe, expect, it } from "vitest";

import { missingCredentials } from "../src/graph/credentials";
import type { CredentialView, Graph, NodeManifest } from "../src/ipc/types";

function manifest(kind: string, labelVi: string, provider?: string, kinds?: string[]): NodeManifest {
  return {
    kind,
    version: 1,
    category: "video",
    labelVi,
    inputs: [],
    outputs: [],
    defaultParams: {},
    paramsSchema: {},
    ...(provider ? { credential: { provider, kinds: kinds ?? ["api_key"] } } : {}),
  };
}

const CATALOG: NodeManifest[] = [
  manifest("prompt", "Prompt"),
  manifest("google_video", "Google video", "gemini", ["api_key"]),
  manifest("veo3_video", "Veo3 video", "google", ["login"]),
  manifest("museai_video", "MuseAI video", "museai", ["login", "session"]),
];

function cred(provider: string, kind: string, enabled = true): CredentialView {
  return {
    id: `${provider}-${kind}`,
    provider,
    kind,
    label: provider,
    secretHint: "••••1234",
    meta: {},
    proxyMasked: "",
    status: "ok",
    enabled,
    lastUsedAt: 0,
    lastVerifiedAt: 0,
  } as CredentialView;
}

function graph(kinds: string[]): Graph {
  return {
    version: 2,
    // `Node.kind` la union co the ben trong — xem `NodeSchema` (src/ipc/types.ts).
    nodes: kinds.map((kind, i) => ({ id: `n${i}`, kind: { kind }, x: 0, y: 0 })),
    edges: [],
  } as unknown as Graph;
}

describe("missingCredentials", () => {
  it("du tai khoan -> khong thieu gi", () => {
    const out = missingCredentials(graph(["google_video"]), CATALOG, [cred("gemini", "api_key")]);
    expect(out).toEqual([]);
  });

  it("thieu -> bao dung provider va nhan node", () => {
    const out = missingCredentials(graph(["google_video"]), CATALOG, []);
    expect(out.length).toBe(1);
    expect(out[0]!.provider).toBe("gemini");
    expect(out[0]!.nodeLabel).toBe("Google video");
    expect(out[0]!.kinds).toEqual(["api_key"]);
  });

  it("node khong can tai khoan khong bao gio bi bao thieu", () => {
    expect(missingCredentials(graph(["prompt", "prompt"]), CATALOG, [])).toEqual([]);
  });

  it("gop theo kind: ba node cung loai ra MOT dong voi nodeCount=3", () => {
    const out = missingCredentials(
      graph(["veo3_video", "veo3_video", "veo3_video"]),
      CATALOG,
      [],
    );
    expect(out.length).toBe(1);
    expect(out[0]!.nodeCount).toBe(3);
  });

  it("credential bi TAT khong tinh la co", () => {
    const out = missingCredentials(
      graph(["google_video"]),
      CATALOG,
      [cred("gemini", "api_key", false)],
    );
    expect(out.length).toBe(1);
  });

  it("dung sai LOAI bi mat van tinh la thieu", () => {
    // Node can `api_key` cua gemini; kho chi co `session` cua gemini -> van thieu.
    const out = missingCredentials(graph(["google_video"]), CATALOG, [cred("gemini", "session")]);
    expect(out.length).toBe(1);
  });

  it("node chap nhan NHIEU loai: co MOT loai la du", () => {
    const out = missingCredentials(graph(["museai_video"]), CATALOG, [cred("museai", "session")]);
    expect(out).toEqual([]);
  });

  it("node khong co trong catalog bi bo qua, khong doan bua", () => {
    expect(missingCredentials(graph(["kind_la_khong_biet"]), CATALOG, [])).toEqual([]);
  });

  it("nhieu nhu cau thieu -> sap theo nhan, thu tu on dinh", () => {
    const out = missingCredentials(graph(["veo3_video", "google_video"]), CATALOG, []);
    expect(out.map((m) => m.nodeLabel)).toEqual(["Google video", "Veo3 video"]);
  });
});
