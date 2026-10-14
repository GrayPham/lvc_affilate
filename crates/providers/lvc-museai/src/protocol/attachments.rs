//! Ảnh tham chiếu gửi kèm prompt tới `/chat/stream`.
//!
//! muse.ai KHÔNG có endpoint tải ảnh riêng: web client nhúng thẳng ảnh vào mảng `items` của
//! `/chat/stream` dưới dạng base64 (port từ `attachments.py` của bản tham chiếu, CHỈ ĐỌC). Vì thế
//! module này không gọi mạng — chỉ đọc file, kiểm ngân sách, rồi dựng `items`.
//!
//! ## Vì sao phải có ngân sách
//! Gói Noise ráp lại ở phía máy chủ có hạn ~16 MB. Base64 phình dữ liệu lên ~4/3, nên tổng ảnh
//! THÔ phải ở dưới ~11 MB thì cả yêu cầu mới lọt. Vượt ngưỡng thì `/chat/stream` hỏng giữa chừng
//! với lỗi khó hiểu — thà chặn ngay ở đây với câu nói rõ người dùng cần làm gì.
//!
//! ## Khác bản Python một chỗ, có chủ đích
//! Bản Python tự nén/thu nhỏ ảnh quá cỡ bằng PIL. Ở đây KHÔNG làm: workspace không có thư viện
//! xử lý ảnh, và kéo cả bộ codec vào chỉ để co một tấm ảnh là cái giá quá đắt cho một việc người
//! dùng tự làm được trong 5 giây. Ảnh quá cỡ bị từ chối kèm hướng dẫn cụ thể.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::{Value, json};

use super::errors::MuseError;

type Result<T> = std::result::Result<T, MuseError>;

/// Trần cho MỘT ảnh (byte thô). Khớp `_PER_IMAGE_CAP` của bản tham chiếu.
pub const PER_IMAGE_CAP: u64 = 3_500_000;

/// Trần cho TỔNG các ảnh trong một yêu cầu. Khớp `_TOTAL_IMAGE_BUDGET`.
pub const TOTAL_IMAGE_BUDGET: u64 = 11_000_000;

/// Kiểu MIME suy từ đuôi file. Đuôi lạ → `None` (gọi hàm sẽ từ chối file đó).
#[must_use]
pub fn mime_for(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

/// Dựng mảng `items` cho `/chat/stream`: các ảnh TRƯỚC, text sau — đúng thứ tự `build_items` của
/// bản tham chiếu (muse.ai đọc ảnh làm ngữ cảnh cho câu lệnh đứng sau).
///
/// # Errors
/// [`MuseError::Protocol`] khi: file không đọc được, đuôi không phải ảnh nhận được, một ảnh vượt
/// [`PER_IMAGE_CAP`], hoặc tổng vượt [`TOTAL_IMAGE_BUDGET`].
pub fn build_items(images: &[PathBuf], prompt: &str) -> Result<Value> {
    let mut items: Vec<Value> = Vec::with_capacity(images.len() + 1);
    let mut total: u64 = 0;

    for path in images {
        let mime = mime_for(path).ok_or_else(|| {
            MuseError::Protocol(format!(
                "{} khong phai anh nhan duoc (chi jpg/jpeg/png/webp/gif)",
                path.display()
            ))
        })?;
        let bytes = std::fs::read(path)
            .map_err(|e| MuseError::Protocol(format!("khong doc duoc {}: {e}", path.display())))?;
        let size = bytes.len() as u64;

        if size > PER_IMAGE_CAP {
            return Err(MuseError::Protocol(format!(
                "{} nang {} — vuot muc {} cho MOT anh. Thu nho anh (vd xuat lai o canh dai 2048px) \
                 roi chay lai.",
                path.display(),
                human_size(size),
                human_size(PER_IMAGE_CAP)
            )));
        }
        total += size;
        if total > TOTAL_IMAGE_BUDGET {
            return Err(MuseError::Protocol(format!(
                "tong anh tham chieu vuot {} — bo bot anh hoac thu nho chung roi chay lai",
                human_size(TOTAL_IMAGE_BUDGET)
            )));
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("anh")
            .to_string();
        items.push(json!({
            "type": "image",
            "mime_type": mime,
            "data_base64": B64.encode(&bytes),
            "filename": filename,
        }));
    }

    items.push(json!({ "type": "text", "text": prompt }));
    Ok(Value::Array(items))
}

/// `3,5 MB` / `820 KB` — chỉ để ghép vào thông điệp lỗi cho người đọc.
fn human_size(bytes: u64) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "chi de hien thi, khong tinh toan tiep"
    )]
    let mb = bytes as f64 / 1_000_000.0;
    if mb >= 1.0 {
        format!("{mb:.1} MB").replace('.', ",")
    } else {
        #[allow(clippy::cast_precision_loss, reason = "chi de hien thi")]
        let kb = bytes as f64 / 1_000.0;
        format!("{kb:.0} KB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, len: usize) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, vec![b'x'; len]).unwrap();
        p
    }

    #[test]
    fn mime_theo_duoi_file() {
        assert_eq!(mime_for(Path::new("a.jpg")), Some("image/jpeg"));
        assert_eq!(mime_for(Path::new("a.JPEG")), Some("image/jpeg"));
        assert_eq!(mime_for(Path::new("a.png")), Some("image/png"));
        assert_eq!(mime_for(Path::new("a.webp")), Some("image/webp"));
        assert_eq!(mime_for(Path::new("a.txt")), None);
        assert_eq!(mime_for(Path::new("khong_co_duoi")), None);
    }

    #[test]
    fn khong_co_anh_thi_chi_con_text() {
        let items = build_items(&[], "canh 1").unwrap();
        let arr = items.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "canh 1");
    }

    #[test]
    fn anh_dung_truoc_text() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "nhan_vat.png", 10);
        let items = build_items(&[a], "canh 1").unwrap();
        let arr = items.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "image", "anh phai dung TRUOC cau lenh");
        assert_eq!(arr[0]["mime_type"], "image/png");
        assert_eq!(arr[0]["filename"], "nhan_vat.png");
        assert_eq!(arr[1]["type"], "text");
    }

    #[test]
    fn du_lieu_la_base64_cua_dung_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.png");
        std::fs::write(&p, b"PNGDATA").unwrap();
        let items = build_items(&[p], "x").unwrap();
        let b64 = items[0]["data_base64"].as_str().unwrap();
        assert_eq!(B64.decode(b64).unwrap(), b"PNGDATA");
    }

    #[test]
    fn duoi_khong_phai_anh_bi_tu_choi_voi_cau_ro() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "ghi_chu.txt", 10);
        let err = build_items(&[p], "x").unwrap_err().to_string();
        assert!(err.contains("khong phai anh nhan duoc"), "{err}");
    }

    #[test]
    fn file_khong_ton_tai_bao_ro_ten_file() {
        let err = build_items(&[PathBuf::from("khong/co/that.png")], "x")
            .unwrap_err()
            .to_string();
        assert!(err.contains("khong doc duoc"), "{err}");
    }

    #[test]
    fn mot_anh_qua_co_bi_chan_kem_huong_dan() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(
            dir.path(),
            "to.jpg",
            usize::try_from(PER_IMAGE_CAP + 1).expect("tran anh vua trong usize tren moi nen"),
        );
        let err = build_items(&[p], "x").unwrap_err().to_string();
        assert!(err.contains("vuot muc"), "{err}");
        assert!(
            err.contains("Thu nho anh"),
            "phai noi nguoi dung can lam gi: {err}"
        );
    }

    #[test]
    fn tong_vuot_ngan_sach_bi_chan_du_tung_anh_deu_hop_le() {
        let dir = tempfile::tempdir().unwrap();
        // 4 anh x 3MB = 12MB > 11MB, nhung MOI anh deu duoi tran mot anh (3,5MB).
        let imgs: Vec<PathBuf> = (0..4)
            .map(|i| write(dir.path(), &format!("a{i}.jpg"), 3_000_000))
            .collect();
        let err = build_items(&imgs, "x").unwrap_err().to_string();
        assert!(err.contains("tong anh tham chieu vuot"), "{err}");
    }

    #[test]
    fn nhieu_anh_trong_ngan_sach_thi_qua() {
        let dir = tempfile::tempdir().unwrap();
        let imgs: Vec<PathBuf> = (0..3)
            .map(|i| write(dir.path(), &format!("a{i}.jpg"), 1_000_000))
            .collect();
        let items = build_items(&imgs, "canh 1").unwrap();
        assert_eq!(items.as_array().unwrap().len(), 4, "3 anh + 1 text");
    }

    #[test]
    fn human_size_doc_duoc() {
        assert_eq!(human_size(3_500_000), "3,5 MB");
        assert_eq!(human_size(820_000), "820 KB");
    }
}
