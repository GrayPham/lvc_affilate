//! Fixture JSON cho frontend (Task 11, `app/src/ipc/fixtures/*.json`) — TDD Task 9 Step 1. Ghi khi
//! `LVC_WRITE_FIXTURES=1` (chi 1 lan, thu cong: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app
//! commands::fixtures`), MAC DINH so sanh BANG voi file da commit (khong drift am tham). Moi gia
//! tri o day PHAI tat dinh (id/timestamp co dinh — KHONG `uuid::Uuid::new_v4()`/`now_unix()`, xem
//! ruling brief Task 9).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::scripts::{SceneDto, ScriptDoc};
use lvc_browser::driver::{POLL_STAGES, SUBMIT_STAGES};
use lvc_core::graph::{Edge, Graph, GraphError, Node, NodeData, PortType};
use lvc_core::{AppSettings, LogoMode, Mode, Model, Ratio};
use lvc_engine::events::EngineEvent;
use lvc_engine::generate::{STAGE_DONE, STAGE_LAUNCHING};
use lvc_engine::resume::RequeueReport;
use lvc_engine::{AccountStatus, ImportCookiesReport, LegacyImportReport};
use lvc_engine::{ScriptRunError, ScriptRunReport, ScriptRunTask};
use lvc_store::{NodeStateRow, RunRow, RunStatus, ScriptMeta, TaskRow, WorkflowMeta};
use serde_json::{Value, json};

use super::IpcError;
use super::tasks::TaskDto;
use super::workflow::PlanSummary;

fn fixtures_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/lvc-app; fixture cua frontend nam o app/src/ipc/fixtures (Task
    // 11 doc truc tiep, ghim boi test nay).
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src/ipc/fixtures")
}

/// Ghi/so sanh MOT fixture. `LVC_WRITE_FIXTURES=1` -> ghi (tao thu muc neu chua co); mac dinh ->
/// doc file da commit va so sanh BANG (fixture troi -> test do, chi RO RANG bao "chay lai voi
/// `LVC_WRITE_FIXTURES=1`" thay vi tu sua). JSON dep (`to_string_pretty`, LF san co trong
/// `serde_json` — khong dung `\r\n`) + xuong dong cuoi file.
fn check_or_write(name: &str, value: &Value) {
    let dir = fixtures_dir();
    let path = dir.join(name);
    let mut text = serde_json::to_string_pretty(value).expect("fixture phai serialize duoc");
    text.push('\n');

    if matches!(std::env::var("LVC_WRITE_FIXTURES").as_deref(), Ok("1")) {
        std::fs::create_dir_all(&dir).expect("tao thu muc fixtures");
        std::fs::write(&path, &text).unwrap_or_else(|e| panic!("ghi {path:?} that bai: {e}"));
        return;
    }

    let existing = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "khong doc duoc fixture da commit {path:?}: {e}\n\
             chay `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures` de tao/cap nhat, \
             roi commit file JSON sinh ra."
        )
    });
    assert_eq!(
        existing, text,
        "fixture {name} da troi khoi ban da commit — neu day la thay doi CO CHU DICH, chay \
         `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures` roi commit lai file JSON."
    );
}

// ---- gia tri tat dinh dung chung ----

fn video_kind() -> NodeData {
    NodeData::new(
        "dola_video",
        serde_json::json!({ "mode": Mode::Fast, "model": Model::Seedance20, "ratio": Ratio::R9x16, "duration": 10, "logo": LogoMode::Off, "account": String::new() }),
    )
}

/// Do thi Prompt(3 canh) -> Tao video -> Luu — dung o nhieu noi trong report Task 9 lam vi du.
fn three_scene_graph() -> Graph {
    Graph {
        version: 2,
        nodes: vec![
            Node {
                id: "n1".to_string(),
                kind: NodeData::new(
                    "prompt",
                    serde_json::json!({ "text": "canh 1\ncanh 2\ncanh 3".to_string() }),
                ),
                x: 0.0,
                y: 0.0,
            },
            Node {
                id: "n2".to_string(),
                kind: video_kind(),
                x: 300.0,
                y: 0.0,
            },
            Node {
                id: "n3".to_string(),
                kind: NodeData::new(
                    "save",
                    serde_json::json!({ "out_dir": "downloads".to_string() }),
                ),
                x: 600.0,
                y: 0.0,
            },
        ],
        edges: vec![
            Edge {
                id: "e1".to_string(),
                from: "n1".to_string(),
                from_port: "out".to_string(),
                to: "n2".to_string(),
                to_port: "prompt".to_string(),
            },
            Edge {
                id: "e2".to_string(),
                from: "n2".to_string(),
                from_port: "out".to_string(),
                to: "n3".to_string(),
                to_port: "video".to_string(),
            },
        ],
    }
}

fn graph_errors() -> Vec<GraphError> {
    vec![
        GraphError::UnknownNode {
            id: "n99".to_string(),
        },
        GraphError::MissingRequiredInput {
            node: "n2".to_string(),
            port: "prompt".to_string(),
        },
    ]
}

fn task_row_fixture() -> TaskRow {
    TaskRow {
        id: "task_1".to_string(),
        prompt: "con meo di bo tren duong lang".to_string(),
        ratio: "9:16".to_string(),
        duration: 10,
        model: "seedance-2.0".to_string(),
        mode: "fast".to_string(),
        logo: "off".to_string(),
        status: lvc_core::TaskStatus::Completed,
        failure_code: None,
        error: None,
        account: Some("acc1".to_string()),
        account_pref: None,
        conversation_id: Some("conv_1".to_string()),
        video_url: Some("https://v.example/x.mp4".to_string()),
        local_path: Some(r"C:\out\run_1_n2_1.mp4".to_string()),
        reference_images: vec![],
        batch_id: Some("run_1".to_string()),
        batch_name: Some("n2".to_string()),
        scene_index: Some(1),
        retry_count: 0,
        retry_at: 0.0,
        created_at: 1_700_000_000.0,
        updated_at: 1_700_000_010.0,
        run_id: "run_1".to_string(),
        node_id: "n2".to_string(),
        stage: "done".to_string(),
        started_at: Some(1_700_000_001.0),
        finished_at: Some(1_700_000_010.0),
    }
}

fn engine_events_fixture() -> Vec<EngineEvent> {
    vec![
        EngineEvent::TaskStage {
            task_id: "task_1".to_string(),
            stage: "generating".to_string(),
            detail: Some("acc1".to_string()),
            elapsed_ms: 1234,
        },
        EngineEvent::TaskDone {
            task_id: "task_1".to_string(),
            account: "acc1".to_string(),
            conversation_id: "conv_1".to_string(),
            local_path: r"C:\out\run_1_n2_1.mp4".to_string(),
            bytes: 12_345,
            watermarked: false,
        },
        EngineEvent::TaskFailed {
            task_id: "task_2".to_string(),
            code: Some("PROXY_DOWN".to_string()),
            message: "PROXY_DOWN: acc2: loi mang".to_string(),
        },
        EngineEvent::NodeState {
            run_id: "run_1".to_string(),
            node_id: "n2".to_string(),
            state: "running".to_string(),
            done: 1,
            total: 3,
            ok: 1,
            failed: 0,
            error: None,
        },
        EngineEvent::NodeState {
            run_id: "run_1".to_string(),
            node_id: "n3".to_string(),
            state: "failed".to_string(),
            done: 1,
            total: 1,
            ok: 0,
            failed: 1,
            error: Some("node n3: canh 1 that bai".to_string()),
        },
        EngineEvent::RunDone {
            run_id: "run_1".to_string(),
            status: "completed".to_string(),
        },
    ]
}

fn port_type_json(ty: PortType) -> Value {
    serde_json::to_value(ty).expect("PortType Serialize")
}

/// `{kind, inputs:[{name,type}], outputs:[…]}` cho MOI node trong registry (P4a: sinh tu manifest
/// that cua node module, sap theo `kind`) — UI ve cong React Flow tu day.
fn ports_json() -> Value {
    let port =
        |p: &lvc_core::graph::PortSpec| json!({"name": p.name, "type": port_type_json(p.ty)});
    Value::Array(
        crate::registry::build_registry()
            .manifests()
            .iter()
            .map(|m| {
                json!({
                    "kind": m.kind,
                    "inputs": m.inputs.iter().map(port).collect::<Vec<_>>(),
                    "outputs": m.outputs.iter().map(port).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// Task 10: mot [`AccountStatus`] du moi truong, gia tri tinh toan (`cooling`/`rate_limited`/
/// `quota_blocked`/`remaining`) khop THAT SU voi cac truong tho (khong hand-wave) — proxy da MASK
/// (khong bao gio lo mat khau, ke ca trong fixture).
fn account_status_fixture() -> AccountStatus {
    AccountStatus {
        name: "acc1".to_string(),
        scheduling: true,
        note: "tai khoan chinh".to_string(),
        email: "acc1@example.com".to_string(),
        created_at: 1_700_000_000.0,
        last_used_at: 1_700_000_500.0,
        login_ok: true,
        login_checked_at: 1_700_000_400.0,
        cooldown_until: 0.0,
        cooling: false,
        rate_limited_until: 0.0,
        rate_limited: false,
        limit_reason: String::new(),
        quota_blocked_until: 0.0,
        quota_blocked: false,
        quota_reason: String::new(),
        credit_balance: Some(10),
        credit_checked_at: 1_700_000_300.0,
        proxy: "1.2.3.4:8080:user:***".to_string(),
        browser: "chromium".to_string(),
        proxy_set: true,
        logged_out_at: 0.0,
        auto_verify_state: String::new(),
        auto_verify_tries: 0,
        cookie_expires_at: 1_800_000_000.0,
        used_today: 2,
        limit: 30,
        remaining: 28,
        used_pro: 0,
        limit_pro: 10,
        used_pro_max: 0,
        limit_pro_max: 10,
        used_agent: 0,
        limit_agent: 30,
    }
}

/// Task 10: `import_reports.json` gom hai mau bao cao trong MOT file (`cookies` = `import_cookies`,
/// `legacy` = `import_legacy`) — ca hai deu nho, tach file rieng khong dang.
fn import_reports_fixture() -> Value {
    json!({
        "cookies": serde_json::to_value(ImportCookiesReport {
            name: "acc1".to_string(),
            sessionid_expires_at: Some(1_800_000_000),
        })
        .unwrap(),
        "legacy": serde_json::to_value(LegacyImportReport {
            tasks: 12,
            accounts: 3,
            usage: 8,
            settings: 5,
            skipped: vec!["pool_usage.db".to_string()],
            profiles_copied: 2,
            profiles_skipped: vec!["acc_da_ton_tai".to_string()],
            // P3c Task 1 (R5): scripts/scenes tu tasks.db Python neu co bang.
            scripts: 2,
            scenes: 5,
        })
        .unwrap(),
    })
}

/// `[STAGE_LAUNCHING, ...SUBMIT_STAGES, ...POLL_STAGES, STAGE_DONE]` da khu trung (giu lan xuat
/// hien DAU TIEN) — `STAGE_GENERATING` co mat o ca cuoi `SUBMIT_STAGES` lan dau `POLL_STAGES`.
fn stages_json() -> Vec<String> {
    let mut raw = vec![STAGE_LAUNCHING.to_string()];
    raw.extend(SUBMIT_STAGES.iter().map(|s| (*s).to_string()));
    raw.extend(POLL_STAGES.iter().map(|s| (*s).to_string()));
    raw.push(STAGE_DONE.to_string());

    let mut seen = HashSet::new();
    raw.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

/// P3c Task 1: [`ScriptMeta`] mau — 3/5 xong, 1 loi, 1 dang chay (dem gop tu `queued`+`processing`,
/// xem `lvc_store::scripts::ScriptRepo::list_with_counts`).
fn script_meta_fixture() -> ScriptMeta {
    ScriptMeta {
        id: "script_1".to_string(),
        name: "Demo".to_string(),
        dir: String::new(),
        created_at: 1_700_000_000.0,
        updated_at: 1_700_000_100.0,
        total: 5,
        done: 3,
        failed: 1,
        running: 1,
        // Ba cot them sau (migration `0006`) — fixture phai khai day du de khop `ScriptMeta`
        // hien tai, khong duoc de UI tuong chung la tuy chon.
        video_kind: "dola_video".to_string(),
        video_params: String::new(),
        category: String::new(),
    }
}

/// P3c Task 1: [`ScriptDoc`] mau — 2 canh, mot canh KEM `taskStatus` (da chay xong).
fn script_doc_fixture() -> ScriptDoc {
    ScriptDoc {
        meta: script_meta_fixture(),
        scenes: vec![
            SceneDto {
                id: "scene_1".to_string(),
                idx: 1,
                prompt: "con meo di bo tren duong lang".to_string(),
                ratio: Some("9:16".to_string()),
                duration: Some(10),
                model: None,
                mode: None,
                logo: None,
                account: String::new(),
                refs: vec![],
                task_id: Some("task_1".to_string()),
                task_status: Some("completed".to_string()),
                failure_code: None,
                local_path: Some(r"C:\out\run_1_video_1.mp4".to_string()),
            },
            SceneDto {
                id: "scene_2".to_string(),
                idx: 2,
                prompt: "canh hai chua chay".to_string(),
                ratio: None,
                duration: None,
                model: None,
                mode: None,
                logo: None,
                account: String::new(),
                refs: vec![],
                task_id: None,
                task_status: None,
                failure_code: None,
                local_path: None,
            },
        ],
    }
}

/// P3c Task 1: [`ScriptRunReport`] mau — 1 canh chay duoc, 1 canh loi `SCENE_RUNNING`.
fn script_run_report_fixture() -> ScriptRunReport {
    ScriptRunReport {
        run_id: Some("run_1".to_string()),
        tasks: vec![ScriptRunTask {
            scene_id: "scene_1".to_string(),
            task_id: "task_1".to_string(),
        }],
        errors: vec![ScriptRunError {
            scene_id: "scene_2".to_string(),
            code: "SCENE_RUNNING".to_string(),
        }],
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "mot danh sach check_or_write phang cho tung fixture - de doc tuan tu, khong nen tach"
)]
fn fixtures_khop_ban_da_commit() {
    check_or_write(
        "graph.json",
        &serde_json::to_value(three_scene_graph()).unwrap(),
    );
    check_or_write(
        "graph_errors.json",
        &serde_json::to_value(graph_errors()).unwrap(),
    );
    check_or_write(
        "task_dto.json",
        &serde_json::to_value(vec![TaskDto::from_row(&task_row_fixture())]).unwrap(),
    );
    check_or_write(
        "engine_events.json",
        &serde_json::to_value(engine_events_fixture()).unwrap(),
    );
    check_or_write(
        "app_settings.json",
        &serde_json::to_value(AppSettings::default()).unwrap(),
    );
    check_or_write(
        "ipc_error.json",
        &serde_json::to_value(
            IpcError::new("GRAPH_INVALID", "do thi khong hop le: 2 loi")
                .with_details(serde_json::to_value(graph_errors()).unwrap()),
        )
        .unwrap(),
    );
    check_or_write(
        "workflow_meta.json",
        &serde_json::to_value(WorkflowMeta {
            id: "wf_1".to_string(),
            name: "Demo".to_string(),
            updated_at: 1_700_000_000.0,
            node_count: 3,
        })
        .unwrap(),
    );
    check_or_write(
        "run_row.json",
        &serde_json::to_value(RunRow {
            id: "run_1".to_string(),
            workflow_id: "wf_1".to_string(),
            status: RunStatus::Completed,
            started_at: 1_700_000_000.0,
            finished_at: 1_700_000_100.0,
            error: String::new(),
        })
        .unwrap(),
    );
    check_or_write(
        "node_states.json",
        &serde_json::to_value(vec![
            NodeStateRow {
                run_id: "run_1".to_string(),
                node_id: "im_1".to_string(),
                state: "done".to_string(),
                done: 1,
                total: 1,
                ok: 1,
                failed: 0,
                error: String::new(),
                updated_at: 1_700_000_050.0,
            },
            NodeStateRow {
                run_id: "run_1".to_string(),
                node_id: "vid_1".to_string(),
                state: "failed".to_string(),
                done: 1,
                total: 1,
                ok: 0,
                failed: 1,
                error: "echo ref".to_string(),
                updated_at: 1_700_000_060.0,
            },
        ])
        .unwrap(),
    );
    check_or_write(
        "plan_summary.json",
        &serde_json::to_value(PlanSummary {
            video_total: 3,
            warnings: Vec::new(),
        })
        .unwrap(),
    );
    check_or_write(
        "requeue_report.json",
        &serde_json::to_value(RequeueReport {
            requeued: vec!["task_3".to_string()],
            resumable: vec!["task_4".to_string()],
            // Fix-round 1 (final-review C1): truong moi — run con `running` luc khoi dong lai bi
            // dong thanh `failed`.
            runs_failed: vec!["run_5".to_string()],
        })
        .unwrap(),
    );
    check_or_write("ports.json", &ports_json());
    check_or_write(
        "catalog.json",
        &serde_json::to_value(crate::registry::build_registry().manifests()).unwrap(),
    );
    check_or_write("stages.json", &serde_json::to_value(stages_json()).unwrap());
    check_or_write(
        "account_status.json",
        &serde_json::to_value(account_status_fixture()).unwrap(),
    );
    check_or_write("import_reports.json", &import_reports_fixture());
    check_or_write(
        "script_meta.json",
        &serde_json::to_value(script_meta_fixture()).unwrap(),
    );
    check_or_write(
        "script_doc.json",
        &serde_json::to_value(script_doc_fixture()).unwrap(),
    );
    check_or_write(
        "script_run_report.json",
        &serde_json::to_value(script_run_report_fixture()).unwrap(),
    );
    check_or_write("credentials.json", &credentials_fixture());
    check_or_write("template_list.json", &template_list_fixture());
    check_or_write(
        "template_doc.json",
        &serde_json::to_value(template_doc_fixture()).unwrap(),
    );
    check_or_write(
        "verify_outcome.json",
        &serde_json::to_value(lvc_vault::VerifyOutcome {
            status: "ok".to_string(),
            detail: "key hop le".to_string(),
        })
        .unwrap(),
    );
}

/// P4d: danh sach meta template (1 public + 1 private) — UI dung gallery.
fn template_list_fixture() -> Value {
    use crate::commands::templates::TemplateMeta;
    serde_json::to_value(vec![
        TemplateMeta {
            id: "public:prompt-video-save".to_string(),
            name: "Prompt -> Video -> Luu".to_string(),
            description: "Mau co ban".to_string(),
            scope: "public".to_string(),
            category: "Cơ bản".to_string(),
            thumbnail: String::new(),
            updated_at: 0.0,
            node_count: 3,
            version: String::new(),
        },
        TemplateMeta {
            id: "tpl_1".to_string(),
            name: "Mau cua toi".to_string(),
            description: String::new(),
            scope: "private".to_string(),
            category: "Cơ bản".to_string(),
            thumbnail: String::new(),
            updated_at: 1_700_000_000.0,
            node_count: 3,
            version: String::new(),
        },
    ])
    .unwrap()
}

/// P4d: mot template day du (graph 3 node) — cho "Dung mau".
fn template_doc_fixture() -> crate::commands::templates::TemplateDoc {
    use crate::commands::templates::{TemplateDoc, TemplateMeta};
    TemplateDoc {
        meta: TemplateMeta {
            id: "public:prompt-video-save".to_string(),
            name: "Prompt -> Video -> Luu".to_string(),
            description: "Mau co ban".to_string(),
            scope: "public".to_string(),
            category: "Cơ bản".to_string(),
            thumbnail: String::new(),
            updated_at: 0.0,
            node_count: 3,
            version: String::new(),
        },
        graph: three_scene_graph(),
    }
}

/// P4c: hai [`lvc_vault::CredentialView`] (một api key đã che, một session Dola) — UI dựng tab
/// "Tài khoản AI" từ đây. Bí mật KHÔNG bao giờ có trong view (chỉ `secretHint`).
fn credentials_fixture() -> Value {
    let views = vec![
        lvc_vault::CredentialView {
            id: "cred_1".to_string(),
            provider: "openai".to_string(),
            kind: "api_key".to_string(),
            label: "OpenAI chinh".to_string(),
            secret_hint: "••••1234".to_string(),
            meta: serde_json::json!({"base_url": ""}),
            proxy_masked: String::new(),
            status: "ok".to_string(),
            enabled: true,
            daily_limit: Some(50),
            used_today: 12,
            last_used_at: 1_700_000_050.0,
            last_verified_at: 1_700_000_060.0,
        },
        lvc_vault::CredentialView {
            id: "cred_2".to_string(),
            provider: "dola".to_string(),
            kind: "session".to_string(),
            label: "dola-01".to_string(),
            secret_hint: "12 cookie · có sessionid".to_string(),
            meta: serde_json::json!({"profile_dir": "accounts/dola-01"}),
            proxy_masked: "http://1.2.3.4:***".to_string(),
            status: "unknown".to_string(),
            enabled: true,
            daily_limit: None,
            used_today: 0,
            last_used_at: 0.0,
            last_verified_at: 0.0,
        },
    ];
    serde_json::to_value(views).unwrap()
}
