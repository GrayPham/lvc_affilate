//! Client HTTP tới server cấp phép (rustls): đăng nhập tài khoản + kích hoạt license (payload mã
//! hoá ECDH, verify chữ ký 4 lớp). Lỗi quy về thông điệp UI CHUNG — KHÔNG lộ lớp nào fail, KHÔNG log
//! bí mật.

use base64::Engine as _;
use serde_json::{Value, json};

use crate::crypto::encrypt_data;
use crate::device;
use crate::verify::{LicenseInfo, check_license_response};

/// Lỗi cấp phép — `Display` là thông điệp UI CHUNG (không mách nước lớp nào fail, không kèm bí mật).
#[derive(Debug, thiserror::Error)]
pub enum LicenseError {
    #[error("Lỗi kết nối — kiểm tra mạng rồi thử lại")]
    Network,
    #[error("Sai tài khoản hoặc mật khẩu")]
    BadCredentials,
    #[error("Không xác minh được phản hồi máy chủ")]
    VerifyFailed,
}

/// Client cấp phép. `pubkey_pem`/`api_url`/`tool_code` tiêm vào (test dùng mock + key test).
pub struct LicenseClient {
    api_url: String,
    pubkey_pem: String,
    tool_code: String,
    device_id: String,
    device_info: Value,
    http: reqwest::Client,
}

impl LicenseClient {
    /// Tạo client (tính `device_id`/`device_info` của máy hiện tại).
    #[must_use]
    pub fn new(
        api_url: impl Into<String>,
        pubkey_pem: impl Into<String>,
        tool_code: impl Into<String>,
    ) -> Self {
        Self {
            api_url: api_url.into(),
            pubkey_pem: pubkey_pem.into(),
            tool_code: tool_code.into(),
            device_id: device::device_id(),
            device_info: device::device_info(),
            http: reqwest::Client::new(),
        }
    }

    /// Đăng nhập tài khoản → JWT `access_token`.
    ///
    /// # Errors
    /// [`LicenseError::Network`] khi mất mạng; [`LicenseError::BadCredentials`] khi server ≠ 200 hoặc
    /// thiếu `access_token`.
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, LicenseError> {
        let resp = self
            .http
            .post(format!("{}/api/v1/auth/login", self.api_url))
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .map_err(|_| LicenseError::Network)?;
        if resp.status().as_u16() != 200 {
            return Err(LicenseError::BadCredentials);
        }
        let body: Value = resp.json().await.map_err(|_| LicenseError::Network)?;
        body.get("access_token")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or(LicenseError::BadCredentials)
    }

    /// Kích hoạt license: gửi payload mã hoá → verify phản hồi (4 lớp guard) → trả [`LicenseInfo`].
    ///
    /// # Errors
    /// [`LicenseError::Network`] khi mất mạng; [`LicenseError::VerifyFailed`] khi mã hoá/HTTP≠200/
    /// phản hồi không qua guard.
    pub async fn verify_license(
        &self,
        license_key: &str,
        auth_token: &str,
        username: &str,
    ) -> Result<LicenseInfo, LicenseError> {
        let nonce = random_hex16();
        let now = chrono::Utc::now();
        let verify_data = json!({
            "license_key": license_key,
            "device_id": self.device_id,
            "device_name": device::device_name(),
            "device_info": self.device_info,
            "tool_code": self.tool_code,
            "timestamp": iso_utc(now),
            "nonce": nonce,
        });
        let encrypted_payload =
            encrypt_data(&self.pubkey_pem, &verify_data).map_err(|_| LicenseError::VerifyFailed)?;
        let user_id = jwt_sub(auth_token).unwrap_or_else(|| username.to_string());
        let auth_data = json!({
            "user": username,
            "user_id": user_id,
            "token": auth_token,
            "expires_at": iso_utc(now + chrono::Duration::hours(1)),
        });
        let encrypted_auth =
            encrypt_data(&self.pubkey_pem, &auth_data).map_err(|_| LicenseError::VerifyFailed)?;
        let resp = self
            .http
            .post(format!("{}/api/v1/verify/secure-verify", self.api_url))
            .json(&json!({
                "encrypted_payload": encrypted_payload,
                "tool_code": self.tool_code,
                "auth_token": encrypted_auth,
            }))
            .send()
            .await
            .map_err(|_| LicenseError::Network)?;
        if resp.status().as_u16() != 200 {
            return Err(LicenseError::VerifyFailed);
        }
        let result: Value = resp.json().await.map_err(|_| LicenseError::Network)?;
        match check_license_response(
            &result,
            &self.pubkey_pem,
            &nonce,
            &self.device_id,
            license_key,
            now.timestamp(),
            86400,
        ) {
            Ok(info) => Ok(info),
            Err(e) => {
                self.report_tamper(&e.to_string()).await;
                Err(LicenseError::VerifyFailed)
            }
        }
    }

    /// Báo tamper best-effort (không bao giờ throw, không chặn luồng chính).
    async fn report_tamper(&self, reason: &str) {
        let detail: String = reason.chars().take(200).collect();
        let _ = self
            .http
            .post(format!("{}/api/v1/audit/report", self.api_url))
            .json(&json!({
                "event": "signature_mismatch",
                "tool_code": self.tool_code,
                "device_id": self.device_id,
                "detail": detail,
            }))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
    }
}

fn random_hex16() -> String {
    use rand::RngCore as _;
    use std::fmt::Write as _;
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    let mut s = String::with_capacity(32);
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

fn iso_utc(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
}

/// Lấy `sub` từ JWT (CHỈ base64url-decode payload, KHÔNG verify chữ ký — khớp Veo3). `None` nếu lỗi.
fn jwt_sub(token: &str) -> Option<String> {
    let part = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(part)
        .ok()?;
    let v: Value = serde_json::from_slice(&decoded).ok()?;
    v.get("sub").and_then(Value::as_str).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::pkcs8::EncodePublicKey as _;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_pubkey() -> String {
        p256::SecretKey::random(&mut rand::rngs::OsRng)
            .public_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap()
    }

    #[tokio::test]
    async fn authenticate_200_tra_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/auth/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token":"t"})))
            .mount(&server)
            .await;
        let c = LicenseClient::new(server.uri(), test_pubkey(), "lvc-shop");
        assert_eq!(c.authenticate("u", "p").await.unwrap(), "t");
    }

    #[tokio::test]
    async fn authenticate_401_bad_credentials() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/auth/login"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({"detail":"no"})))
            .mount(&server)
            .await;
        let c = LicenseClient::new(server.uri(), test_pubkey(), "lvc-shop");
        assert!(matches!(
            c.authenticate("u", "p").await,
            Err(LicenseError::BadCredentials)
        ));
    }

    #[tokio::test]
    async fn verify_license_500_tra_verifyfailed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/verify/secure-verify"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let c = LicenseClient::new(server.uri(), test_pubkey(), "lvc-shop");
        assert!(matches!(
            c.verify_license("K", "authtok", "user").await,
            Err(LicenseError::VerifyFailed)
        ));
    }

    #[tokio::test]
    async fn verify_license_mat_mang_tra_network() {
        let c = LicenseClient::new("http://127.0.0.1:1", test_pubkey(), "lvc-shop");
        assert!(matches!(
            c.verify_license("K", "authtok", "user").await,
            Err(LicenseError::Network)
        ));
    }
}
