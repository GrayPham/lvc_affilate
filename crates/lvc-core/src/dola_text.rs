//! Phan loai chu Dola tra ve (da ngon ngu): het han muc / tu choi chinh sach / het credit /
//! agent xin xac nhan hoac hoi lai. Thuan — khong I/O. Nguon: repo Python `lvcseedance`
//! `src/utils/modes.py` (dong 28-31, 57-139, 161-170, 288-358) + `src/video/video_worker_ui.py`
//! (dong 62-95) + `src/api/dola_client.py` (dong 39-45).
//!
//! `regex` crate khong ho tro lookaround nen pattern "generating" (co negative lookbehind
//! `(?<!cannot )...`, modes.py:129) dung `fancy-regex` rieng; cac pattern con lai dung `regex`.

use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use serde::{Deserialize, Serialize};

// ===== Nguon pattern (chep tung ky tu tu Python, xem doc comment moi hang) =====

/// modes.py:57-61 `DOLA_TEXT["daily_limit"]`. Luon (?i) — `video_worker_ui.py:60` compile voi
/// `re.IGNORECASE`.
const DAILY_LIMIT_SRC: &str = concat!(
    r"動画生成の\s*1日あたりの上限|每日(?:视频|影片)?生成.*(?:上限|限额|额度)|",
    r"daily.*(?:limit|quota)|(?:limit|quota).*per\s+day|",
    r"đạt\s*giới\s*hạn\s*tạo\s*video|giới\s*hạn\s*tạo\s*video\s*hằng\s*ngày|",
    r"thử\s*lại\s*vào\s*ngày\s*mai|hết\s*lượt\s*(?:tạo|hôm\s*nay)|",
    r"đạt\s*giới\s*hạn\s*hằng\s*ngày"
);

/// dola_client.py:39-45 `CREDIT_FAIL_PATTERN`.
const CREDIT_FAIL_BASE_SRC: &str = concat!(
    r"無法生成|无法生成|不能生成|无法完成|無法完成|",
    r"余额不足|餘額不足|额度不足|額度不足|额度耗尽|額度耗盡|",
    r"生成できません|残高不足|",
    r"生成할 수 없|한도.*부족|부족.*한도|",
    r"insufficient|unable to (generate|create)|failed to (generate|create)"
);

/// modes.py:71-75 `DOLA_TEXT["credit_fail"]`.
const CREDIT_FAIL_MODES_SRC: &str = concat!(
    r"クレジットが使用されます|本日は残り\d+のみ|クレジットが(?:不足|足りません)|",
    r"パラメーターを変更してもう一度|not enough credits?|insufficient credits?|",
    r"không\s*đủ\s*(?:tín\s*dụng|điểm|credit)|",
    r"hết\s*(?:tín\s*dụng|điểm|credit)|thiếu\s*(?:tín\s*dụng|điểm|credit)"
);

/// modes.py:69 `DOLA_TEXT["credit_fail_extra"]` — noi vao trong `video_worker_ui.py:63`.
const CREDIT_FAIL_EXTRA: [&str; 2] = ["生成できません", "残高不足"];

/// video_worker_ui.py:69-71 `GENERIC_REFUSAL_RE`.
const GENERIC_REFUSAL_SRC: &str = concat!(
    r"temporarily unable to generate|i(?:'m| am) (?:currently )?unable to generate the (?:content|video)|",
    r"unable to generate the content you requested|please try entering other requirements"
);

/// video_worker_ui.py:72-73 `EXPLICIT_CREDIT_RE`.
const EXPLICIT_CREDIT_SRC: &str = r"クレジット|credit|残高|余额|餘額|额度|額度|积分|한도|tín\s*dụng|điểm|points?\b|残り\s*\d+|上限";

/// `video_worker_ui.py:76` `PORTRAIT_RE`.
const PORTRAIT_SRC: &str = concat!(
    r"肖像保護|肖像権|あなた自身が写っている|portrait (?:protection|rights)|",
    r"videos? (?:of|showing|featuring) yourself|chân\s*dung"
);

/// modes.py:109-126 `DOLA_TEXT["policy_reject"]`.
const POLICY_REJECT_SRC: &str = concat!(
    r"安全ポリシー|ポリシーの制限|コンテンツポリシー|ポリシーに違反|違反する可能性|利用規約に(?:違反|抵触)|ガイドラインに(?:違反|抵触)|",
    r"content policy|safety policy|usage polic|violat(?:es|ion|ing)|against (?:our|the) polic|",
    r"chính\s*sách\s*an\s*toàn|chính\s*sách\s*nội\s*dung|vi\s*phạm\s*chính\s*sách|",
    r"điều\s*khoản\s*dịch\s*vụ|hướng\s*dẫn\s*cộng\s*đồng|",
    r"i(?:'m| am) sorry,? (?:but )?i (?:cannot|can't|can not|won't|will not|am unable to|am not able to) (?:generate|create|produce|make|help)|",
    r"i (?:cannot|can't|can not) (?:generate|create|produce) (?:this|that|the|such)|",
    r"unable to (?:generate|create) this (?:video|content)|",
    r"objectif(?:ies|y|ying)|sexualiz(?:es|ed|ing)|inappropriate (?:and|or) disrespectful|explicit sexual|",
    r"nudity|pornograph|not (?:allowed|permitted) to (?:generate|create)|",
    r"申し訳(?:ありません|ございません).{0,60}(?:生成|作成)(?:できません|することはできません|いたしかねます)|",
    r"生成をお断り|お手伝い(?:できません|いたしかねます)|不適切な(?:内容|コンテンツ)|",
    r"肖像保護|肖像権|あなた自身が写っている|portrait (?:protection|rights)|videos? (?:of|showing|featuring) yourself|",
    r"bảo\s*vệ\s*chân\s*dung|quyền\s*chân\s*dung|",
    r"không\s*thể\s*tạo\s*(?:video|nội\s*dung)\s*này|nội\s*dung\s*không\s*phù\s*hợp|xin\s*lỗi.{0,40}không\s*thể\s*tạo"
);

/// modes.py:128-132 `DOLA_TEXT["generating"]` — co negative lookbehind (modes.py:129)
/// nen phai dung `fancy-regex` (crate `regex` khong ho tro lookaround).
const GENERATING_SRC: &str = concat!(
    r"生成しています|生成中|作成しています|生成を開始|完成します|お知らせします|クレジットを使用|",
    r"(?<!cannot )(?<!can't )(?<!not )(?<!unable to )(?<!won't )generat(?:ing|e)\b.{0,80}?\b(?:video|now)|",
    r"is being generated|being processed|processing your|",
    r"will (?:be|take) (?:ready|completed|about|approximately|\d)|i'll (?:generate|create|make)|creating (?:the |your )?video|",
    r"đang\s*tạo|sẽ\s*hoàn\s*thành|đang\s*xử\s*lý"
);

/// modes.py:80-85 `DOLA_TEXT["confirm_request"]`.
const CONFIRM_REQUEST_SRC: &str = concat!(
    r"ご確認ください|確認のお返事|お返事をいただければ|生成を開始します|",
    r"「確認」|「生成開始」|とお答えください|お返事ください|よろしければ|",
    r"wait(?:ing)? for (?:your )?confirmation|please confirm|",
    r"shall I proceed|can I proceed|should I proceed|proceed with|",
    r"would you like (?:me )?to proceed|do you want (?:me )?to proceed|",
    r"xác nhận|tiếp tục|bạn có muốn|tiến hành|đồng ý|bắt đầu tạo"
);

/// modes.py:89-96 `DOLA_TEXT["choice_request"]`.
const CHOICE_REQUEST_SRC: &str = concat!(
    r"missing\s+parameters?|required\s+to\s+generate|please\s+provide|provide\s+them\s+in\s+one\s+of\s+these\s+ways|",
    r"choose\s+automatically|if\s+you\s+want\s+me\s+to\s+choose|i\s+suggest|as\s+suggested|",
    r"tự\s*động\s*chọn|chọn\s*tự\s*động|đề\s*xuất|gợi\s*ý|phương\s*án|",
    r"bạn\s*chọn|vui\s*lòng\s*cung\s*cấp|cung\s*cấp\s*thêm|cần\s*thêm\s*thông\s*số|",
    r"[aA][.．\)]\s+.*[bB][.．\)]|option\s+[a-c]|lựa\s*chọn|",
    r"パラメータ|不足|自動的"
);

/// modes.py:134 `DOLA_TEXT["question"]`. Dung 2 luot khac flag: `needs_confirmation`
/// (modes.py:301) goi voi `re.IGNORECASE`, con `agent_question` (modes.py:358) goi KHONG
/// co flag (case-sensitive) — xem hang `QUESTION`/`AGENT_QUESTION` ben duoi.
const QUESTION_SRC: &str = r"[？?]\s*$|[？?]\s*(?:A|Ａ)[.．]|(?:^|\s)A[.．]\s*\S+\s+B[.．]";

/// modes.py:137-139 `DOLA_TEXT["duration_request"]`.
const DURATION_REQUEST_SRC: &str = concat!(
    r"秒数を指定|\d+\s*秒\s*から\s*\d+\s*秒\s*まで|specify (?:the )?(?:number of )?seconds|how many seconds|",
    r"(?:choose|specify|set|provide) (?:a |the )?(?:video )?(?:duration|length)|",
    r"chỉ\s*định\s*(?:số\s*)?giây|(?:thời\s*lượng|độ\s*dài).{0,20}(?:chỉ\s*định|chọn)"
);

/// modes.py:87 `DOLA_TEXT["confirm_reply"]` — cau tra loi mac dinh khi xin xac nhan.
const CONFIRM_REPLY: &str = "Yes, please proceed. Có, hãy tiếp tục tạo video. Đúng rồi, hãy tạo video theo các thông số này. はい、確認しました。生成開始してください。";

/// modes.py:101-104 `DOLA_TEXT["choice_reply_generic"]`. `choice_reply_auto` (modes.py:97-100)
/// khong dung duoc voi chu ky `confirmation_reply(kind, duration)` (khong co van ban goc de
/// biet la truong hop "C. tu dong") nen bo qua, xem report.
const CHOICE_REPLY_GENERIC: &str = "Hãy tự động chọn phương án tối ưu nhất và tiến hành tạo video ngay giúp tôi. Please choose automatically with the best settings and proceed to generate the video now.";

/// video_worker_ui.py:79-81 `REFUSAL_NOTE` — tien to loi `PromptViolationError` khi tu choi
/// (khong phai het credit).
pub const REFUSAL_NOTE: &str = "Dola từ chối tạo video (KHÔNG phải hết credit — cùng tài khoản vẫn tạo được cảnh khác). Thường do ảnh tham chiếu (mặt người thật, trẻ em, nhân vật bản quyền) hoặc nội dung prompt → đổi ảnh/prompt rồi Chạy lại (tự mở chat mới). Dola nói: ";

// ===== Regex bien tinh (compile 1 lan) =====

static DAILY_LIMIT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){DAILY_LIMIT_SRC}")).expect("DAILY_LIMIT_PATTERN hop le")
});

/// video_worker_ui.py:62-64 `CREDIT_FAIL_RE` = `CREDIT_FAIL_PATTERN.pattern` | `credit_fail` |
/// cac tu trong `credit_fail_extra` (escape).
static CREDIT_FAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    let joined = format!(
        "(?i){CREDIT_FAIL_BASE_SRC}|{CREDIT_FAIL_MODES_SRC}|{}|{}",
        regex::escape(CREDIT_FAIL_EXTRA[0]),
        regex::escape(CREDIT_FAIL_EXTRA[1]),
    );
    Regex::new(&joined).expect("CREDIT_FAIL_RE hop le")
});

static GENERIC_REFUSAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){GENERIC_REFUSAL_SRC}")).expect("GENERIC_REFUSAL_RE hop le")
});

static EXPLICIT_CREDIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){EXPLICIT_CREDIT_SRC}")).expect("EXPLICIT_CREDIT_RE hop le")
});

static PORTRAIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("(?i){PORTRAIT_SRC}")).expect("PORTRAIT_RE hop le"));

static POLICY_REJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){POLICY_REJECT_SRC}")).expect("POLICY_REJECT hop le")
});

static GENERATING: LazyLock<FancyRegex> =
    LazyLock::new(|| FancyRegex::new(&format!("(?i){GENERATING_SRC}")).expect("GENERATING hop le"));

static CONFIRM_REQUEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){CONFIRM_REQUEST_SRC}")).expect("CONFIRM_REQUEST hop le")
});

static CHOICE_REQUEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){CHOICE_REQUEST_SRC}")).expect("CHOICE_REQUEST hop le")
});

/// modes.py:301 `needs_confirmation` goi `re.search(..., re.IGNORECASE)`.
static QUESTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("(?i){QUESTION_SRC}")).expect("QUESTION hop le"));

/// modes.py:358 `agent_question` goi `re.search(...)` KHONG co `re.IGNORECASE`.
static AGENT_QUESTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(QUESTION_SRC).expect("AGENT_QUESTION hop le"));

static DURATION_REQUEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){DURATION_REQUEST_SRC}")).expect("DURATION_REQUEST hop le")
});

// ===== API cong khai =====

/// Nhan phan loai cua `classify_dola_text` — video_worker_ui.py:84-95.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DolaTextClass {
    /// Het han muc video/ngay.
    Limit,
    /// Tu choi noi dung chung (chinh sach, chan dung...), KHONG phai het credit.
    Refusal,
    /// Het credit/diem.
    Credit,
}

/// `s` da duoc trim; true neu khop `POLICY_REJECT` (modes.py:338 `policy_rejection`).
fn is_policy_rejection(s: &str) -> bool {
    !s.is_empty() && POLICY_REJECT.is_match(s)
}

/// 'limit' (het luot ngay) | 'refusal' (tu choi noi dung chung) | 'credit' (het credit) | None.
/// Thu tu kiem tra dung y video_worker_ui.py:84-95: limit truoc, roi (refusal|policy) va
/// KHONG explicit-credit -> credit.
#[must_use]
pub fn classify_dola_text(text: &str) -> Option<DolaTextClass> {
    if DAILY_LIMIT_PATTERN.is_match(text) {
        return Some(DolaTextClass::Limit);
    }
    let refusal_hit = GENERIC_REFUSAL_RE.is_match(text) || is_policy_rejection(text.trim());
    if refusal_hit && !EXPLICIT_CREDIT_RE.is_match(text) {
        return Some(DolaTextClass::Refusal);
    }
    if CREDIT_FAIL_RE.is_match(text) {
        return Some(DolaTextClass::Credit);
    }
    None
}

/// modes.py:28-31 `looks_like_json` — chuoi metadata JSON (vd `{"limit_type":"PerDay",...}`),
/// khong phai cau Dola noi voi nguoi dung.
#[must_use]
pub fn looks_like_json(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('{') || t.starts_with('[') || t.contains("\"limit_type\"")
}

/// `PORTRAIT_RE` `video_worker_ui.py:76` — Dola chan anh tham chieu vi "bao ve chan dung".
#[must_use]
pub fn is_portrait_hint(text: &str) -> bool {
    PORTRAIT_RE.is_match(text)
}

/// `DOLA_TEXT["generating"]` modes.py:128-132 — dau hieu agent DANG tao (chua xong), tiep
/// tuc doi thay vi coi la "tra loi xong ma khong co video". `fancy_regex::Error` chi xay ra
/// khi timeout backtracking (khong gap voi pattern/do dai text o day) -> an toan `unwrap_or(false)`.
#[must_use]
pub fn is_generating(text: &str) -> bool {
    GENERATING.is_match(text).unwrap_or(false)
}

/// modes.py:333-340 `policy_rejection` — khoi chu dau tien khop tu choi chinh sach; tra ve
/// toi da 300 KY TU (khop `s[:300]` cua Python, khong phai byte).
#[must_use]
pub fn policy_rejection(latest: &[String]) -> Option<String> {
    for t in latest {
        let s = t.trim();
        if is_policy_rejection(s) {
            return Some(s.chars().take(300).collect());
        }
    }
    None
}

/// Ly do can xac nhan/hoi lai — tuong ung 4 pattern modes.py kiem trong `needs_confirmation`
/// (modes.py:288-305), theo dung thu tu uu tien (Confirm truoc, Duration sau cung).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmKind {
    /// `confirm_request` — xin xac nhan (BUG-002).
    Confirm,
    /// `choice_request` — hoi tham so / de xuat lua chon (TUT Dola Agent).
    Choice,
    /// `question` — cau hoi ket thuc bang dau hoi hoac dang A/B.
    Question,
    /// `duration_request` — hoi so giay (vd "秒数を指定してください").
    Duration,
}

/// modes.py:288-305 `needs_confirmation` — tin MOI NHAT cua hoi thoai (cac khoi da noi bang
/// " ") co phai agent xin xac nhan / hoi lua chon / hoi / hoi so giay khong. `None` khi
/// `latest` rong hoac khong khop pattern nao.
#[must_use]
pub fn needs_confirmation(latest: &[String]) -> Option<ConfirmKind> {
    if latest.is_empty() {
        return None;
    }
    let combined = latest.join(" ");
    if CONFIRM_REQUEST.is_match(&combined) {
        return Some(ConfirmKind::Confirm);
    }
    if CHOICE_REQUEST.is_match(&combined) {
        return Some(ConfirmKind::Choice);
    }
    if QUESTION.is_match(&combined) {
        return Some(ConfirmKind::Question);
    }
    if DURATION_REQUEST.is_match(&combined) {
        return Some(ConfirmKind::Duration);
    }
    None
}

/// modes.py:308-322 `get_confirmation_reply` + 161-170 `duration_reply` (chi phan dinh dang
/// truc tiep — xem report ve pham vi kep 4-15s bi bo vi chu ky nay khong nhan van ban goc).
/// `Confirm`/`Question` -> cau tra loi mac dinh (modes.py:322); `Choice` -> cau tra loi chung
/// (modes.py:104, bo nhanh "C. tu dong" vi khong co van ban goc de phan biet).
#[must_use]
pub fn confirmation_reply(kind: ConfirmKind, duration: u16) -> String {
    match kind {
        ConfirmKind::Duration => {
            format!("{duration}秒 ({duration} seconds). {duration}秒の動画を生成してください。")
        }
        ConfirmKind::Choice => CHOICE_REPLY_GENERIC.to_string(),
        ConfirmKind::Confirm | ConfirmKind::Question => CONFIRM_REPLY.to_string(),
    }
}

/// modes.py:350-358 `agent_question` — tin moi nhat la cau hoi cho nguoi dung -> tra cau hoi;
/// cau xin xac nhan (BUG-002) tra None.
///
/// KHAC ban Python mot diem (xem report task-1): ban Python goi nguyen `needs_confirmation`
/// (ca nhanh Question) lam cong, nhung vi `needs_confirmation` tu no cung kiem dung pattern
/// "question" nen ham Python nay luon tra "" voi MOI input (da kiem chung bang Python that:
/// `agent_question(["どの比率にしますか？"]) == ''`) — logic chet, trai voi docstring va vi
/// du BUG-005 cua chinh no. Ban Rust chi loai khi khop Confirm/Choice/Duration (nhung dau
/// hieu "xin xac nhan" THAT SU khac), khong loai khi chi khop Question.
#[must_use]
pub fn agent_question(latest: &[String]) -> Option<String> {
    let last = latest.last()?;
    match needs_confirmation(latest) {
        Some(ConfirmKind::Confirm | ConfirmKind::Choice | ConfirmKind::Duration) => return None,
        Some(ConfirmKind::Question) | None => {}
    }
    let last = last.trim();
    if AGENT_QUESTION.is_match(last) {
        Some(last.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_nhat() {
        assert_eq!(
            classify_dola_text("動画生成の1日あたりの上限に達しました"),
            Some(DolaTextClass::Limit)
        );
    }

    #[test]
    fn refusal_khong_phai_credit() {
        assert_eq!(
            classify_dola_text("このリクエストはポリシーに違反しています"),
            Some(DolaTextClass::Refusal)
        );
    }

    #[test]
    fn credit_thang_refusal_khi_co_explicit_credit() {
        // chuoi khop ca GENERIC_REFUSAL_RE ("temporarily unable to generate") lan
        // EXPLICIT_CREDIT_RE ("credit") -> explicit credit thang, roi CREDIT_FAIL_RE khop "insufficient"
        assert_eq!(
            classify_dola_text(
                "I am temporarily unable to generate the content you requested due to insufficient credit balance."
            ),
            Some(DolaTextClass::Credit)
        );
    }

    #[test]
    fn credit_thuan() {
        assert_eq!(
            classify_dola_text("残高不足のため生成できません"),
            Some(DolaTextClass::Credit)
        );
    }

    #[test]
    fn chu_thuong_khong_khop() {
        assert_eq!(classify_dola_text("猫が歩く動画を作ります"), None);
    }

    #[test]
    fn json_bi_bo_qua() {
        assert!(looks_like_json("{\"a\":1}"));
        assert!(looks_like_json("[1,2,3]"));
        assert!(looks_like_json("  prefix \"limit_type\":\"PerDay\""));
        assert!(!looks_like_json("hello"));
    }

    #[test]
    fn confirm_duration_tra_loi_dung_dinh_dang() {
        assert_eq!(
            confirmation_reply(ConfirmKind::Duration, 15),
            "15秒 (15 seconds). 15秒の動画を生成してください。"
        );
    }

    #[test]
    fn confirm_mac_dinh_tra_loi_yes_proceed() {
        let expected = "Yes, please proceed. Có, hãy tiếp tục tạo video. Đúng rồi, hãy tạo video theo các thông số này. はい、確認しました。生成開始してください。";
        assert_eq!(confirmation_reply(ConfirmKind::Confirm, 10), expected);
        assert_eq!(confirmation_reply(ConfirmKind::Question, 10), expected);
    }

    #[test]
    fn choice_tra_loi_generic() {
        assert_eq!(
            confirmation_reply(ConfirmKind::Choice, 10),
            "Hãy tự động chọn phương án tối ưu nhất và tiến hành tạo video ngay giúp tôi. Please choose automatically with the best settings and proceed to generate the video now."
        );
    }

    #[test]
    fn agent_question_dau_hoi_cuoi() {
        // BUG-005: dua ngay vao needs_confirmation()'s own "question" check se lam ham nay
        // luon tra None (da kiem tra bang Python that: needs_confirmation cung khop "question"
        // -> agent_question() luon tra "" — day la loi tiem an cua ban Python, xem report).
        // Ban Rust sua: chi loai khi khop Confirm/Choice/Duration, KHONG loai khi chi khop Question.
        assert!(agent_question(&["どの比率にしますか？".to_string()]).is_some());
    }

    #[test]
    fn agent_question_abc_la_choice_khong_phai_question() {
        // Khop CHOICE_REQUEST (mau "A. ... B.") -> needs_confirmation tra Choice -> agent_question None
        assert_eq!(agent_question(&["A. 猫  B. 犬".to_string()]), None);
    }

    #[test]
    fn agent_question_rong_thi_none() {
        assert_eq!(agent_question(&[]), None);
    }

    #[test]
    fn generating_nhat() {
        assert!(is_generating("動画を生成しています"));
        assert!(is_generating("Creating the video now, please wait"));
    }

    #[test]
    fn generating_lookbehind_loai_tu_choi() {
        // modes.py:129: (?<!cannot ) ... — "cannot generate" KHONG duoc coi la dang tao
        assert!(!is_generating("I cannot generate the video now"));
    }

    #[test]
    fn portrait_hint() {
        assert!(is_portrait_hint(
            "肖像保護のため、Dreamina Seedance 2.5で生成できるのはあなた自身が写っている動画のみです。"
        ));
        assert!(!is_portrait_hint("普通のテキストです"));
    }

    #[test]
    fn policy_rejection_tra_chuoi_khop() {
        let latest = vec![
            "前の発言".to_string(),
            "安全ポリシーの制限により動画生成ができませんでした。".to_string(),
        ];
        assert_eq!(
            policy_rejection(&latest),
            Some("安全ポリシーの制限により動画生成ができませんでした。".to_string())
        );
        assert_eq!(policy_rejection(&["ただの動画です".to_string()]), None);
    }

    #[test]
    fn needs_confirmation_bon_loai_va_uu_tien() {
        assert_eq!(
            needs_confirmation(&["ご確認ください".to_string()]),
            Some(ConfirmKind::Confirm)
        );
        assert_eq!(
            needs_confirmation(&["missing parameters, please provide them".to_string()]),
            Some(ConfirmKind::Choice)
        );
        assert_eq!(
            needs_confirmation(&["本当ですか?".to_string()]),
            Some(ConfirmKind::Question)
        );
        assert_eq!(
            needs_confirmation(&["秒数を指定してください".to_string()]),
            Some(ConfirmKind::Duration)
        );
        assert_eq!(
            needs_confirmation(&["動画を生成しました".to_string()]),
            None
        );
        assert_eq!(needs_confirmation(&[]), None);
    }

    #[test]
    fn refusal_note_khop_ban_python() {
        assert_eq!(
            REFUSAL_NOTE,
            "Dola từ chối tạo video (KHÔNG phải hết credit — cùng tài khoản vẫn tạo được cảnh khác). \
Thường do ảnh tham chiếu (mặt người thật, trẻ em, nhân vật bản quyền) hoặc nội dung prompt → đổi ảnh/prompt rồi \
Chạy lại (tự mở chat mới). Dola nói: "
        );
    }
}
