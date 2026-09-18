use std::sync::LazyLock;

use regex::Regex;

use crate::{Model, Ratio};

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

/// `modes.py:406-418` `agent_model_label` — khoá model → tên agent hiểu. `Model` đã chuẩn hoá nên
/// không cần các alias; Python `model=None` (không rõ) → "Seedance 2.0" = `Model::default()`.
#[must_use]
pub fn agent_model_label(model: Model) -> &'static str {
    match model {
        Model::Seedance25 => "Seedance 2.5",
        Model::Seedance20 => "Seedance 2.0",
        Model::Seedance10 => "Seedance 1.0",
    }
}

/// `modes.py:435` — dòng `Style: …` trong prompt (Cố định style của kịch bản) khi không truyền `style`.
static STYLE_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*style\s*:\s*(.+?)\s*$").expect("regex STYLE_LINE_RE hợp lệ")
});

/// Dola Agent (TUT Bỏ skill): câu lệnh chat tiếng Anh, không skill, không `/Create Video`.
/// Chèn sẵn tham số + chỉ thị cưỡng chế tạo video một lần (1-shot) — port `modes.py:421-451`
/// (HEAD 3d07c19):
/// - `MODEL=` theo `model` của cảnh ([`agent_model_label`]); 2.5 → cost note "accept the higher credit
///   cost…", khác → "exactly this model…";
/// - `VIDEO_STYLE=` CHỈ khi có `style` (đã strip, không rỗng) hoặc prompt có dòng `Style: …`
///   (`(?im)^\s*style\s*:\s*(.+?)\s*$`, khớp ĐẦU tiên trên prompt đã strip) — không còn cứng
///   "cinematic fairy tale 3D animation";
/// - câu cuối "Return the finished video as the standard video card only …".
///
/// Mặc định `16:9` và `10s` — **khác** mặc định `9:16` của scene, theo đúng Python.
#[must_use]
pub fn agent_prompt(
    prompt: &str,
    ratio: Option<Ratio>,
    duration: Option<u16>,
    model: Model,
    style: Option<&str>,
) -> String {
    let prompt = prompt.trim();
    let r = ratio.unwrap_or(Ratio::R16x9).as_str();
    let d = duration.unwrap_or(10);
    let m = agent_model_label(model);
    // modes.py:433-437: `style` (strip) rỗng → dòng `Style: …` đầu tiên trong prompt (đã strip).
    let style = style
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            STYLE_LINE_RE
                .captures(prompt)
                .and_then(|c| c.get(1))
                .map(|g| g.as_str().trim())
        })
        .unwrap_or("");
    let style_param = if style.is_empty() {
        String::new()
    } else {
        format!(", VIDEO_STYLE={style}")
    };
    let cost_note = if m == "Seedance 2.5" {
        " (accept the higher credit cost, do not downgrade to a cheaper model)"
    } else {
        " (exactly this model, do not switch to another)"
    };
    format!(
        "Create a {r} video (duration: {d}s).\n\
         [PARAMETERS: DISPLAY_FORMAT={r}, DURATION={d}s, SCENE_COUNT=1, MODEL={m}{style_param}, \
         DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\n\
         DIRECTIVE: Use {m}{cost_note}. \
         Do NOT ask for missing parameters. Do NOT ask for confirmation. \
         Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now. \
         Return the finished video as the standard video card only - do NOT paste the video URL, markdown link or HTML <video> tag.\n\n\
         Content:\n{prompt}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Model, Ratio};

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

    // ---- agent_prompt (modes.py:421-451, HEAD 3d07c19) — moi chuoi ky vong lay tu oracle
    // `.venv` Python: `modes.agent_prompt(prompt, ratio, duration, model=..., style=...)` ----

    /// `model=seedance_v2.5`, `style=None`, prompt khong co dong `Style:` (oracle, 591 ky tu).
    #[test]
    fn agent_prompt_2_5_khong_style_khop_python_tung_ky_tu() {
        let expected = "Create a 9:16 video (duration: 15s).\n[PARAMETERS: DISPLAY_FORMAT=9:16, DURATION=15s, SCENE_COUNT=1, MODEL=Seedance 2.5, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\nDIRECTIVE: Use Seedance 2.5 (accept the higher credit cost, do not downgrade to a cheaper model). Do NOT ask for missing parameters. Do NOT ask for confirmation. Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now. Return the finished video as the standard video card only - do NOT paste the video URL, markdown link or HTML <video> tag.\n\nContent:\ncon meo";
        let out = agent_prompt(
            "con meo",
            Some(Ratio::R9x16),
            Some(15),
            Model::Seedance25,
            None,
        );
        assert_eq!(out, expected);
        assert_eq!(out.chars().count(), 591);
    }

    /// `model=seedance_v2.0` -> `MODEL=Seedance 2.0` + cost note "(exactly this model, ...)" (oracle,
    /// 569 ky tu).
    #[test]
    fn agent_prompt_2_0_cost_note_exactly_this_model() {
        let expected = "Create a 9:16 video (duration: 15s).\n[PARAMETERS: DISPLAY_FORMAT=9:16, DURATION=15s, SCENE_COUNT=1, MODEL=Seedance 2.0, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\nDIRECTIVE: Use Seedance 2.0 (exactly this model, do not switch to another). Do NOT ask for missing parameters. Do NOT ask for confirmation. Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now. Return the finished video as the standard video card only - do NOT paste the video URL, markdown link or HTML <video> tag.\n\nContent:\ncon meo";
        let out = agent_prompt(
            "con meo",
            Some(Ratio::R9x16),
            Some(15),
            Model::Seedance20,
            None,
        );
        assert_eq!(out, expected);
        assert_eq!(out.chars().count(), 569);
    }

    /// `model=seedance_v1.0`, `style="cinematic realism"` -> `VIDEO_STYLE=` chen sau MODEL (oracle,
    /// 594 ky tu).
    #[test]
    fn agent_prompt_1_0_co_style_tham_so() {
        let expected = "Create a 16:9 video (duration: 10s).\n[PARAMETERS: DISPLAY_FORMAT=16:9, DURATION=10s, SCENE_COUNT=1, MODEL=Seedance 1.0, VIDEO_STYLE=cinematic realism, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\nDIRECTIVE: Use Seedance 1.0 (exactly this model, do not switch to another). Do NOT ask for missing parameters. Do NOT ask for confirmation. Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now. Return the finished video as the standard video card only - do NOT paste the video URL, markdown link or HTML <video> tag.\n\nContent:\nx";
        let out = agent_prompt(
            "x",
            Some(Ratio::R16x9),
            Some(10),
            Model::Seedance10,
            Some("cinematic realism"),
        );
        assert_eq!(out, expected);
        assert_eq!(out.chars().count(), 594);
    }

    /// Python `model=None` -> `agent_model_label` tra "Seedance 2.0" (mac dinh giao dien) =
    /// `Model::default()`; `ratio or "16:9"`, `duration or 10` (oracle, 563 ky tu).
    #[test]
    fn agent_prompt_mac_dinh_16_9_10s_model_2_0() {
        let expected = "Create a 16:9 video (duration: 10s).\n[PARAMETERS: DISPLAY_FORMAT=16:9, DURATION=10s, SCENE_COUNT=1, MODEL=Seedance 2.0, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\nDIRECTIVE: Use Seedance 2.0 (exactly this model, do not switch to another). Do NOT ask for missing parameters. Do NOT ask for confirmation. Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now. Return the finished video as the standard video card only - do NOT paste the video URL, markdown link or HTML <video> tag.\n\nContent:\nx";
        assert_eq!(Model::default(), Model::Seedance20);
        let out = agent_prompt("x", None, None, Model::default(), None);
        assert_eq!(out, expected);
        assert_eq!(out.chars().count(), 563);
    }

    /// Khong co `style` -> lay dong `Style: ...` DAU TIEN trong prompt (da strip) bang
    /// `(?im)^\s*style\s*:\s*(.+?)\s*$`; prompt trong `Content:` GIU NGUYEN dong do (oracle, 651 ky tu).
    #[test]
    fn agent_prompt_style_lay_tu_dong_style_trong_prompt() {
        let expected = "Create a 9:16 video (duration: 10s).\n[PARAMETERS: DISPLAY_FORMAT=9:16, DURATION=10s, SCENE_COUNT=1, MODEL=Seedance 2.5, VIDEO_STYLE=thực tế điện ảnh, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]\nDIRECTIVE: Use Seedance 2.5 (accept the higher credit cost, do not downgrade to a cheaper model). Do NOT ask for missing parameters. Do NOT ask for confirmation. Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now. Return the finished video as the standard video card only - do NOT paste the video URL, markdown link or HTML <video> tag.\n\nContent:\nStyle: thực tế điện ảnh  \nmột con mèo";
        let out = agent_prompt(
            "  Style: thực tế điện ảnh  \nmột con mèo",
            Some(Ratio::R9x16),
            Some(10),
            Model::Seedance25,
            None,
        );
        assert_eq!(out, expected);
        assert_eq!(out.chars().count(), 651);
    }

    /// `style` tham so (da strip) THANG dong `Style:` trong prompt; `style` toan khoang trang = khong
    /// co -> lay tu prompt (`style :  C ` khong phan biet hoa/thuong, khoang trang quanh `:`);
    /// nhieu dong `Style:` -> dong DAU; `\r\n` -> `\s*$` an `\r` (oracle: 619 / 589 / 607 / 603 ky tu).
    #[test]
    fn agent_prompt_style_uu_tien_va_bien() {
        let wins = agent_prompt(
            "Style: A\nmột con mèo",
            Some(Ratio::R9x16),
            Some(10),
            Model::Seedance25,
            Some("  B  "),
        );
        assert!(
            wins.contains(", MODEL=Seedance 2.5, VIDEO_STYLE=B, DIALOGUE_DENSITY"),
            "{wins}"
        );
        assert!(
            wins.ends_with("\n\nContent:\nStyle: A\nmột con mèo"),
            "{wins}"
        );
        assert_eq!(wins.chars().count(), 619);

        let blank = agent_prompt(
            "style :  C \n x",
            Some(Ratio::R1x1),
            Some(15),
            Model::Seedance20,
            Some("   "),
        );
        assert!(
            blank.contains(", MODEL=Seedance 2.0, VIDEO_STYLE=C, DIALOGUE_DENSITY"),
            "{blank}"
        );
        assert!(blank.ends_with("\n\nContent:\nstyle :  C \n x"), "{blank}");
        assert_eq!(blank.chars().count(), 589);

        let multi = agent_prompt(
            "a\nStyle: first\nStyle: second",
            Some(Ratio::R4x3),
            Some(10),
            Model::Seedance20,
            None,
        );
        assert!(
            multi.contains("VIDEO_STYLE=first, DIALOGUE_DENSITY"),
            "{multi}"
        );
        assert_eq!(multi.chars().count(), 607);

        let crlf = agent_prompt(
            "Style:   spaced   \r\nmèo",
            Some(Ratio::R3x4),
            Some(10),
            Model::Seedance20,
            None,
        );
        assert!(
            crlf.contains("VIDEO_STYLE=spaced, DIALOGUE_DENSITY"),
            "{crlf}"
        );
        assert!(
            crlf.ends_with("\n\nContent:\nStyle:   spaced   \r\nmèo"),
            "{crlf}"
        );
        assert_eq!(crlf.chars().count(), 603);
    }

    /// `agent_model_label` (modes.py:406-418) tren 3 model; `AGENT_MODEL_LABEL` (`:406`).
    #[test]
    fn agent_model_label_theo_modes_py() {
        assert_eq!(agent_model_label(Model::Seedance25), "Seedance 2.5");
        assert_eq!(agent_model_label(Model::Seedance20), "Seedance 2.0");
        assert_eq!(agent_model_label(Model::Seedance10), "Seedance 1.0");
    }
}
