//! Node `dola_video` — mỗi `TextItem` trên cổng `prompt` là MỘT cảnh = MỘT video (map ngầm, spec
//! §4.3). Ảnh ở cổng `refs` gắn vào MỌI cảnh. Mặc định lấy từ tham số node; override TXT trên dòng
//! thắng mặc định (riêng `account`: node thắng nếu khác rỗng).
//!
//! Chạy thật qua dịch vụ host [`lvc_dola::DolaVideoHost`] (`NodeCtx::services`) — host sở hữu pool
//! tài khoản, limiter, bảng `tasks`. Node không tự mở trình duyệt/gọi mạng.

mod jobs;

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_core::{LogoMode, Mode, Model, Ratio};
use lvc_dola::DolaVideoHost;
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub use jobs::build_jobs;

pub const KIND: &str = "dola_video";

/// Tham số node (khớp `NodeKind::Video` của P3 — file workflow cũ đọc được nguyên vẹn).
#[derive(Debug, Clone, Deserialize)]
pub struct VideoParams {
    pub mode: Mode,
    pub model: Model,
    pub ratio: Ratio,
    pub duration: u16,
    pub logo: LogoMode,
    #[serde(default)]
    pub account: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Video,
    label_vi: "Tạo video Dola".to_string(),
    inputs: vec![
        PortSpec::new("prompt", PortType::Text).many().required(),
        PortSpec::new("refs", PortType::Image).many(),
    ],
    outputs: vec![PortSpec::new("out", PortType::Video).many()],
    require_any: vec![],
    credential: Some(CredentialReq {
        provider: "dola".to_string(),
        kinds: vec!["session".to_string()],
    }),
    default_params: json!({
        "mode": "fast", "model": "seedance-2.0", "ratio": "9:16",
        "duration": 10, "logo": "off", "account": ""
    }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "mode": { "type": "string", "title": "Chế độ", "enum": ["fast", "pro", "pro_max", "agent"] },
            "model": { "type": "string", "title": "Model", "enum": ["seedance-2.0", "seedance-2.5", "seedance-1.0"] },
            "ratio": { "type": "string", "title": "Tỉ lệ", "enum": ["9:16", "16:9", "1:1", "4:3", "3:4"] },
            "duration": { "type": "integer", "title": "Thời lượng (giây)" },
            "logo": { "type": "string", "title": "Logo", "enum": ["off", "crop", "delogo", "auto"] },
            "account": { "type": "string", "title": "Tài khoản (rỗng = tự chọn)" }
        },
        "required": ["mode", "model", "ratio", "duration", "logo"]
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(DolaVideoNode)
}

struct DolaVideoNode;

#[async_trait]
impl NodeExecutor for DolaVideoNode {
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
        let jobs = build_jobs(cx.node_id, &p, inputs, cx.limits)?;
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
        let jobs = build_jobs(node_id, &p, &inputs, &ctx.limits)
            .map_err(|e| NodeError::new(e[0].to_string()))?;
        let host = ctx
            .services
            .get::<Arc<dyn DolaVideoHost>>()
            .ok_or_else(|| {
                NodeError::fatal(format!("node {node_id}: host khong cap DolaVideoHost"))
            })?;

        let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        let report = host.run_jobs(&ctx.run_id, node_id, jobs, &ctx.cancel).await;

        let ok = u32::try_from(report.done.len()).unwrap_or(u32::MAX);
        let failed = u32::try_from(report.failed.len()).unwrap_or(u32::MAX);
        let videos = report
            .done
            .into_iter()
            .map(|d| {
                Value::Video(
                    Asset::new(d.local_path)
                        .with(meta::NODE, node_id)
                        .with(meta::INDEX, d.scene_index)
                        .with("account", d.account)
                        .with("conversation_id", d.conversation_id)
                        .with("watermarked", d.watermarked),
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
            failure: (!report.failed.is_empty())
                .then(|| format!("node {node_id}: canh {} that bai", report.failed.join(","))),
        })
    }
}
