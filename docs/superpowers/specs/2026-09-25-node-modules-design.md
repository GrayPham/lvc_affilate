# Thiết kế — Node độc lập, kho template, kho tài khoản AI, Dola qua API app

> Ngày: 2026-09-25 · Nối tiếp: `2026-09-16-lvc-seedance-rust-design.md` (P0–P3c xong)
> Nguồn tham chiếu: `A:\ToolLVC\lvcseedance` (chỉ đọc) — `src/api/dola_app.py`, `dola_*_template.py`,
> `src/media/unwatermark.py`, `src/api/upscaledola/{dola_signer,dola_device,device_generator,image_cloaker}.py`
> Trạng thái: **đã duyệt 2026-09-25** (mục 12).

## 1. Mục tiêu

1. Canvas kiểu BytePlus Lumina: node ảnh/video/văn bản nối dây, panel node có chip tham chiếu,
   prompt, model, tỉ lệ, độ phân giải, thời lượng, số lượng (1×).
2. **Kho template**: tab *Public* (mẫu dựng sẵn, chỉ đọc) và *Private* (người dùng lưu), có tìm kiếm,
   nút "Dùng mẫu" → mở ra canvas.
3. **Node độc lập**: mỗi loại node là một module riêng (crate Rust + thư mục UI), chỉ biết
   *đầu vào / đầu ra / tham số / loại tài khoản cần*. Thêm node Gemini video, ChatGPT kịch bản…
   không được sửa node khác, không được sửa engine.
4. **Dola tạo ảnh + tạo video qua API app Android** (kênh `api` của Python — mặc định từ v0.59),
   không qua UI trình duyệt.
5. **Kho tài khoản AI**: Dola (cookie/phiên), Google, ChatGPT, kho API key (Gemini, OpenAI, …).

## 2. Hiện trạng (vấn đề phải giải)

- `NodeKind` là enum đóng (`lvc-core/src/graph/node.rs:36`). Thêm 1 kind = sửa ~10 chỗ ở 2 ngôn ngữ:
  `node.rs` (tag/inputs/outputs/supported/label), `validate.rs:235`, `plan.rs:122` (downcast kind
  thượng nguồn), `dag.rs:441` (`match Step`), `fixtures.rs`, `types.ts`, `ports.ts`, `store/graph.ts`,
  `Workflow.tsx`, `rules.ts`.
- `BrowserDriver` trộn phương thức trình duyệt chung với `submit`/`poll_and_download` riêng Dola video.
- Chưa có tạo ảnh, chưa có template, bảng tài khoản chỉ phục vụ Dola.

## 3. Quyết định kiến trúc

### ADR-R014 — Node là module độc lập, đăng ký tĩnh qua registry

Hợp đồng node nằm trong crate mới **`lvc-node-api`** (chỉ phụ thuộc serde, serde_json,
async-trait, tokio-util, thiserror). Mọi node chỉ phụ thuộc crate này (+ provider của chính nó).

```rust
pub struct NodeManifest {
    pub kind: &'static str,            // "dola_video" — khoá duy nhất, không đổi
    pub version: u32,                  // tăng khi đổi params → migrate(params)
    pub category: Category,            // Input | Text | Image | Video | Audio | Output
    pub label_vi: &'static str,
    pub inputs: Vec<PortSpec>,         // {name, ty, many, required, collect}
    pub outputs: Vec<PortSpec>,
    pub params_schema: serde_json::Value, // JSON Schema con: UI tự dựng form nếu không có panel riêng
    pub credential: Option<CredentialReq>, // {provider:"dola", kinds:[Session]} — None nếu không cần
    pub exec: ExecMode,                // PerItem (map ngầm theo từng dòng prompt) | Whole
}

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    fn manifest(&self) -> &NodeManifest;
    fn validate_params(&self, params: &Value) -> Result<(), Vec<ParamError>>; // thuần, không I/O
    fn plan(&self, params: &Value, inputs: &PlanInputs) -> Result<u32, Vec<ParamError>>; // số job, để báo "sẽ tạo N video"
    async fn run(&self, ctx: NodeCtx, params: Value, inputs: NodeInputs)
        -> Result<NodeOutputs, NodeError>;
}
```

**Dữ liệu chảy trên dây** — kiểu chung, không node nào biết kind của node thượng nguồn:

```rust
pub enum PortType { Text, Image, Video, Audio, Json }
pub enum Value { Text(TextItem), Image(Asset), Video(Asset), Audio(Asset), Json(serde_json::Value) }
pub struct TextItem { pub text: String, pub overrides: BTreeMap<String,String> } // ratio=, dur=… của dòng TXT
pub struct Asset {
    pub local_path: PathBuf,
    pub mime: String,
    pub remote: Option<RemoteRef>,     // {provider:"dola", uri:"tos-mya-i-…/rc_gen_image/x.jpeg", url}
    pub meta: BTreeMap<String, String> // width, height, duration, label ("character")…
}
pub type NodeInputs  = BTreeMap<String /*port*/, Vec<Value>>;
pub type NodeOutputs = BTreeMap<String /*port*/, Vec<Value>>;
```

`Asset.remote` cho phép ảnh Seedream do node Dola ảnh tạo ra được node Dola video gắn thẳng
bằng URI (không upload lại → né lọc chân dung, như `generate_scene_asset` của Python). Node
khác provider chỉ dùng `local_path`.

**`NodeCtx`** — thứ duy nhất node được chạm vào thế giới ngoài:

| Trường | Ý nghĩa |
|---|---|
| `run_id, node_id, job_index` | định danh |
| `cancel: CancellationToken` | huỷ |
| `events: EventSink` | `stage(name, detail)`, `progress(done,total)`, `log(level,msg)` — engine chuyển thành `EngineEvent` |
| `credentials: Arc<dyn CredentialLease>` | `lease(provider, kinds, prefer) -> Lease{id, secret, meta, proxy}`; trả lại khi drop; `report(outcome)` (ok / banned / quota) |
| `limits: Arc<dyn Throttle>` | slot đồng thời + khoảng cách gửi theo provider |
| `workdir: PathBuf` | thư mục tạm của run cho node này |
| `http: HttpFactory` | `reqwest::Client` dựng sẵn theo proxy của lease |

**Luật độc lập (cưỡng chế trong `scripts/check.ps1` bằng `cargo tree`)**:
1. `crates/nodes/*` chỉ được phụ thuộc `lvc-node-api`, `lvc-core` (thuần) và `crates/providers/*`.
2. Node không được phụ thuộc node khác, `lvc-store`, `lvc-engine`, `tauri`.
3. Provider không biết khái niệm node (dùng lại được từ CLI/test).
4. Engine không `match` theo kind — chỉ gọi `registry.get(kind)`.

Node lỗi/panic chỉ làm fail node đó (catch_unwind đã có ở engine), node cùng run nhánh khác
vẫn chạy nếu không phụ thuộc.

### ADR-R015 — Dola ảnh/video đi qua API app Android; trình duyệt thành backend dự phòng

Đảo ADR-R009 (mặc định browser). Lý do: Python v0.59 đã chạy mặc định `BROWSER_MODE="api"`
ổn định; không tốn RAM Chrome; tham chiếu ảnh upload/URI được; đo được mã lỗi rõ ràng.
Kênh browser hiện có (`lvc-browser::dola`) giữ lại làm backend `browser` của node `dola_video`
(param `backend: api|browser`, mặc định `api`), dùng khi gặp 710022004 captcha lặp.

### ADR-R016 — Template = graph JSON + metadata; Public nhúng trong binary

- Public: `crates/lvc-app/templates/*.json` (`include_str!`), seed vào DB khi khởi động với
  `scope='public'`, chỉ đọc; cập nhật theo `version`.
- Private: người dùng "Lưu thành mẫu" từ canvas; có ảnh thumbnail (PNG chụp canvas, ≤200KB).
- Template tham chiếu kind chưa có trong registry → vẫn hiển thị, node đó xám "chưa cài".
- Template không chứa bí mật: tham số `account` luôn là `auto` khi lưu.

### ADR-R017 — Kho tài khoản (vault) chung cho mọi provider, bí mật mã hoá DPAPI

Bảng `credentials` thay `accounts_meta` về mặt khái niệm; tài khoản Dola cũ được di trú sang
(profile Chromium vẫn nằm ở `accounts/<name>/`). Bí mật (cookie JSON, API key, mật khẩu, TOTP)
mã hoá bằng Windows DPAPI (`CryptProtectData`, phạm vi user) trước khi ghi DB; không bao giờ log,
không bao giờ gửi qua IPC ra UI (UI chỉ thấy `••••1234`).

## 4. Bố cục crate mới

```
crates/
  lvc-core/            THUẦN: graph chung (Node{id, kind:String, params:Value, x, y}), validate/topo
                       dựa trên trait ManifestLookup; TXT scenes; modes
  lvc-node-api/        hợp đồng node (mục 3) + FakeCtx cho test node
  lvc-vault/           trait CredentialLease impl: pool, lease, cooldown, DPAPI
  lvc-store/           + migration 0004 (credentials, templates, graph v2)
  lvc-engine/          DAG executor chung theo registry (bỏ enum Step)
  lvc-browser/         giữ nguyên (backend browser của dola_video)
  providers/
    lvc-dola/          client API app: session, signer, device register, upload, SSE, poll,
                       unwatermark, bảng model/ratio/duration, phân loại mã lỗi
  nodes/
    node-prompt/       prompt (TXT nhiều dòng → nhiều TextItem)
    node-image-upload/ ảnh tải lên (+ nhãn "character", "cloths"…)
    node-save/         lưu file ra thư mục
    node-dola-image/   Seedream 5.0 Pro / 4.5
    node-dola-video/   Seedance 2.5 / 2.0 / 1.0
    (sau) node-gemini-video/, node-chatgpt-script/, node-concat/, node-remove-logo/
  lvc-app/             Tauri: đăng ký node (1 dòng/node), commands, templates/*.json
```

### 4.1 Cấu trúc bắt buộc của một node (áp dụng cho mọi node, kể cả tương lai)

```
crates/nodes/node-dola-video/
  Cargo.toml           deps: lvc-node-api, lvc-dola (không gì khác trong workspace)
  CLAUDE.md            LUẬT của node: hợp đồng I/O, được/không được phụ thuộc gì, lệnh test
  MEMORY.md            ghi nhớ kỹ thuật: quirk của provider, mã lỗi đã gặp, quyết định nhỏ
  README.md            mô tả cho người dùng: đầu vào, đầu ra, tham số, chi phí credit
  manifest.json        sinh từ code bằng test (LVC_WRITE_FIXTURES=1), UI đọc để ghim hợp đồng
  src/lib.rs           pub fn node() -> Arc<dyn NodeExecutor>
  src/params.rs        struct Params + JSON Schema + migrate(version)
  src/run.rs           logic chạy
  tests/contract.rs    manifest khớp manifest.json; chạy với FakeCtx + provider giả (wiremock)
app/src/nodes/dola_video/
  index.ts             đăng ký UI: { kind, Panel?, Preview?, icon }
  Panel.tsx            panel riêng kiểu Lumina (tuỳ chọn — không có thì dùng form sinh từ schema)
```

`scripts/new-node.ps1 <kind>` sinh khung trên từ template để node mới luôn đủ file.

## 5. Node đợt đầu

| kind | Vào | Ra | Params chính | Tài khoản |
|---|---|---|---|---|
| `prompt` | — | `out: Text*` | text (TXT nhiều dòng, cú pháp `\|` như cũ) | — |
| `image_upload` | — | `out: Image*` | paths[], label | — |
| `dola_image` | `prompt: Text` (bắt buộc), `refs: Image*` | `out: Image*` | model (seedream_5.0_pro, seedream_4.5), ratio (8 giá trị), count | dola/Session |
| `dola_video` | `prompt: Text*` (bắt buộc), `refs: Image*` | `out: Video*` | model (2.5/2.0/1.0), ratio, duration (5/10/15/30 theo model), resolution 1080p, mode (fast/agent), logo, count, backend (api/browser), cloak_refs | dola/Session |
| `save` | `in: Image* | Video*` (cổng Any-media) | — | out_dir, pattern tên file | — |

Chip tham chiếu trên panel `dola_video` = danh sách cạnh vào cổng `refs`, hiển thị `Asset.meta.label`
(= nhãn node nguồn, vd "character", "cloths", "package"); bấm × là xoá cạnh.

Map ngầm giữ nguyên ngữ nghĩa P3: `dola_video` khai `exec: PerItem` trên cổng `prompt` → mỗi
`TextItem` là một job, `refs` dùng chung cho mọi job; `count` nhân thêm.

## 6. Provider `lvc-dola` (port từ `dola_app.py`)

| Phần | Nguồn Python | Ghi chú port |
|---|---|---|
| `AppSession` từ cookies | `dola_app.py:263-383` | query common (aid 489823, JP/ja), header UA Cronet, **`Accept-Encoding: gzip, deflate` — cấm ttzip** |
| Ký RC4 X-Gorgon/Khronos | `sign()` L205-259 | thuần, test bằng vector chụp từ Python |
| Ký thật + đăng ký thiết bị | `upscaledola/dola_signer.py`, `dola_device.py`, `device_generator.py`, `x_signatures` | xem câu hỏi mở Q1 |
| Upload ảnh | `upload_image` L405-465 | prepare_upload → ApplyImageUpload (AWS4, region `sdwdmwlll`, service `imagex`) → upload/v1 (CRC32) → resource/register (gzip) |
| Cloak ảnh tham chiếu | `image_cloaker.py` | bỏ EXIF, nhiễu pHash; crate `image` |
| Payload action-bar | `build_video_payload` L506, `build_image_payload` L702, template JSON | template nhúng nguyên văn, thay option đã chọn |
| Payload agent/pro | `build_text_payload` L549, `modes.agent_prompt` | |
| SSE `chat/completion` | `_sse` L618 | SSE_ACK / STREAM_* / FULL_MSG_NOTIFY / STREAM_ERROR |
| Poll `im/chain/single` cmd 3100 | `poll_video` L1025, `poll_images` | 10s/lần; auto-reply agent ≤3 |
| Bỏ watermark | `media/unwatermark.py` | fallback_api + giải mã `qAAB` AES-CBC; allow-list host |
| Mã lỗi | L940 | 710022002 block · 710022013 tạm · 710022004 captcha · 710010202 chữ ký · 710012001 hết phiên → `NodeError{code, retryable}` và `lease.report()` |

Khoảng cách gửi 8s/tài khoản (`API_SEND_GAP`) do `Throttle` của ctx đảm nhiệm, không nằm trong node.

## 7. Kho tài khoản (tab "Tài khoản AI")

```sql
CREATE TABLE credentials (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,        -- dola | google | openai | gemini | chatgpt_web | …
  kind TEXT NOT NULL,            -- session | api_key | login (email+mật khẩu+TOTP)
  label TEXT NOT NULL,
  secret BLOB NOT NULL,          -- DPAPI
  meta TEXT NOT NULL DEFAULT '{}', -- email, device.json (dola), profile_dir, base_url, model mặc định
  proxy_id TEXT REFERENCES proxies(id),
  status TEXT NOT NULL DEFAULT 'unknown', -- ok | expired | banned | quota | unknown
  enabled INTEGER NOT NULL DEFAULT 1,
  daily_limit INTEGER, cooldown_until TEXT, last_used_at TEXT, last_verified_at TEXT,
  created_at TEXT NOT NULL
);
```

- UI: nhóm theo provider (Dola · Google · ChatGPT · API key), thêm / nhập cookie / dán key /
  kiểm tra / bật-tắt / gắn proxy. Trang "Tài khoản" Dola hiện tại chuyển thành một nhóm trong đây.
- Mỗi provider khai một `Verifier` (Dola: gọi nhẹ API kiểm phiên; OpenAI/Gemini: list models).
- Node chỉ khai `CredentialReq`; param `account` = `auto` hoặc id cụ thể. Pool chọn theo LRU +
  hạn mức ngày + cooldown (giữ ADR-R004).

## 8. UI

- **Canvas** (giữ React Flow): nền tối lưới chấm; node hiển thị ảnh/video kết quả làm thân node;
  nhãn trên đầu; thanh công cụ nổi khi chọn node (tải xuống, phóng to, chạy lại node).
- **Menu thêm node** (nút + và chuột phải): nhóm *Thêm node* sinh từ `node_catalog()`; nhóm
  *Thêm tài nguyên*: Tải lên, Chọn từ thư viện.
- **Panel node** mở dưới node khi chọn (kiểu ảnh Lumina): chip tham chiếu, prompt, thanh tham số.
- **Kho template**: nút ở thanh trái; modal Public/Private, lưới thẻ có thumbnail, tìm kiếm,
  hover "Dùng mẫu".
- Registry UI: `app/src/nodes/registry.ts` gom `import.meta.glob('./*/index.ts')` — thêm node UI
  không phải sửa file chung.

## 9. Commands Tauri mới/đổi

| Command | Ghi chú |
|---|---|
| `node_catalog() -> Vec<NodeManifest>` | UI dựng palette/form/luật nối từ đây — bỏ bảng `ports.ts` chép tay |
| `workflow_validate/save/get/run_*` | graph v2; v1 tự migrate khi đọc (chỉ `video`→`dola_video`; `prompt`/`image_upload`/`save` giữ tên — P4a) |
| `template_list(scope, q)`, `template_get`, `template_save_private`, `template_delete_private` | |
| `credential_list(provider?)`, `credential_add`, `credential_update`, `credential_verify`, `credential_delete` | bí mật chỉ đi vào, không đi ra |
| `node_rerun(runId, nodeId)` | chạy lại 1 node với đầu vào cũ (dùng output đã lưu) |

Sự kiện: giữ `engine://event`; thêm `NodeOutput{run_id, node_id, port, value}` để thân node hiện
ảnh/video ngay khi có.

## 10. Lộ trình

| Giai đoạn | Phạm vi | Nghiệm thu |
|---|---|---|
| **P4a** | `lvc-node-api`, graph v2 chung, engine theo registry, chuyển 4 node cũ thành module (prompt, image_upload, dola_video[backend browser hiện có], save); UI registry + `node_catalog` | toàn bộ test cũ xanh; workflow v1 mở được; tab Kịch bản chạy như cũ (FakeDriver) |
| **P4b** | `lvc-dola` (API app) + `node-dola-image` + backend `api` của `node-dola-video` | wiremock đủ luồng; 1 ảnh + 1 video thật (hỏi trước vì tốn credit) |
| **P4c** | `lvc-vault` + migration credentials + tab Tài khoản AI (Dola, Google, ChatGPT, API key) | di trú tài khoản Dola cũ; DPAPI round-trip; UI không bao giờ nhận bí mật |
| **P4d** | Kho template + canvas/panel kiểu Lumina | 4 mẫu Public; lưu/xoá Private; "Dùng mẫu" chạy được |
| P5+ | node Gemini video, ChatGPT kịch bản, concat, xoá logo | mỗi node = 1 crate + 1 thư mục UI, không sửa file khác ngoài 1 dòng đăng ký |

Mẫu Public đợt đầu: *Prompt → Video Dola*; *UGC unboxing (nhân vật + trang phục + sản phẩm → video)*;
*Seedream tạo ảnh cảnh → video*; *Kịch bản nhiều dòng → nhiều video → lưu*.

## 11. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Refactor graph phá P3 | P4a không đổi hành vi; test DAG cũ chạy lại trên engine mới trước khi xoá code cũ |
| Chữ ký/thiết bị sai → 710022002 khoá tài khoản | test vector so với Python; chạy thật đầu tiên trên 1 tài khoản phụ |
| Dola đổi template action-bar | template là dữ liệu nhúng, cập nhật không đổi code; `list_models()` để kiểm |
| DPAPI gắn với user Windows → chuyển máy mất bí mật | nút xuất/nhập kho có mật khẩu (AES-GCM + Argon2) |

## 12. Quyết định đã chốt (2026-09-25)

- **Q1 → port đủ**: ký thật (X-Argus/Gorgon/Ladon) + đăng ký thiết bị theo từng tài khoản, lưu
  `device.json` trong `meta` của credential. RC4 giữ làm đường ký dự phòng như Python.
- **Q2 → giữ browser làm dự phòng**: `dola_video.backend = api` mặc định; `browser` dùng `lvc-browser::dola` hiện có.
- **Q3 → cả API key lẫn đăng nhập**: kind `api_key` (Gemini, OpenAI…) và `login` (email/mật khẩu/TOTP + cookie/profile).
- **Thứ tự**: P4a → P4b → P4c → P4d.
