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
    // Python 8ec1cfa: hậu tố VÔ ĐIỀU KIỆN — agent Pro hay hỏi lại/xin xác nhận.
    text.push_str(
        "。確認や質問は不要です。最適な設定を自動選択し、直ちに動画生成を開始してください。",
    );
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

/// Dola Agent (TUT Bỏ skill): câu lệnh chat tiếng Anh, không skill, không `/Create Video`.
/// Chèn sẵn tham số + chỉ thị cưỡng chế tạo video một lần (1-shot).
/// Mặc định `16:9` và `10s` — **khác** mặc định `9:16` của scene, theo đúng Python.
#[must_use]
pub fn agent_prompt(prompt: &str, ratio: Option<Ratio>, duration: Option<u16>) -> String {
    let prompt = prompt.trim();
    let r = ratio.unwrap_or(Ratio::R16x9).as_str();
    let d = duration.unwrap_or(10);
    format!(
        "Create a {r} video (duration: {d}s).\n\
         [PARAMETERS: DISPLAY_FORMAT={r}, DURATION={d}s, SCENE_COUNT=1, MODEL=Seedance 2.5, \
         VIDEO_STYLE=cinematic fairy tale 3D animation, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\n\
         DIRECTIVE: Use Seedance 2.5 (accept the higher credit cost, do not downgrade to a cheaper model). \
         Do NOT ask for missing parameters. Do NOT ask for confirmation. \
         Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now.\n\n\
         Content:\n{prompt}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ratio;

    const SUFFIX: &str =
        "。確認や質問は不要です。最適な設定を自動選択し、直ちに動画生成を開始してください。";

    #[test]
    fn day_du_thoi_luong_va_ti_le() {
        let out = pro_prompt("con meo", Some(Ratio::R9x16), Some(15), true);
        assert_eq!(
            out,
            format!(
                "15秒の縦動画（9:16）を1本作ってください。内容: con meo。BGMなし、環境音のみ{SUFFIX}"
            )
        );
    }

    #[test]
    fn tat_bgm_van_co_hau_to_chong_xac_nhan() {
        let out = pro_prompt("x", Some(Ratio::R16x9), Some(10), false);
        assert_eq!(
            out,
            format!("10秒の横動画（16:9）を1本作ってください。内容: x{SUFFIX}")
        );
    }

    #[test]
    fn chi_co_thoi_luong() {
        assert_eq!(
            pro_prompt("x", None, Some(30), false),
            format!("30秒の動画を1本作ってください。内容: x{SUFFIX}")
        );
    }

    #[test]
    fn chi_co_ti_le() {
        assert_eq!(
            pro_prompt("x", Some(Ratio::R1x1), None, false),
            format!("正方形動画（1:1）を1本作ってください。内容: x{SUFFIX}")
        );
    }

    #[test]
    fn khong_co_gi_thi_giu_nguyen_prompt_khong_bia() {
        assert_eq!(pro_prompt("  x  ", None, None, true), "x");
    }

    #[test]
    fn pro_max_them_lenh_ky_nang() {
        assert_eq!(
            pro_max_prompt("x", None, Some(10), false, "/Create Video"),
            format!("/Create Video 10秒の動画を1本作ってください。内容: x{SUFFIX}")
        );
    }

    #[test]
    fn agent_prompt_khop_python_tung_ky_tu() {
        // Lay tu Python 8ec1cfa: modes.agent_prompt('con meo', '9:16', 15)
        let expected = "Create a 9:16 video (duration: 15s).\n[PARAMETERS: DISPLAY_FORMAT=9:16, DURATION=15s, SCENE_COUNT=1, MODEL=Seedance 2.5, VIDEO_STYLE=cinematic fairy tale 3D animation, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\nDIRECTIVE: Use Seedance 2.5 (accept the higher credit cost, do not downgrade to a cheaper model). Do NOT ask for missing parameters. Do NOT ask for confirmation. Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now.\n\nContent:\ncon meo";
        assert_eq!(
            agent_prompt("con meo", Some(Ratio::R9x16), Some(15)),
            expected
        );
    }

    #[test]
    fn agent_prompt_mac_dinh_16_9_va_10s() {
        // Python: ratio or "16:9", duration or 10 — KHAC mac dinh 9:16 cua scene.
        let out = agent_prompt("x", None, None);
        assert!(
            out.starts_with("Create a 16:9 video (duration: 10s).\n"),
            "{out}"
        );
        assert!(out.ends_with("\n\nContent:\nx"));
    }
}
