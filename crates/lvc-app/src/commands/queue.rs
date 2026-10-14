//! Lệnh hàng chờ workflow (màn "Tác vụ") — chạy TUẦN TỰ từng workflow một.
//!
//! Khác `run_start` (chạy NGAY một đồ thị trên canvas): ở đây người dùng xếp nhiều workflow vào
//! hàng, bấm "Chạy hàng chờ", engine lấy từng mục theo thứ tự. Đồ thị được CHỤP LẠI lúc thêm
//! (`queue_items.graph`) nên sửa workflow gốc sau đó không làm đổi mục đang chờ.
//!
//! Bộ chạy là MỘT task nền duy nhất (`spawn_runner`): giữ `QueueRunning` trong `Ctx` để không bao
//! giờ có hai vòng chạy song song (hàng chờ mà chạy song song thì mất ý nghĩa "tuần tự", và sẽ
//! tranh nhau tài khoản).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lvc_browser::BrowserDriver;
use lvc_core::OnError;
use lvc_core::graph::{Graph, migrate};
use lvc_store::{NewQueueItem, QueueRepo, QueueStatus};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use super::{Ctx, IpcError};
use crate::state::AppState;
use lvc_engine::noop_hooks_factory;

/// Một mục hàng chờ trả cho UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemDto {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub position: i64,
    /// `waiting` | `running` | `done` | `failed` | `skipped`.
    pub status: String,
    pub run_id: Option<String>,
    pub error: String,
    pub created_at: f64,
    pub started_at: Option<f64>,
    pub finished_at: Option<f64>,
}

/// Tình trạng chung của hàng chờ — UI dùng để bật/tắt nút "Chạy"/"Dừng".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    pub items: Vec<QueueItemDto>,
    /// Bộ chạy đang hoạt động (kể cả lúc đang nghỉ giữa hai workflow).
    pub running: bool,
}

fn dto(row: &lvc_store::QueueRow) -> QueueItemDto {
    QueueItemDto {
        id: row.id.clone(),
        workflow_id: row.workflow_id.clone(),
        name: row.name.clone(),
        position: row.position,
        status: row.status.as_str().to_string(),
        run_id: row.run_id.clone(),
        error: row.error.clone(),
        created_at: row.created_at,
        started_at: row.started_at,
        finished_at: row.finished_at,
    }
}

/// # Errors
/// `IpcError` (`STORE`) khi truy vấn lỗi.
pub async fn queue_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
) -> Result<QueueState, IpcError> {
    let items = QueueRepo::new(&ctx.db).list().await?;
    Ok(QueueState {
        items: items.iter().map(dto).collect(),
        running: ctx.queue_running.load(Ordering::SeqCst),
    })
}

#[tauri::command]
pub async fn queue_list(state: tauri::State<'_, AppState>) -> Result<QueueState, IpcError> {
    queue_list_impl(&state).await
}

/// Thêm một workflow vào CUỐI hàng chờ. `graph` được migrate + kiểm hợp lệ TRƯỚC khi lưu — hàng
/// chờ không nhận đồ thị hỏng (nếu không sẽ chỉ phát hiện lúc tới lượt chạy, rất khó hiểu).
///
/// # Errors
/// `IpcError` `GRAPH_INVALID` khi đồ thị sai; `GRAPH_ENCODE`/`STORE` khi ghi lỗi.
pub async fn queue_add_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    workflow_id: &str,
    name: &str,
    graph: &Graph,
) -> Result<QueueItemDto, IpcError> {
    let graph = migrate(graph.clone());
    // Kiem hop le ngay luc THEM — bao loi som thay vi doi toi luot chay moi vo.
    crate::registry::build_registry()
        .prepare(&graph, ctx.engine.limits())
        .map_err(|errs| {
            IpcError::new("GRAPH_INVALID", "do thi khong hop le")
                .with_details(serde_json::json!(errs))
        })?;
    let json = serde_json::to_string(&graph)
        .map_err(|e| IpcError::new("GRAPH_ENCODE", format!("khong ma hoa duoc do thi: {e}")))?;
    let row = QueueRepo::new(&ctx.db)
        .push(&NewQueueItem {
            id: format!("q_{}", Uuid::new_v4()),
            workflow_id: workflow_id.to_string(),
            name: name.to_string(),
            graph: json,
        })
        .await?;
    Ok(dto(&row))
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn queue_add(
    state: tauri::State<'_, AppState>,
    workflow_id: String,
    name: String,
    graph: Graph,
) -> Result<QueueItemDto, IpcError> {
    queue_add_impl(&state, &workflow_id, &name, &graph).await
}

/// # Errors
/// `IpcError` (`STORE`) khi xoá lỗi.
pub async fn queue_remove_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<bool, IpcError> {
    Ok(QueueRepo::new(&ctx.db).remove(id).await?)
}

#[tauri::command]
pub async fn queue_remove(state: tauri::State<'_, AppState>, id: String) -> Result<bool, IpcError> {
    queue_remove_impl(&state, &id).await
}

/// # Errors
/// `IpcError` (`STORE`) khi ghi lỗi.
pub async fn queue_reorder_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    ids: &[String],
) -> Result<(), IpcError> {
    QueueRepo::new(&ctx.db).reorder(ids).await?;
    Ok(())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn queue_reorder(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), IpcError> {
    queue_reorder_impl(&state, &ids).await
}

/// # Errors
/// `IpcError` (`STORE`) khi xoá lỗi.
pub async fn queue_clear_finished_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
) -> Result<u64, IpcError> {
    Ok(QueueRepo::new(&ctx.db).clear_finished().await?)
}

#[tauri::command]
pub async fn queue_clear_finished(state: tauri::State<'_, AppState>) -> Result<u64, IpcError> {
    queue_clear_finished_impl(&state).await
}

/// Bật bộ chạy hàng chờ. Trả `false` khi ĐÃ đang chạy (gọi lại không tạo vòng thứ hai).
pub fn queue_start_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>) -> bool {
    // `compare_exchange` thay vì `load` rồi `store`: hai lệnh IPC gọi sát nhau không thể cùng thắng.
    if ctx
        .queue_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }
    let token = CancellationToken::new();
    *ctx.queue_cancel.lock().expect("queue_cancel mutex") = Some(token.clone());

    // Bộ chạy sống LÂU HƠN lệnh IPC này nên không mượn được `&Ctx` — truyền bản sao 'static của
    // đúng những thứ nó cần (`Arc`/`Db` đều rẻ để clone).
    spawn_runner(
        RunnerCtx {
            engine: Arc::clone(&ctx.engine),
            db: ctx.db.clone(),
            runs: Arc::clone(&ctx.runs),
            running: Arc::clone(&ctx.queue_running),
            cancel: Arc::clone(&ctx.queue_cancel),
            settings: Arc::clone(&ctx.settings),
        },
        token,
    );
    true
}

/// Dừng hàng chờ. Workflow ĐANG chạy vẫn chạy nốt (huỷ giữa chừng là việc của `run_cancel`);
/// bộ chạy chỉ không lấy mục kế tiếp. Trả `false` khi vốn không chạy.
pub fn queue_stop_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>) -> bool {
    let token = ctx.queue_cancel.lock().expect("queue_cancel mutex").take();
    match token {
        Some(t) => {
            t.cancel();
            true
        }
        None => false,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "chu ky `#[tauri::command]` bat buoc nhan State theo gia tri - bien gioi IPC"
)]
#[tauri::command]
pub fn queue_start(state: tauri::State<'_, AppState>) -> bool {
    queue_start_impl(&state)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "chu ky `#[tauri::command]` bat buoc nhan State theo gia tri - bien gioi IPC"
)]
#[tauri::command]
pub fn queue_stop(state: tauri::State<'_, AppState>) -> bool {
    queue_stop_impl(&state)
}

/// Những gì bộ chạy nền cần — bản sao 'static, không mượn `Ctx`.
struct RunnerCtx<D: BrowserDriver + 'static> {
    engine: Arc<lvc_engine::Engine<D>>,
    db: lvc_store::Db,
    runs: Arc<dashmap::DashMap<String, CancellationToken>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    cancel: Arc<std::sync::Mutex<Option<CancellationToken>>>,
    /// Đọc LẠI trước mỗi workflow — người dùng đổi "Khi một workflow lỗi"/"Nghỉ giữa hai
    /// workflow" giữa lúc hàng chờ đang chạy thì mục kế tiếp áp dụng ngay, không phải tắt bật lại.
    settings: Arc<tokio::sync::RwLock<lvc_core::AppSettings>>,
}

/// Vòng chạy tuần tự: lấy mục `waiting` đầu tiên → chạy → ghi kết quả → nghỉ → lặp.
/// Dừng khi hết việc hoặc `token` bị huỷ.
fn spawn_runner<D: BrowserDriver + 'static>(rc: RunnerCtx<D>, token: CancellationToken) {
    tokio::spawn(async move {
        let repo = QueueRepo::new(&rc.db);
        loop {
            if token.is_cancelled() {
                break;
            }
            let next = match repo.next_waiting().await {
                Ok(Some(row)) => row,
                Ok(None) => break, // Hết việc.
                Err(e) => {
                    warn!(error = %e, "hang cho: khong doc duoc muc ke tiep - dung bo chay");
                    break;
                }
            };

            let graph: Graph = match serde_json::from_str(&next.graph) {
                Ok(g) => g,
                Err(e) => {
                    // Đồ thị hỏng (dù đã kiểm lúc thêm) -> bỏ qua, chạy tiếp mục sau.
                    if let Err(e2) = repo
                        .finish(&next.id, QueueStatus::Skipped, &format!("do thi hong: {e}"))
                        .await
                    {
                        warn!(error = %e2, "hang cho: khong ghi duoc trang thai skipped");
                    }
                    continue;
                }
            };

            let handle = match rc
                .engine
                .run_graph(&next.workflow_id, &graph, noop_hooks_factory())
                .await
            {
                Ok(h) => h,
                Err(e) => {
                    if let Err(e2) = repo
                        .finish(&next.id, QueueStatus::Failed, &e.to_string())
                        .await
                    {
                        warn!(error = %e2, "hang cho: khong ghi duoc trang thai failed");
                    }
                    continue;
                }
            };

            if let Err(e) = repo.mark_running(&next.id, &handle.run_id).await {
                warn!(error = %e, "hang cho: khong ghi duoc trang thai running");
            }
            rc.runs.insert(handle.run_id.clone(), handle.token.clone());
            info!(queue_id = %next.id, run_id = %handle.run_id, "hang cho: bat dau mot workflow");

            let outcome = handle.join.await;
            rc.runs.remove(&handle.run_id);

            // `join` chỉ cho biết TASK có panic không — kết quả từng node đã được engine ghi vào
            // bảng `runs`/`tasks` và phát qua sự kiện; ở đây chỉ phân biệt "chạy trót lọt" với
            // "task chết bất thường".
            let (status, err) = match outcome {
                Ok(()) => (QueueStatus::Done, String::new()),
                Err(e) => (
                    QueueStatus::Failed,
                    format!("lan chay ket thuc bat thuong: {e}"),
                ),
            };
            if let Err(e) = repo.finish(&next.id, status, &err).await {
                warn!(error = %e, "hang cho: khong ghi duoc ket qua");
            }

            // Đọc cài đặt SAU khi workflow xong (không phải lúc khởi động bộ chạy) để người dùng
            // đổi cài đặt giữa lúc chạy vẫn có tác dụng cho mục kế tiếp.
            let (on_error, rest_secs) = {
                let s = rc.settings.read().await;
                (s.on_error, u64::from(s.rest_between))
            };

            // "Khi một workflow lỗi": `Stop` = dừng cả hàng chờ, các mục sau vẫn `waiting` để
            // người dùng xem lại rồi bật tiếp.
            if status == QueueStatus::Failed && on_error == OnError::Stop {
                info!(queue_id = %next.id, "hang cho: mot workflow loi, dung ca hang cho theo cai dat");
                break;
            }

            // Nghỉ giữa hai workflow — giảm rủi ro bị gắn cờ khi chạy liên tục (bản bàn giao, mục
            // "Nghỉ giữa hai workflow"). Huỷ trong lúc nghỉ thì thoát ngay.
            if rest_secs > 0 {
                tokio::select! {
                    () = token.cancelled() => break,
                    () = tokio::time::sleep(std::time::Duration::from_secs(rest_secs)) => {}
                }
            }
        }

        rc.running.store(false, Ordering::SeqCst);
        *rc.cancel.lock().expect("queue_cancel mutex") = None;
        info!("hang cho: bo chay dung");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::ctx_with;
    use lvc_core::graph::{Edge, Node, NodeData};

    /// Đồ thị hợp lệ tối thiểu: MỘT node `prompt` (không cổng vào bắt buộc, không cần tài khoản,
    /// không gọi mạng). Thêm node `save` vào đây sẽ KHÔNG hợp lệ — `save` bắt buộc có đầu vào.
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

    #[tokio::test]
    async fn them_roi_liet_ke_theo_thu_tu() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;

        let a = queue_add_impl(&ctx, "wf1", "Alpha", &simple_graph())
            .await
            .unwrap();
        let b = queue_add_impl(&ctx, "wf2", "Beta", &simple_graph())
            .await
            .unwrap();

        let state = queue_list_impl(&ctx).await.unwrap();
        assert_eq!(state.items.len(), 2);
        assert_eq!(state.items[0].id, a.id);
        assert_eq!(state.items[1].id, b.id);
        assert_eq!(state.items[0].status, "waiting");
        assert!(!state.running, "chua bam chay thi `running` phai la false");
    }

    #[tokio::test]
    async fn them_do_thi_hong_bao_graph_invalid_khong_vao_hang_cho() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let bad = Graph {
            version: 2,
            nodes: vec![],
            // Canh tro toi node khong ton tai.
            edges: vec![Edge {
                id: "e".to_string(),
                from: "khong-co".to_string(),
                from_port: "out".to_string(),
                to: "cung-khong-co".to_string(),
                to_port: "in".to_string(),
            }],
        };

        let err = queue_add_impl(&ctx, "wf", "Hỏng", &bad).await.unwrap_err();
        assert_eq!(err.code, "GRAPH_INVALID");
        assert!(
            queue_list_impl(&ctx).await.unwrap().items.is_empty(),
            "do thi hong KHONG duoc vao hang cho"
        );
    }

    #[tokio::test]
    async fn sap_lai_thu_tu_va_xoa() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        let a = queue_add_impl(&ctx, "w", "A", &simple_graph())
            .await
            .unwrap();
        let b = queue_add_impl(&ctx, "w", "B", &simple_graph())
            .await
            .unwrap();
        let c = queue_add_impl(&ctx, "w", "C", &simple_graph())
            .await
            .unwrap();

        queue_reorder_impl(&ctx, &[c.id.clone(), a.id.clone(), b.id.clone()])
            .await
            .unwrap();
        let names: Vec<String> = queue_list_impl(&ctx)
            .await
            .unwrap()
            .items
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(names, ["C", "A", "B"]);

        assert!(queue_remove_impl(&ctx, &a.id).await.unwrap());
        assert!(!queue_remove_impl(&ctx, &a.id).await.unwrap());
        assert_eq!(queue_list_impl(&ctx).await.unwrap().items.len(), 2);
    }

    #[tokio::test]
    async fn stop_khi_chua_chay_tra_false() {
        let (_dir, ctx, _d) = ctx_with(vec![], &[]).await;
        assert!(!queue_stop_impl(&ctx), "chua chay thi khong co gi de dung");
    }
}
