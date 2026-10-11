//! Dựng danh sách [`ImageJob`] (THUẦN). Mỗi `TextItem` cổng `prompt` = MỘT ảnh. Seedream không nhận
//! ảnh tham chiếu ở t2i nên node này không có cổng `refs`.

use lvc_node_api::{GraphError, Inputs, TextItem};
use lvc_seedance::job::ImageJob;
use lvc_seedance::req::Aspect;

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
/// `GraphError::Scene` / `GraphError::Param`.
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
            "credential (ten/label API key Seedance) khong duoc rong",
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
