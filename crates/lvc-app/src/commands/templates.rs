//! Lệnh kho template (P4d). Mẫu **Public** nhúng sẵn trong binary (`crates/lvc-app/templates/*.json`,
//! chỉ đọc); mẫu **Private** lưu bảng `templates`. UI: gallery Public/Private, "Dùng mẫu" nạp graph
//! vào canvas, "Lưu thành mẫu" từ canvas. Graph là workflow JSON v2 — migrate khi đọc để mẫu cũ vẫn mở.

use lvc_browser::BrowserDriver;
use lvc_core::graph::{Graph, migrate};
use lvc_store::TemplateRepo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Một mục template cho UI (không kèm graph — nhẹ cho danh sách). `scope`: `public` | `private`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scope: String,
    /// data URL PNG hoặc rỗng (Public không có).
    pub thumbnail: String,
    pub updated_at: f64,
}

/// Template đầy đủ (kèm graph đã migrate) — cho "Dùng mẫu".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDoc {
    pub meta: TemplateMeta,
    pub graph: Graph,
}

/// Một mẫu Public nhúng: JSON `{name, description, graph}`.
#[derive(Debug, Deserialize)]
struct PublicTemplate {
    name: String,
    description: String,
    graph: Graph,
}

/// `(id, raw json)` mỗi mẫu Public. id ổn định `public:<tên file>` (lưu trong workflow người dùng).
const PUBLIC: [(&str, &str); 5] = [
    (
        "public:product-unboxing",
        include_str!("../../templates/product-unboxing.json"),
    ),
    (
        "public:talking-head-review",
        include_str!("../../templates/talking-head-review.json"),
    ),
    (
        "public:multi-scene-story",
        include_str!("../../templates/multi-scene-story.json"),
    ),
    (
        "public:refs-to-video",
        include_str!("../../templates/refs-to-video.json"),
    ),
    (
        "public:prompt-video-save",
        include_str!("../../templates/prompt-video-save.json"),
    ),
];

fn public_doc(id: &str, raw: &str) -> Option<TemplateDoc> {
    let t: PublicTemplate = serde_json::from_str(raw).ok()?;
    Some(TemplateDoc {
        meta: TemplateMeta {
            id: id.to_string(),
            name: t.name,
            description: t.description,
            scope: "public".to_string(),
            thumbnail: String::new(),
            updated_at: 0.0,
        },
        graph: migrate(t.graph),
    })
}

/// Danh sách meta template. `scope=None` → cả hai; `public`/`private` → lọc.
///
/// # Errors
/// `IpcError` (`STORE`) khi truy vấn bảng `templates` lỗi.
pub async fn template_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    scope: Option<String>,
) -> Result<Vec<TemplateMeta>, IpcError> {
    let want = scope.as_deref();
    let mut out = Vec::new();
    if want != Some("private") {
        for (id, raw) in PUBLIC {
            if let Some(d) = public_doc(id, raw) {
                out.push(d.meta);
            }
        }
    }
    if want != Some("public") {
        for row in TemplateRepo::new(&ctx.db).list().await? {
            out.push(TemplateMeta {
                id: row.id,
                name: row.name,
                description: row.description,
                scope: "private".to_string(),
                thumbnail: row.thumbnail,
                #[allow(clippy::cast_precision_loss)]
                updated_at: row.updated_at as f64,
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn template_list(
    state: tauri::State<'_, AppState>,
    scope: Option<String>,
) -> Result<Vec<TemplateMeta>, IpcError> {
    template_list_impl(&state, scope).await
}

/// Lấy template đầy đủ (Public theo id nhúng, Private theo bảng). Graph đã migrate.
///
/// # Errors
/// `IpcError` `TEMPLATE_NOT_FOUND` khi không có id; `TEMPLATE_CORRUPT` khi graph hỏng; `STORE`.
pub async fn template_get_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<TemplateDoc, IpcError> {
    if let Some((pid, raw)) = PUBLIC.iter().find(|(pid, _)| *pid == id) {
        return public_doc(pid, raw)
            .ok_or_else(|| IpcError::new("TEMPLATE_CORRUPT", "mau public hong"));
    }
    let row = TemplateRepo::new(&ctx.db)
        .get(id)
        .await?
        .ok_or_else(|| IpcError::new("TEMPLATE_NOT_FOUND", format!("khong tim thay mau {id}")))?;
    let graph: Graph = serde_json::from_str(&row.graph)
        .map_err(|e| IpcError::new("TEMPLATE_CORRUPT", format!("mau da luu hong: {e}")))?;
    Ok(TemplateDoc {
        meta: TemplateMeta {
            id: row.id,
            name: row.name,
            description: row.description,
            scope: "private".to_string(),
            thumbnail: row.thumbnail,
            #[allow(clippy::cast_precision_loss)]
            updated_at: row.updated_at as f64,
        },
        graph: migrate(graph),
    })
}

#[tauri::command]
pub async fn template_get(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<TemplateDoc, IpcError> {
    template_get_impl(&state, &id).await
}

/// Lưu một template RIÊNG từ canvas. `id` rỗng → sinh mới; có id (private) → ghi đè. Chặn ghi đè
/// id `public:*`. Graph migrate trước khi lưu.
///
/// # Errors
/// `IpcError` `TEMPLATE_READONLY` khi id là public; `GRAPH_ENCODE`/`STORE` khi ghi lỗi.
pub async fn template_save_private_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: Option<String>,
    name: &str,
    description: &str,
    graph: &Graph,
    thumbnail: &str,
) -> Result<TemplateMeta, IpcError> {
    if id.as_deref().is_some_and(|i| i.starts_with("public:")) {
        return Err(IpcError::new(
            "TEMPLATE_READONLY",
            "mau Public khong sua duoc — luu thanh mau moi",
        ));
    }
    let id = id.unwrap_or_else(|| format!("tpl_{}", Uuid::new_v4()));
    let json = serde_json::to_string(&migrate(graph.clone()))
        .map_err(|e| IpcError::new("GRAPH_ENCODE", format!("khong ma hoa duoc do thi: {e}")))?;
    let repo = TemplateRepo::new(&ctx.db);
    repo.upsert(&id, name, description, &json, thumbnail)
        .await?;
    let row = repo
        .get(&id)
        .await?
        .ok_or_else(|| IpcError::new("STORE", "vua luu ma khong doc lai duoc"))?;
    Ok(TemplateMeta {
        id: row.id,
        name: row.name,
        description: row.description,
        scope: "private".to_string(),
        thumbnail: row.thumbnail,
        #[allow(clippy::cast_precision_loss)]
        updated_at: row.updated_at as f64,
    })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn template_save_private(
    state: tauri::State<'_, AppState>,
    id: Option<String>,
    name: String,
    description: Option<String>,
    graph: Graph,
    thumbnail: Option<String>,
) -> Result<TemplateMeta, IpcError> {
    template_save_private_impl(
        &state,
        id,
        &name,
        &description.unwrap_or_default(),
        &graph,
        &thumbnail.unwrap_or_default(),
    )
    .await
}

/// Xoá template riêng (Public không xoá được).
///
/// # Errors
/// `IpcError` `TEMPLATE_READONLY` khi id public; `STORE` khi xoá lỗi.
pub async fn template_delete_private_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<bool, IpcError> {
    if id.starts_with("public:") {
        return Err(IpcError::new(
            "TEMPLATE_READONLY",
            "mau Public khong xoa duoc",
        ));
    }
    Ok(TemplateRepo::new(&ctx.db).delete(id).await?)
}

#[tauri::command]
pub async fn template_delete_private(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<bool, IpcError> {
    template_delete_private_impl(&state, &id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::ctx_with;

    #[tokio::test]
    async fn public_mau_hop_le_va_migrate_duoc() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        // 3 mau Public parse + prepare qua registry (hop le, node dung kind).
        let reg = crate::registry::build_registry();
        let limits = ctx.engine.limits();
        for (id, raw) in PUBLIC {
            let doc = public_doc(id, raw).unwrap_or_else(|| panic!("{id} hong"));
            reg.prepare(&doc.graph, limits)
                .unwrap_or_else(|e| panic!("{id} khong hop le: {e:?}"));
            assert_eq!(doc.graph.version, 2);
        }
    }

    #[tokio::test]
    async fn list_gop_public_va_private() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let only_public = template_list_impl(&ctx, Some("public".into()))
            .await
            .unwrap();
        assert_eq!(only_public.len(), 5);
        assert!(only_public.iter().all(|t| t.scope == "public"));

        let g = template_get_impl(&ctx, "public:product-unboxing")
            .await
            .unwrap()
            .graph;
        let meta = template_save_private_impl(&ctx, None, "cua toi", "ghi chu", &g, "")
            .await
            .unwrap();
        assert_eq!(meta.scope, "private");

        let all = template_list_impl(&ctx, None).await.unwrap();
        assert_eq!(all.len(), 6);
        let priv_only = template_list_impl(&ctx, Some("private".into()))
            .await
            .unwrap();
        assert_eq!(priv_only.len(), 1);
        assert_eq!(priv_only[0].name, "cua toi");
    }

    #[tokio::test]
    async fn public_khong_sua_khong_xoa() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let g = template_get_impl(&ctx, "public:prompt-video-save")
            .await
            .unwrap()
            .graph;
        let e = template_save_private_impl(&ctx, Some("public:x".into()), "n", "", &g, "")
            .await
            .unwrap_err();
        assert_eq!(e.code, "TEMPLATE_READONLY");
        assert_eq!(
            template_delete_private_impl(&ctx, "public:prompt-video-save")
                .await
                .unwrap_err()
                .code,
            "TEMPLATE_READONLY"
        );
    }

    #[tokio::test]
    async fn get_khong_co_bao_not_found() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        assert_eq!(
            template_get_impl(&ctx, "tpl_khong-co")
                .await
                .unwrap_err()
                .code,
            "TEMPLATE_NOT_FOUND"
        );
    }
}
