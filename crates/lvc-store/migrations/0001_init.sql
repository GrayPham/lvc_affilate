-- Gop tasks.db + pool_usage.db cua ban Python vao mot DB, them index.

CREATE TABLE IF NOT EXISTS tasks (
  id               TEXT PRIMARY KEY,
  prompt           TEXT    NOT NULL,
  ratio            TEXT    NOT NULL DEFAULT '9:16',
  duration         INTEGER NOT NULL DEFAULT 10,
  model            TEXT    NOT NULL DEFAULT 'seedance-2.0',
  mode             TEXT    NOT NULL DEFAULT 'fast',
  logo             TEXT    NOT NULL DEFAULT 'off',
  logo_applied     INTEGER NOT NULL DEFAULT 0,
  status           TEXT    NOT NULL DEFAULT 'queued',
  failure_code     TEXT,
  error            TEXT,
  account          TEXT,
  account_pref     TEXT,
  conversation_id  TEXT,
  video_url        TEXT,
  local_path       TEXT,
  reference_images TEXT    NOT NULL DEFAULT '[]',
  batch_id         TEXT,
  batch_name       TEXT,
  scene_index      INTEGER,
  project_name     TEXT,
  project_dir      TEXT,
  retry_count      INTEGER NOT NULL DEFAULT 0,
  retry_at         REAL    NOT NULL DEFAULT 0,
  deadline_at      REAL,
  last_poll_at     REAL,
  created_at       REAL    NOT NULL,
  updated_at       REAL    NOT NULL,
  started_at       REAL,
  finished_at      REAL
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_batch  ON tasks(batch_id, scene_index);
CREATE INDEX IF NOT EXISTS idx_tasks_retry  ON tasks(status, retry_at);

CREATE TABLE IF NOT EXISTS accounts_meta (
  name                TEXT PRIMARY KEY,
  scheduling          INTEGER NOT NULL DEFAULT 1,
  note                TEXT    NOT NULL DEFAULT '',
  email               TEXT    NOT NULL DEFAULT '',
  created_at          REAL    NOT NULL DEFAULT 0,
  last_used_at        REAL    NOT NULL DEFAULT 0,
  login_ok            INTEGER NOT NULL DEFAULT 1,
  login_checked_at    REAL    NOT NULL DEFAULT 0,
  cooldown_until      REAL    NOT NULL DEFAULT 0,
  rate_limited_until  REAL    NOT NULL DEFAULT 0,
  limit_reason        TEXT    NOT NULL DEFAULT '',
  quota_blocked_until REAL    NOT NULL DEFAULT 0,
  quota_reason        TEXT    NOT NULL DEFAULT '',
  credit_balance      INTEGER,
  credit_checked_at   REAL    NOT NULL DEFAULT 0,
  proxy               TEXT    NOT NULL DEFAULT '',
  logged_out_at       REAL    NOT NULL DEFAULT 0,
  cookie_expires_at   REAL    NOT NULL DEFAULT 0,
  browser             TEXT    NOT NULL DEFAULT '',
  auto_verify_state   TEXT    NOT NULL DEFAULT '',
  auto_verify_tries   INTEGER NOT NULL DEFAULT 0
);

-- PRIMARY KEY da tao san unique index cho (account, day, mode).
CREATE TABLE IF NOT EXISTS usage (
  account TEXT    NOT NULL,
  day     TEXT    NOT NULL,
  mode    TEXT    NOT NULL,
  used    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, day, mode)
);

CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS proxies (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  url          TEXT    NOT NULL UNIQUE,
  label        TEXT    NOT NULL DEFAULT '',
  enabled      INTEGER NOT NULL DEFAULT 1,
  health       TEXT    NOT NULL DEFAULT 'unknown',
  last_checked REAL    NOT NULL DEFAULT 0,
  fail_count   INTEGER NOT NULL DEFAULT 0,
  last_used_at REAL    NOT NULL DEFAULT 0,
  exit_ip      TEXT    NOT NULL DEFAULT ''
);
