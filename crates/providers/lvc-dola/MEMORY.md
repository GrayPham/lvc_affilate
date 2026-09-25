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
