//! Su kien tien trinh cua engine (P3 Task 4, ruling R5, ADR-R005): [`EngineEvent`] phat qua
//! `tokio::sync::broadcast` (dung luong [`EVENT_CAPACITY`] = 1024; receiver cham -> mat su kien cu,
//! bo qua + `warn`, khong bao gio chan engine). Tauri (Task 8) `subscribe()` roi forward tung su
//! kien thanh `emit("engine://event", payload)`; JSON: `type` `snake_case`, truong `camelCase` (khop
//! `Graph`/`RunPlan` cua lvc-core — ruling T1/T2).
//!
//! Python: `task_stage.set_stage` (task_stage.py:16-22, trong bo nho, UI poll `GET /api/admin/tasks`)
//! — o day day (push) thay vi keo (poll); `TaskDone`/`TaskFailed` tuong ung `_finish_task`
//! (server.py:442-480) va nhanh `except` cua `_run_task` (server.py:674-712).

use serde::Serialize;
use tokio::sync::broadcast;
use tracing::warn;

/// R5: dung luong kenh broadcast; day -> receiver cham bi `Lagged` (bo qua co log).
pub const EVENT_CAPACITY: usize = 1024;

/// Mot su kien engine (R5). `task_id` = id dong `tasks`; `run_id`/`node_id` = DAG executor (Task 7).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum EngineEvent {
    /// Buoc hien tai cua task (`task_stage.set_stage(stage, detail)`): `launching` (engine, detail =
    /// tai khoan) → `page`/`mode`/`composer`/`refs`/`prompt`/`sending` (submit) → `generating` (khi co
    /// conv id, va dau vong poll) → `confirm`/`downloading` (poll) → `done` (engine). `elapsed_ms` ke
    /// tu luc `generate_video` bat dau.
    ///
    /// Fix-round Minor M7 (review task-4-review.md finding 8): su kien nay phat TRUOC khi cot
    /// `tasks.stage` tuong ung duoc ghi (ghi qua kenh `mpsc` boi mot task nen rieng, xem
    /// `generate::EngineHooks::stage` — hook la ham DONG BO, khong `await` duoc). Mot UI nhan
    /// `TaskStage` roi doc lai dong `tasks` NGAY LAP TUC co the van thay `stage` CU. Vo hai cho trang
    /// thai CUOI (`TaskDone`/`TaskFailed` chi phat SAU khi kenh ghi da can), chi anh huong neu UI
    /// dua vao cot DB giua chung thay vi truong `stage` cua chinh su kien nay.
    TaskStage {
        task_id: String,
        stage: String,
        detail: Option<String>,
        elapsed_ms: u64,
    },
    /// Video xong, DB da ghi `completed` (`VideoArtifact` phang hoa: `local_path` dang chuoi).
    TaskDone {
        task_id: String,
        account: String,
        conversation_id: String,
        local_path: String,
        bytes: u64,
        watermarked: bool,
    },
    /// Task loi, DB da ghi `failed` + `code` (`FailureCode::as_str`, `None` khi loi khong ma) +
    /// `message` (= cot `error`, da cat 500 ky tu). Huy = `code: Some("cancelled")`.
    TaskFailed {
        task_id: String,
        code: Option<String>,
        message: String,
    },
    /// Task 7: trang thai mot node trong run (`queued`/`running`/`done`/`failed`...), `done`/`total`
    /// = so job con da xong / tong.
    NodeState {
        run_id: String,
        node_id: String,
        state: String,
        done: u32,
        total: u32,
    },
    /// Task 7: run ket thuc (`status` = `completed`/`failed`/`cancelled`).
    RunDone { run_id: String, status: String },
}

/// Kenh phat su kien. `emit` khong bao gio loi: khong co receiver -> bo qua (`SendError`).
#[derive(Debug)]
pub struct EngineEvents {
    tx: broadcast::Sender<EngineEvent>,
}

impl Default for EngineEvents {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineEvents {
    /// Kenh dung luong [`EVENT_CAPACITY`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(EVENT_CAPACITY)
    }

    /// Dung luong tuy chon (test lag). `capacity` phai > 0 (broadcast panic neu 0).
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Receiver moi — chi thay su kien phat SAU khi subscribe (broadcast khong giu lich su).
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.tx.subscribe()
    }

    /// Phat `ev` cho moi receiver; khong co receiver -> bo qua (khong loi, khong log).
    pub fn emit(&self, ev: EngineEvent) {
        // `SendError` chi xay ra khi 0 receiver (broadcast khong bao gio day theo nghia chan).
        let _ = self.tx.send(ev);
    }

    #[must_use]
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

/// Bridge (Task 8 dung lai): su kien ke tiep tren `rx`. `Lagged(n)` (receiver cham, mat `n` su kien
/// cu) -> `warn` + doc tiep (bo qua, R5); `Closed` (engine drop) -> `None`.
pub async fn next_event(rx: &mut broadcast::Receiver<EngineEvent>) -> Option<EngineEvent> {
    loop {
        match rx.recv().await {
            Ok(ev) => return Some(ev),
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!(
                    skipped = n,
                    "receiver su kien engine cham -> bo qua su kien cu"
                );
            }
            Err(broadcast::error::RecvError::Closed) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn stage(i: u64) -> EngineEvent {
        EngineEvent::TaskStage {
            task_id: "t1".into(),
            stage: "generating".into(),
            detail: None,
            elapsed_ms: i,
        }
    }

    /// Ghim JSON cho frontend (Task 11 `ipc/types.ts`): `type` `snake_case`, truong `camelCase`, moi
    /// bien the mot fixture.
    #[test]
    fn serialize_moi_bien_the_tag_type_camel_case() {
        let cases: Vec<(EngineEvent, Value)> = vec![
            (
                EngineEvent::TaskStage {
                    task_id: "t1".into(),
                    stage: "page".into(),
                    detail: Some("acc1".into()),
                    elapsed_ms: 1234,
                },
                json!({"type":"task_stage","taskId":"t1","stage":"page","detail":"acc1","elapsedMs":1234}),
            ),
            (
                stage(5),
                json!({"type":"task_stage","taskId":"t1","stage":"generating","detail":null,"elapsedMs":5}),
            ),
            (
                EngineEvent::TaskDone {
                    task_id: "t1".into(),
                    account: "acc1".into(),
                    conversation_id: "123".into(),
                    local_path: r"C:\out\a.mp4".into(),
                    bytes: 10,
                    watermarked: false,
                },
                json!({"type":"task_done","taskId":"t1","account":"acc1","conversationId":"123","localPath":"C:\\out\\a.mp4","bytes":10,"watermarked":false}),
            ),
            (
                EngineEvent::TaskFailed {
                    task_id: "t1".into(),
                    code: Some("cancelled".into()),
                    message: "cancelled".into(),
                },
                json!({"type":"task_failed","taskId":"t1","code":"cancelled","message":"cancelled"}),
            ),
            (
                EngineEvent::TaskFailed {
                    task_id: "t1".into(),
                    code: None,
                    message: "TIMEOUT: x".into(),
                },
                json!({"type":"task_failed","taskId":"t1","code":null,"message":"TIMEOUT: x"}),
            ),
            (
                EngineEvent::NodeState {
                    run_id: "r1".into(),
                    node_id: "n1".into(),
                    state: "running".into(),
                    done: 1,
                    total: 3,
                },
                json!({"type":"node_state","runId":"r1","nodeId":"n1","state":"running","done":1,"total":3}),
            ),
            (
                EngineEvent::RunDone {
                    run_id: "r1".into(),
                    status: "completed".into(),
                },
                json!({"type":"run_done","runId":"r1","status":"completed"}),
            ),
        ];
        assert_eq!(cases.len(), 7, "5 bien the (+2 truong hop None)");
        for (ev, want) in cases {
            assert_eq!(serde_json::to_value(&ev).unwrap(), want, "{ev:?}");
        }
    }

    /// Khong receiver -> `emit` khong loi; subscribe SAU -> khong thay su kien cu.
    #[test]
    fn emit_khong_receiver_bo_qua() {
        let ev = EngineEvents::new();
        assert_eq!(ev.receiver_count(), 0);
        ev.emit(stage(1));
        ev.emit(stage(2));
        let mut rx = ev.subscribe();
        assert_eq!(ev.receiver_count(), 1);
        assert!(matches!(
            rx.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        ev.emit(stage(3));
        assert_eq!(rx.try_recv().unwrap(), stage(3));
    }

    /// Hai receiver deu nhan moi su kien (broadcast), theo thu tu phat.
    #[tokio::test]
    async fn hai_receiver_cung_nhan() {
        let ev = EngineEvents::new();
        let mut a = ev.subscribe();
        let mut b = ev.subscribe();
        ev.emit(stage(1));
        ev.emit(stage(2));
        assert_eq!(next_event(&mut a).await, Some(stage(1)));
        assert_eq!(next_event(&mut a).await, Some(stage(2)));
        assert_eq!(next_event(&mut b).await, Some(stage(1)));
        assert_eq!(next_event(&mut b).await, Some(stage(2)));
    }

    /// R5: dung luong 1024, phat 1100 khong doc -> receiver `Lagged(76)`; `next_event` bo qua co
    /// `warn` roi tra su kien CU NHAT con giu (#76), tong doc duoc = 1024; sender drop -> `None`.
    #[tokio::test]
    async fn lagged_bo_qua_va_doc_tiep_1024() {
        let ev = EngineEvents::new();
        let mut rx = ev.subscribe();
        for i in 0..1100u64 {
            ev.emit(stage(i));
        }
        // try_recv thay Lagged(76) — chinh la loi ma next_event nuot
        assert!(matches!(
            rx.try_recv(),
            Err(broadcast::error::TryRecvError::Lagged(76))
        ));
        let first = next_event(&mut rx).await.expect("con su kien");
        assert_eq!(first, stage(76));
        let mut n = 1;
        let mut last = 76;
        while let Ok(e) = rx.try_recv() {
            let EngineEvent::TaskStage { elapsed_ms, .. } = e else {
                panic!("{e:?}");
            };
            assert_eq!(elapsed_ms, last + 1, "thu tu");
            last = elapsed_ms;
            n += 1;
        }
        assert_eq!(n, EVENT_CAPACITY);
        assert_eq!(last, 1099);
        drop(ev);
        assert_eq!(next_event(&mut rx).await, None, "sender dong -> None");
    }

    /// `next_event` gap Lagged NHIEU lan lien tiep (receiver rat cham) van tra su kien, khong panic.
    #[tokio::test]
    async fn lagged_nhieu_lan_khong_panic() {
        let ev = EngineEvents::with_capacity(2);
        let mut rx = ev.subscribe();
        for i in 0..10u64 {
            ev.emit(stage(i));
        }
        assert_eq!(next_event(&mut rx).await, Some(stage(8)));
        assert_eq!(next_event(&mut rx).await, Some(stage(9)));
        for i in 10..20u64 {
            ev.emit(stage(i));
        }
        assert_eq!(next_event(&mut rx).await, Some(stage(18)));
    }

    #[test]
    fn engine_events_send_sync_va_default() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EngineEvents>();
        assert_send_sync::<EngineEvent>();
        assert_eq!(EngineEvents::default().receiver_count(), 0);
    }
}
