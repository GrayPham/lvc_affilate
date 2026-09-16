use crate::Ratio;

/// Ghép yêu cầu thời lượng/tỉ lệ (tiếng Nhật) vào đầu prompt cho agent Pro.
///
/// Không có cả hai → trả nguyên prompt (không bịa thêm).
/// Đã đo thật: `"15秒の縦動画（9:16）を1本作ってください: ..."` → agent tạo đúng 15s 9:16.
#[must_use]
pub fn pro_prompt(
    prompt: &str,
    ratio: Option<Ratio>,
    duration: Option<u16>,
    no_bgm: bool,
) -> String {
    let prompt = prompt.trim();
    let head = match (duration, ratio) {
        (Some(d), Some(r)) => format!("{d}秒の{}", r.japanese()),
        (Some(d), None) => format!("{d}秒の動画"),
        (None, Some(r)) => r.japanese().to_string(),
        (None, None) => return prompt.to_string(),
    };
    let mut text = format!("{head}を1本作ってください。内容: {prompt}");
    if no_bgm {
        // Tránh agent tự thêm nhạc → lỗi bản quyền âm thanh (IDEA-012).
        text.push_str("。BGMなし、環境音のみ");
    }
    text
}

/// Pro Max: gõ thẳng lệnh kỹ năng vào ô chat thay vì chọn スキル → 動画の生成.
#[must_use]
pub fn pro_max_prompt(
    prompt: &str,
    ratio: Option<Ratio>,
    duration: Option<u16>,
    no_bgm: bool,
    command: &str,
) -> String {
    format!(
        "{} {}",
        command.trim(),
        pro_prompt(prompt, ratio, duration, no_bgm)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ratio;

    #[test]
    fn day_du_thoi_luong_va_ti_le() {
        let out = pro_prompt("con meo", Some(Ratio::R9x16), Some(15), true);
        assert_eq!(
            out,
            "15秒の縦動画（9:16）を1本作ってください。内容: con meo。BGMなし、環境音のみ"
        );
    }

    #[test]
    fn tat_bgm_thi_khong_them_duoi() {
        let out = pro_prompt("x", Some(Ratio::R16x9), Some(10), false);
        assert_eq!(out, "10秒の横動画（16:9）を1本作ってください。内容: x");
    }

    #[test]
    fn chi_co_thoi_luong() {
        let out = pro_prompt("x", None, Some(30), false);
        assert_eq!(out, "30秒の動画を1本作ってください。内容: x");
    }

    #[test]
    fn chi_co_ti_le() {
        let out = pro_prompt("x", Some(Ratio::R1x1), None, false);
        assert_eq!(out, "正方形動画（1:1）を1本作ってください。内容: x");
    }

    #[test]
    fn khong_co_gi_thi_giu_nguyen_prompt_khong_bia() {
        assert_eq!(pro_prompt("  x  ", None, None, true), "x");
    }

    #[test]
    fn pro_max_them_lenh_ky_nang() {
        let out = pro_max_prompt("x", None, Some(10), false, "/Create Video");
        assert_eq!(out, "/Create Video 10秒の動画を1本作ってください。内容: x");
    }
}
