//! Node `museai_video` — mỗi `TextItem` trên cổng `prompt` là MỘT job = MỘT video (map ngầm,
//! giống `dola_video`/`veo3_video`). Chỉ prompt văn bản — CHƯA có ảnh tham chiếu (xem `CLAUDE.md`
//! mục "Tham số"). Chạy thật qua dịch vụ host [`lvc_museai::MuseaiVideoHost`]
//! (`NodeCtx::services`) — host tự quyết `local_path`/đăng nhập/bootstrap; node KHÔNG biết trình
//! duyệt/giao thức Hatch tồn tại (xem `CLAUDE.md` mục "Cơ chế tạo video").

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_museai::{MuseaiVideoHost, VideoJob};
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "museai_video";

/// Tham số node. `account` = `credential_id` (`museai/login`) — rỗng = tự chọn tài khoản đầu tiên
/// còn dùng được trong kho (`VaultMuseaiAccountResolver`, `lvc-app`), giống `dola_video`.
///
/// `aspect`/`duration` — xem CẢNH BÁO ở `CLAUDE.md` mục "Tham số": muse.ai KHÔNG có tham số API
/// cho hai thứ này (xác nhận qua `GET /model` thật) — rỗng/`None` = không chèn gì; có giá trị =
/// CHỈ chèn thêm câu chỉ thị vào prompt (`lvc_museai::protocol::client::build_video_prompt`),
/// KHÔNG đảm bảo AI tuân theo.
#[derive(Debug, Clone, Deserialize)]
pub struct VideoParams {
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub aspect: String,
    #[serde(default)]
    pub duration: Option<u32>,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Video,
    label_vi: "Tạo video MuseAI".to_string(),
    inputs: vec![PortSpec::new("prompt", PortType::Text).many().required()],
    outputs: vec![PortSpec::new("out", PortType::Video).many()],
    require_any: vec![],
    credential: Some(CredentialReq {
        provider: "museai".to_string(),
        kinds: vec!["login".to_string()],
    }),
    default_params: json!({ "account": "", "aspect": "", "duration": null }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "account": { "type": "string", "title": "Tài khoản (rỗng = tự chọn)" },
            "aspect": {
                "type": "string",
                "title": "Tỉ lệ khung hình (chỉ chèn vào prompt — muse.ai không có API, không đảm bảo)",
                "enum": ["", "16:9", "9:16", "1:1"]
            },
            "duration": { "type": ["integer", "null"], "title": "Thời lượng giây (chỉ chèn vào prompt, không đảm bảo)" }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(MuseaiVideoNode)
}

struct MuseaiVideoNode;

/// Dựng danh sách [`VideoJob`] (THUẦN, không I/O) — mỗi `TextItem` trên cổng `prompt` là MỘT job,
/// `scene_index` 1..N liên tục. `account` rỗng — KHÔNG lỗi, `credential_id` rỗng truyền thẳng
/// xuống `MuseaiAccountResolver` (`VaultMuseaiAccountResolver`) để TỰ CHỌN tài khoản đầu tiên còn
/// dùng được trong kho, giống hành vi "để trống = tự chọn" của `dola_video` (review người dùng
/// 2026-10-13 — trước đây bắt buộc gõ tay, không có lựa chọn tự động). KHÔNG còn nhánh lỗi nào —
/// trả thẳng `Vec` thay vì `Result` (giữ `GraphError` trong chữ ký sẽ bị clippy
/// `unnecessary_wraps` bắt).
fn build_jobs(p: &VideoParams, inputs: &Inputs) -> Vec<VideoJob> {
    let aspect = (!p.aspect.trim().is_empty()).then(|| p.aspect.clone());
    inputs
        .texts("prompt")
        .enumerate()
        .map(|(i, t)| VideoJob {
            scene_index: u32::try_from(i + 1).unwrap_or(u32::MAX),
            credential_id: p.account.clone(),
            prompt: t.text.clone(),
            aspect: aspect.clone(),
            duration_secs: p.duration,
        })
        .collect()
}

#[async_trait]
impl NodeExecutor for MuseaiVideoNode {
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
        let jobs = build_jobs(&p, inputs);
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
        let jobs = build_jobs(&p, &inputs);
        let host = ctx
            .services
            .get::<Arc<dyn MuseaiVideoHost>>()
            .ok_or_else(|| {
                NodeError::fatal(format!("node {node_id}: host khong cap MuseaiVideoHost"))
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

    #[test]
    fn build_jobs_account_rong_khong_loi_truyen_thang_cho_resolver_tu_chon() {
        let p = VideoParams {
            account: String::new(),
            aspect: String::new(),
            duration: None,
        };
        let jobs = build_jobs(&p, &Inputs::new());
        assert!(
            jobs.is_empty(),
            "khong co prompt noi vao -> 0 job, nhung khong loi"
        );
    }

    #[test]
    fn build_jobs_map_dung_tung_prompt_thanh_mot_job() {
        use lvc_node_api::{EdgeInput, TextItem};
        let p = VideoParams {
            account: "tk-1".to_string(),
            aspect: String::new(),
            duration: None,
        };
        let mut inputs = Inputs::new();
        inputs.push(
            "prompt",
            EdgeInput {
                from_node: "pr".into(),
                from_port: "out".into(),
                values: Some(vec![
                    Value::Text(TextItem {
                        text: "canh 1".into(),
                        ..TextItem::default()
                    }),
                    Value::Text(TextItem {
                        text: "canh 2".into(),
                        ..TextItem::default()
                    }),
                ]),
            },
        );
        let jobs = build_jobs(&p, &inputs);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].scene_index, 1);
        assert_eq!(jobs[1].scene_index, 2);
        assert_eq!(jobs[0].credential_id, "tk-1");
        assert_eq!(jobs[0].prompt, "canh 1");
    }

    #[test]
    fn build_jobs_aspect_duration_truyen_thang_vao_moi_job() {
        use lvc_node_api::{EdgeInput, TextItem};
        let p = VideoParams {
            account: "tk-1".to_string(),
            aspect: "9:16".to_string(),
            duration: Some(30),
        };
        let mut inputs = Inputs::new();
        inputs.push(
            "prompt",
            EdgeInput {
                from_node: "pr".into(),
                from_port: "out".into(),
                values: Some(vec![Value::Text(TextItem {
                    text: "canh 1".into(),
                    ..TextItem::default()
                })]),
            },
        );
        let jobs = build_jobs(&p, &inputs);
        assert_eq!(jobs[0].aspect.as_deref(), Some("9:16"));
        assert_eq!(jobs[0].duration_secs, Some(30));
    }

    #[test]
    fn build_jobs_aspect_rong_la_none() {
        use lvc_node_api::{EdgeInput, TextItem};
        let p = VideoParams {
            account: "tk-1".to_string(),
            aspect: String::new(),
            duration: None,
        };
        let mut inputs = Inputs::new();
        inputs.push(
            "prompt",
            EdgeInput {
                from_node: "pr".into(),
                from_port: "out".into(),
                values: Some(vec![Value::Text(TextItem {
                    text: "canh 1".into(),
                    ..TextItem::default()
                })]),
            },
        );
        let jobs = build_jobs(&p, &inputs);
        assert_eq!(jobs[0].aspect, None);
        assert_eq!(jobs[0].duration_secs, None);
    }

    #[test]
    fn preview_thuan_tra_dung_so_job_giu_cho() {
        use lvc_core::Limits;
        let p = params_json("tk-1", &["a", "b", "c"]);
        let prev = node()
            .preview(
                &PreviewCtx {
                    node_id: "n1",
                    limits: &Limits::default(),
                },
                &p,
                &Inputs::new(),
            )
            .unwrap();
        assert_eq!(
            prev.jobs, 0,
            "khong co input prompt noi vao -> 0 job luc preview"
        );
    }

    fn params_json(account: &str, _prompts: &[&str]) -> Params {
        json!({ "account": account }).as_object().unwrap().clone()
    }
}
