//! Dựng danh sách cảnh (THUẦN) — chuyển nguyên từ `lvc_core::graph::plan::step_video` (P3).

use std::collections::HashSet;

use lvc_core::{CoreError, Limits, RawScene, Scene};
use lvc_dola::SceneJob;
use lvc_node_api::{GraphError, Inputs, TextItem};

use crate::VideoParams;

/// Khoá `TextItem.attrs` (trùng với `node_prompt::attrs` — không import để giữ node độc lập;
/// hợp đồng là chuỗi khoá, ghim bằng test đồ thị đầy đủ trong `tests/`).
const RATIO: &str = "ratio";
const DURATION: &str = "duration";
const MODEL: &str = "model";
const MODE: &str = "mode";
const LOGO: &str = "logo";
const REFS: &str = "refs";
const ACCOUNT: &str = "account";

fn scene_error(node: &str, e: &lvc_core::SceneError) -> GraphError {
    GraphError::Scene {
        node: node.to_string(),
        line: e.line,
        message: e.to_string(),
    }
}

/// `TextItem` → `RawScene` (chưa chuẩn hoá).
fn to_raw(t: &TextItem) -> RawScene {
    let get = |k: &str| t.attrs.get(k).cloned();
    RawScene {
        prompt: (!t.text.is_empty()).then(|| t.text.clone()),
        ratio: get(RATIO),
        duration: get(DURATION),
        model: get(MODEL),
        mode: get(MODE),
        logo: get(LOGO),
        reference_images: get(REFS).map(|r| r.split('\n').map(str::to_string).collect()),
        account: get(ACCOUNT),
    }
}

fn defaults(p: &VideoParams) -> RawScene {
    RawScene {
        prompt: None,
        ratio: Some(p.ratio.as_str().to_string()),
        duration: Some(p.duration.to_string()),
        model: Some(p.model.as_str().to_string()),
        mode: Some(p.mode.as_str().to_string()),
        logo: Some(p.logo.as_str().to_string()),
        reference_images: None,
        account: (!p.account.is_empty()).then(|| p.account.clone()),
    }
}

/// Gộp ảnh tham chiếu: cổng `refs` TRƯỚC, `refs=` trong TXT SAU; bỏ trùng/rỗng, giữ lần xuất hiện
/// ĐẦU TIÊN.
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

/// Mọi cảnh của node: gộp `TextItem` từ cổng `prompt` (thứ tự cạnh), đánh `scene_index` 1..N liên
/// tục, gộp refs (kiểm trần), gộp mặc định + override TXT rồi `Scene::normalize`. Gom HẾT lỗi.
///
/// # Errors
/// `GraphError::Scene` (không có cảnh, vượt `batch_max_scenes`, giá trị sai) / `TooManyRefs`.
pub fn build_jobs(
    node_id: &str,
    p: &VideoParams,
    inputs: &Inputs,
    limits: &Limits,
) -> Result<Vec<SceneJob>, Vec<GraphError>> {
    let port_refs: Vec<String> = inputs
        .assets("refs")
        .map(|a| a.path.to_string_lossy().into_owned())
        .collect();
    let items: Vec<&TextItem> = inputs.texts("prompt").collect();

    if items.is_empty() {
        return Err(vec![GraphError::Scene {
            node: node_id.to_string(),
            line: None,
            message: "khong co canh nao".to_string(),
        }]);
    }
    if items.len() > limits.batch_max_scenes {
        return Err(vec![GraphError::Scene {
            node: node_id.to_string(),
            line: None,
            message: CoreError::TooManyScenes(limits.batch_max_scenes).to_string(),
        }]);
    }

    let d = defaults(p);
    let mut errors = Vec::new();
    let mut jobs = Vec::with_capacity(items.len());
    for (idx, item) in items.into_iter().enumerate() {
        let scene_index = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let raw = to_raw(item);

        let txt_refs = raw.reference_images.clone().unwrap_or_default();
        let merged_refs = merge_refs(&port_refs, &txt_refs);
        if merged_refs.len() > limits.reference_image_max_count {
            errors.push(GraphError::TooManyRefs {
                node: node_id.to_string(),
                scene_index,
                count: merged_refs.len(),
                max: limits.reference_image_max_count,
            });
            continue;
        }

        // TXT thắng mặc định node — TRỪ account: node thắng nếu khác "".
        let merged = RawScene {
            prompt: raw.prompt,
            ratio: raw.ratio.or_else(|| d.ratio.clone()),
            duration: raw.duration.or_else(|| d.duration.clone()),
            model: raw.model.or_else(|| d.model.clone()),
            mode: raw.mode.or_else(|| d.mode.clone()),
            logo: raw.logo.or_else(|| d.logo.clone()),
            reference_images: None,
            account: d.account.clone().or(raw.account),
        };

        match Scene::normalize(&merged, limits, item.line) {
            Ok(mut scene) => {
                scene.reference_images = merged_refs;
                jobs.push(SceneJob { scene_index, scene });
            }
            Err(e) => errors.push(scene_error(&item.source_node, &e)),
        }
    }
    if errors.is_empty() {
        Ok(jobs)
    } else {
        Err(errors)
    }
}
