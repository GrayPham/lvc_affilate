//! Bộ chia tải tài khoản `MuseAI` — xoay vòng theo TẢI THỰC chứ không theo thứ tự gọi.
//!
//! Vấn đề cũ: resolver xoay vòng một con trỏ mỗi lần node gọi, không biết tài khoản nào đang bận.
//! Với `node_threads = 3` và 2 tài khoản, vòng xoay có thể dồn 2 cảnh sinh cùng lúc vào MỘT tài khoản
//! (muse.ai trả 502 / "không có ảnh mới trong 300s"), trong khi tài khoản kia rảnh. Một tài khoản hết
//! phiên đăng nhập thì mọi node rơi vào nó đều hỏng thay vì chuyển sang tài khoản còn lại.
//!
//! Module này giữ, cho MỖI tài khoản: số job đang chạy, mốc "đang bị cách ly" và chuỗi lỗi liên
//! tiếp. [`AccountScheduler::acquire`] chọn tài khoản RẢNH NHẤT (ít job nhất, hoà thì lâu chưa dùng
//! nhất), chờ khi tất cả đang bận/bị cách ly, và trả [`Lease`] — thả nó (drop) là trả chỗ.
//!
//! Không biết gì về kho tài khoản hay trình duyệt: caller đưa danh sách [`Candidate`]. THUẦN theo
//! nghĩa không I/O, nên test được tất định (xem `mod tests`). KHÔNG log bí mật — chỉ khoá tài khoản
//! (id) và đếm.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// Số job sinh đồng thời tối đa trên MỖI tài khoản. 1 = an toàn nhất: muse.ai xử lý mỗi tài khoản
/// một chuỗi sinh, dồn thêm chỉ làm chậm/502. Tăng được qua [`AccountScheduler::set_per_account`].
pub const DEFAULT_PER_ACCOUNT: usize = 1;

/// Cách ly ngắn cho lỗi TẠM THỜI (502, hết giờ chờ): lần chọn kế ưu tiên tài khoản khác, nhưng tài
/// khoản này sớm được dùng lại.
const COOLDOWN_TRANSIENT: Duration = Duration::from_secs(30);

/// Cách ly cho lỗi CẤP TÀI KHOẢN (không mở được phiên, hết đăng nhập): gấp đôi theo chuỗi lỗi liên
/// tiếp, từ mốc này tới [`COOLDOWN_MAX`].
const COOLDOWN_ACCOUNT_BASE: Duration = Duration::from_secs(120);
const COOLDOWN_MAX: Duration = Duration::from_secs(900);

/// Chờ tối đa một chỗ trống trước khi bỏ cuộc — tránh một node treo vĩnh viễn khi mọi tài khoản đều
/// hỏng.
const MAX_WAIT: Duration = Duration::from_secs(20 * 60);

/// Nhịp kiểm tra lại khi chờ (hết cách ly không có sự kiện đánh thức nên phải thăm dò).
const POLL: Duration = Duration::from_secs(1);

/// Một tài khoản có thể được chọn. `key` là định danh ỔN ĐỊNH (id credential), dùng cho cách ly và
/// danh sách loại trừ; `profile_dir` là nơi trình duyệt đã đăng nhập.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub key: String,
    pub profile_dir: PathBuf,
}

/// Kết quả một lần dùng tài khoản — để bộ chia tải điều chỉnh lần chọn sau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Có ít nhất một job xong: xoá chuỗi lỗi và cách ly.
    Ok,
    /// Lỗi tạm thời (502, quá hạn): cách ly ngắn, không tăng chuỗi lỗi.
    Transient,
    /// Không dùng được tài khoản (không mở được phiên): cách ly tăng dần.
    AccountFailed,
}

#[derive(Debug, Default)]
struct AccountState {
    in_flight: usize,
    cooling_until: Option<Instant>,
    fail_streak: u32,
    /// Nhãn thứ tự lần chọn gần nhất — hoà tải thì tài khoản có nhãn nhỏ (lâu chưa dùng) được chọn.
    last_pick: u64,
}

#[derive(Debug)]
struct Inner {
    accounts: HashMap<String, AccountState>,
    tick: u64,
    per_account: usize,
}

/// Kết quả một lượt thử chọn (hàm thuần [`pick`]).
#[derive(Debug, PartialEq, Eq)]
enum Pick {
    /// Chọn được ứng viên ở chỉ số này (đã tăng `in_flight`).
    Got(usize),
    /// Có ứng viên nhưng tất cả đang bận/bị cách ly. `Some(t)` = mốc sớm nhất một tài khoản hết cách
    /// ly; `None` = chỉ đang bận (đợi có người trả chỗ).
    Wait(Option<Instant>),
    /// Không còn ứng viên nào hợp lệ (rỗng, hoặc đều nằm trong danh sách loại trừ).
    NoCandidate,
}

/// Chọn tài khoản rảnh nhất. THUẦN: chỉ đọc/ghi `inner` và nhận `now` từ ngoài.
fn pick(inner: &mut Inner, cands: &[Candidate], avoid: &[String], now: Instant) -> Pick {
    let eligible: Vec<usize> = (0..cands.len())
        .filter(|&i| !avoid.contains(&cands[i].key))
        .collect();
    if eligible.is_empty() {
        return Pick::NoCandidate;
    }

    let per_account = inner.per_account;
    let mut best: Option<(usize, usize, u64)> = None; // (chỉ số, in_flight, last_pick)
    let mut earliest_cooldown_end: Option<Instant> = None;

    for &i in &eligible {
        let st = inner.accounts.entry(cands[i].key.clone()).or_default();
        if let Some(until) = st.cooling_until {
            if until > now {
                earliest_cooldown_end =
                    Some(earliest_cooldown_end.map_or(until, |e: Instant| e.min(until)));
                continue;
            }
            st.cooling_until = None; // hết cách ly
        }
        if st.in_flight >= per_account {
            continue;
        }
        let key = (st.in_flight, st.last_pick);
        if best.is_none_or(|(_, f, l)| key < (f, l)) {
            best = Some((i, st.in_flight, st.last_pick));
        }
    }

    match best {
        Some((i, _, _)) => {
            inner.tick += 1;
            let tick = inner.tick;
            let st = inner.accounts.entry(cands[i].key.clone()).or_default();
            st.in_flight += 1;
            st.last_pick = tick;
            Pick::Got(i)
        }
        None => Pick::Wait(earliest_cooldown_end),
    }
}

/// Bộ chia tải dùng chung (một `Arc` cho cả engine). Xem doc module.
#[derive(Debug)]
pub struct AccountScheduler {
    inner: Mutex<Inner>,
    notify: Notify,
}

impl Default for AccountScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                accounts: HashMap::new(),
                tick: 0,
                per_account: DEFAULT_PER_ACCOUNT,
            }),
            notify: Notify::new(),
        }
    }

    /// Khoá trạng thái. Mutex bị nhiễm độc (một task panic khi đang giữ) vẫn dùng tiếp được: trạng thái
    /// chỉ là bộ đếm/mốc thời gian, mất nhất quán nhỏ còn hơn làm sập mọi node đang chờ tài khoản.
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Đổi số job đồng thời tối đa mỗi tài khoản (kẹp tối thiểu 1). Có hiệu lực cho lần chọn kế tiếp.
    pub fn set_per_account(&self, n: usize) {
        self.lock().per_account = n.max(1);
        self.notify.notify_waiters();
    }

    /// Mượn một tài khoản trong `cands` (bỏ các `key` trong `avoid`). Chờ nếu tất cả đang bận/bị cách
    /// ly, tối đa [`MAX_WAIT`]; dừng ngay khi `cancel` bị huỷ.
    ///
    /// # Errors
    /// Chuỗi mô tả AN TOÀN: không còn tài khoản hợp lệ, chờ quá lâu, hoặc đã huỷ.
    pub async fn acquire(
        self: &Arc<Self>,
        cands: &[Candidate],
        avoid: &[String],
        cancel: &CancellationToken,
    ) -> Result<Lease, String> {
        let deadline = Instant::now() + MAX_WAIT;
        loop {
            // Tạo `Notified` TRƯỚC khi kiểm tra: tokio bảo đảm nó nhận `notify_waiters` ngay từ lúc
            // tạo, nên không mất đánh thức nếu có người trả chỗ giữa lúc kiểm tra và lúc chờ.
            let notified = self.notify.notified();
            let now = Instant::now();
            let picked = {
                let mut inner = self.lock();
                pick(&mut inner, cands, avoid, now)
            };
            match picked {
                Pick::Got(i) => {
                    return Ok(Lease {
                        sched: Some(Arc::clone(self)),
                        key: cands[i].key.clone(),
                        profile_dir: cands[i].profile_dir.clone(),
                    });
                }
                Pick::NoCandidate => {
                    return Err(
                        "khong con tai khoan MuseAI hop le de thu (het tai khoan du phong)"
                            .to_string(),
                    );
                }
                Pick::Wait(until) => {
                    if cancel.is_cancelled() {
                        return Err("da huy".to_string());
                    }
                    if now >= deadline {
                        return Err(
                            "cho tai khoan MuseAI qua lau — tat ca dang ban hoac bi cach ly sau loi"
                                .to_string(),
                        );
                    }
                    let until_cooldown = until.map_or(POLL, |u| u.saturating_duration_since(now));
                    let wait = until_cooldown.min(POLL).min(deadline - now);
                    tokio::select! {
                        biased;
                        () = cancel.cancelled() => return Err("da huy".to_string()),
                        () = notified => {}
                        () = tokio::time::sleep(wait) => {}
                    }
                }
            }
        }
    }

    fn release(&self, key: &str) {
        {
            let mut inner = self.lock();
            if let Some(st) = inner.accounts.get_mut(key) {
                st.in_flight = st.in_flight.saturating_sub(1);
            }
        }
        self.notify.notify_waiters();
    }

    fn record(&self, key: &str, outcome: Outcome) {
        {
            let mut inner = self.lock();
            let st = inner.accounts.entry(key.to_string()).or_default();
            let now = Instant::now();
            match outcome {
                Outcome::Ok => {
                    st.fail_streak = 0;
                    st.cooling_until = None;
                }
                Outcome::Transient => {
                    st.cooling_until = Some(now + COOLDOWN_TRANSIENT);
                }
                Outcome::AccountFailed => {
                    st.fail_streak = st.fail_streak.saturating_add(1);
                    let doublings = st.fail_streak.saturating_sub(1).min(8);
                    let cooldown = COOLDOWN_ACCOUNT_BASE
                        .saturating_mul(1u32 << doublings)
                        .min(COOLDOWN_MAX);
                    st.cooling_until = Some(now + cooldown);
                }
            }
        }
        self.notify.notify_waiters();
    }

    /// Số job đang chạy trên `key` (cho test/chẩn đoán).
    #[must_use]
    pub fn in_flight(&self, key: &str) -> usize {
        self.lock().accounts.get(key).map_or(0, |s| s.in_flight)
    }
}

/// Giấy mượn một tài khoản. Thả (drop) = trả chỗ và đánh thức người đang chờ. Lease "không quản lý"
/// ([`Lease::unmanaged`]) không gắn bộ chia tải — dùng cho resolver mặc định (CLI/test).
#[derive(Debug)]
pub struct Lease {
    sched: Option<Arc<AccountScheduler>>,
    key: String,
    profile_dir: PathBuf,
}

impl Lease {
    /// Lease không giới hạn tải, không báo kết quả — giữ hành vi cũ cho resolver không chia tải.
    #[must_use]
    pub fn unmanaged(profile_dir: PathBuf) -> Self {
        Self {
            sched: None,
            key: String::new(),
            profile_dir,
        }
    }

    #[must_use]
    pub fn profile_dir(&self) -> &Path {
        &self.profile_dir
    }

    /// Khoá tài khoản; RỖNG nếu lease không quản lý.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Báo kết quả để lần chọn sau tránh/ưu tiên tài khoản này. Không làm gì với lease không quản lý.
    pub fn report(&self, outcome: Outcome) {
        if let Some(s) = &self.sched {
            s.record(&self.key, outcome);
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(s) = &self.sched {
            s.release(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(key: &str) -> Candidate {
        Candidate {
            key: key.to_string(),
            profile_dir: PathBuf::from(format!("/profiles/{key}")),
        }
    }

    fn inner(per_account: usize) -> Inner {
        Inner {
            accounts: HashMap::new(),
            tick: 0,
            per_account,
        }
    }

    // ---- pick (thuần) ----

    #[test]
    fn pick_khong_ung_vien_tra_no_candidate() {
        let mut i = inner(1);
        assert_eq!(pick(&mut i, &[], &[], Instant::now()), Pick::NoCandidate);
    }

    #[test]
    fn pick_tat_ca_trong_danh_sach_loai_tru_tra_no_candidate() {
        let mut i = inner(1);
        let c = [cand("a"), cand("b")];
        let avoid = vec!["a".to_string(), "b".to_string()];
        assert_eq!(pick(&mut i, &c, &avoid, Instant::now()), Pick::NoCandidate);
    }

    #[test]
    fn pick_xoay_deu_hai_tai_khoan_khi_moi_lan_giai_phong() {
        // Mỗi lần mượn rồi trả: hoà tải thì chọn tài khoản LÂU CHƯA DÙNG → a, b, a, b.
        let mut i = inner(1);
        let c = [cand("a"), cand("b")];
        let now = Instant::now();
        let mut order = Vec::new();
        for _ in 0..4 {
            let Pick::Got(idx) = pick(&mut i, &c, &[], now) else {
                panic!("phải chọn được");
            };
            order.push(c[idx].key.clone());
            i.accounts.get_mut(&c[idx].key).unwrap().in_flight -= 1; // trả chỗ
        }
        assert_eq!(order, ["a", "b", "a", "b"]);
    }

    #[test]
    fn pick_uu_tien_tai_khoan_it_viec_hon_khong_don_vao_mot_tai_khoan() {
        let mut i = inner(2);
        let c = [cand("a"), cand("b")];
        let now = Instant::now();
        // 3 job liên tiếp, KHÔNG trả chỗ: a, b, rồi (hoà 1-1) lâu chưa dùng nhất là a.
        let got: Vec<String> = (0..3)
            .map(|_| match pick(&mut i, &c, &[], now) {
                Pick::Got(idx) => c[idx].key.clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(got, ["a", "b", "a"]);
        assert_eq!(i.accounts["a"].in_flight, 2);
        assert_eq!(i.accounts["b"].in_flight, 1);
    }

    #[test]
    fn pick_dung_o_tran_moi_tai_khoan_thi_cho() {
        let mut i = inner(1);
        let c = [cand("a"), cand("b")];
        let now = Instant::now();
        assert!(matches!(pick(&mut i, &c, &[], now), Pick::Got(_)));
        assert!(matches!(pick(&mut i, &c, &[], now), Pick::Got(_)));
        // Cả hai đã đủ 1 job → phải chờ, không có mốc cách ly.
        assert_eq!(pick(&mut i, &c, &[], now), Pick::Wait(None));
    }

    #[test]
    fn pick_bo_qua_tai_khoan_dang_bi_cach_ly() {
        let mut i = inner(1);
        let c = [cand("a"), cand("b")];
        let now = Instant::now();
        i.accounts.entry("a".into()).or_default().cooling_until =
            Some(now + Duration::from_secs(60));
        // `a` bị cách ly → dù `a` đứng đầu danh sách, phải chọn `b`.
        let Pick::Got(idx) = pick(&mut i, &c, &[], now) else {
            panic!("phải chọn được b");
        };
        assert_eq!(c[idx].key, "b");
    }

    #[test]
    fn pick_tat_ca_bi_cach_ly_tra_moc_het_han_som_nhat() {
        let mut i = inner(1);
        let c = [cand("a"), cand("b")];
        let now = Instant::now();
        let soon = now + Duration::from_secs(10);
        i.accounts.entry("a".into()).or_default().cooling_until =
            Some(now + Duration::from_secs(60));
        i.accounts.entry("b".into()).or_default().cooling_until = Some(soon);
        assert_eq!(pick(&mut i, &c, &[], now), Pick::Wait(Some(soon)));
    }

    #[test]
    fn pick_het_cach_ly_thi_dung_lai_duoc() {
        let mut i = inner(1);
        let c = [cand("a")];
        let now = Instant::now();
        i.accounts.entry("a".into()).or_default().cooling_until =
            Some(now.checked_sub(Duration::from_secs(1)).unwrap_or(now));
        assert!(matches!(pick(&mut i, &c, &[], now), Pick::Got(0)));
    }

    #[test]
    fn pick_bo_qua_tai_khoan_trong_danh_sach_loai_tru() {
        let mut i = inner(1);
        let c = [cand("a"), cand("b")];
        let avoid = vec!["a".to_string()];
        let Pick::Got(idx) = pick(&mut i, &c, &avoid, Instant::now()) else {
            panic!("phải chọn được b");
        };
        assert_eq!(c[idx].key, "b");
    }

    // ---- acquire / Lease ----

    #[tokio::test]
    async fn hai_lease_dong_thoi_roi_vao_hai_tai_khoan_khac_nhau() {
        let s = Arc::new(AccountScheduler::new());
        let c = [cand("a"), cand("b")];
        let cancel = CancellationToken::new();
        let l1 = s.acquire(&c, &[], &cancel).await.unwrap();
        let l2 = s.acquire(&c, &[], &cancel).await.unwrap();
        assert_ne!(
            l1.key(),
            l2.key(),
            "2 job đồng thời phải ở 2 tài khoản khác nhau"
        );
        assert_eq!(s.in_flight("a"), 1);
        assert_eq!(s.in_flight("b"), 1);
    }

    #[tokio::test]
    async fn lease_thu_ba_cho_den_khi_co_nguoi_tra_cho() {
        let s = Arc::new(AccountScheduler::new());
        let c = [cand("a"), cand("b")];
        let cancel = CancellationToken::new();
        let l1 = s.acquire(&c, &[], &cancel).await.unwrap();
        let _l2 = s.acquire(&c, &[], &cancel).await.unwrap();

        let s2 = Arc::clone(&s);
        let c2 = c.clone();
        let cancel2 = cancel.clone();
        let waiter = tokio::spawn(async move { s2.acquire(&c2, &[], &cancel2).await });

        // Chưa ai trả chỗ → người thứ ba chưa xong.
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(
            !waiter.is_finished(),
            "phải đang chờ vì cả hai tài khoản đều bận"
        );

        let freed = l1.key().to_string();
        drop(l1); // trả chỗ → đánh thức người chờ
        let l3 = tokio::time::timeout(Duration::from_secs(3), waiter)
            .await
            .expect("phải được đánh thức sau khi có chỗ trống")
            .unwrap()
            .unwrap();
        assert_eq!(l3.key(), freed, "chỗ vừa trả phải về tay người đang chờ");
    }

    #[tokio::test]
    async fn drop_lease_tra_cho_in_flight_ve_khong() {
        let s = Arc::new(AccountScheduler::new());
        let c = [cand("a")];
        let cancel = CancellationToken::new();
        let l = s.acquire(&c, &[], &cancel).await.unwrap();
        assert_eq!(s.in_flight("a"), 1);
        drop(l);
        assert_eq!(s.in_flight("a"), 0);
    }

    #[tokio::test]
    async fn huy_khi_dang_cho_thi_thoat_ngay_khong_doi_het_han() {
        let s = Arc::new(AccountScheduler::new());
        let c = [cand("a")];
        let cancel = CancellationToken::new();
        let _held = s.acquire(&c, &[], &cancel).await.unwrap();

        let s2 = Arc::clone(&s);
        let c2 = c.clone();
        let cancel2 = cancel.clone();
        let waiter = tokio::spawn(async move { s2.acquire(&c2, &[], &cancel2).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
        let r = tokio::time::timeout(Duration::from_secs(3), waiter)
            .await
            .expect("huỷ phải cắt ngang lúc chờ")
            .unwrap();
        assert_eq!(r.unwrap_err(), "da huy");
    }

    #[tokio::test]
    async fn danh_sach_loai_tru_het_ung_vien_bao_loi_khong_cho() {
        let s = Arc::new(AccountScheduler::new());
        let c = [cand("a")];
        let avoid = vec!["a".to_string()];
        let err = s
            .acquire(&c, &avoid, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(err.contains("du phong"), "{err}");
    }

    #[tokio::test]
    async fn bao_loi_tai_khoan_thi_lan_chon_sau_chuyen_sang_tai_khoan_khac() {
        let s = Arc::new(AccountScheduler::new());
        let c = [cand("a"), cand("b")];
        let cancel = CancellationToken::new();
        let l = s.acquire(&c, &[], &cancel).await.unwrap();
        assert_eq!(l.key(), "a");
        l.report(Outcome::AccountFailed);
        drop(l);
        // `a` đã rảnh lại nhưng đang bị cách ly → các lần sau phải rơi vào `b`.
        for _ in 0..3 {
            let l = s.acquire(&c, &[], &cancel).await.unwrap();
            assert_eq!(l.key(), "b");
        }
    }

    #[tokio::test]
    async fn bao_ok_xoa_cach_ly() {
        let s = Arc::new(AccountScheduler::new());
        let c = [cand("a")];
        let cancel = CancellationToken::new();
        let l = s.acquire(&c, &[], &cancel).await.unwrap();
        l.report(Outcome::AccountFailed);
        l.report(Outcome::Ok); // sau đó có job xong → hết cách ly
        drop(l);
        let again = tokio::time::timeout(Duration::from_secs(2), s.acquire(&c, &[], &cancel))
            .await
            .expect("không được bị cách ly sau khi báo Ok");
        assert!(again.is_ok());
    }

    #[test]
    fn cach_ly_tai_khoan_tang_gap_doi_theo_chuoi_loi_va_co_tran() {
        let s = AccountScheduler::new();
        let first = {
            s.record("a", Outcome::AccountFailed);
            s.inner.lock().unwrap().accounts["a"].cooling_until.unwrap()
        };
        let second = {
            s.record("a", Outcome::AccountFailed);
            s.inner.lock().unwrap().accounts["a"].cooling_until.unwrap()
        };
        assert!(second > first, "lỗi liên tiếp phải bị cách ly lâu hơn");
        for _ in 0..30 {
            s.record("a", Outcome::AccountFailed);
        }
        let capped = s.inner.lock().unwrap().accounts["a"].cooling_until.unwrap();
        assert!(
            capped <= Instant::now() + COOLDOWN_MAX + Duration::from_secs(1),
            "cách ly không được vượt trần"
        );
    }

    #[test]
    fn lease_khong_quan_ly_khong_lam_gi_khi_bao_cao_va_tra() {
        let l = Lease::unmanaged(PathBuf::from("/p"));
        assert_eq!(l.key(), "");
        l.report(Outcome::AccountFailed); // không panic
        assert_eq!(l.profile_dir(), Path::new("/p"));
    }

    #[test]
    fn set_per_account_ke_toi_thieu_mot() {
        let s = AccountScheduler::new();
        s.set_per_account(0);
        assert_eq!(s.inner.lock().unwrap().per_account, 1);
        s.set_per_account(3);
        assert_eq!(s.inner.lock().unwrap().per_account, 3);
    }
}
