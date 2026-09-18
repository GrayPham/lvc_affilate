//! `SessionCache` — phien trinh duyet AM theo tai khoan (Task 17), port phan can thiet cua
//! `video_worker_ui.py:145` `WarmBrowserManager` (che do `profile`: moi tai khoan mot Chrome;
//! cac che do hub/tabs/wtabs v0.55 khong port — P2 mo Chrome rieng tung profile, ADR-R012).
//!
//! | Rust | Python | Ghi chu |
//! |---|---|---|
//! | [`SessionCache::take_or_launch`] | `acquire` `:190-227` | co phien + con ket noi → tai dung (`reused`); mat ket noi (`page.is_closed()` `:197`) → dong roi mo moi |
//! | [`SessionCache::put`] | `release(close=False)` | tra phien ve, ghi moc idle (`last_used` `:216`) |
//! | [`SessionCache::remove`] + driver `close` | `release(close=True)` / `close_session` | engine goi khi Dola bao mat phien / captcha / han muc / profile ban |
//! | [`SessionCache::reap`] | `cleanup_idle` `:159-166` | idle > `warm_idle` → dong; them: `!is_connected` (ruling Task 17) |
//! | [`SessionCache::spawn_reaper`] | `cleanup_idle` goi moi `acquire` | o day chay nen moi [`REAPER_TICK`] (30 s), kep xuong `warm_idle` khi ngan hon |
//! | [`SessionCache::close_all`] | `close_all` (app tat) | P3 Tauri `on_exit`; Job Object (T13) la luoi cuoi |
//!
//! **Busy**: Python giu co `busy` de `cleanup_idle` khong dong phien dang chay task (`:164`). O day
//! phien dang dung bi LAY RA khoi map (`take_*`) nen reaper khong bao gio thay no — khong can co.
//!
//! **Dong phien**: `driver.close()` cua Chromium co the mat toi 30 s (`CLOSE_WAIT`); moi ham o day
//! lay phien ra khoi map DUOI khoa roi moi `close` NGOAI khoa de khong chan `take_or_launch` cua
//! job khac.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lvc_browser::{BrowserDriver, BrowserError, LaunchOpts};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Chu ky reaper nen (brief Task 17: "moi 30 s").
pub const REAPER_TICK: Duration = Duration::from_secs(30);
/// San chu ky reaper khi `warm_idle` cuc ngan (test 100 ms) — tranh vong lap nong.
pub const MIN_REAPER_TICK: Duration = Duration::from_millis(10);

/// Chu ky reaper cho `warm_idle`: `min(REAPER_TICK, warm_idle)`, khong duoi [`MIN_REAPER_TICK`].
#[must_use]
pub fn reaper_tick(warm_idle: Duration) -> Duration {
    REAPER_TICK.min(warm_idle).max(MIN_REAPER_TICK)
}

/// Mot phien am: phien + moc bat dau idle (`last_used` `:216`).
struct Warm<S> {
    session: S,
    idle_since: Instant,
}

/// Phien trinh duyet am theo ten tai khoan. Dung chung mot `Arc` giua engine va reaper.
pub struct SessionCache<S> {
    inner: tokio::sync::Mutex<HashMap<String, Warm<S>>>,
}

impl<S> Default for SessionCache<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> SessionCache<S> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Lay phien am cua `name` ra (khong con trong cache) — `None` neu khong co.
    pub async fn take(&self, name: &str) -> Option<S> {
        self.inner.lock().await.remove(name).map(|w| w.session)
    }

    /// Tra phien ve cache, moc idle = bay gio. Neu da co phien cung ten (khong xay ra khi
    /// `Lease` giu doc quyen tai khoan — hang rao phong thu) thi tra phien CU ra de nguoi goi dong.
    pub async fn put(&self, name: &str, session: S) -> Option<S> {
        self.inner
            .lock()
            .await
            .insert(
                name.to_string(),
                Warm {
                    session,
                    idle_since: Instant::now(),
                },
            )
            .map(|w| w.session)
    }

    /// Bo phien am cua `name` khoi cache (nguoi goi tu `close`).
    pub async fn remove(&self, name: &str) -> Option<S> {
        self.take(name).await
    }

    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.is_empty()
    }

    /// Ten tai khoan dang co phien am, sap theo ten (test/UI).
    pub async fn names(&self) -> Vec<String> {
        let mut out: Vec<String> = self.inner.lock().await.keys().cloned().collect();
        out.sort();
        out
    }
}

impl<S: Send + Sync + 'static> SessionCache<S> {
    /// `acquire` `:190-227`: co phien am va driver noi con ket noi → tai dung; mat ket noi → dong
    /// (best-effort) roi `launch` moi; khong co → `launch`.
    ///
    /// # Errors
    /// Loi `launch` cua driver (nguyen van — engine phan loai `ProfileBusy`/`ExecutableNotFound`/
    /// proxy chet).
    pub async fn take_or_launch<D: BrowserDriver<Session = S>>(
        &self,
        name: &str,
        opts: &LaunchOpts,
        driver: &D,
    ) -> Result<S, BrowserError> {
        if let Some(s) = self.take(name).await {
            if driver.is_connected(&s) {
                debug!(account = name, "tai dung phien am");
                return Ok(s);
            }
            warn!(account = name, "phien am mat ket noi -> dong va mo lai");
            close_quiet(driver, name, s).await;
        }
        driver.launch(opts).await
    }

    /// Mot luot quet (`cleanup_idle` `:159-166`): dong phien idle qua `warm_idle` va phien
    /// `!is_connected` (Chrome chet/bi dong tay). Tra so phien da dong.
    pub async fn reap<D: BrowserDriver<Session = S>>(
        &self,
        driver: &D,
        warm_idle: Duration,
    ) -> usize {
        let victims: Vec<(String, S)> = {
            let mut map = self.inner.lock().await;
            let now = Instant::now();
            let names: Vec<String> = map
                .iter()
                .filter(|(_, w)| {
                    now.duration_since(w.idle_since) > warm_idle || !driver.is_connected(&w.session)
                })
                .map(|(n, _)| n.clone())
                .collect();
            names
                .into_iter()
                .filter_map(|n| map.remove(&n).map(|w| (n, w.session)))
                .collect()
        };
        let n = victims.len();
        for (name, s) in victims {
            info!(
                account = %name,
                idle_max_s = warm_idle.as_secs(),
                "dong phien am (idle/mat ket noi)"
            );
            close_quiet(driver, &name, s).await;
        }
        n
    }

    /// Dong MOI phien am (app tat — `close_all`). Tra so phien da dong.
    pub async fn close_all<D: BrowserDriver<Session = S>>(&self, driver: &D) -> usize {
        let all: Vec<(String, S)> = {
            let mut map = self.inner.lock().await;
            map.drain().map(|(n, w)| (n, w.session)).collect()
        };
        let n = all.len();
        for (name, s) in all {
            close_quiet(driver, &name, s).await;
        }
        n
    }

    /// Reaper nen: moi [`reaper_tick`]`(warm_idle)` goi [`SessionCache::reap`]. Handle tra ve de
    /// `abort()` khi tat engine (`Engine::shutdown`/`Drop`).
    pub fn spawn_reaper<D: BrowserDriver<Session = S> + 'static>(
        self: &Arc<Self>,
        driver: Arc<D>,
        warm_idle: Duration,
    ) -> JoinHandle<()> {
        let cache = Arc::clone(self);
        let tick = reaper_tick(warm_idle);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let n = cache.reap(&*driver, warm_idle).await;
                if n > 0 {
                    debug!(closed = n, "reaper: da dong phien am");
                }
            }
        })
    }
}

/// `driver.close` best-effort: loi chi `warn` (Python `close_session` cung nuot loi).
async fn close_quiet<D: BrowserDriver>(driver: &D, name: &str, s: D::Session) {
    if let Err(e) = driver.close(s).await {
        warn!(account = name, error = %e, "dong phien loi (bo qua)");
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use lvc_browser::{BrowserDriver, FakeCall, FakeDriver, FakeStep, LaunchOpts};

    use super::*;

    fn opts(name: &str) -> LaunchOpts {
        LaunchOpts {
            profile_dir: PathBuf::from("accounts").join(name),
            ..LaunchOpts::default()
        }
    }

    fn calls(driver: &FakeDriver) -> Vec<FakeCall> {
        driver.calls.lock().unwrap().clone()
    }

    fn launch_call(name: &str) -> FakeCall {
        FakeCall::Launch {
            profile_dir: PathBuf::from("accounts").join(name),
            browser: lvc_core::BrowserKind::Chromium,
        }
    }

    /// Lan dau khong co phien am -> `launch`; `put` roi lay lai -> CUNG phien (seq 0), khong
    /// `launch` them (WarmBrowserManager.acquire :190-201 `reused=True`).
    #[tokio::test]
    async fn take_or_launch_mo_moi_roi_tai_dung() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(()))]);
        let cache = SessionCache::new();
        assert!(cache.is_empty().await);

        let s = cache
            .take_or_launch("a", &opts("a"), &driver)
            .await
            .unwrap();
        assert_eq!(s.seq, 0);
        assert_eq!(calls(&driver), vec![launch_call("a")]);
        assert!(
            cache.is_empty().await,
            "phien dang dung khong nam trong cache"
        );

        assert!(cache.put("a", s).await.is_none());
        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.names().await, vec!["a".to_string()]);

        let s2 = cache
            .take_or_launch("a", &opts("a"), &driver)
            .await
            .unwrap();
        assert_eq!(s2.seq, 0, "phai la phien cu");
        assert_eq!(calls(&driver).len(), 1, "khong launch lan hai");
        assert!(cache.is_empty().await);
    }

    /// Phien am mat ket noi (`is_connected` = false) -> dong roi mo phien MOI (acquire :197-201
    /// `page.is_closed()` -> "Clean up stale session").
    #[tokio::test]
    async fn take_or_launch_bo_phien_mat_ket_noi() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))]);
        let cache = SessionCache::new();
        let mut s = cache
            .take_or_launch("a", &opts("a"), &driver)
            .await
            .unwrap();
        s.connected = false;
        cache.put("a", s).await;

        let s2 = cache
            .take_or_launch("a", &opts("a"), &driver)
            .await
            .unwrap();
        assert_eq!(s2.seq, 1, "phien moi");
        assert_eq!(
            calls(&driver),
            vec![launch_call("a"), FakeCall::Close, launch_call("a")]
        );
    }

    /// `put` cung ten khi da co phien -> tra phien CU ra de nguoi goi dong (khong ro ri Chrome).
    #[tokio::test]
    async fn put_tra_phien_cu_cung_ten() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))]);
        let cache = SessionCache::new();
        let s0 = driver.launch(&opts("a")).await.unwrap();
        let s1 = driver.launch(&opts("a")).await.unwrap();
        assert!(cache.put("a", s0).await.is_none());
        let old = cache.put("a", s1).await.expect("phai tra phien cu");
        assert_eq!(old.seq, 0);
        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.remove("a").await.map(|s| s.seq), Some(1));
        assert!(cache.remove("a").await.is_none());
    }

    /// Mot luot `reap`: dong phien idle qua `warm_idle` (`cleanup_idle` :159-166) va phien mat ket
    /// noi (ruling Task 17) — phien con tuoi + con ket noi giu nguyen.
    #[tokio::test]
    async fn reap_dong_idle_va_mat_ket_noi() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))]);
        let cache = SessionCache::new();
        let a = driver.launch(&opts("a")).await.unwrap();
        let mut b = driver.launch(&opts("b")).await.unwrap();
        b.connected = false;
        cache.put("a", a).await;
        cache.put("b", b).await;

        // warm_idle rat dai -> chi b (mat ket noi) bi dong
        assert_eq!(cache.reap(&driver, Duration::from_secs(3600)).await, 1);
        assert_eq!(cache.names().await, vec!["a".to_string()]);
        assert_eq!(calls(&driver).len(), 3, "2 launch + 1 close");

        // a idle qua 10 ms -> dong
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(cache.reap(&driver, Duration::from_millis(10)).await, 1);
        assert!(cache.is_empty().await);
        assert_eq!(calls(&driver).len(), 4);
        // khong con gi -> 0
        assert_eq!(cache.reap(&driver, Duration::from_millis(10)).await, 0);
    }

    /// `close_all` (app tat, P3 `on_exit`) dong moi phien, ke ca phien con tuoi.
    #[tokio::test]
    async fn close_all_dong_het() {
        let driver = FakeDriver::new(vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))]);
        let cache = SessionCache::new();
        cache
            .put("a", driver.launch(&opts("a")).await.unwrap())
            .await;
        cache
            .put("b", driver.launch(&opts("b")).await.unwrap())
            .await;
        assert_eq!(cache.close_all(&driver).await, 2);
        assert!(cache.is_empty().await);
        assert_eq!(
            calls(&driver)
                .iter()
                .filter(|c| **c == FakeCall::Close)
                .count(),
            2
        );
        assert_eq!(cache.close_all(&driver).await, 0);
    }

    /// Reaper nen (`spawn_reaper`, moi `REAPER_TICK` — kep xuong `warm_idle` khi ngan hon) tu
    /// dong phien idle > `warm_idle` = 100 ms; `abort()` handle thi dung.
    #[tokio::test]
    async fn spawn_reaper_dong_sau_idle() {
        let driver = Arc::new(FakeDriver::new(vec![FakeStep::Launch(Ok(()))]));
        let cache = Arc::new(SessionCache::new());
        cache
            .put("a", driver.launch(&opts("a")).await.unwrap())
            .await;
        let handle = cache.spawn_reaper(Arc::clone(&driver), Duration::from_millis(100));

        let t0 = std::time::Instant::now();
        while !cache.is_empty().await {
            assert!(
                t0.elapsed() < Duration::from_secs(3),
                "reaper khong dong phien idle sau 3 s"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            t0.elapsed() >= Duration::from_millis(80),
            "khong duoc dong TRUOC khi idle du 100 ms ({:?})",
            t0.elapsed()
        );
        assert_eq!(calls(&driver).last(), Some(&FakeCall::Close));
        handle.abort();
        assert!(handle.await.unwrap_err().is_cancelled());
    }

    #[test]
    fn reaper_tick_kep_theo_warm_idle() {
        assert_eq!(reaper_tick(Duration::from_secs(900)), REAPER_TICK);
        assert_eq!(
            reaper_tick(Duration::from_millis(100)),
            Duration::from_millis(100)
        );
        assert_eq!(reaper_tick(Duration::ZERO), MIN_REAPER_TICK);
    }

    /// Engine giu `Arc<SessionCache<D::Session>>` va chia se giua task tokio (reaper) -> `Send +
    /// Sync` voi `S: Send + Sync`.
    #[test]
    fn session_cache_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SessionCache<lvc_browser::FakeSession>>();
    }
}
