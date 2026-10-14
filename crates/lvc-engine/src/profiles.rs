//! `Engine::provision_profile` (Phần A — tổng quát hoá tiêm cookie→profile cho MỌI nền tảng khác
//! Dola) — dán cookie (Netscape) → tiêm THẬT vào một profile Chromium qua CDP, giống pipeline Dola
//! (`accounts.rs::import_cookies_on_dir`) nhưng KHÔNG probe xác nhận đăng nhập (Dola có "trang chat"
//! chuẩn để đọc trạng thái; nền tảng khác chưa có — xác nhận đăng nhập vẫn qua `credential_verify`
//! riêng, tách biệt). `profile_dir` dùng CHUNG quy ước với B (đăng nhập thủ công, `login.rs`):
//! `accounts_dir/.logins/<provider>/<label an toàn>` (tiền tố `.` để `AccountPool` bỏ qua).
//!
//! Tách file riêng theo LUẬT chia module (CLAUDE.md): đây là một "kịch bản" độc lập (tiêm cookie có
//! sẵn), khác B (`login.rs`, mở cửa sổ HIỆN cho người dùng tự đăng nhập) dù dùng chung
//! `login::safe_component`/quy ước thư mục.

use std::path::PathBuf;

use lvc_browser::{BrowserDriver, LaunchOpts};
use lvc_core::{BrowserKind, effective_proxy};
use tracing::warn;

use crate::accounts::AccountError;
use crate::generate::Engine;
use crate::login::safe_component;

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Tiêm cookie (Netscape, `cookie_text`) vào MỘT profile Chromium bền vững cho `provider`/
    /// `label` — luồng: kiểm proxy + parse cookie (THUẦN, trước khi chạm đĩa/trình duyệt) → tạo
    /// `profile_dir` → `launch` (`headless=true`, chỉ tiêm KHÔNG hiện cửa sổ) → `add_cookies` →
    /// `close`. KHÔNG probe đăng nhập (xem doc đầu module) — trả về `profile_dir` để caller
    /// (`lvc-app`) lưu vào `meta.profile_dir` của credential.
    ///
    /// Lỗi `launch`/`add_cookies` KHÔNG xoá `profile_dir` đã tạo (khác `accounts.rs::import_cookies`
    /// của Dola) — caller vẫn lưu được credential với cookie đã dán (phương án lùi "chỉ lưu cookie,
    /// chưa có profile") thay vì mất trắng cookie người dùng vừa nhập chỉ vì trình duyệt lỗi.
    ///
    /// # Errors
    /// - [`AccountError::InvalidProxy`]: `proxy` sai định dạng ([`effective_proxy`]) hoặc (khi
    ///   `proxy` rỗng) proxy chung cấu hình cũng sai.
    /// - [`AccountError::CookieInvalid`]: `cookie_text` không đọc được cookie nào (Netscape rỗng/sai
    ///   khuôn dạng hoàn toàn).
    /// - [`AccountError::Io`]: không tạo được `profile_dir`.
    /// - [`AccountError::Browser`]: `launch`/`add_cookies` lỗi (thiếu Chrome, profile đang bị khoá,
    ///   CDP lỗi).
    pub async fn provision_profile(
        &self,
        provider: &str,
        label: &str,
        cookie_text: &str,
        proxy: &str,
    ) -> Result<PathBuf, AccountError> {
        let proxy = proxy.trim();
        // Kiem THUAN truoc — sai proxy/cookie rong thi khong tao rac thu muc / khong mo trinh duyet.
        let proxy_spec = effective_proxy(proxy, &self.cfg.global_proxy)
            .map_err(|e| AccountError::InvalidProxy(e.to_string()))?;
        let cookies = lvc_core::cookie::parse_netscape(cookie_text);
        if cookies.is_empty() {
            return Err(AccountError::CookieInvalid(
                "khong doc duoc cookie nao tu noi dung da dan (kiem tra dinh dang Netscape)"
                    .to_string(),
            ));
        }

        let profile_dir = self
            .cfg
            .accounts_dir
            .join(".logins")
            .join(safe_component(provider))
            .join(safe_component(label));
        tokio::fs::create_dir_all(&profile_dir).await?;

        let opts = LaunchOpts {
            profile_dir: profile_dir.clone(),
            browser: BrowserKind::Chromium,
            executable_override: self.cfg.executable_override.clone(),
            // headless=true: chi tiem cookie, KHONG can hien cua so (khac `login::open_login`/
            // `open_profile_viewer` — nhung ham do cho NGUOI DUNG thao tac that).
            headless: true,
            proxy: proxy_spec,
            logs_dir: self.cfg.logs_dir.clone(),
            ..LaunchOpts::default()
        };
        let session = self.driver.launch(&opts).await?;
        let result = self.driver.add_cookies(&session, &cookies).await;
        if let Err(close_err) = self.driver.close(session).await {
            warn!(provider, error = %close_err, "provision_profile: khong dong duoc phien (bo qua)");
        }
        result?;
        Ok(profile_dir)
    }
}

#[cfg(test)]
mod tests {
    use lvc_browser::{BrowserError, FakeCall, FakeStep};

    use super::*;
    use crate::generate::tests::fx;

    const COOKIE_TEXT: &str = ".google.com\tTRUE\t/\tTRUE\t1800000000\tSID\tabc123\n";

    #[tokio::test]
    async fn provision_profile_tao_thu_muc_tiem_cookie_dung_quy_uoc() {
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;
        let dir = f
            .engine
            .provision_profile("google", "Gmail acc1", COOKIE_TEXT, "")
            .await
            .unwrap();
        let expected = f
            .accounts_dir()
            .join(".logins")
            .join("google")
            .join("Gmail_acc1");
        assert_eq!(dir, expected);
        assert!(dir.is_dir(), "profile_dir phai duoc tao");
        let calls = f.calls();
        assert_eq!(
            calls.first(),
            Some(&FakeCall::Launch {
                profile_dir: expected,
                browser: BrowserKind::Chromium,
            })
        );
        assert!(
            calls.contains(&FakeCall::Close),
            "phien headless phai duoc dong sau khi tiem cookie"
        );
    }

    #[tokio::test]
    async fn provision_profile_cookie_rong_bao_loi_khong_dung_toi_dia() {
        let f = fx(&[], vec![]).await;
        let err = f
            .engine
            .provision_profile("google", "acc1", "khong-phai-cookie-netscape", "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "COOKIE_INVALID");
        let dir = f.accounts_dir().join(".logins").join("google").join("acc1");
        assert!(!dir.exists(), "cookie rong thi khong tao rac thu muc");
        assert!(f.calls().is_empty());
    }

    #[tokio::test]
    async fn provision_profile_proxy_sai_bao_loi_khong_tao_thu_muc() {
        let f = fx(&[], vec![]).await;
        let err = f
            .engine
            .provision_profile("google", "acc1", COOKIE_TEXT, "khong-hop-le")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_PROXY");
        let dir = f.accounts_dir().join(".logins").join("google").join("acc1");
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn provision_profile_launch_loi_van_tra_loi_browser() {
        let f = fx(
            &[],
            vec![FakeStep::Launch(Err(BrowserError::ProfileBusy(
                "dang bi Chrome khac dung".into(),
            )))],
        )
        .await;
        let err = f
            .engine
            .provision_profile("museai", "acc1", COOKIE_TEXT, "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "PROFILE_BUSY");
    }
}
