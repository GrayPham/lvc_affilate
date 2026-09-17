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

/// modes.py:97-100 `DOLA_TEXT["choice_reply_auto"]` — dung khi phat hien Dola tu goi y
/// phuong an "C. tu dong" (xem `CHOICE_AUTO_HINT_SRC` va `modes.py:318`).
const CHOICE_REPLY_AUTO: &str = "C. Hãy chọn tự động các thông số như bạn đã gợi ý và tiến hành tạo video ngay. Please choose automatically as suggested and proceed to generate the video now.";

/// modes.py:101-104 `DOLA_TEXT["choice_reply_generic"]`.
const CHOICE_REPLY_GENERIC: &str = "Hãy tự động chọn phương án tối ưu nhất và tiến hành tạo video ngay giúp tôi. Please choose automatically with the best settings and proceed to generate the video now.";

/// `modes.py:318` — pattern rieng (khong nam trong `DOLA_TEXT`) ma `get_confirmation_reply`
/// tu dung inline de phan biet "C. tu dong" voi lua chon chung chung.
const CHOICE_AUTO_HINT_SRC: &str = r"choose\s+automatically|c\.\s+if\s+you\s+want\s+me\s+to\s+choose|chọn\s*tự\s*động|tự\s*động\s*chọn|[cC][.．\)]\s+";

/// modes.py:166 `re.search(r"(\d+)\s*秒\s*から\s*(\d+)\s*秒", combined)` — KHONG co `re.IGNORECASE`.
const DURATION_RANGE_JP_SRC: &str = r"(\d+)\s*秒\s*から\s*(\d+)\s*秒";

/// modes.py:166 `re.search(r"(\d+)\s*(?:to|-|–)\s*(\d+)\s*seconds", combined, re.IGNORECASE)`.
const DURATION_RANGE_EN_SRC: &str = r"(\d+)\s*(?:to|-|–)\s*(\d+)\s*seconds";

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

/// modes.py:318 `get_confirmation_reply` goi `re.search(..., re.IGNORECASE)`.
static CHOICE_AUTO_HINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){CHOICE_AUTO_HINT_SRC}")).expect("CHOICE_AUTO_HINT hop le")
});

/// modes.py:166 — khong `re.IGNORECASE` (chi so + 秒/から, khong can khong phan biet hoa/thuong).
static DURATION_RANGE_JP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(DURATION_RANGE_JP_SRC).expect("DURATION_RANGE_JP hop le"));

/// modes.py:166 — co `re.IGNORECASE` ("to"/"seconds").
static DURATION_RANGE_EN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("(?i){DURATION_RANGE_EN_SRC}")).expect("DURATION_RANGE_EN hop le")
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

/// modes.py:161-170 `duration_reply` — cau tra loi khi Dola hoi so giay: thoi luong cua
/// task, kep trong khoang Dola neu (vd "4 秒 から 15 秒") neu doc duoc.
fn duration_reply(latest: &[String], duration: Option<u16>) -> String {
    let combined = latest.join(" ");
    // modes.py:165 `d = int(duration or 10)` — Python: gia tri falsy (None HOAC 0) deu ve 10.
    let mut d: u32 = match duration {
        None | Some(0) => 10,
        Some(n) => u32::from(n),
    };
    let caps = DURATION_RANGE_JP
        .captures(&combined)
        .or_else(|| DURATION_RANGE_EN.captures(&combined));
    if let Some(caps) = caps {
        if let (Ok(lo), Ok(hi)) = (caps[1].parse::<u32>(), caps[2].parse::<u32>()) {
            // modes.py:169 `d = max(lo, min(hi, d))` — chep dung thu tu, khong dung `.clamp()`
            // (clamp doi hoi lo <= hi, se panic neu van ban Dola dao nguoc thu tu so).
            d = lo.max(hi.min(d));
        }
    }
    format!("{d}秒 ({d} seconds). {d}秒の動画を生成してください。")
}

/// modes.py:308-322 `get_confirmation_reply` (dong 161-170 `duration_reply` cho nhanh
/// duration). Nhan nguyen `latest_texts`/`duration` nhu Python (KHAC ban dau cua task 1 dung
/// `ConfirmKind` — fix round 1 RULING B: port day du de giu dung thu tu uu tien cua Python,
/// vi thu tu nay KHAC thu tu cua `needs_confirmation`/`ConfirmKind` — vd mot doan vua khop
/// `confirm_request` vua khop `duration_request` thi `needs_confirmation` xep Confirm nhung
/// `get_confirmation_reply` van tra loi so giay truoc).
///
/// Thu tu dung y modes.py:308-322: `duration_request` -> `duration_reply`; roi pattern rieng
/// "C. tu dong" (modes.py:318, KHONG nam trong `DOLA_TEXT`) -> `choice_reply_auto`; roi
/// `choice_request` -> `choice_reply_generic`; con lai -> `confirm_reply` mac dinh.
#[must_use]
pub fn confirmation_reply(latest: &[String], duration: Option<u16>) -> String {
    let combined = latest.join(" ");
    if DURATION_REQUEST.is_match(&combined) {
        return duration_reply(latest, duration);
    }
    if CHOICE_AUTO_HINT.is_match(&combined) {
        return CHOICE_REPLY_AUTO.to_string();
    }
    if CHOICE_REQUEST.is_match(&combined) {
        return CHOICE_REPLY_GENERIC.to_string();
    }
    CONFIRM_REPLY.to_string()
}

/// modes.py:350-358 `agent_question` — port LITERAL, tung dong: rong -> None; con lai, neu
/// `needs_confirmation(latest)` la `Some` (BAT KY loai nao, KE CA Question) -> None; con lai
/// -> tin cuoi (trim) co khop `AGENT_QUESTION` (case-sensitive, modes.py:358 khong co
/// `re.IGNORECASE`) thi tra tin do, khong thi None.
///
/// KHONG BAO GIO tra `Some` tai HEAD (fix round 1, RULING A). Ly do: `needs_confirmation`
/// (modes.py:288-305) tu no da kiem CHINH pattern "question" (modes.py:301, dong thu 3 trong
/// 4 dieu kien) tren `combined` (noi TAT CA `latest_texts`), con o day chi kiem tin CUOI CUNG
/// (mot phan cua `combined`, hoac giong het neu `latest` chi co 1 phan tu) — nen bat cu input
/// nao khien dong 358 khop thi dong 301 (rong hon) CHAC CHAN cung da khop truoc do, khien
/// nhanh `needs_confirmation(...).is_some()` tra True va ham thoat som voi None. Ket qua: ham
/// nay KHONG THE tra Some cho bat ky input nao — da xac minh bang python that (`.venv`,
/// `A:\ToolLVC\lvcseedance`, chi doc khong sua): `modes.agent_question([...])` tra `''` cho
/// ca 4 cau probe (bao gom ca vi du BUG-005 trong chinh docstring cua no).
///
/// Day la PARITY CO CHU DICH voi Python, khong phai sao chep sai: step 8 `_handle_poll`
/// (`video_worker_ui.py`) goi ham nay SAU khi da qua nhanh `needs_confirmation` (khong tra ve
/// som) va KHONG co guard `is_generating` truoc no — neu ham nay tra `Some` cho bat ky cau
/// bot noi ket thuc bang ？/? (rat pho bien trong cau tieng Nhat) trong luc video con dang
/// tao (`is_generating` true, ~5 phut), no se bi bao `AGENT_QUESTION` sau 60s im lang du agent
/// van dang chay binh thuong -> treo chat / mat luot Dola oan uong. Ban Python "may man"
/// tranh duoc rui ro nay chinh vi no la ham chet; giu nguyen hanh vi do o day. Neu sau nay can
/// phat hien cau hoi that su (khac voi xin xac nhan), phai sua ca `needs_confirmation` (bo
/// nhanh Question rieng, vd them mot `ConfirmKind` moi) trong MOT task rieng co ra soat ky
/// rui ro credit/treo chat, khong sua ngam trong ham nay.
#[must_use]
pub fn agent_question(latest: &[String]) -> Option<String> {
    if latest.is_empty() {
        return None;
    }
    if needs_confirmation(latest).is_some() {
        return None;
    }
    let last = latest[latest.len() - 1].trim();
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
    fn confirmation_reply_duration_dung_gia_tri_da_cho() {
        assert_eq!(
            confirmation_reply(&["秒数を指定してください".to_string()], Some(15)),
            "15秒 (15 seconds). 15秒の動画を生成してください。"
        );
    }

    #[test]
    fn confirmation_reply_duration_none_mac_dinh_10() {
        // modes.py:165 `int(duration or 10)` — None (va ca Some(0), falsy trong Python) -> 10
        assert_eq!(
            confirmation_reply(&["秒数を指定してください".to_string()], None),
            "10秒 (10 seconds). 10秒の動画を生成してください。"
        );
    }

    #[test]
    fn confirmation_reply_duration_kep_theo_khoang_dola_neu() {
        // modes.py:166-169: doc duoc "4 秒 から 15 秒" -> d = max(lo, min(hi, d))
        // = max(4, min(15, 30)) = 15
        let latest = vec![
            "この動画は 4 秒 から 15 秒まで生成できます。秒数を指定してください。".to_string(),
        ];
        assert_eq!(
            confirmation_reply(&latest, Some(30)),
            "15秒 (15 seconds). 15秒の動画を生成してください。"
        );
    }

    #[test]
    fn confirmation_reply_choice_auto_khi_co_goi_y_tu_chon_c() {
        // modes.py:318: pattern rieng "choose automatically|...|[cC][.．\)]\s+" khop truoc
        // -> choice_reply_auto (modes.py:97-100), khong phai choice_reply_generic
        let latest = vec!["C. If you want me to choose automatically, let me know.".to_string()];
        assert_eq!(
            confirmation_reply(&latest, None),
            "C. Hãy chọn tự động các thông số như bạn đã gợi ý và tiến hành tạo video ngay. Please choose automatically as suggested and proceed to generate the video now."
        );
    }

    #[test]
    fn confirmation_reply_choice_generic_khi_khong_khop_auto() {
        let latest = vec!["missing parameters, please provide them".to_string()];
        assert_eq!(
            confirmation_reply(&latest, None),
            "Hãy tự động chọn phương án tối ưu nhất và tiến hành tạo video ngay giúp tôi. Please choose automatically with the best settings and proceed to generate the video now."
        );
    }

    #[test]
    fn confirmation_reply_mac_dinh_yes_proceed() {
        let expected = "Yes, please proceed. Có, hãy tiếp tục tạo video. Đúng rồi, hãy tạo video theo các thông số này. はい、確認しました。生成開始してください。";
        assert_eq!(
            confirmation_reply(&["ご確認ください".to_string()], None),
            expected
        );
    }

    #[test]
    fn question_pattern_van_khop_masuka_full_width_qmark() {
        // Pin QUESTION_SRC van khop "...ますか？" (dau hoi full-width cuoi chuoi) — modes.py:134.
        assert_eq!(
            needs_confirmation(&["どの比率にしますか？".to_string()]),
            Some(ConfirmKind::Question)
        );
    }

    #[test]
    fn agent_question_luon_none_nhu_python() {
        // RULING A (fix round 1): khop dung HANH VI THAT cua Python (da xac minh bang
        // `.venv` python that: modes.agent_question(...) tra '' cho ca 4 input duoi day) —
        // xem doc comment tren ham agent_question ve ly do giu nguyen (parity co chu dich).
        assert_eq!(agent_question(&["どの比率にしますか？".to_string()]), None);
        assert_eq!(agent_question(&["A. 猫  B. 犬".to_string()]), None);
        assert_eq!(
            agent_question(&["生成を開始します。よろしいですか？".to_string()]),
            None
        );
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
