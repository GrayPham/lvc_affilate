//! Đồ thị đầy đủ Prompt → Tạo video Dola → Lưu qua `Registry::prepare` + chạy node với host giả.
//! Chuyển từ test `lvc_core::graph::plan` (P3) — cùng bất biến, giờ đi qua hợp đồng node chung.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lvc_core::graph::{Edge, Graph, Node, NodeData};
use lvc_core::{Limits, Ratio};
use lvc_dola::{DolaVideoHost, JobsReport, SceneJob, VideoDone};
use lvc_node_api::{
    GraphError, Inputs, NodeCtx, Registry, Services, TextItem, Value, gather_inputs,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn registry() -> Registry {
    Registry::new()
        .with(node_prompt::node())
        .with(node_image_upload::node())
        .with(node_save::node())
        .with(node_dola_video::node())
}

fn n(id: &str, kind: &str, params: serde_json::Value) -> Node {
    Node {
        id: id.into(),
        kind: NodeData::new(kind, params),
        x: 0.0,
        y: 0.0,
    }
}

fn prompt(id: &str, text: &str) -> Node {
    n(id, "prompt", json!({ "text": text }))
}

fn video(id: &str) -> Node {
    n(
        id,
        "dola_video",
        json!({"mode":"fast","model":"seedance-2.0","ratio":"9:16","duration":10,"logo":"off","account":""}),
    )
}

fn save(id: &str) -> Node {
    n(id, "save", json!({ "out_dir": "downloads" }))
}

fn image_upload(id: &str, paths: &[&str]) -> Node {
    n(id, "image_upload", json!({ "paths": paths }))
}

fn edge(id: &str, from: &str, fp: &str, to: &str, tp: &str) -> Edge {
    Edge {
        id: id.into(),
        from: from.into(),
        from_port: fp.into(),
        to: to.into(),
        to_port: tp.into(),
    }
}

fn fixture(text: &str) -> Graph {
    Graph {
        version: 2,
        nodes: vec![prompt("n1", text), video("n2"), save("n3")],
        edges: vec![
            edge("e1", "n1", "out", "n2", "prompt"),
            edge("e2", "n2", "out", "n3", "video"),
        ],
    }
}

/// Cảnh của node video `vid` sau khi preview mọi node thượng nguồn (chạy lại preview theo topo).
fn jobs_of(g: &Graph, vid: &str, limits: &Limits) -> Result<Vec<SceneJob>, Vec<GraphError>> {
    let reg = registry();
    let prepared = reg.prepare(g, limits)?;
    let mut outs = std::collections::HashMap::new();
    for id in &prepared.order {
        let node = g.nodes.iter().find(|x| &x.id == id).unwrap();
        let inputs = gather_inputs(g, id, &outs);
        if id == vid {
            let p = serde_json::from_value(serde_json::Value::Object(node.kind.params.clone()))
                .unwrap();
            return node_dola_video::build_jobs(vid, &p, &inputs, limits);
        }
        let pv = reg
            .get(&node.kind.kind)
            .unwrap()
            .preview(
                &lvc_node_api::PreviewCtx {
                    node_id: id,
                    limits,
                },
                &node.kind.params,
                &inputs,
            )
            .unwrap();
        outs.insert(id.clone(), pv.outputs);
    }
    panic!("khong thay node {vid}")
}

#[test]
fn prompt_3_dong_ra_3_job_danh_so_1_den_3() {
    let g = fixture("canh 1\ncanh 2\ncanh 3");
    let l = Limits::default();
    let jobs = jobs_of(&g, "n2", &l).unwrap();
    assert_eq!(
        jobs.iter().map(|j| j.scene_index).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(jobs[0].scene.prompt, "canh 1");
    assert_eq!(jobs[2].scene.prompt, "canh 3");
    let p = registry().prepare(&g, &l).unwrap();
    assert_eq!(p.video_total, 3);
    assert_eq!(p.jobs["n2"], 3);
    assert_eq!(p.jobs["n3"], 1);
}

#[test]
fn txt_ratio_thang_mac_dinh_node() {
    let mut g = fixture("canh 1 | ratio=16:9\ncanh 2");
    g.nodes[1].kind.params.insert("ratio".into(), json!("1:1"));
    let jobs = jobs_of(&g, "n2", &Limits::default()).unwrap();
    assert_eq!(jobs[0].scene.ratio, Ratio::R16x9, "TXT ratio=16:9 thang");
    assert_eq!(
        jobs[1].scene.ratio,
        Ratio::R1x1,
        "khong co TXT -> mac dinh node"
    );
}

#[test]
fn refs_tu_image_upload_gan_vao_moi_canh_va_gop_voi_txt_refs() {
    let mut g = fixture("canh 1\ncanh 2 | refs=p3.jpg,p1.jpg");
    g.nodes.push(image_upload("n4", &["p1.jpg", "p2.jpg"]));
    g.edges.push(edge("e3", "n4", "out", "n2", "refs"));
    let jobs = jobs_of(&g, "n2", &Limits::default()).unwrap();
    assert_eq!(jobs[0].scene.reference_images, ["p1.jpg", "p2.jpg"]);
    assert_eq!(
        jobs[1].scene.reference_images,
        ["p1.jpg", "p2.jpg", "p3.jpg"]
    );
}

#[test]
fn vuot_tran_anh_tham_chieu_bao_loi_too_many_refs() {
    let mut g = fixture("canh 1");
    g.nodes.push(image_upload("n4", &["p1.jpg", "p2.jpg"]));
    g.edges.push(edge("e3", "n4", "out", "n2", "refs"));
    let l = Limits {
        reference_image_max_count: 1,
        ..Limits::default()
    };
    let errs = registry().prepare(&g, &l).unwrap_err();
    assert!(errs.contains(&GraphError::TooManyRefs {
        node: "n2".into(),
        scene_index: 1,
        count: 2,
        max: 1,
    }));
}

#[test]
fn hai_node_prompt_vao_1_video_noi_chi_so_theo_thu_tu_canh() {
    let mut g = Graph {
        version: 2,
        nodes: vec![
            prompt("n1", "a\nb"),
            prompt("n1b", "c"),
            video("n2"),
            save("n3"),
        ],
        edges: vec![
            edge("e1", "n1", "out", "n2", "prompt"),
            edge("e1b", "n1b", "out", "n2", "prompt"),
            edge("e2", "n2", "out", "n3", "video"),
        ],
    };
    g.edges.sort_by_key(|e| e.id.clone());
    let jobs = jobs_of(&g, "n2", &Limits::default()).unwrap();
    assert_eq!(
        jobs.iter()
            .map(|j| j.scene.prompt.clone())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(
        jobs.iter().map(|j| j.scene_index).collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[test]
fn video_total_la_tong_job_qua_nhieu_node_video() {
    let g = Graph {
        version: 2,
        nodes: vec![
            prompt("n1", "a\nb"),
            video("n2"),
            prompt("n1b", "c"),
            video("n2b"),
            save("n3"),
        ],
        edges: vec![
            edge("e1", "n1", "out", "n2", "prompt"),
            edge("e1b", "n1b", "out", "n2b", "prompt"),
            edge("e2", "n2", "out", "n3", "video"),
            edge("e2b", "n2b", "out", "n3", "video"),
        ],
    };
    assert_eq!(
        registry()
            .prepare(&g, &Limits::default())
            .unwrap()
            .video_total,
        3
    );
}

#[test]
fn do_thi_khong_hop_le_tra_nguyen_loi_validate() {
    let g = Graph {
        version: 2,
        nodes: vec![],
        edges: vec![],
    };
    assert_eq!(
        registry().prepare(&g, &Limits::default()),
        Err(vec![GraphError::Empty])
    );
}

#[test]
fn prompt_rong_thi_loi_khong_co_canh_nao() {
    let g = fixture("   \n\n  ");
    let errs = registry().prepare(&g, &Limits::default()).unwrap_err();
    assert!(errs.contains(&GraphError::Scene {
        node: "n2".into(),
        line: None,
        message: "khong co canh nao".into(),
    }));
}

#[test]
fn prompt_va_image_upload_don_doc_van_hop_le_video_total_0() {
    let g = Graph {
        version: 2,
        nodes: vec![prompt("n1", "chi mot canh"), image_upload("n2", &["a.jpg"])],
        edges: vec![],
    };
    let p = registry().prepare(&g, &Limits::default()).unwrap();
    assert_eq!(p.video_total, 0);
    assert_eq!(p.order, ["n1", "n2"]);
}

#[test]
fn account_node_thang_neu_khac_rong() {
    let mut g = fixture("canh 1 | acc=tu_txt\ncanh 2 | acc=tu_txt");
    g.nodes[1]
        .kind
        .params
        .insert("account".into(), json!("tu_node"));
    let jobs = jobs_of(&g, "n2", &Limits::default()).unwrap();
    assert_eq!(jobs[0].scene.account, "tu_node");
    assert_eq!(jobs[1].scene.account, "tu_node");
}

#[test]
fn account_node_rong_thi_lay_txt_acc() {
    let g = fixture("canh 1 | acc=tu_txt\ncanh 2");
    let jobs = jobs_of(&g, "n2", &Limits::default()).unwrap();
    assert_eq!(jobs[0].scene.account, "tu_txt");
    assert_eq!(jobs[1].scene.account, "");
}

#[test]
fn vuot_tran_tong_so_canh_bao_loi_scene() {
    let l = Limits {
        batch_max_scenes: 2,
        ..Limits::default()
    };
    // 1 node Prompt tự vượt trần → lỗi nằm ở chính node Prompt (báo 1 lần, không kép).
    let errs = registry().prepare(&fixture("a\nb\nc"), &l).unwrap_err();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(matches!(&errs[0], GraphError::Scene { node, .. } if node == "n1"));

    // 2 node Prompt mỗi cái hợp lệ nhưng GỘP vượt trần → lỗi ở node video.
    let g = Graph {
        version: 2,
        nodes: vec![prompt("n1", "a\nb"), prompt("n1b", "c"), video("n2")],
        edges: vec![
            edge("e1", "n1", "out", "n2", "prompt"),
            edge("e1b", "n1b", "out", "n2", "prompt"),
        ],
    };
    let errs = registry().prepare(&g, &l).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, GraphError::Scene { node, .. } if node == "n2")),
        "{errs:?}"
    );
}

#[test]
fn dong_comment_va_dong_trong_khong_thanh_canh() {
    let jobs = jobs_of(
        &fixture("canh that\n# ghi chu\n   \n"),
        "n2",
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(jobs.len(), 1);
}

#[test]
fn loi_cu_phap_o_prompt_chi_bao_1_lan_tai_prompt() {
    let errs = registry()
        .prepare(&fixture("canh 1 | ratio=7:3"), &Limits::default())
        .unwrap_err();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(matches!(&errs[0], GraphError::Scene { node, .. } if node == "n1"));
}

#[test]
fn tham_so_sai_bao_loi_param() {
    let mut g = fixture("a");
    g.nodes[1]
        .kind
        .params
        .insert("model".into(), json!("khong-co"));
    let errs = registry().prepare(&g, &Limits::default()).unwrap_err();
    assert!(matches!(&errs[0], GraphError::Param { node, .. } if node == "n2"));
}

// ---- run() với host giả ----

struct FakeHost {
    seen: Mutex<Vec<SceneJob>>,
}

#[async_trait]
impl DolaVideoHost for FakeHost {
    async fn run_jobs(
        &self,
        _run_id: &str,
        _node_id: &str,
        jobs: Vec<SceneJob>,
        _cancel: &CancellationToken,
    ) -> JobsReport {
        let mut r = JobsReport::default();
        for j in &jobs {
            if j.scene.prompt == "hong" {
                r.failed.push(j.scene_index.to_string());
            } else {
                r.done.push(VideoDone {
                    scene_index: j.scene_index,
                    local_path: format!("v{}.mp4", j.scene_index).into(),
                    account: "acc".into(),
                    conversation_id: "c".into(),
                    watermarked: false,
                });
            }
        }
        self.seen.lock().unwrap().extend(jobs);
        r
    }
}

fn text_inputs(lines: &[&str]) -> Inputs {
    let mut i = Inputs::new();
    i.push(
        "prompt",
        lvc_node_api::EdgeInput {
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

#[tokio::test]
async fn run_goi_host_va_bao_that_bai_mot_phan() {
    let host = Arc::new(FakeHost {
        seen: Mutex::new(vec![]),
    });
    let mut services = Services::new();
    services.insert::<Arc<dyn DolaVideoHost>>(host.clone());
    let ctx = NodeCtx {
        run_id: "r".into(),
        workflow_id: "w".into(),
        node_id: "vid".into(),
        cancel: CancellationToken::new(),
        limits: Arc::new(Limits::default()),
        services,
    };
    let params = video("vid").kind.params;
    let r = node_dola_video::node()
        .run(&ctx, &params, text_inputs(&["a", "hong", "c"]))
        .await
        .unwrap();
    assert_eq!(host.seen.lock().unwrap().len(), 3);
    assert_eq!(r.counts.ok, 2);
    assert_eq!(r.counts.failed, 1);
    assert_eq!(r.counts.total, 3);
    assert_eq!(r.failure.as_deref(), Some("node vid: canh 2 that bai"));
    assert_eq!(r.outputs["out"].len(), 2);
}

#[tokio::test]
async fn run_thieu_host_la_loi_nghiem_trong() {
    let ctx = NodeCtx {
        run_id: "r".into(),
        workflow_id: "w".into(),
        node_id: "vid".into(),
        cancel: CancellationToken::new(),
        limits: Arc::new(Limits::default()),
        services: Services::new(),
    };
    let e = node_dola_video::node()
        .run(&ctx, &video("vid").kind.params, text_inputs(&["a"]))
        .await
        .unwrap_err();
    assert!(e.fatal);
}

#[test]
fn manifest_khop_file_manifest_json() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json");
    let got = serde_json::to_string_pretty(node_dola_video::node().manifest()).unwrap() + "\n";
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
