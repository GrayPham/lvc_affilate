# Node `script_agent` — luật

Hợp đồng chung: `crates/lvc-node-api` | Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Khác biệt với `script_gen` (đọc trước khi sửa)

Hai node TRÙNG hợp đồng vào/ra và cơ chế gọi `TextGenHost`, chỉ khác Ở CHỖ: node này KHOÁ sẵn một
chỉ dẫn hệ thống CHUYÊN cho kịch bản AFFILIATE MARKETING (hook mở đầu, nêu lợi ích/điểm đau, CTA
cuối — `src/prompt.rs::build_system`) — người dùng KHÔNG thể thay thế toàn bộ chỉ dẫn hệ thống như
`script_gen` (field `system` của `script_gen` non-rỗng sẽ THAY THẾ mặc định); ở đây field tương ứng
tên `extra_instructions` chỉ NỐI THÊM vào cuối nền affiliate, không bao giờ xoá nền đó — đây CHÍNH
LÀ lý do tồn tại một node riêng (theo yêu cầu người dùng, không chọn "thêm cờ affiliate" vào
`script_gen` chung).

Yêu cầu gốc người dùng (2026-10-13): "Thêm 1 node mới là agent kịch bản — (1) Model AI nào hỗ trợ
(trong kho có api key nền tảng nào thì hiện nền tảng đó — ĐÃ loại Claude, chỉ Gemini/OpenAI qua
`api_key`), (2) Prompt đầu vào qua cổng text, (3) Dữ liệu trả ra, (4) Chuyên sinh kịch bản
affiliate". Phần (1) "liệt kê động theo kho credential" là việc của FRONTEND
(`app/src/nodes/ScriptAgentNode.tsx` đọc `useCredentialsStore`, lọc provider ∈ {gemini, openai} có
ít nhất 1 credential `kind=api_key` còn `enabled`) — backend KHÔNG đổi gì để hỗ trợ việc này (manifest
vẫn chỉ khai `credential: {provider:"gemini", kinds:["api_key"]}` giống `script_gen`, vì
`lvc_genai::TextGenHost` hiện chỉ có 2 nhà cung cấp đó; không có tham số API nào mới ở backend).

## Phụ thuộc được phép

lvc-node-api, lvc-core, `lvc-genai` (provider sinh text), `lvc-scripts` (dịch vụ lưu kịch bản, +
thư viện ngoài). **Cấm** node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`.

## Hợp đồng vào/ra

- Vào: `brief` (Text, tuỳ chọn, `.many()`) — mô tả sản phẩm/chương trình affiliate; rỗng thì dùng
  tham số `topic`.
- Ra: `out` (Text, `.many()`) — mỗi dòng một cảnh (định dạng như node `prompt`, nối thẳng vào cổng
  `prompt` của node tạo video).
- Tài khoản: `credential = { provider: "gemini", kinds: ["api_key"] }` (openai cũng dùng `api_key`;
  provider thực chọn ở tham số `provider`, engine resolve theo đó — giống `script_gen`).

## Tham số

`topic`, `num_scenes` (>=1, mặc định 4 — nhiều hơn mặc định `script_gen` vì kịch bản affiliate
thường cần ít nhất hook+lợi ích+CTA), `language` (mặc định "vi"), `style`, `extra_instructions`
(nối vào CUỐI chỉ dẫn affiliate có sẵn — KHÔNG thay thế), `credential` (tên API key), `provider`
("gemini"|"openai"), `save_as_script` (mặc định false), `script_name`.

## Luật riêng

- `preview` THUẦN: kiểm tham số, trả `num_scenes` cảnh giữ chỗ có prompt KHÔNG rỗng.
- `run` lấy `Arc<dyn lvc_genai::TextGenHost>` từ `ctx.services`, LUÔN gửi `TextGenReq.system` đã
  dựng qua `prompt::build_system` (không bao giờ rỗng — khác `script_gen` để không rơi về mặc định
  chung của `lvc_genai::prompt::build_system`), tách dòng bằng `lvc_genai::split_scenes`. KHÔNG tự
  gọi mạng, KHÔNG giữ API key (chỉ truyền TÊN credential).
- Lỗi API là lỗi MỘT node (không fatal) — không dừng cả run.
- `save_as_script=true`: giống `script_gen` — lỗi lưu (thiếu service/ghi kho thất bại) KHÔNG làm
  thất bại cả node, chỉ báo vào `NodeResult.failure`.

## Kiểm tra

- `cargo test -p node-script-agent` (dùng FAKE host, KHÔNG gọi API thật).
- Đổi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-script-agent` rồi sinh lại fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
