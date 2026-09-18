//! `Engine::resume` — P3 Task 6: tiep tuc MOT task da co `conversation_id` (Dola co the van dang
//! tao video luc app bi tat/crash giua chung, credit DA tieu tu lan gui truoc) sau khi khoi dong
//! lai. Port `browser_pool.resume_video` (`:1059-1090`) + `video_worker_ui.resume_video`
//! (`:1787-1842`) + `server._resume_task` (`:823-856`).
//!
//! Khac [`Engine::generate_video`] (Task 17): KHONG `submit` (KHONG gui prompt lai — Dola co the da
//! tao video roi, gui lai se ra hoi thoai MOI va tru credit oan lan nua); CHI dung DUNG tai khoan da
//! luu trong dong task (KHONG `pick_account`/`candidates_for`, KHONG xoay khi loi, KHONG doi mode);
//! va KHONG BAO GIO `claim` (BUG-003 "resume KHONG tang usage") — ke ca khi `Timeout`, khac
//! `generate_video` (`after_dola_error` claim luc Timeout vi coi nhu Dola van dang chay).
//!
//! [`Engine::requeue_on_start`] port `server.resume_incomplete_tasks` (`:897-915`) buoc 1+2: task
//! `processing` KHONG `conversation_id` -> `queued` ngay ([`lvc_store::TaskRepo::requeue_orphans`]);
//! task `processing` CO `conversation_id` -> CHI tra ve danh sach id
//! ([`lvc_store::TaskRepo::orphans_with_conversation`]) — ham nay KHONG tu goi [`Engine::resume`],
//! Task 7 (DAG executor) / Task 9 (Tauri command) quyet dinh chay khi nao va theo thu tu nao.
//!
//! | Rust | Python | Ghi chu |
//! |---|---|---|
//! | [`Engine::resume`] | `resume_video` + `_resume_task` | dang ky token + kenh stage NHU `generate_video` (xem `Engine::generate_video`) |
//! | limiter slot + lease DUNG 1 tai khoan | `DynamicLimiter::acquire` + `try_lease`/`lease_wait` | KHONG `pick_account`/`candidates_for` |
//! | `goto` `/chat/<conv>` + `probe_session` | `goto` + `assert_logged_in` | khong dang nhap -> `DolaError::LoggedOut` (dong phien, `mark_logged_out`) |
//! | `poll_and_download` voi `timeout = video_timeout` day du | `remaining = deadline_at - now` | ruling: `TaskRow` chua co `deadline_at` — dung `video_timeout` day du tu luc resume (Python cung fallback vay khi thieu `deadline_at`) |
//! | Ok -> `touch_last_used` (KHONG `claim`) | BUG-003 | |
//! | Loi -> [`Engine::mark_dola_error`] (dung lai) nhung KHONG `claim`/KHONG xoay | `except` `:1192-1237` tru phan claim luc Timeout | |
//! | `EngineError::NotResumable` | — (moi, P3) | row thieu `conversation_id`/`account` -> khong the resume; `failure_code` tai su dung `NoConversation` (khong them ma Dola moi) |
//!
//! **Khong log prompt/cookie/URL co chu ky** (giong `generate.rs`).

use std::collections::HashSet;
use std::time::Duration;

use lvc_browser::dola::labels::DOLA_CHAT_URL;
use lvc_browser::dola::{ConversationId, PollHooks, PollOpts};
use lvc_browser::{BrowserDriver, DolaError};
use lvc_core::{FailureCode, Mode, SessionState, TaskStatus, video_timeout};
use lvc_store::{AccountRow, TaskRow};
use serde::Serialize;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::EngineError;
use crate::events::EngineEvent;
use crate::generate::{
    ERROR_MAX_CHARS, Engine, EngineHooks, GenerateOutcome, Opened, STAGE_DONE, STAGE_LAUNCHING,
    SessionAction, clip, now_unix, stage_writer,
};
use crate::pool::Lease;

/// Ket qua [`Engine::requeue_on_start`] (Task 9 Tauri command tra ve JSON cho UI). `Serialize`
/// `camelCase` (khop quy uoc cac kieu huong Tauri khac cua workspace, vd `lvc_core::graph::plan`).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequeueReport {
    /// `task_id` da dua ve `queued` (khong co `conversation_id` — chua gui gi, requeue an toan).
    pub requeued: Vec<String>,
    /// `task_id` co `conversation_id` — UNG VIEN cho [`Engine::resume`]; ham nay KHONG tu chay
    /// chung, chi liet ke (Task 7/9 quyet dinh thu tu/thoi diem).
    pub resumable: Vec<String>,
}

/// server.py:1108-1123-style validate THUAN (khong I/O) tren mot [`TaskRow`]: `conversation_id` +
/// `account` khong rong (sau `trim`) va `mode` doc duoc -> `(conv_id, account, mode, duration,
/// has_refs)`; thieu mot trong hai truong dau -> [`EngineError::NotResumable`].
fn resume_fields(
    task_id: &str,
    row: &TaskRow,
) -> Result<(String, String, Mode, u16, bool), EngineError> {
    let conv_id = row
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let account = row
        .account
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let (conv_id, name) = match (conv_id, account) {
        (Some(c), Some(a)) => (c.to_string(), a.to_string()),
        _ => {
            return Err(EngineError::NotResumable {
                task_id: task_id.to_string(),
                why: "thieu conversation_id hoac account -> khong the mo lai hoi thoai".to_string(),
            });
        }
    };
    let mode = Mode::parse(&row.mode).map_err(|e| EngineError::NotResumable {
        task_id: task_id.to_string(),
        why: format!("mode khong hop le: {e}"),
    })?;
    // duration luon nho (giay, xem lvc_core::scene::DURATIONS) — phong thu khi cot bi hong du.
    let duration = u16::try_from(row.duration).unwrap_or(u16::MAX);
    let has_refs = !row.reference_images.is_empty();
    Ok((conv_id, name, mode, duration, has_refs))
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// `server.resume_incomplete_tasks` (`:897-915`) buoc 1+2 — xem doc module. Chi doc/ghi
    /// `requeue_orphans`, KHONG tu goi [`Engine::resume`] (Task 7/9 quyet dinh).
    ///
    /// # Errors
    /// `Store` khi doc/ghi DB loi.
    pub async fn requeue_on_start(&self, now: f64) -> Result<RequeueReport, EngineError> {
        let requeued = self.tasks.requeue_orphans(now).await?;
        let resumable = self
            .tasks
            .orphans_with_conversation()
            .await?
            .into_iter()
            .map(|r| r.id)
            .collect();
        Ok(RequeueReport {
            requeued,
            resumable,
        })
    }

    /// Tiep tuc `task_id` tu `conversation_id` da luu — xem doc module. `hooks` nhu
    /// [`Engine::generate_video`]: boc trong [`EngineHooks`] (dang ky token TRUOC moi viec khac,
    /// stage -> su kien + cot `tasks.stage`); dong task duoc ghi `Failed`/`Completed` MOT CHO DUY
    /// NHAT (sau khi `resume_run` tra ve), giong het cach `generate_video` lam voi `run`.
    ///
    /// # Errors
    /// [`EngineError::TaskNotFound`] (khong co dong); [`EngineError::NotResumable`] (thieu
    /// `conversation_id`/`account`); loi trinh duyet/Dola (dong task da ghi `Failed` +
    /// `failure_code` TRUOC khi tra ve). KHONG BAO GIO `claim` usage (ruling BUG-003).
    pub async fn resume(
        &self,
        task_id: &str,
        hooks: &dyn PollHooks,
    ) -> Result<GenerateOutcome, EngineError> {
        // R4: dang ky token TRUOC moi viec khac (nhu generate_video) — cua so "huy bi lo" nho nhat.
        let (token, _guard) = self.cancels().register_guarded(task_id);
        if self.tasks.get(task_id).await?.is_none() {
            return Err(EngineError::TaskNotFound(task_id.to_string()));
        }
        let (stage_tx, stage_rx) = mpsc::unbounded_channel::<String>();
        let writer = tokio::spawn(stage_writer(
            self.tasks.clone(),
            task_id.to_string(),
            stage_rx,
        ));
        let eh = EngineHooks::new(task_id, token, self.events(), hooks, stage_tx);

        let res = self.resume_run(task_id, &eh).await;
        match &res {
            Ok(out) => {
                info!(
                    task_id,
                    account = %out.account,
                    conversation_id = %out.artifact.conversation_id,
                    "resume: video xong"
                );
                eh.stage(STAGE_DONE, None);
            }
            Err(e) => {
                let code = e.failure_code();
                let msg = clip(&e.to_string(), ERROR_MAX_CHARS);
                warn!(task_id, code = ?code.map(FailureCode::as_str), error = %msg, "resume: that bai");
                if let Err(se) = self
                    .tasks
                    .set_status(task_id, TaskStatus::Failed, code, Some(&msg))
                    .await
                {
                    warn!(task_id, error = %se, "resume: khong ghi duoc trang thai failed");
                }
            }
        }
        drop(eh);
        if let Err(e) = writer.await {
            warn!(task_id, error = %e, "resume: task ghi stage ket thuc bat thuong");
        }
        match &res {
            Ok(out) => self.events().emit(EngineEvent::TaskDone {
                task_id: task_id.to_string(),
                account: out.account.clone(),
                conversation_id: out.artifact.conversation_id.0.clone(),
                local_path: out.artifact.local_path.display().to_string(),
                bytes: out.artifact.bytes,
                watermarked: out.artifact.watermarked,
            }),
            Err(e) => self.events().emit(EngineEvent::TaskFailed {
                task_id: task_id.to_string(),
                code: e.failure_code().map(|c| c.as_str().to_string()),
                message: clip(&e.to_string(), ERROR_MAX_CHARS),
            }),
        }
        res
    }

    /// Than `resume` (tach de `resume` giu nguyen phan dang ky token/kenh stage/ghi dong task o MOT
    /// CHO, nhu `generate_video`/`run`): doc lai row -> validate -> slot -> lease DUNG tai khoan ->
    /// `set_status(Processing)`/`set_started` -> mo phien + goto + probe
    /// ([`Engine::resume_open`]) -> `poll_and_download` -> Ok (khong claim) hoac Err (qua
    /// [`Engine::fail_dola_session`], cung khong claim/khong xoay).
    async fn resume_run(
        &self,
        task_id: &str,
        eh: &EngineHooks<'_>,
    ) -> Result<GenerateOutcome, EngineError> {
        let row = self
            .tasks
            .get(task_id)
            .await?
            .ok_or_else(|| EngineError::TaskNotFound(task_id.to_string()))?;
        let (conv_id, name, mode, duration, has_refs) = resume_fields(task_id, &row)?;

        // browser_pool.py:1105 slot — nhu `run` (R4 #1: uu tien token khi cho vo thoi han).
        let _slot = tokio::select! {
            biased;
            () = eh.cancel_token().cancelled() => return Err(EngineError::Dola(DolaError::Cancelled)),
            p = self.limiter().acquire() => p,
        };
        eh.check("sau slot")?;

        // Dung DUNG tai khoan trong row de lay proxy — KHONG candidates_for/pick_account.
        let acc_row = self.pool().account_row(&name).await?.ok_or_else(|| {
            EngineError::PreferredUnavailable(format!("account '{name}' does not exist"))
        })?;
        // browser_pool.py:1133-1136 (REQ-004): tai khoan chi dinh dang ban -> CHO, khong xoay.
        let lease = tokio::select! {
            biased;
            () = eh.cancel_token().cancelled() => return Err(EngineError::Dola(DolaError::Cancelled)),
            r = self.pool().lease_wait(&name, &acc_row.proxy, self.cfg.busy_wait_max) => r?,
        };

        self.tasks
            .set_status(task_id, TaskStatus::Processing, None, None)
            .await?;
        if row.started_at.is_none() {
            self.tasks.set_started(task_id, now_unix()).await?;
        }

        let session = self
            .resume_open(&name, mode, &acc_row, &lease, &conv_id, eh)
            .await?;

        let popts = PollOpts {
            // video_worker_ui.py:2398 (fallback khi thieu deadline_at — xem doc module).
            timeout: Duration::from_secs(video_timeout(mode, duration, has_refs, &self.limits)),
            mode,
            duration,
            ignore_videos: HashSet::new(),
            ignore_links: HashSet::new(),
            accept_links: mode.is_pro_like(),
        };
        let conv = ConversationId(conv_id);
        let polled = self
            .driver
            .poll_and_download(
                &session,
                &name,
                &conv,
                &self.cfg.downloads_dir,
                &popts,
                &self.limits,
                eh,
            )
            .await;
        match polled {
            Ok(artifact) => {
                // BUG-003: resume KHONG claim — CHI ghi last_used_at.
                self.pool().touch_last_used(&name, now_unix()).await?;
                self.stash(&name, session).await;
                self.tasks
                    .set_result(task_id, &artifact.video_url, &artifact.local_path)
                    .await?;
                self.tasks
                    .set_status(task_id, TaskStatus::Completed, None, None)
                    .await?;
                self.tasks.set_finished(task_id, now_unix()).await?;
                Ok(GenerateOutcome {
                    artifact,
                    account: name,
                    mode_used: mode,
                })
            }
            Err(e) => Err(self.fail_dola_session(&name, mode, e, session).await),
        }
    }

    /// Mo/tai dung phien ([`Engine::open_session`] — dung lai nguyen ven, `Rotate` doc nhu loi CUOI
    /// CUNG vi resume khong co tai khoan ke tiep) -> `goto` chat -> `probe_session`; khong dang nhap
    /// -> xu ly nhu loi Dola ([`Engine::fail_dola_session`], `DolaError::LoggedOut`). Tach khoi
    /// `resume_run` de gioi han do dai ham.
    async fn resume_open(
        &self,
        name: &str,
        mode: Mode,
        acc_row: &AccountRow,
        lease: &Lease,
        conv_id: &str,
        eh: &EngineHooks<'_>,
    ) -> Result<D::Session, EngineError> {
        // R4: sau lease, TRUOC launch/tai dung phien.
        eh.check("truoc launch")?;
        eh.stage(STAGE_LAUNCHING, Some(name));
        let session = match self.open_session(acc_row, lease).await? {
            Opened::Session(s) => s,
            Opened::Rotate(e) => {
                return Err(e.unwrap_or_else(|| {
                    EngineError::PreferredUnavailable(format!(
                        "account '{name}': khong mo duoc trinh duyet"
                    ))
                }));
            }
        };

        // R4: TRUOC khi dieu huong toi hoi thoai cu.
        eh.check("truoc goto")?;
        let url = format!("{DOLA_CHAT_URL}/{conv_id}");
        if let Err(e) = self.driver.goto(&session, &url).await {
            self.close(name, session).await;
            return Err(e.into());
        }
        let probe = match self.driver.probe_session(&session).await {
            Ok(p) => p,
            Err(e) => {
                self.close(name, session).await;
                return Err(e.into());
            }
        };
        if probe.state != SessionState::LoggedIn {
            let why = format!("khong dang nhap (probe state={})", probe.state.as_str());
            let logged_out = DolaError::LoggedOut {
                account: name.to_string(),
                why,
                revoked: false,
            };
            return Err(self
                .fail_dola_session(name, mode, logged_out, session)
                .await);
        }
        Ok(session)
    }

    /// Loi Dola sau khi da co phien (probe khong dang nhap, hoac `poll_and_download` loi): phan loai
    /// qua [`Engine::mark_dola_error`] (dung lai nguyen ven bang dong/giu phien + danh dau
    /// `AccountPool` — KHONG `claim`, kha ca `Timeout`, khac `after_dola_error`), dong/giu phien roi
    /// tra ve `EngineError` cuoi cung de `resume_run` propagate (`resume` ghi dong task + phat su
    /// kien TAI MOT CHO, xem `Engine::resume`).
    async fn fail_dola_session(
        &self,
        name: &str,
        mode: Mode,
        e: DolaError,
        session: D::Session,
    ) -> EngineError {
        let now = now_unix();
        match self.mark_dola_error(name, mode, &e, now).await {
            Ok(SessionAction::Close) => self.close(name, session).await,
            Ok(SessionAction::Keep) => self.stash(name, session).await,
            Err(store_err) => {
                // Khong ghi duoc AccountPool -> van phai dong task ben duoi; giu phien (an toan hon
                // dong nham mot phien con dung duoc) roi log loi ghi DB.
                warn!(account = name, error = %store_err, "resume: khong danh dau duoc tai khoan, giu phien");
                self.stash(name, session).await;
            }
        }
        EngineError::from_dola(name, e)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use lvc_browser::dola::NoopHooks;
    use lvc_browser::{DolaError, FakeStep};
    use lvc_core::{FailureCode, Mode, TaskStatus};

    use super::*;
    use crate::generate::tests::{
        Fx, StageHooks, artifact, assert_cancelled, assert_row_cancelled, drain, fx, poll, scene,
    };

    /// Chuyen mot task vua tao (`queued`) thanh "dang xu ly co `conversation_id`" — mo phong task bi
    /// ngat giua chung boi restart (Task 3 `requeue_orphans`/`orphans_with_conversation` cung xet
    /// dung dieu kien nay: `status = processing` + `conversation_id` khac rong).
    async fn make_processing(f: &Fx, id: &str, account: &str, conv_id: &str) {
        f.tasks
            .set_status(id, TaskStatus::Processing, None, None)
            .await
            .unwrap();
        f.tasks.set_account(id, account).await.unwrap();
        f.tasks.set_conversation(id, conv_id).await.unwrap();
    }

    /// (1) Row co `conversation_id` -> `resume` CHI `launch` + `poll` (KHONG `submit` — khong gui
    /// prompt lai), row `Completed` co `local_path`, usage KHONG bi claim (BUG-003), `TaskDone` phat.
    #[tokio::test]
    async fn resume_co_conv_chi_launch_va_poll_khong_submit() {
        let f = fx(
            &["a"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Poll(Ok(artifact("a", "999"))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;
        make_processing(&f, "t1", "a", "999").await;
        let mut rx = f.engine.events().subscribe();

        let out = f.engine.resume("t1", &NoopHooks).await.unwrap();
        assert_eq!(out.account, "a");
        assert_eq!(out.artifact.conversation_id.0, "999");
        assert_eq!(
            f.calls(),
            vec![f.launch("a"), poll("a", "999")],
            "khong Submit"
        );

        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Completed);
        assert!(row.local_path.is_some());
        assert_eq!(row.conversation_id.as_deref(), Some("999"));
        assert_eq!(f.used("a", Mode::Fast).await, 0, "resume KHONG claim");
        assert!(row.started_at.is_some());

        let evs = drain(&mut rx);
        assert!(
            evs.iter()
                .any(|e| matches!(e, EngineEvent::TaskDone { task_id, .. } if task_id == "t1")),
            "{evs:?}"
        );
    }

    /// (2) Row KHONG co `conversation_id` -> `NotResumable`, row ghi `Failed` + `failure_code`
    /// (tai su dung `NO_CONVERSATION`, khong them ma Dola moi), KHONG dung toi trinh duyet.
    #[tokio::test]
    async fn resume_thieu_conversation_id_bao_not_resumable() {
        let f = fx(&["a"], vec![]).await;
        f.task("t1", &scene()).await;

        let e = f.engine.resume("t1", &NoopHooks).await.unwrap_err();
        assert!(matches!(e, EngineError::NotResumable { .. }), "{e:?}");
        assert_eq!(e.failure_code(), Some(FailureCode::NoConversation));

        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::NoConversation));
        assert!(f.calls().is_empty(), "khong duoc dung browser");
    }

    /// (3) `task_id` khong ton tai -> `TaskNotFound`, khong co dong nao de ghi nen khong panic.
    #[tokio::test]
    async fn resume_id_khong_ton_tai_bao_task_not_found() {
        let f = fx(&["a"], vec![]).await;
        let e = f.engine.resume("ghost", &NoopHooks).await.unwrap_err();
        assert!(matches!(e, EngineError::TaskNotFound(_)), "{e:?}");
        assert_eq!(e.to_string(), "TASK_NOT_FOUND: ghost");
        assert!(f.calls().is_empty());
    }

    /// (4) `requeue_on_start`: mot task `processing` KHONG `conversation_id` (orphan-khong-conv) va
    /// mot task `processing` CO `conversation_id` (orphan-co-conv) -> `requeued=[a]`, `resumable=[b]`;
    /// `b` KHONG bi dong (T6 chi liet ke, T7/T9 moi tu chay resume).
    #[tokio::test]
    async fn requeue_on_start_tach_orphan_co_va_khong_conv() {
        let f = fx(&["a"], vec![]).await;
        f.task("a1", &scene()).await;
        f.tasks
            .set_status("a1", TaskStatus::Processing, None, None)
            .await
            .unwrap();
        f.task("b1", &scene()).await;
        make_processing(&f, "b1", "a", "42").await;

        let report = f.engine.requeue_on_start(now_unix()).await.unwrap();
        assert_eq!(report.requeued, vec!["a1".to_string()]);
        assert_eq!(report.resumable, vec!["b1".to_string()]);

        let a = f.row("a1").await;
        assert_eq!(a.status, TaskStatus::Queued);
        assert!(a.account.is_none());
        let b = f.row("b1").await;
        assert_eq!(b.status, TaskStatus::Processing, "T6 khong tu resume b");
        assert_eq!(b.conversation_id.as_deref(), Some("42"));
    }

    /// (5) Huy giua poll (R4, giong `generate_video`): `Cancelled`, `conversation_id`/`account`
    /// GIU nguyen, KHONG claim, phien tra ve cache (KHONG dong).
    #[tokio::test]
    async fn resume_huy_giua_poll_giu_conv_khong_claim() {
        let f = fx(
            &["a"],
            vec![FakeStep::Launch(Ok(())), FakeStep::PollUntilCancelled],
        )
        .await;
        f.task("t1", &scene()).await;
        make_processing(&f, "t1", "a", "77").await;
        let hooks = StageHooks::default();

        let (res, cancelled) = tokio::join!(f.engine.resume("t1", &hooks), async {
            let t0 = Instant::now();
            while !hooks.seen().iter().any(|s| s == "generating") {
                assert!(t0.elapsed() < Duration::from_secs(5), "poll khong bat dau");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            f.engine.cancel("t1")
        });
        assert!(cancelled, "task dang chay -> cancel = true");
        let e = res.unwrap_err();
        assert_cancelled(&e);
        assert_eq!(
            f.calls(),
            vec![f.launch("a"), poll("a", "77")],
            "khong Close, khong xoay"
        );

        let row = f.row("t1").await;
        assert_row_cancelled(&row);
        assert_eq!(row.account.as_deref(), Some("a"), "giu account");
        assert_eq!(
            row.conversation_id.as_deref(),
            Some("77"),
            "giu conv de resume lai"
        );
        assert_eq!(f.used("a", Mode::Fast).await, 0, "khong claim");
        assert_eq!(
            f.engine.cache().names().await,
            vec!["a".to_string()],
            "phien tra cache"
        );
    }

    /// (6) `poll_and_download` -> `Timeout`: KHONG claim (khac `generate_video`, ruling BUG-003),
    /// row `Failed` (`failure_code = None`, `error` bat dau `TIMEOUT:`), phien GIU (tra cache).
    #[tokio::test]
    async fn resume_poll_timeout_khong_claim() {
        let f = fx(
            &["a"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Poll(Err(DolaError::Timeout("qua han".into()))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;
        make_processing(&f, "t1", "a", "88").await;

        let e = f.engine.resume("t1", &NoopHooks).await.unwrap_err();
        assert!(
            matches!(e, EngineError::Dola(DolaError::Timeout(_))),
            "{e:?}"
        );

        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, None);
        assert_eq!(row.error.as_deref(), Some("TIMEOUT: qua han"));
        assert_eq!(row.conversation_id.as_deref(), Some("88"));
        assert_eq!(
            f.used("a", Mode::Fast).await,
            0,
            "resume KHONG claim ngay ca Timeout"
        );
        assert_eq!(
            f.engine.cache().names().await,
            vec!["a".to_string()],
            "phien giu (Keep, giong bang after_dola_error)"
        );
    }
}
