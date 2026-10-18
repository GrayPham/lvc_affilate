//! Kiem tra proxy — backend cho nut "Kiem tra proxy" o hop thoai "Proxy & trinh duyet"
//! (`references/lvc-ui-design/screens/AssignProxy.dc.html`).
//!
//! Mot lan GET nhe qua proxy toi dich vu "IP cua ban" de lay: IP RA (may chu dich nhin thay sau
//! proxy), vi tri (quoc gia), do tre. KHONG dung cho automation, khong ton credit — chi chay khi
//! nguoi dung bam nut.
//!
//! An toan bi mat (luat #3 CLAUDE.md): moi thong diep loi dung `reqwest::Error::without_url()` nen
//! khong lo URL; proxy chi di vao `reqwest::Proxy` (user/pass qua `basic_auth`) — KHONG BAO GIO
//! ghep vao chuoi log/loi. Han che tai dung client proxy giong `dola::http::DolaHttp::new`.

use std::time::{Duration, Instant};

use lvc_core::proxy::ProxySpec;
use serde::{Deserialize, Serialize};

use crate::error::BrowserError;

/// Dich vu tra IP ra + quoc gia, mien phi, khong can key. Chi xin ba truong can dung.
/// HTTP (ban mien phi ip-api khong co HTTPS) — request chi hoi "IP cua toi", khong mang bi mat.
const IPAPI_URL: &str = "http://ip-api.com/json/?fields=status,message,country,query";

/// Thoi gian cho mac dinh cho mot lan kiem (proxy cham van kip, nhung khong treo UI qua lau).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Ket qua kiem tra proxy — camelCase cho UI (`outboundIp`/`location`/`latencyMs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyProbe {
    /// IP ma may chu dich NHIN THAY (IP ra sau proxy). Rong neu dich vu khong tra.
    pub outbound_ip: String,
    /// Vi tri — ten quoc gia dich vu tra ve (vd "Vietnam"); rong neu khong co.
    pub location: String,
    /// Do tre tron goi GET (ms).
    pub latency_ms: u64,
}

/// Hinh JSON cua ip-api.com — chi doc truong can.
#[derive(Deserialize)]
struct IpApi {
    status: String,
    message: Option<String>,
    country: Option<String>,
    query: Option<String>,
}

/// Dung [`ProxyProbe`] tu than JSON ip-api — THUAN (khong I/O), test duoc khong can mang.
///
/// # Errors
/// [`BrowserError::ProxyTest`] khi JSON khong doc duoc hoac `status != "success"` (dich vu bao loi,
/// vd proxy chan dich vu nay).
fn parse_ipapi(body: &str, latency_ms: u64) -> Result<ProxyProbe, BrowserError> {
    let r: IpApi = serde_json::from_str(body)
        .map_err(|e| BrowserError::ProxyTest(format!("phan hoi khong phai JSON hop le: {e}")))?;
    if r.status != "success" {
        return Err(BrowserError::ProxyTest(format!(
            "dich vu IP bao loi: {}",
            r.message.as_deref().unwrap_or("khong ro")
        )));
    }
    Ok(ProxyProbe {
        outbound_ip: r.query.unwrap_or_default(),
        location: r.country.unwrap_or_default(),
        latency_ms,
    })
}

/// Noi thu qua `proxy` (`None` = noi truc tiep, lay IP may) va tra IP ra + vi tri + do tre.
///
/// # Errors
/// [`BrowserError::ProxyTest`] khi khong dung duoc client, khong noi duoc (proxy chet/sai), qua
/// thoi gian, hoac dich vu tra loi khong doc duoc. Thong diep KHONG chua mat khau proxy.
pub async fn test_proxy(
    proxy: Option<&ProxySpec>,
    timeout: Duration,
) -> Result<ProxyProbe, BrowserError> {
    // `no_proxy()` truoc: khong doc HTTP(S)_PROXY cua he thong — chi dung dung proxy duoc truyen.
    let mut builder = reqwest::Client::builder().timeout(timeout).no_proxy();
    if let Some(spec) = proxy {
        let server = spec.server();
        let mut p = reqwest::Proxy::all(&server).map_err(|e| {
            BrowserError::ProxyTest(format!("proxy khong hop le: {}", e.without_url()))
        })?;
        if spec.has_auth() {
            p = p.basic_auth(
                spec.username.as_deref().unwrap_or(""),
                spec.password.as_deref().unwrap_or(""),
            );
        }
        builder = builder.proxy(p);
    }
    let client = builder.build().map_err(|e| {
        BrowserError::ProxyTest(format!("khong khoi tao duoc client: {}", e.without_url()))
    })?;

    let start = Instant::now();
    let resp = client.get(IPAPI_URL).send().await.map_err(|e| {
        BrowserError::ProxyTest(format!("khong noi duoc qua proxy: {}", e.without_url()))
    })?;
    // Do tre do NGAY sau khi nhan phan hoi dau (truoc khi doc body) — phan anh RTT noi mang.
    let latency_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let body = resp
        .text()
        .await
        .map_err(|e| BrowserError::ProxyTest(format!("doc phan hoi loi: {}", e.without_url())))?;
    parse_ipapi(&body, latency_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_thanh_cong_lay_ip_va_quoc_gia() {
        let body = r#"{"status":"success","country":"Vietnam","query":"113.161.4.22"}"#;
        let p = parse_ipapi(body, 182).unwrap();
        assert_eq!(p.outbound_ip, "113.161.4.22");
        assert_eq!(p.location, "Vietnam");
        assert_eq!(p.latency_ms, 182);
    }

    #[test]
    fn parse_status_fail_thanh_loi_co_message() {
        let body = r#"{"status":"fail","message":"private range","query":"10.0.0.1"}"#;
        let e = parse_ipapi(body, 10).unwrap_err();
        let s = e.to_string();
        assert!(s.contains("private range"), "{s}");
    }

    #[test]
    fn parse_status_fail_khong_message_van_co_thong_diep() {
        let body = r#"{"status":"fail"}"#;
        let e = parse_ipapi(body, 10).unwrap_err();
        assert!(e.to_string().contains("khong ro"), "{e}");
    }

    #[test]
    fn parse_thieu_truong_khong_panic() {
        // Dich vu doi hinh/thieu field: `status=success` nhung khong co query/country -> rong,
        // khong panic.
        let body = r#"{"status":"success"}"#;
        let p = parse_ipapi(body, 5).unwrap();
        assert_eq!(p.outbound_ip, "");
        assert_eq!(p.location, "");
    }

    #[test]
    fn parse_khong_phai_json_thanh_loi() {
        let e = parse_ipapi("<html>503</html>", 5).unwrap_err();
        assert!(e.to_string().contains("JSON"), "{e}");
    }
}
