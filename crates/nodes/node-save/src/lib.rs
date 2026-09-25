//! Node `save` — copy (không di chuyển) video tới `out_dir/<run_id>_<node_nguon>_<index>.mp4`.
//! Nhận `video` HOẶC `image` (`require_any`). Ảnh: P4a chưa copy (giữ hành vi P3 — no-op).
//! Cạnh mà node nguồn không có đầu ra (bị bỏ qua/lỗi) → lỗi "khong co dau ra de luu".
//!
//! Tên file dùng id NODE NGUỒN (không phải id node Lưu): một node Lưu nhận từ nhiều node video,
//! dùng id node Lưu sẽ trùng tên giữa các nguồn có cùng `index`.

use std::path::Path;
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, Counts, EdgeInput, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

pub const KIND: &str = "save";

#[derive(Debug, Deserialize)]
struct SaveParams {
    #[serde(default)]
    out_dir: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Output,
    label_vi: "Lưu".to_string(),
    inputs: vec![
        PortSpec::new("video", PortType::Video).many(),
        PortSpec::new("image", PortType::Image).many(),
    ],
    outputs: vec![],
    require_any: vec![vec!["video".to_string(), "image".to_string()]],
    credential: None,
    default_params: json!({ "out_dir": "" }),
    params_schema: json!({
        "type": "object",
        "properties": { "out_dir": { "type": "string", "title": "Thư mục lưu", "format": "directory" } }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(SaveNode)
}

struct SaveNode;

async fn copy_videos(
    run_id: &str,
    edge: &EdgeInput,
    videos: &[&Asset],
    out_dir: &Path,
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(out_dir).await?;
    for (i, a) in videos.iter().enumerate() {
        let idx = a
            .get(meta::INDEX)
            .map_or_else(|| (i + 1).to_string(), str::to_string);
        let dest = out_dir.join(format!("{run_id}_{}_{idx}.mp4", edge.from_node));
        tokio::fs::copy(&a.path, &dest).await?;
    }
    Ok(())
}

#[async_trait]
impl NodeExecutor for SaveNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        _inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let _: SaveParams = parse_params(cx.node_id, params)?;
        Ok(Preview::single(Outputs::new()))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let p: SaveParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let node_id = &ctx.node_id;
        let mut ok = true;
        for edge in inputs.edges("video") {
            let Some(values) = &edge.values else {
                warn!(run_id = ctx.run_id, node_id, from = %edge.from_node, "khong co dau ra de luu");
                ok = false;
                continue;
            };
            let videos: Vec<&Asset> = values
                .iter()
                .filter_map(|v| match v {
                    Value::Video(a) => Some(a),
                    _ => None,
                })
                .collect();
            if let Err(e) = copy_videos(&ctx.run_id, edge, &videos, Path::new(&p.out_dir)).await {
                warn!(run_id = ctx.run_id, node_id, from = %edge.from_node, error = %e, "khong luu duoc video");
                ok = false;
            }
        }
        for edge in inputs.edges("image") {
            // P4a: chua copy anh (giu hanh vi P3) — chi kiem nguon co dau ra.
            if edge.values.is_none() {
                warn!(run_id = ctx.run_id, node_id, from = %edge.from_node, "khong co dau ra de luu");
                ok = false;
            }
        }
        Ok(if ok {
            NodeResult::ok(Outputs::new())
        } else {
            NodeResult {
                outputs: Outputs::new(),
                counts: Counts::ONE_FAILED,
                failure: Some(format!("{node_id}: luu loi")),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_core::Limits;
    use lvc_node_api::Services;

    fn ctx() -> NodeCtx {
        NodeCtx {
            run_id: "run1".into(),
            workflow_id: "w".into(),
            node_id: "sv".into(),
            cancel: tokio_util::sync::CancellationToken::new(),
            limits: Arc::new(Limits::default()),
            services: Services::new(),
        }
    }

    fn params(dir: &Path) -> Params {
        json!({ "out_dir": dir.to_str().unwrap() })
            .as_object()
            .unwrap()
            .clone()
    }

    #[tokio::test]
    async fn copy_video_ten_theo_run_node_nguon_index() {
        let src = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let f = src.path().join("x.mp4");
        std::fs::write(&f, b"v").unwrap();
        let mut inputs = Inputs::new();
        inputs.push(
            "video",
            EdgeInput {
                from_node: "vid".into(),
                from_port: "out".into(),
                values: Some(vec![Value::Video(Asset::new(&f).with(meta::INDEX, 3))]),
            },
        );
        let r = node()
            .run(&ctx(), &params(out.path()), inputs)
            .await
            .unwrap();
        assert!(!r.is_failed());
        assert!(out.path().join("run1_vid_3.mp4").exists());
        assert!(f.exists(), "copy, khong di chuyen");
    }

    #[tokio::test]
    async fn nguon_khong_co_dau_ra_la_luu_loi() {
        let out = tempfile::tempdir().unwrap();
        let mut inputs = Inputs::new();
        inputs.push(
            "video",
            EdgeInput {
                from_node: "vid".into(),
                from_port: "out".into(),
                values: None,
            },
        );
        let r = node()
            .run(&ctx(), &params(out.path()), inputs)
            .await
            .unwrap();
        assert_eq!(r.failure.as_deref(), Some("sv: luu loi"));
        assert_eq!(r.counts, Counts::ONE_FAILED);
    }

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
            "manifest.json lech — chay lai voi LVC_WRITE_FIXTURES=1"
        );
    }
}
