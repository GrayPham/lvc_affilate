//! Hợp đồng node `veo3_image` — `build_jobs` (map ngầm, refs chung, lỗi model/aspect/account/scene),
//! `preview` (thuần) và `run` với `VeoImageHost` giả (không cần Chrome/mạng thật). Test ghim
//! `manifest.json` nằm trong `src/lib.rs` (theo đúng khuôn `node-dola-video`/`node-veo3-video`).

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, EdgeInput, GraphError, Inputs, NodeCtx, Params, PreviewCtx, Services, TextItem, Value,
};
use lvc_veo3::VeoImageHost;
use lvc_veo3::job::{ImageJob, ImageJobDone, ImageJobsReport};
use lvc_veo3::model::{Aspect, ImageModel};
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn params(model: &str, aspect: &str, seed: Option<i64>, account: &str) -> Params {
    let v = json!({ "model": model, "aspect": aspect, "seed": seed, "account": account });
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
        node_id: "img".into(),
        cancel: CancellationToken::new(),
        limits: Arc::new(lvc_core::Limits::default()),
        services,
    }
}

fn image_params(
    model: &str,
    aspect: &str,
    seed: Option<i64>,
    account: &str,
) -> node_veo3_image::ImageParams {
    node_veo3_image::ImageParams {
        model: model.into(),
        aspect: aspect.into(),
        seed,
        account: account.into(),
    }
}

// ---- build_jobs (thuần) ----

#[test]
fn mot_dong_prompt_ra_mot_job_anh() {
    let inputs = text_inputs("prompt", &["canh 1", "canh 2"]);
    let p = image_params("GEM_PIX_2", "portrait", None, "acc1");
    let jobs = node_veo3_image::build_jobs("img", &p, &inputs).unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].scene_index, 1);
    assert_eq!(jobs[1].scene_index, 2);
    assert_eq!(jobs[0].prompt, "canh 1");
    assert_eq!(jobs[1].prompt, "canh 2");
    assert_eq!(jobs[0].credential_id, "acc1");
    assert_eq!(jobs[0].model, ImageModel::GemPix2);
    assert_eq!(jobs[0].aspect, Aspect::Portrait);
    assert_eq!(jobs[0].seed, 0, "seed rong -> 0");
}

#[test]
fn seed_chi_dinh_thang_mac_dinh_0() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = image_params("GEM_PIX_2", "portrait", Some(42), "acc1");
    let jobs = node_veo3_image::build_jobs("img", &p, &inputs).unwrap();
    assert_eq!(jobs[0].seed, 42);
}

#[test]
fn refs_dung_chung_cho_moi_job() {
    let inputs = with_refs(
        text_inputs("prompt", &["canh 1", "canh 2"]),
        &["a.jpg", "b.jpg"],
    );
    let p = image_params("HARBOR_SEAL", "landscape", None, "acc1");
    let jobs = node_veo3_image::build_jobs("img", &p, &inputs).unwrap();
    assert_eq!(jobs.len(), 2);
    for j in &jobs {
        assert_eq!(
            j.ref_image_paths,
            [
                std::path::PathBuf::from("a.jpg"),
                std::path::PathBuf::from("b.jpg")
            ]
        );
        assert_eq!(j.model, ImageModel::HarborSeal);
        assert_eq!(j.aspect, Aspect::Landscape);
    }
}

#[test]
fn model_khong_hop_le_bao_loi_param_model() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = image_params("khong-ton-tai", "portrait", None, "acc1");
    let err = node_veo3_image::build_jobs("img", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "img" && field == "model"
    ));
}

#[test]
fn aspect_khong_hop_le_bao_loi_param_aspect() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = image_params("GEM_PIX_2", "vuong", None, "acc1");
    let err = node_veo3_image::build_jobs("img", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "img" && field == "aspect"
    ));
}

#[test]
fn account_rong_bao_loi_param_account() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = image_params("GEM_PIX_2", "portrait", None, "");
    let err = node_veo3_image::build_jobs("img", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "img" && field == "account"
    ));
}

#[test]
fn khong_co_prompt_bao_loi_scene() {
    let inputs = Inputs::new();
    let p = image_params("GEM_PIX_2", "portrait", None, "acc1");
    let err = node_veo3_image::build_jobs("img", &p, &inputs).unwrap_err();
    assert!(matches!(&err[0], GraphError::Scene { node, .. } if node == "img"));
}

// ---- preview (thuần) ----

#[test]
fn preview_tra_placeholder_anh_theo_so_job() {
    let node = node_veo3_image::node();
    let limits = lvc_core::Limits::default();
    let cx = PreviewCtx {
        node_id: "img",
        limits: &limits,
    };
    let inputs = text_inputs("prompt", &["a", "b", "c"]);
    let pv = node
        .preview(&cx, &params("GEM_PIX_2", "portrait", None, "acc1"), &inputs)
        .unwrap();
    assert_eq!(pv.jobs, 3);
    assert_eq!(pv.outputs["out"].len(), 3);
    assert!(matches!(pv.outputs["out"][0], Value::Image(_)));
}

#[test]
fn preview_aspect_la_tra_loi_param() {
    let node = node_veo3_image::node();
    let limits = lvc_core::Limits::default();
    let cx = PreviewCtx {
        node_id: "img",
        limits: &limits,
    };
    let inputs = text_inputs("prompt", &["a"]);
    let errs = node
        .preview(
            &cx,
            &params("GEM_PIX_2", "khong-hop-le", None, "acc1"),
            &inputs,
        )
        .unwrap_err();
    assert!(matches!(&errs[0], GraphError::Param { field, .. } if field == "aspect"));
}

// ---- run() với VeoImageHost giả ----

struct FakeHost {
    seen: Mutex<Vec<ImageJob>>,
}

#[async_trait]
impl VeoImageHost for FakeHost {
    async fn generate_images(
        &self,
        _run_id: &str,
        _node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport {
        let mut r = ImageJobsReport::default();
        for j in &jobs {
            if j.prompt == "hong" {
                r.failed.push(format!("canh {}: loi gia", j.scene_index));
            } else {
                r.done.push(ImageJobDone {
                    scene_index: j.scene_index,
                    local_path: format!("i{}.png", j.scene_index).into(),
                    width: Some(1024),
                    height: Some(768),
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
    services.insert::<Arc<dyn VeoImageHost>>(host.clone());
    let c = ctx(services);

    let r = node_veo3_image::node()
        .run(
            &c,
            &params("GEM_PIX_2", "portrait", None, "acc1"),
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
    let Value::Image(a) = &r.outputs["out"][0] else {
        panic!("dau ra phai la Value::Image")
    };
    assert_eq!(a.get("width"), Some("1024"));
    assert_eq!(a.get("height"), Some("768"));
}

#[tokio::test]
async fn run_refs_chuyen_toi_host() {
    let host = Arc::new(FakeHost {
        seen: Mutex::new(vec![]),
    });
    let mut services = Services::new();
    services.insert::<Arc<dyn VeoImageHost>>(host.clone());
    let c = ctx(services);

    let inputs = with_refs(text_inputs("prompt", &["canh 1"]), &["a.jpg", "b.jpg"]);
    let r = node_veo3_image::node()
        .run(&c, &params("GEM_PIX_2", "portrait", None, "acc1"), inputs)
        .await
        .unwrap();

    assert_eq!(r.counts.ok, 1);
    let seen = host.seen.lock().unwrap();
    assert_eq!(seen[0].ref_image_paths.len(), 2);
}

#[tokio::test]
async fn run_thieu_host_la_loi_nghiem_trong() {
    let c = ctx(Services::new());
    let e = node_veo3_image::node()
        .run(
            &c,
            &params("GEM_PIX_2", "portrait", None, "acc1"),
            text_inputs("prompt", &["a"]),
        )
        .await
        .unwrap_err();
    assert!(e.fatal);
}

#[tokio::test]
async fn run_tham_so_sai_la_loi_khong_nghiem_trong() {
    let c = ctx(Services::new());
    let e = node_veo3_image::node()
        .run(
            &c,
            &params("GEM_PIX_2", "portrait", None, ""),
            text_inputs("prompt", &["a"]),
        )
        .await
        .unwrap_err();
    assert!(!e.fatal);
}
