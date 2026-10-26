//! Lệnh "Publish workflow lên GitHub" — mở Pull Request thêm/sửa một mục trong `index.json` của
//! kho cộng đồng (đối xứng với `commands::templates` chỉ ĐỌC kho đó).
//!
//! ## Bí mật
//! Token là personal access token do CHÍNH người dùng tạo, lưu mã hoá trong `lvc-vault` như một
//! API key (`provider = "github"`). Lệnh này chỉ MƯỢN plaintext trong đúng lời gọi
//! `GithubClient::publish` (`Vault::lease_secret`, không đi qua IPC) rồi thả — không log, không
//! đưa vào thông điệp lỗi (`lvc_github::GithubError` đã có test chứng minh).
//!
//! KHÔNG đẩy thẳng vào nhánh chính: luôn tạo nhánh mới + PR để chủ repo duyệt.

use lvc_browser::BrowserDriver;
use lvc_core::graph::{Graph, migrate};
use lvc_github::{GithubClient, PublishRequest, PublishResult, RepoRef};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Chủ/tên repo kho cộng đồng mặc định — cùng repo mà `lvc-template-registry` đang ĐỌC
/// (`DEFAULT_INDEX_URL`). Tách hằng riêng để đổi một chỗ.
const DEFAULT_OWNER: &str = "GrayPham";
const DEFAULT_REPO: &str = "lvc-template-registry";

/// Những gì người dùng điền trên hộp thoại Publish. Gộp thành struct (thay vì 8 tham số rời) vì
/// một mục `index.json` có nhiều trường mô tả và danh sách đó còn dài thêm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishForm {
    /// Id ổn định của mục trên kho — TRÙNG id = cập nhật mục đã publish.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    /// Tên ảnh bìa (vd `"g_food"`).
    #[serde(default)]
    pub cover: String,
    pub version: String,
    /// Tên hiện cho người tải về biết ai làm; rỗng = không hiện.
    #[serde(default)]
    pub author: String,
    /// Thay đổi ở phiên bản này, MỖI DÒNG một thay đổi dạng `"Thêm: ..."` — xem
    /// [`parse_changelog`]. Rỗng = không ghi gì (vẫn publish được).
    #[serde(default)]
    pub changelog: String,
    pub graph: Graph,
}

/// Loại thay đổi mặc định khi người dùng không ghi tiền tố.
const DEFAULT_CHANGE_KIND: &str = "Sửa";

/// Đổi khối text nhiều dòng thành danh sách thay đổi cho `index.json`.
///
/// Mỗi dòng một thay đổi. Có `":"` thì phần trước là LOẠI (`Thêm`/`Sửa`/`Bỏ`), phần sau là mô tả;
/// không có thì cả dòng là mô tả với loại [`DEFAULT_CHANGE_KIND`]. Dòng trống bị bỏ.
///
/// Dấu `-`/`*`/`•` đầu dòng được bỏ: người ta gõ danh sách theo phản xạ, không nên để ký tự đó lọt
/// vào nội dung hiện cho người khác đọc.
///
/// Loại chỉ nhận khi NGẮN (≤ 12 ký tự, không chứa khoảng trắng bất thường) — nếu không, một câu
/// bình thường có dấu hai chấm giữa câu sẽ bị cắt làm hai sai ý.
fn parse_changelog(raw: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if line.is_empty() {
            continue;
        }
        let (kind, text) = match line.split_once(':') {
            Some((k, t)) if !k.trim().is_empty() && k.trim().chars().count() <= 12 => {
                (k.trim(), t.trim())
            }
            _ => (DEFAULT_CHANGE_KIND, line),
        };
        if text.is_empty() {
            continue;
        }
        out.push(json!({ "kind": kind, "text": text }));
    }
    out
}

/// Dựng một mục `index.json` cho workflow. `kind: "workflow"` để `lvc-app` lọc đúng khi đọc lại
/// (xem `commands::templates::refresh_remote_cache`).
fn workflow_entry(form: &PublishForm, graph: &Graph) -> Result<serde_json::Value, IpcError> {
    let graph_value = serde_json::to_value(graph)
        .map_err(|e| IpcError::new("GRAPH_ENCODE", format!("khong ma hoa duoc do thi: {e}")))?;
    Ok(json!({
        "id": form.id,
        "kind": "workflow",
        "name": form.name,
        "description": form.description,
        "category": form.category,
        "thumbnail": form.cover,
        "version": form.version,
        "author": form.author,
        "changelog": parse_changelog(&form.changelog),
        "graph": graph_value,
    }))
}

/// Publish một workflow lên kho cộng đồng. `credential_id` là id credential `provider="github"`
/// (token người dùng đã lưu ở tab API key).
///
/// # Errors
/// `IpcError`: `GRAPH_INVALID` khi đồ thị sai; `CRED_NOT_FOUND` khi không có token;
/// `GITHUB_*` (xem `lvc_github::GithubError::code`) khi GitHub từ chối.
pub async fn publish_workflow_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    credential_id: &str,
    form: &PublishForm,
) -> Result<PublishResult, IpcError> {
    let graph = migrate(form.graph.clone());
    // Kiem hop le TRUOC khi dua len kho chung — khong publish do thi hong cho nguoi khac tai ve.
    crate::registry::build_registry()
        .prepare(&graph, ctx.engine.limits())
        .map_err(|errs| {
            IpcError::new("GRAPH_INVALID", "do thi khong hop le")
                .with_details(serde_json::json!(errs))
        })?;

    let entry = workflow_entry(form, &graph)?;

    // Mượn plaintext token trong đúng phạm vi lời gọi này.
    let (token, _meta) = ctx.vault.lease_secret(credential_id).await?;

    GithubClient::default()
        .publish(
            &RepoRef::new(DEFAULT_OWNER, DEFAULT_REPO),
            &token,
            &PublishRequest {
                id: form.id.clone(),
                name: form.name.clone(),
                version: form.version.clone(),
                entry,
            },
        )
        .await
        .map_err(|e| IpcError::new(e.code(), e.to_string()))
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn publish_workflow(
    license: tauri::State<'_, super::license::LicenseState>,
    state: tauri::State<'_, AppState>,
    credential_id: String,
    form: PublishForm,
) -> Result<PublishResult, IpcError> {
    license.require_licensed()?;
    publish_workflow_impl(&state, &credential_id, &form).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::ctx_with;
    use lvc_core::graph::{Node, NodeData};

    fn simple_graph() -> Graph {
        Graph {
            version: 2,
            nodes: vec![Node {
                id: "p".to_string(),
                kind: NodeData::new("prompt", serde_json::json!({ "text": "canh 1" })),
                x: 0.0,
                y: 0.0,
            }],
            edges: vec![],
        }
    }

    fn form(graph: Graph) -> PublishForm {
        PublishForm {
            id: "wf-1".to_string(),
            name: "Ten hien thi".to_string(),
            description: "Mo ta".to_string(),
            category: "Affiliate".to_string(),
            cover: "g_live".to_string(),
            version: "1.3.0".to_string(),
            author: "son vu".to_string(),
            changelog: String::new(),
            graph,
        }
    }

    #[test]
    fn entry_co_du_truong_kho_cong_dong_can() {
        let e = workflow_entry(&form(simple_graph()), &simple_graph()).unwrap();
        assert_eq!(e["id"], "wf-1");
        assert_eq!(e["kind"], "workflow", "phai co kind de ben doc loc dung");
        assert_eq!(e["version"], "1.3.0");
        assert_eq!(e["category"], "Affiliate");
        assert_eq!(e["author"], "son vu");
        assert!(e["graph"]["nodes"].is_array());
    }

    #[test]
    fn changelog_moi_dong_mot_thay_doi_co_tien_to_loai() {
        let out = parse_changelog("Thêm: node phụ đề\nSửa: prompt cảnh 2");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["kind"], "Thêm");
        assert_eq!(out[0]["text"], "node phụ đề");
        assert_eq!(out[1]["kind"], "Sửa");
    }

    #[test]
    fn changelog_khong_co_tien_to_thi_mac_dinh_sua() {
        let out = parse_changelog("đổi tỉ lệ sang 9:16");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["kind"], DEFAULT_CHANGE_KIND);
        assert_eq!(
            out[0]["text"], "đổi tỉ lệ sang 9:16",
            "dau hai cham GIUA cau khong duoc cat lam loai"
        );
    }

    #[test]
    fn changelog_bo_dau_gach_dau_dong() {
        let out = parse_changelog("- Thêm: a\n* Sửa: b\n• Bỏ: c");
        assert_eq!(out.len(), 3);
        assert_eq!(out[0]["text"], "a");
        assert_eq!(out[2]["kind"], "Bỏ");
    }

    #[test]
    fn changelog_bo_dong_trong_va_dong_chi_co_loai() {
        let out = parse_changelog("\n  \nThêm: a\nSửa:\n");
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0]["text"], "a");
    }

    #[test]
    fn changelog_rong_ra_mang_rong_khong_phai_loi() {
        assert!(parse_changelog("").is_empty());
        assert!(parse_changelog("   \n  ").is_empty());
    }

    #[test]
    fn changelog_tien_to_qua_dai_khong_bi_coi_la_loai() {
        // Mot cau dai co dau hai cham -> KHONG cat, giu nguyen ca cau.
        let out = parse_changelog("Lưu ý quan trọng với người dùng: phải có tài khoản");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["kind"], DEFAULT_CHANGE_KIND);
        assert!(
            out[0]["text"]
                .as_str()
                .unwrap()
                .starts_with("Lưu ý quan trọng")
        );
    }

    #[tokio::test]
    async fn do_thi_hong_bi_chan_truoc_khi_cham_toi_github() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let bad = Graph {
            version: 2,
            nodes: vec![Node {
                id: "s".to_string(),
                // `save` bat buoc co dau vao -> do thi khong hop le.
                kind: NodeData::new("save", serde_json::json!({ "out_dir": "" })),
                x: 0.0,
                y: 0.0,
            }],
            edges: vec![],
        };

        // `credential_id` khong ton tai — neu kiem do thi chay TRUOC thi loi phai la GRAPH_INVALID,
        // chung minh khong he dung toi token/mang.
        let err = publish_workflow_impl(&ctx, "khong-co", &form(bad))
            .await
            .unwrap_err();
        assert_eq!(err.code, "GRAPH_INVALID");
    }

    #[tokio::test]
    async fn thieu_token_bao_cred_not_found() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let err = publish_workflow_impl(&ctx, "khong-co", &form(simple_graph()))
            .await
            .unwrap_err();
        assert_eq!(err.code, "CRED_NOT_FOUND");
    }
}
