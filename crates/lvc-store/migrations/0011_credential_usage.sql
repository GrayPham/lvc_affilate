-- Bộ đếm "hạn mức" theo NGÀY cho api_key (màn API key: cột "Hạn mức" + "Tự xoay vòng key").
-- `used_today` = số lần key được dùng (lease_secret) trong `used_day`; sang ngày mới thì reset.
-- Ngày tính theo giờ Tokyo (đồng bộ với bộ đếm tài khoản Dola — lvc_core::tokyo_day).
-- `daily_limit` (đã có từ 0004) = ngưỡng; NULL/0 = không giới hạn.
ALTER TABLE credentials ADD COLUMN used_today INTEGER NOT NULL DEFAULT 0;
ALTER TABLE credentials ADD COLUMN used_day TEXT NOT NULL DEFAULT '';
