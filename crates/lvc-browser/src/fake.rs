//! `FakeDriver` (Task 14, P2): bo doi (test double) cua `BrowserDriver` dieu khien bang KICH BAN
//! (`Vec<FakeStep>`) — de engine (Task 15-17, crate khac) test duoc logic cua no (chon tai khoan,
//! xoay khi loi, ghi log...) MA KHONG can Chrome that. Luon duoc bien dich (KHONG sau cargo
//! feature/`#[cfg(test)]`): mot `#[cfg(test)]` chi co hieu luc trong build TEST cua CHINH crate
//! nay, con crate engine dung `lvc-browser` nhu mot dependency binh thuong nen can `FakeDriver`
//! la API `pub` cong khai, luon co mat.
//!
//! Kich ban chi bao boc 3 loai loi goi co re nhanh Ok/Err ma engine can dieu khien tu test:
//! `launch`/`submit`/`poll_and_download`. Cac ham con lai (`add_cookies`/`goto`/`probe_session`/
//! `screenshot`/`user_agent`/`close`) KHONG tieu thu kich ban — chung la "hanh vi mac dinh vo hai"
//! (Ok/no-op) vi engine khong can gia lap loi rieng cho tung ham (T15-17 xoay quanh
//! launch/submit/poll). Moi loi goi trait deu duoc ghi lai vao `calls` (theo THU TU) de test cua
//! engine khang dinh duoc trinh tu goi.
//!
//! Kich ban het TRUOC khi mot ham can no, hoac buoc ke tiep sai loai (vd `submit()` goi nhung
//! buoc ke tiep la `Poll(..)`), la LOI LOGIC cua test (kich ban sai) — `panic!` ngay voi thong
//! diep neu ro ham nao goi va kich ban con lai, thay vi tra loi mac dinh im lang.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use lvc_core::{BrowserKind, Cookie, Limits, Mode, SessionState};

use crate::dola::{ConversationId, DolaError, PollHooks, PollOpts, SubmitRequest, VideoArtifact};
use crate::driver::{BrowserDriver, LaunchOpts, SessionProbe};
use crate::error::BrowserError;

/// UA co dinh tra ve boi [`FakeDriver::user_agent`] — khong dai dien Chrome that nao, chi de
/// phan biet voi UA that trong log/assert cua test.
pub const FAKE_USER_AGENT: &str = "FakeUA/1";

/// Mot buoc kich ban. Chi 3 loai — xem doc module ve ly do cac ham con lai khong can buoc rieng.
#[derive(Debug)]
pub enum FakeStep {
    /// `launch()`: `Ok(())` -> tra phien moi; `Err(e)` -> tra `e` nguyen van.
    Launch(Result<(), BrowserError>),
    /// `submit()`.
    Submit(Result<ConversationId, DolaError>),
    /// `poll_and_download()`.
    Poll(Result<VideoArtifact, DolaError>),
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
}

/// Bo doi `BrowserDriver` dieu khien bang kich ban. Xem doc module.
pub struct FakeDriver {
    script: Mutex<VecDeque<FakeStep>>,
    /// Moi loi goi trait, THEO THU TU. `pub` de test doc truc tiep (`driver.calls.lock().unwrap()`).
    pub calls: Mutex<Vec<FakeCall>>,
    next_seq: AtomicU64,
}

impl FakeDriver {
    /// `steps` la kich ban DUNG THU TU se can (`launch` tieu thu buoc dau, v.v.).
    #[must_use]
    pub fn new(steps: Vec<FakeStep>) -> Self {
        Self {
            script: Mutex::new(steps.into()),
            calls: Mutex::new(Vec::new()),
            next_seq: AtomicU64::new(0),
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
}

/// Ten tai khoan tu ten file cuoi cung cua `profile_dir` (`accounts/<ten>/` -> `<ten>`); duong dan
/// rong/ket thuc bang `..`/`/` (khong co ten file) -> chuoi rong.
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
            Some(FakeStep::Launch(res)) => res.map(|()| {
                let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
                FakeSession {
                    account: account_from_profile_dir(&opts.profile_dir),
                    probe_state: SessionState::LoggedIn,
                    seq,
                }
            }),
            Some(other) => panic!(
                "FakeDriver::launch: buoc kich ban ke tiep la {other:?}, khong phai Launch(..) \
                 — kich ban con lai: {:?}",
                self.remaining_debug()
            ),
            None => panic!("FakeDriver::launch: kich ban da het truoc khi goi launch()"),
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
    ) -> Result<ConversationId, DolaError> {
        let _ = limits;
        self.record(FakeCall::Submit {
            account: s.account.clone(),
            mode: req.mode,
            duration: req.duration,
        });
        match self.pop() {
            Some(FakeStep::Submit(res)) => res,
            Some(other) => panic!(
                "FakeDriver::submit: buoc kich ban ke tiep la {other:?}, khong phai Submit(..) \
                 — kich ban con lai: {:?}",
                self.remaining_debug()
            ),
            None => panic!("FakeDriver::submit: kich ban da het truoc khi goi submit()"),
        }
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
        let _ = (s, out_dir, opts, limits, hooks);
        self.record(FakeCall::Poll {
            account: account.to_string(),
            conv: conv.clone(),
        });
        match self.pop() {
            Some(FakeStep::Poll(res)) => res,
            Some(other) => panic!(
                "FakeDriver::poll_and_download: buoc kich ban ke tiep la {other:?}, khong phai \
                 Poll(..) — kich ban con lai: {:?}",
                self.remaining_debug()
            ),
            None => panic!(
                "FakeDriver::poll_and_download: kich ban da het truoc khi goi poll_and_download()"
            ),
        }
    }

    async fn user_agent(&self, s: &FakeSession) -> Result<String, BrowserError> {
        let _ = s;
        Ok(FAKE_USER_AGENT.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use lvc_core::{Mode, Model, Ratio};

    use super::*;
    use crate::driver::DEFAULT_LAUNCH_TIMEOUT;

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

    /// Kich ban hanh phuc: `launch` -> `submit` Ok -> `poll` Ok -> `close`; cac loi goi duoc ghi
    /// lai DUNG THU TU (ke ca cac loi khong tieu thu kich ban nhu `goto`).
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
            .submit(&s, &req("acc1"), &limits)
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
        let err = driver.submit(&s, &req("acc1"), &limits).await.unwrap_err();
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
        let _ = driver.submit(&s, &req("acc1"), &limits).await;
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
        let _ = driver.submit(&s, &req("acc1"), &limits).await;
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
