//! Kiểu dữ liệu thuần cho luồng publish. Không biết khái niệm node/scope của app — `lvc-app` dựng
//! sẵn `entry` (một mục `index.json`) rồi truyền vào.

use serde::{Deserialize, Serialize};

/// Yêu cầu publish một workflow/kịch bản lên kho cộng đồng.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishRequest {
    /// Id mục trong `index.json` — trùng id = CẬP NHẬT mục đó, khác = thêm mới.
    pub id: String,
    /// Tên hiển thị, chỉ dùng để đặt tiêu đề PR cho dễ đọc.
    pub name: String,
    /// Phiên bản, cũng chỉ để đặt tiêu đề/nhánh.
    pub version: String,
    /// Mục hoàn chỉnh sẽ nằm trong `index.json` (đã có `kind`, `graph`/`scenes`, …).
    pub entry: serde_json::Value,
}

/// Kết quả publish — đường dẫn PR để người dùng mở xem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    /// URL Pull Request vừa mở.
    pub pull_request_url: String,
    /// Tên nhánh đã tạo.
    pub branch: String,
    /// `true` = thay thế mục cùng id, `false` = thêm mục mới.
    pub updated_existing: bool,
}

/// Chèn/thay `entry` vào mảng `index.json` theo `id`. Trả `(mảng mới, đã-thay-thế?)`.
///
/// THUẦN — tách riêng để test không cần mạng.
///
/// # Errors
/// [`crate::GithubError::BadIndex`] khi `index` không phải mảng JSON.
pub fn upsert_entry(
    index: &serde_json::Value,
    id: &str,
    entry: &serde_json::Value,
) -> Result<(Vec<serde_json::Value>, bool), crate::GithubError> {
    let arr = index.as_array().ok_or(crate::GithubError::BadIndex)?;
    let mut out: Vec<serde_json::Value> = arr.clone();
    let found = out
        .iter()
        .position(|v| v.get("id").and_then(serde_json::Value::as_str) == Some(id));
    if let Some(i) = found {
        out[i] = entry.clone();
        Ok((out, true))
    } else {
        out.push(entry.clone());
        Ok((out, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn them_moi_khi_chua_co_id() {
        let index = json!([{ "id": "a", "name": "A" }]);
        let entry = json!({ "id": "b", "name": "B" });
        let (out, updated) = upsert_entry(&index, "b", &entry).unwrap();
        assert!(!updated);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1]["name"], "B");
    }

    #[test]
    fn thay_the_khi_trung_id_va_giu_nguyen_vi_tri() {
        let index = json!([
            { "id": "a", "name": "A" },
            { "id": "b", "name": "B cu" },
            { "id": "c", "name": "C" }
        ]);
        let entry = json!({ "id": "b", "name": "B moi" });
        let (out, updated) = upsert_entry(&index, "b", &entry).unwrap();
        assert!(updated);
        assert_eq!(out.len(), 3, "thay the chu khong them");
        assert_eq!(out[1]["name"], "B moi");
        assert_eq!(out[0]["id"], "a", "giu nguyen thu tu");
        assert_eq!(out[2]["id"], "c");
    }

    #[test]
    fn index_rong_van_them_duoc() {
        let (out, updated) = upsert_entry(&json!([]), "a", &json!({"id":"a"})).unwrap();
        assert!(!updated);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn index_khong_phai_mang_bao_loi_ro() {
        let err = upsert_entry(&json!({"khong": "phai mang"}), "a", &json!({})).unwrap_err();
        assert_eq!(err.code(), "GITHUB_BAD_INDEX");
    }
}
