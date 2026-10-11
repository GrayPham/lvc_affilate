//! Dựng danh sách [`ImageJob`] (THUẦN, không I/O) — map ngầm giống `node_veo3_image`: mỗi
//! `TextItem` trên cổng `prompt` là MỘT job/ảnh, `scene_index` 1..N liên tục. Google official
//! (Imagen `:predict`) KHÔNG nhận ảnh tham chiếu ở bước predict nên node này KHÔNG có cổng `refs`.

use lvc_google::job::ImageJob;
use lvc_google::req::Aspect;
use lvc_node_api::{GraphError, Inputs, TextItem};

use crate::ImageParams;

fn param_error(node: &str, field: &str, message: impl Into<String>) -> Vec<GraphError> {
    vec![GraphError::Param {
        node: node.to_string(),
        field: field.to_string(),
        message: message.into(),
    }]
}

fn parse_aspect(node: &str, raw: &str) -> Result<Aspect, Vec<GraphError>> {
    Aspect::from_tag(raw).ok_or_else(|| {
        param_error(
            node,
            "aspect",
            format!("aspect khong hop le: '{raw}' (chi nhan portrait|landscape|square)"),
        )
    })
}

/// # Errors
/// `GraphError::Scene` (không có ảnh nào) / `GraphError::Param` (`aspect` sai, `credential` rỗng).
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
    let aspect = parse_aspect(node_id, &p.aspect)?;
    if p.credential.trim().is_empty() {
        return Err(param_error(
            node_id,
            "credential",
            "credential (ten/label API key Google) khong duoc rong",
        ));
    }

    let jobs = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| ImageJob {
            scene_index: u32::try_from(idx + 1).unwrap_or(u32::MAX),
            credential_name: p.credential.clone(),
            prompt: item.text.clone(),
            model: p.model.clone(),
            aspect,
        })
        .collect();
    Ok(jobs)
}
