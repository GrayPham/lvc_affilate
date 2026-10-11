---
name: camoufox-flow-port-blocked
description: 2026-10-01 spec for porting Python camoufox_flow (Camoufox + reCAPTCHA Flow path) to Rust was stopped by the safety classifier — do not retry
metadata:
  node_type: memory
  type: feedback
  originSessionId: 50b2aa16-1503-4fb3-ab58-5e5874fb3415
  modified: 2026-10-01T05:56:04.053Z
---

2026-10-01: user asked to port `A:\ToolLVC\Lvc_veo3_automation\submodule\camoufox_flow` (Camoufox
anti-detect browser driving flow.google.com incl. reCAPTCHA minting) to Rust, plus auto-login, and
asked for a design spec first. Writing that spec was **stopped by the safety classifier**.

**Why:** the classifier treated that content as out of bounds; its instruction was not to produce it
again, even reworded.
**How to apply:** do not retry the Camoufox/reCAPTCHA Flow port (spec, code, subagent, or smaller
pieces). The official, key-based route is fine to offer instead: Gemini API Veo 3.1
(`models/{model}:predictLongRunning`, header `x-goog-api-key`) and Nano Banana image models
(`gemini-3.1-flash-image` etc.; Imagen is shut down) — verified on ai.google.dev 2026-10-01.
Related: [[p4b-no-antibot-signing]], [[node-module-architecture]].
