//! Trích xuất tham chiếu video/session từ JSON lịch sử chat — port từ `media.py` (tham khảo, CHỈ
//! ĐỌC). THUẦN (không I/O) — chỉ duyệt cây `serde_json::Value` bất kỳ hình dạng nào.

use std::collections::HashSet;

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoRef {
    pub path: Option<String>,
    pub url: Option<String>,
    pub mime_type: Option<String>,
    pub resource_id: Option<String>,
    pub media_handle: Option<String>,
}

impl VideoRef {
    /// URL ký (signed) có thể đổi trong khi file sinh ra không đổi — ưu tiên định danh BỀN qua
    /// `path`/`resource_id`/`media_handle` trước `url`.
    #[must_use]
    pub fn identity(&self) -> String {
        self.path
            .clone()
            .or_else(|| self.resource_id.clone())
            .or_else(|| self.media_handle.clone())
            .or_else(|| self.url.clone())
            .unwrap_or_else(|| format!("{self:?}"))
    }
}

fn is_video_mime(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("video/") || lower == "mp4"
}

fn looks_video_path(value: &str) -> bool {
    let clean = value
        .to_ascii_lowercase()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_string();
    [".mp4", ".webm", ".mov", ".m4v"]
        .iter()
        .any(|ext| clean.ends_with(ext))
}

/// Tìm mọi `session_id`/`sessionId`/`thread_id`/`threadId` (chuỗi, khác rỗng) trong cây JSON,
/// giữ thứ tự gặp đầu tiên, không trùng lặp.
#[must_use]
pub fn extract_session_ids(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk_session_ids(value, &mut found);
    found
}

fn walk_session_ids(node: &Value, found: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (key, item) in map {
                if matches!(
                    key.as_str(),
                    "session_id" | "sessionId" | "thread_id" | "threadId"
                ) {
                    if let Some(s) = item.as_str() {
                        if !s.is_empty() && !found.iter().any(|f| f == s) {
                            found.push(s.to_string());
                        }
                    }
                }
                walk_session_ids(item, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_session_ids(item, found);
            }
        }
        _ => {}
    }
}

fn path_basename(path: Option<&str>) -> Option<String> {
    let path = path?;
    let normalized = path.replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    let base = trimmed.rsplit('/').next().unwrap_or(trimmed);
    (!base.is_empty()).then(|| base.to_string())
}

/// Tập "bí danh" dùng để phát hiện 2 `VideoRef` thật ra là CÙNG một video (vd lịch sử trả về cả
/// tên file trần lẫn đường dẫn `sandbox://` đầy đủ cho cùng file — xem ghi chú gốc `_ref_aliases`).
fn ref_aliases(r: &VideoRef) -> HashSet<String> {
    let mut values: HashSet<String> = [&r.path, &r.url, &r.resource_id, &r.media_handle]
        .into_iter()
        .filter_map(Clone::clone)
        .collect();
    if let Some(base) = path_basename(r.path.as_deref()) {
        values.insert(format!("name:{base}"));
    }
    values
}

/// Giữ đường dẫn "đầy đủ" (nhiều `/` hơn) khi gộp 2 ref trùng — tie nghiêng về `a`.
fn more_specific_path(a: Option<&str>, b: Option<&str>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let count = |s: &str| s.matches('/').count();
            Some(if count(a) >= count(b) { a } else { b }.to_string())
        }
        (Some(a), None) => Some(a.to_string()),
        (None, Some(b)) => Some(b.to_string()),
        (None, None) => None,
    }
}

struct RefCollector {
    found: Vec<VideoRef>,
}

impl RefCollector {
    fn add(&mut self, r: VideoRef) {
        if r.path.is_none() && r.url.is_none() && r.media_handle.is_none() {
            return;
        }
        let aliases = ref_aliases(&r);
        if let Some(pos) = self
            .found
            .iter()
            .position(|existing| !aliases.is_disjoint(&ref_aliases(existing)))
        {
            let existing = self.found.remove(pos);
            let merged = VideoRef {
                path: more_specific_path(r.path.as_deref(), existing.path.as_deref()),
                url: r.url.or(existing.url),
                mime_type: r.mime_type.or(existing.mime_type),
                resource_id: r.resource_id.or(existing.resource_id),
                media_handle: r.media_handle.or(existing.media_handle),
            };
            self.found.push(merged);
            return;
        }
        self.found.push(r);
    }
}

fn str_field<'a>(map: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| map.get(*k))
        .and_then(Value::as_str)
}

fn walk_video_refs(node: &Value, collector: &mut RefCollector) {
    match node {
        Value::Object(map) => {
            let mime = str_field(
                map,
                &[
                    "mime_type",
                    "mimeType",
                    "mime",
                    "content_type",
                    "contentType",
                ],
            );
            let kind = str_field(map, &["kind", "type"]);
            let path = map.get("path").and_then(Value::as_str);
            let url = map.get("url").and_then(Value::as_str);
            let original = map
                .get("variants")
                .and_then(Value::as_object)
                .and_then(|v| v.get("original"))
                .and_then(Value::as_str);
            let resource_id = str_field(map, &["resource_id", "resourceId"]);
            let media_handle = str_field(map, &["media_handle", "mediaHandle"]);

            let videoish = kind == Some("video")
                || mime.is_some_and(is_video_mime)
                || path.is_some_and(looks_video_path)
                || url.is_some_and(looks_video_path)
                || original.is_some_and(looks_video_path);

            if videoish {
                collector.add(VideoRef {
                    path: path.map(str::to_string),
                    url: original.or(url).map(str::to_string),
                    mime_type: mime.map(str::to_string),
                    resource_id: resource_id.map(str::to_string),
                    media_handle: media_handle.map(str::to_string),
                });
            }
            for item in map.values() {
                walk_video_refs(item, collector);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_video_refs(item, collector);
            }
        }
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    walk_video_refs(&parsed, collector);
                    return;
                }
            }
            if looks_video_path(s) {
                if s.starts_with("http://") || s.starts_with("https://") {
                    collector.add(VideoRef {
                        url: Some(s.clone()),
                        ..VideoRef::default()
                    });
                } else {
                    collector.add(VideoRef {
                        path: Some(s.clone()),
                        ..VideoRef::default()
                    });
                }
            }
        }
        _ => {}
    }
}

/// Duyệt TOÀN BỘ cây JSON (chuỗi lồng JSON cũng được parse đệ quy) tìm mọi tham chiếu video, gộp
/// các ref trùng (cùng file dưới nhiều dạng khác nhau — xem [`ref_aliases`]).
#[must_use]
pub fn extract_video_refs(value: &Value) -> Vec<VideoRef> {
    let mut collector = RefCollector { found: Vec::new() };
    walk_video_refs(value, &mut collector);
    collector.found
}

/// Tham chiếu một ẢNH sinh ra trong lịch sử chat — ĐÃ XÁC NHẬN qua trang thật 2026-10-12 (prompt
/// xin ảnh tĩnh thay vì video → phản hồi có `"kind":"image"` + `data.images[].{path,mime,variants.
/// original}`, cùng hình dạng field với [`VideoRef`] nên port y hệt, KHÔNG dùng chung struct để
/// không đụng code video đã chạy thật ổn định — xem `CLAUDE.md`/`MEMORY.md`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageRef {
    pub path: Option<String>,
    pub url: Option<String>,
    pub mime_type: Option<String>,
    pub resource_id: Option<String>,
    pub media_handle: Option<String>,
}

impl ImageRef {
    #[must_use]
    pub fn identity(&self) -> String {
        self.path
            .clone()
            .or_else(|| self.resource_id.clone())
            .or_else(|| self.media_handle.clone())
            .or_else(|| self.url.clone())
            .unwrap_or_else(|| format!("{self:?}"))
    }
}

fn is_image_mime(value: &str) -> bool {
    value.to_ascii_lowercase().starts_with("image/")
}

fn looks_image_path(value: &str) -> bool {
    let clean = value
        .to_ascii_lowercase()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_string();
    [".webp", ".png", ".jpg", ".jpeg", ".gif"]
        .iter()
        .any(|ext| clean.ends_with(ext))
}

fn image_ref_aliases(r: &ImageRef) -> HashSet<String> {
    let mut values: HashSet<String> = [&r.path, &r.url, &r.resource_id, &r.media_handle]
        .into_iter()
        .filter_map(Clone::clone)
        .collect();
    if let Some(base) = path_basename(r.path.as_deref()) {
        values.insert(format!("name:{base}"));
    }
    values
}

struct ImageRefCollector {
    found: Vec<ImageRef>,
}

impl ImageRefCollector {
    fn add(&mut self, r: ImageRef) {
        if r.path.is_none() && r.url.is_none() && r.media_handle.is_none() {
            return;
        }
        let aliases = image_ref_aliases(&r);
        if let Some(pos) = self
            .found
            .iter()
            .position(|existing| !aliases.is_disjoint(&image_ref_aliases(existing)))
        {
            let existing = self.found.remove(pos);
            let merged = ImageRef {
                path: more_specific_path(r.path.as_deref(), existing.path.as_deref()),
                url: r.url.or(existing.url),
                mime_type: r.mime_type.or(existing.mime_type),
                resource_id: r.resource_id.or(existing.resource_id),
                media_handle: r.media_handle.or(existing.media_handle),
            };
            self.found.push(merged);
            return;
        }
        self.found.push(r);
    }
}

fn walk_image_refs(node: &Value, collector: &mut ImageRefCollector) {
    match node {
        Value::Object(map) => {
            let mime = str_field(
                map,
                &[
                    "mime_type",
                    "mimeType",
                    "mime",
                    "content_type",
                    "contentType",
                ],
            );
            let kind = str_field(map, &["kind", "type"]);
            let path = map.get("path").and_then(Value::as_str);
            let url = map.get("url").and_then(Value::as_str);
            let original = map
                .get("variants")
                .and_then(Value::as_object)
                .and_then(|v| v.get("original"))
                .and_then(Value::as_str);
            let resource_id = str_field(map, &["resource_id", "resourceId"]);
            let media_handle = str_field(map, &["media_handle", "mediaHandle"]);

            let imageish = kind == Some("image")
                || mime.is_some_and(is_image_mime)
                || path.is_some_and(looks_image_path)
                || url.is_some_and(looks_image_path)
                || original.is_some_and(looks_image_path);

            if imageish {
                collector.add(ImageRef {
                    path: path.map(str::to_string),
                    url: original.or(url).map(str::to_string),
                    mime_type: mime.map(str::to_string),
                    resource_id: resource_id.map(str::to_string),
                    media_handle: media_handle.map(str::to_string),
                });
            }
            for item in map.values() {
                walk_image_refs(item, collector);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_image_refs(item, collector);
            }
        }
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    walk_image_refs(&parsed, collector);
                    return;
                }
            }
            if looks_image_path(s) {
                if s.starts_with("http://") || s.starts_with("https://") {
                    collector.add(ImageRef {
                        url: Some(s.clone()),
                        ..ImageRef::default()
                    });
                } else {
                    collector.add(ImageRef {
                        path: Some(s.clone()),
                        ..ImageRef::default()
                    });
                }
            }
        }
        _ => {}
    }
}

/// Như [`extract_video_refs`] nhưng cho ẢNH — xem doc [`ImageRef`].
#[must_use]
pub fn extract_image_refs(value: &Value) -> Vec<ImageRef> {
    let mut collector = ImageRefCollector { found: Vec::new() };
    walk_image_refs(value, &mut collector);
    collector.found
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_session_ids_tim_dung_khong_trung() {
        let value = json!({
            "a": {"session_id": "s1"},
            "b": [{"thread_id": "s2"}, {"sessionId": "s1"}],
        });
        assert_eq!(
            extract_session_ids(&value),
            vec!["s1".to_string(), "s2".to_string()]
        );
    }

    #[test]
    fn extract_video_refs_nhan_dien_theo_kind() {
        let value = json!({"items": [{"kind": "video", "path": "sandbox://a/b.bin"}]});
        let refs = extract_video_refs(&value);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].path.as_deref(), Some("sandbox://a/b.bin"));
    }

    #[test]
    fn extract_video_refs_nhan_dien_theo_duoi_file_url() {
        let value = json!({"url": "https://cdn.example.com/x/video.mp4?sig=1"});
        let refs = extract_video_refs(&value);
        assert_eq!(refs.len(), 1);
        assert_eq!(
            refs[0].url.as_deref(),
            Some("https://cdn.example.com/x/video.mp4?sig=1")
        );
    }

    #[test]
    fn extract_video_refs_gop_ref_trung_qua_ten_file() {
        // Mot video xuat hien 2 dang: ten file tran + duong dan sandbox day du.
        let value = json!([
            {"kind": "video", "path": "media-generation-123.mp4"},
            {"kind": "video", "path": "sandbox://workspace/imagine_media/media-generation-123.mp4",
             "url": "https://cdn.example.com/media-generation-123.mp4"},
        ]);
        let refs = extract_video_refs(&value);
        assert_eq!(refs.len(), 1, "phai gop thanh 1 video, khong phai 2");
        assert_eq!(
            refs[0].path.as_deref(),
            Some("sandbox://workspace/imagine_media/media-generation-123.mp4"),
            "giu duong dan DAY DU hon, khong giu ten file tran"
        );
        assert!(refs[0].url.is_some());
    }

    #[test]
    fn extract_video_refs_bo_qua_anh_khong_phai_video() {
        let value = json!({"kind": "image", "path": "a.png"});
        assert!(extract_video_refs(&value).is_empty());
    }

    #[test]
    fn extract_video_refs_tu_chuoi_json_long_nhau() {
        let inner = json!({"kind": "video", "path": "x.mp4"}).to_string();
        let value = json!({"payload": inner});
        let refs = extract_video_refs(&value);
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn identity_uu_tien_path_roi_resource_id_roi_media_handle_roi_url() {
        let r = VideoRef {
            path: None,
            url: Some("u".into()),
            mime_type: None,
            resource_id: Some("r".into()),
            media_handle: Some("m".into()),
        };
        assert_eq!(r.identity(), "r");
    }

    /// Hình dạng THẬT đã xác nhận 2026-10-12 (prompt xin ảnh tĩnh qua `/chat/stream` that, xem
    /// `CLAUDE.md`/`MEMORY.md` provider) — `data.images[].{path,mime,variants.original}` lồng
    /// trong một object `kind:"image"` ở cấp cha.
    #[test]
    fn extract_image_refs_dung_hinh_dang_phan_hoi_that() {
        let value = json!({
            "kind": "image",
            "image_path": "workspace/imagine_media/media-generation-abc.webp",
            "data": {
                "images": [{
                    "byte_len": 229_938,
                    "label": "red apple on white background",
                    "mime": "image/webp",
                    "missing": false,
                    "path": "sandbox://workspace/imagine_media/media-generation-abc.webp",
                    "variants": {
                        "original": "https://vm.metaaivm.com/media/raw/workspace/imagine_media/media-generation-abc.webp"
                    }
                }]
            }
        });
        let refs = extract_image_refs(&value);
        assert!(!refs.is_empty());
        assert!(
            refs.iter()
                .any(|r| r.mime_type.as_deref() == Some("image/webp"))
        );
        assert!(
            refs.iter()
                .any(|r| r.url.as_deref() == Some("https://vm.metaaivm.com/media/raw/workspace/imagine_media/media-generation-abc.webp"))
        );
    }

    #[test]
    fn extract_image_refs_bo_qua_video() {
        let value = json!({"kind": "video", "path": "a.mp4"});
        assert!(extract_image_refs(&value).is_empty());
    }

    #[test]
    fn extract_video_refs_bo_qua_anh() {
        let value = json!({"kind": "image", "path": "a.webp"});
        assert!(extract_video_refs(&value).is_empty());
    }

    #[test]
    fn extract_image_refs_gop_ref_trung_qua_ten_file() {
        let value = json!([
            {"kind": "image", "path": "media-gen-123.webp"},
            {"kind": "image", "path": "sandbox://workspace/imagine_media/media-gen-123.webp",
             "url": "https://cdn.example.com/media-gen-123.webp"},
        ]);
        let refs = extract_image_refs(&value);
        assert_eq!(refs.len(), 1, "phai gop thanh 1 anh, khong phai 2");
    }
}
