//! Nhan UI, JS va regex cua Dola — CHEP NGUYEN VAN tu Python `lvcseedance` HEAD 3d07c19
//! (`src/video/video_worker_ui.py`, `src/utils/modes.py`, `src/utils/session_state.py`). Moi hang
//! ghi ro `file:dong`; JS nhieu dong dung raw string (`r"…"`/`r#"…"#`) va GIU nguyen dau/cuoi dong nhu triple-quoted
//! string Python (test `chep_nguyen_van_khop_video_worker_ui_py` so byte voi file Python khi co).
//!
//! # Regex: hai "the gioi"
//! Python dung cac regex nay o HAI cho: (a) `page.locator(..., has_text=re.compile(...))` — Playwright
//! chuyen `re.Pattern` thanh JS `RegExp(source, flags)` va CHAY TRONG TRINH DUYET (`\b` ASCII, `\d`
//! ASCII, `\s` Unicode); (b) `re.search`/`finditer` trong Python (`_is_mode_match`, `ADD_REF_TIP_RE`,
//! `VIDEO_ID_RE` — `\b`/`\d` Unicode). Vi the moi pattern giu MOT nguon `*_SRC` (dung byte Python),
//! tu do sinh ca `Regex` Rust (`(?i)` + src; semantics nhom b) lan literal JS `/src/i` cho cac
//! `finder_*` (semantics nhom a — chinh xac nhu Playwright da chay). Xem `MODEL_BTN_SRC` cho diem lech.

use std::sync::LazyLock;

use lvc_core::Model;
use regex::Regex;

/// `video_worker_ui.py:1195`, `:2462` — trang chat (goto truoc moi task).
pub const DOLA_CHAT_URL: &str = "https://www.dola.com/chat";
/// `video_worker_ui.py:1029` — hai cookie ngon ngu ma `DOLA_SET_JA_JS` xoa.
pub const LANGUAGE_COOKIES: [&str; 2] = ["user_language_code", "i18next"];
/// `video_worker_ui.py:540` `CAPTCHA_FRAME_KEY` — iframe captcha verifycenter.
pub const CAPTCHA_FRAME_KEY: &str = "bdcaptcha.html";

/// `video_worker_ui.py:1005-1016` `SESSION_JS` = `browser.py:337-348` `SESSION_PROBE_JS` (cung
/// byte) — re-export, khong chep lai.
pub use lvc_core::SESSION_PROBE_JS as SESSION_JS;
/// `video_worker_ui.py:1432` `VIDEO_ID_RE` — da co trong `lvc_core::chain`, re-export.
pub use lvc_core::VIDEO_ID_RE;

/// `video_worker_ui.py:635-664` `RECENT_IDS_JS` — ham async nhan `{msToken, fp}`, tra id hoi thoai
/// gan day (moi nhat truoc) qua `POST /im/chain/recent_conv` (de nhan dien CHAT MOI sau khi gui).
pub const RECENT_IDS_JS: &str = r#"
async ({msToken, fp}) => {
  const params = new URLSearchParams({
    version_code: "20800", language: "ja", device_platform: "web",
    doubao_device_platform: "web", aid: "495671", real_aid: "495671",
    pkg_type: "release_version", pc_version: "3.32.62", doubao_pc_version: "3.32.62",
    region: "JP", sys_region: "JP", samantha_web: "1", web_platform: "browser",
    "use-olympus-account": "1", web_tab_id: crypto.randomUUID(),
  });
  if (msToken) params.set("msToken", msToken);
  if (fp) params.set("fp", fp);
  const r = await fetch("/im/chain/recent_conv?" + params.toString(), {
    method: "POST", credentials: "include",
    headers: {"Content-Type": "application/json; encoding=utf-8", "agw-js-conv": "str", "Accept": "*/*"},
    body: JSON.stringify({
      cmd: 3200,
      uplink_body: {pull_recent_conv_chain_uplink_body: {
        limit: 20, message_count_per_conv: 1, api_version: 1, conv_version: 0, direction: 3,
        option: {not_need_message: false, need_complete_conversation: true, need_coco_conversation: true,
          need_coco_bot: true, need_pc_pin_chain: true, pc_pin_query_type: 0},
      }},
      sequence_id: crypto.randomUUID(), channel: 2, version: "1",
    }),
  });
  if (!r.ok) return [];
  const body = (await r.json()).downlink_body || {};
  const cells = (body.pull_recent_conv_chain_downlink_body || {}).cells || [];
  return cells.map(c => String((c.conversation || {}).conversation_id || c.id || "")).filter(x => /^\d+$/.test(x));
}
"#;

/// `video_worker_ui.py:710` `COUNT_IMG_JS`.
pub const COUNT_IMG_JS: &str = "() => document.querySelectorAll('img').length";

/// `video_worker_ui.py:713-722` `PASTE_IMAGE_JS` — dan File qua `ClipboardEvent` (duong fallback
/// isolated world; o MAIN world cua ta co the dinh 2 lan nhu Python ghi chu, Task 8 quyet).
pub const PASTE_IMAGE_JS: &str = r#"({b64, name, mime}) => {
  const bytes = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
  const file = new File([bytes], name, {type: mime});
  const dt = new DataTransfer(); dt.items.add(file);
  const box = document.querySelector('textarea, [contenteditable="true"]');
  if (!box) return false;
  box.focus();
  box.dispatchEvent(new ClipboardEvent('paste', {bubbles: true, cancelable: true, clipboardData: dt}));
  return true;
}"#;

/// `video_worker_ui.py:730-753` `PASTE_MAIN_WORLD_SCRIPT` — listener `postMessage` o MAIN world:
/// `clipboardData` la Proxy, ben doc `files`/`items` DAU TIEN nhan file that -> dung 1 upload
/// (ADR-014). Voi chromiumoxide `Runtime.evaluate` DA o MAIN world nen co the chay thang (Task 8).
pub const PASTE_MAIN_WORLD_SCRIPT: &str = r#"(() => {
  if (window.__dolaPasteReady) return;
  window.__dolaPasteReady = true;
  window.addEventListener('message', (m) => {
    const a = m.data; if (!a || a.type !== 'dola-paste') return;
    const out = (r) => { document.documentElement.dataset.dolaPasteResult = r; };
    try {
      const bytes = Uint8Array.from(atob(a.b64), c => c.charCodeAt(0));
      const file = new File([bytes], a.name, {type: a.mime});
      const dt = new DataTransfer(); dt.items.add(file); const empty = new DataTransfer();
      const box = document.querySelector('textarea, [contenteditable="true"]'); if (!box) { out('nobox'); return; }
      box.focus();
      const ev = new ClipboardEvent('paste', {bubbles: true, cancelable: true, clipboardData: dt});
      let consumed = false, acc = 0;
      const guarded = new Proxy(dt, {get(t, prop) {
        if (prop === 'files' || prop === 'items') consumed = true;
        const v = Reflect.get(t, prop); return typeof v === 'function' ? v.bind(t) : v; }});
      Object.defineProperty(ev, 'clipboardData', {get: () => { acc++; return consumed ? empty : guarded; }});
      box.dispatchEvent(ev);
      out('main acc=' + acc + ' consumed=' + consumed + ' dp=' + ev.defaultPrevented);
    } catch (e) { out('error ' + String(e).slice(0, 120)); }
  });
  document.documentElement.dataset.dolaPasteReady = '1';
})();"#;

/// `video_worker_ui.py:754-758` `PASTE_INJECT_JS` — tiem `<script>` (CSP co the chan -> 'blocked').
pub const PASTE_INJECT_JS: &str = r"(code) => {
  if (document.documentElement.dataset.dolaPasteReady === '1') return 'ready';
  const s = document.createElement('script'); s.textContent = code; document.head.appendChild(s); s.remove();
  return document.documentElement.dataset.dolaPasteReady === '1' ? 'ready' : 'blocked';
}";

/// `video_worker_ui.py:759-763` `PASTE_TRIGGER_JS`.
pub const PASTE_TRIGGER_JS: &str = r"(a) => {
  delete document.documentElement.dataset.dolaPasteResult;
  window.postMessage({type: 'dola-paste', b64: a.b64, name: a.name, mime: a.mime}, '*');
  return true;
}";

/// `video_worker_ui.py:764` `PASTE_RESULT_JS`.
pub const PASTE_RESULT_JS: &str = "() => document.documentElement.dataset.dolaPasteResult || ''";

/// `video_worker_ui.py:790` `ATTACH_CARD_SEL` — thumbnail anh dinh kem trong composer (do DOM
/// 2026-09-17).
pub const ATTACH_CARD_SEL: &str = r#"[class*="image-container-"] [class*="container-"]"#;
/// `video_worker_ui.py:791` `COUNT_ATTACH_JS` (f-string tren [`ATTACH_CARD_SEL`]) — so the dinh kem.
pub const COUNT_ATTACH_JS: &str = r#"() => document.querySelectorAll('[class*="image-container-"] [class*="container-"]').length"#;

/// `video_worker_ui.py:1019-1025` `DOLA_LANG_JS` — ngon ngu dang chay: `ls` (localStorage
/// `i18nextLng`), `ck` (cookie `i18next`), `ul` (cookie `user_language_code`), `jp` (co nut 高速/プロ).
pub const DOLA_LANG_JS: &str = r"() => {
  const ls = (() => { try { return localStorage.getItem('i18nextLng') || ''; } catch (e) { return ''; } })();
  const ck = (document.cookie.match(/(?:^|;\s*)i18next=([^;]+)/) || [])[1] || '';
  const ul = (document.cookie.match(/(?:^|;\s*)user_language_code=([^;]+)/) || [])[1] || '';
  const jp = !![...document.querySelectorAll('button')].find(b => /高速|プロ/.test((b.innerText || '').trim()));
  return {ls, ck, ul, jp};
}";

/// `video_worker_ui.py:1026-1033` `DOLA_SET_JA_JS` — XOA (khong dat) cookie/localStorage ngon ngu
/// de trang theo locale ja-JP cua trinh duyet. R4: day la duong DUY NHAT ta port (Python con
/// duong Cai dat UI `switch_dola_language_ui`, khong port).
pub const DOLA_SET_JA_JS: &str = r"() => {
  // XOÁ cookie ngôn ngữ (không đặt ja: lệch hồ sơ → Dola tải lại vô hạn) → trang theo locale ja-JP của trình duyệt
  try { localStorage.removeItem('i18nextLng'); } catch (e) {}
  for (const n of ['user_language_code', 'i18next']) {
    document.cookie = n + '=; path=/; domain=.dola.com; max-age=0';
    document.cookie = n + '=; path=/; max-age=0';
  }
}";

/// `video_worker_ui.py:1055` `MODE_BTN_JS` — co nut che do bat dau bang 高速/プロ (trang da on dinh).
pub const MODE_BTN_JS: &str = "() => !![...document.querySelectorAll('button')].find(b => /^(高速|プロ)/.test((b.innerText || '').trim()))";

/// `video_worker_ui.py:1948` `_BUTTONS_JS` — 25 nhan nut dau tien (cho thong diep `MODE_UNAVAILABLE`).
pub const BUTTONS_JS: &str = "() => [...document.querySelectorAll('button')].map(b => (b.innerText||'').trim()).filter(Boolean).slice(0, 25)";

/// `video_worker_ui.py:1306` (inline trong `_handle_poll`, quet DOM buoc 4 — Task 11): 3000 ky tu
/// cuoi cua `innerText` trong `<main>` (KHONG `body`: thanh ben co ten chat cu 'Nhắc nhở tín dụng
/// video' khop credit oan — chu thich `:1303-1304`).
pub const DOM_TAIL_JS: &str =
    "() => ((document.querySelector('main') || document.body).innerText || '').slice(-3000)";

/// `video_worker_ui.py:1087-1095` `SIDEBAR_LAST_LEAF_JS` — la cuoi cua `nav|aside` (hang tai khoan)
/// cho duong Cai dat UI. R4: duong UI KHONG duoc port; giu hang de tham chieu/Task sau.
pub const SIDEBAR_LAST_LEAF_JS: &str = r"() => {
  const side = document.querySelector('nav') || document.querySelector('aside');
  if (!side) return null;
  const leaves = [...side.querySelectorAll('*')].filter(e => e.children.length === 0 && (e.innerText||'').trim() && e.getBoundingClientRect().height > 0);
  const last = leaves[leaves.length - 1];
  if (!last) return null;
  const r = last.getBoundingClientRect();
  return {txt: last.innerText.trim().slice(0, 40), x: r.x + r.width / 2, y: r.y + r.height / 2};
}";

/// `video_worker_ui.py:1529-1531` `NEW_IMAGES_JS` — anh Dola tao (>= 400px, host byteimg/tos, khong avatar).
pub const NEW_IMAGES_JS: &str = r"() => [...document.querySelectorAll('img')]
  .filter(i => i.naturalWidth >= 400 && /ibyteimg|byteimg|tos-/.test(i.src) && !/avatar/.test(i.src))
  .map(i => ({src: i.src, w: i.naturalWidth, h: i.naturalHeight}))";

/// `video_worker_ui.py:2215-2242` `DURATION_HOOK_JS` (ADR-044) — boc `fetch`/XHR o MAIN world de
/// doi `duration` trong `ability_param` TRUOC khi SDK ky (`a_bogus`). Giu placeholder `__DUR__`;
/// dung [`duration_hook_script`] de thay.
pub const DURATION_HOOK_JS: &str = r#"
(() => {
  const N = __DUR__;
  const patch = (body) => {
    if (typeof body !== 'string' || !body.includes('chat_ability')) return body;
    let changed = false;
    let out = body.replace(/\\"duration\\":\s*\d+/g, () => { changed = true; return '\\"duration\\":' + N; });
    out = out.replace(/"duration":\s*\d+/g, () => { changed = true; return '"duration":' + N; });
    if (changed) document.documentElement.setAttribute('data-lvc-hook', 'patched:' + N);
    return out;
  };
  const isTarget = (u) => typeof u === 'string' && u.includes('/chat/completion');
  if (window.__lvcDurationHook) { window.__lvcDurationHook.N = N; document.documentElement.setAttribute('data-lvc-hook', 'installed:' + N); return; }
  const st = window.__lvcDurationHook = {N};
  const of = window.fetch;
  window.fetch = function (input, init) {
    try {
      const u = typeof input === 'string' ? input : (input && input.url);
      if (isTarget(u) && init && typeof init.body === 'string') init = Object.assign({}, init, {body: patch(init.body)});
    } catch (e) {}
    return of.call(this, input, init);
  };
  const oo = XMLHttpRequest.prototype.open, os = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (m, u) { this.__lvc_u = u; return oo.apply(this, arguments); };
  XMLHttpRequest.prototype.send = function (b) { try { if (isTarget(this.__lvc_u)) b = patch(b); } catch (e) {} return os.call(this, b); };
  document.documentElement.setAttribute('data-lvc-hook', 'installed:' + N);
})();
"#;

/// `video_worker_ui.py:2245-2246` `duration_hook_script(duration)` — thay `__DUR__` bang so giay.
#[must_use]
pub fn duration_hook_script(duration: u16) -> String {
    DURATION_HOOK_JS.replace("__DUR__", &duration.to_string())
}

/// `video_worker_ui.py:2329` `ABILITY_TAG_SEL` — the ky nang trong o nhap (do DOM 2026-09-17: dau
/// ✕ la SVG, khong co chu).
pub const ABILITY_TAG_SEL: &str = r#"[data-input-engine-action-source], [class*="exit-skill-btn"]"#;

/// `video_worker_ui.py:2330-2346` `CLEAR_ABILITY_TAG_JS` — go the ky nang con dinh trong o nhap;
/// tra `'cleared-svg' | 'cleared' | 'none' | 'no-close-button'`.
pub const CLEAR_ABILITY_TAG_JS: &str = r#"() => {
  const re = /(動画を作成|動画の生成|画像を作成|ドキュメントを作成|Tạo video|Create Videos?)\s*[✕x×]/i;
  const modern = document.querySelector('[data-input-engine-action-source] svg, [class*="exit-skill-close"]');
  if (modern) {
    modern.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));
    return 'cleared-svg';
  }
  const tag = [...document.querySelectorAll('button, div, span')]
      .filter(e => re.test((e.innerText || '')) && e.children.length <= 6)
      .pop();
  if (!tag) return 'none';
  const close = [...tag.querySelectorAll('*')].reverse()
      .find(c => /^[✕x×]$/.test((c.innerText || '').trim()) || /close|clear|remove/i.test(c.className || ''));
  if (!close) return 'no-close-button';   // KHÔNG bấm vào thẻ (bấm vào thẻ sẽ mở lại menu kỹ năng, không phải gỡ)
  close.click();
  return 'cleared';
}"#;

// ---------------------------------------------------------------------------------------------
// Selector / nhan
// ---------------------------------------------------------------------------------------------

/// `video_worker_ui.py:2038` — selector muc menu khi chon che do (menuitem/option/div menu-item/
/// div option/button).
pub const MENU_ITEM_SEL: &str =
    r#"[role="menuitem"], [role="option"], div[class*="menu-item"], div[class*="option"], button"#;
/// `video_worker_ui.py:1903` — selector muc menu thoi luong (KHONG co `div[class*="option"]`).
pub const DUR_MENU_ITEM_SEL: &str =
    r#"[role="menuitem"], [role="option"], div[class*="menu-item"], button"#;

/// `video_worker_ui.py:1987` — nhan nut tat cookie banner (`button:has-text("OK"), button:has-text("Đồng ý")`).
pub const COOKIE_OK_LABELS: [&str; 2] = ["OK", "Đồng ý"];
/// Finder JS cho nut cookie banner, THEO PYTHON (:1987-1990, ruling review Task 6 M-5):
/// `locator('button:has-text("OK"), button:has-text("Đồng ý")').first` = nut DAU TIEN theo thu tu
/// DOM co chu chua "ok"/"đồng ý" (`:has-text()`: chuoi con, khong phan biet hoa/thuong, chuan hoa
/// khoang trang — nen "OK" cung khop "Look"), ROI moi `is_visible()`: nut do bi an -> `null`
/// (Python bo qua ca banner, KHONG bam nut khop ke tiep).
pub const COOKIE_OK_FINDER: &str = "(() => { const b = [...document.querySelectorAll('button')].find(b => { const t = (b.innerText || '').replace(/\\s+/g,' ').trim().toLowerCase(); return ['ok', 'đồng ý'].some(l => t.includes(l)); }); if (!b) return null; const r = b.getBoundingClientRect(); return r.width > 0 && r.height > 0 ? b : null; })()";

/// `_close_open_menus` (Task 7): co menu dang mo khong.
pub const OPEN_MENU_JS: &str = r#"!!document.querySelector('[role="menu"]')"#;

/// `video_worker_ui.py:2003` — o nhap de xoa nhap cu khi composer con mo (`select_mode`).
pub const DRAFT_BOX_SEL: &str = "div[contenteditable='true'], textarea";

/// `video_worker_ui.py:1929` — ung vien 2 cua `_wait_mode_button`: nhan che do o dau/sau khoang trang.
pub const MODE_BTN_LOOSE_SRC: &str = r"(?:^|\s|\n)(高速|Nhanh|Fast|プロ|Pro)";
/// `video_worker_ui.py:1930` — ung vien 3: `button:has-text("高速"), button:has-text("Fast"), ...` =
/// nut DAU TIEN theo DOM co chu (chuan hoa khoang trang, khong phan biet hoa/thuong) CHUA mot trong
/// 5 nhan; KHONG loc visible (caller kiem nhu Python `count() and is_visible()`).
pub const MODE_HAS_TEXT_FINDER: &str = "[...document.querySelectorAll('button')].find(b => { const t = (b.innerText||'').replace(/\\s+/g,' ').trim().toLowerCase(); return ['高速', 'fast', 'nhanh', 'pro', 'プロ'].some(l => t.includes(l)); })";
/// `video_worker_ui.py:2037` — muc menu che do Nhanh / Pro (`has_text=`, IGNORECASE).
pub const FAST_ITEM_SRC: &str = r"(?:^|\s|\n)(高速|Nhanh|Fast)";
pub const PRO_ITEM_SRC: &str = r"(?:^|\s|\n)(プロ|Pro)";

/// `video_worker_ui.py:1903-1904` — muc menu thoi luong `rf"^\s*{int(duration)}s(?:\b|\s|$)"`.
#[must_use]
pub fn dur_item_src(duration: u16) -> String {
    format!(r"^\s*{duration}s(?:\b|\s|$)")
}

/// `video_worker_ui.py:2192` — muc menu ti le `rf"^\s*{re.escape(ratio)}(?:\b|\s|$)"` (Python
/// KHONG co IGNORECASE; `finder_menu_item_in` them `i` — vo hai voi chu so/dau `:`).
#[must_use]
pub fn ratio_item_src(ratio: &str) -> String {
    format!(r"^\s*{}(?:\b|\s|$)", regex::escape(ratio))
}

/// `video_worker_ui.py:1854-1856` — nhan che do (da ngon ngu: Nhat/Viet/Anh).
pub const FAST_MODE_LABELS: [&str; 3] = ["高速", "Nhanh", "Fast"];
pub const PRO_MODE_LABELS: [&str; 2] = ["プロ", "Pro"];
pub const ALL_MODE_LABELS: [&str; 5] = ["高速", "Nhanh", "Fast", "プロ", "Pro"];

/// `modes.py:36-50` `UI_LABELS` — moi nhan tieng Nhat cua trang Dola (locale ja-JP); Dola doi nhan
/// thi chi sua o day (IDEA-006).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiLabels {
    /// nut che do + menuitem
    pub mode_fast: &'static str,
    pub mode_pro: &'static str,
    /// Nhanh: mo composer video
    pub video_btn: &'static str,
    /// Pro: nut mo listbox ky nang
    pub skill_btn: &'static str,
    /// Pro: option ky nang tao video
    pub skill_video: &'static str,
    pub ratio_btn: &'static str,
    /// nut model bat dau bang "モデル "
    pub model_prefix: &'static str,
    /// `modes.py:44` — nhan nut model hien tai (`get_by_text(exact=True)`).
    pub model_current: &'static [&'static str],
    /// `modes.py:45-50` — xem [`MODEL_OPTIONS`].
    pub model_options: &'static [(Model, &'static [&'static str]); 3],
}

/// `modes.py:45-50` `UI_LABELS["model_options"]` — nhan option trong menu Model theo model (thu tu
/// Python: 2.5, 2.0, 1.0; `get_by_text(exact=False)` thu lan luot).
pub const MODEL_OPTIONS: [(Model, &[&str]); 3] = [
    (
        Model::Seedance25,
        &["Dreamina Seedance 2.5", "Seedance 2.5"],
    ),
    (
        Model::Seedance20,
        &[
            "Dreamina Seedance 2.0高速",
            "Dreamina Seedance 2.0 Nhanh",
            "Dreamina Seedance 2.0",
            "Seedance2.0Fast",
            "2.0 Nhanh",
            "2.0",
        ],
    ),
    (
        Model::Seedance10,
        &[
            "Dreamina Seedance 1.0",
            "Seedance 1.0",
            "Seedance1.0Fast",
            "1.0高速",
            "1.0",
        ],
    ),
];

pub const UI_LABELS: UiLabels = UiLabels {
    mode_fast: "高速",
    mode_pro: "プロ",
    video_btn: "動画を作成",
    skill_btn: "スキル",
    skill_video: "動画の生成",
    ratio_btn: "比率",
    model_prefix: "モデル",
    model_current: &[
        "モデル 2.0高速",
        "モデル 2.5",
        "モデル 1.0",
        "2.0 Nhanh",
        "Seedance 2.5",
        "Seedance 1.0",
    ],
    model_options: &MODEL_OPTIONS,
};

// ---------------------------------------------------------------------------------------------
// Regex — `*_SRC` = pattern dung byte Python; `*_RE` = Regex Rust `(?i)` + src (Python IGNORECASE)
// ---------------------------------------------------------------------------------------------

/// Regex Rust = semantics Python `re` (`\b`/`\d`/`\s` Unicode) — dung cho kiem tra THUAN (test,
/// `_is_mode_match`, `ADD_REF_TIP_RE`, `VIDEO_ID_RE`). CHON PHAN TU tren trang phai qua `finder_*`
/// (JS `RegExp`, `\b`/`\d` ASCII — dung nhu Playwright `has_text=` da chay trong Python); KHONG ap
/// `*_RE` Rust len chu doc ve tu trang de "chon" (review Task 6 M-6, deviation 3).
fn re_i(src: &str) -> Regex {
    Regex::new(&format!("(?i){src}")).unwrap_or_else(|e| panic!("regex Dola hop le: {src:?}: {e}"))
}

/// `video_worker_ui.py:1858` — `"(" + "|".join(re.escape(v) for v in ALL_MODE_LABELS) + ")"`;
/// xay dung y het (khong nhan nao co ky tu dac biet -> `(高速|Nhanh|Fast|プロ|Pro)`).
pub static MODE_BTN_SRC: LazyLock<String> = LazyLock::new(|| {
    let alts: Vec<String> = ALL_MODE_LABELS.iter().map(|v| regex::escape(v)).collect();
    format!("({})", alts.join("|"))
});
/// Nhan che do KEM huy hieu/dong moi/phien ban (`'Nhanh\nMới'`, `'Pro\nNew'`, `'モデル\n2.0高速'`).
pub static MODE_BTN_RE: LazyLock<Regex> = LazyLock::new(|| re_i(&MODE_BTN_SRC));

/// `video_worker_ui.py:1859` `VIDEO_BTN_RE`.
pub const VIDEO_BTN_SRC: &str = r"^\s*(動画を作成|Tạo video|Create Video)\s*$";
pub static VIDEO_BTN_RE: LazyLock<Regex> = LazyLock::new(|| re_i(VIDEO_BTN_SRC));

/// `video_worker_ui.py:1862` `RATIO_BTN_RE` — nut ti le CHUA chon = "比率"; DA chon = "比率\n16:9".
pub const RATIO_BTN_SRC: &str = r"^\s*(比率|Tỷ lệ|Tỉ lệ|Ratio)(?:\s*\d+\s*:\s*\d+)?\s*$";
pub static RATIO_BTN_RE: LazyLock<Regex> = LazyLock::new(|| re_i(RATIO_BTN_SRC));

/// `video_worker_ui.py:1863` `MODEL_BTN_RE`. LUU Y: Python chi dung no qua `has_text=` (chay trong
/// trinh duyet, JS `\b` ASCII) nen thuc te "モデル 2.0高速" KHONG khop trong Python (ル va khoang
/// trang deu la non-word ASCII) va `_select_model` roi xuong `model_current`/`model_prefix`; Regex
/// Rust o day (`\b` Unicode) THI khop. Task 7 dung `finder_button_text(MODEL_BTN_SRC)` de giu
/// dung hanh vi Python.
pub const MODEL_BTN_SRC: &str = r"(モデル|Model)\b";
pub static MODEL_BTN_RE: LazyLock<Regex> = LazyLock::new(|| re_i(MODEL_BTN_SRC));

/// `video_worker_ui.py:1864` `DUR_BTN_RE` — "10s", "15 秒 (β)".
pub const DUR_BTN_SRC: &str = r"^\s*\d+\s*(?:s|秒)(?:\s*\(.*?\))?\s*$";
pub static DUR_BTN_RE: LazyLock<Regex> = LazyLock::new(|| re_i(DUR_BTN_SRC));

/// `video_worker_ui.py:2091` — nut スキル (Pro).
pub const SKILL_BTN_SRC: &str = r"^\s*(スキル|Kỹ năng|Skills?)\s*$";
pub static SKILL_BTN_RE: LazyLock<Regex> = LazyLock::new(|| re_i(SKILL_BTN_SRC));

/// `video_worker_ui.py:2095` — option 動画の生成 trong listbox ky nang (chi can bat dau bang).
pub const VIDEO_SKILL_OPT_SRC: &str = r"^\s*(動画の生成|Tạo video|Create Video)";
pub static VIDEO_SKILL_OPT_RE: LazyLock<Regex> = LazyLock::new(|| re_i(VIDEO_SKILL_OPT_SRC));

/// `video_worker_ui.py:2326` `ABILITY_TAG_RE` — the ky nang "動画を作成 ✕" trong o nhap (moi loai).
pub const ABILITY_TAG_SRC: &str =
    r"(動画を作成|動画の生成|画像を作成|ドキュメントを作成|Tạo video|Create Videos?)\s*[✕x×]";
pub static ABILITY_TAG_RE: LazyLock<Regex> = LazyLock::new(|| re_i(ABILITY_TAG_SRC));

/// `video_worker_ui.py:2059` — chi the VIDEO cua che do Nhanh (composer dang mo).
pub const VIDEO_TAG_SRC: &str = r"動画を作成\s*[✕x×]";
pub static VIDEO_TAG_RE: LazyLock<Regex> = LazyLock::new(|| re_i(VIDEO_TAG_SRC));

/// `video_worker_ui.py:1548` `ADD_REF_TIP_RE` — tooltip nut "参照資料として追加"/"Add as reference".
pub const ADD_REF_TIP_SRC: &str = r"参照|参考|reference|tham khảo|참조|참고";
pub static ADD_REF_TIP_RE: LazyLock<Regex> = LazyLock::new(|| re_i(ADD_REF_TIP_SRC));

// ---------------------------------------------------------------------------------------------
// Finder JS (bieu thuc `Element|null` cho `ops::click_js`/`hover_js`)
// ---------------------------------------------------------------------------------------------

/// Bien nguon regex JS `src` (KHONG co dau `/` bao ngoai) + `flags` thanh literal `/src/flags`:
/// escape moi `/` chua duoc escape (`a/b` -> `a\/b`, `a\/b` giu nguyen); backslash cua regex GIU
/// NGUYEN (khong nhan doi — day la regex, khong phai chuoi). `src` khong duoc chua xuong dong.
#[must_use]
pub fn js_regex_literal(src: &str, flags: &str) -> String {
    let mut out = String::with_capacity(src.len() + flags.len() + 4);
    out.push('/');
    let mut escaped = false;
    for c in src.chars() {
        match c {
            '\\' if !escaped => {
                escaped = true;
                out.push(c);
                continue;
            }
            '/' if !escaped => out.push_str("\\/"),
            _ => out.push(c),
        }
        escaped = false;
    }
    out.push('/');
    out.push_str(flags);
    out
}

/// Nut `<button>` DAU TIEN co `innerText.trim()` khop `/re_js/i` — Playwright
/// `page.locator("button", has_text=re.compile(re_js, re.IGNORECASE)).first`.
#[must_use]
pub fn finder_button_text(re_js: &str) -> String {
    let lit = js_regex_literal(re_js, "i");
    format!(
        "[...document.querySelectorAll('button')].find(b => {lit}.test((b.innerText||'').trim()))"
    )
}

/// Muc menu DAU TIEN ([`MENU_ITEM_SEL`]) co `innerText.trim()` khop `/re_js/i` —
/// `page.locator(MENU_ITEM_SEL, has_text=...).first` (`video_worker_ui.py:2038`). Bao boc
/// [`finder_menu_item_in`] voi selector mac dinh.
#[must_use]
pub fn finder_menu_item(re_js: &str) -> String {
    finder_menu_item_in(MENU_ITEM_SEL, re_js)
}

/// Nhu [`finder_menu_item`] nhung selector tuy chon (review Task 6 M-8): menu thoi luong dung
/// [`DUR_MENU_ITEM_SEL`] (`:1903`), option ky nang Pro dung `[role="option"]` (`:2095`). `selector`
/// duoc escape `\` va `'` de nam trong chuoi JS `'...'`.
#[must_use]
pub fn finder_menu_item_in(selector: &str, re_js: &str) -> String {
    let lit = js_regex_literal(re_js, "i");
    let sel = selector.replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        "[...document.querySelectorAll('{sel}')].find(e => {lit}.test((e.innerText||'').trim()))"
    )
}

/// Phan tu SAU NHAT (khong con nao cung chu) dau tien theo thu tu DOM co text == `text` — gan
/// `page.get_by_text(text, exact=True).first`. Review Task 6 I-1: Playwright CHUAN HOA khoang trang
/// CA HAI PHIA ke ca `exact=True` (`\s+` -> 1 khoang trang, trim) nen "モデル 2.0高速" khop node
/// `モデル\n2.0高速` (`:1857`). `text` duoc ma hoa JSON (an toan voi dau nhay/backslash/xuong dong).
#[must_use]
pub fn finder_text_exact(text: &str) -> String {
    let json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "(() => {{ const norm = (s) => (s||'').replace(/\\s+/g,' ').trim(); const t = norm({json}); \
         const same = (e) => norm(e.textContent) === t; \
         return [...document.querySelectorAll('*')].find(e => e.tagName !== 'SCRIPT' && e.tagName !== 'STYLE' \
         && same(e) && (e.childElementCount === 0 || ![...e.children].some(same))) || null; }})()"
    )
}

/// Phan tu SAU NHAT dau tien theo DOM co text (chuan hoa khoang trang, ha thuong) CHUA `text` — gan
/// `page.get_by_text(text, exact=False).first` / `page.click("text=...")` (Playwright: lax = chuoi
/// con, khong phan biet hoa/thuong, chuan hoa `\s+`; chi lay phan tu ma KHONG con nao cung khop).
/// Dung cho option model (`:2160`), `get_by_text(re.compile(re.escape("モデル")))` (`:2148`) va
/// `page.click("text=" + UI_LABELS[...])` (`:2074`, `:2190`). `text` ma hoa JSON.
#[must_use]
pub fn finder_text_lax(text: &str) -> String {
    let json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "(() => {{ const norm = (s) => (s||'').replace(/\\s+/g,' ').trim().toLowerCase(); const t = norm({json}); \
         const has = (e) => norm(e.textContent).includes(t); \
         return [...document.querySelectorAll('*')].find(e => e.tagName !== 'SCRIPT' && e.tagName !== 'STYLE' && e.tagName !== 'NOSCRIPT' \
         && has(e) && ![...e.children].some(has)) || null; }})()"
    )
}

/// O soan (textarea hoac contenteditable) dau tien — Python `'textarea, [contenteditable="true"]'`
/// (`:717`, `_type_and_send`).
#[must_use]
pub fn finder_composer() -> &'static str {
    r#"document.querySelector('textarea, [contenteditable="true"]')"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dola::py_src::{MODES_PY, VIDEO_WORKER_UI, python_source, same_bytes};

    // ---------- regex (semantics Python/Playwright) ----------

    #[test]
    fn mode_btn_re_khop_nhan_kem_huy_hieu() {
        for s in [
            "高速\nNew",
            "プロ",
            "Pro >",
            "モデル\n2.0高速",
            "2.0 Fast",
            "nhanh",
            "PRO",
        ] {
            assert!(MODE_BTN_RE.is_match(s), "{s:?}");
        }
        for s in ["Model", "モデル", "比率", ""] {
            assert!(!MODE_BTN_RE.is_match(s), "{s:?}");
        }
    }

    /// Python :1858 xay pattern tu `ALL_MODE_LABELS` bang `re.escape` — ket qua phai dung la
    /// literal `(高速|Nhanh|Fast|プロ|Pro)` cua plan (khong nhan nao co ky tu dac biet).
    #[test]
    fn mode_btn_re_pattern_tu_all_mode_labels() {
        assert_eq!(ALL_MODE_LABELS, ["高速", "Nhanh", "Fast", "プロ", "Pro"]);
        assert_eq!(MODE_BTN_SRC.as_str(), "(高速|Nhanh|Fast|プロ|Pro)");
        assert_eq!(MODE_BTN_RE.as_str(), "(?i)(高速|Nhanh|Fast|プロ|Pro)");
    }

    #[test]
    fn dur_btn_re_so_giay() {
        for s in ["10s", " 15 秒 (β)", "30S", "10 s", "5秒(β)"] {
            assert!(DUR_BTN_RE.is_match(s), "{s:?}");
        }
        for s in ["10sec", "s", "10 s x", "秒", "10"] {
            assert!(!DUR_BTN_RE.is_match(s), "{s:?}");
        }
    }

    #[test]
    fn ratio_btn_re_co_va_khong_co_ti_le() {
        for s in ["比率\n16:9", "比率", "Ratio 9:16", "Tỷ lệ", " Tỉ lệ 1 : 1 "] {
            assert!(RATIO_BTN_RE.is_match(s), "{s:?}");
        }
        for s in ["比率 16:9 x", "比率16", "x 比率", "Ratios"] {
            assert!(!RATIO_BTN_RE.is_match(s), "{s:?}");
        }
    }

    #[test]
    fn video_btn_re_chi_nhan_tran() {
        for s in ["動画を作成", " Tạo video ", "create video"] {
            assert!(VIDEO_BTN_RE.is_match(s), "{s:?}");
        }
        for s in ["動画を作成 ✕", "Create Videos", "動画の生成"] {
            assert!(!VIDEO_BTN_RE.is_match(s), "{s:?}");
        }
    }

    /// `\b` cua crate regex la Unicode (nhu Python `re`): "モデル 2.0高速" khop. LUU Y JS `\b` chi
    /// ASCII — xem doc `MODEL_BTN_SRC`.
    #[test]
    fn model_btn_re_unicode_word_boundary() {
        assert!(MODEL_BTN_RE.is_match("モデル 2.0高速"));
        assert!(MODEL_BTN_RE.is_match("Model"));
        assert!(MODEL_BTN_RE.is_match("model 2.5"));
        assert!(!MODEL_BTN_RE.is_match("Models"));
        assert!(!MODEL_BTN_RE.is_match("比率"));
    }

    #[test]
    fn skill_va_option_video_re() {
        for s in ["スキル", " Skills ", "skill", "Kỹ năng"] {
            assert!(SKILL_BTN_RE.is_match(s), "{s:?}");
        }
        assert!(!SKILL_BTN_RE.is_match("スキル一覧"));
        assert!(VIDEO_SKILL_OPT_RE.is_match("動画の生成 — Seedance"));
        assert!(VIDEO_SKILL_OPT_RE.is_match("  Create Video"));
        assert!(!VIDEO_SKILL_OPT_RE.is_match("x 動画の生成"));
    }

    #[test]
    fn ability_tag_re_can_dau_x() {
        for s in [
            "動画を作成 ✕",
            "Create Videos x",
            "画像を作成×",
            "動画の生成✕",
        ] {
            assert!(ABILITY_TAG_RE.is_match(s), "{s:?}");
        }
        assert!(!ABILITY_TAG_RE.is_match("動画を作成"));
        // :2059 — chi the video cua che do Nhanh
        assert!(VIDEO_TAG_RE.is_match("動画を作成 ✕"));
        assert!(!VIDEO_TAG_RE.is_match("画像を作成 ✕"));
    }

    #[test]
    fn add_ref_tip_re_da_ngon_ngu() {
        for s in [
            "参照資料として追加",
            "Add as reference",
            "Tham khảo",
            "참조",
        ] {
            assert!(ADD_REF_TIP_RE.is_match(s), "{s:?}");
        }
        assert!(!ADD_REF_TIP_RE.is_match("download"));
    }

    #[test]
    fn video_id_re_la_cua_lvc_core() {
        assert!(VIDEO_ID_RE.is_match("v186a3gm000cdal5gdvog65tvg6d5png"));
        assert_eq!(VIDEO_ID_RE.as_str(), lvc_core::VIDEO_ID_RE.as_str());
    }

    #[test]
    fn session_js_la_session_probe_js() {
        assert_eq!(SESSION_JS, lvc_core::SESSION_PROBE_JS);
    }

    // ---------- finder JS ----------

    #[test]
    fn js_regex_literal_escape_gach_cheo() {
        assert_eq!(js_regex_literal(r"a/b", "i"), r"/a\/b/i");
        // da escape san thi khong escape lai
        assert_eq!(js_regex_literal(r"a\/b", "i"), r"/a\/b/i");
        // backslash cua regex giu nguyen (khong nhan doi)
        assert_eq!(js_regex_literal(r"^\s*\d+\s*$", ""), r"/^\s*\d+\s*$/");
        // `\\` roi `/` -> `/` KHONG duoc coi la da escape
        assert_eq!(js_regex_literal(r"a\\/b", "g"), r"/a\\\/b/g");
    }

    #[test]
    fn finder_button_text_sinh_literal_js() {
        let js = finder_button_text(VIDEO_BTN_SRC);
        assert!(
            js.starts_with("[...document.querySelectorAll('button')].find(b => "),
            "{js}"
        );
        assert!(
            js.contains(
                r"/^\s*(動画を作成|Tạo video|Create Video)\s*$/i.test((b.innerText||'').trim())"
            ),
            "{js}"
        );
        let js2 = finder_button_text("a/b");
        assert!(js2.contains(r"/a\/b/i.test("), "{js2}");
        // Dung duoc voi MODE_BTN_SRC (LazyLock<String>)
        assert!(finder_button_text(&MODE_BTN_SRC).contains("/(高速|Nhanh|Fast|プロ|Pro)/i"));
    }

    #[test]
    fn finder_menu_item_dung_selector_python_2038() {
        let js = finder_menu_item(r"^\s*10s(\b|\s|$)");
        assert_eq!(
            MENU_ITEM_SEL,
            r#"[role="menuitem"], [role="option"], div[class*="menu-item"], div[class*="option"], button"#
        );
        assert!(
            js.contains(&format!("querySelectorAll('{MENU_ITEM_SEL}')")),
            "{js}"
        );
        assert!(js.contains(r"/^\s*10s(\b|\s|$)/i.test("), "{js}");
        assert_eq!(
            DUR_MENU_ITEM_SEL,
            r#"[role="menuitem"], [role="option"], div[class*="menu-item"], button"#
        );
        // M-8: selector tuy chon (menu thoi luong :1903, option ky nang :2095)
        let dur = finder_menu_item_in(DUR_MENU_ITEM_SEL, r"^\s*10s(\b|\s|$)");
        assert!(
            dur.contains(&format!("querySelectorAll('{DUR_MENU_ITEM_SEL}')")),
            "{dur}"
        );
        let opt = finder_menu_item_in(r#"[role="option"]"#, VIDEO_SKILL_OPT_SRC);
        assert!(
            opt.contains(r#"querySelectorAll('[role="option"]')"#),
            "{opt}"
        );
        assert!(
            opt.contains(&format!("/{VIDEO_SKILL_OPT_SRC}/i.test(")),
            "{opt}"
        );
        assert_eq!(js, finder_menu_item_in(MENU_ITEM_SEL, r"^\s*10s(\b|\s|$)"));
        // selector co dau nhay don -> escape de JS van hop le
        let quoted = finder_menu_item_in("a[x='1']", "b");
        assert!(
            quoted.contains(r"querySelectorAll('a[x=\'1\']')"),
            "{quoted}"
        );
    }

    #[test]
    fn finder_text_exact_ma_hoa_json() {
        let js = finder_text_exact("OK");
        assert!(js.contains("const t = norm(\"OK\")"), "{js}");
        assert!(js.contains("childElementCount"), "phai la phan tu la: {js}");
        let tricky = finder_text_exact("a\"b\\c\n日本語");
        assert!(tricky.contains(r#""a\"b\\c\n日本語""#), "{tricky}");
    }

    /// I-1 (review Task 6): Playwright `get_by_text(exact=True)` CHUAN HOA khoang trang ca hai phia
    /// (`\s+` -> 1 khoang trang, trim) — "モデル 2.0高速" phai khop node `モデル\n2.0高速` (:1857).
    #[test]
    fn finder_text_exact_chuan_hoa_khoang_trang() {
        let js = finder_text_exact("モデル 2.0高速");
        assert!(js.contains(r"replace(/\s+/g,' ')"), "{js}");
        // ca chuoi truy van lan textContent deu qua norm
        assert!(
            js.contains("norm(") && js.matches("norm(").count() >= 2,
            "{js}"
        );
    }

    #[test]
    fn finder_composer_va_open_menu() {
        assert_eq!(
            finder_composer(),
            r#"document.querySelector('textarea, [contenteditable="true"]')"#
        );
        assert_eq!(OPEN_MENU_JS, r#"!!document.querySelector('[role="menu"]')"#);
        assert_eq!(COOKIE_OK_LABELS, ["OK", "Đồng ý"]);
        assert!(
            COOKIE_OK_FINDER.contains("'ok'") && COOKIE_OK_FINDER.contains("'đồng ý'"),
            "{COOKIE_OK_FINDER}"
        );
        assert!(
            COOKIE_OK_FINDER.contains("getBoundingClientRect"),
            "phai loc visible"
        );
        // M-5 (ruling): Python `.first` theo thu tu DOM roi moi `is_visible()` -> tim TRUOC (khong
        // loc visible trong `find`), kiem rect SAU; an -> null. `:has-text()` chuan hoa khoang trang.
        let find_at = COOKIE_OK_FINDER.find("].find(").expect("co find");
        let rect_at = COOKIE_OK_FINDER
            .find("getBoundingClientRect")
            .expect("co rect");
        let find_end = COOKIE_OK_FINDER[find_at..]
            .find("});")
            .map(|i| find_at + i)
            .unwrap();
        assert!(
            rect_at > find_end,
            "rect phai nam SAU khoi find: {COOKIE_OK_FINDER}"
        );
        assert!(
            COOKIE_OK_FINDER.contains("return null"),
            "{COOKIE_OK_FINDER}"
        );
        assert!(
            COOKIE_OK_FINDER.contains(r"replace(/\s+/g,' ')"),
            "{COOKIE_OK_FINDER}"
        );
    }

    /// Task 7: hang/regex inline trong cac ham compose cua Python (`:791`, `:1929-1930`, `:1948`,
    /// `:2003`, `:2037`, `:1903-1904`, `:2192`).
    #[test]
    fn hang_inline_cho_compose() {
        assert_eq!(
            COUNT_ATTACH_JS,
            format!("() => document.querySelectorAll('{ATTACH_CARD_SEL}').length")
        );
        assert_eq!(MODE_BTN_LOOSE_SRC, r"(?:^|\s|\n)(高速|Nhanh|Fast|プロ|Pro)");
        assert_eq!(FAST_ITEM_SRC, r"(?:^|\s|\n)(高速|Nhanh|Fast)");
        assert_eq!(PRO_ITEM_SRC, r"(?:^|\s|\n)(プロ|Pro)");
        assert_eq!(DRAFT_BOX_SEL, "div[contenteditable='true'], textarea");
        assert!(
            BUTTONS_JS.starts_with("() => [...document.querySelectorAll('button')]"),
            "{BUTTONS_JS}"
        );
        assert!(BUTTONS_JS.ends_with(".slice(0, 25)"), "{BUTTONS_JS}");
        // :1930 `button:has-text("高速"), ...` — substring khong phan biet hoa/thuong, chuan hoa
        // khoang trang, nut DAU TIEN theo DOM (khong loc visible — caller kiem).
        for l in ["'高速'", "'fast'", "'nhanh'", "'pro'", "'プロ'"] {
            assert!(MODE_HAS_TEXT_FINDER.contains(l), "{MODE_HAS_TEXT_FINDER}");
        }
        assert!(MODE_HAS_TEXT_FINDER.contains("toLowerCase()"));
        assert!(MODE_HAS_TEXT_FINDER.contains(r"replace(/\s+/g,' ')"));
        // :1903-1904 `rf"^\s*{int(duration)}s(?:\b|\s|$)"` (IGNORECASE)
        assert_eq!(dur_item_src(15), r"^\s*15s(?:\b|\s|$)");
        // :2192 `rf"^\s*{re.escape(ratio)}(?:\b|\s|$)"` — re.escape("16:9") == "16:9"
        assert_eq!(ratio_item_src("16:9"), r"^\s*16:9(?:\b|\s|$)");
        assert_eq!(ratio_item_src("a.b"), r"^\s*a\.b(?:\b|\s|$)");
    }

    /// `get_by_text(text, exact=False)` / `text=...` (khong nhay): chuan hoa khoang trang, khong
    /// phan biet hoa/thuong, CHUOI CON, phan tu sau nhat dau tien theo DOM.
    #[test]
    fn finder_text_lax_ma_hoa_json_va_lowercase() {
        let js = finder_text_lax("Dreamina Seedance 2.5");
        assert!(js.contains("norm(\"Dreamina Seedance 2.5\")"), "{js}");
        assert!(js.contains("toLowerCase()"), "{js}");
        assert!(js.contains(".includes(t)"), "{js}");
        assert!(js.contains(r"replace(/\s+/g,' ')"), "{js}");
        assert!(
            js.contains("![...e.children].some(has)"),
            "phai la phan tu sau nhat: {js}"
        );
        let tricky = finder_text_lax("a\"b\\c");
        assert!(tricky.contains(r#""a\"b\\c""#), "{tricky}");
    }

    #[test]
    fn duration_hook_script_thay_dur() {
        assert_eq!(DURATION_HOOK_JS.matches("__DUR__").count(), 1);
        let js = duration_hook_script(30);
        assert!(js.contains("const N = 30;"), "{js}");
        assert!(!js.contains("__DUR__"));
        assert!(js.starts_with("\n(() => {"));
        assert!(js.ends_with("})();\n"));
    }

    #[test]
    fn model_options_khop_modes_py() {
        use lvc_core::Model;
        assert_eq!(MODEL_OPTIONS[0].0, Model::Seedance25);
        assert_eq!(
            MODEL_OPTIONS[0].1,
            ["Dreamina Seedance 2.5", "Seedance 2.5"]
        );
        assert_eq!(MODEL_OPTIONS[1].0, Model::Seedance20);
        assert_eq!(
            MODEL_OPTIONS[1].1,
            [
                "Dreamina Seedance 2.0高速",
                "Dreamina Seedance 2.0 Nhanh",
                "Dreamina Seedance 2.0",
                "Seedance2.0Fast",
                "2.0 Nhanh",
                "2.0"
            ]
        );
        assert_eq!(MODEL_OPTIONS[2].0, Model::Seedance10);
        assert_eq!(
            MODEL_OPTIONS[2].1,
            [
                "Dreamina Seedance 1.0",
                "Seedance 1.0",
                "Seedance1.0Fast",
                "1.0高速",
                "1.0"
            ]
        );
        assert_eq!(UI_LABELS.mode_fast, "高速");
        assert_eq!(UI_LABELS.model_prefix, "モデル");
        assert_eq!(UI_LABELS.model_current.len(), 6);
        assert_eq!(UI_LABELS.model_options, &MODEL_OPTIONS);
        assert_eq!(LANGUAGE_COOKIES, ["user_language_code", "i18next"]);
        assert_eq!(DOLA_CHAT_URL, "https://www.dola.com/chat");
        assert_eq!(CAPTCHA_FRAME_KEY, "bdcaptcha.html");
    }

    // ---------- so byte voi nguon Python (bo qua neu file vang) ----------

    /// Moi hang chep nguyen van <-> literal Python cung ten trong `video_worker_ui.py` (HEAD
    /// 3d07c19). Doc file luc test; vang -> in ly do, khong fail.
    #[test]
    fn chep_nguyen_van_khop_video_worker_ui_py() {
        let Some(src) = python_source(VIDEO_WORKER_UI) else {
            return;
        };
        let pairs: [(&str, &str); 22] = [
            ("CAPTCHA_FRAME_KEY", CAPTCHA_FRAME_KEY),
            ("RECENT_IDS_JS", RECENT_IDS_JS),
            ("COUNT_IMG_JS", COUNT_IMG_JS),
            ("PASTE_IMAGE_JS", PASTE_IMAGE_JS),
            ("PASTE_MAIN_WORLD_SCRIPT", PASTE_MAIN_WORLD_SCRIPT),
            ("PASTE_INJECT_JS", PASTE_INJECT_JS),
            ("PASTE_TRIGGER_JS", PASTE_TRIGGER_JS),
            ("PASTE_RESULT_JS", PASTE_RESULT_JS),
            ("ATTACH_CARD_SEL", ATTACH_CARD_SEL),
            ("SESSION_JS", SESSION_JS),
            ("DOLA_LANG_JS", DOLA_LANG_JS),
            ("DOLA_SET_JA_JS", DOLA_SET_JA_JS),
            ("MODE_BTN_JS", MODE_BTN_JS),
            ("SIDEBAR_LAST_LEAF_JS", SIDEBAR_LAST_LEAF_JS),
            ("NEW_IMAGES_JS", NEW_IMAGES_JS),
            ("ADD_REF_TIP_RE", ADD_REF_TIP_SRC),
            ("VIDEO_BTN_RE", VIDEO_BTN_SRC),
            ("RATIO_BTN_RE", RATIO_BTN_SRC),
            ("MODEL_BTN_RE", MODEL_BTN_SRC),
            ("DUR_BTN_RE", DUR_BTN_SRC),
            ("DURATION_HOOK_JS", DURATION_HOOK_JS),
            ("ABILITY_TAG_SEL", ABILITY_TAG_SEL),
        ];
        let mut bad = Vec::new();
        for (name, rust) in pairs {
            if !same_bytes(&src, name, rust) {
                bad.push(name);
            }
        }
        if !same_bytes(&src, "CLEAR_ABILITY_TAG_JS", CLEAR_ABILITY_TAG_JS) {
            bad.push("CLEAR_ABILITY_TAG_JS");
        }
        if !same_bytes(&src, "ABILITY_TAG_RE", ABILITY_TAG_SRC) {
            bad.push("ABILITY_TAG_RE");
        }
        // Task 7: `_BUTTONS_JS` (:1948) la hang module (ten bat dau `_`)
        if !same_bytes(&src, "_BUTTONS_JS", BUTTONS_JS) {
            bad.push("_BUTTONS_JS");
        }
        // M-4: VIDEO_ID_RE (re-export lvc-core) — bo tien to `(?i)` roi so voi :1432
        let video_id_src = VIDEO_ID_RE
            .as_str()
            .strip_prefix("(?i)")
            .expect("VIDEO_ID_RE co (?i)");
        if !same_bytes(&src, "VIDEO_ID_RE", video_id_src) {
            bad.push("VIDEO_ID_RE");
        }
        assert!(bad.is_empty(), "lech nguon Python: {bad:?}");
        // M-4: DOLA_CHAT_URL (:1195, :2462) va OPEN_MENU_JS (:1848) — chuoi nam trong ham
        assert!(
            src.contains(&format!("\"{DOLA_CHAT_URL}\"")),
            "DOLA_CHAT_URL"
        );
        assert!(
            src.contains("page.locator('[role=\"menu\"]')"),
            "OPEN_MENU_JS selector :1848"
        );
        assert!(
            OPEN_MENU_JS.contains(r#"'[role="menu"]'"#),
            "OPEN_MENU_JS dung cung selector"
        );

        // Nhan che do :1854-1856 (tuple) + regex :1858 xay tu do
        assert!(src.contains("\nFAST_MODE_LABELS = (\"高速\", \"Nhanh\", \"Fast\")\n"));
        assert!(src.contains("\nPRO_MODE_LABELS = (\"プロ\", \"Pro\")\n"));
        assert!(src.contains("\nMODE_BTN_RE = re.compile(r\"(\" + \"|\".join(re.escape(v) for v in ALL_MODE_LABELS) + r\")\", re.IGNORECASE)\n"));
        // Regex/chuoi nam TRONG ham (khong phai hang module) :1903, :2038, :2059, :2091, :2095, :1987
        for needle in [
            format!("has_text=re.compile(r\"{SKILL_BTN_SRC}\", re.IGNORECASE)"),
            format!("has_text=re.compile(r\"{VIDEO_SKILL_OPT_SRC}\", re.IGNORECASE)"),
            format!("has_text=re.compile(r\"{VIDEO_TAG_SRC}\", re.IGNORECASE)"),
            format!("page.locator('{MENU_ITEM_SEL}', has_text=item_re)"),
            format!("page.locator('{DUR_MENU_ITEM_SEL}', has_text=re.compile("),
            "page.locator('button:has-text(\"OK\"), button:has-text(\"Đồng ý\")')".to_string(),
            // Task 7 (:791 f-string, :1929-1930, :2003, :2037, :1903-1904, :2192)
            "COUNT_ATTACH_JS = f\"() => document.querySelectorAll('{ATTACH_CARD_SEL}').length\"".to_string(),
            format!("has_text=re.compile(r\"{MODE_BTN_LOOSE_SRC}\", re.IGNORECASE)"),
            "page.locator('button:has-text(\"高速\"), button:has-text(\"Fast\"), button:has-text(\"Nhanh\"), button:has-text(\"Pro\"), button:has-text(\"プロ\")')".to_string(),
            format!("page.locator(\"{DRAFT_BOX_SEL}\")"),
            format!("re.compile(r\"{FAST_ITEM_SRC}\", re.IGNORECASE) if mode == \"fast\" else re.compile(r\"{PRO_ITEM_SRC}\", re.IGNORECASE)"),
            r#"has_text=re.compile(
                rf"^\s*{int(duration)}s(?:\b|\s|$)", re.IGNORECASE)"#.to_string(),
            r#"page.locator('[role="menuitem"]', has_text=re.compile(rf"^\s*{re.escape(ratio)}(?:\b|\s|$)"))"#.to_string(),
        ] {
            assert!(src.contains(&needle), "khong thay trong Python: {needle}");
        }
        // Playwright chay regex `has_text=` TRONG trinh duyet (JS RegExp, co i) — literal JS cua ta
        // phai la chinh pattern do.
        assert!(finder_button_text(VIDEO_BTN_SRC).contains(&format!("/{VIDEO_BTN_SRC}/i")));
    }

    /// `UI_LABELS` <-> `modes.py:36-50`.
    #[test]
    fn ui_labels_khop_modes_py() {
        let Some(src) = python_source(MODES_PY) else {
            return;
        };
        for (key, val) in [
            ("mode_fast", UI_LABELS.mode_fast),
            ("mode_pro", UI_LABELS.mode_pro),
            ("video_btn", UI_LABELS.video_btn),
            ("skill_btn", UI_LABELS.skill_btn),
            ("skill_video", UI_LABELS.skill_video),
            ("ratio_btn", UI_LABELS.ratio_btn),
            ("model_prefix", UI_LABELS.model_prefix),
        ] {
            assert!(
                src.contains(&format!("    \"{key}\": \"{val}\",")),
                "UI_LABELS[{key:?}] != {val:?}"
            );
        }
        let tuple = |xs: &[&str]| {
            xs.iter()
                .map(|x| format!("\"{x}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        assert!(src.contains(&format!(
            "    \"model_current\": ({}),",
            tuple(UI_LABELS.model_current)
        )));
        for (model, opts) in &MODEL_OPTIONS {
            let line = format!("        \"{}\": ({}),", model.wire_str(), tuple(opts));
            assert!(src.contains(&line), "khong thay: {line}");
        }
    }
}
