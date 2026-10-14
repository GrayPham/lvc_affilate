//! Dựng danh sách [`VideoJob`] (THUẦN, không I/O) — map ngầm giống
//! `node_dola_video::jobs::build_jobs`: mỗi `TextItem` trên cổng `prompt` là MỘT job/video,
//! `scene_index` 1..N liên tục. Override TXT theo dòng (`attrs`, khoá trùng `node_prompt::attrs`
//! — không import để giữ node độc lập, xem `MEMORY.md`): `ratio` (chuyển sang `aspect`, giá trị
//! không nhận diện được thì IM LẶNG bỏ qua — khác lỗi cứng của `aspect` tĩnh ở tham số node),
//! `refs` (gộp thêm vào cổng `refs`, chỉ áp dụng khi `mode="r2v"`), `account` (ghi đè KHI dòng có
//! giá trị khác rỗng — tham số node vẫn là giá trị nền cho các dòng không override). KHÔNG đọc
//! `duration`/`model`/`mode` theo dòng — không có field tương đương ý nghĩa ở Veo3 (`duration`),
//! hoặc dùng chung cho cả node không hợp lý đổi theo từng dòng (`model`/`mode` vẫn tĩnh).

use std::collections::HashSet;
use std::path::PathBuf;

use lvc_node_api::{GraphError, Inputs, TextItem};
use lvc_veo3::job::{VideoJob, VideoSource};
use lvc_veo3::model::{Aspect, default_video_model};

use crate::VideoParams;

/// Số ảnh tham chiếu tối đa cho 1 job `r2v` — Google Flow (`MZZa6b`, xem spec ADR-R019) chỉ
/// nhận 1-6 ảnh. Không tìm thấy hằng số tương đương ở `dola_video` (Dola không có khái niệm
/// "reference-to-video" giống nhau) nên tự đặt riêng ở đây.
pub const MAX_REFERENCE_IMAGES: usize = 6;

/// Khoá `TextItem.attrs` dùng cho override theo dòng — trùng với `node_prompt::attrs` (không
/// import để giữ node độc lập, giống `node_dola_video::jobs`).
const RATIO: &str = "ratio";
const REFS: &str = "refs";
const ACCOUNT: &str = "account";

/// `"16:9"`/`"9:16"` (cú pháp `ratio=` của Dola/`node-prompt`) → [`Aspect`] của Veo3. Giá trị
/// không nhận diện được (vd `"1:1"` — Veo3 không hỗ trợ vuông) → `None`, gọi nơi dùng tự fallback
/// về `aspect` tĩnh của node, KHÔNG lỗi (khác validate `aspect` tĩnh ở tham số node).
fn aspect_from_ratio_attr(raw: &str) -> Option<Aspect> {
    match raw.trim() {
        "16:9" => Some(Aspect::Landscape),
        "9:16" => Some(Aspect::Portrait),
        _ => None,
    }
}

/// Gộp ảnh tham chiếu: cổng `refs` TRƯỚC, `refs=` trong TXT SAU; bỏ trùng/rỗng, giữ lần xuất hiện
/// ĐẦU TIÊN — giống hệt `node_dola_video::jobs::merge_refs` (không tái dùng chung, xem doc module).
fn merge_refs(port_refs: &[String], txt_refs: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for r in port_refs.iter().chain(txt_refs.iter()) {
        let r = r.trim();
        if r.is_empty() {
            continue;
        }
        if seen.insert(r.to_string()) {
            out.push(r.to_string());
        }
    }
    out
}

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

fn param_error_single(node: &str, field: &str, message: impl Into<String>) -> GraphError {
    GraphError::Param {
        node: node.to_string(),
        field: field.to_string(),
        message: message.into(),
    }
}

fn param_error(node: &str, field: &str, message: impl Into<String>) -> Vec<GraphError> {
    vec![param_error_single(node, field, message)]
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
/// `GraphError::Scene` (không có cảnh nào) / `GraphError::Param` (`mode`/`aspect` sai, hoặc `refs`
/// vượt [`MAX_REFERENCE_IMAGES`] khi `mode="r2v"`). `account` rỗng KHÔNG lỗi — truyền thẳng xuống
/// `VeoAccountResolver` để tự chọn tài khoản (2026-10-13, xem `CLAUDE.md`).
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

    // `refs` chỉ có ý nghĩa với r2v — t2v bỏ qua nên không cần đọc cổng khi không dùng tới.
    let port_refs: Vec<String> = if mode == ModeKind::R2v {
        inputs
            .assets("refs")
            .map(|a| a.path.to_string_lossy().into_owned())
            .collect()
    } else {
        Vec::new()
    };

    let model = if p.model.trim().is_empty() {
        mode.default_model().to_string()
    } else {
        p.model.clone()
    };
    let seed_count = p.seed_count.max(1);

    let mut errors = Vec::new();
    let mut jobs = Vec::with_capacity(items.len());
    for (idx, item) in items.into_iter().enumerate() {
        let scene_index = u32::try_from(idx + 1).unwrap_or(u32::MAX);

        let item_aspect = item
            .attrs
            .get(RATIO)
            .and_then(|r| aspect_from_ratio_attr(r))
            .unwrap_or(aspect);

        let item_account = item
            .attrs
            .get(ACCOUNT)
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
            .map_or_else(|| p.account.clone(), str::to_string);

        let source = if mode == ModeKind::R2v {
            let txt_refs: Vec<String> = item
                .attrs
                .get(REFS)
                .map(|r| r.split('\n').map(str::to_string).collect())
                .unwrap_or_default();
            let merged = merge_refs(&port_refs, &txt_refs);
            if merged.len() > MAX_REFERENCE_IMAGES {
                errors.push(param_error_single(
                    node_id,
                    "refs",
                    format!(
                        "qua nhieu anh tham chieu ({} > {MAX_REFERENCE_IMAGES}) o canh {scene_index}",
                        merged.len()
                    ),
                ));
                continue;
            }
            VideoSource::Reference {
                prompt: item.text.clone(),
                ref_image_paths: merged.into_iter().map(PathBuf::from).collect(),
            }
        } else {
            VideoSource::Text {
                prompt: item.text.clone(),
            }
        };

        jobs.push(VideoJob {
            scene_index,
            credential_id: item_account,
            source,
            model: model.clone(),
            aspect: item_aspect,
            seed_count,
        });
    }
    if errors.is_empty() {
        Ok(jobs)
    } else {
        Err(errors)
    }
}
