# Ghi nho ky thuat - node `script_gen`

Ghi nhung gi hoc duoc khi lam/chay node nay (quirk nha cung cap, ma loi, quyet dinh nho). Moi muc
mot dong, co ngay.

- 2026-09-27: tao node. Dich vu thuc la `lvc_genai::TextGenHost` (engine impl, resolve API key tu
  vault). Node chi truyen TEN credential + provider - KHONG bao gio thay key.
- 2026-09-27: `preview` tra cai gia cho co prompt KHONG rong ("(canh N - AI se sinh...)") vi
  `dola_video` normalize canh se loi neu prompt rong -> workflow_validate se do oan.
- 2026-09-27: quirk ma loi Gemini/OpenAI ghi o `crates/providers/lvc-genai/MEMORY.md` (dung cho).
