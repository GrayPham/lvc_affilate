-- Kho kich ban (P-script-library): cache offline cua kich ban CONG DONG (scope='remote'), cung
-- nguon index.json voi kho mau (crates/providers/lvc-template-registry, phan biet bang truong
-- `kind: "script"` tren tung muc) — xem CLAUDE.md provider do. KHAC bang `scripts` (private, nguoi
-- dung CRUD scenes/task): bang nay chi luu BAN NHAP (ten + danh sach dong prompt tho, CHUA tao
-- scenes/task) do APP ghi lai ket qua fetch, thay the TOAN BO moi lan fetch thanh cong. `id` la id
-- GOC trong index.json (chua gan tien to "remote:" — tien to gan o tang command).
CREATE TABLE IF NOT EXISTS remote_scripts_cache (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT '',
    scenes TEXT NOT NULL DEFAULT '[]',
    updated_at REAL NOT NULL DEFAULT 0,
    fetched_at REAL NOT NULL DEFAULT 0
);
