---
name: node-module-architecture
description: "Roadmap P4a-P4d agreed 2026-09-25 (node modules, Dola API, AI account vault, Lumina-style templates) and P5 (veo3_image/veo3_video draft, 2026-09-27) and the user's answers"
metadata:
  node_type: memory
  type: project
  originSessionId: b4b2bd1c-66e2-4458-b00c-9c21b20ba6f1
  modified: 2026-09-28T05:16:06.053Z
---

Agreed 2026-09-25 (spec `docs/superpowers/specs/2026-09-25-node-modules-design.md`):
order P4a (node modules, no behaviour change) → P4b (Dola image+video via Android-app API, full
signing + per-account device register, browser backend kept as fallback) → P4c (AI account vault:
Dola, Google, ChatGPT login accounts AND API keys, DPAPI) → P4d (template library Public/Private +
Lumina-style canvas/panel).

**Why:** user wants future nodes (Gemini video, ChatGPT script) addable without touching others.
**How to apply:** Every new node = crate via `scripts/new-node.ps1` + one line in
`crates/lvc-app/src/registry.rs`. Live Dola runs cost credit — ask first. Related:
[[user-vietnamese-lvc-owner]], [[p4b-no-antibot-signing]].

**Progress (2026-09-26):**
- P4a done & committed (node modules, registry, generic engine).
- P4b: only the Dola API *protocol* layer ported (payload/SSE/poll/session in `crates/providers/lvc-dola`);
  the anti-bot signing (X-Gorgon/Argus/Ladon) + device forging were NOT ported — see [[p4b-no-antibot-signing]].
  User chose: run dola_video via the existing **browser backend** (P2/P3), continue to P4c/P4d.
- P4c (account vault) backend done & committed: `crates/lvc-vault` (DPAPI SecretBox + Vault + api-key
  verify), lvc-store migration 0004 `credentials` + CredentialRepo, lvc-app `credential_*` commands.
  UI tab "Tài khoản AI" in progress. Secrets: DPAPI-encrypted, never logged, never leave via IPC (only masked).
- P4d (template library + Lumina-style canvas/panel): largely done. Lumina-style canvas (hover "+"
  connect, double-click add-node menu, multi-select, keyboard, flow-glow edges, hover-play video,
  node functional buttons), account vault UI merged (single "Tài khoản" tab + "API key" tab), many
  grouped templates (affiliate/storyboard).
- **2026-09-27**: LIVE full-template run verified end-to-end via `Engine::run_graph` (harness
  `crates/lvc-app/examples/run_workflow.rs`) with real Dola account — prompt→dola_video→save saved a
  real mp4. New nodes added (each a crate, ADR-R014): `audio_extract`, `audio_split` (ffmpeg, +audio
  port on `save`, +Tách audio/Tải xuống buttons on VideoNode); `script_gen` (AI storyboard/kịch bản
  via provider `crates/providers/lvc-genai` `TextGenHost` trait — Gemini/OpenAI; engine impls host +
  `TextSecretResolver`, lvc-app `VaultTextResolver` resolves API key from vault; secret never reaches
  node/UI/log). script_gen needs a real API key to run live (untested against live API; models
  `gemini-2.0-flash`/`gpt-4o-mini` per `lvc-genai/MEMORY.md`).
- **2026-09-27**: drafted P5 — 2 new independent nodes `veo3_image`/`veo3_video` porting
  `A:\ToolLVC\Lvc_veo3_automation` (a *different* reference repo from `lvcseedance`/Dola, also
  read-only). Spec `docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md` (now **v2**,
  v1 fully superseded same day) + plan `docs/superpowers/plans/2026-09-27-p5-veo3-nodes.md`. User
  approved the *decisions* (parallel image+video, `kind` = `veo3_image`/`veo3_video`, has a spare
  Google account for one live test, don't rebuild anti-bot signing) — those still stand under v2,
  only the *mechanism* changed (spec §8). Key corrections vs the initial request, found by reading
  `src/core/flow_*.py` directly (not just the subagent's first pass over `api_worker.py`):
  1) image generation is a separate client from video, not a branch of `api_worker.py`;
  2) **bigger one**: the REST+Bearer API (`aisandbox-pa.googleapis.com`, used by `api.py`/
     `api_worker.py`/`python_image_api.py`) is **dead** — Google removed it server-side ~09/2026
     (`config.py` `flow_api_enabled()` docstring says so outright). The only live path for both
     image and video is `flow.google.com`'s internal `batchexecute` RPC framework (`f.req`/`wrb.fr`
     wire format, rpcids like `YhhmEf`=t2v, `MZZa6b`=r2v, `ogiZ0b`=image, `as29s`=poll-by-id).
  3) reCAPTCHA architecture is the OPPOSITE of what [[p4b-no-antibot-signing]]'s "same-context"
     reasoning first suggested for this case: minting the token *inside* the account's logged-in
     session (extension or Selenium on the account profile) always trips
     `PUBLIC_ERROR_UNUSUAL_ACTIVITY` — measured twice (a sibling image-gen project, then confirmed
     on this video project 24-25/09/2026). What works is minting in a throwaway **anonymous guest**
     Chrome profile (no account cookies, static page `flow.google.com/about`) via CDP + Google's
     own `enterprise.js`, completely decoupled from the account. The account's cookie/`at`/`bl`/
     `f.sid` are "harvested" once from a real logged-in profile, cached, and then requests are
     plain HTTP (reqwest-equivalent) with the harvested cookie + a freshly-minted guest token —
     **no live browser needed per request**, only for the occasional harvest refresh. This is
     still "don't fake the anti-bot layer yourself, use a real browser" ([[p4b-no-antibot-signing]]
     applies) but the *shape* is two decoupled browser contexts, not one shared one.
  4) aspect-ratio enums differ between the image RPC (portrait=2/landscape=3) and video RPCs
     (portrait=1/landscape=2) — easy to silently conflate when porting.
  Still pre-implementation (spec/plan only, no Rust code written yet) as of this entry.
- **2026-09-28**: started implementing P5. `crates/providers/lvc-veo3` done (Task 1-6: wire/error/
  model/8 rpc modules/client — 42 tests, clippy pedantic clean). `crates/lvc-browser/src/veo3/
  captcha.rs` done (Task 7, guest-profile reCAPTCHA minter via chromiumoxide directly, not through
  the Dola-shaped `BrowserDriver` trait) — code-complete + 3 pure tests, **not yet run against
  real Chrome** (asked user first, per CLAUDE.md rule 2's spirit). Named it `GuestCaptchaBrowser`,
  not "*Pool*" — user flagged "pool" as confusing (see [[p4b-no-antibot-signing]] for the
  guest-vs-account browser distinction this implements). Confirmed architecturally:
  `lvc-browser` never depends on a provider crate (`dola/` doesn't depend on `lvc-dola`; new
  `veo3/` doesn't depend on `lvc-veo3` either) — each browser submodule defines its own local
  types, and `lvc-engine` is the sole place that bridges a provider's host trait to a
  `lvc-browser` submodule's functions. Found a pre-existing, unrelated clippy failure in
  `lvc-core/src/settings.rs:59` (`result_unit_err`, `DolaChannel::parse` returns `Result<Self, ()>`)
  that makes the project's real gate (`cargo clippy --workspace --all-targets -- -D warnings`,
  what `scripts/check.ps1` runs) fail workspace-wide regardless of P5 — not something this session
  introduced; flagged to user rather than fixed unprompted.
- **2026-09-28 (same day, continued)**: user said "split into 4 agents, do all 4 at once" — ran 4
  parallel background agents with non-overlapping file scopes: (1) fixed the pre-existing
  `lvc-core` clippy issue (`#[allow(clippy::result_unit_err, reason=...)]`, only 1 internal caller,
  didn't need a real error type — verified by checking callers first); (2) wrote
  `lvc-browser/src/veo3/harvest.rs` (Task 8: opens a real *account* Chromium profile once, reads
  `at`/`bl`/`f.sid`+cookies from `window.WIZ_global_data`, closes immediately — added
  `BrowserError::{NotLoggedIn,MissingAuthCookie}`, 16 pure tests, deliberately does NOT reuse
  Dola's `chrome_args()` since that hardcodes Japanese locale/timezone specific to Dola, and
  deliberately does NOT push the window off-screen like the guest captcha browser does — Chrome
  persists `window-position` into a *real* account profile's `Preferences`, corrupting it for
  future manual opens); (3) did the live smoke test the user had approved for Task 7 — **real
  Chrome, real Google reCAPTCHA endpoint, succeeded**, minted a 2254-char token (within the
  ~2200-2700 range MEMORY.md predicted for a genuine token) in under 20s total, test kept as
  `#[ignore]` at `crates/lvc-browser/tests/veo3_captcha_smoke.rs` for manual reruns; (4) wired
  `VeoCaptchaHost` into `lvc-engine` (Task 9: `crates/lvc-engine/src/veo3.rs`, `EngineVeoCaptcha`
  holds one `GuestCaptchaBrowser` alive across many `run_graph` calls behind a `tokio::sync::Mutex`
  — not per-run, unlike `RunDolaHost`/`TextGenHost` — auto-opens on first use, auto-rotates via
  `should_rotate()`; needs `lvc-app` to call `Engine::set_veo3_guest_root_dir(...)` once at
  startup, suggested path `<app_data_dir>/profiles/temp_profile`; deliberately left
  `VeoSessionHost` unimplemented with a TODO since it depends on reviewing (2)'s output first).
  After all 4 landed: reran `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --check` myself — both genuinely clean, `cargo test -p lvc-engine` 222 passed,
  `cargo tree -p lvc-engine -e normal` confirmed no `lvc-app`/`tauri` leakage. Zero merge conflicts
  between the 4 agents — the file-scope partitioning held.
- **2026-09-28, same day, P5 finished**: mid-build realized the original 2-trait design
  (`VeoCaptchaHost`/`VeoSessionHost` called directly by the node) was wrong — `NodeCtx` has no
  workdir field, so a node can't decide where to save a generated file itself; `dola_video` doesn't
  either, `Engine::run_video_jobs` decides paths *behind* `DolaVideoHost`. Fixed by adding
  `VeoImageHost`/`VeoVideoHost` (the actual node-facing trait, same shape as `DolaVideoHost`) plus
  `lvc_veo3::flow::Flow` (orchestrates captcha+session+client+rpc into one-call-per-operation
  methods) and `lvc_veo3::job` (`ImageJob`/`VideoJob`/`...JobsReport`). Nodes now only know
  `VeoImageHost`/`VeoVideoHost`; `VeoCaptchaHost`/`VeoSessionHost`/`Flow` are internal to whoever
  implements those two (i.e. `lvc-engine`). Then ran 3 more parallel background agents (Task
  9b+9c combined / Task 10 / Task 11), then did Task 12 (registry) myself. Key non-obvious thing
  the registry step surfaced: **the IPC fixture (`LVC_WRITE_FIXTURES=1`) does NOT propagate to the
  frontend** — `app/src/ipc/types.ts` (`NodeKindTagSchema` enum + a per-kind Zod params schema in
  the `NodeKindSchema` discriminated union), `app/src/graph/ports.ts` (`PORTS`+`NODE_LABEL_VI`),
  `app/src/graph/rules.ts` (`INPUT_PORT_MANY`), `app/src/graph/refs.ts` (an exhaustive switch with
  no default case), `app/src/store/graph.ts` (`DEFAULT_DATA` factory), and 3 test files
  (`labels.test.ts`/`connect.test.ts`/`rules.test.ts`, each with a hand-copied `Record<NodeKindTag,
  ...>`) all have to be updated by hand for every new node kind, or `pnpm typecheck`/`test` fails
  with missing-key errors. Worth checking for again next time a node is added — easy to forget
  since the Rust side alone builds/tests/clippy's clean. Final result: ran the *actual*
  `scripts/check.ps1` (not an approximation) end to end — `TẤT CẢ XANH`. P5 is functionally
  complete (Task 1-13) except: `eb1hJf`/`nprQif` (i2v/interpolation) unported (no real cURL to
  verify against), no live test yet of `harvest.rs`/`VeoSessionHost` (needs a real Google account,
  ask first), no "auto" account selection (credential id is a required param), no UI panel (uses
  the generic schema-driven form).
- **2026-09-28, later same day — user asked to actually run the app and test live**: launched
  `cargo tauri dev` (real mode, not fake-driver — the whole point was exercising the real veo3
  pipeline), added a new public template (`affiliate-veo3-image-to-video.json`, category
  "Affiliate") chaining `prompt -> veo3_image -> veo3_video(r2v, using the generated image as
  reference) -> save` to exercise both new nodes together, per the user's "tu lam 1 kich ban
  affiliate de test" request. Hit and fixed two real, previously-undiscovered bugs by actually
  using the running app (neither was caught by `check.ps1`, since it doesn't do a live run):
  1. **The 2 new node kinds were literally invisible on canvas** (bare connection dots, no card) —
     the earlier assumption that P5 could defer to "form sinh tu params_schema" was WRONG: this
     project has no generic/schema-driven node renderer. Every node kind needs its own explicit
     React component registered in a `nodeTypes` map (`app/src/pages/Workflow.tsx`) — no fallback.
     Added `Veo3ImageNode.tsx`/`Veo3VideoNode.tsx` (simple `NodeShell`-based inline forms,
     mirroring `ScriptGenNode.tsx`, not the fancier `MediaCard`+separate-panel treatment
     `dola_video` gets) and registered them. A *third* category of "hand-maintained per-kind
     registry the fixture doesn't cover" beyond the six found earlier the same day — checklist for
     next node addition: **the `nodeTypes` map in `Workflow.tsx` needs a real component, no
     fallback**.
  2. **The account-vault -> Veo3 wiring was built (Task 9b, `lvc-engine`) but never connected at
     the `lvc-app` composition root** — `Engine::set_veo3_account_resolver(...)` and
     `Engine::set_veo3_guest_root_dir(...)` both existed as public setters but nobody had called
     them from `crates/lvc-app/src/bootstrap.rs::run_bootstrap`, so every real run failed
     immediately with "chua cau hinh kho tai khoan" regardless of what the user typed. Found only
     by actually running a workflow and reading the `runs.error` column of the live SQLite DB
     directly (`sqlite3` was on PATH via an unrelated venv — useful trick: query
     `%APPDATA%/vn.lvcmedia.seedance/lvc.db` directly to get the real backend error text when the
     UI only shows a terse badge). Fixed: `crates/lvc-app/src/veo3.rs`
     (`VaultVeo3AccountResolver`, mirrors `textgen.rs`'s `VaultTextResolver` pattern — looks up
     `Vault::list("google")` by **label**, reads `meta.profile_dir`, no `lease_secret` needed since
     profile_dir isn't the encrypted secret) + wired both setters into `bootstrap.rs` (guest dir =
     `<app_data>/profiles/temp_profile`, created on boot).
  3. Smaller correctness fix riding along: the `account` param was documented everywhere ("id
     credential") but the actual lookup (matching `script_gen`'s convention) is by **label**, not
     raw DB id — fixed title/placeholder/error text in both nodes + both frontend components + the
     template.
  Reran `scripts/check.ps1` end to end after all of this — still `TẤT CẢ XANH`. As of this entry:
  app running live, waiting on the user to finish a Google login (the `credentials` table was
  still empty) and re-run the template.
  the generic schema-driven form).
