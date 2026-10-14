//! Dựng chỉ dẫn hệ thống AFFILIATE — THUẦN (không I/O), test độc lập với phần gọi `TextGenHost`.
//!
//! Khác `lvc_genai::prompt::build_system` (dùng cho `script_gen`, chung cho mọi chủ đề):
//! [`build_system`] LUÔN bắt đầu từ một chỉ dẫn CHUYÊN cho kịch bản AFFILIATE (hook mở đầu, nêu lợi
//! ích/điểm đau, có CTA cuối) — `extra_instructions` (tham số node) chỉ NỐI THÊM vào cuối, không
//! thay thế, để node giữ đúng đặc trưng "chuyên sinh kịch bản affiliate" dù người dùng có tuỳ biến
//! thêm hay không.

/// Chỉ dẫn hệ thống đầy đủ gửi cho `TextGenHost` qua `TextGenReq.system` — không bao giờ rỗng,
/// nên provider sinh text (`lvc_genai`) không rơi về chỉ dẫn mặc định chung của nó.
#[must_use]
pub fn build_system(
    num_scenes: u32,
    language: &str,
    style: &str,
    extra_instructions: &str,
) -> String {
    let style_part = if style.trim().is_empty() {
        String::new()
    } else {
        format!(" Phong cach: {}.", style.trim())
    };
    let base = format!(
        "Ban la chuyen gia viet kich ban video AFFILIATE MARKETING (quang cao gan link hoa hong) \
ngan, dung nhip dieu bat mat kieu TikTok/Shopee/Reels: canh dau la HOOK giat tit giu nguoi xem, cac \
canh giua neu bat loi ich/diem dau cua khach hang va cong dung san pham, canh cuoi la LOI KEU GOI \
HANH DONG (CTA) ro rang (vd \"bam vao gio hang vang\", \"link o bio\", \"san ngay truoc khi het \
uu dai\"). Hay viet dung {n} phan canh, MOI CANH MOT DONG, KHONG danh so, KHONG tieu de, KHONG dong \
trong giua cac canh; moi dong la mot prompt hinh anh tu du nghia (boi canh, hanh dong, goc may, anh \
sang) bang ngon ngu '{lang}'.{style}",
        n = num_scenes,
        lang = language.trim(),
        style = style_part,
    );
    let extra = extra_instructions.trim();
    if extra.is_empty() {
        base
    } else {
        format!("{base}\n\nYeu cau them: {extra}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn co_hook_va_cta_co_so_canh_va_ngon_ngu() {
        let s = build_system(4, "vi", "", "");
        assert!(s.contains("HOOK"));
        assert!(s.contains("CTA"));
        assert!(s.contains("dung 4 phan canh"));
        assert!(s.contains("'vi'"));
    }

    #[test]
    fn co_phong_cach_khi_khac_rong() {
        let s = build_system(3, "vi", "hai huoc", "");
        assert!(s.contains("Phong cach: hai huoc"));
    }

    #[test]
    fn noi_them_yeu_cau_phu_khong_thay_the_nen() {
        let s = build_system(3, "vi", "", "nhan manh gia re");
        assert!(s.contains("HOOK"), "van giu nen affiliate");
        assert!(s.ends_with("Yeu cau them: nhan manh gia re"));
    }

    #[test]
    fn rong_thi_khong_them_dong_yeu_cau_them() {
        let s = build_system(3, "vi", "", "   ");
        assert!(!s.contains("Yeu cau them"));
    }
}
