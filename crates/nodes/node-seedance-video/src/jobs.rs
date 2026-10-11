//! Dựng danh sách [`VideoJob`] (THUẦN). Mỗi `TextItem` cổng `prompt` = MỘT job. `mode="i2v"` lấy
//! ảnh ĐẦU TIÊN cổng `ref` làm ảnh gốc (engine tự dựng data URL).

use std::path::PathBuf;

use lvc_node_api::{GraphError, Inputs, TextItem};
use lvc_seedance::job::VideoJob;
use lvc_seedance::req::Aspect;

use crate::VideoParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModeKind {
    T2v,
    I2v,
}

impl ModeKind {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "t2v" => Some(Self::T2v),
            "i2v" => Some(Self::I2v),
            _ => None,
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
            format!("mode khong hop le: '{}' (chi nhan t2v|i2v)", p.mode),
        )
    })?;
    let aspect = parse_aspect(node_id, &p.aspect)?;
    if p.credential.trim().is_empty() {
        return Err(param_error(
            node_id,
            "credential",
            "credential (ten/label API key Seedance) khong duoc rong",
        ));
    }

    let ref_image_path: Option<PathBuf> = if mode == ModeKind::I2v {
        let first = inputs.assets("ref").map(|a| a.path.clone()).next();
        if first.is_none() {
            return Err(param_error(
                node_id,
                "ref",
                "mode i2v nhung cong 'ref' khong co anh nao",
            ));
        }
        first
    } else {
        None
    };

    let jobs = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| VideoJob {
            scene_index: u32::try_from(idx + 1).unwrap_or(u32::MAX),
            credential_name: p.credential.clone(),
            prompt: item.text.clone(),
            model: p.model.clone(),
            aspect,
            ref_image_path: ref_image_path.clone(),
        })
        .collect();
    Ok(jobs)
}
