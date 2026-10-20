# Ghi nhớ — `lvc-museai`

- 2026-10-13: `GET /model` (qua `HatchConnection`) là RPC THUẦN ĐỌC, KHÔNG sinh video/ảnh — dùng để
  nghiên cứu khả năng API mà KHÔNG tốn credit. Đã test thật: trả `{selected_model, default_model,
  available_models:[{id,provider,model,context_window_tokens,display_name,preview_model}],
  generation, display_name, model_identity_visibility}` — đây là danh sách MODEL LLM điều phối hội
  thoại (`auto/auto`, `ipnext/avocado-5.16-v0`), KHÔNG liên quan thông số video (không có
  aspect/duration/resolution). Kết luận: muse.ai không có API cấu hình tỉ lệ/thời lượng — xem
  `node-museai-video/CLAUDE.md` mục "Tham số".
- 2026-10-13 (cùng ngày, sau phát hiện trên): bản đầu `build_video_prompt` tự bịa câu tiếng Việt
  chèn cuối ("(yêu cầu thêm: ...)") — test thật xin 30s ra 10s, coi là "không tuân theo". Người
  dùng phản bác (tự tay gõ thẳng trên web muse.ai ra đúng thời lượng), rồi chỉ ra
  `E:\ProjectAILVC\Lvc_veo3_automation` (tham khảo, CHỈ ĐỌC) có HẲN module
  `src/core/museai/prompt_builder.py` ghi lại kết quả ĐO THẬT 10/10 LẦN của người khác: câu đúng là
  tiếng ANH `"Output video: {N} seconds, {tỉ lệ}."` nối CUỐI prompt (không phải đầu, không phải
  tiếng Việt) — `build_video_prompt` đã PORT lại đúng câu này. Bài học: đừng tự bịa câu chỉ thị cho
  một AI chat khi có sẵn dữ liệu đo thật từ nơi khác — tìm tham khảo TRƯỚC khi tự thử-sai tốn credit.
  `prompt_builder.py` cũng ghi: muse.ai KHÔNG tuân thủ tuyệt đối ngay cả với câu đúng (10/10: xin 5s
  vẫn ra 10s — do 1 clip Muse mặc định ~10s) nhưng xin 15–30s thì muse.ai TỰ ĐỘNG dựng nhiều clip
  ghép lại (xin 30s → 4 clip 10s ghép thành 30s, ~5 phút) — không cần tool tự ffmpeg concat. File đó
  còn có `MUSE_RULES` (chưa port): chặn muse.ai "nhớ nhầm" trả lại video cũ thay vì tạo mới, và nhắc
  giữ track âm thanh khi tự ghép nhiều clip (video ghép hay bị mất tiếng) — cân nhắc port nếu gặp
  đúng 2 lỗi đó khi dùng thật.

- 2026-10-11: người dùng 2 lần yêu cầu "kích hoạt MuseAI" dựa trên
  `E:\ProjectAILVC\Lvc_veo3_automation` — lần đầu tôi kết luận SAI rằng nguồn đó không có MuseAI
  (chỉ tìm thấy phần Veo3/Flow đã port sẵn). Tìm lại kỹ hơn (grep toàn bộ cây, không chỉ `.py` cấp
  cao) mới thấy `src\core\museai\` — module MuseAI THẬT, riêng biệt hoàn toàn với phần Veo3/Flow
  trong CÙNG repo nguồn. Bài học: lần sau khảo sát "nguồn X có chứa Y không" phải grep toàn cây,
  không dừng lại ở cấu trúc thư mục cấp cao/README.
- Đã cảnh báo người dùng: bản thân `protocol/` là client reverse-engineered giao thức độc quyền
  (cùng rủi ro bị chặn như chữ ký chống-bot Dola, xem `docs/superpowers/memory/
  p4b-no-antibot-signing.md`), đề xuất phương án an toàn hơn (tự động hoá trình duyệt thật như
  Dola/Veo3, vì muse.ai có giao diện web dạng chat bình thường — xác nhận qua khảo sát
  `browser_login.py`/`museai_video_tab.py` trong nguồn tham khảo). Người dùng CHỌN port giao thức
  Noise/Hatch thật thay vì phương án an toàn hơn — quyết định có ý thức, đã port theo hướng đó.
- `noise.rs`: `Noise_XX_25519_AESGCM_SHA256` là giao thức Noise Protocol Framework CHUẨN CÔNG KHAI
  (noiseprotocol.org) — khác bản chất với chữ ký chống-bot độc quyền (X-Gorgon/Argus/Ladon của
  Dola). Quyết định port phần này trước vì rủi ro thấp hơn nhiều so với phần OTP-native-auth/
  WebSocket-RPC-sống (auth.rs/client.rs/transport.rs — CHƯA port, xem CLAUDE.md "bối cảnh rủi ro").
- `x25519-dalek`: dùng `StaticSecret` (KHÔNG dùng `EphemeralSecret`) cho `e_priv`/`s_priv` dù về mặt
  khái niệm là khoá tạm thời (ephemeral) — vì bắt tay Noise XX cần tái sử dụng CÙNG MỘT `e_priv` cho
  HAI phép Diffie-Hellman khác nhau (`ee` rồi `es`), còn `EphemeralSecret::diffie_hellman` trong
  x25519-dalek 2.x TIÊU THỤ `self` (chỉ dùng được 1 lần, chủ đích chống lạm dụng lại khoá tạm) —
  không phù hợp dù tên "ephemeral" nghe đúng hơn.
- `signed64`/`unsigned64` (wire.rs): ép kiểu bit `u64 as i64`/`i64 as u64` là diễn giải lại bù-2 CÓ
  CHỦ ĐÍCH (khớp `value &= (1<<64)-1` bên Python khi mã hoá số âm vào varint) — không phải lỗi
  tràn/mất dữ liệu, đã `#[allow(...)]` kèm lý do thay vì sửa logic (sửa sẽ làm sai ý nghĩa).
- 2026-10-12 — **QUIRK NGHIÊM TRỌNG đã xác nhận bằng dữ liệu thật (5/5)**: chạy lô "3 video/lô,
  tuần tự, CÙNG một `HatchConnection`, mỗi job `session_id: None`" (xem
  `crates/lvc-engine/src/museai.rs::EngineMuseaiJobs::generate_videos`) qua 5 lần độc lập
  (`examples/museai_batch.rs`, 5 kịch bản × 3 video) → **job thứ 2 trong MỖI lô fail 100% (5/5)**,
  job 1 và job 3 luôn thành công. Không phải nhiễu ngẫu nhiên — khả năng cao nhất: `chat_stream`
  gửi job 2 (dùng lại cùng `device_id`/`node_id` với job 1, cùng kết nối) bị server gắn nhầm vào
  CÙNG session hội thoại với job 1 (thay vì tạo session mới), khiến `resolve_session_id`/
  `wait_for_videos` của job 2 poll sai phạm vi lịch sử; job 3 sau đó "thoát" được pattern này vì lý
  do chưa rõ (có thể do đã chờ hết 600s timeout của job 2 trước khi gửi, tạo khoảng cách đủ lớn).
  **CHƯA sửa trong code** (cần debug thêm với dữ liệu thật — tốn credit — mới xác nhận đúng
  nguyên nhân trước khi vá). Hệ quả thực tế hiện tại: `museai_video`/`museai_image` KHÔNG nên nhận
  nhiều hơn 1 prompt/job cho mỗi lần chạy node (cổng `prompt` tuy khai `many`, nhưng job thứ 2+ gửi
  trong cùng một lần gọi `generate_videos`/`generate_images` có rủi ro fail rất cao) — nếu cần
  nhiều video, ưu tiên chạy node nhiều LẦN (nhiều `run_graph` riêng, mỗi lần 1 kết nối Hatch mới)
  thay vì dồn nhiều prompt vào một lần gọi.
  **XÁC NHẬN workaround (2026-10-12, cùng ngày)**: chạy lại ĐÚNG 5 "cảnh 2" đã fail, mỗi cái qua
  MỘT `run_graph`/MỘT kết nối Hatch riêng (`examples/museai_retry_scene2.rs`, không dồn vào lô) →
  **5/5 thành công**. Càng củng cố giả thuyết lỗi nằm ở việc TÁI SỬ DỤNG một `HatchConnection` cho
  nhiều job tuần tự, không phải do bản chất prompt/kịch bản. Vẫn CHƯA sửa `EngineMuseaiJobs`/
  `EngineMuseaiImageJobs` (sẽ cần mở N kết nối riêng cho N job thay vì 1 kết nối dùng chung — đổi
  kiến trúc file `museai.rs`, nên để dành cho lúc có yêu cầu sửa thật, không tự ý sửa khi chưa được
  giao).

## Bootstrap song song (run-robustness A, 2026-10-20)
`fetch_bootstrap` mở Chromium THẬT tại `profile_dir`. Khi scheduler chạy nhiều node MuseAI song
song CÙNG account → 2 Chromium cùng `profile_dir` sẽ kẹt profile lock. Giải: host engine
(`lvc-engine/src/museai.rs`) giữ lease per-`profile_dir` (`BootstrapLocks`), nối tiếp CHỈ phần
`fetch_bootstrap`, thả trước WebSocket để generation (~phút) chạy chồng. Lease do Engine giữ, chia
cho cả 2 host image+video (cùng run).

## Mỗi cảnh một SESSION riêng + retry 502 (2026-10-20)
- GỐC RỄ lỗi "dồn 20 sự kiện vào 1 chat chính" + ảnh đính kèm không lên + hay 502: host gọi
  generate_video/image với `session_id: None` → đẩy vào chat chính muse.ai. Bản tham chiếu
  (`service.py:507`) luôn `session_id or str(uuid.uuid4())` → MỖI cảnh một chat riêng. Fix: host
  (`lvc-engine/src/museai.rs`) đặt `session_id: Some(uuid::new_v4())` mỗi job (video + image).
- 502 Bad Gateway hay xảy ra ở bước KẾT NỐI Hatch → thêm `connect_with_retry` (retry khi
  `MuseError::is_transient()` = 429/5xx, backoff 3s→×2 trần 30s, 4 lượt, tôn trọng huỷ). Mirror Veo3
  retry 5xx. Lỗi generation-phase (Rpc 502 giữa chừng) CHƯA reconnect — follow-up nếu còn gặp.
