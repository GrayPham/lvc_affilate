use serde::{Deserialize, Serialize};

use crate::{CoreError, Limits, LogoMode, Mode, Model, Ratio, SceneError};

pub const DURATIONS: [u16; 3] = [10, 15, 30];
pub const DEFAULT_DURATION: u16 = 10;

/// Cảnh thô: đúng hình dạng dict của bản Python, mọi trường tuỳ chọn.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawScene {
    pub prompt: Option<String>,
    pub ratio: Option<String>,
    pub duration: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub logo: Option<String>,
    pub reference_images: Option<Vec<String>>,
    pub account: Option<String>,
}

/// Cảnh đã chuẩn hoá và kiểm hợp lệ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scene {
    pub prompt: String,
    pub ratio: Ratio,
    pub duration: u16,
    pub model: Model,
    pub mode: Mode,
    pub logo: LogoMode,
    pub reference_images: Vec<String>,
    /// `""` = để scheduler tự chọn.
    pub account: String,
}

fn valid_account(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl Scene {
    /// # Errors
    /// `SceneError` kèm số dòng khi bất kỳ trường nào không hợp lệ.
    pub fn normalize(
        raw: &RawScene,
        limits: &Limits,
        line: Option<usize>,
    ) -> Result<Self, SceneError> {
        let err = |k: CoreError| SceneError::new(line, k);

        let prompt = raw.prompt.as_deref().unwrap_or("").trim().to_string();
        if prompt.is_empty() {
            return Err(err(CoreError::EmptyPrompt));
        }
        if limits.prompt_max_chars > 0 && prompt.chars().count() > limits.prompt_max_chars {
            return Err(err(CoreError::PromptTooLong {
                actual: prompt.chars().count(),
                limit: limits.prompt_max_chars,
            }));
        }

        let ratio_raw = raw.ratio.as_deref().unwrap_or("").trim();
        let ratio = if ratio_raw.is_empty() {
            Ratio::default()
        } else {
            Ratio::parse(ratio_raw).map_err(err)?
        };

        let dur_raw = raw.duration.as_deref().unwrap_or("").trim();
        let dur_trimmed = dur_raw.trim_end_matches(['s', 'S']);
        let duration = if dur_trimmed.is_empty() {
            DEFAULT_DURATION
        } else {
            dur_trimmed
                .parse::<u16>()
                .map_err(|_| err(CoreError::InvalidDuration(dur_raw.to_string())))?
        };
        if !DURATIONS.contains(&duration) {
            return Err(err(CoreError::InvalidDuration(dur_raw.to_string())));
        }

        let model_raw = raw.model.as_deref().unwrap_or("").trim();
        let model = if model_raw.is_empty() {
            Model::default()
        } else {
            Model::parse(model_raw).map_err(err)?
        };

        let mode = Mode::parse(raw.mode.as_deref().unwrap_or("")).map_err(err)?;
        let logo = LogoMode::parse(raw.logo.as_deref().unwrap_or("")).map_err(err)?;

        let reference_images: Vec<String> = raw
            .reference_images
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();
        if reference_images.len() > limits.reference_image_max_count {
            return Err(err(CoreError::TooManyReferenceImages(
                limits.reference_image_max_count,
            )));
        }

        let account = raw.account.as_deref().unwrap_or("").trim().to_string();
        if !account.is_empty() && !valid_account(&account) {
            return Err(err(CoreError::InvalidAccount(account)));
        }

        Ok(Self {
            prompt,
            ratio,
            duration,
            model,
            mode,
            logo,
            reference_images,
            account,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(prompt: &str) -> RawScene {
        RawScene {
            prompt: Some(prompt.into()),
            ..RawScene::default()
        }
    }

    #[test]
    fn dien_mac_dinh_dung() {
        let s = Scene::normalize(&raw("con meo"), &Limits::default(), None).unwrap();
        assert_eq!(s.prompt, "con meo");
        assert_eq!(s.ratio, Ratio::R9x16);
        assert_eq!(s.duration, 10);
        assert_eq!(s.model, Model::Seedance20);
        assert_eq!(s.mode, Mode::Fast);
        assert_eq!(s.logo, LogoMode::Off);
        assert!(s.reference_images.is_empty());
        assert_eq!(s.account, "");
    }

    #[test]
    fn prompt_rong_thi_loi() {
        let e = Scene::normalize(&raw("   "), &Limits::default(), Some(2)).unwrap_err();
        assert_eq!(e.to_string(), "line 2: prompt is empty");
    }

    #[test]
    fn prompt_qua_dai_chi_khi_limit_duong() {
        let l = Limits {
            prompt_max_chars: 5,
            ..Limits::default()
        };
        let e = Scene::normalize(&raw("abcdefgh"), &l, None).unwrap_err();
        assert_eq!(e.to_string(), "prompt too long (8 > 5 chars)");
        // 0 = khong gioi han
        assert!(Scene::normalize(&raw("abcdefgh"), &Limits::default(), None).is_ok());
    }

    #[test]
    fn thoi_luong_chap_nhan_hau_to_s() {
        let mut r = raw("x");
        r.duration = Some("30s".into());
        assert_eq!(
            Scene::normalize(&r, &Limits::default(), None)
                .unwrap()
                .duration,
            30
        );
        r.duration = Some("15S".into());
        assert_eq!(
            Scene::normalize(&r, &Limits::default(), None)
                .unwrap()
                .duration,
            15
        );
        r.duration = Some(String::new());
        assert_eq!(
            Scene::normalize(&r, &Limits::default(), None)
                .unwrap()
                .duration,
            10
        );
    }

    #[test]
    fn thoi_luong_la_thi_loi() {
        let mut r = raw("x");
        r.duration = Some("7".into());
        let e = Scene::normalize(&r, &Limits::default(), None).unwrap_err();
        assert_eq!(e.to_string(), "dur must be 10, 15 or 30 (got '7')");
        r.duration = Some("abc".into());
        let e = Scene::normalize(&r, &Limits::default(), None).unwrap_err();
        assert_eq!(e.to_string(), "dur must be 10, 15 or 30 (got 'abc')");
    }

    #[test]
    fn refs_bo_phan_tu_rong_va_kep_so_luong() {
        let mut r = raw("x");
        r.reference_images = Some(vec!["a.jpg".into(), "  ".into(), " b.png ".into()]);
        let s = Scene::normalize(&r, &Limits::default(), None).unwrap();
        assert_eq!(s.reference_images, vec!["a.jpg", "b.png"]);

        let l = Limits {
            reference_image_max_count: 1,
            ..Limits::default()
        };
        let e = Scene::normalize(&r, &l, None).unwrap_err();
        assert_eq!(e.to_string(), "too many reference images (max 1)");
    }

    #[test]
    fn ten_tai_khoan_theo_regex() {
        let mut r = raw("x");
        r.account = Some("acc_2-b".into());
        assert_eq!(
            Scene::normalize(&r, &Limits::default(), None)
                .unwrap()
                .account,
            "acc_2-b"
        );
        r.account = Some("bad name".into());
        let e = Scene::normalize(&r, &Limits::default(), None).unwrap_err();
        assert_eq!(
            e.to_string(),
            "acc must be an account name (letters, digits, _ -; got 'bad name')"
        );
        r.account = Some("x".repeat(33));
        assert!(Scene::normalize(&r, &Limits::default(), None).is_err());
    }

    #[test]
    fn do_dai_prompt_dem_theo_ky_tu_khong_phai_byte() {
        // "猫が歩く" = 4 ký tự nhưng 12 byte. Nếu đếm theo byte, mọi prompt
        // tiếng Việt/tiếng Nhật sẽ bị từ chối oan.
        let l = Limits {
            prompt_max_chars: 4,
            ..Limits::default()
        };
        assert!(Scene::normalize(&raw("猫が歩く"), &l, None).is_ok());
        // 5 ký tự thì vượt thật
        assert!(Scene::normalize(&raw("猫が歩く猫"), &l, None).is_err());
    }
}
