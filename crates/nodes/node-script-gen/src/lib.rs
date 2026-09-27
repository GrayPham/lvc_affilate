//! Node `script_gen` — sinh KỊCH BẢN nhiều phân cảnh bằng AI text (Gemini/`OpenAI`).
//!
//! Đầu vào `brief` (Text, tuỳ chọn — mô tả sản phẩm/chủ đề; rỗng thì dùng tham số `topic`). Đầu ra
//! `out` (Text, nhiều dòng): MỖI DÒNG = MỘT CẢNH, cùng định dạng node `prompt` để nối thẳng vào cổng
//! `prompt` của node `dola_video`.
//!
//! Node KHÔNG giữ bí mật: chỉ truyền TÊN credential + provider trong [`TextGenReq`]. Dịch vụ thật là
//! [`lvc_genai::TextGenHost`] lấy qua `NodeCtx::services` — engine impl (resolve API key từ vault rồi
//! gọi API). Node không tự gọi mạng.

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_genai::{Provider, TextGenHost, TextGenReq};
use lvc_node_api::{
    Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, TextItem,
    Value, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "script_gen";

/// Tham số node.
#[derive(Debug, Clone, Deserialize)]
pub struct ScriptParams {
    /// Chủ đề/brief nhập tại node (thắng cổng `brief` nếu khác rỗng).
    #[serde(default)]
    pub topic: String,
    #[serde(default = "default_num_scenes")]
    pub num_scenes: u32,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub style: String,
    /// Chỉ dẫn hệ thống tuỳ chọn (rỗng = mặc định của provider).
    #[serde(default)]
    pub system: String,
    /// Tên (label) API key trong kho.
    #[serde(default)]
    pub credential: String,
    /// `"gemini"` | `"openai"`.
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_num_scenes() -> u32 {
    3
}
fn default_language() -> String {
    "vi".to_string()
}
fn default_provider() -> String {
    "gemini".to_string()
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Text,
    label_vi: "Kịch bản AI".to_string(),
    inputs: vec![PortSpec::new("brief", PortType::Text).many()],
    outputs: vec![PortSpec::new("out", PortType::Text).many()],
    require_any: vec![],
    // Cần API key sinh text. Cả gemini lẫn openai đều dùng kind `api_key`; provider chọn ở tham số
    // `provider` (resolver của engine dùng tham số đó để tra đúng nhà cung cấp).
    credential: Some(CredentialReq {
        provider: "gemini".to_string(),
        kinds: vec!["api_key".to_string()],
    }),
    default_params: json!({
        "topic": "", "num_scenes": 3, "language": "vi", "style": "",
        "system": "", "credential": "", "provider": "gemini"
    }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "topic": { "type": "string", "title": "Chủ đề / brief", "format": "textarea" },
            "num_scenes": { "type": "integer", "title": "Số phân cảnh", "minimum": 1 },
            "language": { "type": "string", "title": "Ngôn ngữ" },
            "style": { "type": "string", "title": "Phong cách" },
            "system": { "type": "string", "title": "Chỉ dẫn hệ thống (tuỳ chọn)", "format": "textarea" },
            "credential": { "type": "string", "title": "Tên API key" },
            "provider": { "type": "string", "title": "Nhà cung cấp", "enum": ["gemini", "openai"] }
        },
        "required": ["num_scenes", "provider"]
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(ScriptGenNode)
}

struct ScriptGenNode;

/// Chủ đề hiệu lực: tham số `topic` thắng; rỗng thì gộp các dòng từ cổng `brief`.
fn effective_topic(topic: &str, inputs: &Inputs) -> String {
    if !topic.trim().is_empty() {
        return topic.trim().to_string();
    }
    inputs
        .texts("brief")
        .map(|t| t.text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `Vec<String>` cảnh -> `Outputs` cổng `out` (mỗi cảnh một `TextItem`, `line` 1-based).
fn to_outputs(node_id: &str, scenes: &[String]) -> Outputs {
    let values = scenes
        .iter()
        .enumerate()
        .map(|(i, text)| {
            Value::Text(TextItem {
                text: text.clone(),
                source_node: node_id.to_string(),
                line: Some(i + 1),
                attrs: std::collections::BTreeMap::new(),
            })
        })
        .collect();
    Outputs::from([("out".to_string(), values)])
}

/// Kiểm tham số chung cho `preview`/`run`. Trả provider đã parse.
fn validate(node_id: &str, p: &ScriptParams) -> Result<Provider, GraphError> {
    if p.num_scenes == 0 {
        return Err(GraphError::Param {
            node: node_id.to_string(),
            field: "num_scenes".to_string(),
            message: "số phân cảnh phải >= 1".to_string(),
        });
    }
    Provider::from_tag(&p.provider).ok_or_else(|| GraphError::Param {
        node: node_id.to_string(),
        field: "provider".to_string(),
        message: format!("nhà cung cấp không hỗ trợ: {}", p.provider),
    })
}

#[async_trait]
impl NodeExecutor for ScriptGenNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        _inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: ScriptParams = parse_params(cx.node_id, params)?;
        validate(cx.node_id, &p).map_err(|e| vec![e])?;
        // Đầu ra giữ chỗ: `num_scenes` cảnh có prompt KHÔNG rỗng (để node sau — vd dola_video —
        // dựng được kế hoạch/đếm job lúc validate). Nội dung thật do AI sinh khi `run`.
        let scenes: Vec<String> = (1..=p.num_scenes)
            .map(|i| format!("(cảnh {i} — AI sẽ sinh khi chạy)"))
            .collect();
        Ok(Preview::single(to_outputs(cx.node_id, &scenes)))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let node_id = &ctx.node_id;
        let p: ScriptParams =
            parse_params(node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let provider = validate(node_id, &p).map_err(|e| NodeError::new(e.to_string()))?;

        let topic = effective_topic(&p.topic, &inputs);
        if topic.is_empty() {
            return Err(NodeError::new(format!(
                "node {node_id}: thiếu chủ đề (nhập tham số hoặc nối cổng brief)"
            )));
        }

        let host = ctx.services.get::<Arc<dyn TextGenHost>>().ok_or_else(|| {
            NodeError::fatal(format!("node {node_id}: host khong cap TextGenHost"))
        })?;

        let req = TextGenReq {
            credential_name: p.credential.clone(),
            provider,
            system: p.system.clone(),
            prompt: topic,
            num_scenes: p.num_scenes,
            language: p.language.clone(),
            style: p.style.clone(),
        };
        // Lỗi API là lỗi của MỘT node (không fatal) — không dừng cả run.
        let text = host
            .generate(req)
            .await
            .map_err(|e| NodeError::new(format!("node {node_id}: {e}")))?;

        let scenes = lvc_genai::split_scenes(&text);
        if scenes.is_empty() {
            return Err(NodeError::new(format!(
                "node {node_id}: AI không trả về phân cảnh nào"
            )));
        }
        let n = u32::try_from(scenes.len()).unwrap_or(u32::MAX);
        Ok(NodeResult {
            outputs: to_outputs(node_id, &scenes),
            counts: Counts {
                done: n,
                total: n,
                ok: n,
                failed: 0,
            },
            failure: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use lvc_core::Limits;
    use lvc_genai::{TextGenError, TextGenReq};
    use lvc_node_api::{EdgeInput, Services};

    /// Host GIẢ: trả chuỗi cố định (KHÔNG gọi API thật), ghi lại `TextGenReq` cuối để kiểm.
    struct FakeHost {
        reply: Result<String, TextGenError>,
        seen: Mutex<Option<TextGenReq>>,
    }

    #[async_trait]
    impl TextGenHost for FakeHost {
        async fn generate(&self, req: TextGenReq) -> Result<String, TextGenError> {
            *self.seen.lock().unwrap() = Some(req);
            self.reply.clone()
        }
    }

    fn params(topic: &str, provider: &str, n: u32) -> Params {
        json!({
            "topic": topic, "num_scenes": n, "language": "vi", "style": "",
            "system": "", "credential": "khoa-1", "provider": provider
        })
        .as_object()
        .unwrap()
        .clone()
    }

    fn ctx_with(host: Arc<FakeHost>) -> NodeCtx {
        let mut services = Services::new();
        services.insert::<Arc<dyn TextGenHost>>(host);
        NodeCtx {
            run_id: "r".into(),
            workflow_id: "w".into(),
            node_id: "n1".into(),
            cancel: tokio_util::sync::CancellationToken::new(),
            limits: Arc::new(Limits::default()),
            services,
        }
    }

    #[test]
    fn preview_tra_dung_so_canh_giu_cho() {
        let p = params("giày", "gemini", 4);
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
        assert_eq!(prev.outputs["out"].len(), 4);
        assert_eq!(prev.jobs, 1);
    }

    #[test]
    fn preview_provider_sai_bao_loi() {
        let p = params("x", "khac", 3);
        let err = node()
            .preview(
                &PreviewCtx {
                    node_id: "n1",
                    limits: &Limits::default(),
                },
                &p,
                &Inputs::new(),
            )
            .unwrap_err();
        assert_eq!(err.len(), 1);
    }

    #[tokio::test]
    async fn run_tach_dong_thanh_tung_canh() {
        let host = Arc::new(FakeHost {
            reply: Ok("1. Cảnh mở đầu\n2. Nhân vật hành động\n\n3. Cao trào".to_string()),
            seen: Mutex::new(None),
        });
        let ctx = ctx_with(Arc::clone(&host));
        let r = node()
            .run(&ctx, &params("giày chạy bộ", "gemini", 3), Inputs::new())
            .await
            .unwrap();
        let out = &r.outputs["out"];
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].as_text().unwrap().text, "Cảnh mở đầu");
        assert_eq!(out[2].as_text().unwrap().text, "Cao trào");
        assert_eq!(out[0].as_text().unwrap().source_node, "n1");
        assert!(!r.is_failed());
        // credential + provider truyền đúng; KHÔNG có bí mật trong req.
        let seen = host.seen.lock().unwrap().clone().unwrap();
        assert_eq!(seen.credential_name, "khoa-1");
        assert_eq!(seen.provider, Provider::Gemini);
        assert_eq!(seen.prompt, "giày chạy bộ");
    }

    #[tokio::test]
    async fn run_lay_brief_tu_cong_khi_topic_rong() {
        let host = Arc::new(FakeHost {
            reply: Ok("canh 1".to_string()),
            seen: Mutex::new(None),
        });
        let ctx = ctx_with(Arc::clone(&host));
        let mut inputs = Inputs::new();
        inputs.push(
            "brief",
            EdgeInput {
                from_node: "p".into(),
                from_port: "out".into(),
                values: Some(vec![Value::Text(TextItem {
                    text: "áo thun mùa hè".into(),
                    ..TextItem::default()
                })]),
            },
        );
        node()
            .run(&ctx, &params("", "openai", 2), inputs)
            .await
            .unwrap();
        let seen = host.seen.lock().unwrap().clone().unwrap();
        assert_eq!(seen.prompt, "áo thun mùa hè");
        assert_eq!(seen.provider, Provider::OpenAi);
    }

    #[tokio::test]
    async fn run_thieu_chu_de_bao_loi_khong_fatal() {
        let host = Arc::new(FakeHost {
            reply: Ok("x".to_string()),
            seen: Mutex::new(None),
        });
        let ctx = ctx_with(host);
        let err = node()
            .run(&ctx, &params("", "gemini", 3), Inputs::new())
            .await
            .unwrap_err();
        assert!(!err.fatal);
        assert!(err.message.contains("thiếu chủ đề"));
    }

    #[tokio::test]
    async fn run_loi_api_thanh_node_error() {
        let host = Arc::new(FakeHost {
            reply: Err(TextGenError::Auth(401)),
            seen: Mutex::new(None),
        });
        let ctx = ctx_with(host);
        let err = node()
            .run(&ctx, &params("x", "gemini", 3), Inputs::new())
            .await
            .unwrap_err();
        assert!(!err.fatal);
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
