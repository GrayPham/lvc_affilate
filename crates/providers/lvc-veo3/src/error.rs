//! Bang loi Google (`flow.google.com`) dung chung cho ca anh va video. Port thuan tu
//! `Lvc_veo3_automation/src/core/flow_error_catalog.py`. MOT bang (ma chinh xac) + khop-theo-ho
//! (substring, HEP truoc RONG) de ma CHUA TUNG GAP van ra huong dan dung huong.

use std::sync::LazyLock;

use regex::Regex;

/// Phan loai loi: co nen retry khong, va retry co cuu duoc khong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// Thu lai y het chac chan hong: het han muc ngay, vi pham noi dung, sai ngon ngu, model
    /// khong co quyen.
    Hard,
    /// Nghi roi thu lai duoc: 403/reCAPTCHA, qua tai, nhip.
    Transient,
    /// Hong o PHIEN tai khoan — nap lai cookie/dang nhap, retry vo ich toi khi sua.
    Auth,
}

type Entry = (&'static str, ErrorClass, &'static str);

/// Ma CHINH XAC da gap that trong log Python (giu nguyen tieng Viet cua ban goc).
const EXACT: &[Entry] = &[
    (
        "PUBLIC_ERROR_UNUSUAL_ACTIVITY",
        ErrorClass::Transient,
        "Google chan vi thieu hoac sai token reCAPTCHA — sinh token moi roi thu lai.",
    ),
    (
        "PUBLIC_ERROR_UNUSUAL_ACTIVITY_TOO_MUCH_TRAFFIC",
        ErrorClass::Transient,
        "Google chan vi ban qua nhieu yeu cau mot luc. Giam so luong va cho vai phut roi chay lai.",
    ),
    (
        "PUBLIC_ERROR_HIGH_TRAFFIC",
        ErrorClass::Transient,
        "May chu Google dang qua tai. Cho vai phut roi thu lai.",
    ),
    (
        "PUBLIC_ERROR_PER_MODEL_DAILY_QUOTA_REACHED",
        ErrorClass::Hard,
        "Tai khoan het han muc ngay cua model dang chon. Doi tai khoan hoac doi model.",
    ),
    (
        "PUBLIC_ERROR_USER_QUOTA_REACHED",
        ErrorClass::Hard,
        "Tai khoan het credit. Doi tai khoan khac hoac cho reset.",
    ),
    (
        "PUBLIC_ERROR_MODEL_ACCESS_DENIED",
        ErrorClass::Hard,
        "Tai khoan khong co quyen dung model nay. Doi model hoac doi tai khoan.",
    ),
    (
        "PUBLIC_ERROR_UNSAFE_GENERATION",
        ErrorClass::Hard,
        "Prompt vi pham chinh sach noi dung — sua loi ta nhe hon.",
    ),
    (
        "PUBLIC_ERROR_SEXUAL_CONTENT",
        ErrorClass::Hard,
        "Bi chan vi noi dung tinh duc/khieu dam. Sua loi ta.",
    ),
    (
        "PUBLIC_ERROR_VIOLENCE_FILTER",
        ErrorClass::Hard,
        "Bi chan vi noi dung bao luc/mau me. Sua loi ta.",
    ),
    (
        "PUBLIC_ERROR_HATE_SPEECH",
        ErrorClass::Hard,
        "Bi chan vi ngon tu thu ghet. Sua loi ta.",
    ),
    (
        "PUBLIC_ERROR_MINOR",
        ErrorClass::Hard,
        "Bi chan vi mo ta tre vi thanh nien. Bo chi tiet tuoi/tre em trong prompt.",
    ),
    (
        "PUBLIC_ERROR_MINOR_UPLOAD",
        ErrorClass::Hard,
        "Anh tham chieu bi chan vi co tre vi thanh nien. Doi anh tham chieu.",
    ),
    (
        "PUBLIC_ERROR_PROMINENT_PEOPLE_UPLOAD",
        ErrorClass::Hard,
        "Anh tham chieu bi chan vi co nguoi noi tieng. Doi anh tham chieu.",
    ),
    (
        "PUBLIC_ERROR_PROMINENT_PEOPLE_FILTER_FAILED",
        ErrorClass::Hard,
        "Bi chan vi dinh nguoi noi tieng/nhan vat co thuc. Bo ten rieng hoac doi anh nhan vat.",
    ),
    (
        "PUBLIC_ERROR_DANGER_FILTER",
        ErrorClass::Hard,
        "Google anti-deepfake chan yeu cau. Bo 'replace person'/'swap face' khoi prompt.",
    ),
    (
        "PUBLIC_ERROR_NON_ENGLISH_PROMPT",
        ErrorClass::Hard,
        "Google doi prompt tieng Anh.",
    ),
    (
        "PUBLIC_ERROR_AUDIO_FILTERED",
        ErrorClass::Transient,
        "Phan tieng bi loc vi noi dung. Video van ra nhung mat tieng.",
    ),
    (
        "PUBLIC_ERROR_SOMETHING_WENT_WRONG",
        ErrorClass::Transient,
        "Google bao loi chung, thuong tam thoi. Thu lai sau it phut.",
    ),
    (
        "PUBLIC_ERROR_VIDEO_GENERATION_TIMED_OUT",
        ErrorClass::Transient,
        "Google render qua lau roi bo. Tao lai canh nay.",
    ),
    (
        "LỌC NỘI DUNG",
        ErrorClass::Hard,
        "Google tao video xong roi XOA vi hau kiem (nguoi noi tieng/tre em/anh trung-mirror, mau me).",
    ),
    (
        "HTTP 400",
        ErrorClass::Hard,
        "Du lieu gui len bi Google tu choi.",
    ),
    (
        "HTTP 401",
        ErrorClass::Auth,
        "Phien dang nhap het han — nap lai cookie roi chay tiep.",
    ),
    (
        "HTTP 403",
        ErrorClass::Transient,
        "Google tu choi quyen — thuong do token reCAPTCHA bi choi. Nghi mot lat roi thu lai.",
    ),
    (
        "HTTP 404",
        ErrorClass::Hard,
        "Khong tim thay (project/endpoint) — kiem tra lai cau hinh.",
    ),
    (
        "HTTP 429",
        ErrorClass::Transient,
        "Ban qua nhanh. Giam luong va cho mot lat.",
    ),
    (
        "HTTP 500",
        ErrorClass::Transient,
        "Google loi phia may chu. Cho roi thu lai.",
    ),
    (
        "HTTP 502",
        ErrorClass::Transient,
        "Google loi phia may chu. Cho roi thu lai.",
    ),
    (
        "HTTP 503",
        ErrorClass::Transient,
        "Google tam qua tai. Cho roi thu lai.",
    ),
    (
        "HTTP 504",
        ErrorClass::Transient,
        "Google khong tra loi kip. Thu lai.",
    ),
    (
        "GRPC_3",
        ErrorClass::Hard,
        "Yeu cau bi tu choi (INVALID_ARGUMENT) — sai model/tham so, hoac token da dung roi.",
    ),
    (
        "GRPC_5",
        ErrorClass::Hard,
        "Google khong tim thay ket qua (NOT_FOUND) — thuong la video da bi xoa sau hau kiem.",
    ),
    (
        "GRPC_7",
        ErrorClass::Transient,
        "Google choi quyen tam (PERMISSION_DENIED) — nghi roi thu lai.",
    ),
    (
        "GRPC_8",
        ErrorClass::Transient,
        "Google siet nhip (RESOURCE_EXHAUSTED) — cho lau, giam luong, xoay tai khoan.",
    ),
    (
        "GRPC_13",
        ErrorClass::Transient,
        "Google loi noi bo (INTERNAL) — thu lai.",
    ),
    (
        "GRPC_16",
        ErrorClass::Auth,
        "Google doi dang nhap lai (UNAUTHENTICATED) — nap lai cookie.",
    ),
];

/// Khop theo MANH TEN (substring) cho ma CHUA TUNG GAP. Xep HEP -> RONG, lay cai dau tien.
const FAMILY: &[Entry] = &[
    (
        "TOO_MUCH_TRAFFIC",
        ErrorClass::Transient,
        "Ban qua nhieu yeu cau cung luc — giam luong.",
    ),
    (
        "HIGH_TRAFFIC",
        ErrorClass::Transient,
        "Google dang qua tai — cho vai phut roi thu lai.",
    ),
    (
        "UNUSUAL_ACTIVITY",
        ErrorClass::Transient,
        "Google chan vi token reCAPTCHA thieu/sai/het han.",
    ),
    (
        "DAILY_QUOTA",
        ErrorClass::Hard,
        "Het han muc NGAY — doi tai khoan.",
    ),
    (
        "PROMINENT_PEOPLE",
        ErrorClass::Hard,
        "Dinh nguoi noi tieng — doi anh tham chieu.",
    ),
    (
        "MINOR",
        ErrorClass::Hard,
        "Dinh tre vi thanh nien — sua loi ta hoac doi anh tham chieu.",
    ),
    (
        "SEXUAL",
        ErrorClass::Hard,
        "Noi dung tinh duc — sua loi ta.",
    ),
    (
        "VIOLENCE",
        ErrorClass::Hard,
        "Noi dung bao luc — sua loi ta.",
    ),
    ("HATE", ErrorClass::Hard, "Ngon tu thu ghet — sua loi ta."),
    (
        "NON_ENGLISH",
        ErrorClass::Hard,
        "Google doi prompt tieng Anh.",
    ),
    (
        "UNSAFE",
        ErrorClass::Hard,
        "Vi pham chinh sach noi dung — thu lai vo ich.",
    ),
    (
        "AUDIO",
        ErrorClass::Transient,
        "Phan tieng bi loc — video van ra nhung mat tieng.",
    ),
    (
        "RECAPTCHA",
        ErrorClass::Transient,
        "Token reCAPTCHA bi choi — sinh token moi roi thu lai.",
    ),
    (
        "FILTER",
        ErrorClass::Hard,
        "Google loc noi dung — thu lai vo ich.",
    ),
    (
        "MODEL_ACCESS",
        ErrorClass::Hard,
        "Tai khoan khong co quyen model nay.",
    ),
    (
        "QUOTA",
        ErrorClass::Hard,
        "Het luot cua tai khoan — doi tai khoan hoac cho reset.",
    ),
    (
        "LIMIT",
        ErrorClass::Transient,
        "Cham gioi han nhip — cho mot lat roi thu lai.",
    ),
    (
        "EXPIRED",
        ErrorClass::Auth,
        "Phien/token het han — nap lai cookie.",
    ),
    (
        "UNAUTH",
        ErrorClass::Auth,
        "Phien dang nhap hong — nap lai cookie.",
    ),
    (
        "PERMISSION",
        ErrorClass::Auth,
        "Tai khoan bi tu choi quyen — kiem tra cookie/tai khoan.",
    ),
    (
        "TIMED_OUT",
        ErrorClass::Transient,
        "Google render qua lau roi bo — tao lai.",
    ),
    ("TIMEOUT", ErrorClass::Transient, "Qua han cho — thu lai."),
];

/// Ma loi hay xay ra do PROMPT/ANH — xoay tai khoan khac vo ich, phai sua noi dung.
const PROMPT_FAULT: &[&str] = &[
    "UNSAFE",
    "MINOR",
    "SEXUAL",
    "VIOLENCE",
    "HATE",
    "NON_ENGLISH",
    "PROMINENT_PEOPLE",
    "DANGER",
    "FILTER",
    "LỌC NỘI DUNG",
    "INVALID_ARGUMENT",
    "GRPC_3",
];

static CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"PUBLIC_ERROR_[A-Z0-9_]+").unwrap());
static HTTP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)HTTP[\s:]*([45]\d{2})\b|\bStatus[\s:=]*([45]\d{2})\b").unwrap()
});
static GRPC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""wrb\.fr","[^"]*",null,null,null,\[(\d+)[,\]]"#).unwrap());

/// Rut ma loi tu body/message bat ky. Uu tien `PUBLIC_ERROR_*` -> nhan tool -> HTTP -> gRPC.
#[must_use]
pub fn find_code(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    if let Some(m) = CODE_RE.find(text) {
        return Some(m.as_str().to_string());
    }
    if text.contains("LỌC NỘI DUNG") {
        return Some("LỌC NỘI DUNG".to_string());
    }
    if let Some(caps) = HTTP_RE.captures(text) {
        let code = caps.get(1).or_else(|| caps.get(2))?;
        return Some(format!("HTTP {}", code.as_str()));
    }
    if let Some(caps) = GRPC_RE.captures(text) {
        return Some(format!("GRPC_{}", &caps[1]));
    }
    None
}

/// `(ma, phan loai, cau cho nguoi dung)` — `None` khi khong nhan ra MA nao.
#[must_use]
pub fn explain(text: &str) -> Option<(String, ErrorClass, String)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let key = if EXACT.iter().any(|(code, _, _)| *code == trimmed) {
        trimmed.to_string()
    } else {
        find_code(trimmed)?
    };
    if let Some((code, class, message)) = EXACT.iter().find(|(code, _, _)| *code == key) {
        return Some(((*code).to_string(), *class, (*message).to_string()));
    }
    let upper = key.to_uppercase();
    FAMILY
        .iter()
        .find(|(needle, _, _)| upper.contains(needle))
        .map(|(_, class, message)| (key.clone(), *class, (*message).to_string()))
}

/// Mot dong cho nhat ky/o loi: `"MA — viec can lam"`. Khong nhan ra thi giu nguyen text goc.
#[must_use]
pub fn describe(text: &str, fallback: &str) -> String {
    if let Some((code, _, message)) = explain(text) {
        format!("{code} — {message}")
    } else {
        let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            fallback.to_string()
        } else {
            collapsed
        }
    }
}

/// `Hard`/`Transient`/`Auth` — hoac `None` khi khong ro.
#[must_use]
pub fn classify(text: &str) -> Option<ErrorClass> {
    explain(text).map(|(_, class, _)| class)
}

/// `true` khi thu lai Y HET chac chan hong (het han muc ngay, vi pham noi dung, ...).
#[must_use]
pub fn is_retry_pointless(text: &str) -> bool {
    classify(text) == Some(ErrorClass::Hard)
}

/// `true` khi loi nam o PROMPT/ANH — xoay tai khoan vo ich, phai sua noi dung.
#[must_use]
pub fn is_prompt_fault(text: &str) -> bool {
    let Some((code, class, _)) = explain(text) else {
        return false;
    };
    if class != ErrorClass::Hard {
        return false;
    }
    let upper = code.to_uppercase();
    PROMPT_FAULT.iter().any(|needle| upper.contains(needle))
}

/// Loi da phan loai cho 1 lan goi rpc that bai — dung o `client`/`rpc::*`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct VeoError {
    pub code: Option<String>,
    pub class: Option<ErrorClass>,
    pub message: String,
}

impl VeoError {
    /// Phan loai `text` (body loi hoac message bat ky) thanh `VeoError`.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        match explain(text) {
            Some((code, class, message)) => Self {
                code: Some(code),
                class: Some(class),
                message,
            },
            None => Self {
                code: None,
                class: None,
                message: text.trim().to_string(),
            },
        }
    }

    #[must_use]
    pub fn is_retry_pointless(&self) -> bool {
        self.class == Some(ErrorClass::Hard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nhan_dung_ma_chinh_xac() {
        let (code, class, _) = explain("PUBLIC_ERROR_USER_QUOTA_REACHED").unwrap();
        assert_eq!(code, "PUBLIC_ERROR_USER_QUOTA_REACHED");
        assert_eq!(class, ErrorClass::Hard);
    }

    #[test]
    fn nhan_dung_ma_lan_trong_body_that() {
        let body =
            r#"{"error":{"code":403,"details":[{"reason":"PUBLIC_ERROR_UNUSUAL_ACTIVITY"}]}}"#;
        assert_eq!(
            find_code(body),
            Some("PUBLIC_ERROR_UNUSUAL_ACTIVITY".to_string())
        );
        assert_eq!(classify(body), Some(ErrorClass::Transient));
    }

    #[test]
    fn ma_moi_chua_biet_khop_theo_ho() {
        // Google them mot ma moi hoan toan nhung con MODEL_ACCESS trong ten -> van ra Hard.
        let (code, class, _) = explain("PUBLIC_ERROR_SOME_NEW_MODEL_ACCESS_VARIANT").unwrap();
        assert_eq!(code, "PUBLIC_ERROR_SOME_NEW_MODEL_ACCESS_VARIANT");
        assert_eq!(class, ErrorClass::Hard);
    }

    #[test]
    fn http_status_tran_duoc_nhan_dien() {
        assert_eq!(
            find_code("Loi HTTP 429 tu server"),
            Some("HTTP 429".to_string())
        );
        assert_eq!(
            classify("Loi HTTP 429 tu server"),
            Some(ErrorClass::Transient)
        );
    }

    #[test]
    fn grpc_tran_trong_khung_wrb_fr() {
        let raw = r#"["wrb.fr","YhhmEf",null,null,null,[8],"generic"]"#;
        assert_eq!(find_code(raw), Some("GRPC_8".to_string()));
        assert_eq!(classify(raw), Some(ErrorClass::Transient));
    }

    #[test]
    fn khong_nhan_dien_thi_tra_none() {
        assert_eq!(explain("mot loi la hoac khong ro"), None);
        assert!(!is_retry_pointless("mot loi la hoac khong ro"));
    }

    #[test]
    fn is_prompt_fault_dung_cho_loi_noi_dung_khong_dung_cho_loi_tai_khoan() {
        assert!(is_prompt_fault("PUBLIC_ERROR_UNSAFE_GENERATION"));
        assert!(!is_prompt_fault("PUBLIC_ERROR_USER_QUOTA_REACHED"));
        assert!(!is_prompt_fault("PUBLIC_ERROR_UNUSUAL_ACTIVITY")); // Transient, khong phai Hard
    }

    #[test]
    fn describe_ghep_ma_va_cau_giai_thich() {
        let s = describe("PUBLIC_ERROR_MINOR", "?");
        assert!(s.starts_with("PUBLIC_ERROR_MINOR — "));
    }

    #[test]
    fn describe_fallback_khi_khong_nhan_ra() {
        assert_eq!(describe("", "khong ro loi"), "khong ro loi");
    }

    #[test]
    fn veo_error_from_text() {
        let err = VeoError::from_text("PUBLIC_ERROR_USER_QUOTA_REACHED");
        assert!(err.is_retry_pointless());
        assert_eq!(err.code.as_deref(), Some("PUBLIC_ERROR_USER_QUOTA_REACHED"));
    }
}
