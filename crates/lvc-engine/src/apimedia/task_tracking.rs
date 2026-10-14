//! Tạo dòng `TaskRow` + báo `tasks_ready` SỚM cho các host API chính thức (Veo3/Google/Seedance) —
//! dùng chung cho `veo3.rs`/`apimedia/google.rs`/`apimedia/seedance.rs`, mirror 2 giai đoạn của
//! `Engine::run_video_jobs` (Dola, `dag.rs`): tạo HẾT dòng `TaskRow` (nhanh, không I/O mạng) → báo
//! `tasks_ready` MỘT LẦN cho cả batch → rồi mới làm việc thật (chậm, mạng).
//!
//! `Engine::run_script` (P3c, "danh sách kịch bản") chờ `tasks_ready` để ánh xạ `scene_id ->
//! task_id` — KHÔNG có bước này thì mọi cảnh chạy qua Veo3/Google/Seedance sẽ treo hết
//! `SAFETY_TIMEOUT` (10s) rồi báo `RUN_FAILED` dù video có thể vẫn tạo thành công (phát hiện khi
//! hoàn thiện "kịch bản affiliate đa-engine", 2026-10-11).
//!
//! Bảng `tasks` vốn thiết kế CHO DOLA (cột `ratio`/`duration`/`model`/`mode`/`logo` dùng enum của
//! Dola) — [`placeholder_scene`] chỉ điền giá trị HỢP LỆ VỀ KIỂU để `TaskRepo::create` chấp nhận,
//! KHÔNG phản ánh tham số thật đã gửi cho Veo3/Google/Seedance (tham số thật nằm trong
//! `VideoJob` riêng của từng provider, đã gửi đi trước khi dòng này được đọc lại). UI "Nhật ký"
//! hiển thị các cột đó cho task Veo3/Google/Seedance là KHÔNG đáng tin — chỉ `status`/`local_path`/
//! `error` là thật.

use std::path::Path;

use lvc_core::{LogoMode, Mode, Model, Ratio, Scene, TaskStatus};
use lvc_store::{BatchRef, StoreError, TaskRepo};
use tokio::sync::watch;

use crate::dag::TaskRef;
use crate::generate::now_unix;

fn placeholder_scene(prompt: &str, account: &str) -> Scene {
    Scene {
        prompt: prompt.to_string(),
        ratio: Ratio::R9x16,
        duration: 10,
        model: Model::Seedance20,
        mode: Mode::Fast,
        logo: LogoMode::Off,
        reference_images: Vec::new(),
        account: account.to_string(),
    }
}

/// Tạo MỘT dòng `TaskRow` "chờ" (`queued`) cho một job, gán `run_id`/`node_id`. Gọi
/// [`announce_batch`] SAU KHI tạo xong HẾT các dòng của node (không gọi ở đây — xem doc module).
///
/// # Errors
/// `StoreError` khi ghi/đọc lại thất bại.
pub(crate) async fn create_pending_task(
    tasks: &TaskRepo,
    run_id: &str,
    node_id: &str,
    scene_index: u32,
    prompt: &str,
    account: &str,
) -> Result<String, StoreError> {
    let task_id = format!("task_{}", uuid::Uuid::new_v4());
    let scene = placeholder_scene(prompt, account);
    let batch = BatchRef {
        batch_id: run_id.to_string(),
        batch_name: node_id.to_string(),
        scene_index: i64::from(scene_index),
    };
    tasks.create(&task_id, &scene, Some(&batch)).await?;
    tasks.set_run(&task_id, run_id, node_id).await?;
    Ok(task_id)
}

/// Báo MỘT LẦN cho cả batch (xem doc module — KHÔNG gọi nhiều lần cho cùng 1 node, dễ làm
/// `scripts::run_script` đọc trúng lúc dữ liệu chưa đủ do `watch` dồn các lần ghi liên tiếp).
pub(crate) fn announce_batch(
    tx: &watch::Sender<Vec<TaskRef>>,
    node_id: &str,
    refs: &[(u32, String)],
) {
    tx.send_modify(|v| {
        v.extend(refs.iter().map(|(scene_index, task_id)| TaskRef {
            node_id: node_id.to_string(),
            scene_index: *scene_index,
            task_id: task_id.clone(),
        }));
    });
}

/// Ghi kết quả THÀNH CÔNG vào dòng đã tạo bởi [`create_pending_task`]. Nuốt lỗi ghi DB (chỉ cảnh
/// báo) — giống `Engine::mark_row_failed`, báo cáo chính (`VideoJobsReport`) không phụ thuộc việc
/// ghi dòng tiến trình có thành công hay không.
pub(crate) async fn finish_task_done(tasks: &TaskRepo, task_id: &str, local_path: &Path) {
    let _ = tasks.set_result(task_id, "", local_path).await;
    let _ = tasks
        .set_status(task_id, TaskStatus::Completed, None, None)
        .await;
    let _ = tasks.set_finished(task_id, now_unix()).await;
}

/// Ghi kết quả THẤT BẠI vào dòng đã tạo bởi [`create_pending_task`]. `reason` KHÔNG được chứa bí
/// mật (cookie/API key/prompt đầy đủ) — gọi nơi chỉ truyền thông điệp lỗi đã có sẵn trong
/// `VideoJobsReport.failed`.
pub(crate) async fn finish_task_failed(tasks: &TaskRepo, task_id: &str, reason: &str) {
    let _ = tasks
        .set_status(task_id, TaskStatus::Failed, None, Some(reason))
        .await;
    let _ = tasks.set_finished(task_id, now_unix()).await;
}
