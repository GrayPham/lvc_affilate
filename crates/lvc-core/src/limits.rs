/// Giới hạn cấu hình được TIÊM vào miền thuần.
///
/// Bản Python đọc thẳng `config` trong `scenes.py`, phá vỡ tính thuần.
/// Ở đây gọi hàm nào cần giới hạn thì truyền vào.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// 0 = không giới hạn.
    pub prompt_max_chars: usize,
    pub batch_max_scenes: usize,
    pub reference_image_max_count: usize,
    pub daily_limit_fast: u32,
    pub daily_limit_pro: u32,
    pub daily_limit_pro_max: u32,
    /// Thêm "BGMなし、環境音のみ" vào prompt Pro.
    pub pro_no_bgm: bool,
    /// Lệnh kỹ năng của Pro Max.
    pub pro_max_command: String,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            prompt_max_chars: 0,
            batch_max_scenes: 100,
            reference_image_max_count: 30,
            daily_limit_fast: 2,
            daily_limit_pro: 10,
            daily_limit_pro_max: 10,
            pro_no_bgm: true,
            pro_max_command: "/Create Video".to_string(),
        }
    }
}
