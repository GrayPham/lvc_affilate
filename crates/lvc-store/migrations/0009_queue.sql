-- Hang cho workflow (man "Tác vụ") — chay TUAN TU tung workflow mot, khac han bang `runs`
-- (0001_init) von chi ghi lai MOT lan chay da bat dau. Bang nay la DANH SACH CHO: nguoi dung them
-- workflow vao, sap thu tu, roi bam chay; engine lay tung muc theo `position` tang dan.
--
-- `graph` la BAN CHUP (snapshot) do thi luc THEM vao hang cho — khong tham chieu `workflows.graph`:
-- sua workflow goc sau do KHONG lam doi muc dang cho (dung ky vong "chay dung bản tôi đã thêm",
-- ban ban giao UI ghi "Workflow trong hàng chờ đang dùng bản 1.0.0 vẫn chạy bản cũ").
--
-- `status`: waiting | running | done | failed | skipped. `skipped` = bi bo qua vi thieu tai khoan
-- hoac loi ma chinh sach "Bỏ qua, chạy tiếp" (xem `queue_on_error` trong settings).
CREATE TABLE IF NOT EXISTS queue_items (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL DEFAULT '',
    name TEXT NOT NULL,
    graph TEXT NOT NULL,
    position INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'waiting',
    -- `runs.id` cua lan chay (neu da chay). Khong dat FOREIGN KEY: run co the bi don dep doc lap,
    -- luc do day chi con id "mo coi" — doc lai coi nhu chua chay (giong `scenes.task_id`).
    run_id TEXT,
    error TEXT NOT NULL DEFAULT '',
    created_at REAL NOT NULL DEFAULT 0,
    started_at REAL,
    finished_at REAL
);

-- Lay muc ke tiep: loc theo status roi sap theo position.
CREATE INDEX IF NOT EXISTS idx_queue_status_pos ON queue_items(status, position);
