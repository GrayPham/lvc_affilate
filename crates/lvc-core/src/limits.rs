/// Giới hạn cấu hình được TIÊM vào miền thuần.
///
/// Bản Python đọc thẳng `config` trong `scenes.py`, phá vỡ tính thuần.
/// Ở đây gọi hàm nào cần giới hạn thì truyền vào.
// 4 truong bool (pro_enabled, pro_max_enabled, agent_enabled, pro_no_bgm) la co bat/tat
// doc lap theo dung Limits cua brief Task 1/4 — khong phai trang thai loai tru lan nhau
// nen khong gop duoc thanh enum; clippy::struct_excessive_bools la pedantic, khong ap dung.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// 0 = không giới hạn.
    pub prompt_max_chars: usize,
    pub batch_max_scenes: usize,
    pub reference_image_max_count: usize,
    pub daily_limit_fast: u32,
    pub daily_limit_pro: u32,
    pub daily_limit_pro_max: u32,
    pub daily_limit_agent: u32,
    /// Cờ bật/tắt chế độ — dùng bởi `mode_fallback_chain`.
    pub pro_enabled: bool,
    pub pro_max_enabled: bool,
    pub agent_enabled: bool,
    /// Thêm "BGMなし、環境音のみ" vào prompt Pro.
    pub pro_no_bgm: bool,
    /// Lệnh kỹ năng của Pro Max.
    pub pro_max_command: String,

    // ---- modes.py / config.py: timeout & mốc chờ khi tạo video (Task 4) ----
    /// config.py:92 `VIDEO_TIMEOUT` — timeout cơ sở (giây) trước khi cộng dồn theo mode/thời lượng/ảnh tham chiếu.
    pub video_timeout_s: u32,
    /// config.py:178 `PRO_VIDEO_TIMEOUT` — sàn timeout khi mode pro-like (agent Pro chạy nhiều bước, đo thật ~600s).
    pub pro_video_timeout_s: u32,
    /// modes.py:275 `LONG_VIDEO_TIMEOUT` — sàn timeout khi thời lượng video ≥ 30s.
    pub long_video_timeout_s: u32,
    /// config.py:168 `REFERENCE_VIDEO_TIMEOUT` — sàn timeout khi task có ảnh tham chiếu.
    pub reference_video_timeout_s: u32,
    /// config.py:180 `PRO_MAX_TIMEOUT` — trần gia hạn khi agent Pro còn hoạt động gần đây.
    pub pro_max_timeout_s: u32,
    /// config.py:181 `PRO_ACTIVITY_WINDOW` — "còn hoạt động gần đây" trong ngần này giây thì được gia hạn poll.
    pub pro_activity_window_s: u32,
    /// `video_worker_ui.py:1499` `await asyncio.sleep(5)` — chu kỳ poll trạng thái tạo video.
    pub poll_interval_s: u32,
    /// config.py:287 `REFERENCE_UPLOAD_TIMEOUT` — chờ Dola nhận xong ảnh tham chiếu trước khi gửi prompt.
    pub reference_upload_timeout_s: u32,
    /// config.py:205 `AGENT_QUESTION_GRACE` — agent hỏi lại người dùng, không có video sau ngần này giây thì fail sớm.
    pub agent_question_grace_s: u32,
    /// config.py:208 `AGENT_IDLE_FAIL` — agent trả lời xong bằng chữ (không video) và im lặng ngần này giây thì fail (0 = tắt).
    pub agent_idle_fail_s: u32,
    /// config.py:292 `CONFIRM_MAX_REPLIES` — agent Pro xin xác nhận nhiều lần, trả lời tối đa N lần/hội thoại.
    pub confirm_max_replies: u32,
    /// config.py:295 `LINK_RAW_GRACE` — thấy link video trước khối video nhưng chưa khớp bản raw, đợi thêm tối đa ngần này giây.
    pub link_raw_grace_s: u32,
    /// video_worker_ui.py:2563-2600 `for i in range(30): await page.wait_for_timeout(1000)` —
    /// 30 giây không có `conversation_id` thì `NO_CONVERSATION` (literal trong vòng lặp, không phải hằng số Python).
    pub no_conversation_timeout_s: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            prompt_max_chars: 0,
            batch_max_scenes: 100,
            reference_image_max_count: 30,
            daily_limit_fast: 30,
            daily_limit_pro: 10,
            daily_limit_pro_max: 10,
            daily_limit_agent: 30,
            pro_enabled: true,
            pro_max_enabled: true,
            agent_enabled: true,
            pro_no_bgm: true,
            pro_max_command: "/Create Video".to_string(),
            video_timeout_s: 300,
            pro_video_timeout_s: 1800,
            long_video_timeout_s: 1500,
            reference_video_timeout_s: 900,
            pro_max_timeout_s: 3600,
            pro_activity_window_s: 600,
            poll_interval_s: 5,
            reference_upload_timeout_s: 90,
            agent_question_grace_s: 60,
            agent_idle_fail_s: 120,
            confirm_max_replies: 3,
            link_raw_grace_s: 45,
            no_conversation_timeout_s: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_dinh_khop_config_py() {
        let l = Limits::default();
        assert_eq!(l.video_timeout_s, 300, "config.py:92 VIDEO_TIMEOUT");
        assert_eq!(
            l.pro_video_timeout_s, 1800,
            "config.py:178 PRO_VIDEO_TIMEOUT"
        );
        assert_eq!(
            l.long_video_timeout_s, 1500,
            "modes.py:275 LONG_VIDEO_TIMEOUT"
        );
        assert_eq!(
            l.reference_video_timeout_s, 900,
            "config.py:168 REFERENCE_VIDEO_TIMEOUT"
        );
        assert_eq!(l.pro_max_timeout_s, 3600, "config.py:180 PRO_MAX_TIMEOUT");
        assert_eq!(
            l.pro_activity_window_s, 600,
            "config.py:181 PRO_ACTIVITY_WINDOW"
        );
        assert_eq!(l.poll_interval_s, 5, "video_worker_ui.py:1499 sleep(5)");
        assert_eq!(
            l.reference_upload_timeout_s, 90,
            "config.py:287 REFERENCE_UPLOAD_TIMEOUT"
        );
        assert_eq!(
            l.agent_question_grace_s, 60,
            "config.py:205 AGENT_QUESTION_GRACE"
        );
        assert_eq!(l.agent_idle_fail_s, 120, "config.py:208 AGENT_IDLE_FAIL");
        assert_eq!(
            l.confirm_max_replies, 3,
            "config.py:292 CONFIRM_MAX_REPLIES"
        );
        assert_eq!(l.link_raw_grace_s, 45, "config.py:295 LINK_RAW_GRACE");
        assert_eq!(
            l.no_conversation_timeout_s, 30,
            "video_worker_ui.py:2563-2600 range(30) x wait_for_timeout(1000)"
        );
    }
}
