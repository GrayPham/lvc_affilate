-- P3 Task 3: DAG executor (Task 7), resume (Task 6), Tauri commands (Task 9).
-- tasks da co san started_at/finished_at/retry_at/batch_id/batch_name/scene_index/
-- deadline_at/last_poll_at/retry_count tu 0001_init.sql (ban Python khong co khai
-- niem workflow/run — chi task don) — o day CHI them 3 cot lien ket task voi
-- workflow-run dang chay no: run_id/node_id (id trong React Flow graph) + stage
-- (buoc hien tai trong node, vd "uploading"/"polling"/"downloading").
ALTER TABLE tasks ADD COLUMN run_id TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN node_id TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN stage TEXT NOT NULL DEFAULT '';

-- Workflow = do thi node (React Flow) nguoi dung ve trong Tauri app; luu nguyen
-- JSON (Task 9 doc/ghi qua Tauri command), khong chuan hoa cot rieng cho tung node.
CREATE TABLE IF NOT EXISTS workflows (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  json       TEXT NOT NULL,
  updated_at REAL NOT NULL
);

-- Mot lan chay workflow (DAG executor Task 7 tao/cap nhat; resume Task 6 doc lai
-- khi khoi dong lai). error rong khi chua co loi.
CREATE TABLE IF NOT EXISTS runs (
  id          TEXT PRIMARY KEY,
  workflow_id TEXT NOT NULL DEFAULT '',
  status      TEXT NOT NULL,
  started_at  REAL NOT NULL DEFAULT 0,
  finished_at REAL NOT NULL DEFAULT 0,
  error       TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_tasks_run ON tasks(run_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status_retry ON tasks(status, retry_at);
