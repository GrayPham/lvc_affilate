# Node `museai_image` — luật

Hợp đồng chung: `crates/lvc-node-api` | Luật repo: `CLAUDE.md` gốc (mục "Node độc lập"). Anh em
với `node-museai-video` (cùng provider `lvc-museai`, cùng cơ chế đăng nhập/bootstrap) — xem
`crates/providers/lvc-museai/CLAUDE.md` mục "Cơ chế tạo video/ảnh" cho kiến trúc đầy đủ.

## Cơ chế tạo ảnh

ĐÃ XÁC NHẬN qua trang thật 2026-10-12: muse.ai sinh được ẢNH TĨNH qua CÙNG `/chat/stream` dùng cho
video — phân biệt bằng Ý PROMPT, KHÔNG phải endpoint/capability riêng. Mặc định agent thiên về
VIDEO (tự mô tả "tôi tạo video" khi hỏi trống không) nên **BẮT BUỘC** phải báo rõ muốn ảnh tĩnh.
Node này KHÔNG tự thêm từ khoá gì — `lvc_museai::protocol::client::generate_image` (provider) tự
gắn hướng dẫn rõ ràng (câu đã xác nhận THÀNH CÔNG trong test thật: "a single still photo...no
motion, no video, just one static image") vào CUỐI prompt người dùng trước khi gửi — node chỉ cần
truyền nguyên văn mô tả chủ đề.

3 giai đoạn giống hệt `museai_video` (xem `node-museai-video/CLAUDE.md` để biết chi tiết từng
bước): [1] đăng nhập tay qua tab "Tài khoản AI" → [2] trình duyệt thật lấy token bootstrap qua
`eval_json` → [3] gửi/poll qua API Noise/WebSocket thật (KHÔNG giả mạo fingerprint gì).

## Phụ thuộc được phép

lvc-node-api, lvc-core, `lvc-museai` (provider, + thư viện ngoài). **Cấm** node khác, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri`.

## Hợp đồng vào/ra

- Vào: `prompt: Text*` (bắt buộc) — mô tả chủ đề ảnh, KHÔNG cần tự thêm "ảnh tĩnh"/"static" (đã có
  provider tự lo, xem trên).
- Ra: `out: Image*`.
- Tài khoản: `museai/login` (`credential_id` — CÙNG profile/cookie với `museai_video`, một tài
  khoản MuseAI dùng được cho cả 2 node).

## Tham số

`account` (credential_id) — rỗng = tự chọn tài khoản đầu tiên còn dùng được trong kho (giống
`museai_video`/`dola_video`, 2026-10-13). muse.ai KHÔNG có tham số API cho tỉ lệ/thời lượng/số
lượng — xem `node-museai-video/CLAUDE.md` mục "Tham số" (nghiên cứu đã làm, đừng lặp lại).

## Luật riêng

- `preview` THUẦN (không I/O). `run` chỉ chạm thế giới ngoài qua `NodeCtx` — dịch vụ
  `Arc<dyn lvc_museai::MuseaiImageHost>` (engine impl, mở trình duyệt lấy bootstrap TƯƠNG TỰ
  `museai_video` — xem `crates/lvc-engine/src/museai.rs::EngineMuseaiImageJobs`).
- Thất bại một phần → `Ok` + `failure`; thất bại hoàn toàn → `NodeError` (`fatal` chỉ khi phải
  dừng cả run).
- Định dạng file ảnh tải về KHÔNG cố định — quan sát thật là `.webp`, nhưng engine giữ NGUYÊN đuôi
  file thật trả về (không ép `.png`/`.jpg`) — xem `EngineMuseaiImageJobs::generate_images`.

## Kiểm tra

- `cargo test -p node-museai-image`
- Đổi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-museai-image` rồi sinh lại fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
