-- P7: cache offline cua kho template CONG DONG (scope='remote', nguon la file index.json tren
-- GitHub — xem crates/providers/lvc-template-registry). KHAC bang `templates` (private, nguoi dung
-- CRUD): bang nay do APP ghi lai ket qua fetch (read-through cache), thay the TOAN BO moi lan fetch
-- thanh cong; khi mat mang van doc duoc ban cu. `id` la id GOC trong index.json (chua gan tien to
-- "remote:" — tien to do gan o tang command khi tra ve TemplateMeta cho UI).
CREATE TABLE IF NOT EXISTS remote_templates_cache (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT '',
    thumbnail TEXT NOT NULL DEFAULT '',
    graph TEXT NOT NULL,
    updated_at REAL NOT NULL DEFAULT 0,
    fetched_at REAL NOT NULL DEFAULT 0
);
