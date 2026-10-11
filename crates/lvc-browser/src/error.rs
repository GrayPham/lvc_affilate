//! Loi cua lvc-browser (ADR-R002): bao boc loi CDP/I-O/JSON thanh thong diep tieng Viet.

use std::path::PathBuf;

use lvc_core::BrowserKind;

/// Hien thi danh sach duong dan da thu trong `ExecutableNotFound` (rong -> ghi ro ly do).
fn tried_display(tried: &[PathBuf]) -> String {
    if tried.is_empty() {
        return "(khong co duong dan nao de thu - thieu bien moi truong goc)".to_string();
    }
    tried
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    /// Review P1 I-6 / ADR-R012: khong resolve duoc file chay cho `kind` - KHONG de chromiumoxide
    /// tu do Chrome (profile patchright chi mo dung bang Chromium cung ban). `tried` = moi duong
    /// dan da kiem tra theo thu tu (xem `executable.rs`).
    #[error(
        "khong tim thay file chay trinh duyet '{}' - da thu: {}",
        .kind.as_str(),
        tried_display(.tried)
    )]
    ExecutableNotFound {
        kind: BrowserKind,
        tried: Vec<PathBuf>,
    },
    /// Ngoai le JavaScript khi `Runtime.evaluate` (ops.rs): thong diep da rut gon
    /// (`exception.description` hoac `text`) thay vi `Debug` cua ca `ExceptionDetails`.
    #[error("loi JavaScript trong trang: {0}")]
    Js(String),
    #[error("khong khoi dong duoc trinh duyet: {0}")]
    Launch(String),
    /// I-1: `CdpError::LaunchExit` tach duoc ma thoat + stderr co cau truc (khac `Launch`
    /// von chi con mot chuoi `Display` da render san) khi tien trinh Chrome thoat truoc khi
    /// websocket URL duoc doc xong, MA KHONG phai loi `ProfileBusy` (ma 21/24 - xem `ProfileBusy`).
    #[error("trinh duyet thoat truoc khi ket noi CDP (ma thoat {code:?}): {stderr}")]
    LaunchExit { code: Option<i32>, stderr: String },
    /// Loi CDP con lai (khong phai [`Self::Disconnected`] — xem `impl From<CdpError>`).
    #[error("loi CDP: {0}")]
    Cdp(chromiumoxide::error::CdpError),
    #[error("profile dang bi tien trinh khac dung: {0}")]
    ProfileBusy(String),
    #[error("loi I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("loi JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Task 13: `Page.navigate` tra `errorText` (vd `net::ERR_PROXY_CONNECTION_FAILED`,
    /// `net::ERR_CONNECTION_REFUSED`). `url_host_path` = URL DA CAT query/fragment
    /// ([`url_host_path`]) — khong bao gio dua URL co chu ky vao thong diep loi/log.
    #[error("dieu huong that bai toi {url_host_path}: {net_error}")]
    Navigation {
        url_host_path: String,
        net_error: String,
    },
    /// M-6: het thoi gian cho (vd `goto` khong "load" xong trong 60s).
    #[error("qua thoi gian cho: {0}")]
    Timeout(String),
    /// Task 13: handler CDP da ket thuc / Chrome chet / target bi huy — kenh lenh dong
    /// (`CdpError::ChannelSendError`). Engine (T15) dong session nay va mo lai, khong thu lai lenh.
    #[error("mat ket noi CDP (Chrome da thoat hoac trang bi huy)")]
    Disconnected,
    /// Task 8 (`veo3::harvest`): URL sau dieu huong roi vao `/about`/`accounts.google.com`/trang
    /// dang nhap — profile CHUA dang nhap Google/Flow (loi tai khoan, khong phai loi Chrome).
    /// `0` = URL do (khong co query/fragment nhay cam trong truong hop nay, nhung van giu nguyen
    /// de chan doan — URL flow.google.com khong mang chu ky nhu Dola).
    #[error("profile chua dang nhap flow.google.com (url hien tai: {0})")]
    NotLoggedIn(String),
    /// Task 8 (`veo3::harvest`): doc duoc `at` (`WIZ_global_data.SNlM0e`) nhung cookie
    /// flow.google.com KHONG co bat ky khoa dang nhap nao (`SID`/`__Secure-1PSID`/
    /// `__Secure-3PSID`/`__Secure-1PSIDTS`) — dau hieu AV/khoa profile chan doc cookie DB. Loi
    /// THAT da gap ben Python: cache "at co, cookie rong" lam moi request sau deu 401 toi khi
    /// restart app — PHAI tra loi o day, KHONG cho qua. `0` = danh sach TEN cookie da thay (KHONG
    /// bao gio gia tri).
    #[error("moi phien thieu cookie dang nhap thuc (chi thay ten: {0})")]
    MissingAuthCookie(String),
}

/// `#[from]` thu cong: kenh lenh dong (`ChannelSendError` = handler da dung / oneshot bi huy) →
/// [`BrowserError::Disconnected`] de moi `?` trong crate cung mot phan loai; con lai → `Cdp`.
impl From<chromiumoxide::error::CdpError> for BrowserError {
    fn from(e: chromiumoxide::error::CdpError) -> Self {
        match e {
            chromiumoxide::error::CdpError::ChannelSendError(_) => Self::Disconnected,
            other => Self::Cdp(other),
        }
    }
}

/// URL bo phan query (`?…`) va fragment (`#…`) — chi con `scheme://host/path` de dua vao thong diep
/// loi/log (URL Dola co chu ky trong query; chinh sach log o `lib.rs`).
#[must_use]
pub fn url_host_path(url: &str) -> String {
    let end = url.find(['?', '#']).unwrap_or(url.len());
    url[..end].to_string()
}

impl BrowserError {
    /// Task 13: loi dieu huong do PROXY chet/khong noi duoc (`net::ERR_PROXY_*`, `net::ERR_TUNNEL_*`
    /// (CONNECT qua proxy HTTP), `net::ERR_SOCKS_*`) — engine gan `FailureCode::ProxyDown` cho tai
    /// khoan do va xoay sang tai khoan khac (plan T15 buoc 4). Loi mang khac (`ERR_CONNECTION_REFUSED`,
    /// `ERR_NAME_NOT_RESOLVED`…) va moi bien the khac → `false`.
    #[must_use]
    pub fn is_proxy_down(&self) -> bool {
        match self {
            Self::Navigation { net_error, .. } => {
                net_error.contains("ERR_PROXY_")
                    || net_error.contains("ERR_TUNNEL_")
                    || net_error.contains("ERR_SOCKS_")
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_not_found_liet_ke_tried_tieng_viet() {
        let e = BrowserError::ExecutableNotFound {
            kind: BrowserKind::Chrome,
            tried: vec![
                PathBuf::from(r"C:\a\chrome.exe"),
                PathBuf::from(r"D:\b\chrome.exe"),
            ],
        };
        let s = e.to_string();
        assert!(s.contains("'chrome'"), "{s}");
        assert!(s.contains(r"C:\a\chrome.exe; D:\b\chrome.exe"), "{s}");
    }

    /// Task 13: chi 3 ho ma loi proxy (`ERR_PROXY_` / `ERR_TUNNEL_` / `ERR_SOCKS_`) la "proxy chet".
    #[test]
    fn is_proxy_down_theo_ho_ma_loi() {
        let nav = |net_error: &str| BrowserError::Navigation {
            url_host_path: "https://www.dola.com/chat".into(),
            net_error: net_error.into(),
        };
        assert!(nav("net::ERR_PROXY_CONNECTION_FAILED").is_proxy_down());
        assert!(nav("net::ERR_TUNNEL_CONNECTION_FAILED").is_proxy_down());
        assert!(nav("net::ERR_SOCKS_CONNECTION_FAILED").is_proxy_down());
        assert!(nav("net::ERR_PROXY_AUTH_UNSUPPORTED").is_proxy_down());
        assert!(!nav("net::ERR_CONNECTION_REFUSED").is_proxy_down());
        assert!(!nav("net::ERR_NAME_NOT_RESOLVED").is_proxy_down());
        assert!(!nav("").is_proxy_down());
        assert!(!BrowserError::Timeout("ERR_PROXY_CONNECTION_FAILED".into()).is_proxy_down());
        assert!(!BrowserError::Disconnected.is_proxy_down());
        assert!(!BrowserError::Launch("x".into()).is_proxy_down());
    }

    /// Thong diep `Navigation` chi co host/path + ma loi, khong query.
    #[test]
    fn navigation_display_khong_co_query() {
        let e = BrowserError::Navigation {
            url_host_path: url_host_path("https://www.dola.com/chat?sig=BIMAT#x"),
            net_error: "net::ERR_PROXY_CONNECTION_FAILED".into(),
        };
        let s = e.to_string();
        assert_eq!(
            s,
            "dieu huong that bai toi https://www.dola.com/chat: net::ERR_PROXY_CONNECTION_FAILED"
        );
        assert!(!s.contains("BIMAT"));
    }

    #[test]
    fn url_host_path_cat_query_va_fragment() {
        assert_eq!(url_host_path("https://a.b/c?x=1&y=2"), "https://a.b/c");
        assert_eq!(url_host_path("https://a.b/c#frag"), "https://a.b/c");
        assert_eq!(url_host_path("https://a.b/c#f?x"), "https://a.b/c");
        assert_eq!(url_host_path("http://127.0.0.1:1/"), "http://127.0.0.1:1/");
        assert_eq!(url_host_path(""), "");
    }

    /// Kenh lenh dong (`ChannelSendError`) → `Disconnected`; loi CDP khac giu `Cdp`.
    #[test]
    fn from_cdp_error_kenh_dong_thanh_disconnected() {
        use chromiumoxide::error::{CdpError, ChannelError};
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        drop(tx);
        let canceled = futures::executor::block_on(rx).unwrap_err();
        let e: BrowserError = CdpError::ChannelSendError(ChannelError::Canceled(canceled)).into();
        assert!(matches!(e, BrowserError::Disconnected), "{e:?}");
        assert_eq!(
            e.to_string(),
            "mat ket noi CDP (Chrome da thoat hoac trang bi huy)"
        );

        let e: BrowserError = CdpError::msg("net::ERR_X").into();
        assert!(matches!(e, BrowserError::Cdp(_)), "{e:?}");
        let e: BrowserError = CdpError::Timeout.into();
        assert!(matches!(e, BrowserError::Cdp(CdpError::Timeout)), "{e:?}");
    }

    #[test]
    fn executable_not_found_tried_rong_van_co_thong_diep() {
        let e = BrowserError::ExecutableNotFound {
            kind: BrowserKind::Opera,
            tried: vec![],
        };
        let s = e.to_string();
        assert!(s.contains("'opera'"), "{s}");
        assert!(s.contains("khong co duong dan nao"), "{s}");
    }
}
