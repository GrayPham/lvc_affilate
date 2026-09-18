-- P3c Task 1: kich ban (scripts) + canh (scenes) — schema y het Python
-- src/core/scripts_store.py:_init (chi 2 bang; characters/character_groups la kho
-- nhan vat, P4, chua port). Kich ban la goc; scenes.task_id tro task MOI NHAT trong
-- bang tasks co san (0001_init.sql) — khong co khoa ngoai SQL (Python cung khong dat
-- FOREIGN KEY o day) vi task co the bi xoa doc lap, luc do scenes.task_id chi con la
-- id "mo coi" cho toi khi ScriptRepo doc lai va coi nhu chua chay (xem scripts.rs).
CREATE TABLE IF NOT EXISTS scripts (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  dir        TEXT NOT NULL DEFAULT '',
  created_at REAL,
  updated_at REAL
);

-- ratio/duration/model/mode/logo de NULL duoc (khac cot cung ten trong bang tasks) —
-- gia tri NULL/rong nghia la "chua dat, dung mac dinh AppSettings luc CHAY" (xem
-- lvc-engine::scripts::run_script), khong phai loi du lieu.
CREATE TABLE IF NOT EXISTS scenes (
  id         TEXT PRIMARY KEY,
  script_id  TEXT NOT NULL,
  idx        INTEGER NOT NULL,
  prompt     TEXT NOT NULL DEFAULT '',
  ratio      TEXT,
  duration   INTEGER,
  model      TEXT,
  mode       TEXT,
  logo       TEXT,
  account    TEXT NOT NULL DEFAULT '',
  refs       TEXT NOT NULL DEFAULT '[]',
  task_id    TEXT,
  updated_at REAL
);

CREATE INDEX IF NOT EXISTS idx_scenes_script ON scenes(script_id, idx);
CREATE INDEX IF NOT EXISTS idx_scenes_task ON scenes(task_id);
