# Node `tts` - luat

Hop dong chung: `crates/lvc-node-api` | Luat repo: `CLAUDE.md` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, `lvc-tts` (provider, + thu vien ngoai). **Cam** node khac, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri`.

## Hop dong vao/ra
- Vao: `prompt: Text*` (bat buoc) — moi `TextItem` noi vao la MOT cau can doc, map thanh MOT job
  (giong het `museai_video`/`grok_image`). Dung lai `PromptField` phia UI (go tay tu tao node
  Prompt noi vao cong `prompt`).
- Ra: `out: Audio*` — moi job sinh MOT file audio (mp3).
- Tai khoan: `credential = { provider: "elevenlabs", kinds: ["api_key"] }` (hint tinh — MiniMax
  cung dung kind `api_key`, provider that chon o tham so `provider`, giong cach `script_gen` khai
  co dinh "gemini" du provider co the la "openai").

## Tham so
`provider` ("elevenlabs"|"minimax", mac dinh "elevenlabs"), `voice_id` (bat buoc, tu do theo tung
nha cung cap — KHONG hardcode danh sach giong vi catalog doi lien tuc), `model` (rong = mac dinh
cua provider), `speed` (mac dinh 1.0), `credential` (ten API key trong kho; rong = tu chon key BAT
dau tien cua provider do).

## Luat rieng
- `preview`/`run` deu goi `validate` chung (kiem `provider` hop le + `voice_id` khong rong) TRUOC
  khi dung `build_jobs` — loi tham so bao cung mot cho, khong lap code kiem rieng tung ham.
- `build_jobs` THUAN (khong I/O): moi `TextItem` tren cong `prompt` -> mot `lvc_tts::TtsJob`,
  `scene_index` 1..N lien tuc.
- `run()` lay `Arc<dyn lvc_tts::TtsHost>` tu `ctx.services`, goi `generate_audios`, map
  `done`/`failed` y het `node-museai-video::run`. Thieu service -> `NodeError::fatal` (dung ca
  run); loi API tung job -> vao `report.failed`, KHONG chan job khac, KHONG lam fatal ca node.
- Node KHONG tu goi mang, KHONG giu API key — chi truyen `credential_id` (ten/label) trong moi job.

## Kiem tra
- `cargo test -p node-tts` (dung FAKE `TtsHost`, KHONG goi API that).
- Doi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-tts` roi sinh lai fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
