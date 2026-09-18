//! `DynamicLimiter` — gioi han so job chay dong thoi, co the doi lai LUC DANG CHAY (spec §5.1).
//!
//! `tokio::sync::Semaphore` khong co ho tro co permit da phat ra lai (`forget`/`add_permits` chi
//! them, khong bot an toan khi dang co nguoi giu), nen phai tu viet: `AtomicUsize` dem so permit
//! dang giu (`Drop` khong the `async` nen khong dung `tokio::Mutex`/`Semaphore::acquire` cho viec
//! tra permit) + `Notify` de danh thuc nguoi cho + hai `watch::Receiver<usize>` de nhan cai dat moi
//! (so luong luong nguoi dung chinh, so tai khoan dang bat lich) ma khong can khoa.
//!
//! **Ngu nghia ADR-016** (giu dung, khong duoc lam yeu di): tang so luong luong -> danh thuc NGAY
//! cac lenh `acquire` dang cho; giam so luong luong -> KHONG cat job dang chay (permit dang giu van
//! giu duoc den khi tu `drop`), chi luot `acquire` TIEP THEO moi bi cho theo gioi han moi.
//!
//! **Luu y FIFO**: `Notify::notify_waiters` danh thuc TAT CA nguoi dang cho cung luc, khong co ve
//! thu tu (khac ban Python dung hang doi ve co thu tu — xem `browser_pool.py`). Day la ruling cua
//! plan P2 Task 15: KHONG port lai ve FIFO trong ban Rust nay; moi nguoi cho tranh CAS lai tu dau,
//! ai thanh cong truoc thi lay permit truoc — cong bang "best effort", khong dam bao thu tu den.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::{Notify, watch};

/// Gioi han dong (spec §5.1). Tao qua [`DynamicLimiter::new`], dung chung mot [`Arc`] giua cac
/// task goi [`DynamicLimiter::acquire`].
pub struct DynamicLimiter {
    /// So permit dang duoc giu (chua tra lai qua `Drop`).
    active: AtomicUsize,
    /// Cai dat nguoi dung, kep 1..=100 boi [`LimiterHandle::set_threads`] (spec: "cai dat nguoi
    /// dung 1..100").
    threads: watch::Receiver<usize>,
    /// So tai khoan dang bat lich — 0 nghia la chua tai khoan nao san sang, `effective` van kep ve
    /// toi thieu 1 (xem [`DynamicLimiter::effective`]).
    accounts: watch::Receiver<usize>,
    /// Danh thuc nguoi cho khi co permit duoc tra lai (`Permit::drop`) — bo sung cho `watch::changed`
    /// (danh thuc khi CAI DAT thay doi).
    notify: Notify,
}

/// Nua con lai cua [`DynamicLimiter::new`]: giu hai `watch::Sender` de doi cai dat luc dang chay.
/// Tach khoi `DynamicLimiter` vi chi UI/config-loader duoc goi `set_threads`/`set_accounts`, con
/// worker chi can `acquire`.
pub struct LimiterHandle {
    threads: watch::Sender<usize>,
    accounts: watch::Sender<usize>,
}

impl LimiterHandle {
    /// Doi so luong luong nguoi dung, kep vao `[1, 100]` (spec §5.1: "cai dat nguoi dung 1..100").
    /// Danh thuc ngay lap tuc moi lenh `acquire` dang cho (qua `watch::Receiver::changed`) du gia
    /// tri sau khi kep co doi that hay khong — vo hai, nguoi cho chi kiem tra lai dieu kien.
    ///
    /// `watch::Sender::send` chi loi khi khong con `Receiver` nao (moi `DynamicLimiter`/ban sao cua
    /// no da bi drop het) — bo qua loi do, khong con ai can duoc canh bao nua.
    pub fn set_threads(&self, n: usize) {
        let _ = self.threads.send(n.clamp(1, 100));
    }

    /// Doi so tai khoan dang bat lich. Khong kep (0 la gia tri hop le — chua tai khoan nao san
    /// sang; [`DynamicLimiter::effective`] tu kep san toi thieu 1 permit).
    pub fn set_accounts(&self, n: usize) {
        let _ = self.accounts.send(n);
    }
}

/// Ve cho phep chay 1 job, tra ve tu [`DynamicLimiter::acquire`]. `#[must_use]`: de roi (khong
/// gan vao bien) la mat slot ngay lap tuc do `Drop` chay lien (khong giu duoc "quyen chay" nao).
///
/// `Send + Sync` tu dong (khong can `unsafe impl`): truong duy nhat la `Arc<DynamicLimiter>`, va
/// `DynamicLimiter` chi gom `AtomicUsize`/`watch::Receiver<usize>`/`Notify` — deu `Send + Sync`.
#[must_use]
pub struct Permit {
    inner: Arc<DynamicLimiter>,
}

impl DynamicLimiter {
    /// Tao gioi han moi: `threads` (kep 1..=100, xem [`LimiterHandle::set_threads`]), `accounts`
    /// (so tai khoan dang bat lich, khong kep — co the la 0 luc khoi dong truoc khi nap DB).
    #[must_use]
    pub fn new(threads: usize, accounts: usize) -> (Arc<Self>, LimiterHandle) {
        let (threads_tx, threads_rx) = watch::channel(threads.clamp(1, 100));
        let (accounts_tx, accounts_rx) = watch::channel(accounts);
        let limiter = Arc::new(Self {
            active: AtomicUsize::new(0),
            threads: threads_rx,
            accounts: accounts_rx,
            notify: Notify::new(),
        });
        let handle = LimiterHandle {
            threads: threads_tx,
            accounts: accounts_tx,
        };
        (limiter, handle)
    }

    /// So permit toi da duoc phep cung luc: `min(threads, accounts).max(1)` (spec §5.1 nguyen
    /// van) — it hon giua "nguoi dung cho phep bao nhieu luong" va "co bao nhieu tai khoan de
    /// chay", nhung khong bao gio bang 0 (0 tai khoan van phai cho 1 luong cho, roi bao loi ro
    /// rang o tang tren thay vi treo may khong acquire duoc bao gio).
    #[must_use]
    pub fn effective(&self) -> usize {
        (*self.threads.borrow()).min(*self.accounts.borrow()).max(1)
    }

    /// So permit dang duoc giu tai thoi diem goi (khong dam bao con dung ngay sau do — dung de
    /// test/quan sat, khong dung de quyet dinh nghiep vu).
    #[must_use]
    pub fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    /// Xin mot permit — cho toi khi `active() < effective()`. Vong lap CAS (khong `Mutex`, vi
    /// `Drop` cua [`Permit`] khong the `async`): doc `active`, neu con cho thi thu tang bang
    /// `compare_exchange_weak` (that bai gia — spurious failure — chi can thu lai, KHONG phai loi
    /// that); neu da day thi cho mot trong ba tin hieu (`notify` tu `Permit::drop`, hoac
    /// `threads`/`accounts` thay doi qua `LimiterHandle`) roi vong lai tu dau.
    pub async fn acquire(self: &Arc<Self>) -> Permit {
        let mut threads = self.threads.clone();
        let mut accounts = self.accounts.clone();
        loop {
            let cur = self.active.load(Ordering::Acquire);
            if cur < self.effective() {
                if self
                    .active
                    .compare_exchange_weak(cur, cur + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return Permit {
                        inner: Arc::clone(self),
                    };
                }
                // CAS that bai (gia tri doi hoac spurious failure) -> doc lai tu dau, khong cho.
                continue;
            }
            tokio::select! {
                () = self.notify.notified() => {}
                _ = threads.changed() => {}
                _ = accounts.changed() => {}
            }
        }
    }
}

impl Drop for Permit {
    /// Tra permit + danh thuc TAT CA nguoi dang cho (khong chi 1 - xem luu y FIFO o dau file).
    fn drop(&mut self) {
        self.inner.active.fetch_sub(1, Ordering::Release);
        self.inner.notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    use super::*;

    /// `Permit` phai `Send + Sync` (yeu cau brief Task 15) — kiem tra o muc kieu, khong panic.
    #[test]
    fn permit_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Permit>();
    }

    /// `effective = min(threads, accounts).max(1)`: threads=5, accounts=2 -> 2 permit cap ngay,
    /// permit thu 3 phai CHO (khong duoc cap trong 100ms).
    #[tokio::test]
    async fn effective_min_threads_accounts() {
        let (limiter, _handle) = DynamicLimiter::new(5, 2);
        assert_eq!(limiter.effective(), 2);

        let p1 = limiter.acquire().await;
        let p2 = limiter.acquire().await;
        assert_eq!(limiter.active(), 2);

        let res = tokio::time::timeout(Duration::from_millis(100), limiter.acquire()).await;
        assert!(res.is_err(), "permit thu 3 phai cho, khong duoc cap ngay");

        drop(p1);
        drop(p2);
    }

    /// Tang `threads` phai danh thuc NGAY lenh `acquire` dang cho (ADR-016).
    #[tokio::test]
    async fn tang_threads_danh_thuc_ngay() {
        let (limiter, handle) = DynamicLimiter::new(1, 5); // effective = 1
        let p1 = limiter.acquire().await;

        let waiter = Arc::clone(&limiter);
        let waiting = tokio::spawn(async move { waiter.acquire().await });
        // Nhuong CPU de task cho kip vao trang thai `tokio::select!` truoc khi doi cai dat.
        tokio::time::sleep(Duration::from_millis(50)).await;

        handle.set_threads(2);

        let permit = tokio::time::timeout(Duration::from_millis(100), waiting)
            .await
            .expect("phai co permit trong 100ms sau khi tang threads")
            .expect("task cho khong duoc panic");

        drop(p1);
        drop(permit);
    }

    /// Giam `threads` KHONG duoc cat job dang chay (ADR-016): 2 permit dang giu, `set_threads(1)`
    /// -> khong panic, `active()` van la 2, permit moi phai cho den khi CA HAI permit cu duoc tha.
    #[tokio::test]
    async fn giam_threads_khong_cat_job() {
        let (limiter, handle) = DynamicLimiter::new(2, 5); // effective = 2
        let p1 = limiter.acquire().await;
        let p2 = limiter.acquire().await;
        assert_eq!(limiter.active(), 2);

        handle.set_threads(1);
        assert_eq!(
            limiter.active(),
            2,
            "giam threads khong duoc cat permit dang giu"
        );

        let pending = tokio::time::timeout(Duration::from_millis(100), limiter.acquire()).await;
        assert!(
            pending.is_err(),
            "effective moi la 1, da co 2 permit giu -> permit moi phai cho"
        );

        drop(p1);
        drop(p2);

        let p3 = tokio::time::timeout(Duration::from_millis(100), limiter.acquire())
            .await
            .expect("sau khi tha ca hai permit cu, effective=1 phai cap duoc permit moi");
        drop(p3);
    }

    /// `Permit::drop` phai danh thuc nguoi dang cho (khong chi doi `watch::changed`).
    #[tokio::test]
    async fn drop_permit_notify() {
        let (limiter, _handle) = DynamicLimiter::new(1, 5); // effective = 1
        let p1 = limiter.acquire().await;

        let waiter = Arc::clone(&limiter);
        let waiting = tokio::spawn(async move { waiter.acquire().await });
        tokio::time::sleep(Duration::from_millis(50)).await;

        drop(p1);

        let _p2 = tokio::time::timeout(Duration::from_millis(100), waiting)
            .await
            .expect("drop permit phai danh thuc nguoi cho trong 100ms")
            .expect("task cho khong duoc panic");
    }

    /// `accounts = 0` -> `effective` kep ve toi thieu 1 (khong bao gio 0 permit).
    #[tokio::test]
    async fn effective_toi_thieu_1() {
        let (limiter, _handle) = DynamicLimiter::new(5, 0);
        assert_eq!(limiter.effective(), 1);

        let p = tokio::time::timeout(Duration::from_millis(100), limiter.acquire())
            .await
            .expect("effective=1 phai cap duoc dung 1 permit");
        drop(p);
    }

    /// `set_threads` kep vao `[1, 100]`: 0 -> 1, 1000 -> 100. `accounts=1000` de `effective` phan
    /// anh dung gia tri `threads` da kep (khong bi accounts gioi han).
    #[tokio::test]
    async fn set_threads_clamp() {
        let (limiter, handle) = DynamicLimiter::new(50, 1000);

        handle.set_threads(0);
        assert_eq!(limiter.effective(), 1, "0 phai kep len 1");

        handle.set_threads(1000);
        assert_eq!(limiter.effective(), 100, "1000 phai kep xuong 100");

        // Constructor cung phai kep, khong chi `set_threads`.
        let (limiter2, _h2) = DynamicLimiter::new(0, 1000);
        assert_eq!(limiter2.effective(), 1);
        let (limiter3, _h3) = DynamicLimiter::new(1000, 1000);
        assert_eq!(limiter3.effective(), 100);
    }

    /// Nhieu task xin permit dong thoi: so permit dang giu CUNG LUC khong bao gio vuot `effective()`.
    #[tokio::test]
    async fn nhieu_task_dong_thoi_khong_vuot_effective() {
        let (limiter, _handle) = DynamicLimiter::new(3, 5); // effective = 3
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));

        let mut tasks = Vec::new();
        for _ in 0..20 {
            let limiter = Arc::clone(&limiter);
            let concurrent = Arc::clone(&concurrent);
            let max_seen = Arc::clone(&max_seen);
            tasks.push(tokio::spawn(async move {
                let _permit = limiter.acquire().await;
                let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                max_seen.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                concurrent.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for t in tasks {
            t.await.expect("task khong duoc panic");
        }

        assert!(
            max_seen.load(Ordering::SeqCst) <= limiter.effective(),
            "so permit dong thoi ({}) vuot effective ({})",
            max_seen.load(Ordering::SeqCst),
            limiter.effective()
        );
    }

    /// Giam `accounts` khi dang chay: giong `giam_threads_khong_cat_job` nhung qua kenh `accounts`.
    #[tokio::test]
    async fn accounts_giam_khi_dang_chay() {
        let (limiter, handle) = DynamicLimiter::new(5, 2); // effective = 2
        let p1 = limiter.acquire().await;
        let p2 = limiter.acquire().await;
        assert_eq!(limiter.active(), 2);

        handle.set_accounts(1);
        assert_eq!(
            limiter.active(),
            2,
            "giam accounts khong duoc cat permit dang giu"
        );

        let pending = tokio::time::timeout(Duration::from_millis(100), limiter.acquire()).await;
        assert!(
            pending.is_err(),
            "effective moi la 1 -> permit moi phai cho"
        );

        drop(p1);
        drop(p2);

        let p3 = tokio::time::timeout(Duration::from_millis(100), limiter.acquire())
            .await
            .expect("sau khi tha ca hai, effective=1 (accounts) phai cap permit");
        drop(p3);
    }
}
