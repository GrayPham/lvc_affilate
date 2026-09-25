# Node modules

Mỗi thư mục con là MỘT loại node độc lập (ADR-R014). Luật chung: `CLAUDE.md` ở gốc repo, mục
"Node độc lập". Hợp đồng: crate `crates/lvc-node-api`.

| Crate | kind | Vào | Ra | Tài khoản |
|---|---|---|---|---|
| `node-prompt` | `prompt` | — | `out: Text*` | — |
| `node-image-upload` | `image_upload` | — | `out: Image*` | — |
| `node-dola-video` | `dola_video` | `prompt: Text*` (bắt buộc), `refs: Image*` | `out: Video*` | dola/session |
| `node-save` | `save` | `video: Video*` \| `image: Image*` | — | — |

Tạo node mới: `powershell -File scripts/new-node.ps1 -Kind <kind> -Category <input|text|image|video|audio|output>`.
