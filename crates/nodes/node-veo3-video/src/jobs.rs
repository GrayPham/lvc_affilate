//! Dựng danh sách [`VideoJob`] (THUẦN, không I/O) — map ngầm giống
//! `node_dola_video::jobs::build_jobs`: mỗi `TextItem` trên cổng `prompt` là MỘT job/video,
//! `scene_index` 1..N liên tục. KHÁC `dola_video`: không có override TXT theo dòng (`attrs`) —
//! P5 chưa cần, xem `MEMORY.md`.

use std::path::PathBuf;

use lvc_node_api::{GraphError, Inputs, TextItem};
use lvc_veo3::job::{VideoJob, VideoSource};
use lvc_veo3::model::{Aspect, default_video_model};

use crate::VideoParams;

/// Số ảnh tham chiếu tối đa cho 1 job `r2v` — Google Flow (`MZZa6b`, xem spec ADR-R019) chỉ
/// nhận 1-6 ảnh. Không tìm thấy hằng số tương đương ở `dola_video` (Dola không có khái niệm
/// "reference-to-video" giống nhau) nên tự đặt riêng ở đây.
pub const MAX_REFERENCE_IMAGES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModeKind {
    T2v,
    R2v,
}

impl ModeKind {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "t2v" => Some(Self::T2v),
            "r2v" => Some(Self::R2v),
            _ => None,
        }
    }

    const fn default_model(self) -> &'static str {
        match self {
            Self::T2v => default_video_model::TEXT_TO_VIDEO,
            Self::R2v => default_video_model::REFERENCE_TO_VIDEO,
        }
    }
}

fn param_error(node: &str, field: &str, message: impl Into<String>) -> Vec<GraphError> {
    vec![GraphError::Param {
        node: node.to_string(),
        field: field.to_string(),
        message: message.into(),
    }]
}

fn parse_aspect(node: &str, raw: &str) -> Result<Aspect, Vec<GraphError>> {
    match raw.trim().to_lowercase().as_str() {
        "portrait" => Ok(Aspect::Portrait),
        "landscape" => Ok(Aspect::Landscape),
        _ => Err(param_error(
            node,
            "aspect",
            format!("aspect khong hop le: '{raw}' (chi nhan portrait|landscape)"),
        )),
    }
}

/// Mọi job của node: gộp `TextItem` từ cổng `prompt` (thứ tự cạnh), đánh `scene_index` 1..N liên
/// tục. Với `mode="r2v"`, MỌI job dùng CHUNG danh sách ảnh của cổng `refs` (tối đa
/// [`MAX_REFERENCE_IMAGES`]); với `mode="t2v"`, `refs` bị BỎ QUA hoàn toàn — KHÔNG lỗi dù có nối
/// ảnh vào (xem `MEMORY.md`, không có tiền lệ giống ở `dola_video` nên tự quyết theo hướng "im
/// lặng bỏ qua tham số không áp dụng cho mode hiện tại").
///
/// # Errors
/// `GraphError::Scene` (không có cảnh nào) / `GraphError::Param` (`mode`/`aspect` sai, `account`
/// rỗng, hoặc `refs` vượt [`MAX_REFERENCE_IMAGES`] khi `mode="r2v"`).
pub fn build_jobs(
    node_id: &str,
    p: &VideoParams,
    inputs: &Inputs,
) -> Result<Vec<VideoJob>, Vec<GraphError>> {
    let items: Vec<&TextItem> = inputs.texts("prompt").collect();
    if items.is_empty() {
        return Err(vec![GraphError::Scene {
            node: node_id.to_string(),
            line: None,
            message: "khong co canh nao".to_string(),
        }]);
    }

    let mode = ModeKind::parse(&p.mode).ok_or_else(|| {
        param_error(
            node_id,
            "mode",
            format!("mode khong hop le: '{}' (chi nhan t2v|r2v)", p.mode),
        )
    })?;
    let aspect = parse_aspect(node_id, &p.aspect)?;
    if p.account.trim().is_empty() {
        return Err(param_error(
            node_id,
            "account",
            "account (ten/label credential Google) khong duoc rong - P5 chua ho tro auto-chon",
        ));
    }

    // `refs` chỉ có ý nghĩa với r2v — t2v bỏ qua nên không cần đọc cổng khi không dùng tới.
    let ref_paths: Vec<PathBuf> = if mode == ModeKind::R2v {
        let paths: Vec<PathBuf> = inputs.assets("refs").map(|a| a.path.clone()).collect();
        if paths.len() > MAX_REFERENCE_IMAGES {
            return Err(param_error(
                node_id,
                "refs",
                format!(
                    "qua nhieu anh tham chieu ({} > {MAX_REFERENCE_IMAGES})",
                    paths.len()
                ),
            ));
        }
        paths
    } else {
        Vec::new()
    };

    let model = if p.model.trim().is_empty() {
        mode.default_model().to_string()
    } else {
        p.model.clone()
    };
    let seed_count = p.seed_count.max(1);

    let jobs = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let scene_index = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            let source = match mode {
                ModeKind::T2v => VideoSource::Text {
                    prompt: item.text.clone(),
                },
                ModeKind::R2v => VideoSource::Reference {
                    prompt: item.text.clone(),
                    ref_image_paths: ref_paths.clone(),
                },
            };
            VideoJob {
                scene_index,
                credential_id: p.account.clone(),
                source,
                model: model.clone(),
                aspect,
                seed_count,
            }
        })
        .collect();
    Ok(jobs)
}
