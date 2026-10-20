//! Node `museai_image` — mỗi `TextItem` trên cổng `prompt` là MỘT job = MỘT ảnh tĩnh (map ngầm,
//! khớp khuôn `node-museai-video`). Chạy thật qua dịch vụ host [`lvc_museai::MuseaiImageHost`]
//! (`NodeCtx::services`) — host tự quyết `local_path`/đăng nhập/bootstrap VÀ tự gắn hướng dẫn
//! "ảnh tĩnh, không phải video" vào prompt (xem `lvc_museai::protocol::client::build_image_prompt`)
//! nên node ở đây chỉ cần truyền nguyên văn mô tả của người dùng — KHÔNG cần tự thêm từ khoá gì.

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_museai::{ImageJob, MuseaiImageHost};
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "museai_image";

/// Tham số node. `account` = `credential_id` (`museai/login`) — rỗng = tự chọn tài khoản đầu tiên
/// còn dùng được trong kho (`VaultMuseaiAccountResolver`, `lvc-app`), giống `dola_video`.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageParams {
    #[serde(default)]
    pub account: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Image,
    label_vi: "Tạo ảnh MuseAI".to_string(),
    inputs: vec![
        PortSpec::new("prompt", PortType::Text).many().required(),
        // Ảnh tham chiếu dùng CHUNG cho mọi cảnh (khoá nhân vật) — giống `museai_video`.
        PortSpec::new("refs", PortType::Image).many(),
    ],
    outputs: vec![PortSpec::new("out", PortType::Image).many()],
    require_any: vec![],
    credential: Some(CredentialReq {
        provider: "museai".to_string(),
        kinds: vec!["login".to_string()],
    }),
    default_params: json!({ "account": "" }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "account": { "type": "string", "title": "Tài khoản (rỗng = tự chọn)" }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(MuseaiImageNode)
}

struct MuseaiImageNode;

/// Dựng danh sách [`ImageJob`] (THUẦN, không I/O) — mỗi `TextItem` trên cổng `prompt` là MỘT job,
/// `scene_index` 1..N liên tục. `account` rỗng — KHÔNG lỗi, truyền thẳng xuống
/// `MuseaiAccountResolver` để TỰ CHỌN tài khoản (xem `museai_video::build_jobs`). KHÔNG còn nhánh
/// lỗi nào — trả thẳng `Vec` thay vì `Result` (clippy `unnecessary_wraps`).
fn build_jobs(p: &ImageParams, inputs: &Inputs) -> Vec<ImageJob> {
    // Ảnh trên cổng `refs` dùng CHUNG cho mọi cảnh (không map 1-1) — khớp `museai_video::build_jobs`.
    let images: Vec<std::path::PathBuf> = inputs
        .assets("refs")
        .map(|a| std::path::PathBuf::from(&a.path))
        .collect();
    inputs
        .texts("prompt")
        .enumerate()
        .map(|(i, t)| ImageJob {
            scene_index: u32::try_from(i + 1).unwrap_or(u32::MAX),
            credential_id: p.account.clone(),
            prompt: t.text.clone(),
            images: images.clone(),
        })
        .collect()
}

#[async_trait]
impl NodeExecutor for MuseaiImageNode {
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
        let jobs = build_jobs(&p, inputs);
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
        let jobs = build_jobs(&p, &inputs);
        let host = ctx
            .services
            .get::<Arc<dyn MuseaiImageHost>>()
            .ok_or_else(|| {
                NodeError::fatal(format!("node {node_id}: host khong cap MuseaiImageHost"))
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

    #[test]
    fn build_jobs_account_rong_khong_loi_truyen_thang_cho_resolver_tu_chon() {
        let p = ImageParams {
            account: String::new(),
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
        let p = ImageParams {
            account: "tk-1".to_string(),
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

    /// Cổng `refs` (ảnh tham chiếu) — DÙNG CHUNG cho mọi job của node (khoá nhân vật ảnh-trước).
    #[test]
    fn build_jobs_gan_refs_cho_moi_job() {
        use lvc_node_api::{Asset, EdgeInput, TextItem};
        use std::path::PathBuf;
        let p = ImageParams {
            account: "tk-1".to_string(),
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
        inputs.push(
            "refs",
            EdgeInput {
                from_node: "up".into(),
                from_port: "out".into(),
                values: Some(vec![
                    Value::Image(Asset::new("a.png")),
                    Value::Image(Asset::new("b.png")),
                ]),
            },
        );
        let jobs = build_jobs(&p, &inputs);
        assert_eq!(jobs.len(), 2);
        let want = vec![PathBuf::from("a.png"), PathBuf::from("b.png")];
        assert_eq!(jobs[0].images, want, "ảnh refs dùng chung cho mọi cảnh");
        assert_eq!(jobs[1].images, want);
    }

    /// Không nối refs → `images` rỗng, VẪN ra job (fallback: chỉ prompt).
    #[test]
    fn build_jobs_khong_refs_thi_images_rong() {
        use lvc_node_api::{EdgeInput, TextItem};
        let p = ImageParams {
            account: String::new(),
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
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].images.is_empty());
    }
}
