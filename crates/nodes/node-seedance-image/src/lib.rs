//! Node `seedance_image` — tạo ảnh bằng API chính thức Seedream (Volcengine). Mỗi `TextItem` cổng
//! `prompt` là MỘT ảnh. Người dùng nhập API key Volcengine Ark; node chỉ truyền TÊN credential.
//!
//! Dịch vụ host [`lvc_seedance::SeedanceImageHost`] (`NodeCtx::services`) — engine resolve key +
//! gọi Seedream + ghi file. Node không tự gọi mạng.

mod jobs;

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use lvc_seedance::SeedanceImageHost;
use serde::Deserialize;
use serde_json::json;

pub use jobs::build_jobs;

pub const KIND: &str = "seedance_image";

fn default_aspect() -> String {
    "square".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageParams {
    #[serde(default)]
    pub model: String,
    /// `"portrait"` | `"landscape"` | `"square"`.
    #[serde(default = "default_aspect")]
    pub aspect: String,
    #[serde(default)]
    pub credential: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Image,
    label_vi: "Tạo ảnh Seedance (API Seedream)".to_string(),
    inputs: vec![PortSpec::new("prompt", PortType::Text).many().required()],
    outputs: vec![PortSpec::new("out", PortType::Image).many()],
    require_any: vec![],
    credential: Some(CredentialReq {
        provider: "seedance".to_string(),
        kinds: vec!["api_key".to_string()],
    }),
    default_params: json!({ "model": "", "aspect": "square", "credential": "" }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "model": { "type": "string", "title": "Model / endpoint id (rong = mac dinh)" },
            "aspect": { "type": "string", "title": "Ty le khung", "enum": ["portrait", "landscape", "square"] },
            "credential": { "type": "string", "title": "Ten API key Seedance", "minLength": 1 }
        },
        "required": ["aspect", "credential"]
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(SeedanceImageNode)
}

struct SeedanceImageNode;

#[async_trait]
impl NodeExecutor for SeedanceImageNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: ImageParams = parse_params(cx.node_id, params)?;
        let jobs = build_jobs(cx.node_id, &p, inputs)?;
        let placeholders = jobs
            .iter()
            .map(|j| {
                Value::Image(
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
        let p: ImageParams =
            parse_params(node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let jobs =
            build_jobs(node_id, &p, &inputs).map_err(|e| NodeError::new(e[0].to_string()))?;
        let host = ctx
            .services
            .get::<Arc<dyn SeedanceImageHost>>()
            .ok_or_else(|| {
                NodeError::fatal(format!("node {node_id}: host khong cap SeedanceImageHost"))
            })?;

        let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        let report = host.generate_images(&ctx.run_id, node_id, jobs).await;

        let ok = u32::try_from(report.done.len()).unwrap_or(u32::MAX);
        let failed = u32::try_from(report.failed.len()).unwrap_or(u32::MAX);
        let images = report
            .done
            .into_iter()
            .map(|d| {
                Value::Image(
                    Asset::new(d.local_path)
                        .with(meta::NODE, node_id)
                        .with(meta::INDEX, d.scene_index),
                )
            })
            .collect();
        Ok(NodeResult {
            outputs: Outputs::from([("out".to_string(), images)]),
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
