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
//! [`Engine::requeue_on_start`] (fix-round 1, final-review C1 + I6 — thay ban dau port
//! `server.resume_incomplete_tasks` `:897-915` buoc 1+2): P3 KHONG co bo tieu thu doc lai `queued`
//! nen CA HAI nhanh orphan (`processing` co/khong `conversation_id`) deu dua ve
//! `Failed`+`FailureCode::Restart` qua [`lvc_store::TaskRepo::fail_orphans`] (GIU NGUYEN
//! `conversation_id`/`account`), dong thoi dong moi `runs` con `running` qua
//! [`lvc_store::RunRepo::fail_running`] — ham nay KHONG tu goi [`Engine::resume`], Task 7 (DAG
//! executor) / Task 9 (Tauri command) quyet dinh chay khi nao va theo thu tu nao.
//! [`lvc_store::TaskRepo::requeue_orphans`]/[`lvc_store::TaskRepo::orphans_with_conversation`] van
//! giu nguyen trong `lvc-store` cho bo lap lich P4 (khi co consumer thuc su cho `queued`) nhung
//! KHONG con duoc goi tu day.
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
//! **Fix-round 1 (review task-6-review.md):** C1 (Critical) — probe `Unknown` (trang chua tai xong,
//! `lvc_core::classify_session` noi ro "caller khong duoc ket luan") KHONG con bi coi la logged-out;
//! them cho on dinh + doc lai co gioi han (xem [`PROBE_SETTLE`]/[`PROBE_RETRY_TRIES`]). I1 —
//! `Timeout` ghi `last_used_at` (KHONG claim). I2 — nhanh loi cung ghi `finished_at` (best-effort).
//! I3 — tai khoan khong con trong `accounts_meta` -> `NotResumable` (khong phai
//! `PreferredUnavailable`, do la loi TAM THOI trong crate nay). M2 — chi doc dong task MOT LAN. M3 —
//! loi `goto` di qua CUNG bang dong/giu voi cac loi Dola khac (khong dong vo dieu kien nua).
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

/// C1 (fix-round 1): `video_worker_ui.py:1815-1817` cho trang on dinh 3 s (phien tai dung phien am)
/// / 5 s (phien moi mo) TRUOC khi doc dang nhap. O day KHONG phan biet duoc fresh/reused (`Engine::
/// open_session` — tai su dung tu `generate.rs`, khong sua trong dot nay — chi tra `D::Session`,
/// khong co co) nen dung MOT moc (ngan hon, phia an toan); bu lai bang vong doc lai o duoi khi con
/// `Unknown` (`PROBE_RETRY_TRIES`/`PROBE_RETRY_DELAY`). `#[cfg(test)]` = 0 de test KHONG phai cho
/// that (khong `sleep` trong test).
#[cfg(not(test))]
const PROBE_SETTLE: Duration = Duration::from_secs(3);
#[cfg(test)]
const PROBE_SETTLE: Duration = Duration::from_secs(0);

/// C1: so lan doc lai `probe_session` toi da khi con `SessionState::Unknown` — CO GIOI HAN (khac
/// `eval_retry` cua `dola::session::assert_logged_in`, do la trong luc GUI; o day chi la buoc kiem
/// truoc `poll_and_download`, ban than no se nem `LoggedOut` neu that su mat phien qua HTTP 401/403).
const PROBE_RETRY_TRIES: u32 = 3;
#[cfg(not(test))]
const PROBE_RETRY_DELAY: Duration = Duration::from_secs(1);
#[cfg(test)]
const PROBE_RETRY_DELAY: Duration = Duration::from_secs(0);

/// C1: cho `dur` (bo qua sleep khi `dur` = 0, vd trong test) roi doc lai probe — cancel-aware (uu
/// tien token, cung mau `tokio::select!` nhu cac cho khac trong file nay) roi VAN `eh.check(..)`
/// mot lan nua (bat `hooks.inner.should_cancel()` cua caller neu vua bao huy dung luc sleep xong).
async fn probe_settle(eh: &EngineHooks<'_>, dur: Duration) -> Result<(), EngineError> {
    if !dur.is_zero() {
        tokio::select! {
            biased;
            () = eh.cancel_token().cancelled() => return Err(EngineError::Dola(DolaError::Cancelled)),
            () = tokio::time::sleep(dur) => {}
        }
    }
    eh.check("cho on dinh truoc probe")
}

/// Ket qua [`Engine::requeue_on_start`] (Task 9 Tauri command tra ve JSON cho UI). `Serialize`
/// `camelCase` (khop quy uoc cac kieu huong Tauri khac cua workspace, vd `lvc_core::graph::plan`).
///
/// Fix-round 1 (final-review C1 + I6): P3 KHONG co bo tieu thu doc lai `queued` nen CA HAI truong
/// `task_id` duoi day gio la dong da bi dua ve `Failed` + `FailureCode::Restart`
/// ([`lvc_store::TaskRepo::fail_orphans`]) — ten truong GIU NGUYEN (`requeued`/`resumable`, tranh
/// doi schema TS/fixture khong can thiet) nhung Y NGHIA da doi, xem doc tung truong.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequeueReport {
    /// `task_id` KHONG co `conversation_id` -> `Failed`/`RESTART`, KHONG the "Tiep tuc" (chua tung
    /// gui gi len Dola) — nguoi dung phai chay lai workflow.
    pub requeued: Vec<String>,
    /// `task_id` CO `conversation_id` -> `Failed`/`RESTART` NHUNG co the "Tiep tuc"
    /// ([`Engine::resume`] qua lenh IPC `task_resume` — `conversation_id`/`account` da duoc GIU
    /// NGUYEN, xem [`lvc_store::TaskRepo::fail_orphans`]).
    pub resumable: Vec<String>,
    /// Fix-round 1 (C1): `run_id` con `running` luc khoi dong lai -> da chuyen `failed`
    /// ([`lvc_store::RunRepo::fail_running`]) de man Tac vu khong con "run ma mai mai" khi task
    /// cua no da bi dong o tren.
    pub runs_failed: Vec<String>,
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
    /// Fix-round 1 (final-review C1 + I6): thay `server.resume_incomplete_tasks` (`:897-915`) buoc
    /// 1+2 — P3 KHONG co bo tieu thu doc lai `queued` nen CA HAI nhanh orphan (co/khong
    /// `conversation_id`) deu dua thang ve `Failed`/`RESTART` qua MOT lan goi
    /// [`lvc_store::TaskRepo::fail_orphans`] roi tach theo `has_conversation` — KHONG con goi
    /// [`lvc_store::TaskRepo::requeue_orphans`] tu day nua (ham do van giu nguyen cho bo lap lich
    /// P4 dung `queued` that su). Dong thoi dong moi `runs` con `running`
    /// ([`lvc_store::RunRepo::fail_running`]) — xem doc [`RequeueReport`]. KHONG tu goi
    /// [`Engine::resume`] (Task 7/9 quyet dinh khi nao/thu tu nao).
    ///
    /// # Errors
    /// `Store` khi doc/ghi DB loi.
    pub async fn requeue_on_start(&self, now: f64) -> Result<RequeueReport, EngineError> {
        let orphans = self.tasks.fail_orphans(now).await?;
        let mut requeued = Vec::new();
        let mut resumable = Vec::new();
        for (id, has_conversation) in orphans {
            if has_conversation {
                resumable.push(id);
            } else {
                requeued.push(id);
            }
        }
        // Run-robustness B: run dang chay bi tat giua chung KHONG phai loi -> danh dau
        // `Interrupted` (khong `Failed`) de khoi phuc + user tu bam "Chay tiep" (quyet dinh
        // 2026-10-20: khong tu chay tiep, khong tu y ton credit). Ten truong `runs_failed` GIU
        // NGUYEN (chi la danh sach id da cham) de khong phai doi caller.
        let runs_failed = self
            .run_repo
            .interrupt_running(now, "RESTART: app khoi dong lai")
            .await?;
        Ok(RequeueReport {
            requeued,
            resumable,
            runs_failed,
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
        // M2 (fix-round 1): doc dong MOT LAN o day, truyen xuong `resume_run` — truoc doc 2 lan
        // (kiem ton tai o day, doc lai du lieu o `resume_run`).
        let Some(row) = self.tasks.get(task_id).await? else {
            return Err(EngineError::TaskNotFound(task_id.to_string()));
        };
        let (stage_tx, stage_rx) = mpsc::unbounded_channel::<String>();
        let writer = tokio::spawn(stage_writer(
            self.tasks.clone(),
            task_id.to_string(),
            stage_rx,
        ));
        let eh = EngineHooks::new(task_id, token, self.events(), hooks, stage_tx);

        let res = self.resume_run(task_id, &row, &eh).await;
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
                // I2 (fix-round 1): server.py:846-851 ghi `finished_at` tren CA HAI nhanh except —
                // best-effort, giong het cach ghi `failed` phia tren (loi chi warn, khong chan tra ve).
                if let Err(se) = self.tasks.set_finished(task_id, now_unix()).await {
                    warn!(task_id, error = %se, "resume: khong ghi duoc finished_at");
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
    /// CHO, nhu `generate_video`/`run`): validate `row` (da doc MOT LAN o `resume`, review M2) ->
    /// slot -> lease DUNG tai khoan -> `set_status(Processing)`/`set_started` -> mo phien + goto +
    /// probe ([`Engine::resume_open`]) -> `poll_and_download` -> Ok (khong claim) hoac Err (Timeout
    /// -> `touch_last_used` TRUOC, review I1; roi qua [`Engine::fail_dola_session`], cung khong
    /// claim/khong xoay).
    async fn resume_run(
        &self,
        task_id: &str,
        row: &TaskRow,
        eh: &EngineHooks<'_>,
    ) -> Result<GenerateOutcome, EngineError> {
        let (conv_id, name, mode, duration, has_refs) = resume_fields(task_id, row)?;

        // browser_pool.py:1105 slot — nhu `run` (R4 #1: uu tien token khi cho vo thoi han).
        let _slot = tokio::select! {
            biased;
            () = eh.cancel_token().cancelled() => return Err(EngineError::Dola(DolaError::Cancelled)),
            p = self.limiter().acquire() => p,
        };
        eh.check("sau slot")?;

        // Dung DUNG tai khoan trong row de lay proxy — KHONG candidates_for/pick_account.
        // I3 (fix-round 1): tai khoan khong con trong accounts_meta -> KHONG the resume (khac
        // `PreferredUnavailable`, ma trong crate nay nghia la "tam thoi", vd dang ban/rate-limited —
        // xem `pool.rs::lease_wait`); dung `NotResumable` giong het thieu conversation_id/account.
        let acc_row =
            self.pool()
                .account_row(&name)
                .await?
                .ok_or_else(|| EngineError::NotResumable {
                    task_id: task_id.to_string(),
                    why: format!("account '{name}' khong con trong accounts_meta"),
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
            Err(e) => {
                // I1 (fix-round 1): browser_pool.py:1083-1087 ghi last_used_at tren Timeout (Dola
                // van dang tao video du minh mat ket noi) — TRUOC khi xu ly loi, KHONG claim.
                if matches!(e, DolaError::Timeout(_)) {
                    self.pool().touch_last_used(&name, now_unix()).await?;
                }
                Err(self.fail_dola_session(&name, mode, e, session).await)
            }
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
            // M3 (fix-round 1): di qua CUNG bang dong/giu voi moi loi Dola khac (khong dong vo dieu
            // kien nua) — vd loi mang tam thoi (khong phai proxy chet) thi GIU phien nhu Python
            // `release(close=False)`.
            return Err(self
                .fail_dola_session(name, mode, DolaError::Browser(e), session)
                .await);
        }

        // C1 (fix-round 1): video_worker_ui.py:1815-1817 cho trang on dinh TRUOC khi doc dang nhap;
        // roi doc lai co gioi han neu con `Unknown` thay vi ket luan ngay (xem doc [`PROBE_SETTLE`]).
        probe_settle(eh, PROBE_SETTLE).await?;
        let mut probe = match self.driver.probe_session(&session).await {
            Ok(p) => p,
            Err(e) => {
                return Err(self
                    .fail_dola_session(name, mode, DolaError::Browser(e), session)
                    .await);
            }
        };
        let mut tries = 0u32;
        while probe.state == SessionState::Unknown && tries < PROBE_RETRY_TRIES {
            probe_settle(eh, PROBE_RETRY_DELAY).await?;
            probe = match self.driver.probe_session(&session).await {
                Ok(p) => p,
                Err(e) => {
                    return Err(self
                        .fail_dola_session(name, mode, DolaError::Browser(e), session)
                        .await);
                }
            };
            tries += 1;
        }
        match probe.state {
            SessionState::LoggedIn => Ok(session),
            // lvc_core::classify_session: "Unknown" = trang CHUA TAI XONG, caller KHONG duoc ket
            // luan da mat phien (day chinh la loi Critical C1 truoc khi sua — coi Unknown = logged
            // out lam `mark_logged_out` oan tai khoan con dung tot). `poll_and_download` tu no se
            // nem `LoggedOut` neu that su mat phien qua HTTP 401/403, nen o day cu tiep tuc poll.
            SessionState::Unknown => {
                warn!(
                    account = name,
                    tries = PROBE_RETRY_TRIES,
                    "probe chua ket luan duoc dang nhap sau khi doc lai -> tiep tuc poll"
                );
                Ok(session)
            }
            SessionState::LoggedOut => {
                let why = format!("khong dang nhap (probe state={})", probe.state.as_str());
                let logged_out = DolaError::LoggedOut {
                    account: name.to_string(),
                    why,
                    revoked: false,
                };
                Err(self
                    .fail_dola_session(name, mode, logged_out, session)
                    .await)
            }
        }
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
    use lvc_browser::{DolaError, FakeSession, FakeStep};
    use lvc_core::{FailureCode, Mode, SessionState, TaskStatus};

    use super::*;
    use crate::generate::tests::{
        Fx, StageHooks, artifact, assert_cancelled, assert_row_cancelled, drain, fx, poll, scene,
        stages_of,
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
        assert_eq!(row.stage, "done");
        // I1 (fix-round 1): nhanh Ok da luon `touch_last_used` — pin no o day (I1 tap trung vao
        // nhanh Timeout, nhung nhanh thanh cong phai giu dung hanh vi cu nay).
        assert!(f.account("a").await.last_used_at > 0.0);
        // review M6: RAII go token khoi registry sau khi resume xong.
        assert!(f.engine.cancels().is_empty());

        // M6: chuoi stage DUNG THU TU (launching -> generating/downloading cua FakeDriver::poll ->
        // done) va `TaskDone` la su kien CUOI (phat SAU khi DB da ghi, xem `Engine::resume`).
        let evs = drain(&mut rx);
        assert_eq!(
            stages_of(&evs),
            vec!["launching", "generating", "downloading", "done"]
        );
        assert!(
            matches!(evs.last(), Some(EngineEvent::TaskDone { task_id, .. }) if task_id == "t1"),
            "{evs:?}"
        );
    }

    /// (2) Row KHONG co `conversation_id` -> `NotResumable`, row ghi `Failed` + `failure_code`
    /// (tai su dung `NO_CONVERSATION`, khong them ma Dola moi), KHONG dung toi trinh duyet.
    #[tokio::test]
    async fn resume_thieu_conversation_id_bao_not_resumable() {
        let f = fx(&["a"], vec![]).await;
        f.task("t1", &scene()).await;
        let mut rx = f.engine.events().subscribe();

        let e = f.engine.resume("t1", &NoopHooks).await.unwrap_err();
        assert!(matches!(e, EngineError::NotResumable { .. }), "{e:?}");
        assert_eq!(e.failure_code(), Some(FailureCode::NoConversation));

        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::NoConversation));
        assert!(f.calls().is_empty(), "khong duoc dung browser");
        // I2 (fix-round 1): nhanh loi cung ghi finished_at (best-effort), nhu nhanh thanh cong.
        assert!(row.finished_at.is_some());

        // M6: TaskFailed voi ma NO_CONVERSATION (tai su dung, khong them ma Dola moi).
        let evs = drain(&mut rx);
        assert!(
            matches!(
                evs.last(),
                Some(EngineEvent::TaskFailed { task_id, code, .. })
                    if task_id == "t1" && code.as_deref() == Some("NO_CONVERSATION")
            ),
            "{evs:?}"
        );
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

    /// (4) `requeue_on_start` (fix-round 1, C1+I6): ba dong bi cham toi luc "khoi dong lai" — (a)
    /// task `processing` KHONG `conversation_id` (b) task `processing` CO `conversation_id` (c) mot
    /// `runs` con `running`. P3 KHONG co bo tieu thu `queued` nen CA (a) VA (b) deu thanh
    /// `Failed`+`RESTART` (khac ban truoc: (a) tung ve `queued`) — chi khac o cho (b) GIU
    /// `conversation_id`/`account` de "Tiep tuc" con (a) thi khong co gi de giu; (c) chuyen
    /// `runs.status` sang `failed`.
    #[tokio::test]
    async fn requeue_on_start_dua_ca_hai_nhanh_orphan_ve_failed_restart_va_dong_run_dang_chay() {
        let f = fx(&["a"], vec![]).await;
        f.task("a1", &scene()).await;
        f.tasks
            .set_status("a1", TaskStatus::Processing, None, None)
            .await
            .unwrap();
        f.task("b1", &scene()).await;
        make_processing(&f, "b1", "a", "42").await;
        f.engine
            .run_repo
            .create("r1", "wf1", lvc_store::RunStatus::Running, now_unix())
            .await
            .unwrap();

        let report = f.engine.requeue_on_start(now_unix()).await.unwrap();
        assert_eq!(report.requeued, vec!["a1".to_string()]);
        assert_eq!(report.resumable, vec!["b1".to_string()]);
        assert_eq!(report.runs_failed, vec!["r1".to_string()]);

        let a = f.row("a1").await;
        assert_eq!(
            a.status,
            TaskStatus::Failed,
            "P3 khong co bo tieu thu queued"
        );
        assert_eq!(a.failure_code, Some(FailureCode::Restart));
        assert!(a.account.is_none(), "chua tung gan tai khoan");
        assert!(a.finished_at.is_some());

        let b = f.row("b1").await;
        assert_eq!(b.status, TaskStatus::Failed);
        assert_eq!(b.failure_code, Some(FailureCode::Restart));
        assert_eq!(b.conversation_id.as_deref(), Some("42"), "giu de Tiep tuc");
        assert_eq!(b.account.as_deref(), Some("a"), "giu tai khoan");
        assert!(b.finished_at.is_some());

        let run = f.engine.run_repo.get("r1").await.unwrap().unwrap();
        assert_eq!(run.status, lvc_store::RunStatus::Interrupted);
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
        // I2 (fix-round 1): huy cung la mot nhanh loi -> finished_at duoc ghi.
        assert!(row.finished_at.is_some());
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
        // I1 (fix-round 1): browser_pool.py:1083-1087 — Timeout van ghi last_used_at (khong claim).
        assert!(f.account("a").await.last_used_at > 0.0);
        // I2 (fix-round 1): finished_at duoc ghi tren nhanh loi.
        assert!(row.finished_at.is_some());
    }

    /// (7) I3 (fix-round 1): tai khoan trong row khong con trong `accounts_meta` (chua tung tao) ->
    /// `NotResumable` (KHONG phai `PreferredUnavailable`, ma trong crate nay nghia la "tam thoi"),
    /// row `Failed` + ma tai su dung `NO_CONVERSATION`, KHONG dung toi trinh duyet.
    #[tokio::test]
    async fn resume_account_khong_ton_tai_bao_not_resumable() {
        let f = fx(&["a"], vec![]).await;
        f.task("t1", &scene()).await;
        make_processing(&f, "t1", "ghost", "1").await;

        let e = f.engine.resume("t1", &NoopHooks).await.unwrap_err();
        assert!(matches!(e, EngineError::NotResumable { .. }), "{e:?}");
        assert_eq!(e.failure_code(), Some(FailureCode::NoConversation));
        assert!(f.calls().is_empty(), "khong duoc dung browser");

        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::NoConversation));
    }

    /// C1 (fix-round 1, Critical): phien am san co trong cache voi `probe_state: LoggedOut` — resume
    /// tai dung phien (KHONG `Launch`), `probe_session` bao logged-out -> danh dau + dong phien +
    /// row `Failed` + `LOGGED_OUT`, KHONG claim.
    #[tokio::test]
    async fn resume_probe_logged_out_mark_va_dong_phien() {
        let f = fx(&["a"], vec![]).await;
        f.task("t1", &scene()).await;
        make_processing(&f, "t1", "a", "555").await;
        f.engine
            .cache()
            .put(
                "a",
                FakeSession {
                    account: "a".into(),
                    probe_state: SessionState::LoggedOut,
                    seq: 0,
                    connected: true,
                },
            )
            .await;

        let e = f.engine.resume("t1", &NoopHooks).await.unwrap_err();
        assert!(
            matches!(e, EngineError::Dola(DolaError::LoggedOut { .. })),
            "{e:?}"
        );
        assert_eq!(e.failure_code(), Some(FailureCode::LoggedOut));
        assert_eq!(
            f.calls(),
            vec![lvc_browser::FakeCall::Close],
            "chi Close — tai dung phien tu cache, khong Launch"
        );

        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::LoggedOut));
        assert!(!f.account("a").await.login_ok, "phai mark_logged_out");
        assert!(f.engine.cache().names().await.is_empty(), "phien phai dong");
        assert_eq!(f.used("a", Mode::Fast).await, 0, "khong claim");
    }

    /// C1 (fix-round 1, Critical — day chinh la loi truoc khi sua): phien am san co trong cache voi
    /// `probe_state: Unknown` ("trang chua tai xong", `lvc_core::classify_session`) — resume PHAI
    /// tiep tuc poll (KHONG duoc `mark_logged_out`/dong phien oan mot tai khoan con dung tot).
    #[tokio::test]
    async fn resume_probe_unknown_van_poll() {
        let f = fx(&["a"], vec![FakeStep::Poll(Ok(artifact("a", "556")))]).await;
        f.task("t1", &scene()).await;
        make_processing(&f, "t1", "a", "556").await;
        f.engine
            .cache()
            .put(
                "a",
                FakeSession {
                    account: "a".into(),
                    probe_state: SessionState::Unknown,
                    seq: 0,
                    connected: true,
                },
            )
            .await;

        let out = f.engine.resume("t1", &NoopHooks).await.unwrap();
        assert_eq!(out.account, "a");
        assert_eq!(
            f.calls(),
            vec![poll("a", "556")],
            "tai dung phien, van poll"
        );

        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Completed);
        assert!(
            f.account("a").await.login_ok,
            "Unknown KHONG duoc mark_logged_out (day la loi C1 truoc khi sua)"
        );
    }

    /// M5 (fix-round 1): pin JSON `camelCase` cua `RequeueReport` (quy uoc Task 5); them
    /// `runsFailed` (fix-round 1 final-review, C1).
    #[test]
    fn requeue_report_serde_camel_case() {
        let r = RequeueReport {
            requeued: vec!["a1".to_string()],
            resumable: vec!["b1".to_string(), "c1".to_string()],
            runs_failed: vec!["r1".to_string()],
        };
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            serde_json::json!({
                "requeued": ["a1"],
                "resumable": ["b1", "c1"],
                "runsFailed": ["r1"],
            })
        );
    }
}
