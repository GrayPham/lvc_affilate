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

/// Thay MOI xuong dong that (`\r\n`/`\r`/`\n`) bang MOT dau cach, roi trim hai dau — dam bao MOT
/// prompt LUON la DUNG MOT dong TXT. CHI dong toi ky tu xuong dong: khoang trang khac (dau cach,
/// tab, nhieu dau cach lien tiep...) giu NGUYEN TUNG BYTE — day KHONG phai ham "don dep" chung
/// chung, chi rieng loai bo ky tu gay sai lech so dong TXT.
///
/// Fix-round 1 (P3c Task 1, C1 — Critical, sua lai theo phat hien proptest
/// `crates/lvc-core/tests/round_trip.rs`): ban dau dung `split_whitespace().join(" ")` — gop LUON
/// ca cac day dau cach thuong (khong lien quan xuong dong) thanh mot dau cach, lam vo bat bien
/// `format_txt(parse_txt(x)) == x` cho prompt hop le co nhieu dau cach lien tiep. Sua: CHI thay the
/// `\r\n` (mot cap) / `\r` don / `\n` don — moi lan xuat hien la MOT dau cach (khong gop nhieu
/// xuong dong lien tiep thanh mot).
///
/// Mot ky tu xuong dong that con lai trong prompt lam `parse_txt`'s `str::lines()` (goi qua
/// [`parse_lines_raw`]) hieu nham la ranh gioi giua HAI canh, tach MOT canh logic thanh NHIEU dong
/// TXT — pha vo bat bien "N canh -> N dong TXT -> N job video" ma
/// `lvc_engine::scripts::run_script` dua vao de anh xa `scene_id -> task_id` theo `scene_index`
/// (xem doc do). Dung boi CA [`format_txt`] (phong thu — bat ky nguon `Scene` nao, khong chi kich
/// ban) LAN `lvc_store::scripts::ScriptRepo::replace_scenes`/`lvc_engine::scripts::run_script`
/// (chuan hoa luc ghi VA luc doc — du lieu cu ghi truoc ban sua nay, hoac nhap legacy R5 ghi thang
/// khong qua `replace_scenes`, van co the con `\n` that trong cot `prompt`).
#[must_use]
pub fn normalize_prompt_line(prompt: &str) -> String {
    prompt
        .replace("\r\n", " ")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

/// Phân tích MỘT dòng cú pháp TXT (`prompt | key=value | …`) thành `RawScene` CHƯA chuẩn
/// hoá — tách khỏi `parse_txt` để `graph::plan` (Task 2, spec §4.3) dùng lại đúng nguyên
/// cú pháp pipe/escape khi cần hoãn việc điền mặc định tới lúc biết node Tạo video nào
/// đang tiêu thụ dòng này (mặc định của node đó, không phải mặc định chung ở đây).
///
/// # Errors
/// `CoreError::ExpectedKeyValue` / `CoreError::UnknownKey` khi cú pháp `key=value` sai.
pub(crate) fn parse_line(line: &str) -> Result<RawScene, CoreError> {
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
            return Err(CoreError::ExpectedKeyValue(part.to_string()));
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
                scene.reference_images = Some(value.split(',').map(str::to_string).collect());
            }
            "acc" | "account" => scene.account = Some(value),
            _ => return Err(CoreError::UnknownKey(key)),
        }
    }
    Ok(scene)
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
        let scene = parse_line(line).map_err(|k| SceneError::new(Some(no), k))?;
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

/// TXT nhiều dòng → `(số dòng 1-based, RawScene CHƯA chuẩn hoá)` — bỏ qua dòng trống/`#`
/// giống `parse_txt`, nhưng KHÔNG gọi `Scene::normalize`: dùng bởi `graph::plan` (Task 2,
/// spec §4.3) để hoãn áp mặc định tới khi biết node Tạo video nào đang tiêu thụ (mặc định
/// của node đó thắng mặc định chung, còn giá trị TXT trên dòng thắng cả hai).
///
/// # Errors
/// `SceneError` kèm số dòng khi cú pháp `key=value` sai ở dòng đó.
pub fn parse_lines_raw(text: &str) -> Result<Vec<(usize, RawScene)>, SceneError> {
    let mut out = Vec::new();
    for (no, raw_line) in text.lines().enumerate() {
        let no = no + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let scene = parse_line(line).map_err(|k| SceneError::new(Some(no), k))?;
        out.push((no, scene));
    }
    Ok(out)
}

/// Ngược lại `parse_txt`: chỉ ghi khoá khác mặc định, theo thứ tự
/// ratio, dur, model, mode, logo, acc, refs.
///
/// Giới hạn kế thừa nguyên vẹn từ bản Python: nếu `prompt` bắt đầu bằng `#`,
/// dòng ghi ra sẽ bị `parse_txt` coi là comment và bỏ qua toàn bộ — cảnh đó
/// KHÔNG round-trip được (`parse_txt(format_txt(&[s])) != vec![s]`). Đây là
/// hành vi gốc, không phải lỗi mới; bộ điều khiển đã quyết định không thêm ký
/// tự thoát `\#` vì sẽ làm hỏng file TXT của bản Python. Người gọi không được
/// dựa vào việc round-trip cho prompt dạng này.
///
/// Fix-round 1 (C1): `s.prompt` luon di qua [`normalize_prompt_line`] TRUOC — dam bao KHONG BAO
/// GIO co ky tu xuong dong that trong dong TXT ghi ra, du `Scene` truyen vao lay tu dau (`format_txt(
/// parse_txt(x))` khong bao gio doi so dong, xem test `format_txt_khong_bao_gio_doi_so_dong_du_prompt_co_xuong_dong`).
#[must_use]
pub fn format_txt(scenes: &[Scene]) -> String {
    let mut lines = Vec::with_capacity(scenes.len());
    for s in scenes {
        let mut parts = vec![escape(&normalize_prompt_line(&s.prompt))];
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
            "line 1: unknown key 'bogus' (allowed: ratio, dur, model, mode, logo, refs, acc)"
        );
        let e = parse_txt("x | noequals", &Limits::default()).unwrap_err();
        assert_eq!(e.to_string(), "line 1: expected key=value, got 'noequals'");
    }

    #[test]
    fn vuot_tran_so_canh_thi_loi() {
        let l = Limits {
            batch_max_scenes: 2,
            ..Limits::default()
        };
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
        let txt =
            "p | refs=a.jpg | acc=me | logo=crop | mode=pro | model=2.5 | dur=30 | ratio=16:9";
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

    /// Fix-round 1 (sua lai theo phat hien tu `tests/round_trip.rs` proptest): CHI xuong dong that
    /// moi bi thay bang dau cach — dau cach thuong (ke ca nhieu dau cach lien tiep) PHAI giu
    /// NGUYEN, khong duoc gop lai. Ghim rieng de khong tai dien loi ban dau (dung
    /// `split_whitespace().join(" ")` gop luon ca day dau cach thuong).
    #[test]
    fn normalize_prompt_line_khong_dung_toi_dau_cach_thuong() {
        assert_eq!(
            normalize_prompt_line("A  a"),
            "A  a",
            "hai dau cach lien tiep (khong lien quan xuong dong) phai giu nguyen"
        );
        assert_eq!(
            normalize_prompt_line("mot   nhieu    dau cach"),
            "mot   nhieu    dau cach"
        );
        // Xuong dong VAN bi thay — chi rieng no.
        assert_eq!(normalize_prompt_line("a\nb"), "a b");
    }

    /// Fix-round 1 (P3c Task 1, C1 — Critical): mot Scene co `\n` that GIUA prompt (vd nguoi dung
    /// dan van ban nhieu dong vao o prompt cua kich ban) PHAI van ra DUNG MOT dong TXT — neu khong,
    /// `parse_lines_raw`/`parse_txt` (dung `str::lines()`) se tach MOT canh thanh HAI, lam sai lech
    /// map `scene_index -> task_id` (xem `lvc_engine::scripts::run_script`).
    #[test]
    fn format_txt_khong_bao_gio_doi_so_dong_du_prompt_co_xuong_dong() {
        let scene = Scene {
            prompt: "dong mot\ndong hai\r\ndong ba\rdong bon".to_string(),
            ratio: Ratio::default(),
            duration: DEFAULT_DURATION,
            model: Model::default(),
            mode: Mode::default(),
            logo: LogoMode::default(),
            reference_images: Vec::new(),
            account: String::new(),
        };
        let text = format_txt(std::slice::from_ref(&scene));
        assert_eq!(
            text.lines().count(),
            1,
            "mot Scene phai luon ra DUNG mot dong TXT: {text:?}"
        );
        assert_eq!(text, "dong mot dong hai dong ba dong bon\n");

        // format_txt(parse_txt(x)) khong bao gio doi so dong — bat bien "N canh -> N dong TXT".
        let back = parse_txt(&text, &Limits::default()).unwrap();
        assert_eq!(back.len(), 1, "phai van la DUNG mot canh sau khi parse lai");
    }

    /// Bat bien tong quat hon (nhieu canh, canh giua co xuong dong) — so dong TXT/so canh khop
    /// nhau TRUOC va SAU mot vong `format_txt` -> `parse_txt`.
    #[test]
    fn format_txt_parse_txt_giu_dung_so_canh_du_mot_canh_co_xuong_dong() {
        let scenes = vec![
            Scene {
                prompt: "canh mot".to_string(),
                ratio: Ratio::default(),
                duration: DEFAULT_DURATION,
                model: Model::default(),
                mode: Mode::default(),
                logo: LogoMode::default(),
                reference_images: Vec::new(),
                account: String::new(),
            },
            Scene {
                prompt: "canh hai\ndong phu cua canh hai".to_string(),
                ratio: Ratio::default(),
                duration: DEFAULT_DURATION,
                model: Model::default(),
                mode: Mode::default(),
                logo: LogoMode::default(),
                reference_images: Vec::new(),
                account: String::new(),
            },
            Scene {
                prompt: "canh ba".to_string(),
                ratio: Ratio::default(),
                duration: DEFAULT_DURATION,
                model: Model::default(),
                mode: Mode::default(),
                logo: LogoMode::default(),
                reference_images: Vec::new(),
                account: String::new(),
            },
        ];
        let text = format_txt(&scenes);
        assert_eq!(text.lines().count(), 3, "{text:?}");
        let back = parse_txt(&text, &Limits::default()).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[1].prompt, "canh hai dong phu cua canh hai");
    }

    #[test]
    fn prompt_bat_dau_bang_thang_khong_round_trip_duoc_ghim_gioi_han() {
        // Ghim gioi han ke thua tu ban Python: format_txt ghi prompt nguyen
        // van, con parse_txt coi dong bat dau bang '#' la comment. Neu hanh vi
        // nay thay doi (co y hay khong), test nay se do va bao ngay.
        let scene = Scene {
            prompt: "#1 canh mo dau".to_string(),
            ratio: Ratio::default(),
            duration: DEFAULT_DURATION,
            model: Model::default(),
            mode: Mode::default(),
            logo: LogoMode::default(),
            reference_images: Vec::new(),
            account: String::new(),
        };
        let text = format_txt(std::slice::from_ref(&scene));
        let parsed = parse_txt(&text, &Limits::default()).unwrap();
        assert!(
            parsed.is_empty(),
            "prompt bat dau bang # bi parse_txt nuot mat, dung nhu ban Python"
        );
    }
}
