//! Node `note` — ghi chú tĩnh trên canvas (lưu ý/cách dùng/các bước setup template). KHÔNG có
//! cổng vào/ra — không tham gia luồng dữ liệu, chỉ đứng cạnh các node khác để người đọc template
//! biết cách dùng ngay. `run`/`preview` là no-op (không I/O, không tốn credit).

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Category, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor, NodeManifest, NodeResult,
    Outputs, Params, Preview, PreviewCtx, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "note";

#[derive(Debug, Deserialize)]
#[allow(dead_code, reason = "chi hien tren UI, run() khong dung toi")]
struct NodeParams {
    #[serde(default)]
    text: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Text,
    label_vi: "Ghi chú".to_string(),
    inputs: vec![],
    outputs: vec![],
    require_any: vec![],
    credential: None,
    default_params: json!({ "text": "" }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "text": {
                "type": "string",
                "title": "Nội dung ghi chú",
                "format": "textarea"
            }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(NoteNode)
}

struct NoteNode;

#[async_trait]
impl NodeExecutor for NoteNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        _inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let _p: NodeParams = parse_params(cx.node_id, params)?;
        Ok(Preview::single(Outputs::new()))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        _inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let _p: NodeParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        Ok(NodeResult::ok(Outputs::new()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lvc_core::Limits;

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
    fn khong_cong_vao_ra() {
        let n = node();
        let m = n.manifest();
        assert!(m.inputs.is_empty());
        assert!(m.outputs.is_empty());
    }

    #[test]
    fn preview_khong_sinh_dau_ra() {
        let params = json!({ "text": "cách dùng: điền API key rồi bấm Chạy" })
            .as_object()
            .unwrap()
            .clone();
        let p = node()
            .preview(
                &PreviewCtx {
                    node_id: "n1",
                    limits: &Limits::default(),
                },
                &params,
                &Inputs::new(),
            )
            .unwrap();
        assert!(p.outputs.is_empty());
    }

    #[tokio::test]
    async fn run_khong_lam_gi_luon_thanh_cong() {
        let params = json!({ "text": "ghi chú" }).as_object().unwrap().clone();
        let ctx = NodeCtx {
            run_id: "r".into(),
            workflow_id: "w".into(),
            node_id: "n1".into(),
            cancel: tokio_util::sync::CancellationToken::new(),
            limits: Arc::new(Limits::default()),
            services: lvc_node_api::Services::new(),
        };
        let r = node().run(&ctx, &params, Inputs::new()).await.unwrap();
        assert!(!r.is_failed());
        assert!(r.outputs.is_empty());
    }
}
