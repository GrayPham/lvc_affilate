//! Loại node và cổng vào/ra — spec §4.1 (kiểu cổng `Text`/`Image`/`Video`) và §4.2
//! (bảng node). `PortSpec` tĩnh (`'static`), không phụ thuộc instance — mỗi `NodeKind`
//! có đúng một bộ cổng cố định, tra bằng `inputs()`/`outputs()`.

use serde::{Deserialize, Serialize};

use crate::{LogoMode, Mode, Model, Ratio};

/// Kiểu dữ liệu chạy qua một cổng — spec §4.1: `Text` · `Image` · `Video`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    Text,
    Image,
    Video,
}

/// Mô tả tĩnh một cổng (vào hoặc ra) của một `NodeKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortSpec {
    pub name: &'static str,
    pub ty: PortType,
    /// Cho phép nhiều cạnh nối tới cổng này (cổng vào); nếu `false`, cạnh thứ 2 trở đi
    /// là lỗi `GraphError::MultipleInputsOnSinglePort` (spec: "many=false cổng chỉ nhận 1 cạnh").
    pub many: bool,
    /// Chỉ áp dụng cho cổng vào: phải có ít nhất 1 cạnh nối tới, thiếu thì
    /// `GraphError::MissingRequiredInput`.
    pub required: bool,
    /// Cổng rào chắn (barrier, spec §4.3): chờ toàn bộ nhánh thượng nguồn hoàn tất mới chạy.
    /// Chỉ mang tính mô tả ở Task 1 (Task 2 — kế hoạch chạy — mới dùng để lập lịch).
    pub collect: bool,
}

/// Loại node trong đồ thị — spec §4.2. `#[serde(tag = "kind")]` để UI (React Flow) đọc
/// thẳng field `kind` mà không cần biết layout enum của Rust.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// Nhả `Text[]` (mỗi dòng 1 cảnh) — port `scenes::split_bulk`.
    Prompt { text: String },
    /// Nhả `Image[]` — port `media::save_uploaded_image`.
    ImageUpload { paths: Vec<String> },
    /// `Text` (URL) → `Image` — port `download_one_image` (giữ chống SSRF). P4, chưa hỗ
    /// trợ trong P3 (`supported_in_p3` = false).
    ImageFromUrl { url: String },
    /// `Text` + `Image?` → `Image` — hai backend (ADR-R009). P4.
    ImageGen { prompt: String, backend: String },
    /// `Text[]` + `Image[]?` → `Video[]` — port `generate_video`, tách đôi submit/poll
    /// (spec §4.4). Cấu hình riêng của node (không qua cạnh): chế độ/model/tỉ lệ/thời
    /// lượng/logo/tài khoản.
    Video {
        mode: Mode,
        model: Model,
        ratio: Ratio,
        duration: u16,
        logo: LogoMode,
        account: String,
    },
    /// `Video` → `Video` — port `postprocess::remove_logo`. P4.
    RemoveLogo,
    /// `Video` → `Video` — port `ensure_web_codec`. P4.
    EnsureH264,
    /// `Video[]` (cổng vào rào chắn) → `Video` — node ghép phân cảnh, mới (spec §4.3). P4.
    Concat,
    /// `Video[]` | `Image[]` → – (không có cổng ra) — port chính sách `_task_out_dir`.
    Save { out_dir: String },
}

const EMPTY: [PortSpec; 0] = [];

const PROMPT_OUT: [PortSpec; 1] = [PortSpec {
    name: "out",
    ty: PortType::Text,
    many: true,
    required: false,
    collect: false,
}];

const IMAGE_UPLOAD_OUT: [PortSpec; 1] = [PortSpec {
    name: "out",
    ty: PortType::Image,
    many: true,
    required: false,
    collect: false,
}];

const IMAGE_FROM_URL_IN: [PortSpec; 1] = [PortSpec {
    name: "url",
    ty: PortType::Text,
    many: false,
    required: false,
    collect: false,
}];
const IMAGE_FROM_URL_OUT: [PortSpec; 1] = [PortSpec {
    name: "out",
    ty: PortType::Image,
    many: false,
    required: false,
    collect: false,
}];

const IMAGE_GEN_IN: [PortSpec; 2] = [
    PortSpec {
        name: "prompt",
        ty: PortType::Text,
        many: false,
        required: true,
        collect: false,
    },
    PortSpec {
        name: "ref",
        ty: PortType::Image,
        many: false,
        required: false,
        collect: false,
    },
];
const IMAGE_GEN_OUT: [PortSpec; 1] = [PortSpec {
    name: "out",
    ty: PortType::Image,
    many: false,
    required: false,
    collect: false,
}];

const VIDEO_IN: [PortSpec; 2] = [
    PortSpec {
        name: "prompt",
        ty: PortType::Text,
        many: true,
        required: true,
        collect: false,
    },
    PortSpec {
        name: "refs",
        ty: PortType::Image,
        many: true,
        required: false,
        collect: false,
    },
];
const VIDEO_OUT: [PortSpec; 1] = [PortSpec {
    name: "out",
    ty: PortType::Video,
    many: true,
    required: false,
    collect: false,
}];

// RemoveLogo / EnsureH264: 1 video vào, 1 video ra.
const VIDEO_POST_IN: [PortSpec; 1] = [PortSpec {
    name: "in",
    ty: PortType::Video,
    many: false,
    required: true,
    collect: false,
}];
const VIDEO_POST_OUT: [PortSpec; 1] = [PortSpec {
    name: "out",
    ty: PortType::Video,
    many: false,
    required: false,
    collect: false,
}];

const CONCAT_IN: [PortSpec; 1] = [PortSpec {
    name: "in",
    ty: PortType::Video,
    many: true,
    required: true,
    collect: true,
}];
const CONCAT_OUT: [PortSpec; 1] = [PortSpec {
    name: "out",
    ty: PortType::Video,
    many: false,
    required: false,
    collect: false,
}];

// Save: nhận cả hai kiểu qua 2 cổng riêng (`video`, `image`); mỗi cổng tự nó optional,
// nhưng validate() đòi ít nhất 1 trong 2 phải có cạnh nối (xem GraphError::MissingRequiredInput
// port "video|image" trong validate.rs).
const SAVE_IN: [PortSpec; 2] = [
    PortSpec {
        name: "video",
        ty: PortType::Video,
        many: true,
        required: false,
        collect: false,
    },
    PortSpec {
        name: "image",
        ty: PortType::Image,
        many: true,
        required: false,
        collect: false,
    },
];

impl NodeKind {
    /// Tên "kind" trong JSON (khớp `#[serde(tag = "kind", rename_all = "snake_case")]`)
    /// — dùng nội bộ cho thông điệp lỗi (`GraphError::Unsupported`).
    pub(crate) fn tag(&self) -> &'static str {
        match self {
            Self::Prompt { .. } => "prompt",
            Self::ImageUpload { .. } => "image_upload",
            Self::ImageFromUrl { .. } => "image_from_url",
            Self::ImageGen { .. } => "image_gen",
            Self::Video { .. } => "video",
            Self::RemoveLogo => "remove_logo",
            Self::EnsureH264 => "ensure_h264",
            Self::Concat => "concat",
            Self::Save { .. } => "save",
        }
    }

    /// Danh sách cổng vào tĩnh — spec §4.2.
    #[must_use]
    pub fn inputs(&self) -> &'static [PortSpec] {
        match self {
            Self::Prompt { .. } | Self::ImageUpload { .. } => &EMPTY,
            Self::ImageFromUrl { .. } => &IMAGE_FROM_URL_IN,
            Self::ImageGen { .. } => &IMAGE_GEN_IN,
            Self::Video { .. } => &VIDEO_IN,
            Self::RemoveLogo | Self::EnsureH264 => &VIDEO_POST_IN,
            Self::Concat => &CONCAT_IN,
            Self::Save { .. } => &SAVE_IN,
        }
    }

    /// Danh sách cổng ra tĩnh — spec §4.2.
    #[must_use]
    pub fn outputs(&self) -> &'static [PortSpec] {
        match self {
            Self::Prompt { .. } => &PROMPT_OUT,
            Self::ImageUpload { .. } => &IMAGE_UPLOAD_OUT,
            Self::ImageFromUrl { .. } => &IMAGE_FROM_URL_OUT,
            Self::ImageGen { .. } => &IMAGE_GEN_OUT,
            Self::Video { .. } => &VIDEO_OUT,
            Self::RemoveLogo | Self::EnsureH264 => &VIDEO_POST_OUT,
            Self::Concat => &CONCAT_OUT,
            Self::Save { .. } => &EMPTY,
        }
    }

    /// P3 chỉ dựng 4 loại node (Prompt/Tải ảnh lên/Tạo video/Lưu); phần còn lại (ảnh từ
    /// URL, tạo ảnh, hậu kỳ video, ghép phân cảnh) dời sang P4.
    #[must_use]
    pub fn supported_in_p3(&self) -> bool {
        matches!(
            self,
            Self::Prompt { .. } | Self::ImageUpload { .. } | Self::Video { .. } | Self::Save { .. }
        )
    }

    /// Nhãn tiếng Việt hiển thị trên node (React Flow) — spec §4.2 cột "Node".
    #[must_use]
    pub fn label_vi(&self) -> &'static str {
        match self {
            Self::Prompt { .. } => "Prompt",
            Self::ImageUpload { .. } => "Tải ảnh lên",
            Self::ImageFromUrl { .. } => "Lấy ảnh từ URL",
            Self::ImageGen { .. } => "Tạo ảnh",
            Self::Video { .. } => "Tạo video",
            Self::RemoveLogo => "Xoá logo",
            Self::EnsureH264 => "Chuẩn H.264",
            Self::Concat => "Ghép phân cảnh",
            Self::Save { .. } => "Lưu",
        }
    }
}

/// Một node trong đồ thị: định danh, loại + dữ liệu riêng, toạ độ vẽ (React Flow).
/// Thứ tự trường khớp định dạng file workflow JSON: `id, kind, x, y`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub x: f32,
    pub y: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn don_vi_khong_du_lieu_serde_chi_co_kind() {
        assert_eq!(
            serde_json::to_string(&NodeKind::RemoveLogo).unwrap(),
            r#"{"kind":"remove_logo"}"#
        );
        assert_eq!(
            serde_json::to_string(&NodeKind::EnsureH264).unwrap(),
            r#"{"kind":"ensure_h264"}"#
        );
        assert_eq!(
            serde_json::to_string(&NodeKind::Concat).unwrap(),
            r#"{"kind":"concat"}"#
        );
    }

    #[test]
    fn tag_khop_serde_kind_cho_tung_loai() {
        // Chống lệch giữa NodeKind::tag() (dùng trong thông điệp lỗi) và serde tag thật.
        let samples = [
            NodeKind::Prompt {
                text: String::new(),
            },
            NodeKind::ImageUpload { paths: vec![] },
            NodeKind::ImageFromUrl { url: String::new() },
            NodeKind::ImageGen {
                prompt: String::new(),
                backend: String::new(),
            },
            NodeKind::Video {
                mode: Mode::Fast,
                model: Model::Seedance20,
                ratio: Ratio::R9x16,
                duration: 10,
                logo: LogoMode::Off,
                account: String::new(),
            },
            NodeKind::RemoveLogo,
            NodeKind::EnsureH264,
            NodeKind::Concat,
            NodeKind::Save {
                out_dir: String::new(),
            },
        ];
        for kind in samples {
            let v: serde_json::Value = serde_json::to_value(&kind).unwrap();
            assert_eq!(
                v.get("kind").and_then(serde_json::Value::as_str),
                Some(kind.tag())
            );
        }
    }

    #[test]
    fn supported_in_p3_chi_4_loai() {
        assert!(
            NodeKind::Prompt {
                text: String::new()
            }
            .supported_in_p3()
        );
        assert!(NodeKind::ImageUpload { paths: vec![] }.supported_in_p3());
        assert!(
            NodeKind::Video {
                mode: Mode::Fast,
                model: Model::Seedance20,
                ratio: Ratio::R9x16,
                duration: 10,
                logo: LogoMode::Off,
                account: String::new(),
            }
            .supported_in_p3()
        );
        assert!(
            NodeKind::Save {
                out_dir: String::new()
            }
            .supported_in_p3()
        );
        assert!(!NodeKind::RemoveLogo.supported_in_p3());
        assert!(!NodeKind::EnsureH264.supported_in_p3());
        assert!(!NodeKind::Concat.supported_in_p3());
        assert!(!NodeKind::ImageFromUrl { url: String::new() }.supported_in_p3());
        assert!(
            !NodeKind::ImageGen {
                prompt: String::new(),
                backend: String::new(),
            }
            .supported_in_p3()
        );
    }

    #[test]
    fn label_vi_tieng_viet_dung_bang_spec() {
        assert_eq!(
            NodeKind::Prompt {
                text: String::new()
            }
            .label_vi(),
            "Prompt"
        );
        assert_eq!(
            NodeKind::ImageUpload { paths: vec![] }.label_vi(),
            "Tải ảnh lên"
        );
        assert_eq!(
            NodeKind::ImageFromUrl { url: String::new() }.label_vi(),
            "Lấy ảnh từ URL"
        );
        assert_eq!(
            NodeKind::ImageGen {
                prompt: String::new(),
                backend: String::new(),
            }
            .label_vi(),
            "Tạo ảnh"
        );
        assert_eq!(
            NodeKind::Video {
                mode: Mode::Fast,
                model: Model::Seedance20,
                ratio: Ratio::R9x16,
                duration: 10,
                logo: LogoMode::Off,
                account: String::new(),
            }
            .label_vi(),
            "Tạo video"
        );
        assert_eq!(NodeKind::RemoveLogo.label_vi(), "Xoá logo");
        assert_eq!(NodeKind::EnsureH264.label_vi(), "Chuẩn H.264");
        assert_eq!(NodeKind::Concat.label_vi(), "Ghép phân cảnh");
        assert_eq!(
            NodeKind::Save {
                out_dir: String::new()
            }
            .label_vi(),
            "Lưu"
        );
    }

    #[test]
    fn video_co_2_cong_vao_prompt_bat_buoc_refs_tuy_chon() {
        let kind = NodeKind::Video {
            mode: Mode::Fast,
            model: Model::Seedance20,
            ratio: Ratio::R9x16,
            duration: 10,
            logo: LogoMode::Off,
            account: String::new(),
        };
        let ins = kind.inputs();
        assert_eq!(ins.len(), 2);
        assert_eq!(ins[0].name, "prompt");
        assert!(ins[0].required && ins[0].many && ins[0].ty == PortType::Text);
        assert_eq!(ins[1].name, "refs");
        assert!(!ins[1].required && ins[1].many && ins[1].ty == PortType::Image);
        assert_eq!(kind.outputs()[0].ty, PortType::Video);
    }

    #[test]
    fn save_khong_co_cong_ra_va_2_cong_vao_tuy_chon() {
        let kind = NodeKind::Save {
            out_dir: String::new(),
        };
        assert!(kind.outputs().is_empty());
        let ins = kind.inputs();
        assert_eq!(ins.len(), 2);
        assert!(ins.iter().all(|p| !p.required && p.many));
    }

    #[test]
    fn concat_cong_vao_la_barrier_collect() {
        assert!(NodeKind::Concat.inputs()[0].collect);
    }
}
