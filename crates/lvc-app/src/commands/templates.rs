//! Lệnh kho template (P4d). Mẫu **Public** nhúng sẵn trong binary (`crates/lvc-app/templates/*.json`,
//! chỉ đọc); mẫu **Private** lưu bảng `templates`. Mẫu **Remote** (P7) là kho CỘNG ĐỒNG đọc qua
//! `lvc-template-registry` (1 file `index.json` công khai trên GitHub), cache offline trong bảng
//! `remote_templates_cache` — xem `docs/superpowers/specs/2026-10-11-remote-template-registry-design.md`.
//! UI: gallery Public/Private/Remote, "Dùng mẫu" nạp graph vào canvas, "Lưu thành mẫu" từ canvas.
//! Graph là workflow JSON v2 — migrate khi đọc để mẫu cũ vẫn mở.

use lvc_browser::BrowserDriver;
use lvc_core::graph::{Graph, migrate};
use lvc_store::{NewRemoteTemplate, RemoteTemplateRepo, RemoteTemplateRow, TemplateRepo};
use lvc_template_registry::RegistryClient;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Một mục template cho UI (không kèm graph — nhẹ cho danh sách). `scope`: `public` | `private` |
/// `remote`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scope: String,
    /// Nhóm để lọc/gom trong gallery (vd "Affiliate", "Storyboard"). Mẫu/fixture cũ thiếu →
    /// mặc định "Cơ bản" (tương thích ngược).
    #[serde(default = "default_category")]
    pub category: String,
    /// data URL PNG hoặc rỗng (Public không có).
    pub thumbnail: String,
    pub updated_at: f64,
    /// Số node của đồ thị — thẻ trong kho hiện "N node" mà không phải tải cả đồ thị từng mẫu.
    #[serde(default)]
    pub node_count: u32,
    /// Phiên bản — CHỈ mục cộng đồng có; rỗng với mẫu dựng sẵn/riêng và với mục publish trước khi
    /// trường này tồn tại. UI dùng nó để ghi nguồn gốc khi lưu workflow dựng từ mẫu này
    /// (`commands::updates::workflow_set_origin`), nhờ đó về sau biết có bản mới hay không.
    #[serde(default)]
    pub version: String,
}

/// Nhóm mặc định khi mẫu/fixture cũ không khai `category`.
fn default_category() -> String {
    "Cơ bản".to_string()
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
    #[serde(default = "default_category")]
    category: String,
    /// Ảnh thu nhỏ cho thẻ "Kho mẫu" — data URI (`data:image/...;base64,...`) hoặc rỗng.
    #[serde(default)]
    thumbnail: String,
    graph: Graph,
}

/// Dola đang bị chặn (2026-10) → tạm ẩn các mẫu Public dùng node `dola_video` khỏi gallery
/// (`template_list`). Mở lại = đổi thành `false`. `template_get` KHÔNG chặn (workflow cũ dựng từ
/// mẫu Dola vẫn mở được nếu biết id); chỉ ẩn khỏi danh sách để người dùng không chọn mẫu đang hỏng.
const HIDE_DOLA_TEMPLATES: bool = true;

/// Mẫu Public có dùng node Dola không — soát trên JSON thô (rẻ, không cần parse Graph). Đủ tin vì
/// kind luôn là chuỗi `"dola_video"` trong JSON mẫu.
fn uses_dola(raw: &str) -> bool {
    raw.contains("\"dola_video\"")
}

/// `(id, raw json)` mỗi mẫu Public. id ổn định `public:<tên file>` (lưu trong workflow người dùng).
pub(crate) const PUBLIC: [(&str, &str); 22] = [
    (
        "public:affiliate-review",
        include_str!("../../templates/affiliate-review.json"),
    ),
    (
        "public:affiliate-multi-scene",
        include_str!("../../templates/affiliate-multi-scene.json"),
    ),
    (
        "public:affiliate-concat",
        include_str!("../../templates/affiliate-concat.json"),
    ),
    (
        "public:affiliate-before-after",
        include_str!("../../templates/affiliate-before-after.json"),
    ),
    (
        "public:affiliate-feature-demo",
        include_str!("../../templates/affiliate-feature-demo.json"),
    ),
    (
        "public:affiliate-testimonial",
        include_str!("../../templates/affiliate-testimonial.json"),
    ),
    (
        "public:product-unboxing",
        include_str!("../../templates/product-unboxing.json"),
    ),
    (
        "public:talking-head-review",
        include_str!("../../templates/talking-head-review.json"),
    ),
    (
        "public:storyboard-scenes",
        include_str!("../../templates/storyboard-scenes.json"),
    ),
    (
        "public:storyboard-ai-script",
        include_str!("../../templates/storyboard-ai-script.json"),
    ),
    (
        "public:multi-scene-story",
        include_str!("../../templates/multi-scene-story.json"),
    ),
    (
        "public:multi-scene-concat",
        include_str!("../../templates/multi-scene-concat.json"),
    ),
    (
        "public:refs-to-video",
        include_str!("../../templates/refs-to-video.json"),
    ),
    (
        "public:prompt-video-save",
        include_str!("../../templates/prompt-video-save.json"),
    ),
    (
        "public:video-trim-frame",
        include_str!("../../templates/video-trim-frame.json"),
    ),
    (
        "public:affiliate-veo3-image-to-video",
        include_str!("../../templates/affiliate-veo3-image-to-video.json"),
    ),
    (
        "public:affiliate-museai-review",
        include_str!("../../templates/affiliate-museai-review.json"),
    ),
    (
        "public:museai-product-unboxing",
        include_str!("../../templates/museai-product-unboxing.json"),
    ),
    (
        "public:museai-before-after",
        include_str!("../../templates/museai-before-after.json"),
    ),
    (
        "public:museai-feature-demo",
        include_str!("../../templates/museai-feature-demo.json"),
    ),
    (
        "public:museai-testimonial",
        include_str!("../../templates/museai-testimonial.json"),
    ),
    (
        "public:museai-reel-mon-an",
        include_str!("../../templates/museai-reel-mon-an.json"),
    ),
];

fn public_doc(id: &str, raw: &str) -> Option<TemplateDoc> {
    let t: PublicTemplate = serde_json::from_str(raw).ok()?;
    let graph = migrate(t.graph);
    let node_count = u32::try_from(graph.nodes.len()).unwrap_or(0);
    Some(TemplateDoc {
        meta: TemplateMeta {
            id: id.to_string(),
            name: t.name,
            description: t.description,
            scope: "public".to_string(),
            category: t.category,
            thumbnail: t.thumbnail,
            updated_at: 0.0,
            node_count,
            version: String::new(),
        },
        graph,
    })
}

/// `RemoteTemplateRow` (cache) -> `TemplateMeta` cho UI. `id` gắn tiền tố `"remote:"` (giống
/// `"public:"`) để `template_get`/`template_save_private`/`template_delete_private` phân biệt được
/// nguồn — id gốc trong `index.json` không có tiền tố này.
fn remote_meta(row: &RemoteTemplateRow) -> TemplateMeta {
    TemplateMeta {
        id: format!("remote:{}", row.id),
        name: row.name.clone(),
        description: row.description.clone(),
        scope: "remote".to_string(),
        category: if row.category.is_empty() {
            default_category()
        } else {
            row.category.clone()
        },
        thumbnail: row.thumbnail.clone(),
        updated_at: row.updated_at,
        node_count: count_nodes_json(&row.graph),
        version: row.version.clone(),
    }
}

/// Đếm node từ đồ thị đã mã hoá JSON mà KHÔNG parse thành `Graph` — mục cache hỏng chỉ mất con số
/// trên thẻ, không làm hỏng cả danh sách.
fn count_nodes_json(json: &str) -> u32 {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("nodes").and_then(|n| n.as_array()).map(Vec::len))
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

/// Fetch `index.json` mới nhất rồi THAY TOÀN BỘ cache (xem `RemoteTemplateRepo::replace_all`).
/// Lỗi mạng/registry trả về `Err(mo_ta)` — caller tự quyết định fallback sang cache cũ hay báo lỗi
/// (xem ADR-R021 trong spec P7).
///
/// # Errors
/// Mô tả lỗi (mạng, HTTP, JSON sai hình dạng, hoặc ghi cache thất bại) — dùng để hiển thị/log,
/// KHÔNG phải biến thể strongly-typed vì caller chỉ cần phân biệt thành công/thất bại.
async fn refresh_remote_cache<D: BrowserDriver + 'static>(ctx: &Ctx<D>) -> Result<(), String> {
    let items = RegistryClient::new(ctx.template_registry_url.clone())
        .fetch_index()
        .await
        .map_err(|e| e.to_string())?;
    // `index.json` chung cho CẢ template lẫn kịch bản (xem `lvc_template_registry::RemoteTemplate`
    // doc) — chỉ lấy `kind="workflow"` VÀ có `graph` ở đây; mục kịch bản (`kind="script"`) do
    // `commands::script_gallery::refresh_remote_cache` lọc riêng. Mục thiếu `graph` dù khai
    // `kind="workflow"` bị BỎ QUA (không làm hỏng cả lần fetch vì một mục lỗi).
    let news: Vec<NewRemoteTemplate> = items
        .into_iter()
        .filter(|t| t.kind == "workflow")
        .filter_map(|t| {
            let graph = t.graph?;
            Some(NewRemoteTemplate {
                id: t.id,
                name: t.name,
                description: t.description,
                category: if t.category.is_empty() {
                    default_category()
                } else {
                    t.category
                },
                thumbnail: t.thumbnail,
                graph: graph.to_string(),
                updated_at: t.updated_at,
                version: t.version,
                author: t.author,
                // Lưu nguyên dạng JSON: `lvc-store` không phụ thuộc kiểu của provider, còn
                // `commands::updates` parse lại khi cần hiện. Lỗi mã hoá (không thể xảy ra với
                // `Vec<ChangeEntry>`) thì lùi về mảng rỗng — không làm mất cả mục chỉ vì changelog.
                changelog: serde_json::to_string(&t.changelog).unwrap_or_else(|_| "[]".to_string()),
            })
        })
        .collect();
    RemoteTemplateRepo::new(&ctx.db)
        .replace_all(&news)
        .await
        .map_err(|e| e.to_string())
}

/// Danh sách meta template. `scope=None` → Public + Private (KHÔNG kèm Remote — Remote cần gọi
/// mạng, chỉ tải khi UI yêu cầu đích danh `scope="remote"`, tránh độ trễ bất ngờ ở các lượt gọi
/// khác). `public`/`private`/`remote` → lọc đúng một nguồn.
///
/// # Errors
/// `IpcError` (`STORE`) khi truy vấn bảng `templates`/cache lỗi; `TEMPLATE_REMOTE_UNAVAILABLE` khi
/// `scope="remote"`, fetch mạng thất bại VÀ chưa có bản cache nào để fallback.
pub async fn template_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    scope: Option<String>,
) -> Result<Vec<TemplateMeta>, IpcError> {
    let want = scope.as_deref();
    let mut out = Vec::new();
    if want != Some("private") && want != Some("remote") {
        for (id, raw) in PUBLIC {
            // Dola đang bị chặn → ẩn mẫu dùng node Dola khỏi gallery (xem HIDE_DOLA_TEMPLATES).
            if HIDE_DOLA_TEMPLATES && uses_dola(raw) {
                continue;
            }
            if let Some(d) = public_doc(id, raw) {
                out.push(d.meta);
            }
        }
    }
    if want != Some("public") && want != Some("remote") {
        for row in TemplateRepo::new(&ctx.db).list().await? {
            let node_count = count_nodes_json(&row.graph);
            out.push(TemplateMeta {
                id: row.id,
                name: row.name,
                description: row.description,
                scope: "private".to_string(),
                category: default_category(),
                thumbnail: row.thumbnail,
                #[allow(clippy::cast_precision_loss)]
                updated_at: row.updated_at as f64,
                node_count,
                version: String::new(),
            });
        }
    }
    if want == Some("remote") {
        let repo = RemoteTemplateRepo::new(&ctx.db);
        let fetch_err = refresh_remote_cache(ctx).await.err();
        let rows = repo.list().await?;
        if rows.is_empty() {
            if let Some(detail) = fetch_err {
                return Err(IpcError::new(
                    "TEMPLATE_REMOTE_UNAVAILABLE",
                    format!("khong tai duoc kho mau mien phi va chua co ban luu tam: {detail}"),
                ));
            }
        }
        out.extend(rows.iter().map(remote_meta));
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
    if let Some(raw_id) = id.strip_prefix("remote:") {
        let repo = RemoteTemplateRepo::new(&ctx.db);
        // Chua co cache (vd mo thang bang id ma chua tung mo tab "Mien phi") -> thu fetch 1 lan.
        let row = if let Some(r) = repo.get(raw_id).await? {
            r
        } else {
            refresh_remote_cache(ctx)
                .await
                .map_err(|e| IpcError::new("TEMPLATE_REMOTE_UNAVAILABLE", e))?;
            repo.get(raw_id).await?.ok_or_else(|| {
                IpcError::new("TEMPLATE_NOT_FOUND", format!("khong tim thay mau {id}"))
            })?
        };
        let graph: Graph = serde_json::from_str(&row.graph)
            .map_err(|e| IpcError::new("TEMPLATE_CORRUPT", format!("mau cong dong hong: {e}")))?;
        return Ok(TemplateDoc {
            meta: remote_meta(&row),
            graph: migrate(graph),
        });
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
            category: default_category(),
            thumbnail: row.thumbnail,
            #[allow(clippy::cast_precision_loss)]
            updated_at: row.updated_at as f64,
            node_count: u32::try_from(graph.nodes.len()).unwrap_or(0),
            version: String::new(),
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
    if id
        .as_deref()
        .is_some_and(|i| i.starts_with("public:") || i.starts_with("remote:"))
    {
        return Err(IpcError::new(
            "TEMPLATE_READONLY",
            "mau Public/Mien phi khong sua duoc — luu thanh mau moi",
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
        category: default_category(),
        thumbnail: row.thumbnail,
        #[allow(clippy::cast_precision_loss)]
        updated_at: row.updated_at as f64,
        node_count: count_nodes_json(&row.graph),
        version: String::new(),
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
    if id.starts_with("public:") || id.starts_with("remote:") {
        return Err(IpcError::new(
            "TEMPLATE_READONLY",
            "mau Public/Mien phi khong xoa duoc",
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
        // Dola bị ẩn (HIDE_DOLA_TEMPLATES): 22 mẫu Public - 14 mẫu Dola = 8 hiện ra.
        assert_eq!(only_public.len(), 8);
        assert!(only_public.iter().all(|t| t.scope == "public"));
        // KHÔNG còn mẫu Dola nào trong danh sách.
        assert!(
            !only_public
                .iter()
                .any(|t| t.id.contains("affiliate-review")),
            "mẫu Dola phải bị ẩn"
        );
        // Nhóm được khai đúng: có ít nhất một mẫu mỗi nhóm còn lại (Storyboard toàn Dola → mất).
        assert!(only_public.iter().any(|t| t.category == "Affiliate"));
        assert!(
            only_public
                .iter()
                .any(|t| t.category == "Bán hàng/Sản phẩm")
        );
        assert!(only_public.iter().any(|t| t.category == "Cơ bản"));

        // template_get KHÔNG bị chặn kể cả với mẫu Dola đã ẩn (mở được nếu biết id).
        let g = template_get_impl(&ctx, "public:product-unboxing")
            .await
            .unwrap()
            .graph;
        let meta = template_save_private_impl(&ctx, None, "cua toi", "ghi chu", &g, "")
            .await
            .unwrap();
        assert_eq!(meta.scope, "private");
        // Mẫu riêng chưa có nhóm -> mặc định "Cơ bản" (tương thích ngược).
        assert_eq!(meta.category, "Cơ bản");

        let all = template_list_impl(&ctx, None).await.unwrap();
        assert_eq!(all.len(), 9);
        let priv_only = template_list_impl(&ctx, Some("private".into()))
            .await
            .unwrap();
        assert_eq!(priv_only.len(), 1);
        assert_eq!(priv_only[0].name, "cua toi");
    }

    #[tokio::test]
    async fn gallery_an_het_dola_va_co_mau_museai_ban_hang() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let pub_list = template_list_impl(&ctx, Some("public".into()))
            .await
            .unwrap();
        // Không mẫu hiển thị nào dùng Dola (soát lại trên graph đã nạp).
        for m in &pub_list {
            let doc = template_get_impl(&ctx, &m.id).await.unwrap();
            let json = serde_json::to_string(&doc.graph).unwrap();
            assert!(!json.contains("dola_video"), "mẫu {} vẫn là Dola", m.id);
        }
        // Có đủ 5 mẫu bán hàng MuseAI để test.
        for id in [
            "public:affiliate-museai-review",
            "public:museai-product-unboxing",
            "public:museai-before-after",
            "public:museai-feature-demo",
            "public:museai-testimonial",
        ] {
            assert!(pub_list.iter().any(|m| m.id == id), "thiếu mẫu {id}");
        }
    }

    #[test]
    fn uses_dola_soat_dung() {
        assert!(uses_dola(r#"{"kind":{"kind":"dola_video"}}"#));
        assert!(!uses_dola(r#"{"kind":{"kind":"museai_video"}}"#));
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

    fn remote_index_body() -> serde_json::Value {
        serde_json::json!([{
            "id": "ugc-unboxing-demo",
            "name": "UGC Unboxing (demo)",
            "description": "mo ta",
            "category": "Bán hàng/Sản phẩm",
            "thumbnail": "https://example.com/t.svg",
            "updated_at": 1_791_713_964_000.0_f64,
            "graph": { "version": 2, "nodes": [], "edges": [] }
        }])
    }

    #[tokio::test]
    async fn remote_list_fetch_roi_cache_gan_tien_to() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/index.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(remote_index_body()))
            .mount(&server)
            .await;
        let (_dir, mut ctx, _d) = ctx_with(vec![], &[]).await;
        ctx.template_registry_url = format!("{}/index.json", server.uri());

        let list = template_list_impl(&ctx, Some("remote".into()))
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "remote:ugc-unboxing-demo");
        assert_eq!(list[0].scope, "remote");
        assert_eq!(list[0].category, "Bán hàng/Sản phẩm");

        let doc = template_get_impl(&ctx, "remote:ugc-unboxing-demo")
            .await
            .unwrap();
        assert_eq!(doc.meta.id, "remote:ugc-unboxing-demo");
        assert_eq!(doc.graph.version, 2);
    }

    #[tokio::test]
    async fn remote_list_loi_mang_khong_co_cache_bao_loi_ro() {
        let (_dir, mut ctx, _d) = ctx_with(vec![], &[]).await;
        // Cong 1 vao port that de chac chan khong co gi lang nghe (khong goi mang that, chi loi
        // ket noi cuc bo tuc thi).
        ctx.template_registry_url = "http://127.0.0.1:1/index.json".to_string();

        let err = template_list_impl(&ctx, Some("remote".into()))
            .await
            .unwrap_err();
        assert_eq!(err.code, "TEMPLATE_REMOTE_UNAVAILABLE");
    }

    #[tokio::test]
    async fn remote_list_loi_mang_nhung_da_co_cache_cu_van_tra_duoc() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/index.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(remote_index_body()))
            .mount(&server)
            .await;
        let (_dir, mut ctx, _d) = ctx_with(vec![], &[]).await;
        ctx.template_registry_url = format!("{}/index.json", server.uri());
        // Fetch thanh cong lan 1 -> co cache.
        template_list_impl(&ctx, Some("remote".into()))
            .await
            .unwrap();

        // Lan 2 mang hong nhung cache cu con -> van tra ve (khong loi).
        ctx.template_registry_url = "http://127.0.0.1:1/index.json".to_string();
        let list = template_list_impl(&ctx, Some("remote".into()))
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
    }

    /// Graph THỰC TẾ sẽ đưa vào `index.json` của repo registry ngoài
    /// (`d:\LVCMedia\lvc-template-registry`, mẫu `ugc-unboxing-demo`) — kiểm `registry.prepare`
    /// chấp nhận (đúng kind/port, giống cách `public_mau_hop_le_va_migrate_duoc` kiểm mẫu Public)
    /// TRƯỚC khi nhờ người dùng push lên GitHub thật.
    #[tokio::test]
    async fn remote_mau_thuc_te_hop_le_voi_registry() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let real_graph = serde_json::json!({
            "version": 2,
            "nodes": [
                { "id": "model", "kind": { "kind": "image_upload", "paths": [], "label": "nhân vật" }, "x": 60, "y": 80 },
                { "id": "product", "kind": { "kind": "image_upload", "paths": [], "label": "sản phẩm" }, "x": 60, "y": 300 },
                { "id": "prompt", "kind": { "kind": "prompt", "text": "demo" }, "x": 60, "y": 520 },
                { "id": "video", "kind": { "kind": "dola_video", "mode": "fast", "model": "seedance-2.5", "ratio": "9:16", "duration": 10, "logo": "off", "account": "" }, "x": 470, "y": 240 },
                { "id": "save", "kind": { "kind": "save", "out_dir": "" }, "x": 840, "y": 240 }
            ],
            "edges": [
                { "id": "e1", "from": "model", "from_port": "out", "to": "video", "to_port": "refs" },
                { "id": "e2", "from": "product", "from_port": "out", "to": "video", "to_port": "refs" },
                { "id": "e3", "from": "prompt", "from_port": "out", "to": "video", "to_port": "prompt" },
                { "id": "e4", "from": "video", "from_port": "out", "to": "save", "to_port": "video" }
            ]
        });
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/index.json"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "id": "ugc-unboxing-demo",
                    "name": "UGC Unboxing (demo miễn phí)",
                    "description": "mo ta",
                    "category": "Bán hàng/Sản phẩm",
                    "thumbnail": "https://example.com/t.svg",
                    "updated_at": 1_791_715_752_000.0_f64,
                    "graph": real_graph,
                }])),
            )
            .mount(&server)
            .await;
        let (_dir, mut ctx, _d) = ctx_with(vec![], &[]).await;
        ctx.template_registry_url = format!("{}/index.json", server.uri());

        let doc = template_get_impl(&ctx, "remote:ugc-unboxing-demo")
            .await
            .unwrap();
        let reg = crate::registry::build_registry();
        let limits = ctx.engine.limits();
        reg.prepare(&doc.graph, limits)
            .unwrap_or_else(|e| panic!("mau remote khong hop le: {e:?}"));
    }

    #[tokio::test]
    async fn remote_khong_sua_khong_xoa() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let g = template_get_impl(&ctx, "public:prompt-video-save")
            .await
            .unwrap()
            .graph;
        assert_eq!(
            template_save_private_impl(&ctx, Some("remote:x".into()), "n", "", &g, "")
                .await
                .unwrap_err()
                .code,
            "TEMPLATE_READONLY"
        );
        assert_eq!(
            template_delete_private_impl(&ctx, "remote:x")
                .await
                .unwrap_err()
                .code,
            "TEMPLATE_READONLY"
        );
    }
}
