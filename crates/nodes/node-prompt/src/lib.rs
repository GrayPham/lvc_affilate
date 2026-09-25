//! Node `prompt` — văn bản TXT nhiều dòng → mỗi dòng (bỏ dòng trống/`#`) là một [`TextItem`] trên
//! cổng `out`. Override theo dòng (`| ratio=16:9 | dur=10 | refs=a.jpg ...`) đi kèm trong
//! `TextItem.attrs` cho node tiêu thụ tự dùng. Không I/O.

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_core::scene_txt::parse_lines_raw;
use lvc_core::{RawScene, parse_txt};
use lvc_node_api::{
    Category, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor, NodeManifest, NodeResult,
    Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, TextItem, Value, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "prompt";

/// Khoá chuẩn trong `TextItem.attrs` (dùng chung với node tiêu thụ như `dola_video`).
pub mod attrs {
    pub const RATIO: &str = "ratio";
    pub const DURATION: &str = "duration";
    pub const MODEL: &str = "model";
    pub const MODE: &str = "mode";
    pub const LOGO: &str = "logo";
    /// Nhiều đường dẫn, phân cách `\n`.
    pub const REFS: &str = "refs";
    pub const ACCOUNT: &str = "account";
}

#[derive(Debug, Deserialize)]
struct PromptParams {
    #[serde(default)]
    text: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Text,
    label_vi: "Prompt".to_string(),
    inputs: vec![],
    outputs: vec![PortSpec::new("out", PortType::Text).many()],
    require_any: vec![],
    credential: None,
    default_params: json!({ "text": "" }),
    params_schema: json!({
        "type": "object",
        "properties": { "text": { "type": "string", "title": "Nội dung (mỗi dòng 1 cảnh)", "format": "textarea" } }
    }),
});

/// Node đã sẵn sàng đăng ký.
#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(PromptNode)
}

struct PromptNode;

/// `RawScene` (một dòng TXT chưa chuẩn hoá) → `TextItem`.
fn to_item(source_node: &str, line: usize, raw: RawScene) -> TextItem {
    let mut a = BTreeMap::new();
    let mut put = |k: &str, v: Option<String>| {
        if let Some(v) = v {
            a.insert(k.to_string(), v);
        }
    };
    put(attrs::RATIO, raw.ratio);
    put(attrs::DURATION, raw.duration);
    put(attrs::MODEL, raw.model);
    put(attrs::MODE, raw.mode);
    put(attrs::LOGO, raw.logo);
    put(attrs::REFS, raw.reference_images.map(|r| r.join("\n")));
    put(attrs::ACCOUNT, raw.account);
    TextItem {
        text: raw.prompt.unwrap_or_default(),
        source_node: source_node.to_string(),
        line: Some(line),
        attrs: a,
    }
}

fn scene_error(node: &str, e: &lvc_core::SceneError) -> GraphError {
    GraphError::Scene {
        node: node.to_string(),
        line: e.line,
        message: e.to_string(),
    }
}

/// Parse TXT thành đầu ra. Kiểm bằng `parse_txt` (chuẩn hoá theo mặc định CHUNG — bắt lỗi cú pháp/
/// giá trị ngay tại node Prompt như P3) rồi xuất dòng thô (`parse_lines_raw`) để node tiêu thụ gộp
/// với mặc định của chính nó. Hai hàm dùng chung `parse_line` + cùng luật bỏ dòng → cùng số dòng.
fn items(node_id: &str, text: &str, limits: &lvc_core::Limits) -> Result<Outputs, Vec<GraphError>> {
    parse_txt(text, limits).map_err(|e| vec![scene_error(node_id, &e)])?;
    let lines = parse_lines_raw(text).map_err(|e| vec![scene_error(node_id, &e)])?;
    let values = lines
        .into_iter()
        .map(|(line, raw)| Value::Text(to_item(node_id, line, raw)))
        .collect();
    Ok(Outputs::from([("out".to_string(), values)]))
}

#[async_trait]
impl NodeExecutor for PromptNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        _inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: PromptParams = parse_params(cx.node_id, params)?;
        Ok(Preview::single(items(cx.node_id, &p.text, cx.limits)?))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        _inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let p: PromptParams = parse_params(&ctx.node_id, params)
            .map_err(|e| NodeError::new(format!("node {}: {}", ctx.node_id, e[0])))?;
        let outputs = items(&ctx.node_id, &p.text, &ctx.limits)
            .map_err(|e| NodeError::new(e[0].to_string()))?;
        Ok(NodeResult::ok(outputs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_core::Limits;

    fn preview(text: &str) -> Result<Preview, Vec<GraphError>> {
        let params = json!({ "text": text }).as_object().unwrap().clone();
        node().preview(
            &PreviewCtx {
                node_id: "n1",
                limits: &Limits::default(),
            },
            &params,
            &Inputs::new(),
        )
    }

    fn texts(p: &Preview) -> Vec<&TextItem> {
        p.outputs["out"].iter().filter_map(Value::as_text).collect()
    }

    #[test]
    fn ba_dong_ra_ba_text_item_kem_so_dong() {
        let p = preview("canh 1\ncanh 2\ncanh 3").unwrap();
        let t = texts(&p);
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].text, "canh 1");
        assert_eq!(t[2].line, Some(3));
        assert_eq!(t[0].source_node, "n1");
        assert_eq!(p.jobs, 1);
    }

    #[test]
    fn bo_dong_trong_va_comment() {
        let p = preview("canh that\n# ghi chu\n   \n").unwrap();
        assert_eq!(texts(&p).len(), 1);
    }

    #[test]
    fn override_theo_dong_vao_attrs() {
        let p = preview("canh 1 | ratio=16:9 | refs=a.jpg,b.jpg | acc=x").unwrap();
        let t = texts(&p)[0];
        assert_eq!(t.attrs[attrs::RATIO], "16:9");
        assert_eq!(t.attrs[attrs::REFS], "a.jpg\nb.jpg");
        assert_eq!(t.attrs[attrs::ACCOUNT], "x");
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

    #[tokio::test]
    async fn run_ra_dung_dau_ra_nhu_preview() {
        let params = json!({ "text": "a\nb" }).as_object().unwrap().clone();
        let ctx = NodeCtx {
            run_id: "r".into(),
            workflow_id: "w".into(),
            node_id: "n1".into(),
            cancel: tokio_util::sync::CancellationToken::new(),
            limits: Arc::new(Limits::default()),
            services: lvc_node_api::Services::new(),
        };
        let r = node().run(&ctx, &params, Inputs::new()).await.unwrap();
        assert_eq!(r.outputs["out"].len(), 2);
        assert!(!r.is_failed());
    }
}
