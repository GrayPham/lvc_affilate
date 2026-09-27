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
