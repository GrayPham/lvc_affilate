---
name: google-media-owned-by-peer
description: 2026-10-01 Google official-API media nodes (Veo video + Nano Banana image) are owned by parallel session lvc-seedancev2-7a via crate lvc-google; do not rebuild
metadata:
  node_type: memory
  type: project
  originSessionId: 50b2aa16-1503-4fb3-ab58-5e5874fb3415
  modified: 2026-10-01T10:23:08.318Z
---

2026-10-01: user ran a second Claude session (lvc-seedancev2-7a) in parallel on the same repo that
built the OFFICIAL-API media nodes: provider `crates/providers/lvc-google` (Veo
`:predictLongRunning` + image) + `node-google-image`/`node-google-video` + engine wiring
(`apimedia.rs`) + frontend + plan `docs/superpowers/plans/2026-10-01-p6-official-api-nodes.md`. That
session also owns Seedance (`lvc-seedance`) and Grok (`lvc-grok`).

This session had started a duplicate `crates/providers/lvc-gemini-media` (Nano Banana + Veo, spec
`docs/superpowers/specs/2026-10-01-gemini-api-media-nodes-design.md`). To avoid collision it was
**removed** (dropped from Cargo.toml members+deps, dir deleted); the empty crate had been breaking
the workspace for both sessions.

**Why:** one owner per crate; the peer was far ahead and tested. **How to apply:** don't recreate
lvc-gemini-media or any Google/Seedance/Grok media node — edit `lvc-google` etc. only in
coordination with that session. The spec doc was kept as reference (verified API facts).

Verified API fact handed to the peer (ai.google.dev, 2026-10-01): **Imagen is shut down on the
Gemini Developer API** ("Imagen models are shut down. Use Nano Banana") — image gen must use Nano
Banana `gemini-3.1-flash-image` via `:generateContent` (response at
`candidates[0].content.parts[].inlineData.data`), unless on Vertex AI where Imagen may still live.
Related: [[node-module-architecture]], [[camoufox-flow-port-blocked]].
