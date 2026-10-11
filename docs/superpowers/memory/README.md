# Memory snapshot (chuyển máy)

Bản sao **auto-memory của trợ lý** (Claude Code) cho dự án này, commit vào repo để `git clone` mang
theo khi chuyển sang máy khác — vì auto-memory gốc nằm NGOÀI repo tại
`~/.claude/projects/a--ToolLVC-lvc-seedancev2/memory/` (per-user, không đi theo git).

- `MEMORY.md` — chỉ mục (mỗi dòng một ghi nhớ).
- Các file còn lại — một ghi nhớ / một việc (user, feedback, project, reference).

## Đưa sang máy mới

Sau khi `git clone` ở máy mới, copy các file này vào thư mục auto-memory của Claude Code trên máy đó:

```
~/.claude/projects/<slug-dự-án>/memory/
```

(trên Windows: `C:\Users\<user>\.claude\projects\<slug>\memory\`). `<slug>` sinh từ đường dẫn dự án
nên có thể khác — cứ đặt đúng thư mục `memory/` mà Claude Code tạo cho dự án ở máy mới.

Ngoài ra, mỗi crate/module lớn còn có `CLAUDE.md` + `MEMORY.md` riêng (đã nằm trong repo, tự đi theo
git) — đó mới là nơi ghi quirk kỹ thuật của từng node/provider; thư mục này chỉ là memory tổng của
trợ lý.

> Lưu ý: KHÔNG commit bí mật (API key, cookie, mật khẩu) vào đây. Đã rà — các file hiện tại không
> chứa bí mật.
