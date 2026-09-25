-- P4d: kho template RIENG cua nguoi dung (scope='private'). Mau Public nhung san trong binary
-- (crates/lvc-app/templates/*.json), khong luu DB. `graph` la workflow JSON v2.
CREATE TABLE IF NOT EXISTS templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    graph TEXT NOT NULL,
    thumbnail TEXT NOT NULL DEFAULT '',   -- data URL PNG thu nho (<=200KB), '' neu chua co
    updated_at REAL NOT NULL DEFAULT 0
);
