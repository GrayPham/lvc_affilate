//! Node `veo3_image` — mỗi `TextItem` trên cổng `prompt` là MỘT ảnh (map ngầm, giống `dola_video`/
//! `veo3_video` map `prompt` → cảnh/job). Ảnh ở cổng `refs` gắn vào MỌI job. Đồng bộ — không cần
//! state machine polling trong node (gọi 1 lần, trả ảnh ngay); xem `docs/superpowers/specs/
//! 2026-09-27-veo3-image-video-nodes-design.md` ADR-R019 mục "Node & credential".
//!
//! Chạy thật qua dịch vụ host [`lvc_veo3::VeoImageHost`] (`NodeCtx::services`) — host (thường
//! `lvc-engine`) tự quyết `local_path`/harvest phiên/mint captcha; node không tự mở trình duyệt/
//! gọi mạng, không biết `VeoCaptchaHost`/`VeoSessionHost`/`Flow` tồn tại.

mod jobs;

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use lvc_veo3::VeoImageHost;
use serde::Deserialize;
use serde_json::json;

pub use jobs::build_jobs;

pub const KIND: &str = "veo3_image";

fn default_model() -> String {
    "GEM_PIX_2".to_string()
}

fn default_aspect() -> String {
    "portrait".to_string()
}

/// Tham số node (khớp bảng "Node & credential" của spec — `model`/`aspect` là chuỗi thô, node tự
/// `parse` trong `jobs.rs` để báo lỗi rõ tại đúng field). `account` = `credential_id`
/// (`google/login`) — rỗng = tự chọn tài khoản đầu tiên còn dùng được trong kho
/// (`VaultVeo3AccountResolver`, `lvc-app`), giống `dola_video`.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageParams {
    #[serde(default = "default_model")]
    pub model: String,
    /// `"portrait"` hoặc `"landscape"`.
    #[serde(default = "default_aspect")]
    pub aspect: String,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub account: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Image,
    label_vi: "Tạo ảnh Veo3 (Google Labs Flow)".to_string(),
    inputs: vec![
        PortSpec::new("prompt", PortType::Text).many().required(),
        PortSpec::new("refs", PortType::Image).many(),
    ],
    outputs: vec![PortSpec::new("out", PortType::Image).many()],
    require_any: vec![],
    credential: Some(CredentialReq {
        provider: "google".to_string(),
        kinds: vec!["login".to_string()],
    }),
    default_params: json!({
        "model": "GEM_PIX_2", "aspect": "portrait", "seed": null, "account": ""
    }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "model": { "type": "string", "title": "Model", "enum": ["NARWHAL", "GEM_PIX_2", "HARBOR_SEAL"] },
            "aspect": { "type": "string", "title": "Ty le khung", "enum": ["portrait", "landscape"] },
            "seed": { "type": ["integer", "null"], "title": "Seed (rong = 0)" },
            "account": { "type": "string", "title": "Tai khoan (rong = tu chon)" }
        },
        "required": ["model", "aspect"]
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(Veo3ImageNode)
}

struct Veo3ImageNode;

#[async_trait]
impl NodeExecutor for Veo3ImageNode {
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
        // Đầu ra giữ chỗ (chưa có file) — node sau biết trước số ảnh/`index`.
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
        let host = ctx.services.get::<Arc<dyn VeoImageHost>>().ok_or_else(|| {
            NodeError::fatal(format!("node {node_id}: host khong cap VeoImageHost"))
        })?;

        let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        let report = host.generate_images(&ctx.run_id, node_id, jobs).await;

        let ok = u32::try_from(report.done.len()).unwrap_or(u32::MAX);
        let failed = u32::try_from(report.failed.len()).unwrap_or(u32::MAX);
        let images = report
            .done
            .into_iter()
            .map(|d| {
                let mut a = Asset::new(d.local_path)
                    .with(meta::NODE, node_id)
                    .with(meta::INDEX, d.scene_index);
                if let Some(w) = d.width {
                    a = a.with("width", w);
                }
                if let Some(h) = d.height {
                    a = a.with("height", h);
                }
                Value::Image(a)
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
