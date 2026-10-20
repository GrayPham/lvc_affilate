-- Phan C (run-robustness): luu Outputs (JSON) cua node da xong de "chay lai node loi" seed lai
-- output node da xong, khong chay lai (khong ton credit). Server-side only: KHONG vao NodeStateRow
-- (khong ra IPC) — chi lenh retry doc qua outputs_for_run.
ALTER TABLE node_states ADD COLUMN outputs TEXT NOT NULL DEFAULT '';
