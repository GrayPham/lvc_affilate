//! Chinh sach log (rang buoc toan cuc, P3 Task 8): directive nguoi dung (bien moi truong
//! `RUST_LOG`, mac dinh `"info"`) roi APPEND `chromiumoxide=warn,chromiumoxide::handler=error` de
//! THANG directive truoc do — `chromiumoxide` in nguyen payload CDP (cookie/URL co chu ky) o muc
//! `debug`/`trace`, xem dau file `lvc_browser::driver`. Ghi ra `<app_data_dir>/logs/app.log` (xoay
//! theo ngay, `tracing-appender`) + stderr khi debug build.
//!
//! **Khong bao gio log**: gia tri cookie, noi dung prompt, URL co chu ky, mat khau proxy.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Ghep directive nguoi dung (`user`, `None` -> mac dinh `"info"`) voi phan bat buoc
/// `chromiumoxide=warn,chromiumoxide::handler=error` dat SAU CUNG — `EnvFilter` ap directive theo
/// thu tu xuat hien, directive muc/target cu the hon (dat sau) thang directive chung chung hon
/// (dat truoc), nen phan bat buoc nay LUON thang du nguoi dung co tu dat `chromiumoxide=debug`.
#[must_use]
pub fn log_filter(user: Option<&str>) -> String {
    let base = user.unwrap_or("info");
    format!("{base},chromiumoxide=warn,chromiumoxide::handler=error")
}

/// Cai dat `tracing` toan cuc cho tien trinh: ghi file `<logs_dir>/app.log` xoay theo ngay + (chi
/// debug build) stderr. Doc directive nguoi dung tu `RUST_LOG` (xem [`log_filter`]).
///
/// Tra ve [`WorkerGuard`] — writer khong chan (non-blocking) ghi qua kenh nen, guard nay PHAI con
/// song thi kenh moi con hoat dong; caller (`lib.rs`) leak no het doi ung dung (chap nhan mat vai
/// dong log cuoi neu tien trinh bi ket thuc dot ngot, khong flush kip).
///
/// # Errors
/// `std::io::Error` khi khong tao duoc `logs_dir`.
pub fn init(logs_dir: &Path) -> std::io::Result<WorkerGuard> {
    std::fs::create_dir_all(logs_dir)?;
    let file_appender = tracing_appender::rolling::daily(logs_dir, "app.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let user = std::env::var("RUST_LOG").ok();
    let filter = EnvFilter::new(log_filter(user.as_deref()));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);
    // Option<Layer> tu la mot Layer (None = khong lam gi) — chi bat stderr o debug build, khong
    // can hai nhanh init() rieng.
    let stderr_layer = cfg!(debug_assertions)
        .then(|| tracing_subscriber::fmt::layer().with_writer(std::io::stderr));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_ra_info_them_chromiumoxide() {
        assert_eq!(
            log_filter(None),
            "info,chromiumoxide=warn,chromiumoxide::handler=error"
        );
    }

    #[test]
    fn some_giu_directive_nguoi_dung_them_chromiumoxide() {
        assert_eq!(
            log_filter(Some("debug")),
            "debug,chromiumoxide=warn,chromiumoxide::handler=error"
        );
    }

    #[test]
    fn directive_nguoi_dung_phuc_tap_van_giu_nguyen() {
        assert_eq!(
            log_filter(Some("lvc_engine=trace,warn")),
            "lvc_engine=trace,warn,chromiumoxide=warn,chromiumoxide::handler=error"
        );
    }
}
