//! Dựng danh sách [`ImageJob`] (THUẦN, không I/O) — map ngầm giống
//! `node_dola_video::jobs::build_jobs`/`node_veo3_video::jobs::build_jobs`: mỗi `TextItem` trên
//! cổng `prompt` là MỘT job/ảnh, `scene_index` 1..N liên tục. KHÁC `dola_video`: không có override
//! TXT theo dòng (`attrs`) — bảng thiết kế ADR-R019 chỉ liệt `model/aspect/seed/account` là tham số
//! NODE, không có khoá TXT tương ứng cho ảnh (xem `MEMORY.md`).

use std::path::PathBuf;

use lvc_node_api::{GraphError, Inputs, TextItem};
use lvc_veo3::job::ImageJob;
use lvc_veo3::model::{Aspect, ImageModel};

use crate::ImageParams;

fn param_error(node: &str, field: &str, message: impl Into<String>) -> Vec<GraphError> {
    vec![GraphError::Param {
        node: node.to_string(),
        field: field.to_string(),
        message: message.into(),
    }]
}

fn parse_model(node: &str, raw: &str) -> Result<ImageModel, Vec<GraphError>> {
    ImageModel::parse(raw).ok_or_else(|| {
        param_error(
            node,
            "model",
            format!("model khong hop le: '{raw}' (chi nhan NARWHAL|GEM_PIX_2|HARBOR_SEAL)"),
        )
    })
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
/// tục. Ảnh cổng `refs` dùng CHUNG cho MỌI job — KHÔNG có trần 1-6 ảnh như R2V video
/// (`node_veo3_video::jobs::MAX_REFERENCE_IMAGES`); Google không công bố trần riêng cho `ogiZ0b`
/// (xem `MEMORY.md`), để server tự chặn nếu vượt quá.
///
/// # Errors
/// `GraphError::Scene` (không có ảnh nào) / `GraphError::Param` (`model`/`aspect` sai). `account`
/// rỗng KHÔNG lỗi — truyền thẳng xuống `VeoAccountResolver` để tự chọn tài khoản (2026-10-13).
pub fn build_jobs(
    node_id: &str,
    p: &ImageParams,
    inputs: &Inputs,
) -> Result<Vec<ImageJob>, Vec<GraphError>> {
    let items: Vec<&TextItem> = inputs.texts("prompt").collect();
    if items.is_empty() {
        return Err(vec![GraphError::Scene {
            node: node_id.to_string(),
            line: None,
            message: "khong co anh nao".to_string(),
        }]);
    }

    let model = parse_model(node_id, &p.model)?;
    let aspect = parse_aspect(node_id, &p.aspect)?;

    let refs: Vec<PathBuf> = inputs.assets("refs").map(|a| a.path.clone()).collect();
    let seed = p.seed.unwrap_or(0);

    let jobs = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| ImageJob {
            scene_index: u32::try_from(idx + 1).unwrap_or(u32::MAX),
            credential_id: p.account.clone(),
            prompt: item.text.clone(),
            model,
            aspect,
            seed,
            ref_image_paths: refs.clone(),
        })
        .collect();
    Ok(jobs)
}
