//! Smoke test THẬT cho `veo3::captcha::GuestCaptchaBrowser` — mở 1 Chrome KHÁCH (ẩn danh, profile
//! tạm KHÔNG cookie tài khoản nào), vào `https://flow.google.com/about`, chèn `enterprise.js` rồi
//! mint 1 token reCAPTCHA Enterprise THẬT. Đây là hành động SỐNG (mở tiến trình Chrome thật + gọi
//! ra Internet tới Google) — dù rủi ro gần như 0 (không đụng tài khoản Google/Dola nào, xem
//! `src/veo3/MEMORY.md`), test này CẦN Chrome thật + mạng nên `#[ignore]`: KHÔNG chạy trong CI hay
//! `scripts/check.ps1` thường. Chạy tay khi cần kiểm tra hồi quy:
//! `cargo test -p lvc-browser --test veo3_captcha_smoke -- --ignored --nocapture`

use std::time::Duration;

use lvc_browser::veo3::captcha::{GuestCaptchaBrowser, GuestCaptchaOpts, MIN_TOKEN_LEN};

#[tokio::test]
#[ignore = "can Chrome THAT + mang toi Google, khong chay trong CI/check.ps1 thuong; \
            chay tay: cargo test -p lvc-browser --test veo3_captcha_smoke -- --ignored --nocapture"]
async fn mint_token_thuc_bang_trinh_duyet_khach() {
    let dir = tempfile::tempdir().unwrap();
    let opts = GuestCaptchaOpts::default();
    let mut browser = GuestCaptchaBrowser::open(dir.path(), &opts)
        .await
        .expect("mo Chrome khach + nap enterprise.js");

    let token = browser
        .mint("VIDEO_GENERATION", Duration::from_secs(60))
        .await
        .expect("mint token reCAPTCHA");

    println!("token nhan duoc dai {} ky tu", token.len());
    assert!(!token.is_empty(), "token khong duoc rong");
    if token.len() < MIN_TOKEN_LEN {
        // Khong panic — chi la dau hieu Google da nghi ngo phien (xem MEMORY.md); van la mot
        // token, chi khong "khoe" nhu ky vong (~2200-2700 ky tu).
        eprintln!(
            "canh bao: token ngan hon nguong {MIN_TOKEN_LEN} ky tu (thuc nhan {} ky tu)",
            token.len()
        );
    }

    browser.close().await;
}
