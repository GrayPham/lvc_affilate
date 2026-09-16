use crate::{CoreError, Limits, RawScene, Scene, SceneError};

/// Văn bản nhiều prompt → danh sách prompt.
///
/// `delimiter` rỗng/chỉ khoảng trắng → mỗi dòng không trống là một cảnh.
/// Có `delimiter` → tách theo CHUỖI LITERAL (không phải regex), giữ xuống dòng
/// bên trong một cảnh.
///
/// # Errors
/// `CoreError::TooManyScenes` khi vượt `limits.batch_max_scenes`.
pub fn split_bulk(
    text: &str,
    delimiter: &str,
    limits: &Limits,
) -> Result<Vec<String>, SceneError> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let delim = delimiter.trim();
    let parts: Vec<String> = if delim.is_empty() {
        normalized.split('\n').map(str::to_string).collect()
    } else {
        normalized.split(delim).map(str::to_string).collect()
    };
    let out: Vec<String> = parts
        .iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if out.len() > limits.batch_max_scenes {
        return Err(SceneError::new(
            Some(out.len()),
            CoreError::TooManyScenes(limits.batch_max_scenes),
        ));
    }
    Ok(out)
}

/// Mỗi prompt tách được → một cảnh chuẩn hoá với mặc định chung.
///
/// # Errors
/// `CoreError::NoScenes` khi không có prompt nào; `SceneError` kèm số thứ tự
/// cảnh khi một giá trị mặc định sai.
pub fn bulk_scenes(
    text: &str,
    delimiter: &str,
    defaults: &RawScene,
    limits: &Limits,
) -> Result<Vec<Scene>, SceneError> {
    let prompts = split_bulk(text, delimiter, limits)?;
    if prompts.is_empty() {
        return Err(SceneError::new(None, CoreError::NoScenes));
    }
    prompts
        .into_iter()
        .enumerate()
        .map(|(i, prompt)| {
            let raw = RawScene { prompt: Some(prompt), ..defaults.clone() };
            Scene::normalize(&raw, limits, Some(i + 1))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimiter_rong_thi_moi_dong_la_mot_canh() {
        let out = split_bulk("a\n\n b \r\nc\n", "", &Limits::default()).unwrap();
        assert_eq!(out, vec!["a", "b", "c"]);
    }

    #[test]
    fn co_delimiter_thi_giu_xuong_dong_ben_trong() {
        let out = split_bulk("a\nb\n---\nc", "---", &Limits::default()).unwrap();
        assert_eq!(out, vec!["a\nb", "c"]);
    }

    #[test]
    fn vuot_tran_thi_loi() {
        let l = Limits { batch_max_scenes: 2, ..Limits::default() };
        let e = split_bulk("a\nb\nc", "", &l).unwrap_err();
        assert_eq!(e.to_string(), "line 3: too many scenes (max 2)");
    }

    #[test]
    fn bulk_scenes_ap_mac_dinh_chung() {
        let defaults = RawScene { mode: Some("pro".into()), duration: Some("15".into()), ..RawScene::default() };
        let out = bulk_scenes("a\nb", "", &defaults, &Limits::default()).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].mode, crate::Mode::Pro);
        assert_eq!(out[1].duration, 15);
        assert_eq!(out[0].prompt, "a");
    }

    #[test]
    fn bulk_scenes_rong_thi_loi_no_scenes() {
        let e = bulk_scenes("   ", "", &RawScene::default(), &Limits::default()).unwrap_err();
        assert_eq!(e.to_string(), "no scenes (text is empty)");
    }
}
