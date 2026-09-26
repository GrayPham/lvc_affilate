//! Node `load_video` — video người dùng chọn từ máy → cổng `out` (`Value::Video`). Nguồn video cục
//! bộ để đưa vào Cắt video / Chụp khung hình / Ghép phân cảnh. `label` (tuỳ chọn) gắn vào
//! `Asset.meta.label` cho chip tham chiếu. Thiếu file khi chạy = lỗi NGHIÊM TRỌNG (dừng cả run).

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor, NodeManifest,
    NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value, meta,
    parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "load_video";

#[derive(Debug, Deserialize)]
struct LoadParams {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    label: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Input,
    label_vi: "Tải video lên".to_string(),
    inputs: vec![],
    outputs: vec![PortSpec::new("out", PortType::Video).many()],
    require_any: vec![],
    credential: None,
    default_params: json!({ "paths": [] }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "paths": { "type": "array", "items": { "type": "string" }, "title": "Video", "format": "video-files" },
            "label": { "type": "string", "title": "Nhãn" }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(LoadVideoNode)
}

struct LoadVideoNode;

fn outputs(node_id: &str, p: &LoadParams) -> Outputs {
    let values = p
        .paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let mut a = Asset::new(path)
                .with(meta::NODE, node_id)
                .with(meta::INDEX, i + 1);
            if !p.label.is_empty() {
                a = a.with(meta::LABEL, &p.label);
            }
            Value::Video(a)
        })
        .collect();
    Outputs::from([("out".to_string(), values)])
}

#[async_trait]
impl NodeExecutor for LoadVideoNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        _inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: LoadParams = parse_params(cx.node_id, params)?;
        Ok(Preview::single(outputs(cx.node_id, &p)))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        _inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let p: LoadParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let mut missing = Vec::new();
        for path in &p.paths {
            if tokio::fs::metadata(path).await.is_err() {
                missing.push(path.clone());
            }
        }
        if !missing.is_empty() {
            return Err(NodeError::fatal(format!(
                "node {}: thieu file video: {}",
                ctx.node_id,
                missing.join(", ")
            )));
        }
        Ok(NodeResult::ok(outputs(&ctx.node_id, &p)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_core::Limits;

    fn params(v: serde_json::Value) -> Params {
        match v {
            serde_json::Value::Object(m) => m,
            _ => panic!("params phai la object"),
        }
    }

    #[test]
    fn preview_ra_video_theo_thu_tu_kem_nhan() {
        let p = node()
            .preview(
                &PreviewCtx {
                    node_id: "lv",
                    limits: &Limits::default(),
                },
                &params(json!({"paths": ["a.mp4", "b.mp4"], "label": "clip"})),
                &Inputs::new(),
            )
            .unwrap();
        let a: Vec<&Asset> = p.outputs["out"]
            .iter()
            .filter_map(Value::as_asset)
            .collect();
        assert_eq!(a.len(), 2);
        assert_eq!(a[1].path.to_str(), Some("b.mp4"));
        assert_eq!(a[0].get(meta::LABEL), Some("clip"));
        assert_eq!(a[1].get(meta::INDEX), Some("2"));
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
            "manifest.json lech - chay lai voi LVC_WRITE_FIXTURES=1"
        );
    }
}
