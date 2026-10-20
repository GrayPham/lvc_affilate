//! Luồng sinh video (`chat.stream` → đợi video xuất hiện trong lịch sử → tải xuống) — port TỪNG
//! PHẦN từ `client.py` (tham khảo, CHỈ ĐỌC): bỏ `connect`/`device_id`-file/upload ảnh tham chiếu
//! (CHƯA có trong phạm vi v1 node `museai_video`, chỉ prompt văn bản — xem `node-museai-video/
//! CLAUDE.md`). Nhận [`HatchConnection`] ĐÃ kết nối sẵn (bắt tay Noise + có token bootstrap từ
//! bước trước, xem `CLAUDE.md` "Cơ chế tạo video") — module này không tự đăng nhập/bootstrap gì.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::attachments::build_items;
use super::errors::MuseError;
use super::filesystem::MuseFilesystem;
use super::media::{
    ImageRef, VideoRef, extract_image_refs, extract_session_ids, extract_video_refs,
};
use super::transport::HatchConnection;

type Result<T> = std::result::Result<T, MuseError>;

/// Khớp `CHAT_CAPABILITIES` (`routes.py`) — gửi kèm mỗi lần `chat.stream`.
const CHAT_CAPABILITIES: &[&str] = &[
    "chat_cancel",
    "delta_stream",
    "custom_reactions",
    "custom_reactions_facebook_thumbs_up_v1",
];

#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub session_id: Option<String>,
    pub videos: Vec<VideoRef>,
    pub downloaded: Vec<std::path::PathBuf>,
    pub download_errors: Vec<String>,
}

/// `history(limit=80)` trả JSON tuỳ hình dạng server — chỉ cần nguyên văn để [`extract_video_refs`]
/// duyệt, không parse chặt kiểu.
async fn history(
    conn: &HatchConnection,
    session_id: Option<&str>,
    limit: u32,
    timeout: Duration,
) -> Result<Value> {
    let mut params = serde_json::Map::new();
    params.insert("limit".to_string(), json!(limit));
    params.insert("transcript_mode".to_string(), json!("messages"));
    if let Some(id) = session_id {
        params.insert("session_id".to_string(), json!(id));
    }
    let response = conn
        .request(
            "GET",
            "/chat/history",
            Some(&Value::Object(params)),
            timeout,
        )
        .await?
        .raise_for_status()?;
    response
        .json::<Value>()
        .map_err(|e| MuseError::Protocol(format!("JSON /chat/history loi: {e}")))?
        .ok_or_else(|| MuseError::Protocol("/chat/history tra ve body rong".into()))
}

async fn sessions_list(conn: &HatchConnection, timeout: Duration) -> Result<Value> {
    let response = conn
        .request("GET", "/api/session/list", None, timeout)
        .await?
        .raise_for_status()?;
    response
        .json::<Value>()
        .map_err(|e| MuseError::Protocol(format!("JSON /api/session/list loi: {e}")))?
        .ok_or_else(|| MuseError::Protocol("/api/session/list tra ve body rong".into()))
}

/// Gửi prompt (kèm ảnh tham chiếu nếu có) qua `/chat/stream`, thu mọi event trong `stream_timeout`
/// (hết hạn KHÔNG phải lỗi — endpoint xác nhận, có thể đóng trễ hơn tiến trình sinh thật, xem doc
/// [`HatchConnection::subscribe_collect`]).
///
/// Ảnh đi THẲNG trong `items` dưới dạng base64 — muse.ai không có endpoint tải ảnh riêng (xem
/// `protocol::attachments`).
async fn chat_stream(
    conn: &HatchConnection,
    device_id: &str,
    prompt: &str,
    images: &[PathBuf],
    session_id: Option<&str>,
    stream_timeout: Duration,
) -> Result<Vec<Value>> {
    let mut payload = serde_json::Map::new();
    payload.insert("items".to_string(), build_items(images, prompt)?);
    payload.insert("node_id".to_string(), json!(device_id));
    payload.insert("capabilities".to_string(), json!(CHAT_CAPABILITIES));
    if let Some(id) = session_id {
        payload.insert("session_id".to_string(), json!(id));
    }
    conn.subscribe_collect(
        "POST",
        "/chat/stream",
        Some(&Value::Object(payload)),
        stream_timeout,
    )
    .await
}

async fn resolve_session_id(
    conn: &HatchConnection,
    explicit: Option<&str>,
    events: &[Value],
    sessions_before: &HashSet<String>,
    timeout: Duration,
) -> Option<String> {
    if let Some(id) = explicit {
        return Some(id.to_string());
    }
    let from_events: Vec<String> = events.iter().flat_map(extract_session_ids).collect();
    if let Some(last) = from_events.last() {
        return Some(last.clone());
    }
    let Ok(after) = sessions_list(conn, timeout).await else {
        return None;
    };
    let current = extract_session_ids(&after);
    if let Some(new_id) = current.iter().find(|id| !sessions_before.contains(*id)) {
        return Some(new_id.clone());
    }
    current.into_iter().next()
}

/// Lỗi hết hạn chờ video — tách riêng KHỎI [`MuseError`] vì đây là kết quả "chưa xong" bình
/// thường của việc chờ (caller có thể muốn thử lại/poll tiếp), không phải lỗi giao thức.
#[derive(Debug, thiserror::Error)]
pub enum WaitForVideosError {
    #[error(transparent)]
    Muse(#[from] MuseError),
    #[error("khong co video Muse moi nao trong {0:?}")]
    Timeout(Duration),
}

/// Tham số cho [`wait_for_videos`] — gom lại cho gọn chữ ký hàm (clippy `too_many_arguments`).
struct WaitForVideosParams<'a> {
    session_id: Option<&'a str>,
    timeout: Duration,
    poll_interval: Duration,
    settle: Duration,
    min_videos: usize,
    request_timeout: Duration,
}

/// Poll `/chat/history` tới khi thấy ÍT NHẤT `min_videos` video MỚI (không có trong `baseline`)
/// và kết quả ổn định (`settle_seconds` không đổi thêm) — khớp `wait_for_videos` bên Python.
async fn wait_for_videos(
    conn: &HatchConnection,
    baseline: &HashSet<String>,
    p: &WaitForVideosParams<'_>,
) -> std::result::Result<Vec<VideoRef>, WaitForVideosError> {
    let deadline = tokio::time::Instant::now() + p.timeout;
    let mut first_found_at: Option<tokio::time::Instant> = None;
    let mut latest: Vec<VideoRef> = Vec::new();

    while tokio::time::Instant::now() < deadline {
        let latest_history = history(conn, p.session_id, 80, p.request_timeout).await?;
        let refs: Vec<VideoRef> = extract_video_refs(&latest_history)
            .into_iter()
            .filter(|r| !baseline.contains(&r.identity()))
            .collect();
        // De-dup theo identity (giu lan gap cuoi, khop `{ref.identity: ref for ref in refs}`).
        let mut unique: Vec<VideoRef> = Vec::new();
        for r in refs {
            if let Some(pos) = unique.iter().position(|u| u.identity() == r.identity()) {
                unique[pos] = r;
            } else {
                unique.push(r);
            }
        }
        latest = unique;

        if latest.len() >= p.min_videos {
            let now = tokio::time::Instant::now();
            let first = *first_found_at.get_or_insert(now);
            if now.duration_since(first) >= p.settle {
                return Ok(latest);
            }
        } else {
            first_found_at = None;
        }
        tokio::time::sleep(p.poll_interval).await;
    }

    if !latest.is_empty() {
        return Ok(latest);
    }
    Err(WaitForVideosError::Timeout(p.timeout))
}

fn filename_for(r: &VideoRef, index: usize) -> String {
    let candidate = r.path.clone().or_else(|| r.url.clone());
    if let Some(candidate) = candidate {
        let name = candidate.rsplit('/').next().unwrap_or(&candidate);
        let name = name.split('?').next().unwrap_or(name);
        if !name.is_empty() && name.contains('.') {
            return name.to_string();
        }
    }
    format!("muse_video_{}.mp4", index + 1)
}

/// Thử lần lượt các nguồn tải (khớp thứ tự `download_video` bên Python) — BỎ nhánh "URL công khai
/// tải thẳng" (cần HTTP client riêng ngoài `HatchConnection`, trường hợp hiếm theo ghi chú gốc —
/// `/idea-cards/media/` mới là route chính cho media Muse) và nhánh `sandbox://` qua `fs.raw`
/// (route `fs.raw` CHƯA có bằng chứng hoạt động với `sandbox://` trực tiếp — ghi chú gốc coi đây
/// là fallback cuối, path thường cũng đi qua `idea_media`/`media_handle` trước).
async fn download_video(
    fs: &MuseFilesystem,
    r: &VideoRef,
    destination: &std::path::Path,
    timeout: Duration,
) -> std::result::Result<(), String> {
    let mut attempts = Vec::new();

    if let Some(url) = &r.url {
        if url.contains("/idea-cards/media/") {
            match fs.idea_media_from_url(url, timeout).await {
                Ok(data) => return write_bytes(destination, &data),
                Err(e) => attempts.push(format!("idea-media tu URL: {e}")),
            }
        }
    }
    if let Some(handle) = &r.media_handle {
        match fs.idea_media(handle, timeout).await {
            Ok(data) => return write_bytes(destination, &data),
            Err(e) => attempts.push(format!("media_handle: {e}")),
        }
    }
    if let Some(path) = &r.path {
        match fs.download(path, destination, timeout).await {
            Ok(()) => return Ok(()),
            Err(e) => attempts.push(format!("path fs.raw: {e}")),
        }
    }

    Err(if attempts.is_empty() {
        "khong co nguon tai video nao kha dung".to_string()
    } else {
        attempts.join("; ")
    })
}

fn write_bytes(destination: &std::path::Path, data: &[u8]) -> std::result::Result<(), String> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(destination, data).map_err(|e| e.to_string())
}

/// Tham số cho [`generate_video`] — gom lại cho gọn chữ ký hàm (nhiều tham số tuỳ chọn).
pub struct GenerateVideoParams<'a> {
    pub prompt: &'a str,
    pub device_id: &'a str,
    pub output_dir: &'a std::path::Path,
    pub session_id: Option<&'a str>,
    pub timeout: Duration,
    pub min_videos: usize,
    /// Tỉ lệ khung hình MONG MUỐN (vd `"9:16"`) — CHỈ là chỉ thị chèn vào prompt (xem
    /// [`build_video_prompt`]), muse.ai không có tham số API thật cho việc này, KHÔNG đảm bảo
    /// AI tuân theo (xem `VideoJob::aspect`, `node-museai-video/CLAUDE.md`).
    pub aspect: Option<&'a str>,
    /// Thời lượng MONG MUỐN (giây) — cùng cảnh báo như `aspect`.
    pub duration_secs: Option<u32>,
    /// Ảnh tham chiếu (nhân vật/sản phẩm) gửi kèm prompt. Rỗng = không có.
    ///
    /// CẢNH BÁO giống `aspect`/`duration_secs`: gửi được ảnh lên KHÔNG đảm bảo muse.ai giữ đúng
    /// nhân vật — nó là ngữ cảnh cho mô hình, không phải tham số ràng buộc.
    pub images: &'a [PathBuf],
}

/// Chèn chỉ thị tỉ lệ/thời lượng (nếu có) vào CUỐI prompt người dùng — CÙNG kỹ thuật với
/// [`build_image_prompt`] (prompt là cách DUY NHẤT tác động muse.ai, không có tham số API thật,
/// xem doc [`GenerateVideoParams`]). **2026-10-13 — PORT đúng câu đã đo THẬT 10/10 lần** từ
/// `E:\ProjectAILVC\Lvc_veo3_automation\src\core\museai\prompt_builder.py` (tham khảo, CHỈ ĐỌC):
/// câu tiếng ANH dạng `"Output video: {N} seconds, {tỉ lệ}."` nối vào CUỐI — bản đầu của hàm này tự
/// bịa câu tiếng Việt khác hẳn (chèn cuối kiểu "ghi chú phụ" rồi đổi sang chèn đầu), cả hai đều
/// CHƯA được đo thật, nên bỏ, dùng NGUYÊN câu đã có bằng chứng thật. Phát hiện quan trọng từ tool
/// tham khảo (tự ghi lại qua đo đạc, không phải suy đoán): muse.ai KHÔNG tuân thủ tuyệt đối
/// (10/10: xin 5s vẫn ra 10s) — nhưng khi xin 15-30s, muse.ai TỰ ĐỘNG dựng nhiều clip ~10s ghép lại
/// (xin 30s → 4 clip 10s ghép thành bản 30s, ~5 phút) — không cần ta tự ffmpeg concat. Rỗng cả hai
/// → trả nguyên văn prompt.
fn build_video_prompt(
    user_prompt: &str,
    aspect: Option<&str>,
    duration_secs: Option<u32>,
) -> String {
    let mut fmt = Vec::new();
    if let Some(d) = duration_secs.filter(|d| *d > 0) {
        fmt.push(format!("{d} seconds"));
    }
    let aspect_phrase = aspect.map(str::trim).and_then(|a| match a {
        "16:9" => Some("horizontal 16:9 landscape"),
        "9:16" => Some("vertical 9:16 portrait"),
        "1:1" => Some("square 1:1"),
        _ => None,
    });
    if let Some(phrase) = aspect_phrase {
        fmt.push(phrase.to_string());
    }
    if fmt.is_empty() {
        return user_prompt.to_string();
    }
    format!("{user_prompt} Output video: {}.", fmt.join(", "))
}

/// Luồng đầy đủ: gửi prompt → chờ video mới xuất hiện trong lịch sử → tải xuống `output_dir`.
///
/// # Errors
/// [`MuseError`] khi RPC lỗi; lỗi riêng khi hết hạn chờ video (bọc trong `GenerationResult` qua
/// `download_errors` cho lỗi tải — lỗi KHÔNG làm hỏng cả kết quả nếu MỘT SỐ video đã tải được).
pub async fn generate_video(
    conn: &HatchConnection,
    params: &GenerateVideoParams<'_>,
) -> std::result::Result<GenerationResult, WaitForVideosError> {
    let request_timeout = Duration::from_secs(60);
    let stream_timeout = Duration::from_secs(30);
    let poll_interval = Duration::from_secs(3);
    let settle = Duration::from_secs(5);

    let sessions_before: HashSet<String> = if params.session_id.is_none() {
        sessions_list(conn, request_timeout)
            .await
            .map(|v| extract_session_ids(&v).into_iter().collect())
            .unwrap_or_default()
    } else {
        HashSet::new()
    };

    let baseline_history = history(conn, params.session_id, 80, request_timeout).await?;
    let baseline: HashSet<String> = extract_video_refs(&baseline_history)
        .iter()
        .map(VideoRef::identity)
        .collect();

    let prompt = build_video_prompt(params.prompt, params.aspect, params.duration_secs);
    let events = chat_stream(
        conn,
        params.device_id,
        &prompt,
        params.images,
        params.session_id,
        stream_timeout,
    )
    .await?;
    let resolved_session = resolve_session_id(
        conn,
        params.session_id,
        &events,
        &sessions_before,
        request_timeout,
    )
    .await;

    let videos = wait_for_videos(
        conn,
        &baseline,
        &WaitForVideosParams {
            session_id: resolved_session.as_deref(),
            timeout: params.timeout,
            poll_interval,
            settle,
            min_videos: params.min_videos,
            request_timeout,
        },
    )
    .await?;

    let fs = MuseFilesystem::new(conn.clone());
    let mut downloaded = Vec::new();
    let mut download_errors = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();
    for (index, r) in videos.iter().enumerate() {
        let mut filename = filename_for(r, index);
        let mut suffix = 1u32;
        while used_names.contains(&filename) || params.output_dir.join(&filename).exists() {
            let stem = std::path::Path::new(&filename_for(r, index))
                .file_stem()
                .map_or_else(
                    || format!("muse_video_{}", index + 1),
                    |s| s.to_string_lossy().into_owned(),
                );
            filename = format!("{stem}_{suffix}.mp4");
            suffix += 1;
        }
        used_names.insert(filename.clone());
        let destination = params.output_dir.join(&filename);
        match download_video(&fs, r, &destination, request_timeout).await {
            Ok(()) => downloaded.push(destination),
            Err(e) => download_errors.push(format!("video[{}] {}: {e}", index + 1, r.identity())),
        }
    }

    Ok(GenerationResult {
        session_id: resolved_session,
        videos,
        downloaded,
        download_errors,
    })
}

/// Kết quả [`generate_image`] — khớp khuôn [`GenerationResult`], riêng cho ảnh.
#[derive(Debug, Clone)]
pub struct ImageGenerationResult {
    pub session_id: Option<String>,
    pub images: Vec<ImageRef>,
    pub downloaded: Vec<std::path::PathBuf>,
    pub download_errors: Vec<String>,
}

/// Như [`WaitForVideosError`] nhưng cho ảnh.
#[derive(Debug, thiserror::Error)]
pub enum WaitForImagesError {
    #[error(transparent)]
    Muse(#[from] MuseError),
    #[error("khong co anh Muse moi nao trong {0:?}")]
    Timeout(Duration),
}

struct WaitForImagesParams<'a> {
    session_id: Option<&'a str>,
    timeout: Duration,
    poll_interval: Duration,
    settle: Duration,
    min_images: usize,
    request_timeout: Duration,
}

/// Như [`wait_for_videos`] nhưng cho ảnh — ĐÃ XÁC NHẬN qua trang thật 2026-10-12: muse.ai sinh
/// ảnh tĩnh (`kind:"image"`) khi prompt báo hiệu muốn ảnh thay vì video (vd "a single still
/// photo... no motion, no video"), cùng cơ chế poll `/chat/history` như video (xem CLAUDE.md
/// "Cơ chế tạo video/ảnh").
async fn wait_for_images(
    conn: &HatchConnection,
    baseline: &HashSet<String>,
    p: &WaitForImagesParams<'_>,
) -> std::result::Result<Vec<ImageRef>, WaitForImagesError> {
    let deadline = tokio::time::Instant::now() + p.timeout;
    let mut first_found_at: Option<tokio::time::Instant> = None;
    let mut latest: Vec<ImageRef> = Vec::new();

    while tokio::time::Instant::now() < deadline {
        let latest_history = history(conn, p.session_id, 80, p.request_timeout).await?;
        let refs: Vec<ImageRef> = extract_image_refs(&latest_history)
            .into_iter()
            .filter(|r| !baseline.contains(&r.identity()))
            .collect();
        let mut unique: Vec<ImageRef> = Vec::new();
        for r in refs {
            if let Some(pos) = unique.iter().position(|u| u.identity() == r.identity()) {
                unique[pos] = r;
            } else {
                unique.push(r);
            }
        }
        latest = unique;

        if latest.len() >= p.min_images {
            let now = tokio::time::Instant::now();
            let first = *first_found_at.get_or_insert(now);
            if now.duration_since(first) >= p.settle {
                return Ok(latest);
            }
        } else {
            first_found_at = None;
        }
        tokio::time::sleep(p.poll_interval).await;
    }

    if !latest.is_empty() {
        return Ok(latest);
    }
    Err(WaitForImagesError::Timeout(p.timeout))
}

fn filename_for_image(r: &ImageRef, index: usize) -> String {
    let candidate = r.path.clone().or_else(|| r.url.clone());
    if let Some(candidate) = candidate {
        let name = candidate.rsplit('/').next().unwrap_or(&candidate);
        let name = name.split('?').next().unwrap_or(name);
        if !name.is_empty() && name.contains('.') {
            return name.to_string();
        }
    }
    // .webp la dinh dang thuc te quan sat duoc (xem test co hinh dang phan hoi that trong media.rs)
    // — khong doan .png/.jpg vi chua xac nhan muse.ai tra ve dinh dang nao khac.
    format!("muse_image_{}.webp", index + 1)
}

/// Như [`download_video`] nhưng cho ảnh.
async fn download_image(
    fs: &MuseFilesystem,
    r: &ImageRef,
    destination: &std::path::Path,
    timeout: Duration,
) -> std::result::Result<(), String> {
    let mut attempts = Vec::new();

    if let Some(url) = &r.url {
        if url.contains("/idea-cards/media/") {
            match fs.idea_media_from_url(url, timeout).await {
                Ok(data) => return write_bytes(destination, &data),
                Err(e) => attempts.push(format!("idea-media tu URL: {e}")),
            }
        }
    }
    if let Some(handle) = &r.media_handle {
        match fs.idea_media(handle, timeout).await {
            Ok(data) => return write_bytes(destination, &data),
            Err(e) => attempts.push(format!("media_handle: {e}")),
        }
    }
    if let Some(path) = &r.path {
        match fs.download(path, destination, timeout).await {
            Ok(()) => return Ok(()),
            Err(e) => attempts.push(format!("path fs.raw: {e}")),
        }
    }

    Err(if attempts.is_empty() {
        "khong co nguon tai anh nao kha dung".to_string()
    } else {
        attempts.join("; ")
    })
}

/// Tham số cho [`generate_image`] — khớp khuôn [`GenerateVideoParams`].
pub struct GenerateImageParams<'a> {
    pub prompt: &'a str,
    pub device_id: &'a str,
    pub output_dir: &'a std::path::Path,
    pub session_id: Option<&'a str>,
    pub timeout: Duration,
    pub min_images: usize,
    /// Ảnh tham chiếu đính kèm prompt (base64 trong `items`) — đối xứng [`GenerateVideoParams`].
    pub images: &'a [std::path::PathBuf],
}

/// `kind:"image"` hay `"video"` do CHÍNH muse.ai tự quyết theo Ý PROMPT (đã xác nhận thật
/// 2026-10-12) — mặc định thiên về video (agent tự mô tả bản thân là "tạo video" khi hỏi trống
/// không). Gắn thêm hướng dẫn rõ ràng CUỐI prompt (nguyên văn câu đã xác nhận THÀNH CÔNG trong lần
/// test thật — "a single still photo... no motion, no video, just one static image") để đảm bảo
/// ra ảnh tĩnh thay vì video, bất kể prompt gốc của người dùng viết ngôn ngữ/văn phong nào.
fn build_image_prompt(user_prompt: &str) -> String {
    format!(
        "{user_prompt}. (a single still photo of a real person, candid lifestyle style, \
         no motion, no video, just one static image)"
    )
}

/// Luồng đầy đủ sinh ẢNH TĨNH (không phải video) — tự gắn hướng dẫn rõ ràng vào prompt (xem
/// [`build_image_prompt`]) → chờ ảnh mới xuất hiện trong lịch sử → tải xuống `output_dir`.
///
/// # Errors
/// [`MuseError`] khi RPC lỗi; hết hạn chờ ảnh trả lỗi riêng (ảnh tải được một phần vẫn nằm trong
/// `ImageGenerationResult.download_errors`, không làm hỏng cả kết quả).
pub async fn generate_image(
    conn: &HatchConnection,
    params: &GenerateImageParams<'_>,
) -> std::result::Result<ImageGenerationResult, WaitForImagesError> {
    let request_timeout = Duration::from_secs(60);
    let stream_timeout = Duration::from_secs(30);
    let poll_interval = Duration::from_secs(3);
    let settle = Duration::from_secs(5);
    let prompt = build_image_prompt(params.prompt);

    let sessions_before: HashSet<String> = if params.session_id.is_none() {
        sessions_list(conn, request_timeout)
            .await
            .map(|v| extract_session_ids(&v).into_iter().collect())
            .unwrap_or_default()
    } else {
        HashSet::new()
    };

    let baseline_history = history(conn, params.session_id, 80, request_timeout).await?;
    let baseline: HashSet<String> = extract_image_refs(&baseline_history)
        .iter()
        .map(ImageRef::identity)
        .collect();

    let events = chat_stream(
        conn,
        params.device_id,
        &prompt,
        // Ảnh tham chiếu để KHOÁ nhân vật (pipeline ảnh-trước) — đối xứng `generate_video`.
        params.images,
        params.session_id,
        stream_timeout,
    )
    .await?;
    let resolved_session = resolve_session_id(
        conn,
        params.session_id,
        &events,
        &sessions_before,
        request_timeout,
    )
    .await;

    let images = wait_for_images(
        conn,
        &baseline,
        &WaitForImagesParams {
            session_id: resolved_session.as_deref(),
            timeout: params.timeout,
            poll_interval,
            settle,
            min_images: params.min_images,
            request_timeout,
        },
    )
    .await?;

    let fs = MuseFilesystem::new(conn.clone());
    let mut downloaded = Vec::new();
    let mut download_errors = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();
    for (index, r) in images.iter().enumerate() {
        let mut filename = filename_for_image(r, index);
        let mut suffix = 1u32;
        while used_names.contains(&filename) || params.output_dir.join(&filename).exists() {
            let stem = std::path::Path::new(&filename_for_image(r, index))
                .file_stem()
                .map_or_else(
                    || format!("muse_image_{}", index + 1),
                    |s| s.to_string_lossy().into_owned(),
                );
            let ext = std::path::Path::new(&filename_for_image(r, index))
                .extension()
                .map_or_else(|| "webp".to_string(), |e| e.to_string_lossy().into_owned());
            filename = format!("{stem}_{suffix}.{ext}");
            suffix += 1;
        }
        used_names.insert(filename.clone());
        let destination = params.output_dir.join(&filename);
        match download_image(&fs, r, &destination, request_timeout).await {
            Ok(()) => downloaded.push(destination),
            Err(e) => download_errors.push(format!("anh[{}] {}: {e}", index + 1, r.identity())),
        }
    }

    Ok(ImageGenerationResult {
        session_id: resolved_session,
        images,
        downloaded,
        download_errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_for_dung_ten_file_tu_path() {
        let r = VideoRef {
            path: Some("sandbox://a/b/video.mp4".into()),
            ..VideoRef::default()
        };
        assert_eq!(filename_for(&r, 0), "video.mp4");
    }

    #[test]
    fn filename_for_dung_ten_file_tu_url_bo_query() {
        let r = VideoRef {
            url: Some("https://cdn.example.com/x/clip.mp4?sig=abc".into()),
            ..VideoRef::default()
        };
        assert_eq!(filename_for(&r, 0), "clip.mp4");
    }

    #[test]
    fn filename_for_mac_dinh_khi_khong_co_ten_ro_rang() {
        let r = VideoRef {
            media_handle: Some("h1".into()),
            ..VideoRef::default()
        };
        assert_eq!(filename_for(&r, 2), "muse_video_3.mp4");
    }

    #[test]
    fn filename_for_image_dung_ten_file_tu_path() {
        let r = ImageRef {
            path: Some("sandbox://a/b/pic.webp".into()),
            ..ImageRef::default()
        };
        assert_eq!(filename_for_image(&r, 0), "pic.webp");
    }

    #[test]
    fn filename_for_image_mac_dinh_webp_khi_khong_co_ten_ro_rang() {
        let r = ImageRef {
            media_handle: Some("h1".into()),
            ..ImageRef::default()
        };
        assert_eq!(filename_for_image(&r, 0), "muse_image_1.webp");
    }

    #[test]
    fn build_image_prompt_giu_nguyen_y_nguoi_dung_them_huong_dan_ro_rang() {
        let prompt = build_image_prompt("một con mèo cam");
        assert!(prompt.starts_with("một con mèo cam"));
        // VAN phai bao hieu "anh tinh, khong phai video" de muse.ai tra kind:"image".
        assert!(prompt.to_lowercase().contains("still"));
        assert!(prompt.to_lowercase().contains("no video"));
        // KHONG duoc ep "product photography" — day la cai day ra anh ma-no-canh/san pham,
        // phan muc tieu khoa NGUOI (reel nhan vat). Xem spec image-first-reel 2026-10-20.
        assert!(!prompt.to_lowercase().contains("product photography"));
    }

    #[test]
    fn build_image_prompt_nguoi_giu_nguyen_prompt_goc() {
        let prompt = build_image_prompt("cô gái mặc đầm yếm nâu");
        assert!(prompt.starts_with("cô gái mặc đầm yếm nâu"));
        assert!(!prompt.to_lowercase().contains("product photography"));
    }

    #[test]
    fn build_video_prompt_khong_aspect_khong_duration_giu_nguyen() {
        let prompt = build_video_prompt("một con mèo cam", None, None);
        assert_eq!(prompt, "một con mèo cam");
    }

    /// Khop CHINH XAC cau da do that 10/10 lan (xem doc ham `build_video_prompt`) — port tu
    /// `prompt_builder.py::build_prompt`, KHONG tu bia cau khac.
    #[test]
    fn build_video_prompt_co_aspect_va_duration_khop_dung_cau_da_do_that() {
        let prompt = build_video_prompt("một con mèo cam", Some("9:16"), Some(30));
        assert_eq!(
            prompt,
            "một con mèo cam Output video: 30 seconds, vertical 9:16 portrait."
        );
    }

    #[test]
    fn build_video_prompt_aspect_khong_nhan_dien_duoc_bo_qua_chi_con_duration() {
        let prompt = build_video_prompt("một con mèo cam", Some("khong-hop-le"), Some(10));
        assert_eq!(prompt, "một con mèo cam Output video: 10 seconds.");
    }

    #[test]
    fn build_video_prompt_duration_0_bo_qua_chi_con_aspect() {
        let prompt = build_video_prompt("một con mèo cam", Some("16:9"), Some(0));
        assert_eq!(
            prompt,
            "một con mèo cam Output video: horizontal 16:9 landscape."
        );
    }
}
