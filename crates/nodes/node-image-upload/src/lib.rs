//! Node `image_upload` — danh sách file ảnh người dùng chọn → cổng `out` (`Value::Image`).
//! `label` (tuỳ chọn) gắn vào `Asset.meta.label` để node tiêu thụ hiện chip tham chiếu
//! ("character", "cloths"...). Thiếu file khi chạy = lỗi NGHIÊM TRỌNG (dừng cả run, như P3).

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor, NodeManifest,
    NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value, meta,
    parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "image_upload";

#[derive(Debug, Deserialize)]
struct UploadParams {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    label: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Input,
    label_vi: "Tải ảnh lên".to_string(),
    inputs: vec![],
    outputs: vec![PortSpec::new("out", PortType::Image).many()],
    require_any: vec![],
    credential: None,
    default_params: json!({ "paths": [] }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "paths": { "type": "array", "items": { "type": "string" }, "title": "Ảnh", "format": "image-files" },
            "label": { "type": "string", "title": "Nhãn" }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(ImageUploadNode)
}

struct ImageUploadNode;

fn outputs(node_id: &str, p: &UploadParams) -> Outputs {
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
            Value::Image(a)
        })
        .collect();
    Outputs::from([("out".to_string(), values)])
}

#[async_trait]
impl NodeExecutor for ImageUploadNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        _inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: UploadParams = parse_params(cx.node_id, params)?;
        Ok(Preview::single(outputs(cx.node_id, &p)))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        _inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let p: UploadParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let mut missing = Vec::new();
        for path in &p.paths {
            if tokio::fs::metadata(path).await.is_err() {
                missing.push(path.clone());
            }
        }
        if !missing.is_empty() {
            return Err(NodeError::fatal(format!(
                "node {}: thieu file anh: {}",
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
    use lvc_node_api::Services;

    fn params(v: serde_json::Value) -> Params {
        match v {
            serde_json::Value::Object(m) => m,
            _ => panic!("params phai la object"),
        }
    }

    fn ctx() -> NodeCtx {
        NodeCtx {
            run_id: "r".into(),
            workflow_id: "w".into(),
            node_id: "img".into(),
            cancel: tokio_util::sync::CancellationToken::new(),
            limits: Arc::new(Limits::default()),
            services: Services::new(),
        }
    }

    #[test]
    fn preview_ra_anh_theo_thu_tu_kem_nhan() {
        let p = node()
            .preview(
                &PreviewCtx {
                    node_id: "img",
                    limits: &Limits::default(),
                },
                &params(json!({"paths": ["a.jpg", "b.jpg"], "label": "character"})),
                &Inputs::new(),
            )
            .unwrap();
        let a: Vec<&Asset> = p.outputs["out"]
            .iter()
            .filter_map(Value::as_asset)
            .collect();
        assert_eq!(a.len(), 2);
        assert_eq!(a[1].path.to_str(), Some("b.jpg"));
        assert_eq!(a[0].get(meta::LABEL), Some("character"));
        assert_eq!(a[1].get(meta::INDEX), Some("2"));
    }

    #[tokio::test]
    async fn run_thieu_file_la_loi_nghiem_trong() {
        let e = node()
            .run(
                &ctx(),
                &params(json!({"paths": ["khong_co.jpg"]})),
                Inputs::new(),
            )
            .await
            .unwrap_err();
        assert!(e.fatal);
        assert_eq!(e.message, "node img: thieu file anh: khong_co.jpg");
    }

    #[tokio::test]
    async fn run_du_file_ok() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.jpg");
        std::fs::write(&f, b"x").unwrap();
        let r = node()
            .run(
                &ctx(),
                &params(json!({"paths": [f.to_str().unwrap()]})),
                Inputs::new(),
            )
            .await
            .unwrap();
        assert_eq!(r.outputs["out"].len(), 1);
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
