# Ghi nhớ kỹ thuật — provider Dola

- 2026-09-26: tạo crate (P4a) với `DolaVideoHost`.
- API app: host `api16-normal-i18n-myb.dola.com`, aid 489823. `Accept-Encoding: gzip, deflate` —
  KHÔNG quảng cáo `ttzip` (server trả nén riêng). Nguồn: `dola_app.py:306-315`.
- Mã lỗi: 710022002 khoá tài khoản · 710022013 tạm/credit · 710022004 captcha · 710010202 chữ ký
  sai · 710012001 hết phiên. ID thiết bị tự bịa/xáo → 710022002.
- Khoảng cách gửi ≥ 8s giữa hai lần gửi (`API_SEND_GAP`).

## P4b (2026-09-26): CHU DONG KHONG port phan ky chong bot + gia thiet bi
- X-Gorgon/X-Argus/X-Ladon (chu ky chong bot ByteDance) va TTEncrypt + dang ky thiet bi
  (gia danh tinh thiet bi tung tai khoan) la ky thuat NE PHAT HIEN BOT cua nen tang -> KHONG
  dung lai trong Rust (bo loc an toan chan; quyet dinh cua nguoi dung: giu o ban Python).
- Da port: lop giao thuc (payload/response/session) + `trait RequestSigner` de trong (khong impl).
  Kenh API app do do CHUA tu xac thuc duoc tu Rust.
- Duong hop le de tao video/anh Dola: BACKEND TRINH DUYET (P2/P3) — phien dang nhap that, trang
  tu ky request. Node dola_video dung backend nay. Neu sau nay can kenh HTTP, di qua in-page fetch
  cua chinh trang (nhu webrun Python) thay vi dung lai chu ky.

## P4b-sign (2026-09-27): DA PORT loi ky X-Gorgon/X-Ladon/X-Argus sang Rust THUAN
- Nguoi dung dao quyet dinh (thay muc tren cho phan KY): port chinh xac tu
  `lvcseedance/.../x_signatures/lib/*.py`, khop golden vector TUNG BYTE
  (`tests/sign_vectors.rs` doc `tests/fixtures/sign_vectors.json`). Van CHUA noi vao engine/node/
  settings — chi la ham thuan + `RealSigner` doc lap trong crate. TTEncrypt + dang ky/gia thiet bi
  van CHUA port (device/mod.rs giu stub).
- Chia file: `sign/{sm3,simon,protobuf,gorgon,ladon,argus,pkcs7}.rs`, `mod.rs` ghep + `RealSigner`.
- QUIRK protobuf (`writeVarint`): dieu kien `while vint > 0x80` (KHONG phai `>=`) VA mask
  `vint & 0xFFFFFFFF` (32-bit) TRUOC vong. Sai 1 trong 2 la lech byte (vd 0x80 -> "00", 0x81 ->
  "8101"). Field ghi theo THU TU CHEN cua dict; nested = STRING(2) long. key = (idx<<3)|type.
- QUIRK Ladon padding: KHONG giong PKCS7 chuan. `pkcs7_padding_pad_buffer` tra -pad khi da tron
  khoi -> KHONG them gi khi size % 16 == 0 (chi lap phan thieu). Argus thi dung PKCS7 chuan
  (`Crypto.Util.Padding.pad`, LUON them ca khoi khi da tron) — xem `pkcs7.rs`: `ladon_pad` vs
  `pkcs7_pad`.
- QUIRK Ladon `encrypt_ladon_input`: Python cap `bytearray(26)` nhung slice-assign `output[8:]=8byte`
  cat con 16 byte -> ket qua 1 block = data0_le || data1_le (16 byte), khong phai 26.
- HANG cung Argus (chep NGUYEN, dung sua): `SIGN_KEY` (32B), `SM3_OUTPUT` (32B, = sm3(sign_key +
  b"\xf2\x81ao" + sign_key)). AES key = md5(SIGN_KEY[:16]), iv = md5(SIGN_KEY[16:]). Prefix wrap
  `\xa6n\xad\x9fw\x01\xd0\x0c\x18` + ... + `ao`; enc_pb prefix `\xf2\xf7\xfc\xff`x2; output prepend
  `\xf2\x81`. Simon key_list = unpack "<QQ" 2 lan tu SM3_OUTPUT[:32] (= 4 u64 LE lien tiep).
- QUIRK Argus `encrypt_enc_pb`: XOR byte[i] voi xor_array[i%8] (xor_array = 8 byte dau) tu i>=8,
  ROI DAO NGUOC toan bo (`[::-1]`).
- XGorgon: Khronos = hex(int(time))[2:] (GIAY), X-Khronos = str(secs). hex_str hang
  `[30,64,224,217,147,69,0,180]`, length 20. Prefix "0404" + hex(hex_str[7],[3],[1],[6]) =
  "0404b4d94000". `__handle` sua tai cho -> khi i=19 doc input[0] DA bi ghi de (giu vong for).
- `__encryption` cua Gorgon: `tmp` la '' (falsy) hoac int; mo hinh bang Option<u32>, truthy khi
  Some(v!=0); so sanh `tmp != 85` thi None -> true. Sai cho nay lech ca bang hoan vi.
- Edition 2024: `rng.gen()` -> `rng.r#gen()` (gen la keyword).
- RealSigner runtime KHONG khop vector (random4 + rand field3 + time that); tra
  [X-Gorgon, X-Khronos, X-Ladon, X-Argus]. x-ss-stub = md5(body) HEX HOA khi co body.

## P4b-orchestration (2026-09-27): upload anh (AWS4) + tao video (SSE/poll)
- Files: `aws4.rs` (ky AWS4), `datetime.rs` (amz/http date THUAN, khong them chrono/time),
  `dims.rs` (doc W/H PNG+JPEG THUAN, khong them crate anh), `upload.rs`, `generate.rs`.
- QUIRK AWS4 (ImageX, `_aws4_sign` dola_client.py:418-452): BIEN THE ByteDance — canonical
  querystring KHONG url-encode gia tri (`f"{k}={v}"` tho); tham so `host` co trong chu ky Python
  NHUNG khong duoc dua vao canonical_request (bo qua) -> wiremock host khac van khop. region-aws
  = "sdwdmwlll", service = "imagex". Golden vector sinh tu .venv Python (2 case trong aws4.rs).
- QUIRK upload 3 buoc: (1) POST alice/resource/prepare_upload ky X-Gorgon, body JSON KHONG vao
  chu ky (Python `_sign_request(url)` khong truyen body -> session.post truyen None body: KHOP).
  (2) GET /top/v1 ky AWS4 (khong cookie/X-Gorgon). (3) POST {UploadHosts[0]}/upload/v1/{StoreUri}
  (Auth cua CDN, code==2000), roi POST alice/resource/register body GZIP + header
  `x-bd-content-encoding: gzip` + `Content-Type: application/json; charset=UTF-8`, code==0.
- AccessKeyId nam trong session_token: bo 4 ky tu dau, base64-decode phan con lai, doc JSON
  ["AccessKeyId"]. secret_key = upload_auth_token.secret_key (KHAC AccessKeyId). BI MAT — khong log.
- SUY LUAN (Python mo ho): dung `session.params_im` (co `flow_im_arch=v2&`) cho alice/resource,
  con Python dung base_params (KHONG co flow_im_arch). Vo hai vi server tu dung canonical tu query
  DA GUI (== query da ky). Neu server that tu choi, thu bo flow_im_arch cho 2 call nay.
- CDN scheme suy tu `base_url` (https that / http wiremock) vi Python hardcode https.
- width/height=0 chap nhan (Dola tu do); dims.rs chi co-gang doc PNG/JPEG.
- generate_video: video_payload -> POST chat/completion (params_chat, SSE_TIMEOUT) doc SSE
  (SseParser+ChatStream) -> neu chua co video thi poll im/chain/single (params_im, POLL_TIMEOUT,
  moi 10s) toi khi status==3 & co URL, hoac het `video_timeout(mode,dur,has_refs)`. Uu tien
  nowm_url (video_model giai base64), roi fallback im/conversation/batch_get (walk_video_models)
  neu thieu ban sach, cuoi cung download_url (watermarked=true). Ton trong CancellationToken.
- `generate_video_with(..., poll_interval)`: ban noi bo cho test dat khoang poll ngan (wiremock).
- Test wiremock (`tests/orchestration_wiremock.rs`): AppSession.base_url tro server gia + NoSign;
  KHONG goi Dola that.
