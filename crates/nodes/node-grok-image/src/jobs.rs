//! Dựng danh sách [`ImageJob`] (THUẦN). Mỗi `TextItem` cổng `prompt` = MỘT ảnh. Grok image API
//! không nhận tỉ lệ khung nên node chỉ có `model` + `credential`.

use lvc_grok::job::ImageJob;
use lvc_node_api::{GraphError, Inputs, TextItem};

use crate::ImageParams;

/// # Errors
/// `GraphError::Scene` (không có ảnh) / `GraphError::Param` (`credential` rỗng).
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
    if p.credential.trim().is_empty() {
        return Err(vec![GraphError::Param {
            node: node_id.to_string(),
            field: "credential".to_string(),
            message: "credential (ten/label API key Grok) khong duoc rong".to_string(),
        }]);
    }

    let jobs = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| ImageJob {
            scene_index: u32::try_from(idx + 1).unwrap_or(u32::MAX),
            credential_name: p.credential.clone(),
            prompt: item.text.clone(),
            model: p.model.clone(),
        })
        .collect();
    Ok(jobs)
}
