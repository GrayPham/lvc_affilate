//! Huy mot video dang chay (P3 Task 4, ruling R4): moi `task_id` dang chay co mot
//! [`CancellationToken`] (tokio-util) trong [`CancelRegistry`]; UI goi [`CancelRegistry::cancel`]
//! (hoac `Engine::cancel`), engine hoi `token.is_cancelled()` tai 5 diem kiem (truoc launch, sau
//! lease, truoc submit, moi nhip poll qua `PollHooks::should_cancel`, truoc download) va dung bang
//! `DolaError::Cancelled` — KHONG abort future (tranh bo lo `set_conversation`/tra phien ve cache).
//!
//! Python: `store.is_cancelled(task_id)` (server.py:608, :614) doc cot `status == "cancelled"` trong
//! DB moi lan hoi; o day token trong bo nho (nhanh, khong cham DB) + dong task van ghi
//! `failed/cancelled` khi ket thuc (`generate_video`).
//!
//! `register` la **get-or-insert**: DAG executor (Task 7) co the dang ky truoc moi job cua mot run
//! roi `cancel_many` ca run — job chua chay se thay token da huy ngay diem kiem dau tien
//! (`Cancelled`, khong mo Chrome — IDEA-016). [`CancelGuard`] go token khi `generate_video` ket thuc
//! (RAII) de registry khong phinh.

use dashmap::DashMap;
use tokio_util::sync::CancellationToken;

/// So token dang dang ky theo `task_id`. `Default` = rong.
#[derive(Debug, Default)]
pub struct CancelRegistry {
    /// Cha cua moi token con (`register`) — de sau nay huy ca engine mot phat neu can (chua dung).
    root: CancellationToken,
    map: DashMap<String, CancellationToken>,
}

/// Go token cua `task_id` khoi registry khi drop (cuoi `generate_video`, ke ca khi loi/panic).
#[must_use = "giu guard toi khi task ket thuc, drop som = go token som"]
pub struct CancelGuard<'a> {
    reg: &'a CancelRegistry,
    task_id: String,
}

impl Drop for CancelGuard<'_> {
    fn drop(&mut self) {
        let _ = self.reg.remove(&self.task_id);
    }
}

impl CancelRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Token cua `task_id`: co san thi tra ban sao (cung trang thai), chua co thi tao token con cua
    /// `root` va luu. `cancel` chi tac dong len token DA dang ky — Task 7 muon huy job CHUA chay thi
    /// `register` moi job luc len ke hoach roi `cancel_many`; `generate_video` cua job do se nhan
    /// lai dung token (da huy) va dung ngay diem kiem dau.
    #[must_use]
    pub fn register(&self, task_id: &str) -> CancellationToken {
        self.map
            .entry(task_id.to_string())
            .or_insert_with(|| self.root.child_token())
            .clone()
    }

    /// [`CancelRegistry::register`] + guard RAII go token khi drop.
    pub fn register_guarded(&self, task_id: &str) -> (CancellationToken, CancelGuard<'_>) {
        let token = self.register(task_id);
        (
            token,
            CancelGuard {
                reg: self,
                task_id: task_id.to_string(),
            },
        )
    }

    /// Huy `task_id`: `true` khi co token (dang chay / da dang ky) — goi lai nhieu lan vo hai;
    /// `false` khi khong biet `task_id` (chua chay hoac da xong).
    #[must_use = "false = khong co task nao de huy"]
    pub fn cancel(&self, task_id: &str) -> bool {
        match self.map.get(task_id) {
            Some(tok) => {
                tok.cancel();
                true
            }
            None => false,
        }
    }

    /// Huy nhieu task (vd moi job cua mot run — Task 7); tra so token da huy.
    #[must_use = "so token da huy"]
    pub fn cancel_many<I, S>(&self, ids: I) -> usize
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        ids.into_iter()
            .filter(|id| self.cancel(id.as_ref()))
            .count()
    }

    /// Huy MOI token dang dang ky (khong huy `root` — token dang ky sau van sach); tra so da huy.
    #[must_use = "so token da huy"]
    pub fn cancel_all(&self) -> usize {
        self.map
            .iter()
            .map(|e| {
                e.value().cancel();
            })
            .count()
    }

    /// Go token cua `task_id` (cuoi `generate_video`); `true` khi co de go.
    #[must_use = "false = khong co token de go"]
    pub fn remove(&self, task_id: &str) -> bool {
        self.map.remove(task_id).is_some()
    }

    /// `true` khi `task_id` dang dang ky VA da bi huy; khong biet -> `false`.
    #[must_use]
    pub fn is_cancelled(&self, task_id: &str) -> bool {
        self.map.get(task_id).is_some_and(|tok| tok.is_cancelled())
    }

    #[must_use]
    pub fn contains(&self, task_id: &str) -> bool {
        self.map.contains_key(task_id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_get_or_insert_cung_token() {
        let r = CancelRegistry::new();
        assert!(r.is_empty());
        let a = r.register("t1");
        let b = r.register("t1");
        assert_eq!(r.len(), 1);
        assert!(!a.is_cancelled() && !b.is_cancelled());
        // cung token: huy qua registry -> ca hai ban sao thay
        assert!(r.cancel("t1"));
        assert!(a.is_cancelled());
        assert!(b.is_cancelled());
        assert!(r.is_cancelled("t1"));
        // dang ky lai SAU khi huy -> van la token da huy (Task 7: huy job chua chay)
        assert!(r.register("t1").is_cancelled());
    }

    #[test]
    fn cancel_khong_biet_tra_false() {
        let r = CancelRegistry::new();
        assert!(!r.cancel("ghost"));
        assert!(!r.is_cancelled("ghost"));
        assert!(!r.remove("ghost"));
        assert!(!r.contains("ghost"));
    }

    #[test]
    fn cancel_many_va_cancel_all_dem_dung() {
        let r = CancelRegistry::new();
        let t1 = r.register("t1");
        let t2 = r.register("t2");
        let t3 = r.register("t3");
        assert_eq!(r.cancel_many(["t1", "t2", "ghost"]), 2);
        assert!(t1.is_cancelled() && t2.is_cancelled() && !t3.is_cancelled());
        assert_eq!(r.cancel_all(), 3, "dem ca token da huy truoc do");
        assert!(t3.is_cancelled());
        // root khong bi huy -> token moi van sach
        assert!(!r.register("t4").is_cancelled());
    }

    #[test]
    fn guard_go_token_khi_drop() {
        let r = CancelRegistry::new();
        {
            let (tok, _g) = r.register_guarded("t1");
            assert!(r.contains("t1"));
            assert!(r.cancel("t1"));
            assert!(tok.is_cancelled());
        }
        assert!(!r.contains("t1"), "guard drop -> go");
        assert!(r.is_empty());
        assert!(!r.cancel("t1"));
        // chay lai cung id -> token MOI, sach
        let (tok, _g) = r.register_guarded("t1");
        assert!(!tok.is_cancelled());
    }

    #[test]
    fn token_con_cua_root_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CancelRegistry>();
        assert_send_sync::<CancelGuard<'static>>();
        assert_send_sync::<CancellationToken>();
    }
}
