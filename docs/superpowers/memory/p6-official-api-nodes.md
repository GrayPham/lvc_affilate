---
name: p6-official-api-nodes
description: P6 — node gọi API chính thức Google/Seedance/Grok + smoke test Camoufox trung tính
metadata:
  node_type: memory
  type: project
  originSessionId: da54ba42-1cfd-489f-aae3-ce0d033752b6
  modified: 2026-10-01T10:59:59.271Z
---

P6 hoàn tất 2026-10-01, `scripts/check.ps1` TẤT CẢ XANH. Node gọi **API chính thức** (người dùng tự
nhập API key + endpoint), khác hẳn node `veo3_*` (điều khiển trình duyệt Flow).

- **Provider crate**: `lvc-google` (ảnh Nano Banana `:generateContent`, video Veo 3.1
  `:predictLongRunning`), `lvc-seedance` (Volcengine Ark `/api/v3`), `lvc-grok` (xAI, chỉ ảnh — xAI
  chưa có API video). Mỗi crate khai host trait riêng; client HTTP thuần; key chỉ qua tham số, không log.
- **Node crate**: `google_image`/`google_video`, `seedance_image`/`seedance_video`, `grok_image`.
  Tham số `credential` = TÊN/label API key (resolve theo label, như `script_gen`).
- **Engine**: `crates/lvc-engine/src/apimedia/` (chia google/seedance/grok/util theo luật module),
  impl host trait, DÙNG LẠI `TextSecretResolver` (provider tag: google official dùng key `"gemini"`,
  seedance `"seedance"`, grok `"grok"`). Chèn service vô điều kiện trong `dag.rs::start_run`.
- **Vault**: thêm provider `"seedance"`, `"grok"` (chỉ `api_key`). Google official DÙNG CHUNG key
  `"gemini"`.
- **Camoufox**: KHÔNG build luồng Camoufox-vào-Flow (ranh giới cứng [[camoufox-flow-port-blocked]]).
  Chỉ có smoke test trung tính `crates/lvc-browser/tests/camoufox_bidi_smoke.rs` (`#[ignore]`, BiDi →
  example.com) + nhãn "Sắp có" trên node video Veo3. Quirk API Google (Imagen shutdown, Nano Banana,
  Veo 3.1, header `x-goog-api-key`, tải video kèm header+redirect) ghi ở `crates/providers/lvc-google/MEMORY.md`.
- **Phối hợp đa phiên**: làm cùng lúc với phiên `lvc-seedancev2-1c` (nó định làm `lvc-gemini-media`);
  đã chia việc — phiên này sở hữu Google-media/Seedance/Grok, 1c đứng ngoài. Khi 2 phiên cùng cây
  repo: nhắn phối hợp trước, đọc lại file chung (Cargo.toml/registry/dag/types.ts) ngay trước khi ghi.
- CHƯA chạy live (không có key thật) — như `lvc-genai`, dùng wiremock; endpoint/model đều override được.
