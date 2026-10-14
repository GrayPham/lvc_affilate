//! Hợp đồng `build_jobs` cho node `seedance_video` — t2v/i2v cơ bản + override TXT theo dòng
//! (`ratio`→`aspect`, `refs`→ảnh gốc i2v, `account`→`credential`) thêm cho
//! P-affiliate-multi-engine (xem `jobs.rs`, `MEMORY.md`). Giống hệt cấu trúc
//! `node-google-video/tests/contract.rs` (2 node dùng chung khuôn `apimedia`).

use lvc_node_api::{Asset, EdgeInput, GraphError, Inputs, TextItem, Value};
use lvc_seedance::req::Aspect;

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

fn text_inputs_with_attrs(port: &str, lines: &[(&str, &[(&str, &str)])]) -> Inputs {
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
                    .map(|(k, (text, attrs))| {
                        Value::Text(TextItem {
                            text: (*text).into(),
                            source_node: "p".into(),
                            line: Some(k + 1),
                            attrs: attrs
                                .iter()
                                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                                .collect(),
                        })
                    })
                    .collect(),
            ),
        },
    );
    i
}

fn with_ref(mut inputs: Inputs, path: &str) -> Inputs {
    inputs.push(
        "ref",
        EdgeInput {
            from_node: "u".into(),
            from_port: "out".into(),
            values: Some(vec![Value::Image(Asset::new(path))]),
        },
    );
    inputs
}

fn params(mode: &str, aspect: &str, credential: &str) -> node_seedance_video::VideoParams {
    node_seedance_video::VideoParams {
        mode: mode.to_string(),
        model: String::new(),
        aspect: aspect.to_string(),
        credential: credential.to_string(),
    }
}

#[test]
fn t2v_mot_dong_prompt_ra_mot_job() {
    let inputs = text_inputs("prompt", &["canh 1", "canh 2"]);
    let p = params("t2v", "landscape", "key1");
    let jobs = node_seedance_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].credential_name, "key1");
    assert_eq!(jobs[0].aspect, Aspect::from_tag("landscape").unwrap());
    assert!(jobs[0].ref_image_path.is_none());
}

#[test]
fn i2v_dung_anh_dau_tien_cong_ref_cho_moi_job() {
    let inputs = with_ref(text_inputs("prompt", &["canh 1", "canh 2"]), "a.jpg");
    let p = params("i2v", "portrait", "key1");
    let jobs = node_seedance_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(
        jobs[0].ref_image_path,
        Some(std::path::PathBuf::from("a.jpg"))
    );
    assert_eq!(
        jobs[1].ref_image_path,
        Some(std::path::PathBuf::from("a.jpg"))
    );
}

#[test]
fn i2v_khong_co_anh_nao_bao_loi_param_ref() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = params("i2v", "portrait", "key1");
    let err = node_seedance_video::build_jobs("vid", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "vid" && field == "ref"
    ));
}

#[test]
fn credential_rong_bao_loi_param_credential() {
    let inputs = text_inputs("prompt", &["canh 1"]);
    let p = params("t2v", "landscape", "");
    let err = node_seedance_video::build_jobs("vid", &p, &inputs).unwrap_err();
    assert!(matches!(
        &err[0],
        GraphError::Param { node, field, .. } if node == "vid" && field == "credential"
    ));
}

#[test]
fn txt_ratio_doi_aspect_dong_do_khong_doi_cac_dong_khac() {
    let inputs = text_inputs_with_attrs(
        "prompt",
        &[("canh 1", &[("ratio", "1:1")][..]), ("canh 2", &[][..])],
    );
    let p = params("t2v", "landscape", "key1");
    let jobs = node_seedance_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(jobs[0].aspect, Aspect::from_tag("square").unwrap());
    assert_eq!(jobs[1].aspect, Aspect::from_tag("landscape").unwrap());
}

#[test]
fn txt_account_ghi_de_credential_dong_khong_co_van_dung_tinh() {
    let inputs = text_inputs_with_attrs(
        "prompt",
        &[
            ("canh 1", &[("account", "key_rieng")][..]),
            ("canh 2", &[][..]),
        ],
    );
    let p = params("t2v", "landscape", "key_tinh");
    let jobs = node_seedance_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(jobs[0].credential_name, "key_rieng");
    assert_eq!(jobs[1].credential_name, "key_tinh");
}

#[test]
fn txt_refs_ghi_de_anh_goc_i2v_chi_dong_do() {
    let inputs = with_ref(
        text_inputs_with_attrs(
            "prompt",
            &[
                ("canh 1", &[("refs", "rieng.jpg")][..]),
                ("canh 2", &[][..]),
            ],
        ),
        "chung.jpg",
    );
    let p = params("i2v", "portrait", "key1");
    let jobs = node_seedance_video::build_jobs("vid", &p, &inputs).unwrap();
    assert_eq!(
        jobs[0].ref_image_path,
        Some(std::path::PathBuf::from("rieng.jpg"))
    );
    assert_eq!(
        jobs[1].ref_image_path,
        Some(std::path::PathBuf::from("chung.jpg")),
        "dong khong override dung anh chung cua cong 'ref'"
    );
}
