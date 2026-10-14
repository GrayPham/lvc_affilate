-- Nguồn gốc của một workflow đã lưu: nó được tạo từ mục nào trong kho cộng đồng, ở phiên bản nào.
-- Nhờ đó app biết khi nào kho có bản mới hơn ("Cập nhật workflow") và biết người dùng đã sửa bản
-- trên máy hay chưa.
--
-- Bảng RIÊNG, không thêm cột vào `workflows`: phần lớn workflow do người dùng tự dựng, không có
-- nguồn gốc nào — nhét ba cột rỗng vào bảng chính chỉ làm nặng mọi câu đọc. Xoá workflow thì
-- `WorkflowOriginRepo::remove` tự xoá dòng tương ứng (schema này không dùng khoá ngoại).
CREATE TABLE IF NOT EXISTS workflow_origin (
    workflow_id    TEXT PRIMARY KEY,
    -- id GỐC trong `index.json` (chưa gắn tiền tố "remote:").
    source_id      TEXT NOT NULL,
    -- Phiên bản đã tải về; rỗng với bản publish cũ chưa có trường `version`.
    source_version TEXT NOT NULL DEFAULT '',
    -- Ảnh chụp đồ thị NGAY LÚC tải về. So với `workflows.json` hiện tại để biết người dùng đã sửa
    -- gì trên máy chưa — không có nó thì không thể phân biệt "chưa sửa" với "sửa rồi".
    source_json    TEXT NOT NULL,
    installed_at   REAL NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_workflow_origin_source ON workflow_origin(source_id);

-- Ba trường mới của một mục kho cộng đồng. Đều có mặc định nên `index.json` đã publish trước đây
-- (chưa có các trường này) vẫn đọc được bình thường.
ALTER TABLE remote_templates_cache ADD COLUMN version TEXT NOT NULL DEFAULT '';
ALTER TABLE remote_templates_cache ADD COLUMN author TEXT NOT NULL DEFAULT '';
-- JSON mảng `[{"kind":"Thêm","text":"…"}]`; `[]` khi tác giả không ghi gì.
ALTER TABLE remote_templates_cache ADD COLUMN changelog TEXT NOT NULL DEFAULT '[]';
