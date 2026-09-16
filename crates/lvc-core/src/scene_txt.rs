use crate::{CoreError, Limits, RawScene, Scene, SceneError, scene::DEFAULT_DURATION};

/// Tách theo `|` KHÔNG đứng ngay sau dấu `\`.
///
/// Sao đúng ngữ nghĩa regex `(?<!\\)\|` của bản Python: chỉ nhìn đúng một ký tự
/// phía trước, nên `\\|` cũng không phải điểm tách (giống Python).
fn split_unescaped_pipe(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '|' && (i == 0 || chars[i - 1] != '\\') {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    parts.push(current);
    parts
}

fn unescape(s: &str) -> String {
    s.replace("\\|", "|")
}

fn escape(s: &str) -> String {
    s.replace('|', "\\|")
}

/// TXT → danh sách cảnh đã chuẩn hoá.
///
/// # Errors
/// `SceneError` kèm số dòng (1-based) tại dòng đầu tiên sai.
pub fn parse_txt(text: &str, limits: &Limits) -> Result<Vec<Scene>, SceneError> {
    let mut scenes = Vec::new();
    for (no, raw_line) in text.lines().enumerate() {
        let no = no + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts = split_unescaped_pipe(line);
        let mut scene = RawScene {
            prompt: Some(unescape(parts[0].trim())),
            ..RawScene::default()
        };
        for part in &parts[1..] {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some((key, value)) = part.split_once('=') else {
                return Err(SceneError::new(
                    Some(no),
                    CoreError::ExpectedKeyValue(part.to_string()),
                ));
            };
            let key = key.trim().to_lowercase();
            let value = unescape(value.trim());
            match key.as_str() {
                "ratio" => scene.ratio = Some(value),
                "dur" | "duration" => scene.duration = Some(value),
                "model" => scene.model = Some(value),
                "mode" => scene.mode = Some(value),
                "logo" => scene.logo = Some(value),
                "ref" | "refs" => {
                    scene.reference_images =
                        Some(value.split(',').map(str::to_string).collect());
                }
                "acc" | "account" => scene.account = Some(value),
                _ => return Err(SceneError::new(Some(no), CoreError::UnknownKey(key))),
            }
        }
        scenes.push(Scene::normalize(&scene, limits, Some(no))?);
        if scenes.len() > limits.batch_max_scenes {
            return Err(SceneError::new(
                Some(no),
                CoreError::TooManyScenes(limits.batch_max_scenes),
            ));
        }
    }
    Ok(scenes)
}

/// Ngược lại `parse_txt`: chỉ ghi khoá khác mặc định, theo thứ tự
/// ratio, dur, model, mode, logo, acc, refs.
#[must_use]
pub fn format_txt(scenes: &[Scene]) -> String {
    let mut lines = Vec::with_capacity(scenes.len());
    for s in scenes {
        let mut parts = vec![escape(&s.prompt)];
        if s.ratio != crate::Ratio::default() {
            parts.push(format!("ratio={}", s.ratio.as_str()));
        }
        if s.duration != DEFAULT_DURATION {
            parts.push(format!("dur={}", s.duration));
        }
        if s.model != crate::Model::default() {
            parts.push(format!("model={}", s.model.as_str()));
        }
        if s.mode != crate::Mode::default() {
            parts.push(format!("mode={}", s.mode.as_str()));
        }
        if s.logo != crate::LogoMode::default() {
            parts.push(format!("logo={}", s.logo.as_str()));
        }
        if !s.account.is_empty() {
            parts.push(format!("acc={}", s.account));
        }
        if !s.reference_images.is_empty() {
            let refs: Vec<String> = s.reference_images.iter().map(|r| escape(r)).collect();
            parts.push(format!("refs={}", refs.join(",")));
        }
        lines.push(parts.join(" | "));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LogoMode, Mode, Model, Ratio};

    #[test]
    fn bo_qua_dong_trong_va_comment() {
        let txt = "\n# ghi chu\n  \nmot canh\n";
        let s = parse_txt(txt, &Limits::default()).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].prompt, "mot canh");
    }

    #[test]
    fn doc_day_du_khoa_va_alias() {
        let txt = "canh mot | ratio=16:9 | dur=30s | model=2.5 | mode=pro | logo=crop | acc=acc2 | refs=a.jpg,https://x/y.png";
        let s = parse_txt(txt, &Limits::default()).unwrap();
        assert_eq!(s[0].ratio, Ratio::R16x9);
        assert_eq!(s[0].duration, 30);
        assert_eq!(s[0].model, Model::Seedance25);
        assert_eq!(s[0].mode, Mode::Pro);
        assert_eq!(s[0].logo, LogoMode::Crop);
        assert_eq!(s[0].account, "acc2");
        assert_eq!(s[0].reference_images, vec!["a.jpg", "https://x/y.png"]);
    }

    #[test]
    fn dau_gach_dung_trong_prompt_phai_escape() {
        let s = parse_txt(r"a \| b | dur=30", &Limits::default()).unwrap();
        assert_eq!(s[0].prompt, "a | b");
        assert_eq!(s[0].duration, 30);
    }

    #[test]
    fn khoa_la_va_thieu_dau_bang_thi_loi() {
        let e = parse_txt("x | bogus=1", &Limits::default()).unwrap_err();
        assert_eq!(
            e.to_string(),
            r#"line 1: unknown key "bogus" (allowed: ratio, dur, model, mode, logo, refs, acc)"#
        );
        let e = parse_txt("x | noequals", &Limits::default()).unwrap_err();
        assert_eq!(e.to_string(), r#"line 1: expected key=value, got "noequals""#);
    }

    #[test]
    fn vuot_tran_so_canh_thi_loi() {
        let l = Limits { batch_max_scenes: 2, ..Limits::default() };
        let txt = "a\nb\nc\n";
        let e = parse_txt(txt, &l).unwrap_err();
        assert_eq!(e.to_string(), "line 3: too many scenes (max 2)");
        // dung bang tran thi khong loi
        assert_eq!(parse_txt("a\nb\n", &l).unwrap().len(), 2);
    }

    #[test]
    fn format_chi_ghi_khoa_khac_mac_dinh() {
        let s = parse_txt("chi mot prompt", &Limits::default()).unwrap();
        assert_eq!(format_txt(&s), "chi mot prompt\n");
    }

    #[test]
    fn format_theo_dung_thu_tu_ratio_dur_model_mode_logo_acc_refs() {
        let txt = "p | refs=a.jpg | acc=me | logo=crop | mode=pro | model=2.5 | dur=30 | ratio=16:9";
        let s = parse_txt(txt, &Limits::default()).unwrap();
        assert_eq!(
            format_txt(&s),
            "p | ratio=16:9 | dur=30 | model=seedance-2.5 | mode=pro | logo=crop | acc=me | refs=a.jpg\n"
        );
    }

    #[test]
    fn format_rong_thi_chuoi_rong() {
        assert_eq!(format_txt(&[]), "");
    }

    #[test]
    fn round_trip_voi_dau_gach_dung() {
        let original = parse_txt(r"a \| b | mode=pro", &Limits::default()).unwrap();
        let round = parse_txt(&format_txt(&original), &Limits::default()).unwrap();
        assert_eq!(original, round);
    }
}
