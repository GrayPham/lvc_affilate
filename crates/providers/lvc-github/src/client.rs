//! Client REST GitHub cho luồng publish. Năm bước, đúng thứ tự:
//!
//! 1. `GET /repos/{o}/{r}` — lấy nhánh mặc định (không đoán `main`/`master`).
//! 2. `GET /repos/{o}/{r}/git/ref/heads/{base}` — lấy SHA đầu nhánh đó.
//! 3. `POST /repos/{o}/{r}/git/refs` — tạo nhánh mới từ SHA đó.
//! 4. `GET` + `PUT /repos/{o}/{r}/contents/{path}` — đọc `index.json`, chèn/thay mục, ghi lên nhánh
//!    mới (kèm `sha` bản cũ để GitHub từ chối nếu file đã đổi trong lúc đó).
//! 5. `POST /repos/{o}/{r}/pulls` — mở Pull Request.
//!
//! Token chỉ đi qua tham số `token` của [`GithubClient::publish`], không lưu vào struct.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::json;

use crate::error::GithubError;
use crate::model::{PublishRequest, PublishResult, upsert_entry};

/// GitHub yêu cầu User-Agent cho mọi request API.
const USER_AGENT: &str = "lvc-affiliate";
const API_VERSION: &str = "2022-11-28";

/// Repo kho cộng đồng + đường dẫn file danh mục bên trong nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub repo: String,
    /// Đường dẫn `index.json` trong repo (mặc định `index.json` ở gốc).
    pub index_path: String,
}

impl RepoRef {
    #[must_use]
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            repo: repo.into(),
            index_path: "index.json".to_string(),
        }
    }
}

/// Client ghi lên GitHub. Rẻ để `clone`; KHÔNG giữ token.
#[derive(Debug, Clone)]
pub struct GithubClient {
    http: reqwest::Client,
    api_base: String,
}

impl Default for GithubClient {
    fn default() -> Self {
        Self::new("https://api.github.com")
    }
}

impl GithubClient {
    #[must_use]
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_base: api_base.into(),
        }
    }

    fn req(&self, method: reqwest::Method, url: &str, token: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .bearer_auth(token)
    }

    /// Gửi request rồi trả JSON đã parse. `e.without_url()` để URL (có thể kèm tham số) không lọt
    /// vào thông điệp lỗi.
    async fn send_json(
        &self,
        rb: reqwest::RequestBuilder,
        what: &str,
    ) -> Result<serde_json::Value, GithubError> {
        let resp = rb
            .send()
            .await
            .map_err(|e| GithubError::Network(e.without_url().to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| GithubError::Network(e.without_url().to_string()))?;
        if status == 404 {
            return Err(GithubError::NotFound {
                what: what.to_string(),
            });
        }
        if !(200..=299).contains(&status) {
            return Err(GithubError::http(status, &body));
        }
        serde_json::from_str(&body).map_err(|e| GithubError::Decode(e.to_string()))
    }

    /// Chạy trọn luồng publish: tạo nhánh → cập nhật `index.json` → mở PR.
    ///
    /// # Errors
    /// [`GithubError`] khi mạng lỗi, token thiếu quyền (401/403), repo/file không có (404),
    /// `index.json` không phải mảng, hoặc GitHub trả mã lỗi khác.
    pub async fn publish(
        &self,
        repo: &RepoRef,
        token: &str,
        req: &PublishRequest,
    ) -> Result<PublishResult, GithubError> {
        let base_url = format!("{}/repos/{}/{}", self.api_base, repo.owner, repo.repo);

        // 1. Nhánh mặc định — KHÔNG đoán "main".
        let info = self
            .send_json(
                self.req(reqwest::Method::GET, &base_url, token),
                "repo kho cong dong",
            )
            .await?;
        let base_branch = info
            .get("default_branch")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("main")
            .to_string();

        // 2. SHA đầu nhánh gốc.
        let ref_url = format!("{base_url}/git/ref/heads/{base_branch}");
        let base_ref = self
            .send_json(
                self.req(reqwest::Method::GET, &ref_url, token),
                "nhanh mac dinh",
            )
            .await?;
        let base_sha = base_ref
            .pointer("/object/sha")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| GithubError::Decode("thieu object.sha".to_string()))?
            .to_string();

        // 3. Nhánh mới. Tên gắn id + phiên bản để người duyệt nhìn là hiểu.
        let branch = branch_name(&req.id, &req.version);
        let create_ref = self
            .req(
                reqwest::Method::POST,
                &format!("{base_url}/git/refs"),
                token,
            )
            .json(&json!({ "ref": format!("refs/heads/{branch}"), "sha": base_sha }));
        self.send_json(create_ref, "tao nhanh moi").await?;

        // 4. Đọc index.json ở nhánh gốc, chèn/thay mục, ghi lên nhánh mới.
        let contents_url = format!("{base_url}/contents/{}", repo.index_path);
        let file = self
            .send_json(
                self.req(
                    reqwest::Method::GET,
                    &format!("{contents_url}?ref={base_branch}"),
                    token,
                ),
                "index.json",
            )
            .await?;
        let file_sha = file
            .get("sha")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| GithubError::Decode("thieu sha cua index.json".to_string()))?
            .to_string();
        let raw = file
            .get("content")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| GithubError::Decode("thieu content cua index.json".to_string()))?;
        let decoded = decode_b64_content(raw)?;
        let index: serde_json::Value =
            serde_json::from_str(&decoded).map_err(|e| GithubError::Decode(e.to_string()))?;

        let (next, updated_existing) = upsert_entry(&index, &req.id, &req.entry)?;
        let pretty = serde_json::to_string_pretty(&next)
            .map_err(|e| GithubError::Decode(e.to_string()))?
            + "\n";

        let put = self
            .req(reqwest::Method::PUT, &contents_url, token)
            .json(&json!({
                "message": commit_message(&req.name, &req.version, updated_existing),
                "content": B64.encode(pretty.as_bytes()),
                "sha": file_sha,
                "branch": branch,
            }));
        self.send_json(put, "ghi index.json").await?;

        // 5. Mở PR.
        let pr = self
            .req(reqwest::Method::POST, &format!("{base_url}/pulls"), token)
            .json(&json!({
                "title": commit_message(&req.name, &req.version, updated_existing),
                "head": branch,
                "base": base_branch,
                "body": pr_body(&req.name, &req.version, updated_existing),
            }));
        let pr = self.send_json(pr, "mo pull request").await?;
        let pull_request_url = pr
            .get("html_url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();

        Ok(PublishResult {
            pull_request_url,
            branch,
            updated_existing,
        })
    }
}

/// Tên nhánh an toàn: chỉ chữ/số/`-`, tránh ký tự Git không cho phép.
fn branch_name(id: &str, version: &str) -> String {
    let slug: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let ver: String = version
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("publish/{slug}-{ver}")
}

fn commit_message(name: &str, version: &str, updated: bool) -> String {
    let verb = if updated { "Cap nhat" } else { "Them" };
    format!("{verb} \"{name}\" ({version})")
}

fn pr_body(name: &str, version: &str, updated: bool) -> String {
    let what = if updated {
        "Cap nhat mot muc da co"
    } else {
        "Them mot muc moi"
    };
    format!(
        "{what} trong `index.json`.\n\n- Ten: {name}\n- Phien ban: {version}\n\n\
         Tao tu ung dung LVC Affiliate."
    )
}

/// GitHub trả `content` base64 CÓ xuống dòng mỗi 60 ký tự — phải bỏ khoảng trắng trước khi giải mã.
fn decode_b64_content(raw: &str) -> Result<String, GithubError> {
    let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = B64
        .decode(compact.as_bytes())
        .map_err(|e| GithubError::Decode(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| GithubError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_name_loai_ky_tu_khong_an_toan() {
        assert_eq!(branch_name("abc-123", "1.0.0"), "publish/abc-123-1-0-0");
        assert_eq!(
            branch_name("co dau/va:ky tu", "2.0"),
            "publish/co-dau-va-ky-tu-2-0"
        );
    }

    #[test]
    fn commit_message_phan_biet_them_va_cap_nhat() {
        assert!(commit_message("A", "1.0", false).starts_with("Them"));
        assert!(commit_message("A", "1.0", true).starts_with("Cap nhat"));
    }

    #[test]
    fn decode_b64_bo_qua_xuong_dong_cua_github() {
        // GitHub tra `content` la MOT khoi base64 nhung CO xuong dong moi 60 ky tu.
        let json = r#"[{"id":"mot-muc-kha-dai-de-base64-vuot-qua-60-ky-tu","name":"A"}]"#;
        let wrapped = B64
            .encode(json.as_bytes())
            .as_bytes()
            .chunks(60)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(wrapped.contains('\n'), "fixture phai co xuong dong");
        assert_eq!(decode_b64_content(&wrapped).unwrap(), json);
    }

    #[test]
    fn decode_b64_hong_bao_loi_decode() {
        assert_eq!(
            decode_b64_content("khong-phai-base64!!!")
                .unwrap_err()
                .code(),
            "GITHUB_DECODE"
        );
    }

    #[test]
    fn repo_ref_mac_dinh_index_o_goc() {
        let r = RepoRef::new("GrayPham", "lvc-template-registry");
        assert_eq!(r.index_path, "index.json");
    }
}
