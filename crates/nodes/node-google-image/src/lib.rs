//! Node `google_image` — tạo ảnh bằng **API chính thức** Google (Imagen 3). Mỗi `TextItem` trên cổng
//! `prompt` là MỘT ảnh (map ngầm, giống `veo3_image`). Người dùng nhập API key Google của chính họ
//! (cùng key Gemini) ở màn Tài khoản AI; node chỉ truyền TÊN credential — KHÔNG thấy key.
//!
//! Chạy thật qua dịch vụ host [`lvc_google::GoogleImageHost`] (`NodeCtx::services`) — engine impl
//! (resolve API key từ vault rồi gọi Imagen, tự quyết `local_path`). Node không tự gọi mạng.

mod jobs;

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_google::GoogleImageHost;
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub use jobs::build_jobs;

pub const KIND: &str = "google_image";

fn default_aspect() -> String {
    "portrait".to_string()
}

/// Tham số node. `credential` = TÊN (label) API key Google trong kho, BẮT BUỘC. `model` rỗng =
/// mặc định (`lvc_google::default_model::IMAGE`).
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
    label_vi: "Tạo ảnh Google (API Imagen)".to_string(),
    inputs: vec![PortSpec::new("prompt", PortType::Text).many().required()],
    outputs: vec![PortSpec::new("out", PortType::Image).many()],
    require_any: vec![],
    // API key chinh thuc Google = cung key voi Gemini text (provider vault "gemini", kind api_key).
    credential: Some(CredentialReq {
        provider: "gemini".to_string(),
        kinds: vec!["api_key".to_string()],
    }),
    default_params: json!({ "model": "", "aspect": "portrait", "credential": "" }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "model": { "type": "string", "title": "Model (rong = gemini-3.1-flash-image)" },
            "aspect": { "type": "string", "title": "Ty le khung", "enum": ["portrait", "landscape", "square"] },
            "credential": { "type": "string", "title": "Ten API key Google", "minLength": 1 }
        },
        "required": ["aspect", "credential"]
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(GoogleImageNode)
}

struct GoogleImageNode;

#[async_trait]
impl NodeExecutor for GoogleImageNode {
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
            .get::<Arc<dyn GoogleImageHost>>()
            .ok_or_else(|| {
                NodeError::fatal(format!("node {node_id}: host khong cap GoogleImageHost"))
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
