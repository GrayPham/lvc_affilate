//! Lệnh "Cập nhật workflow" — đối chiếu workflow người dùng đã lưu với kho cộng đồng để biết mục
//! nào có bản mới hơn, rồi áp bản mới theo cách người dùng chọn.
//!
//! ## Dựa vào đâu để biết "có bản mới"
//! `workflow_origin` (migration `0010`) ghi: workflow này tải từ mục nào, ở phiên bản nào, và đồ
//! thị LÚC ĐÓ ra sao. So ba thứ đó với `remote_templates_cache`:
//!
//! - phiên bản khác nhau → có bản mới;
//! - `workflows.json` khác `workflow_origin.source_json` → người dùng ĐÃ SỬA bản trên máy, nên
//!   hộp thoại phải hỏi "Giữ cả hai" hay "Ghi đè" thay vì âm thầm đè mất công sửa.
//!
//! Mục chưa có `version` (publish trước khi trường đó tồn tại) được coi là KHÔNG có cập nhật —
//! không thể so sánh thì không báo, thà im lặng hơn báo sai.
//!
//! ## Vì sao không so bằng `updated_at`
//! `updated_at` đổi cả khi tác giả chỉ sửa mô tả. Phiên bản là thứ tác giả CHỦ ĐỘNG tăng khi đồ
//! thị thật sự đổi, nên dùng nó mới đúng ý "có bản mới để tải".

use lvc_browser::BrowserDriver;
use lvc_core::graph::{Graph, migrate};
use lvc_store::{RemoteTemplateRepo, WorkflowOriginRepo, WorkflowRepo};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Một dòng thay đổi để hiện trong hộp thoại.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeLine {
    pub kind: String,
    pub text: String,
}

/// Một bản cập nhật đang chờ, đủ dữ liệu để dựng hộp thoại không phải gọi thêm lệnh nào.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowUpdate {
    /// Workflow trên máy sẽ được cập nhật.
    pub workflow_id: String,
    pub workflow_name: String,
    /// Id mục trong kho cộng đồng (chưa gắn tiền tố `"remote:"`).
    pub source_id: String,
    /// Phiên bản đang dùng trên máy.
    pub current_version: String,
    /// Phiên bản mới trong kho.
    pub new_version: String,
    pub author: String,
    pub thumbnail: String,
    /// Thời điểm mục trong kho được cập nhật (giây Unix) — UI tự đổi thành "5 ngày trước".
    pub updated_at: f64,
    pub changelog: Vec<ChangeLine>,
    /// `true` = `workflows.json` đã khác đồ thị lúc tải về. Hộp thoại dùng nó để quyết định có
    /// hỏi "Giữ cả hai / Ghi đè" hay cập nhật thẳng.
    pub locally_modified: bool,
}

/// Cách áp bản cập nhật.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    /// Tạo workflow MỚI mang bản mới, giữ nguyên bản đang có trên máy — mặc định trên UI khi
    /// người dùng đã sửa, vì không bao giờ làm mất công sửa của họ.
    KeepBoth,
    /// Ghi đè workflow đang có bằng bản mới; phần đã sửa trên máy mất.
    Overwrite,
}

/// So HAI đồ thị theo NỘI DUNG, không theo chuỗi JSON thô.
///
/// Chuỗi thô khác nhau là chuyện thường dù đồ thị y hệt: thứ tự khoá, số `1.0` vs `1`, khoảng
/// trắng. So thô sẽ báo "đã sửa" cho mọi workflow vừa tải về xong — đúng kiểu cảnh báo giả làm
/// người dùng mất tin vào hộp thoại.
fn graph_changed(current_json: &str, origin_json: &str) -> bool {
    let a: Result<Value, _> = serde_json::from_str(current_json);
    let b: Result<Value, _> = serde_json::from_str(origin_json);
    match (a, b) {
        (Ok(a), Ok(b)) => a != b,
        // Một bên không parse được thì không thể kết luận gì — coi như CHƯA sửa để không hỏi oan;
        // người dùng vẫn cập nhật được bình thường.
        _ => false,
    }
}

/// Parse `changelog` đã lưu dạng JSON; hỏng/rỗng → danh sách rỗng (không phải lỗi).
fn parse_changelog(raw: &str) -> Vec<ChangeLine> {
    serde_json::from_str(raw).unwrap_or_default()
}

/// Danh sách bản cập nhật đang chờ. Mảng rỗng = không có gì mới (trạng thái thường gặp nhất).
///
/// KHÔNG tự gọi mạng: chỉ đọc cache `remote_templates_cache` mà tab "Miễn phí" đã nạp. Mở màn
/// nào cũng nã một lượt HTTP là cách nhanh nhất làm app chậm và bị kho chặn.
///
/// # Errors
/// `IpcError` mã `STORE` khi đọc DB lỗi.
pub async fn workflow_update_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
) -> Result<Vec<WorkflowUpdate>, IpcError> {
    let origins = WorkflowOriginRepo::new(&ctx.db).list().await?;
    let workflows = WorkflowRepo::new(&ctx.db);
    let remote = RemoteTemplateRepo::new(&ctx.db);

    let mut out = Vec::new();
    for origin in origins {
        // Workflow đã bị xoá mà dòng nguồn gốc còn sót (vd xoá bằng bản app cũ hơn) -> bỏ qua.
        let Some(wf) = workflows.get(&origin.workflow_id).await? else {
            continue;
        };
        let Some(row) = remote.get(&origin.source_id).await? else {
            continue;
        };
        // Không có phiên bản ở MỘT trong hai phía thì không so được -> không báo.
        if row.version.is_empty() || origin.source_version.is_empty() {
            continue;
        }
        if row.version == origin.source_version {
            continue;
        }
        out.push(WorkflowUpdate {
            workflow_id: origin.workflow_id,
            workflow_name: wf.name,
            source_id: origin.source_id,
            current_version: origin.source_version,
            new_version: row.version,
            author: row.author,
            thumbnail: row.thumbnail,
            updated_at: row.updated_at,
            changelog: parse_changelog(&row.changelog),
            locally_modified: graph_changed(&wf.json, &origin.source_json),
        });
    }
    Ok(out)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "chu ky `#[tauri::command]` bat buoc nhan State theo gia tri - bien gioi IPC"
)]
#[tauri::command]
pub async fn workflow_update_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<WorkflowUpdate>, IpcError> {
    workflow_update_list_impl(&state).await
}

/// Ghi nhớ workflow này tải từ mục nào của kho — gọi ngay sau khi người dùng lưu một workflow
/// dựng từ mẫu cộng đồng. `source_id` nhận cả dạng có tiền tố `"remote:"` lẫn id gốc.
///
/// Đồ thị được `migrate` + mã hoá lại TRƯỚC khi lưu làm ảnh chụp, để sau này so sánh "đã sửa
/// chưa" không bị lệch chỉ vì hai bên dùng phiên bản lược đồ khác nhau.
///
/// # Errors
/// `IpcError` mã `STORE` khi ghi DB lỗi.
pub async fn workflow_set_origin_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    workflow_id: &str,
    source_id: &str,
    source_version: &str,
    graph: &Graph,
) -> Result<(), IpcError> {
    let raw_id = source_id.strip_prefix("remote:").unwrap_or(source_id);
    let json = serde_json::to_string(&migrate(graph.clone()))
        .map_err(|e| IpcError::new("GRAPH_ENCODE", format!("khong ma hoa duoc do thi: {e}")))?;
    WorkflowOriginRepo::new(&ctx.db)
        .set(workflow_id, raw_id, source_version, &json)
        .await?;
    Ok(())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn workflow_set_origin(
    state: tauri::State<'_, AppState>,
    workflow_id: String,
    source_id: String,
    source_version: String,
    graph: Graph,
) -> Result<(), IpcError> {
    workflow_set_origin_impl(&state, &workflow_id, &source_id, &source_version, &graph).await
}

/// Áp bản cập nhật.
///
/// - [`UpdateMode::Overwrite`]: ghi đè `workflows.json` của chính workflow đó + cập nhật ảnh chụp
///   nguồn gốc. Trả về id CŨ.
/// - [`UpdateMode::KeepBoth`]: tạo workflow MỚI tên `"<tên cũ> (<phiên bản mới>)"`, bản cũ giữ
///   nguyên cả đồ thị lẫn dòng nguồn gốc của nó. Trả về id MỚI.
///
/// Đồ thị mới được kiểm hợp lệ TRƯỚC khi ghi — một mục kho hỏng không được phép đè lên workflow
/// đang chạy tốt.
///
/// # Errors
/// `IpcError`: `WORKFLOW_NOT_FOUND`/`UPDATE_NOT_FOUND` khi không còn mục tương ứng;
/// `TEMPLATE_CORRUPT` khi đồ thị trong kho không đọc được; `GRAPH_INVALID` khi đồ thị mới sai;
/// `STORE` khi ghi DB lỗi.
pub async fn workflow_apply_update_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    workflow_id: &str,
    mode: UpdateMode,
) -> Result<String, IpcError> {
    let origin_repo = WorkflowOriginRepo::new(&ctx.db);
    let origin = origin_repo.get(workflow_id).await?.ok_or_else(|| {
        IpcError::new(
            "UPDATE_NOT_FOUND",
            "workflow nay khong den tu kho cong dong",
        )
    })?;
    let workflows = WorkflowRepo::new(&ctx.db);
    let wf = workflows.get(workflow_id).await?.ok_or_else(|| {
        IpcError::new(
            "WORKFLOW_NOT_FOUND",
            format!("khong tim thay workflow {workflow_id}"),
        )
    })?;
    let row = RemoteTemplateRepo::new(&ctx.db)
        .get(&origin.source_id)
        .await?
        .ok_or_else(|| {
            IpcError::new(
                "UPDATE_NOT_FOUND",
                format!("kho khong con muc {}", origin.source_id),
            )
        })?;

    let graph: Graph = serde_json::from_str(&row.graph)
        .map_err(|e| IpcError::new("TEMPLATE_CORRUPT", format!("mau cong dong hong: {e}")))?;
    let graph = migrate(graph);
    crate::registry::build_registry()
        .prepare(&graph, ctx.engine.limits())
        .map_err(|errs| {
            IpcError::new("GRAPH_INVALID", "do thi ban moi khong hop le")
                .with_details(serde_json::json!(errs))
        })?;
    let json = serde_json::to_string(&graph)
        .map_err(|e| IpcError::new("GRAPH_ENCODE", format!("khong ma hoa duoc do thi: {e}")))?;

    let target_id = match mode {
        UpdateMode::Overwrite => workflow_id.to_string(),
        UpdateMode::KeepBoth => Uuid::new_v4().to_string(),
    };
    let name = match mode {
        UpdateMode::Overwrite => wf.name.clone(),
        UpdateMode::KeepBoth => format!("{} ({})", wf.name, row.version),
    };

    workflows
        .upsert(&target_id, &name, &json, lvc_engine::now_unix())
        .await?;
    origin_repo
        .set(&target_id, &origin.source_id, &row.version, &json)
        .await?;
    Ok(target_id)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn workflow_apply_update(
    state: tauri::State<'_, AppState>,
    workflow_id: String,
    mode: UpdateMode,
) -> Result<String, IpcError> {
    workflow_apply_update_impl(&state, &workflow_id, mode).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::ctx_with;
    use lvc_core::graph::{Node, NodeData};
    use lvc_store::NewRemoteTemplate;

    fn graph_with(text: &str) -> Graph {
        Graph {
            version: 2,
            nodes: vec![Node {
                id: "p".to_string(),
                kind: NodeData::new("prompt", serde_json::json!({ "text": text })),
                x: 0.0,
                y: 0.0,
            }],
            edges: vec![],
        }
    }

    fn remote(id: &str, version: &str, graph: &Graph) -> NewRemoteTemplate {
        NewRemoteTemplate {
            id: id.to_string(),
            name: "Review mon an quan quen".to_string(),
            description: "mo ta".to_string(),
            category: "Affiliate".to_string(),
            thumbnail: "g_food".to_string(),
            graph: serde_json::to_string(graph).unwrap(),
            updated_at: 1_700_000_000.0,
            version: version.to_string(),
            author: "son vu".to_string(),
            changelog: serde_json::json!([
                { "kind": "Thêm", "text": "Node “Chèn phụ đề” sau cảnh 3." },
                { "kind": "Sửa", "text": "Prompt cảnh 2 nhấn mạnh hơi nóng." }
            ])
            .to_string(),
        }
    }

    #[test]
    fn graph_changed_bo_qua_khac_biet_hinh_thuc_cua_json() {
        // Cung noi dung, khac thu tu khoa + khoang trang -> KHONG duoc coi la da sua.
        assert!(!graph_changed(
            r#"{"a":1,"b":[1,2]}"#,
            "{\n  \"b\": [1, 2],\n  \"a\": 1\n}"
        ));
        assert!(graph_changed(r#"{"a":1}"#, r#"{"a":2}"#));
    }

    #[test]
    fn graph_changed_json_hong_coi_nhu_chua_sua_khong_hoi_oan() {
        assert!(!graph_changed("khong-phai-json", r#"{"a":1}"#));
        assert!(!graph_changed(r#"{"a":1}"#, "khong-phai-json"));
    }

    #[test]
    fn parse_changelog_hong_ra_rong_khong_panic() {
        assert!(parse_changelog("khong-phai-json").is_empty());
        assert!(parse_changelog("[]").is_empty());
        let one = parse_changelog(r#"[{"kind":"Thêm","text":"abc"}]"#);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].kind, "Thêm");
    }

    #[tokio::test]
    async fn khong_co_nguon_goc_thi_khong_co_cap_nhat() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Tu dung", "{}", 1.0)
            .await
            .unwrap();
        assert!(workflow_update_list_impl(&ctx).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cung_phien_ban_thi_khong_bao_cap_nhat() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let g = graph_with("canh 1");
        let json = serde_json::to_string(&g).unwrap();
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Review", &json, 1.0)
            .await
            .unwrap();
        WorkflowOriginRepo::new(&ctx.db)
            .set("wf1", "review", "1.0.0", &json)
            .await
            .unwrap();
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "1.0.0", &g)])
            .await
            .unwrap();

        assert!(workflow_update_list_impl(&ctx).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn phien_ban_moi_hon_thi_bao_du_thong_tin_cho_hop_thoai() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let cu = graph_with("canh 1");
        let json = serde_json::to_string(&cu).unwrap();
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Review món ăn quán quen", &json, 1.0)
            .await
            .unwrap();
        WorkflowOriginRepo::new(&ctx.db)
            .set("wf1", "review", "1.0.0", &json)
            .await
            .unwrap();
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "1.0.1", &graph_with("canh 1 moi"))])
            .await
            .unwrap();

        let list = workflow_update_list_impl(&ctx).await.unwrap();
        assert_eq!(list.len(), 1);
        let u = &list[0];
        assert_eq!(u.workflow_name, "Review món ăn quán quen");
        assert_eq!(u.current_version, "1.0.0");
        assert_eq!(u.new_version, "1.0.1");
        assert_eq!(u.author, "son vu");
        assert_eq!(u.thumbnail, "g_food");
        assert_eq!(u.changelog.len(), 2);
        assert!(
            !u.locally_modified,
            "chua sua gi tren may thi khong duoc bao da sua"
        );
    }

    #[tokio::test]
    async fn sua_tren_may_thi_bao_locally_modified() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let goc = serde_json::to_string(&graph_with("canh 1")).unwrap();
        let da_sua = serde_json::to_string(&graph_with("canh 1 toi tu sua")).unwrap();
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Review", &da_sua, 1.0)
            .await
            .unwrap();
        WorkflowOriginRepo::new(&ctx.db)
            .set("wf1", "review", "1.0.0", &goc)
            .await
            .unwrap();
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "1.0.1", &graph_with("canh moi"))])
            .await
            .unwrap();

        let list = workflow_update_list_impl(&ctx).await.unwrap();
        assert!(list[0].locally_modified);
    }

    #[tokio::test]
    async fn kho_chua_ghi_version_thi_khong_bao_gi() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let json = serde_json::to_string(&graph_with("canh 1")).unwrap();
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Review", &json, 1.0)
            .await
            .unwrap();
        WorkflowOriginRepo::new(&ctx.db)
            .set("wf1", "review", "1.0.0", &json)
            .await
            .unwrap();
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "", &graph_with("khac"))])
            .await
            .unwrap();

        assert!(
            workflow_update_list_impl(&ctx).await.unwrap().is_empty(),
            "khong so duoc phien ban thi tha im lang hon bao sai"
        );
    }

    #[tokio::test]
    async fn workflow_da_xoa_khong_con_bao_cap_nhat() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        WorkflowOriginRepo::new(&ctx.db)
            .set("da-xoa", "review", "1.0.0", "{}")
            .await
            .unwrap();
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "1.0.1", &graph_with("x"))])
            .await
            .unwrap();

        assert!(workflow_update_list_impl(&ctx).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn ghi_de_giu_nguyen_id_va_cap_nhat_anh_chup_nguon_goc() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let cu = serde_json::to_string(&graph_with("canh 1")).unwrap();
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Review", &cu, 1.0)
            .await
            .unwrap();
        WorkflowOriginRepo::new(&ctx.db)
            .set("wf1", "review", "1.0.0", &cu)
            .await
            .unwrap();
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "1.0.1", &graph_with("canh 1 moi"))])
            .await
            .unwrap();

        let id = workflow_apply_update_impl(&ctx, "wf1", UpdateMode::Overwrite)
            .await
            .unwrap();

        assert_eq!(id, "wf1", "ghi de KHONG tao workflow moi");
        let wf = WorkflowRepo::new(&ctx.db)
            .get("wf1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(wf.name, "Review", "ten giu nguyen khi ghi de");
        assert!(wf.json.contains("canh 1 moi"));
        // Da cap nhat anh chup -> khong con bao cap nhat, cung khong con bao "da sua".
        assert!(workflow_update_list_impl(&ctx).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn giu_ca_hai_tao_workflow_moi_va_khong_cham_ban_cu() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let cu = serde_json::to_string(&graph_with("ban toi tu sua")).unwrap();
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Review", &cu, 1.0)
            .await
            .unwrap();
        WorkflowOriginRepo::new(&ctx.db)
            .set("wf1", "review", "1.0.0", &cu)
            .await
            .unwrap();
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "1.0.1", &graph_with("ban moi"))])
            .await
            .unwrap();

        let new_id = workflow_apply_update_impl(&ctx, "wf1", UpdateMode::KeepBoth)
            .await
            .unwrap();

        assert_ne!(new_id, "wf1");
        let repo = WorkflowRepo::new(&ctx.db);
        let cu_row = repo.get("wf1").await.unwrap().unwrap();
        assert!(
            cu_row.json.contains("ban toi tu sua"),
            "ban cu PHAI con nguyen cong sua cua nguoi dung"
        );
        let moi = repo.get(&new_id).await.unwrap().unwrap();
        assert_eq!(moi.name, "Review (1.0.1)");
        assert!(moi.json.contains("ban moi"));

        // Ban cu VAN con cap nhat cho (nguoi dung co the doi y ghi de sau), ban moi thi khong.
        let still = workflow_update_list_impl(&ctx).await.unwrap();
        assert_eq!(still.len(), 1);
        assert_eq!(still[0].workflow_id, "wf1");
    }

    #[tokio::test]
    async fn do_thi_ban_moi_hong_bi_chan_khong_de_len_workflow_dang_tot() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let cu = serde_json::to_string(&graph_with("ban tot")).unwrap();
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Review", &cu, 1.0)
            .await
            .unwrap();
        WorkflowOriginRepo::new(&ctx.db)
            .set("wf1", "review", "1.0.0", &cu)
            .await
            .unwrap();
        // `save` bat buoc co dau vao -> do thi khong hop le.
        let hong = Graph {
            version: 2,
            nodes: vec![Node {
                id: "s".to_string(),
                kind: NodeData::new("save", serde_json::json!({ "out_dir": "" })),
                x: 0.0,
                y: 0.0,
            }],
            edges: vec![],
        };
        RemoteTemplateRepo::new(&ctx.db)
            .replace_all(&[remote("review", "1.0.1", &hong)])
            .await
            .unwrap();

        let err = workflow_apply_update_impl(&ctx, "wf1", UpdateMode::Overwrite)
            .await
            .unwrap_err();
        assert_eq!(err.code, "GRAPH_INVALID");

        let wf = WorkflowRepo::new(&ctx.db)
            .get("wf1")
            .await
            .unwrap()
            .unwrap();
        assert!(wf.json.contains("ban tot"), "workflow cu phai con nguyen");
    }

    #[tokio::test]
    async fn workflow_tu_dung_khong_ap_duoc_cap_nhat() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        WorkflowRepo::new(&ctx.db)
            .upsert("wf1", "Tu dung", "{}", 1.0)
            .await
            .unwrap();
        let err = workflow_apply_update_impl(&ctx, "wf1", UpdateMode::Overwrite)
            .await
            .unwrap_err();
        assert_eq!(err.code, "UPDATE_NOT_FOUND");
    }

    #[tokio::test]
    async fn set_origin_bo_tien_to_remote_va_chuan_hoa_do_thi() {
        let (_d, ctx, _drv) = ctx_with(vec![], &[]).await;
        let g = graph_with("canh 1");
        workflow_set_origin_impl(&ctx, "wf1", "remote:review", "1.0.0", &g)
            .await
            .unwrap();

        let origin = WorkflowOriginRepo::new(&ctx.db)
            .get("wf1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(origin.source_id, "review", "phai bo tien to 'remote:'");
        assert_eq!(origin.source_version, "1.0.0");
        // Anh chup la do thi DA migrate -> so sanh sau nay khong lech vi phien ban luoc do.
        let snapshot: Graph = serde_json::from_str(&origin.source_json).unwrap();
        assert_eq!(snapshot, migrate(g));
    }
}
