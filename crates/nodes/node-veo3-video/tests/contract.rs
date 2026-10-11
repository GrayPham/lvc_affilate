//! Hợp đồng node `veo3_video` — `build_jobs` (t2v/r2v, lỗi mode lạ, lỗi quá nhiều refs), `preview`
//! (thuần), `run` với `VeoVideoHost` giả (không cần Chrome/mạng thật) và test ghim `manifest.json`.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, EdgeInput, GraphError, Inputs, NodeCtx, Params, PreviewCtx, Services, TextItem, Value,
};
use lvc_veo3::VeoVideoHost;
use lvc_veo3::job::{VideoJob, VideoJobDone, VideoJobsReport, VideoSource};
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn params(mode: &str, account: &str) -> Params {
    let v = json!({
        "mode": mode, "model": "", "aspect": "portrait", "seed_count": 1, "account": account
    });
    match v {
        serde_json::Value::Object(m) => m,
        _ => unreachable!(),
    }
}

fn text_inputs(port: &str, lines: &[&str]) -> Inputs {
    let mut i = Inputs::new();
    i.push(
        port,
        EdgeInput {
            from_node: "p".into(),
            from_port: "out".into(),
            values: Some(
                lines
                    .iter()
                    .enumerate()
                    .map(|(k, s)| {
                        Value::Text(TextItem {
                            text: (*s).into(),
                            source_node: "p".into(),
                            line: Some(k + 1),
                            ..TextItem::default()
                        })
                    })
                    .collect(),
            ),
        },
    );
    i
}

fn with_refs(mut inputs: Inputs, paths: &[&str]) -> Inputs {
    inputs.push(
        "refs",
        EdgeInput {
            from_node: "u".into(),
            from_port: "out".into(),
            values: Some(paths.iter().map(|p| Value::Image(Asset::new(*p))).collect()),
        },
    );
    inputs
}

fn ctx(services: Services) -> NodeCtx {
    NodeCtx {
        run_id: "r".into(),
        workflow_id: "w".into(),
        node_id: "vid".into(),
        cancel: CancellationToken::new(),
        limits: Arc::new(lvc_core::Limits::default()),
        services,
    }
}

// ---- build_jobs (thuần) ----

#[test]
fn t2v_mot_dong_prompt_ra_mot_job_text() {
    let inputs = text_inputs("prompt", &["canh 1", "canh 2"]);
    let p = node_veo3_video::VideoParams {
        mode: "t2v".into(),
        model: String::new(),
        aspect: "portrait".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let jobs = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].scene_index, 1);
    assert_eq!(jobs[1].scene_index, 2);
    assert_eq!(jobs[0].credential_id, "acc1");
    assert_eq!(jobs[0].model, "veo_3_1_t2v_lite_low_priority");
    match &jobs[0].source {
        VideoSource::Text { prompt } => assert_eq!(prompt, "canh 1"),
        VideoSource::Reference { .. } => panic!("t2v phai dung VideoSource::Text"),
    }
}

#[test]
fn t2v_bo_qua_refs_khong_loi() {
    let inputs = with_refs(text_inputs("prompt", &["canh 1"]), &["a.jpg", "b.jpg"]);
    let p = node_veo3_video::VideoParams {
        mode: "t2v".into(),
        model: String::new(),
        aspect: "portrait".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let jobs = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(jobs.len(), 1);
    match &jobs[0].source {
        VideoSource::Text { .. } => {}
        VideoSource::Reference { .. } => panic!("t2v khong duoc dung refs"),
    }
}

#[test]
fn r2v_dung_chung_refs_cho_moi_job_va_dung_model_mac_dinh_r2v() {
    let inputs = with_refs(text_inputs("prompt", &["canh 1", "canh 2"]), &["a.jpg"]);
    let p = node_veo3_video::VideoParams {
        mode: "r2v".into(),
        model: String::new(),
        aspect: "landscape".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let jobs = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].model, "abra_r2v_8s");
    for j in &jobs {
        match &j.source {
            VideoSource::Reference {
                prompt,
                ref_image_paths,
            } => {
                assert_eq!(ref_image_paths, &[std::path::PathBuf::from("a.jpg")]);
                assert!(!prompt.is_empty());
            }
            VideoSource::Text { .. } => panic!("r2v phai dung VideoSource::Reference"),
        }
    }
}

#[test]
fn r2v_qua_6_anh_bao_loi_param_refs() {
    let paths = [
        "1.jpg", "2.jpg", "3.jpg", "4.jpg", "5.jpg", "6.jpg", "7.jpg",
    ];
    let inputs = with_refs(text_inputs("prompt", &["canh 1"]), &paths);
    let p = node_veo3_video::VideoParams {
        mode: "r2v".into(),
        model: String::new(),
        aspect: "portrait".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let err = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "vid" && field == "refs"
    ));
}

#[test]
fn mode_la_bao_loi_param_mode() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = node_veo3_video::VideoParams {
        mode: "i2v".into(),
        model: String::new(),
        aspect: "portrait".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let err = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "vid" && field == "mode"
    ));
}

#[test]
fn aspect_la_bao_loi_param_aspect() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = node_veo3_video::VideoParams {
        mode: "t2v".into(),
        model: String::new(),
        aspect: "vuong".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let err = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "vid" && field == "aspect"
    ));
}

#[test]
fn account_rong_bao_loi_param_account() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = node_veo3_video::VideoParams {
        mode: "t2v".into(),
        model: String::new(),
        aspect: "portrait".into(),
        seed_count: 1,
        account: String::new(),
    };
    let err = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "vid" && field == "account"
    ));
}

#[test]
fn khong_co_prompt_bao_loi_scene() {
    let inputs = Inputs::new();
    let p = node_veo3_video::VideoParams {
        mode: "t2v".into(),
        model: String::new(),
        aspect: "portrait".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let err = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap_err();
    assert!(matches!(&err[0], GraphError::Scene { node, .. } if node == "vid"));
}

#[test]
fn model_chi_dinh_thang_mac_dinh() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = node_veo3_video::VideoParams {
        mode: "t2v".into(),
        model: "model-tuy-chon".into(),
        aspect: "portrait".into(),
        seed_count: 1,
        account: "acc1".into(),
    };
    let jobs = node_veo3_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(jobs[0].model, "model-tuy-chon");
}

// ---- preview (thuần) ----

#[test]
fn preview_tra_placeholder_video_theo_so_job() {
    let node = node_veo3_video::node();
    let limits = lvc_core::Limits::default();
    let cx = PreviewCtx {
        node_id: "vid",
        limits: &limits,
    };
    let inputs = text_inputs("prompt", &["a", "b", "c"]);
    let pv = node.preview(&cx, &params("t2v", "acc1"), &inputs).unwrap();
    assert_eq!(pv.jobs, 3);
    assert_eq!(pv.outputs["out"].len(), 3);
    assert!(matches!(pv.outputs["out"][0], Value::Video(_)));
}

#[test]
fn preview_mode_la_tra_loi_param() {
    let node = node_veo3_video::node();
    let limits = lvc_core::Limits::default();
    let cx = PreviewCtx {
        node_id: "vid",
        limits: &limits,
    };
    let inputs = text_inputs("prompt", &["a"]);
    let errs = node
        .preview(&cx, &params("khong-hop-le", "acc1"), &inputs)
        .unwrap_err();
    assert!(matches!(&errs[0], GraphError::Param { field, .. } if field == "mode"));
}

// ---- run() với VeoVideoHost giả ----

struct FakeHost {
    seen: Mutex<Vec<VideoJob>>,
}

#[async_trait]
impl VeoVideoHost for FakeHost {
    async fn generate_videos(
        &self,
        _run_id: &str,
        _node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport {
        let mut r = VideoJobsReport::default();
        for j in &jobs {
            let hong = match &j.source {
                VideoSource::Text { prompt } | VideoSource::Reference { prompt, .. } => {
                    prompt == "hong"
                }
            };
            if hong {
                r.failed.push(format!("canh {}: loi gia", j.scene_index));
            } else {
                r.done.push(VideoJobDone {
                    scene_index: j.scene_index,
                    local_path: format!("v{}.mp4", j.scene_index).into(),
                });
            }
        }
        self.seen.lock().unwrap().extend(jobs);
        r
    }
}

#[tokio::test]
async fn run_goi_host_va_bao_that_bai_mot_phan() {
    let host = Arc::new(FakeHost {
        seen: Mutex::new(vec![]),
    });
    let mut services = Services::new();
    services.insert::<Arc<dyn VeoVideoHost>>(host.clone());
    let c = ctx(services);

    let r = node_veo3_video::node()
        .run(
            &c,
            &params("t2v", "acc1"),
            text_inputs("prompt", &["a", "hong", "c"]),
        )
        .await
        .unwrap();

    assert_eq!(host.seen.lock().unwrap().len(), 3);
    assert_eq!(r.counts.ok, 2);
    assert_eq!(r.counts.failed, 1);
    assert_eq!(r.counts.total, 3);
    assert_eq!(r.failure.as_deref(), Some("canh 2: loi gia"));
    assert_eq!(r.outputs["out"].len(), 2);
}

#[tokio::test]
async fn run_r2v_chuyen_dung_refs_toi_host() {
    let host = Arc::new(FakeHost {
        seen: Mutex::new(vec![]),
    });
    let mut services = Services::new();
    services.insert::<Arc<dyn VeoVideoHost>>(host.clone());
    let c = ctx(services);

    let inputs = with_refs(text_inputs("prompt", &["canh 1"]), &["a.jpg", "b.jpg"]);
    let r = node_veo3_video::node()
        .run(&c, &params("r2v", "acc1"), inputs)
        .await
        .unwrap();

    assert_eq!(r.counts.ok, 1);
    let seen = host.seen.lock().unwrap();
    match &seen[0].source {
        VideoSource::Reference {
            ref_image_paths, ..
        } => assert_eq!(ref_image_paths.len(), 2),
        VideoSource::Text { .. } => panic!("r2v phai dung VideoSource::Reference"),
    }
}

#[tokio::test]
async fn run_thieu_host_la_loi_nghiem_trong() {
    let c = ctx(Services::new());
    let e = node_veo3_video::node()
        .run(&c, &params("t2v", "acc1"), text_inputs("prompt", &["a"]))
        .await
        .unwrap_err();
    assert!(e.fatal);
}

#[tokio::test]
async fn run_tham_so_sai_la_loi_khong_nghiem_trong() {
    let c = ctx(Services::new());
    let e = node_veo3_video::node()
        .run(
            &c,
            &params("mode-la", "acc1"),
            text_inputs("prompt", &["a"]),
        )
        .await
        .unwrap_err();
    assert!(!e.fatal);
}
