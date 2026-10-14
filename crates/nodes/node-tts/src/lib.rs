//! Node `tts` — mỗi `TextItem` trên cổng `prompt` là MỘT job = MỘT file audio (map ngầm, giống
//! `museai_video`/`grok_image`). Gọi `ElevenLabs` hoặc `MiniMax` qua dịch vụ host
//! [`lvc_tts::TtsHost`] (`NodeCtx::services`) — node KHÔNG tự gọi mạng, KHÔNG giữ API key (chỉ
//! truyền TÊN credential trong mỗi job).

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, Counts, CredentialReq, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor,
    NodeManifest, NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value,
    meta, parse_params,
};
use lvc_tts::{Provider, TtsHost, TtsJob};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "tts";

/// Tham số node. `provider` chọn nhà cung cấp ("elevenlabs"|"minimax"); `credential` = tên (label)
/// API key trong kho — rỗng = lấy API key BẬT đầu tiên của provider đó (xem
/// `lvc_engine::textgen::VaultTextResolver`, dùng lại NGUYÊN VẸN cho mọi node gọi API chính thức).
#[derive(Debug, Clone, Deserialize)]
pub struct TtsParams {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub voice_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default)]
    pub credential: String,
}

fn default_provider() -> String {
    "elevenlabs".to_string()
}
fn default_speed() -> f32 {
    1.0
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Audio,
    label_vi: "Tạo giọng đọc AI (TTS)".to_string(),
    inputs: vec![PortSpec::new("prompt", PortType::Text).many().required()],
    outputs: vec![PortSpec::new("out", PortType::Audio).many()],
    require_any: vec![],
    // Hint tinh — provider that chon qua tham so `provider` (elevenlabs|minimax), giong cach
    // script_gen khai co dinh "gemini" du provider co the la "openai".
    credential: Some(CredentialReq {
        provider: "elevenlabs".to_string(),
        kinds: vec!["api_key".to_string()],
    }),
    default_params: json!({
        "provider": "elevenlabs", "voice_id": "", "model": "", "speed": 1.0, "credential": ""
    }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "provider": { "type": "string", "title": "Nhà cung cấp", "enum": ["elevenlabs", "minimax"] },
            "voice_id": { "type": "string", "title": "Id giọng đọc (bắt buộc)" },
            "model": { "type": "string", "title": "Model (rỗng = mặc định)" },
            "speed": { "type": "number", "title": "Tốc độ đọc (1.0 = mặc định)" },
            "credential": { "type": "string", "title": "Tên API key (rỗng = tự chọn)" }
        },
        "required": ["provider", "voice_id"]
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(TtsNode)
}

struct TtsNode;

/// Dựng danh sách [`TtsJob`] (THUẦN, không I/O) — mỗi `TextItem` trên cổng `prompt` là MỘT job,
/// `scene_index` 1..N liên tục. `provider` không parse được -> job rỗng (lỗi báo ở `validate`, xem
/// `preview`/`run`).
fn build_jobs(p: &TtsParams, inputs: &Inputs) -> Vec<TtsJob> {
    let Some(provider) = Provider::from_tag(&p.provider) else {
        return Vec::new();
    };
    inputs
        .texts("prompt")
        .enumerate()
        .map(|(i, t)| TtsJob {
            scene_index: u32::try_from(i + 1).unwrap_or(u32::MAX),
            credential_id: p.credential.clone(),
            provider,
            text: t.text.clone(),
            voice_id: p.voice_id.clone(),
            model: p.model.clone(),
            speed: p.speed,
        })
        .collect()
}

/// Kiểm tham số chung cho `preview`/`run`.
fn validate(node_id: &str, p: &TtsParams) -> Result<(), GraphError> {
    if Provider::from_tag(&p.provider).is_none() {
        return Err(GraphError::Param {
            node: node_id.to_string(),
            field: "provider".to_string(),
            message: format!("nhà cung cấp không hỗ trợ: {}", p.provider),
        });
    }
    if p.voice_id.trim().is_empty() {
        return Err(GraphError::Param {
            node: node_id.to_string(),
            field: "voice_id".to_string(),
            message: "thiếu id giọng đọc".to_string(),
        });
    }
    Ok(())
}

#[async_trait]
impl NodeExecutor for TtsNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: TtsParams = parse_params(cx.node_id, params)?;
        validate(cx.node_id, &p).map_err(|e| vec![e])?;
        let jobs = build_jobs(&p, inputs);
        // Đầu ra giữ chỗ (chưa có file) — node sau biết trước số audio/`index`.
        let placeholders = jobs
            .iter()
            .map(|j| {
                Value::Audio(
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
        let p: TtsParams =
            parse_params(node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        validate(node_id, &p).map_err(|e| NodeError::new(e.to_string()))?;
        let jobs = build_jobs(&p, &inputs);

        let host = ctx
            .services
            .get::<Arc<dyn TtsHost>>()
            .ok_or_else(|| NodeError::fatal(format!("node {node_id}: host khong cap TtsHost")))?;

        let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        let report = host.generate_audios(&ctx.run_id, node_id, jobs).await;

        let ok = u32::try_from(report.done.len()).unwrap_or(u32::MAX);
        let failed = u32::try_from(report.failed.len()).unwrap_or(u32::MAX);
        let audios = report
            .done
            .into_iter()
            .map(|d| {
                Value::Audio(
                    Asset::new(d.local_path)
                        .with(meta::NODE, node_id)
                        .with(meta::INDEX, d.scene_index),
                )
            })
            .collect();
        Ok(NodeResult {
            outputs: Outputs::from([("out".to_string(), audios)]),
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

    fn params(provider: &str, voice_id: &str) -> Params {
        json!({ "provider": provider, "voice_id": voice_id, "credential": "khoa-1" })
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn build_jobs_map_dung_tung_prompt_thanh_mot_job() {
        use lvc_node_api::{EdgeInput, TextItem};
        let p = TtsParams {
            provider: "elevenlabs".to_string(),
            voice_id: "v1".to_string(),
            model: String::new(),
            speed: 1.0,
            credential: "tk-1".to_string(),
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
        assert_eq!(jobs[0].provider, Provider::ElevenLabs);
        assert_eq!(jobs[0].text, "canh 1");
        assert_eq!(jobs[0].voice_id, "v1");
    }

    #[test]
    fn build_jobs_provider_sai_tra_rong() {
        let p = TtsParams {
            provider: "khac".to_string(),
            voice_id: "v1".to_string(),
            model: String::new(),
            speed: 1.0,
            credential: String::new(),
        };
        assert!(build_jobs(&p, &Inputs::new()).is_empty());
    }

    #[test]
    fn preview_provider_sai_bao_loi() {
        let p = params("khac", "v1");
        let err = node()
            .preview(
                &PreviewCtx {
                    node_id: "n1",
                    limits: &lvc_core::Limits::default(),
                },
                &p,
                &Inputs::new(),
            )
            .unwrap_err();
        assert_eq!(err.len(), 1);
    }

    #[test]
    fn preview_voice_id_rong_bao_loi() {
        let p = params("elevenlabs", "");
        let err = node()
            .preview(
                &PreviewCtx {
                    node_id: "n1",
                    limits: &lvc_core::Limits::default(),
                },
                &p,
                &Inputs::new(),
            )
            .unwrap_err();
        assert_eq!(err.len(), 1);
    }

    #[test]
    fn preview_thuan_tra_dung_so_job_giu_cho() {
        use lvc_node_api::{EdgeInput, TextItem};
        let p = params("elevenlabs", "v1");
        let mut inputs = Inputs::new();
        inputs.push(
            "prompt",
            EdgeInput {
                from_node: "pr".into(),
                from_port: "out".into(),
                values: Some(vec![
                    Value::Text(TextItem {
                        text: "a".into(),
                        ..TextItem::default()
                    }),
                    Value::Text(TextItem {
                        text: "b".into(),
                        ..TextItem::default()
                    }),
                ]),
            },
        );
        let prev = node()
            .preview(
                &PreviewCtx {
                    node_id: "n1",
                    limits: &lvc_core::Limits::default(),
                },
                &p,
                &inputs,
            )
            .unwrap();
        assert_eq!(prev.outputs["out"].len(), 2);
        assert_eq!(prev.jobs, 2);
    }

    #[tokio::test]
    async fn run_khong_co_host_bao_loi_fatal() {
        use lvc_core::Limits;
        use lvc_node_api::Services;
        let ctx = NodeCtx {
            run_id: "r".into(),
            workflow_id: "w".into(),
            node_id: "n1".into(),
            cancel: tokio_util::sync::CancellationToken::new(),
            limits: Arc::new(Limits::default()),
            services: Services::new(),
        };
        let err = node()
            .run(&ctx, &params("elevenlabs", "v1"), Inputs::new())
            .await
            .unwrap_err();
        assert!(err.fatal, "thieu TtsHost phai la loi fatal");
    }

    #[tokio::test]
    async fn run_goi_dung_host_va_map_ket_qua() {
        use lvc_core::Limits;
        use lvc_node_api::{EdgeInput, Services, TextItem};
        use lvc_tts::TtsJobsReport;
        use std::sync::Mutex;

        struct FakeHost {
            seen: Mutex<Option<Vec<TtsJob>>>,
        }
        #[async_trait]
        impl TtsHost for FakeHost {
            async fn generate_audios(
                &self,
                _run_id: &str,
                _node_id: &str,
                jobs: Vec<TtsJob>,
            ) -> TtsJobsReport {
                *self.seen.lock().unwrap() = Some(jobs.clone());
                TtsJobsReport {
                    done: jobs
                        .iter()
                        .map(|j| lvc_tts::TtsJobDone {
                            scene_index: j.scene_index,
                            local_path: format!("out_{}.mp3", j.scene_index).into(),
                        })
                        .collect(),
                    failed: Vec::new(),
                }
            }
        }

        let host = Arc::new(FakeHost {
            seen: Mutex::new(None),
        });
        let mut services = Services::new();
        services.insert::<Arc<dyn TtsHost>>(Arc::clone(&host) as Arc<dyn TtsHost>);
        let ctx = NodeCtx {
            run_id: "r".into(),
            workflow_id: "w".into(),
            node_id: "n1".into(),
            cancel: tokio_util::sync::CancellationToken::new(),
            limits: Arc::new(Limits::default()),
            services,
        };
        let mut inputs = Inputs::new();
        inputs.push(
            "prompt",
            EdgeInput {
                from_node: "pr".into(),
                from_port: "out".into(),
                values: Some(vec![Value::Text(TextItem {
                    text: "xin chao".into(),
                    ..TextItem::default()
                })]),
            },
        );
        let r = node()
            .run(&ctx, &params("elevenlabs", "v1"), inputs)
            .await
            .unwrap();
        assert!(!r.is_failed());
        assert_eq!(r.outputs["out"].len(), 1);
        let seen = host.seen.lock().unwrap().clone().unwrap();
        assert_eq!(seen[0].text, "xin chao");
        assert_eq!(seen[0].credential_id, "khoa-1");
    }
}
