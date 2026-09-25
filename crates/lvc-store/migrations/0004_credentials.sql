-- P4c (ADR-R017): kho tai khoan AI. `secret` la ciphertext DPAPI (lvc-vault) — KHONG bao gio
-- plaintext. `proxy` luu inline nhu accounts_meta.proxy (bang proxies chua co repo; spec
-- proxy_id REFERENCES proxies(id) bi lech kieu INTEGER/TEXT nen dung chuoi inline).
-- Thoi diem (created_at/last_used_at/...) la REAL epoch giay, dong bo voi accounts_meta.
CREATE TABLE IF NOT EXISTS credentials (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,              -- dola | google | openai | gemini | chatgpt_web | ...
    kind TEXT NOT NULL,                  -- session | api_key | login
    label TEXT NOT NULL,
    secret BLOB NOT NULL,                -- DPAPI ciphertext
    meta TEXT NOT NULL DEFAULT '{}',     -- JSON: email, profile_dir, base_url, model mac dinh, device...
    proxy TEXT NOT NULL DEFAULT '',      -- proxy inline (mask khi hien thi)
    status TEXT NOT NULL DEFAULT 'unknown', -- ok | expired | banned | quota | unknown
    enabled INTEGER NOT NULL DEFAULT 1,
    daily_limit INTEGER,
    cooldown_until REAL NOT NULL DEFAULT 0,
    last_used_at REAL NOT NULL DEFAULT 0,
    last_verified_at REAL NOT NULL DEFAULT 0,
    created_at REAL NOT NULL DEFAULT 0
);

-- Chon credential theo provider + con bat (pool LRU sau nay loc them cooldown/daily_limit).
CREATE INDEX IF NOT EXISTS idx_credentials_provider ON credentials(provider, enabled);
