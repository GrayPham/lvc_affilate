# LVC Seedance v2 (Rust + Tauri)

Bản viết lại của `lvcseedance` (Python) sang Rust + Tauri với workflow kéo thả.

## Trạng thái

Giai đoạn **P0**: lõi thuần + tầng lưu trữ. Chưa có trình duyệt, chưa có UI.

## Cấu trúc

| Crate | Trách nhiệm |
|---|---|
| `lvc-core` | Miền thuần: chế độ, cảnh, TXT, tiến độ. Không I/O. |
| `lvc-store` | SQLite qua sqlx, migration, nhập dữ liệu Python cũ. |

## Phát triển

```bash
cargo test --workspace
powershell -ExecutionPolicy Bypass -File scripts/check.ps1
```

`lvc-core` **không được** phụ thuộc sqlx/tokio/reqwest/chromiumoxide/tauri.
`scripts/check.ps1` cưỡng chế ràng buộc này.

## Nhập dữ liệu từ bản Python

```rust
let db = Db::open(Path::new("lvc.db")).await?;
let report = import_legacy(&db, Path::new(r"A:\ToolLVC\lvcseedance")).await?;
```

Chỉ đọc `tasks.db` và `pool_usage.db` cũ, không bao giờ sửa. Chạy lại được nhiều lần.

## Tài liệu

- Thiết kế: `docs/superpowers/specs/2026-09-16-lvc-seedance-rust-design.md`
- Kế hoạch P0: `docs/superpowers/plans/2026-09-16-p0-core-store.md`
