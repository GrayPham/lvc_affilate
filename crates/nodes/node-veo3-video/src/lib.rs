//! Node `veo3_video` — mỗi `TextItem` trên cổng `prompt` là MỘT job = MỘT video (map ngầm, giống
//! `dola_video`). `mode` chọn `t2v` (text-to-video) hoặc `r2v` (reference-to-video, 1-6 ảnh trên
//! cổng `refs`, dùng CHUNG cho mọi job) — P5 CHƯA làm `i2v`/`interpolation` (spec
//! `docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md` mục 7: 2 rpc đó chưa có
//! cURL thật xác nhận).
//!
//! Chạy thật qua dịch vụ host [`lvc_veo3::VeoVideoHost`] (`NodeCtx::services`) — host tự quyết
//! `local_path`/harvest phiên/mint captcha; node KHÔNG biết `Flow`/`VeoCaptchaHost`/
//! `VeoSessionHost` tồn tại.

mod jobs;

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use lvc_veo3::VeoVideoHost;
use serde::Deserialize;
use serde_json::json;

pub use jobs::{MAX_REFERENCE_IMAGES, build_jobs};

pub const KIND: &str = "veo3_video";

/// Tham số node. `account` = `credential_id` (`google/login`), BẮT BUỘC không rỗng — P5 chưa có
/// "auto" như Dola (xem spec mục 7). `model` rỗng = mặc định theo `mode`
/// (`lvc_veo3::model::default_video_model`).
#[derive(Debug, Clone, Deserialize)]
pub struct VideoParams {
    /// `"t2v"` (text-to-video) hoặc `"r2v"` (reference-to-video, 1-6 ảnh).
    pub mode: String,
    #[serde(default)]
    pub model: String,
    /// `"portrait"` hoặc `"landscape"`.
    pub aspect: String,
    pub seed_count: u32,
    #[serde(default)]
    pub account: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Video,
    label_vi: "Tạo video Veo3".to_string(),
    inputs: vec![
        PortSpec::new("prompt", PortType::Text).many().required(),
        PortSpec::new("refs", PortType::Image).many(),
    ],
    outputs: vec![PortSpec::new("out", PortType::Video).many()],
    require_any: vec![],
    credential: Some(CredentialReq {
        provider: "google".to_string(),
        kinds: vec!["login".to_string()],
    }),
    default_params: json!({
        "mode": "t2v", "model": "", "aspect": "portrait", "seed_count": 1, "account": ""
    }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "mode": { "type": "string", "title": "Che do", "enum": ["t2v", "r2v"] },
            "model": { "type": "string", "title": "Model (rong = mac dinh theo mode)" },
            "aspect": { "type": "string", "title": "Ty le khung", "enum": ["portrait", "landscape"] },
            "seed_count": { "type": "integer", "title": "So bien the (seed_count)", "minimum": 1 },
            "account": { "type": "string", "title": "Tai khoan (ten/label credential Google, bat buoc)" }
        },
        "required": ["mode", "model", "aspect", "seed_count", "account"]
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(Veo3VideoNode)
}

struct Veo3VideoNode;

#[async_trait]
impl NodeExecutor for Veo3VideoNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: VideoParams = parse_params(cx.node_id, params)?;
        let jobs = build_jobs(cx.node_id, &p, inputs)?;
        // Đầu ra giữ chỗ (chưa có file) — node sau biết trước số video/`index`.
        let placeholders = jobs
            .iter()
            .map(|j| {
                Value::Video(
                    Asset::default()
                        .with(meta::NODE, cx.node_id)
                        .with(meta::INDEX, j.scene_index),
                )
            })
            .collect();
        Ok(Preview {
            outputs: Outputs::from([("out".to_string(), placeholders)]),
            jobs: u32::try_from(jobs.len()).unwrap_or(u32::MAX),
        })
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let node_id = &ctx.node_id;
        let p: VideoParams =
            parse_params(node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let jobs =
            build_jobs(node_id, &p, &inputs).map_err(|e| NodeError::new(e[0].to_string()))?;
        let host = ctx.services.get::<Arc<dyn VeoVideoHost>>().ok_or_else(|| {
            NodeError::fatal(format!("node {node_id}: host khong cap VeoVideoHost"))
        })?;

        let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        let report = host.generate_videos(&ctx.run_id, node_id, jobs).await;

        let ok = u32::try_from(report.done.len()).unwrap_or(u32::MAX);
        let failed = u32::try_from(report.failed.len()).unwrap_or(u32::MAX);
        let videos = report
            .done
            .into_iter()
            .map(|d| {
                Value::Video(
                    Asset::new(d.local_path)
                        .with(meta::NODE, node_id)
                        .with(meta::INDEX, d.scene_index),
                )
            })
            .collect();
        Ok(NodeResult {
            outputs: Outputs::from([("out".to_string(), videos)]),
            counts: Counts {
                done: ok + failed,
                total,
                ok,
                failed,
            },
            failure: (!report.failed.is_empty()).then(|| report.failed.join(",")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_khop_file_manifest_json() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json");
        let got = serde_json::to_string_pretty(node().manifest()).unwrap() + "\n";
        if std::env::var_os("LVC_WRITE_FIXTURES").is_some() {
            std::fs::write(path, &got).unwrap();
        }
        let want = std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(
            got, want,
            "manifest.json lech - chay lai voi LVC_WRITE_FIXTURES=1"
        );
    }
}
