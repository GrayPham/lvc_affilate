//! Dựng chỉ dẫn (system/user) và tách kịch bản thành từng cảnh — THUẦN, test độc lập, không I/O.

use crate::req::TextGenReq;

/// Chỉ dẫn hệ thống mặc định: yêu cầu model viết đúng `num_scenes` cảnh, MỖI CẢNH MỘT DÒNG, không
/// đánh số, mỗi dòng là một prompt hình ảnh tự đủ nghĩa (để nối thẳng vào node tạo video). Nếu
/// `req.system` khác rỗng thì dùng nguyên chỉ dẫn của người dùng.
#[must_use]
pub fn build_system(req: &TextGenReq) -> String {
    if !req.system.trim().is_empty() {
        return req.system.trim().to_string();
    }
    let style = if req.style.trim().is_empty() {
        String::new()
    } else {
        format!(" Phong cach: {}.", req.style.trim())
    };
    format!(
        "Ban la bien kich video ngan. Hay viet dung {n} phan canh cho mot video theo yeu cau. \
QUY TAC DAU RA: moi phan canh MOT DONG, KHONG danh so, KHONG tieu de, KHONG dong trong giua cac \
canh; moi dong la mot prompt hinh anh tu du nghia (boi canh, hanh dong, goc may, anh sang) bang \
ngon ngu '{lang}'.{style}",
        n = req.num_scenes,
        lang = req.language.trim(),
        style = style,
    )
}

/// Nội dung người dùng: brief/chủ đề.
#[must_use]
pub fn build_user(req: &TextGenReq) -> String {
    format!("Chu de/san pham: {}", req.prompt.trim())
}

/// Tách văn bản thô của model thành từng cảnh: bỏ dòng trống + dòng `#`, cắt tiền tố đánh số/bullet
/// (`1.`, `1)`, `-`, `*`, `Cảnh 2:`), `trim`. Giữ đúng thứ tự.
#[must_use]
pub fn split_scenes(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(strip_prefix)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Cắt tiền tố liệt kê thường gặp ở đầu dòng (bullet, đánh số) — chỉ thao tác ASCII đầu chuỗi nên
/// an toàn với ký tự đa byte (tiếng Việt) ở phần còn lại.
fn strip_prefix(line: &str) -> &str {
    let s = line.trim();
    // Bullet "- ", "* ", "• "
    if let Some(rest) = s
        .strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .or_else(|| s.strip_prefix("• "))
    {
        return rest.trim();
    }
    // "1. " / "1) " / "1: " (số ASCII ở đầu, chỉ cắt khi vẫn còn nội dung sau dấu).
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 {
        let rest = &s[digits..];
        if let Some(r) = rest
            .strip_prefix(". ")
            .or_else(|| rest.strip_prefix(") "))
            .or_else(|| rest.strip_prefix(": "))
        {
            return r.trim();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Provider;

    fn req(system: &str, style: &str) -> TextGenReq {
        TextGenReq {
            credential_name: "k".into(),
            provider: Provider::Gemini,
            system: system.into(),
            prompt: "  ban chay giay  ".into(),
            num_scenes: 3,
            language: "vi".into(),
            style: style.into(),
        }
    }

    #[test]
    fn system_mac_dinh_co_so_canh_va_ngon_ngu() {
        let s = build_system(&req("", "nang dong"));
        assert!(s.contains("dung 3 phan canh"));
        assert!(s.contains("'vi'"));
        assert!(s.contains("Phong cach: nang dong"));
    }

    #[test]
    fn system_nguoi_dung_thang() {
        assert_eq!(build_system(&req("Viet 5 canh", "")), "Viet 5 canh");
    }

    #[test]
    fn user_co_brief_da_trim() {
        assert_eq!(build_user(&req("", "")), "Chu de/san pham: ban chay giay");
    }

    #[test]
    fn tach_canh_bo_so_bullet_va_dong_trong() {
        let raw = "1. Cảnh mở đầu\n\n2) Nhân vật hành động\n# ghi chu\n- Kết thúc";
        let scenes = split_scenes(raw);
        assert_eq!(
            scenes,
            vec!["Cảnh mở đầu", "Nhân vật hành động", "Kết thúc"]
        );
    }

    #[test]
    fn tach_canh_giu_dong_khong_tien_to() {
        assert_eq!(split_scenes("mot dong don gian"), vec!["mot dong don gian"]);
        assert!(split_scenes("\n  \n#chi comment").is_empty());
    }
}
