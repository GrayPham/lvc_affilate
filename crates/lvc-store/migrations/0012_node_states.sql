-- Trang thai MUC NODE cua mot run (khac `tasks` chi co cho dola_video): MOI node moi run
-- deu co 1 dong, scheduler (lvc-engine dag.rs run_steps) ghi inline cho MOI kind node, de
-- khoi phuc lai trang thai (da xong/dang chay/loi + thong diep loi) khi mo lai app.
CREATE TABLE IF NOT EXISTS node_states (
  run_id     TEXT    NOT NULL,
  node_id    TEXT    NOT NULL,
  state      TEXT    NOT NULL,
  done       INTEGER NOT NULL DEFAULT 0,
  total      INTEGER NOT NULL DEFAULT 0,
  ok         INTEGER NOT NULL DEFAULT 0,
  failed     INTEGER NOT NULL DEFAULT 0,
  error      TEXT    NOT NULL DEFAULT '',
  updated_at REAL    NOT NULL DEFAULT 0,
  PRIMARY KEY (run_id, node_id)
);

CREATE INDEX IF NOT EXISTS idx_node_states_run ON node_states(run_id);
