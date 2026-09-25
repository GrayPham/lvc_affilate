# Ghi nhớ kỹ thuật — provider Dola

- 2026-09-26: tạo crate (P4a) với `DolaVideoHost`.
- API app: host `api16-normal-i18n-myb.dola.com`, aid 489823. `Accept-Encoding: gzip, deflate` —
  KHÔNG quảng cáo `ttzip` (server trả nén riêng). Nguồn: `dola_app.py:306-315`.
- Mã lỗi: 710022002 khoá tài khoản · 710022013 tạm/credit · 710022004 captcha · 710010202 chữ ký
  sai · 710012001 hết phiên. ID thiết bị tự bịa/xáo → 710022002.
- Khoảng cách gửi ≥ 8s giữa hai lần gửi (`API_SEND_GAP`).
