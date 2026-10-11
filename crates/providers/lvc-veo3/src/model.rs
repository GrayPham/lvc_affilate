//! Model + ty le khung anh (aspect). Port tu `image_model_registry.py` va `config.py`
//! (`VEO3_MODELS`/`ASPECT_RATIO_CONFIG`). **Enum aspect ANH va VIDEO khac nhau** (xem
//! [`Aspect::image_enum`]/[`Aspect::video_enum`]) — day la bay de nham nhat khi port, dung gop
//! chung 1 so cho ca hai.

/// Ty le khung — dung chung 1 enum Rust, nhung DOI ra so khac nhau theo rpc anh/video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    Portrait,
    Landscape,
}

impl Aspect {
    /// So dung trong rpc anh (`ogiZ0b`): doc = 2, ngang = 3.
    #[must_use]
    pub const fn image_enum(self) -> i64 {
        match self {
            Self::Portrait => 2,
            Self::Landscape => 3,
        }
    }

    /// So dung trong rpc video (`YhhmEf`/`MZZa6b`/`eb1hJf`/`nprQif`/`p0UkFb`): doc = 1, ngang = 2.
    #[must_use]
    pub const fn video_enum(self) -> i64 {
        match self {
            Self::Portrait => 1,
            Self::Landscape => 2,
        }
    }
}

/// Model tao anh (Google Labs Flow / Nano Banana). Ca 3 deu ho tro anh tham chieu, ca 3 deu chi
/// co 2 aspect (doc/ngang, khong co vuong).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageModel {
    /// "Banana 2" — model nhe.
    Narwhal,
    /// "Banana Pro" — mac dinh, chat luong cao nhat.
    #[default]
    GemPix2,
    /// "Lite" — nhanh, re nhat.
    HarborSeal,
}

impl ImageModel {
    /// Ten model dung trong payload (`imageModelName`/spec\[5\]).
    #[must_use]
    pub const fn api_name(self) -> &'static str {
        match self {
            Self::Narwhal => "NARWHAL",
            Self::GemPix2 => "GEM_PIX_2",
            Self::HarborSeal => "HARBOR_SEAL",
        }
    }

    /// Doc lai tu ten model API (khong phan biet hoa/thuong) — `None` neu la ten la.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_uppercase().as_str() {
            "NARWHAL" => Some(Self::Narwhal),
            "GEM_PIX_2" | "GEM_PIX" | "BANANA_PRO" => Some(Self::GemPix2),
            "HARBOR_SEAL" => Some(Self::HarborSeal),
            _ => None,
        }
    }
}

/// Kieu tao video — quyet dinh rpcid + hinh dang spec (xem `crates/providers/lvc-veo3/src/rpc/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoMode {
    /// Text-to-video (`YhhmEf`).
    TextToVideo,
    /// 1 anh dau -> video (`eb1hJf`). **Chua co cURL thuc xac nhan** — xem MEMORY.md.
    ImageToVideo,
    /// 1-6 anh tham chieu -> video (`MZZa6b`).
    ReferenceToVideo,
    /// Anh dau + anh cuoi -> video noi (`nprQif`). **Chua co cURL thuc xac nhan**.
    Interpolation,
}

/// Model mac dinh 0-credit theo tung mode — DU DUNG de node chay duoc ngay, KHONG phai bang day
/// du `DURATION_MODELS` cua Python (hang tram bien the theo aspect/duration/hang tai khoan). Nguoi
/// dung tu chon model khac qua tham so `model: String` tu do; xem MEMORY.md ly do khong port het.
pub mod default_video_model {
    pub const TEXT_TO_VIDEO: &str = "veo_3_1_t2v_lite_low_priority";
    pub const IMAGE_TO_VIDEO: &str = "abra_i2v_8s";
    pub const REFERENCE_TO_VIDEO: &str = "abra_r2v_8s";
    pub const INTERPOLATION: &str = "veo_3_1_interpolation_lite_low_priority";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_enum_anh_va_video_khac_nhau() {
        assert_eq!(Aspect::Portrait.image_enum(), 2);
        assert_eq!(Aspect::Landscape.image_enum(), 3);
        assert_eq!(Aspect::Portrait.video_enum(), 1);
        assert_eq!(Aspect::Landscape.video_enum(), 2);
    }

    #[test]
    fn image_model_api_name_va_parse_khop_nguoc() {
        for model in [
            ImageModel::Narwhal,
            ImageModel::GemPix2,
            ImageModel::HarborSeal,
        ] {
            assert_eq!(ImageModel::parse(model.api_name()), Some(model));
        }
    }

    #[test]
    fn image_model_parse_ten_cu_deprecated() {
        assert_eq!(ImageModel::parse("gem_pix"), Some(ImageModel::GemPix2));
        assert_eq!(ImageModel::parse("banana_pro"), Some(ImageModel::GemPix2));
        assert_eq!(ImageModel::parse("khong-ton-tai"), None);
    }

    #[test]
    fn image_model_mac_dinh_la_gem_pix_2() {
        assert_eq!(ImageModel::default(), ImageModel::GemPix2);
    }
}
