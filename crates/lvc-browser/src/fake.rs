//! `FakeDriver` (Task 14, P2): bo doi (test double) cua `BrowserDriver` dieu khien bang KICH BAN
//! (`Vec<FakeStep>`) — de engine (Task 15-17, crate khac) test duoc logic cua no (chon tai khoan,
//! xoay khi loi, ghi log...) MA KHONG can Chrome that. Luon duoc bien dich (KHONG sau cargo
//! feature/`#[cfg(test)]`): mot `#[cfg(test)]` chi co hieu luc trong build TEST cua CHINH crate
//! nay, con crate engine dung `lvc-browser` nhu mot dependency binh thuong nen can `FakeDriver`
//! la API `pub` cong khai, luon co mat.
//!
//! Kich ban chi bao boc 3 loai loi goi co re nhanh Ok/Err ma engine can dieu khien tu test:
//! `launch`/`submit`/`poll_and_download`. Cac ham con lai (`add_cookies`/`goto`/`probe_session`/
//! `screenshot`/`user_agent`/`close`/`is_connected`) KHONG tieu thu kich ban — chung la "hanh vi mac dinh vo hai"
//! (Ok/no-op) vi engine khong can gia lap loi rieng cho tung ham (T15-17 xoay quanh
//! launch/submit/poll). CHI 4 loi goi vong doi (`launch`/`submit`/`poll_and_download`/`close`)
//! duoc ghi lai vao `calls` (theo THU TU) de test cua engine khang dinh duoc trinh tu goi;
//! `goto`/`add_cookies`/`probe_session`/`screenshot`/`user_agent` KHONG ghi.
//!
//! Kich ban het TRUOC khi mot ham can no, hoac buoc ke tiep sai loai (vd `submit()` goi nhung
//! buoc ke tiep la `Poll(..)`), la LOI LOGIC cua test (kich ban sai) — `panic!` ngay voi thong
//! diep neu ro ham nao goi va kich ban con lai, thay vi tra loi mac dinh im lang.
//!
//! P3 Task 4 (huy + su kien): `submit()` phat stage qua `SubmitHooks` dung thu tu nhu driver that
//! (`page` -> `mode` -> `composer` khi Fast/Pro -> `refs` khi co anh -> `prompt` -> `sending` ->
//! `generating` khi Ok) va hoi `should_cancel()` TRUOC `page` va TRUOC `prompt` (nhu `dola::submit`)
//! -> `DolaError::Cancelled`; `poll_and_download()` phat `generating` (:1494) va `downloading`
//! (:1268, chi khi Ok); [`FakeStep::PollUntilCancelled`] lap 10 ms/nhip toi khi
//! `PollHooks::should_cancel()` -> `Cancelled` (engine test "huy giua poll").
//!
//! P3 Task 4 fix-round (Minor M2/M3, review task-4-review.md finding 3): chuoi stage tren PHAI khop
//! `driver::SUBMIT_STAGES`/`POLL_STAGES` (danh sach dong dung chung voi `ChromiumDriver`) — truoc ban
//! sua nay `submit()` bo qua `composer`/`refs` nen engine test pin sai chuoi so voi driver that.
//!
//! P3 Task 12a: [`FakeDriver::endless_ok`] — "che do gia lap" dev (bat bang cargo feature
//! `fake-driver` cua `crates/lvc-app`, vd `cargo tauri dev --features fake-driver`, xem
//! `lvc-app::bootstrap`; fix-round 1 final-review M2 — KHONG PHAI bien moi truong
//! `LVC_FAKE_DRIVER=1`, ban truoc sua ghi nham) KHONG ton credit. Khac voi kich ban
//! (`FakeDriver::new`), driver nay bat dau
//! voi kich ban RONG va khong bao gio panic khi kich ban het — moi ham vong doi tra ve mac dinh
//! "luon Ok" (xem doc ham `endless_ok`). Muc dich la de UI (Task 11) thu duoc TOAN BO luong
//! submit -> poll -> tai video ma khong can Dola that.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use lvc_core::{BrowserKind, Cookie, Limits, Mode, SessionState};

use crate::dola::poll::{STAGE_DOWNLOADING, STAGE_GENERATING};
use crate::dola::submit::{
    STAGE_COMPOSER, STAGE_MODE, STAGE_PAGE, STAGE_PROMPT, STAGE_REFS, STAGE_SENDING,
};
use crate::dola::{
    ConversationId, DolaError, PollHooks, PollOpts, SubmitHooks, SubmitRequest, VideoArtifact,
};
use crate::driver::{BrowserDriver, LaunchOpts, SessionProbe};
use crate::error::BrowserError;

/// UA co dinh tra ve boi [`FakeDriver::user_agent`] — khong dai dien Chrome that nao, chi de
/// phan biet voi UA that trong log/assert cua test.
pub const FAKE_USER_AGENT: &str = "FakeUA/1";
/// Nhip hoi `should_cancel()` cua [`FakeStep::PollUntilCancelled`].
pub const POLL_UNTIL_CANCELLED_TICK: Duration = Duration::from_millis(10);
/// Task 12a: nhip hoi `should_cancel()` trong `poll_and_download` cua che do
/// [`FakeDriver::endless_ok`] (khac hang tren — day la mo phong "vong poll that", khong phai
/// "cho huy giua chung" cua `PollUntilCancelled`).
pub const ENDLESS_POLL_TICK: Duration = Duration::from_millis(100);
/// Task 12a: noi dung file placeholder [`FakeDriver::endless_ok`] ghi ra khi "tai video" xong —
/// VAI BYTE, TUYET DOI KHONG PHAI VIDEO THAT (khong trinh phat nao doc duoc) — chi de UI co file
/// ton tai tren dia de mo/hien thi trong luc thu luong khong ton credit.
const ENDLESS_PLACEHOLDER_MP4: &[u8] = b"LVC_FAKE_DRIVER_PLACEHOLDER_KHONG_PHAI_VIDEO_THAT";
/// Task 12a: `VideoArtifact::note` khi video den tu che do [`FakeDriver::endless_ok`] (khac
/// `NOTE_RAW`/`NOTE_UNWATERMARKED`/`NOTE_LINK` cua vong poll that — day la video GIA).
pub const NOTE_FAKE_DRIVER: &str = " (FakeDriver endless_ok, khong phai video that)";

/// Mot buoc kich ban. Chu yeu 3 loai (`Launch`/`Submit`/`Poll`) — xem doc module ve ly do cac ham
/// con lai khong can buoc rieng.
#[derive(Debug)]
pub enum FakeStep {
    /// `launch()`: `Ok(())` -> tra phien moi voi `probe_state: SessionState::LoggedIn` (mac dinh);
    /// `Err(e)` -> tra `e` nguyen van.
    Launch(Result<(), BrowserError>),
    /// Task 10 (`lvc-engine::accounts`, nhap cookie/verify tai khoan): nhu `Launch` nhung dat SAN
    /// `probe_state` cua phien moi thay vi mac dinh `LoggedIn` — `import_cookies`/`verify` goi
    /// `probe_session` NGAY SAU `launch()` (khong qua `submit`/`poll`) nen can kich ban duoc trang
    /// thai probe tra ve ma khong phai tu tay dung `FakeSession` (kieu chi co the chen vao noi nhan
    /// truoc, vd `SessionCache`, xem `resume.rs` test `resume_probe_logged_out_...`). `Ok(state)` ->
    /// tra phien moi voi `probe_state: state`; `Err(e)` -> nhu `Launch(Err(e))`.
    LaunchWithState(Result<SessionState, BrowserError>),
    /// `submit()`.
    Submit(Result<ConversationId, DolaError>),
    /// `poll_and_download()`.
    Poll(Result<VideoArtifact, DolaError>),
    /// P3 Task 4: `poll_and_download()` lap 10 ms/nhip toi khi `hooks.should_cancel()` -> `Cancelled`
    /// (khong bao gio tu tra video) — test "bam Dung trong luc cho video".
    PollUntilCancelled,
}

/// Mot loi goi trait da ghi lai, THEO THU TU goi — de test cua engine khang dinh trinh tu
/// (vd "launch roi submit roi poll roi close", khong phai thu tu khac).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeCall {
    Launch {
        profile_dir: PathBuf,
        browser: BrowserKind,
    },
    Submit {
        account: String,
        mode: Mode,
        duration: u16,
    },
    Poll {
        account: String,
        conv: ConversationId,
    },
    Close,
}

/// Phien gia — KHONG boc Chrome/CDP nao ca. `account` suy tu ten file cuoi cung cua
/// `LaunchOpts::profile_dir` (khop quy uoc `accounts/<ten>/` that — xem `driver::LaunchOpts`).
/// `seq`: so thu tu (bat dau 0) cua LAN `launch()` da tao ra phien nay TREN CUNG MOT `FakeDriver`
/// — engine test dung de phan biet cac phien khi cung mot tai khoan duoc launch lai (vd sau khi
/// phien truoc bi dong/mat ket noi).
#[derive(Debug)]
pub struct FakeSession {
    pub account: String,
    /// Doc boi [`FakeDriver::probe_session`] mac dinh — sua truc tiep truoc khi goi (chu ky trait
    /// chi muon `&FakeSession` nen phai sua LUC con giu quyen so huu doc quyen, tuc TRUOC khi dua
    /// phien vao ngu canh chia se voi trait).
    pub probe_state: SessionState,
    pub seq: u64,
    /// Task 17: doc boi [`FakeDriver::is_connected`] — `true` sau `launch()`; test cua engine dat
    /// `false` (truoc khi dua vao `SessionCache`) de gia lap Chrome da chet/bi dong ngoai y muon.
    pub connected: bool,
}

/// Task 12a: trang thai rieng cua che do [`FakeDriver::endless_ok`] — xem doc ham do.
struct EndlessOk {
    /// Thoi gian `poll_and_download` "cho" (tick [`ENDLESS_POLL_TICK`], kiem `should_cancel()`
    /// moi tick) truoc khi tra video gia.
    poll_delay: Duration,
    /// Dem tang dan de sinh `ConversationId` DUY NHAT (`"fake-<seq>"`) cho MOI lan `submit()`
    /// khi kich ban rong.
    next_conv_seq: AtomicU64,
}

/// Bo doi `BrowserDriver` dieu khien bang kich ban. Xem doc module.
pub struct FakeDriver {
    script: Mutex<VecDeque<FakeStep>>,
    /// Moi loi goi trait, THEO THU TU. `pub` de test doc truc tiep (`driver.calls.lock().unwrap()`).
    pub calls: Mutex<Vec<FakeCall>>,
    next_seq: AtomicU64,
    /// Task 12a: `Some` khi tao bang [`FakeDriver::endless_ok`] — het kich ban (`script` rong)
    /// KHONG con panic ma tra ve mac dinh "luon Ok" (xem doc ham do). `None` (tao bang
    /// [`FakeDriver::new`]) giu nguyen hanh vi cu: het kich ban la LOI TEST, panic ngay (doc module).
    endless: Option<EndlessOk>,
}

impl FakeDriver {
    /// `steps` la kich ban DUNG THU TU se can (`launch` tieu thu buoc dau, v.v.).
    #[must_use]
    pub fn new(steps: Vec<FakeStep>) -> Self {
        Self {
            script: Mutex::new(steps.into()),
            calls: Mutex::new(Vec::new()),
            next_seq: AtomicU64::new(0),
            endless: None,
        }
    }

    /// Task 12a (P3, dev khong ton credit): "che do gia lap" — `LVC_FAKE_DRIVER=1` qua cargo
    /// feature `fake-driver` (`lvc-app::bootstrap::make_driver`) dung ham nay thay vi
    /// `FakeDriver::new(vec![])`. Kich ban bat dau RONG; khi mot ham vong doi
    /// (`launch`/`submit`/`poll_and_download`) can buoc ke tiep ma kich ban da het, KHONG panic —
    /// tra ve mac dinh:
    /// - `launch` -> phien moi (nhu `Launch(Ok(()))`).
    /// - `submit` -> `Ok(ConversationId("fake-<seq>"))` (seq tang dan, DUY NHAT tren driver nay).
    /// - `poll_and_download` -> cho `poll_delay` (tick [`ENDLESS_POLL_TICK`], kiem
    ///   `should_cancel()` moi tick -> `DolaError::Cancelled` neu nguoi dung bam Dung), roi ghi
    ///   file placeholder VAI BYTE (KHONG PHAI VIDEO THAT, xem [`ENDLESS_PLACEHOLDER_MP4`]) tai
    ///   `<out_dir>/<account>_<conv>.mp4` va tra `Ok(VideoArtifact { .. })`.
    /// - `probe_session`/`close`/`user_agent`/... giu nguyen hanh vi mac dinh san co (khong tieu
    ///   thu kich ban — xem doc module).
    ///
    /// Neu can gia lap LOI (vd `LoggedOut`/`Credit`) o mot buoc cu the, dung [`FakeDriver::new`]
    /// voi kich ban nhu binh thuong — che do nay chi danh cho "moi thu deu Ok" (thu luong UI
    /// day du, khong ton credit).
    #[must_use]
    pub fn endless_ok(poll_delay: Duration) -> Self {
        Self {
            script: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
            next_seq: AtomicU64::new(0),
            endless: Some(EndlessOk {
                poll_delay,
                next_conv_seq: AtomicU64::new(0),
            }),
        }
    }

    fn record(&self, call: FakeCall) {
        self.calls.lock().unwrap().push(call);
    }

    /// Lay buoc ke tiep (khong xet loai) — `None` = kich ban da het.
    fn pop(&self) -> Option<FakeStep> {
        self.script.lock().unwrap().pop_front()
    }

    /// Cac buoc CON LAI (chi de in vao thong diep panic — khong tieu thu).
    fn remaining_debug(&self) -> Vec<String> {
        self.script
            .lock()
            .unwrap()
            .iter()
            .map(|s| format!("{s:?}"))
            .collect()
    }

    /// Dung chung boi `Launch`/`LaunchWithState` va nhanh [`FakeDriver::endless_ok`] cua
    /// `launch()`: phien moi voi `probe_state` cho truoc, `seq` tang dan (xem doc truong
    /// `FakeSession::seq`).
    fn new_session(&self, opts: &LaunchOpts, probe_state: SessionState) -> FakeSession {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        FakeSession {
            account: account_from_profile_dir(&opts.profile_dir),
            probe_state,
            seq,
            connected: true,
        }
    }

    /// Task 12a: nhanh "het kich ban NHUNG dang `endless_ok`" cua `poll_and_download` (xem doc
    /// [`FakeDriver::endless_ok`]) — cho `poll_delay` (tick [`ENDLESS_POLL_TICK`], kiem
    /// `should_cancel()` moi tick), roi ghi file placeholder va tra `VideoArtifact` gia.
    async fn poll_endless_ok(
        &self,
        account: &str,
        conv: &ConversationId,
        out_dir: &Path,
        endless: &EndlessOk,
        hooks: &dyn PollHooks,
    ) -> Result<VideoArtifact, DolaError> {
        hooks.on_stage(STAGE_GENERATING);
        let mut waited = Duration::ZERO;
        while waited < endless.poll_delay {
            if hooks.should_cancel() {
                return Err(DolaError::Cancelled);
            }
            let tick = ENDLESS_POLL_TICK.min(endless.poll_delay.saturating_sub(waited));
            tokio::time::sleep(tick).await;
            waited += tick;
        }
        if hooks.should_cancel() {
            return Err(DolaError::Cancelled);
        }
        std::fs::create_dir_all(out_dir).map_err(BrowserError::Io)?;
        let local_path = out_dir.join(format!("{account}_{}.mp4", conv.0));
        std::fs::write(&local_path, ENDLESS_PLACEHOLDER_MP4).map_err(BrowserError::Io)?;
        hooks.on_stage(STAGE_DOWNLOADING);
        Ok(VideoArtifact {
            video_url: format!("fake://{}", conv.0),
            local_path,
            conversation_id: conv.clone(),
            account: account.to_string(),
            watermarked: false,
            bytes: ENDLESS_PLACEHOLDER_MP4.len() as u64,
            note: NOTE_FAKE_DRIVER,
        })
    }
}

/// Ten tai khoan tu ten file cuoi cung cua `profile_dir` (`accounts/<ten>/` -> `<ten>`; dau `/`
/// cuoi duoc `Path::file_name` bo qua); duong dan rong hoac ket thuc bang `..` -> chuoi rong.
fn account_from_profile_dir(profile_dir: &Path) -> String {
    profile_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[async_trait]
impl BrowserDriver for FakeDriver {
    type Session = FakeSession;

    async fn launch(&self, opts: &LaunchOpts) -> Result<FakeSession, BrowserError> {
        self.record(FakeCall::Launch {
            profile_dir: opts.profile_dir.clone(),
            browser: opts.browser,
        });
        match self.pop() {
            Some(FakeStep::Launch(res)) => {
                res.map(|()| self.new_session(opts, SessionState::LoggedIn))
            }
            Some(FakeStep::LaunchWithState(res)) => res.map(|state| self.new_session(opts, state)),
            Some(other) => panic!(
                "FakeDriver::launch: buoc kich ban ke tiep la {other:?}, khong phai Launch(..)/\
                 LaunchWithState(..) — kich ban con lai: {:?}",
                self.remaining_debug()
            ),
            // Task 12a: che do endless_ok — kich ban rong khong con la loi, tra phien moi.
            None => match &self.endless {
                Some(_) => Ok(self.new_session(opts, SessionState::LoggedIn)),
                None => panic!("FakeDriver::launch: kich ban da het truoc khi goi launch()"),
            },
        }
    }

    async fn add_cookies(&self, s: &FakeSession, cookies: &[Cookie]) -> Result<(), BrowserError> {
        let _ = (s, cookies);
        Ok(())
    }

    async fn goto(&self, s: &FakeSession, url: &str) -> Result<(), BrowserError> {
        let _ = (s, url);
        Ok(())
    }

    async fn probe_session(&self, s: &FakeSession) -> Result<SessionProbe, BrowserError> {
        let logged_in = s.probe_state == SessionState::LoggedIn;
        Ok(SessionProbe {
            has_sessionid: logged_in,
            texts: Vec::new(),
            composer: logged_in,
            url: String::new(),
            state: s.probe_state,
        })
    }

    async fn screenshot(&self, s: &FakeSession, path: &Path) -> Result<(), BrowserError> {
        let _ = (s, path);
        Ok(())
    }

    async fn close(&self, s: FakeSession) -> Result<(), BrowserError> {
        let _ = s;
        self.record(FakeCall::Close);
        Ok(())
    }

    async fn submit(
        &self,
        s: &FakeSession,
        req: &SubmitRequest,
        limits: &Limits,
        hooks: &dyn SubmitHooks,
    ) -> Result<ConversationId, DolaError> {
        let _ = limits;
        self.record(FakeCall::Submit {
            account: s.account.clone(),
            mode: req.mode,
            duration: req.duration,
        });
        // Tieu thu buoc TRUOC (kich ban mo ta Dola se tra gi; huy cat ngang truoc khi toi do).
        let res = match self.pop() {
            Some(FakeStep::Submit(res)) => res,
            Some(other) => panic!(
                "FakeDriver::submit: buoc kich ban ke tiep la {other:?}, khong phai Submit(..) \
                 — kich ban con lai: {:?}",
                self.remaining_debug()
            ),
            // Task 12a: che do endless_ok — kich ban rong khong con la loi, sinh id DUY NHAT.
            None => match &self.endless {
                Some(endless) => {
                    let seq = endless.next_conv_seq.fetch_add(1, Ordering::Relaxed);
                    Ok(ConversationId(format!("fake-{seq}")))
                }
                None => panic!("FakeDriver::submit: kich ban da het truoc khi goi submit()"),
            },
        };
        // P3 Task 4: cung diem kiem/stage nhu `dola::submit` (R4: truoc page, truoc prompt).
        if hooks.should_cancel() {
            return Err(DolaError::Cancelled);
        }
        hooks.on_stage(STAGE_PAGE);
        hooks.on_stage(STAGE_MODE);
        // Fix-round Minor M3: cung thu tu ChromiumDriver — `composer` chi Fast/Pro (submit.rs:496,
        // skill.rs:126), `refs` chi khi co anh tham chieu (submit.rs:554); CA HAI truoc diem kiem
        // huy "truoc prompt" (nhu `before_prompt` that).
        if matches!(req.mode, Mode::Fast | Mode::Pro) {
            hooks.on_stage(STAGE_COMPOSER);
        }
        if !req.reference_images.is_empty() {
            hooks.on_stage(STAGE_REFS);
        }
        if hooks.should_cancel() {
            return Err(DolaError::Cancelled);
        }
        hooks.on_stage(STAGE_PROMPT);
        hooks.on_stage(STAGE_SENDING);
        if res.is_ok() {
            hooks.on_stage(STAGE_GENERATING);
        }
        res
    }

    #[allow(clippy::too_many_arguments)] // khop chu ky trait (BrowserDriver::poll_and_download)
    async fn poll_and_download(
        &self,
        s: &FakeSession,
        account: &str,
        conv: &ConversationId,
        out_dir: &Path,
        opts: &PollOpts,
        limits: &Limits,
        hooks: &dyn PollHooks,
    ) -> Result<VideoArtifact, DolaError> {
        let _ = (s, out_dir, opts, limits);
        self.record(FakeCall::Poll {
            account: account.to_string(),
            conv: conv.clone(),
        });
        match self.pop() {
            Some(FakeStep::Poll(res)) => {
                hooks.on_stage(STAGE_GENERATING); // :1494
                if res.is_ok() {
                    hooks.on_stage(STAGE_DOWNLOADING); // :1268
                }
                res
            }
            Some(FakeStep::PollUntilCancelled) => {
                hooks.on_stage(STAGE_GENERATING);
                loop {
                    if hooks.should_cancel() {
                        return Err(DolaError::Cancelled);
                    }
                    tokio::time::sleep(POLL_UNTIL_CANCELLED_TICK).await;
                }
            }
            Some(other) => panic!(
                "FakeDriver::poll_and_download: buoc kich ban ke tiep la {other:?}, khong phai \
                 Poll(..) — kich ban con lai: {:?}",
                self.remaining_debug()
            ),
            // Task 12a: che do endless_ok — kich ban rong khong con la loi, cho roi tra video gia.
            None => match &self.endless {
                Some(endless) => {
                    self.poll_endless_ok(account, conv, out_dir, endless, hooks)
                        .await
                }
                None => panic!(
                    "FakeDriver::poll_and_download: kich ban da het truoc khi goi \
                     poll_and_download()"
                ),
            },
        }
    }

    async fn user_agent(&self, s: &FakeSession) -> Result<String, BrowserError> {
        let _ = s;
        Ok(FAKE_USER_AGENT.to_string())
    }

    /// Task 17: truong `connected` cua phien gia (khong ghi vao `calls`).
    fn is_connected(&self, s: &FakeSession) -> bool {
        s.connected
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use lvc_core::{Mode, Model, Ratio};

    use super::*;
    use crate::dola::submit::{NoopSubmitHooks, SubmitHooks};
    use crate::driver::DEFAULT_LAUNCH_TIMEOUT;

    /// P3 Task 4: ghi lai stage (ca `SubmitHooks` lan `PollHooks`) + co huy chia se; `cancel_at`
    /// = thay stage nay thi bat co (mo phong nguoi dung bam Dung dung luc do).
    #[derive(Default)]
    struct RecHooks {
        stages: Mutex<Vec<String>>,
        cancel: AtomicBool,
        cancel_at: Option<&'static str>,
    }

    impl RecHooks {
        fn stages(&self) -> Vec<String> {
            self.stages.lock().unwrap().clone()
        }
        fn see(&self, stage: &str) {
            self.stages.lock().unwrap().push(stage.to_string());
            if self.cancel_at == Some(stage) {
                self.cancel.store(true, Ordering::SeqCst);
            }
        }
    }

    impl SubmitHooks for RecHooks {
        fn on_stage(&self, stage: &str) {
            self.see(stage);
        }
        fn should_cancel(&self) -> bool {
            self.cancel.load(Ordering::SeqCst)
        }
    }

    impl PollHooks for RecHooks {
        fn on_stage(&self, stage: &str) {
            self.see(stage);
        }
        fn should_cancel(&self) -> bool {
            self.cancel.load(Ordering::SeqCst)
        }
    }

    fn popts() -> PollOpts {
        PollOpts {
            timeout: Duration::from_secs(1),
            mode: Mode::Fast,
            duration: 10,
            ignore_videos: std::collections::HashSet::default(),
            ignore_links: std::collections::HashSet::default(),
            accept_links: false,
        }
    }

    fn opts(account: &str) -> LaunchOpts {
        LaunchOpts {
            profile_dir: PathBuf::from("accounts").join(account),
            ..LaunchOpts::default()
        }
    }

    fn req(account: &str) -> SubmitRequest {
        SubmitRequest {
            account: account.to_string(),
            prompt: "p".into(),
            mode: Mode::Fast,
            ratio: Some(Ratio::R16x9),
            duration: 10,
            model: Model::Seedance20,
            reference_images: Vec::new(),
        }
    }

    fn artifact(account: &str, conv: &ConversationId) -> VideoArtifact {
        VideoArtifact {
            video_url: "https://example.invalid/v.mp4".into(),
            local_path: PathBuf::from("out.mp4"),
            conversation_id: conv.clone(),
            account: account.to_string(),
            watermarked: false,
            bytes: 123,
            note: crate::dola::poll::NOTE_RAW,
        }
    }

    /// Kich ban hanh phuc: `launch` -> `submit` Ok -> `poll` Ok -> `close`; 4 loi goi vong doi duoc
    /// ghi lai DUNG THU TU (`goto` khong ghi — xem doc module).
    #[tokio::test]
    async fn kich_ban_hanh_phuc_ghi_lai_dung_thu_tu() {
        let conv = ConversationId("123".into());
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(conv.clone())),
            FakeStep::Poll(Ok(artifact("acc1", &conv))),
        ]);
        let limits = Limits::default();

        let s = driver.launch(&opts("acc1")).await.expect("launch Ok");
        assert_eq!(s.account, "acc1");
        assert_eq!(s.seq, 0);

        driver.goto(&s, "https://www.dola.com/chat").await.unwrap();

        let got_conv = driver
            .submit(&s, &req("acc1"), &limits, &NoopSubmitHooks)
            .await
            .expect("submit Ok");
        assert_eq!(got_conv, conv);

        let out_dir = PathBuf::from("out");
        let popt = PollOpts {
            timeout: std::time::Duration::from_secs(1),
            mode: Mode::Fast,
            duration: 10,
            ignore_videos: std::collections::HashSet::default(),
            ignore_links: std::collections::HashSet::default(),
            accept_links: false,
        };
        let hooks = crate::dola::NoopHooks;
        let got_art = driver
            .poll_and_download(&s, "acc1", &conv, &out_dir, &popt, &limits, &hooks)
            .await
            .expect("poll Ok");
        assert_eq!(got_art.conversation_id, conv);

        driver.close(s).await.unwrap();

        let calls = driver.calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec![
                FakeCall::Launch {
                    profile_dir: opts("acc1").profile_dir,
                    browser: BrowserKind::Chromium,
                },
                FakeCall::Submit {
                    account: "acc1".into(),
                    mode: Mode::Fast,
                    duration: 10,
                },
                FakeCall::Poll {
                    account: "acc1".into(),
                    conv: conv.clone(),
                },
                FakeCall::Close,
            ]
        );
    }

    /// `Launch(Err(ProfileBusy))` phai loi ra nguyen van tu `launch()`, khong bi nuot/doi loai.
    #[tokio::test]
    async fn launch_err_profile_busy_lo_ra_nguyen_van() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Err(BrowserError::ProfileBusy(
            "dang bi Chrome khac dung".into(),
        )))]);
        let err = driver.launch(&opts("acc1")).await.unwrap_err();
        assert!(matches!(err, BrowserError::ProfileBusy(_)), "{err:?}");
    }

    /// `Submit(Err(LoggedOut))` phai loi ra nguyen van tu `submit()`.
    #[tokio::test]
    async fn submit_err_logged_out_lo_ra_nguyen_van() {
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Err(DolaError::LoggedOut {
                account: "acc1".into(),
                why: "no sessionid cookie".into(),
                revoked: false,
            })),
        ]);
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let limits = Limits::default();
        let err = driver
            .submit(&s, &req("acc1"), &limits, &NoopSubmitHooks)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, DolaError::LoggedOut { account, .. } if account == "acc1"),
            "{err:?}"
        );
    }

    /// Kich ban het truoc khi `submit()` can buoc ke tiep -> panic co thong diep ro rang ten ham.
    #[tokio::test]
    #[should_panic(expected = "FakeDriver::submit: kich ban da het truoc khi goi submit()")]
    async fn kich_ban_het_thi_panic_ro_thong_diep() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(()))]);
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let limits = Limits::default();
        let _ = driver
            .submit(&s, &req("acc1"), &limits, &NoopSubmitHooks)
            .await;
    }

    /// Buoc ke tiep sai LOAI (mong `Submit`, kich ban lai la `Poll`) cung phai panic ro rang.
    #[tokio::test]
    #[should_panic(expected = "buoc kich ban ke tiep la Poll")]
    async fn kich_ban_sai_loai_thi_panic_ro_thong_diep() {
        let conv = ConversationId("1".into());
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Poll(Ok(artifact("acc1", &conv))),
        ]);
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let limits = Limits::default();
        let _ = driver
            .submit(&s, &req("acc1"), &limits, &NoopSubmitHooks)
            .await;
    }

    /// `account` cua `FakeSession` suy tu ten file cuoi cung cua `profile_dir`.
    #[tokio::test]
    async fn account_suy_tu_ten_thu_muc_profile() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(()))]);
        let s = driver
            .launch(&LaunchOpts {
                profile_dir: PathBuf::from(r"C:\data\accounts\acc_042"),
                launch_timeout: DEFAULT_LAUNCH_TIMEOUT,
                ..LaunchOpts::default()
            })
            .await
            .unwrap();
        assert_eq!(s.account, "acc_042");
    }

    /// Task 10: `LaunchWithState` dat san `probe_state` cua phien MOI (khong can sua tay sau khi
    /// `launch()` tra ve — `import_cookies`/`verify` goi `probe_session` ngay sau `launch()`, khong
    /// co diem nao khac de chen `FakeSession` da sua nhu cach `resume.rs` lam qua `SessionCache`).
    #[tokio::test]
    async fn launch_with_state_dat_san_probe_state() {
        let driver = FakeDriver::new(vec![FakeStep::LaunchWithState(Ok(SessionState::LoggedOut))]);
        let s = driver.launch(&opts("acc1")).await.unwrap();
        assert_eq!(s.probe_state, SessionState::LoggedOut);
        let p = driver.probe_session(&s).await.unwrap();
        assert_eq!(p.state, SessionState::LoggedOut);
        assert!(!p.has_sessionid);
    }

    /// `LaunchWithState(Err(..))` loi ra nguyen van nhu `Launch(Err(..))`.
    #[tokio::test]
    async fn launch_with_state_err_lo_ra_nguyen_van() {
        let driver = FakeDriver::new(vec![FakeStep::LaunchWithState(Err(
            BrowserError::ProfileBusy("dang bi Chrome khac dung".into()),
        ))]);
        let err = driver.launch(&opts("acc1")).await.unwrap_err();
        assert!(matches!(err, BrowserError::ProfileBusy(_)), "{err:?}");
    }

    /// `probe_session` mac dinh `LoggedIn`; sua `probe_state` truoc khi goi thi doi theo.
    #[tokio::test]
    async fn probe_session_mac_dinh_logged_in_va_sua_duoc() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(()))]);
        let mut s = driver.launch(&opts("acc1")).await.unwrap();
        let p = driver.probe_session(&s).await.unwrap();
        assert_eq!(p.state, SessionState::LoggedIn);
        assert!(p.has_sessionid);

        s.probe_state = SessionState::LoggedOut;
        let p = driver.probe_session(&s).await.unwrap();
        assert_eq!(p.state, SessionState::LoggedOut);
        assert!(!p.has_sessionid);
    }

    /// Task 17: `is_connected` doc truong `connected` cua phien gia (mac dinh `true` sau
    /// `launch()`); test cua engine (`SessionCache` reaper) dat `false` de gia lap Chrome da chet.
    #[tokio::test]
    async fn is_connected_theo_truong_connected() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(()))]);
        let mut s = driver.launch(&opts("acc1")).await.unwrap();
        assert!(s.connected);
        assert!(driver.is_connected(&s));
        s.connected = false;
        assert!(!driver.is_connected(&s));
    }

    #[tokio::test]
    async fn user_agent_co_dinh() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(()))]);
        let s = driver.launch(&opts("acc1")).await.unwrap();
        assert_eq!(driver.user_agent(&s).await.unwrap(), FAKE_USER_AGENT);
    }

    /// `seq` tang dan MOI lan `launch()` tren CUNG MOT driver (khong reset theo tai khoan).
    #[tokio::test]
    async fn seq_tang_dan_moi_lan_launch() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))]);
        let s1 = driver.launch(&opts("acc1")).await.unwrap();
        let s2 = driver.launch(&opts("acc2")).await.unwrap();
        assert_eq!(s1.seq, 0);
        assert_eq!(s2.seq, 1);
    }

    /// P3 Task 4: `submit()` goi `SubmitHooks::on_stage` theo thu tu Python (`page` :2433, `mode`
    /// :2476, `prompt` :2122, `sending` :2539, `generating` :2603 khi co conv id); `poll_and_download()`
    /// goi `PollHooks::on_stage` `generating` (:1494) roi `downloading` (:1268) khi Ok.
    #[tokio::test]
    async fn submit_va_poll_phat_stage_dung_thu_tu() {
        let conv = ConversationId("123".into());
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(conv.clone())),
            FakeStep::Poll(Ok(artifact("acc1", &conv))),
        ]);
        let limits = Limits::default();
        let hooks = RecHooks::default();
        let s = driver.launch(&opts("acc1")).await.unwrap();
        driver
            .submit(&s, &req("acc1"), &limits, &hooks)
            .await
            .unwrap();
        assert_eq!(
            hooks.stages(),
            vec![
                "page",
                "mode",
                "composer",
                "prompt",
                "sending",
                "generating"
            ]
        );
        driver
            .poll_and_download(
                &s,
                "acc1",
                &conv,
                Path::new("out"),
                &popts(),
                &limits,
                &hooks,
            )
            .await
            .unwrap();
        assert_eq!(
            hooks.stages(),
            vec![
                "page",
                "mode",
                "composer",
                "prompt",
                "sending",
                "generating",
                "generating",
                "downloading"
            ]
        );
    }

    /// P3 Task 4: `Submit(Err)` -> khong phat `generating`; `Poll(Err)` -> khong phat `downloading`.
    #[tokio::test]
    async fn submit_loi_va_poll_loi_khong_phat_stage_sau() {
        let conv = ConversationId("123".into());
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Err(DolaError::Credit("het".into()))),
            FakeStep::Poll(Err(DolaError::Timeout("t".into()))),
        ]);
        let limits = Limits::default();
        let hooks = RecHooks::default();
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let err = driver
            .submit(&s, &req("acc1"), &limits, &hooks)
            .await
            .unwrap_err();
        assert!(matches!(err, DolaError::Credit(_)), "{err:?}");
        assert_eq!(
            hooks.stages(),
            vec!["page", "mode", "composer", "prompt", "sending"]
        );
        let err = driver
            .poll_and_download(
                &s,
                "acc1",
                &conv,
                Path::new("out"),
                &popts(),
                &limits,
                &hooks,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DolaError::Timeout(_)), "{err:?}");
        assert_eq!(
            hooks.stages(),
            vec![
                "page",
                "mode",
                "composer",
                "prompt",
                "sending",
                "generating"
            ]
        );
    }

    /// P3 Task 4 (R4 "truoc submit"): `should_cancel()` true khi toi `prompt` -> `Cancelled`, KHONG
    /// phat `prompt`/`sending`/`generating` (chua gui, chua ton credit); buoc `Submit(..)` van bi
    /// tieu thu (kich ban mo ta Dola se tra gi — huy cat ngang truoc do).
    #[tokio::test]
    async fn submit_huy_truoc_prompt_bao_cancelled() {
        let conv = ConversationId("123".into());
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(conv.clone())),
        ]);
        let limits = Limits::default();
        let hooks = RecHooks {
            cancel_at: Some("mode"),
            ..RecHooks::default()
        };
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let err = driver
            .submit(&s, &req("acc1"), &limits, &hooks)
            .await
            .unwrap_err();
        assert!(matches!(err, DolaError::Cancelled), "{err:?}");
        // `composer` da phat TRUOC diem kiem "truoc prompt" (Fast -> composer luon phat) — huy o
        // day KHONG con kip chan no, giong driver that (submit_fast mo composer roi moi toi
        // before_prompt).
        assert_eq!(hooks.stages(), vec!["page", "mode", "composer"]);
        assert!(
            driver.script.lock().unwrap().is_empty(),
            "buoc Submit da tieu thu"
        );
        assert_eq!(driver.calls.lock().unwrap().len(), 2, "launch + submit");
    }

    /// P3 Task 4 (R4 "truoc launch/page"): co huy da bat TRUOC khi goi `submit()` -> `Cancelled`
    /// ngay, khong phat stage nao.
    #[tokio::test]
    async fn submit_huy_truoc_page_khong_phat_stage() {
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(ConversationId("1".into()))),
        ]);
        let limits = Limits::default();
        let hooks = RecHooks::default();
        hooks.cancel.store(true, Ordering::SeqCst);
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let err = driver
            .submit(&s, &req("acc1"), &limits, &hooks)
            .await
            .unwrap_err();
        assert!(matches!(err, DolaError::Cancelled), "{err:?}");
        assert!(hooks.stages().is_empty());
    }

    /// P3 Task 4: `FakeStep::PollUntilCancelled` lap (10 ms/nhip) toi khi `should_cancel()` true ->
    /// `Cancelled`; phat `generating` mot lan, KHONG `downloading`.
    #[tokio::test]
    async fn poll_until_cancelled_cho_toi_khi_huy() {
        let conv = ConversationId("123".into());
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(())), FakeStep::PollUntilCancelled]);
        let limits = Limits::default();
        let hooks = Arc::new(RecHooks::default());
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let h2 = Arc::clone(&hooks);
        let flip = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            h2.cancel.store(true, Ordering::SeqCst);
        });
        let t0 = std::time::Instant::now();
        let err = driver
            .poll_and_download(
                &s,
                "acc1",
                &conv,
                Path::new("out"),
                &popts(),
                &limits,
                hooks.as_ref(),
            )
            .await
            .unwrap_err();
        flip.await.unwrap();
        assert!(matches!(err, DolaError::Cancelled), "{err:?}");
        assert!(
            t0.elapsed() >= Duration::from_millis(50),
            "phai cho toi khi huy ({:?})",
            t0.elapsed()
        );
        assert_eq!(hooks.stages(), vec!["generating"]);
        assert_eq!(
            *driver.calls.lock().unwrap(),
            vec![
                FakeCall::Launch {
                    profile_dir: opts("acc1").profile_dir,
                    browser: BrowserKind::Chromium,
                },
                FakeCall::Poll {
                    account: "acc1".into(),
                    conv,
                },
            ]
        );
    }

    /// P3 Task 4 fix-round (Minor M2/M3): moi stage `FakeDriver` phat (submit CO anh tham chieu +
    /// poll) deu nam trong danh sach dong `driver::SUBMIT_STAGES`/`POLL_STAGES` — cach "khong can
    /// grep" de bat truong hop them stage moi ma quen dua vao danh sach (xem
    /// `driver::submit_stages_va_poll_stages_la_tap_dong`).
    #[tokio::test]
    async fn stage_phat_ra_nam_trong_danh_sach_dong() {
        let conv = ConversationId("1".into());
        let driver = FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(conv.clone())),
            FakeStep::Poll(Ok(artifact("acc1", &conv))),
        ]);
        let limits = Limits::default();
        let hooks = RecHooks::default();
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let mut r = req("acc1");
        r.reference_images.push(PathBuf::from("ref.png")); // kich hoat stage `refs`
        driver.submit(&s, &r, &limits, &hooks).await.unwrap();
        driver
            .poll_and_download(
                &s,
                "acc1",
                &conv,
                Path::new("out"),
                &popts(),
                &limits,
                &hooks,
            )
            .await
            .unwrap();
        let seen = hooks.stages();
        assert!(
            seen.contains(&"composer".to_string()) && seen.contains(&"refs".to_string()),
            "test phai kich hoat ca composer lan refs: {seen:?}"
        );
        for stage in &seen {
            assert!(
                crate::driver::SUBMIT_STAGES.contains(&stage.as_str())
                    || crate::driver::POLL_STAGES.contains(&stage.as_str()),
                "stage '{stage}' khong nam trong SUBMIT_STAGES/POLL_STAGES: {seen:?}"
            );
        }
    }

    /// Task 12a: `endless_ok` — kich ban RONG, hai job submit/poll LIEN TIEP tren CUNG driver deu
    /// thanh cong voi `ConversationId` KHAC NHAU (dev thu 2 video lien tiep khong bi trung
    /// id/file); moi job ghi file placeholder rieng, `note` bao ro day la `FakeDriver`.
    #[tokio::test]
    async fn endless_ok_hai_job_lien_tiep_id_khac_nhau() {
        let dir = tempfile::tempdir().unwrap();
        let driver = FakeDriver::endless_ok(Duration::from_millis(5));
        let limits = Limits::default();
        let s = driver.launch(&opts("acc1")).await.unwrap();

        let hooks1 = RecHooks::default();
        let conv1 = driver
            .submit(&s, &req("acc1"), &limits, &hooks1)
            .await
            .unwrap();
        let art1 = driver
            .poll_and_download(&s, "acc1", &conv1, dir.path(), &popts(), &limits, &hooks1)
            .await
            .unwrap();
        assert_eq!(art1.conversation_id, conv1);
        assert!(
            art1.local_path.is_file(),
            "phai ghi file placeholder: {:?}",
            art1.local_path
        );
        assert_eq!(art1.note, NOTE_FAKE_DRIVER);
        assert!(!art1.watermarked);

        let hooks2 = RecHooks::default();
        let conv2 = driver
            .submit(&s, &req("acc1"), &limits, &hooks2)
            .await
            .unwrap();
        let art2 = driver
            .poll_and_download(&s, "acc1", &conv2, dir.path(), &popts(), &limits, &hooks2)
            .await
            .unwrap();

        assert_ne!(
            conv1, conv2,
            "hai lan submit lien tiep phai sinh id KHAC nhau"
        );
        assert_ne!(
            art1.local_path, art2.local_path,
            "moi job phai co file rieng"
        );
    }

    /// Task 12a: `endless_ok` — huy (`should_cancel()` true) GIUA vong cho poll phai tra
    /// `Cancelled` NHANH (vai tick [`ENDLESS_POLL_TICK`]), KHONG cho het `poll_delay`; KHONG ghi
    /// file placeholder khi bi huy giua chung.
    #[tokio::test]
    async fn endless_ok_huy_giua_poll_tra_cancelled_nhanh() {
        let dir = tempfile::tempdir().unwrap();
        let driver = FakeDriver::endless_ok(Duration::from_secs(5));
        let limits = Limits::default();
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let submit_hooks = RecHooks::default();
        let conv = driver
            .submit(&s, &req("acc1"), &limits, &submit_hooks)
            .await
            .unwrap();

        let hooks = Arc::new(RecHooks::default());
        let h2 = Arc::clone(&hooks);
        let flip = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            h2.cancel.store(true, Ordering::SeqCst);
        });
        let t0 = std::time::Instant::now();
        let err = driver
            .poll_and_download(
                &s,
                "acc1",
                &conv,
                dir.path(),
                &popts(),
                &limits,
                hooks.as_ref(),
            )
            .await
            .unwrap_err();
        flip.await.unwrap();
        assert!(matches!(err, DolaError::Cancelled), "{err:?}");
        assert!(
            t0.elapsed() < Duration::from_secs(1),
            "phai huy NHANH (~150ms), khong cho het poll_delay (5s): {:?}",
            t0.elapsed()
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "khong duoc ghi file khi bi huy giua chung"
        );
    }

    /// Task 12a (tuong tu `stage_phat_ra_nam_trong_danh_sach_dong`): stage che do `endless_ok`
    /// phat ra (submit CO anh tham chieu + poll) deu nam trong `driver::SUBMIT_STAGES`/
    /// `POLL_STAGES` — che do gia lap KHONG duoc "troi" ten stage rieng so voi driver that/kich
    /// ban, va phai DUNG THU TU nhu kich ban that (`submit_va_poll_phat_stage_dung_thu_tu`).
    #[tokio::test]
    async fn endless_ok_stage_phat_ra_nam_trong_danh_sach_dong() {
        let dir = tempfile::tempdir().unwrap();
        let driver = FakeDriver::endless_ok(Duration::from_millis(5));
        let limits = Limits::default();
        let hooks = RecHooks::default();
        let s = driver.launch(&opts("acc1")).await.unwrap();
        let mut r = req("acc1");
        r.reference_images.push(PathBuf::from("ref.png")); // kich hoat stage `refs`
        let conv = driver.submit(&s, &r, &limits, &hooks).await.unwrap();
        driver
            .poll_and_download(&s, "acc1", &conv, dir.path(), &popts(), &limits, &hooks)
            .await
            .unwrap();
        let seen = hooks.stages();
        for stage in &seen {
            assert!(
                crate::driver::SUBMIT_STAGES.contains(&stage.as_str())
                    || crate::driver::POLL_STAGES.contains(&stage.as_str()),
                "stage '{stage}' khong nam trong SUBMIT_STAGES/POLL_STAGES: {seen:?}"
            );
        }
        assert_eq!(
            seen,
            vec![
                "page",
                "mode",
                "composer",
                "refs",
                "prompt",
                "sending",
                "generating",
                "generating",
                "downloading"
            ]
        );
    }

    /// Kiem tra an toan doi tuong (object safety) cua `BrowserDriver` — cung la dieu kien de
    /// engine (Task 15-17) giu `Arc<dyn BrowserDriver<Session = FakeSession>>` trong test cua no.
    /// Bien dich duoc la du (Ruling R9: trait van o muc CAO, khong them evaluate/click).
    #[allow(dead_code)]
    fn assert_object_safe(_: &dyn BrowserDriver<Session = FakeSession>) {}

    #[tokio::test]
    async fn trait_object_qua_arc_bien_dich_va_goi_duoc() {
        let driver: Arc<dyn BrowserDriver<Session = FakeSession>> =
            Arc::new(FakeDriver::new(vec![FakeStep::Launch(Ok(()))]));
        let s = driver.launch(&opts("acc1")).await.unwrap();
        assert_eq!(driver.user_agent(&s).await.unwrap(), FAKE_USER_AGENT);
    }
}
