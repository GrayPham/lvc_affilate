//! Nhóm cài đặt **Lưu trữ**: thư mục con theo gì, mẫu tên file, khi trùng tên thì làm gì.
//!
//! Toàn bộ file này THUẦN (không I/O) — node "Lưu video" chỉ gọi [`render_filename`] /
//! [`SubdirPattern::subdir`] để biết ghi vào đâu, việc tạo thư mục/ghi đĩa là của tầng engine.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::parse::{get_string, parse_enum};

/// Mẫu tên file mặc định — khớp bản thiết kế (`{workflow}_{gio}_{stt}`), KHÔNG kèm phần mở rộng
/// (phần mở rộng do node quyết định theo định dạng xuất).
pub const FILENAME_TEMPLATE_DEFAULT: &str = "{workflow}_{gio}_{stt}";

/// Các biến hợp lệ trong mẫu tên file, kèm mô tả tiếng Việt cho UI. Biến lạ trong mẫu được GIỮ
/// NGUYÊN (xem [`render_filename`]) — người dùng gõ sai không mất dữ liệu, chỉ thấy ngay chuỗi lạ.
pub const FILENAME_TOKENS: [(&str, &str); 5] = [
    ("{workflow}", "Tên workflow, bỏ dấu"),
    ("{ngay}", "Ngày chạy, dạng 2026-10-14"),
    ("{gio}", "Giờ chạy, dạng 1432"),
    ("{stt}", "Số thứ tự trong lần chạy, 3 chữ số"),
    ("{tai_khoan}", "Nhãn tài khoản đã dùng, bỏ dấu"),
];

/// Tạo thư mục con theo gì, dưới thư mục lưu video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubdirPattern {
    /// Ghi thẳng vào thư mục gốc.
    None,
    /// `<gốc>/<workflow>/`
    Workflow,
    /// `<gốc>/<workflow>/<ngày>/` — mặc định, tránh video nhiều workflow lẫn vào nhau.
    #[default]
    WorkflowDate,
    /// `<gốc>/<ngày>/`
    Date,
}

impl SubdirPattern {
    /// Chuỗi wire cho bảng `settings` + hợp đồng UI.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Workflow => "workflow",
            Self::WorkflowDate => "workflow_date",
            Self::Date => "date",
        }
    }

    /// # Errors
    /// Chuỗi không thuộc 4 giá trị trên.
    #[allow(
        clippy::result_unit_err,
        reason = "cho goi duy nhat (parse_enum) chi can biet dung/sai"
    )]
    pub fn parse(s: &str) -> Result<Self, ()> {
        match s.trim().to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "workflow" => Ok(Self::Workflow),
            "workflow_date" => Ok(Self::WorkflowDate),
            "date" => Ok(Self::Date),
            _ => Err(()),
        }
    }

    /// Đường dẫn TƯƠNG ĐỐI của thư mục con (dùng `/`, rỗng nghĩa là ghi thẳng vào gốc).
    ///
    /// Tên workflow được [`slug`] hoá nên luôn an toàn làm tên thư mục trên mọi hệ điều hành.
    #[must_use]
    pub fn subdir(self, workflow: &str, date: &str) -> String {
        let wf = slug(workflow);
        match self {
            Self::None => String::new(),
            Self::Workflow => wf,
            Self::WorkflowDate => format!("{wf}/{date}"),
            Self::Date => date.to_string(),
        }
    }
}

/// Khi tên file đã tồn tại thì làm gì.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DuplicatePolicy {
    /// Thêm `(1)`, `(2)`… vào trước phần mở rộng — mặc định, KHÔNG BAO GIỜ mất file cũ.
    #[default]
    Number,
    /// Ghi đè file cũ.
    Overwrite,
    /// Bỏ qua, không ghi.
    Skip,
}

impl DuplicatePolicy {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::Overwrite => "overwrite",
            Self::Skip => "skip",
        }
    }

    /// # Errors
    /// Chuỗi không thuộc 3 giá trị trên.
    #[allow(
        clippy::result_unit_err,
        reason = "cho goi duy nhat (parse_enum) chi can biet dung/sai"
    )]
    pub fn parse(s: &str) -> Result<Self, ()> {
        match s.trim().to_lowercase().as_str() {
            "number" => Ok(Self::Number),
            "overwrite" => Ok(Self::Overwrite),
            "skip" => Ok(Self::Skip),
            _ => Err(()),
        }
    }
}

/// Giá trị điền vào các biến của mẫu tên file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileNameVars<'a> {
    /// Tên workflow (chuỗi người dùng đặt, sẽ được bỏ dấu).
    pub workflow: &'a str,
    /// Ngày, đã định dạng sẵn (vd `2026-10-14`) — `lvc-core` THUẦN nên không tự lấy đồng hồ.
    pub date: &'a str,
    /// Giờ, đã định dạng sẵn (vd `1432`).
    pub time: &'a str,
    /// Số thứ tự trong lần chạy, đệm 0 thành 3 chữ số.
    pub index: u32,
    /// Nhãn tài khoản đã dùng (sẽ được bỏ dấu); rỗng nếu node không dùng tài khoản.
    pub account: &'a str,
}

/// Thay các biến trong mẫu bằng giá trị thật rồi làm sạch thành MỘT tên file an toàn.
///
/// - Biến lạ (vd `{khong_co}`) được giữ nguyên để người dùng thấy mình gõ sai.
/// - Kết quả không bao giờ chứa ký tự Windows cấm (`\ / : * ? " < > |`) hay dấu phân cách đường
///   dẫn — mẫu có `/` cũng KHÔNG tạo được thư mục con (chống thoát khỏi thư mục đã chọn).
/// - Mẫu rỗng / chỉ toàn ký tự bị loại → `"video"` (không bao giờ trả tên rỗng).
#[must_use]
pub fn render_filename(template: &str, vars: &FileNameVars) -> String {
    let filled = template
        .replace("{workflow}", &slug(vars.workflow))
        .replace("{ngay}", vars.date)
        .replace("{gio}", vars.time)
        .replace("{stt}", &format!("{:03}", vars.index))
        .replace("{tai_khoan}", &slug(vars.account));
    let cleaned: String = filled
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    if cleaned.is_empty() {
        "video".to_string()
    } else {
        cleaned
    }
}

/// Bỏ dấu tiếng Việt + hạ chữ thường + thay mọi ký tự còn lại bằng `-`, gộp `-` liền nhau.
///
/// Dùng cho tên thư mục/tên file nên kết quả là ASCII thuần: ffmpeg, công cụ dòng lệnh và dịch vụ
/// tải lên đều có nơi còn vỡ với tên Unicode. Rỗng → `"khong-ten"`.
#[must_use]
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true; // chặn dấu `-` ở đầu
    for ch in s.chars().flat_map(char::to_lowercase) {
        let folded = fold_vi(ch);
        if folded.is_ascii_alphanumeric() {
            out.push(folded);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "khong-ten".to_string()
    } else {
        trimmed
    }
}

/// Một ký tự tiếng Việt (ĐÃ hạ chữ thường) → chữ ASCII tương ứng; ký tự khác giữ nguyên.
fn fold_vi(c: char) -> char {
    match c {
        'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ' | 'ẩ'
        | 'ẫ' | 'ậ' => 'a',
        'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => 'e',
        'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ' | 'ở'
        | 'ỡ' | 'ợ' => 'o',
        'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => 'u',
        'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        'đ' => 'd',
        other => other,
    }
}

pub(super) fn parse_subdir_pattern(
    map: &HashMap<String, String>,
    warnings: &mut Vec<String>,
) -> SubdirPattern {
    parse_enum(
        map,
        "subdir_pattern",
        SubdirPattern::default(),
        SubdirPattern::default().as_str(),
        SubdirPattern::parse,
        warnings,
    )
}

pub(super) fn parse_duplicate_policy(
    map: &HashMap<String, String>,
    warnings: &mut Vec<String>,
) -> DuplicatePolicy {
    parse_enum(
        map,
        "duplicate_policy",
        DuplicatePolicy::default(),
        DuplicatePolicy::default().as_str(),
        DuplicatePolicy::parse,
        warnings,
    )
}

/// Mẫu tên file: chuỗi tự do, chỉ ép KHÔNG RỖNG (rỗng → mặc định, có cảnh báo vì người dùng xoá
/// hết thì không còn gì để đặt tên).
pub(super) fn parse_filename_template(
    map: &HashMap<String, String>,
    warnings: &mut Vec<String>,
) -> String {
    let raw = get_string(map, "filename_template", FILENAME_TEMPLATE_DEFAULT);
    if raw.trim().is_empty() {
        if map.contains_key("filename_template") {
            super::parse::invalid(
                warnings,
                "filename_template",
                &raw,
                FILENAME_TEMPLATE_DEFAULT,
            );
        }
        return FILENAME_TEMPLATE_DEFAULT.to_string();
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> FileNameVars<'static> {
        FileNameVars {
            workflow: "Storyboard ảnh từng cảnh",
            date: "2026-10-14",
            time: "1432",
            index: 3,
            account: "Nguyễn Văn A",
        }
    }

    #[test]
    fn slug_bo_dau_tieng_viet() {
        assert_eq!(slug("Storyboard ảnh từng cảnh"), "storyboard-anh-tung-canh");
        assert_eq!(slug("Đường Hồ Chí Minh"), "duong-ho-chi-minh");
        assert_eq!(slug("Nguyễn Văn A"), "nguyen-van-a");
    }

    #[test]
    fn slug_gop_dau_gach_va_bo_o_hai_dau() {
        assert_eq!(slug("  a   b  "), "a-b");
        assert_eq!(slug("--a--b--"), "a-b");
        assert_eq!(slug("a/b:c"), "a-b-c");
    }

    #[test]
    fn slug_rong_ra_khong_ten() {
        assert_eq!(slug(""), "khong-ten");
        assert_eq!(slug("///"), "khong-ten");
    }

    #[test]
    fn render_mau_mac_dinh_dung_nhu_ban_thiet_ke() {
        // Ban thiet ke cho vi du: storyboard-anh-tung-canh_1432_003
        assert_eq!(
            render_filename(FILENAME_TEMPLATE_DEFAULT, &vars()),
            "storyboard-anh-tung-canh_1432_003"
        );
    }

    #[test]
    fn render_stt_dem_0_thanh_3_chu_so() {
        let mut v = vars();
        v.index = 7;
        assert!(render_filename("{stt}", &v) == "007");
        v.index = 1234;
        assert_eq!(
            render_filename("{stt}", &v),
            "1234",
            "so lon hon 3 chu so KHONG bi cat"
        );
    }

    #[test]
    fn render_du_5_bien() {
        assert_eq!(
            render_filename("{workflow}-{ngay}-{gio}-{stt}-{tai_khoan}", &vars()),
            "storyboard-anh-tung-canh-2026-10-14-1432-003-nguyen-van-a"
        );
    }

    #[test]
    fn render_bien_la_giu_nguyen() {
        assert_eq!(
            render_filename("a_{khong_co}_b", &vars()),
            "a_{khong_co}_b",
            "bien la phai thay duoc nguoi dung, khong am tham bo"
        );
    }

    #[test]
    fn render_khong_tao_duoc_thu_muc_con() {
        let out = render_filename("..\\..\\{gio}", &vars());
        assert!(!out.contains('\\'), "{out}");
        assert!(!out.contains('/'), "{out}");
    }

    #[test]
    fn render_loai_ky_tu_windows_cam() {
        let out = render_filename("a:b*c?d\"e<f>g|h", &vars());
        for bad in [':', '*', '?', '"', '<', '>', '|'] {
            assert!(!out.contains(bad), "con sot {bad}: {out}");
        }
    }

    #[test]
    fn render_mau_rong_ra_video() {
        assert_eq!(render_filename("", &vars()), "video");
        assert_eq!(render_filename("...", &vars()), "video");
    }

    #[test]
    fn subdir_bon_kieu() {
        let wf = "Storyboard ảnh từng cảnh";
        let d = "2026-10-14";
        assert_eq!(SubdirPattern::None.subdir(wf, d), "");
        assert_eq!(
            SubdirPattern::Workflow.subdir(wf, d),
            "storyboard-anh-tung-canh"
        );
        assert_eq!(
            SubdirPattern::WorkflowDate.subdir(wf, d),
            "storyboard-anh-tung-canh/2026-10-14"
        );
        assert_eq!(SubdirPattern::Date.subdir(wf, d), "2026-10-14");
    }

    #[test]
    fn enum_as_str_parse_khop_nhau() {
        for p in [
            SubdirPattern::None,
            SubdirPattern::Workflow,
            SubdirPattern::WorkflowDate,
            SubdirPattern::Date,
        ] {
            assert_eq!(SubdirPattern::parse(p.as_str()).unwrap(), p);
        }
        for p in [
            DuplicatePolicy::Number,
            DuplicatePolicy::Overwrite,
            DuplicatePolicy::Skip,
        ] {
            assert_eq!(DuplicatePolicy::parse(p.as_str()).unwrap(), p);
        }
        assert!(SubdirPattern::parse("xyz").is_err());
        assert!(DuplicatePolicy::parse("xyz").is_err());
    }

    #[test]
    fn mac_dinh_la_khong_mat_file_cu() {
        assert_eq!(
            DuplicatePolicy::default(),
            DuplicatePolicy::Number,
            "mac dinh KHONG duoc la ghi de"
        );
    }

    #[test]
    fn filename_tokens_deu_xuat_hien_duoc_trong_render() {
        for (token, desc) in FILENAME_TOKENS {
            assert!(!desc.is_empty(), "{token} thieu mo ta cho UI");
            let out = render_filename(token, &vars());
            assert_ne!(out, token, "{token} phai duoc thay the");
        }
    }
}
