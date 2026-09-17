# Python video pipeline map — `A:\ToolLVC\lvcseedance` @ `3d07c19` (2026-09-17 21:47)

Purpose: exact engineering map of how the Python tool drives dola.com to make ONE video for ONE account, as input for
the Rust port (`lvc-browser`, chromiumoxide 0.9.1 raw CDP). All paths below are relative to `A:\ToolLVC\lvcseedance`;
`file:line` refers to git HEAD `3d07c19`. Nothing under `lvcseedance` was modified; DBs/`accounts/` were not opened.

Conventions used in this document:
- "worker" = `src/video/video_worker_ui.py` (the real UI-automation worker used by the server).
- `src/video/video_worker.py` is the older in-page-fetch worker; only its `POLL_JS`, `_download`,
  `extract_unwatermarked_url`, `RiskControlError` are still imported by the worker (`video_worker_ui.py:23`).
  Its `generate_video` (`video_worker.py:190-256`, uses `SUBMIT_JS` = direct `/chat/completion` fetch) is **dead
  at HEAD** (never called by server/pool); it is documented only where it clarifies the protocol.
- Patchright = Playwright fork; `page.evaluate` runs in an ISOLATED world (repeatedly measured, ADR-041/044).
  Anything the tool needs in the page's MAIN world is injected via `<script>` tag (paste proxy, duration hook) or
  via extension content script `"world": "MAIN"` (lvc_relay).

---

## 1. Two (actually four) launch modes; default at HEAD

### 1.1 Mode switch
- `hub_browser.MODES = ("profile", "tabs", "single", "wtabs")` — `src/browser/hub_browser.py:37`.
- `browser_mode()` — `hub_browser.py:61-67`: returns `_mode_override` if set, else
  `normalize_mode(config.BROWSER_MODE)`.
- `config.BROWSER_MODE = os.getenv("DOLA_BROWSER_MODE", "profile")` — `src/core/config.py:120`.
- `_mode_override` is set from the SQLite `settings` table (`pool_usage.db`, key `browser_mode`) at pool start:
  `BrowserPool._ensure_settings()` — `src/browser/browser_pool.py:279-284`; dashboard `PATCH /api/admin/settings
  {browser_mode}` → `pool.set_browser_mode()` — `browser_pool.py:344-361`.
- **Default at HEAD = `profile`** (code default). The runtime value is whatever the user last saved in
  `pool_usage.db.settings.browser_mode`; NHO notes: on 2026-09-16 the user asked to go back to
  `browser_mode=profile, threads=1` (`.congcode/NHO.md:48`), and wtabs was later used for the 30s E2E on 17/9
  (`.congcode/NHO.md:36`). I did not open the DB (forbidden), so treat the *code default* `profile` as the answer.
- Helper predicates: `is_tabs_mode()`, `is_single_mode()`, `is_wtabs_mode()`, `is_hub_mode()` (any of tabs/single/wtabs)
  — `hub_browser.py:76-91`.
- The worker always goes through `WarmBrowserManager.acquire()` when `config.WARM_SESSION_ENABLED` (default `1`,
  `config.py:114`) or hub mode — `video_worker_ui.py:2409-2413`.

### 1.2 (a) `profile` — one Chromium per account, persistent profile `accounts/<name>/`
Call chain: `generate_video` → `warm_browser_manager.acquire(account, headless=False, use_extension=True, proxy, browser)`
(`video_worker_ui.py:2411-2413`) → `acquire` (`:195-238`) → `_launch_or_busy` (`:128-142`) →
`launch_account_context` (`src/browser/browser.py:118-184`):

```python
LAUNCH_ARGS = [                                   # browser.py:12-18
    "--disable-blink-features=AutomationControlled",
    "--disable-infobars",
    "--silent-debugger-extension-api",
    "--no-first-run",
    "--no-default-browser-check",
]
```
- `profile_dir = Path("accounts") / account` — must exist (`browser.py:128-131`).
- `use_extension=True` forces `launch_headless = False` (`browser.py:135-137`); extension args are added **only if**
  `config.EXTENSION_ENABLED` (`DOLA_EXTENSION_ENABLED`, default `"0"` → False, `config.py:144`) → **no extension is
  loaded in profile mode at HEAD**.
- `SILENT_MODE` (`config.py:111`, settings key `silent_mode`) adds `--window-position=-32000,-32000` (`browser.py:151-152`).
- kwargs: `headless`, `args`, `ignore_default_args=["--enable-automation"]`, `locale="ja-JP"`,
  `timezone_id="Asia/Tokyo"` (`browser.py:153-159`); `proxy` from `proxy_kwargs(proxy)` (`:160-164`);
  `executable_path` for chrome/opera (`browsers.launch_kwargs`, `src/browser/browsers.py:83-94`; chromium = bundled
  Playwright build, `config.py:20-56` sets `PLAYWRIGHT_BROWSERS_PATH` to `<APP_ROOT>/browsers` if present).
- `disable_session_restore(profile_dir)` rewrites `Default/Preferences` `session.restore_on_startup=5`,
  `profile.exit_type="Normal"`, `exited_cleanly=True` and deletes `Default/Sessions/*`, `Current/Last Session|Tabs`
  (`browser.py:21-65`).
- `p.chromium.launch_persistent_context(str(profile_dir), **kwargs)` (`browser.py:171`), `close_extra_pages` (`:172`).
- Language cookies `user_language_code`, `i18next` are cleared right after launch
  (`browser.py:178-183`, names in `src/browser/cookie_import.py:87`).
- Back in `_launch_or_busy`: `hub_browser.ensure_unlock_without_extension(context, account)` (`video_worker_ui.py:137-141`)
  is a **no-op** because `config.UNLOCK_ROUTES` default `0` (`config.py:148`; `hub_browser.py:1007-1010`).
- `fresh_page(context, settle_ms=0)` picks the `about:blank` tab (`browser.py:80-86`).
- Session cache: `WarmBrowserManager._sessions[account] = {playwright, context, page, last_used, busy}`
  (`video_worker_ui.py:224-231`); reused if same mode & page open (`:196-208`); idle close after
  `WARM_SESSION_IDLE_TIMEOUT=900` s (`config.py:115`, `cleanup_idle` `:158-178`); `CLOSE_BROWSERS_WHEN_IDLE=1`
  closes everything 4 s after the last task (`server.py:570-593`).
- Window placement (`run_mode` show|corner|task, `config.py:321-325`) is applied only on the non-warm path
  (`video_worker_ui.py:2417-2426`); the warm path ignores it.

### 1.3 (b) `tabs` — one hub window + one incognito BrowserContext per account
- `launch_hub(p, kind)` (`hub_browser.py:424-450`): `launch_persistent_context("accounts/.hub_<kind>", **hub_launch_kwargs)`;
  `hub_launch_kwargs` (`:400-421`) = same args as profile, `headless=False`, extension only if
  `hub_extension_enabled()` (`HUB_EXTENSION=0` and `EXTENSION_ENABLED=0` → False, `:99-101`).
- `new_account_context(hub, account, cookies, proxy)` (`:681-703`): `hub.browser.new_context(locale="ja-JP",
  timezone_id="Asia/Tokyo", proxy?)`, `context.add_cookies(to_playwright(strip_dola_language(cookies)))`,
  `install_unlock_routes` (no-op), `new_page`, `tile_window` (CDP `Browser.setWindowBounds`, `:662-678`).
- Cookies come from `accounts/<name>/cookies.json` (see §2).

### 1.4 (c) `single` — one hub window, cookie swap per task, HTTP polling
- `swap_account` (`hub_browser.py:706-732`): `context.clear_cookies(domain=/(^|\.)dola\.com$/)`, `add_cookies`,
  `goto https://www.dola.com/`, `localStorage.clear(); sessionStorage.clear()`.
- After conversation_id: `detach_browser` (`video_worker_ui.py:303-324`) → `HttpSession` (`hub_browser.py:796-873`)
  polls `/im/chain/single` with aiohttp using the account cookies (§4.6).

### 1.5 (d) `wtabs` — ONE window, one real TAB per account, `extensions/lvc_relay`
- `launch_wtabs_hub` (`hub_browser.py:485-508`): `relay_launch_kwargs` (`:470-482`) ALWAYS adds
  `--disable-extensions-except=<abs extensions/lvc_relay>` + `--load-extension=<same>`; waits ≤8 s for a service worker.
- `open_relay_tab(hub, account, cookies)` (`:579-614`):
  1. `header = "; ".join(f"{c['name']}={c['value']}" for c in to_playwright(strip_dola_language(cookies)))` (`:585`)
  2. `page = hub.context.new_page()`; `goto https://www.dola.com/robots.txt` (timeout 30000, domcontentloaded) (`:594`)
  3. wait ≤8 s for `document.documentElement.getAttribute('data-lvc-relay-present')` (`:595`, `_relay_wait_attr :552-562`)
  4. `page.evaluate("(v) => window.dispatchEvent(new CustomEvent('LVC_SET_ACCOUNT', {detail: v}))", header)` (`:598`)
  5. wait for attribute `data-lvc-relay` starting with `ready:` (`:599`)
  6. `goto https://www.dola.com/chat` (60000) + `wait_for_timeout(3000)` (`:602-603`)
- `RelayTab.cookies()` (`:532-543`) asks the extension: dispatch `LVC_GET_COOKIE`, read `data-lvc-cookie-out` (`:565-576`).
- Submit-lock: `hub.lock` held from tab open until mode selected (`WTABS_PARALLEL=1`, `config.py:302`;
  `video_worker_ui.py:2486-2489`) or until conversation_id (`:2612-2613`).

### 1.6 What `extensions/lvc_relay` does exactly (v1.4.0)
`manifest.json` (`extensions/lvc_relay/manifest.json:1-43`): MV3, permissions `declarativeNetRequest`,
`declarativeNetRequestWithHostAccess`, `webRequest`, `tabs`, `storage`; host `*://*.dola.com/*`;
background `sw.js`; content scripts (both `document_start`, `all_frames`): `storage_isolate.js` with `"world": "MAIN"`,
`bridge.js` (isolated world).

`sw.js` (`extensions/lvc_relay/sw.js`):
- Per-tab cookie map `tabCookies: Map<tabId, Map<name,value>>` (`:11`), persisted to `chrome.storage.session` as
  `tab:<id>` (`:16-23`), restored on SW wake (`:25-37`, `:176`).
- Rule id 1: strip `set-cookie` **response** header for `||dola.com` on all resource types (`:74-89`), so the shared jar
  never gets Dola cookies.
- Rule id `1000+tabId`, priority 10: `modifyHeaders` requestHeaders `cookie` = `set` to the tab's header string,
  condition `urlFilter "||dola.com"`, `tabIds:[tabId]`, `RESOURCE_TYPES` (`:91-103`, `:9`).
- `webRequest.onHeadersReceived` (observe only, `["responseHeaders","extraHeaders"]`) merges real `Set-Cookie`
  values into the tab's map, re-applies the rule, persists, and pushes `LVC_COOKIE_UPDATE` to the tab (`:118-136`).
- Messages: `LVC_SET_ACCOUNT` (cookie header → map+rule), `LVC_GET_COOKIE`, `LVC_MERGE_COOKIE` (page wrote
  `document.cookie`), `LVC_PING` (`:138-172`); `tabs.onRemoved` clears the rule (`:174`).

`bridge.js` (isolated world, `extensions/lvc_relay/bridge.js`): listens for page `CustomEvent`s `LVC_SET_ACCOUNT`
(`:40`), `LVC_GET_COOKIE` (`:41`), `LVC_COOKIE_SET` (`:43-45`); relays to SW; on ack sets
`data-lvc-relay="ready:<tabId>"`/`"fail:…"` (`:18`), `data-lvc-cookie-out` (`:28`); pushes the tab cookie into the MAIN
world via `LVC_COOKIE_INIT` (`:8-10`, on load `:47-51`, on `LVC_COOKIE_UPDATE` `:35-39`); sets
`data-lvc-relay-present="1"` (`:53`).

`storage_isolate.js` (MAIN world, `extensions/lvc_relay/storage_isolate.js`):
- Replaces `window.localStorage` with an overlay object stored per key in the tab's own `sessionStorage`
  (`__lvc_ls:<k>`, tombstone `__lvc_lsdel:<k>`), seeded once from the real localStorage (`:21-71`);
  marks `data-lvc-storage="isolated"` (`:68`).
- Virtualises `document.cookie` per tab: getter serialises `ckMap` (kept in `sessionStorage["__lvc_ck__"]`), setter
  parses `name=value; …`, honours `max-age=0`/past `expires`, saves, and dispatches `LVC_COOKIE_SET` with the full
  header so the SW rule stays in sync (`:82-139`); seeded by `LVC_COOKIE_INIT` (`:99-116`), writes
  `data-lvc-cookie-js="<n>/<len>"` (`:112`).
- Does NOT touch IndexedDB / BroadcastChannel / request bodies.

**Is the extension required?** Only for `wtabs`. `profile`, `tabs`, `single` load **no** extension at HEAD
(`EXTENSION_ENABLED=0`, `HUB_EXTENSION=0`). The legacy `extensions/dola30` (Zakariya "zDola30s", manifest
`extensions/dola30/manifest.json`) is disabled by default (ADR-031, `docs/KIEN-TRUC.md:207-215`); its 30s trick no longer
works (ADR-036) and its `inject.js` breaks `/chat/completion` (710020202). A Rust single-account-per-Chromium port needs
**zero** extensions.

### 1.7 Which code path executes at HEAD
With `browser_mode=profile` (code default): `acquire → _launch_or_busy → launch_account_context` (persistent
profile, headed, no extension, no routes). Everything in §3-§6 below is written for this path; hub-specific deviations
are flagged `[hub]`.

---

## 2. Cookie source of truth

### 2.1 Profile mode: the Chromium profile jar itself
In `profile` mode the tool never reads `cookies.json`. It reads the live jar via Playwright:
`cookies = await context.cookies("https://www.dola.com")` — `video_worker_ui.py:2451` (existing conv) / `:2469` (new
chat) / `:1178` (`assert_logged_in`) / `:1485` (`poll_conversation`). `cookie_value(cookies, name)` (`browser.py:331-333`)
extracts `sessionid` (login check), `msToken`, `s_v_web_id` (passed as `fp`). Note: `POLL_JS` receives `msToken`/`fp`
but does not use them; the in-page `fetch(..., credentials: "include")` relies on the jar.

### 2.2 `accounts/<name>/cookies.json` (hub modes only)
- Path: `cookie_path()` → `accounts/<name>/cookies.json` (`hub_browser.py:39`, `:206-207`).
- Written by `save_cookies(account, cookies, keep_old_values)` (`hub_browser.py:265-279`): filters to domains containing
  `dola.com` (`_dola_only :210-211`), `to_playwright`, refuses to write without a `sessionid`, writes `.tmp` then
  `os.replace`. **Exact JSON shape**:
  ```json
  {"cookies": [ {"name": "...", "value": "...", "domain": ".dola.com", "path": "/",
                 "secure": true, "httpOnly": false, "expires": 1763000000.0, "sameSite": "Lax"} , ... ],
   "saved_at": 1758100000.123}
  ```
  Only the keys in `_PLAYWRIGHT_KEYS = ("name","value","domain","path","secure","httpOnly","expires","sameSite")`
  survive (`cookie_import.py:13`, `to_playwright :95-97`). Legacy bare-list files are also accepted (`load_cookies :223`).
- Read by `load_cookies(account)` (`:214-226`): returns `None` unless the list has a non-empty `sessionid`.
- Created on demand by `export_cookies_from_profile` (`:333-352`): opens the profile **headless, no extension, does NOT
  navigate to dola.com** (reading mid-load captured `flow_cur_user_sec_id=""`), `context.cookies("https://www.dola.com")`,
  saves, closes.
- `merge_keep_values(new, old)` (`:229-245`): empty new value keeps old value; new entries inherit
  `_COOKIE_META_KEYS = ("domain","path","expires","secure","httpOnly","sameSite")` (`:248`) from the old entry.
- `ensure_persistent(cookies, days=60)` (`:249-262`): missing/≤0 `expires` → `now + 60 d` (Chromium drops session
  cookies on close; measured 2026-09-17).
- `strip_dola_language(cookies)` (`cookie_import.py:87-92`): drops `user_language_code`, `i18next` before injecting.
- Write-back: `sync_cookies(account, context)` after every hub task (`hub_browser.py:294-301`; called
  `video_worker_ui.py:431,448`); `sync_relay_cookies` for wtabs (`hub_browser.py:617-624`); `write_cookies_to_profile`
  (`:304-330`) pushes the fresh set into the persistent profile after a wtabs task (`video_worker_ui.py:449-455`).
- Invalidate: `invalidate_cookies` (`:282-291`) on Verify/cookie reload/manual open (`browser_pool.py:576,595,868`).

### 2.3 Netscape `cookies.txt` import path
- `parse_netscape(text)` (`cookie_import.py:16-49`): 7 TAB columns
  `domain include_subdomains path secure expiry name value`; `#HttpOnly_` prefix → `httpOnly=True`; falls back to
  splitting on 2+ spaces; `expires` only if `expiry > 0`; then `fill_ss_aliases` (`:52-62`) copies `sessionid_ss` →
  `sessionid` etc. (httpOnly+secure) when the base name is missing.
- `validate_dola_cookies` (`:65-72`): needs ≥1 `*dola.com*` cookie and a non-empty `sessionid`.
- `add_account_from_cookies(account, cookies, proxy, browser)` (`src/browser/add_account.py:547-596`): creates
  `accounts/<account>/`, `launch_persistent_context(headless=True, args=LAUNCH_ARGS, locale ja-JP, tz Asia/Tokyo,
  proxy?)`, `context.add_cookies(to_playwright(dola))`, `goto https://www.dola.com/chat` (60000) + 6000 ms,
  `assert_logged_in` (§3.3); failure → profile dir deleted, `RuntimeError("COOKIE_NOT_LOGGED_IN: …")`.
- Reload into existing profile: `reload_account_cookies` (`add_account.py:599+`).

### 2.4 Login-state classification (pure)
`classify_session(has_sessionid, page_texts, has_composer)` (`src/utils/session_state.py:58-74`):
no sessionid → `logged_out`; any leaf label ≤40 chars starting with one of
`LOGIN_MARKERS = ("ログイン","Log In","Log in","Sign in","Sign In","登录","登入","ゲスト","Guest")` (`:9`, `_has_marker
:38-55`) → `logged_out`; else composer present → `logged_in`; else `unknown`.
`dola_self_logout(url)` = `"from_logout=1" in url` (`:27-30`). `is_auth_status(s)` = HTTP 401/403 (`:77-82`).

---

## 3. Submit flow, step by step (profile mode; `generate_video` `video_worker_ui.py:2384-2530`)

### 3.0 Pre-checks (before any I/O)
- `mode = normalize_mode(mode)` (`modes.py:186-195`); `timeout = video_timeout(mode, timeout or VIDEO_TIMEOUT, duration)`
  (`:2397`, `modes.py:278-285`); `model_key = _normalize_model(model)` (`:2398`, `:2106-2114`) → `seedance_v2.5 |
  seedance_v2.0 | seedance_v1.0`.
- `duration not in (10, 15, 30)` → `ValueError` (`:2399-2400`); `duration == 30 and (pro-like or model != 2.5)` →
  `ValueError` (`:2401-2403`).
- reference images → `timeout = max(timeout, REFERENCE_VIDEO_TIMEOUT=900)` (`:2404-2405`, `config.py:168`).
- ContextVars: `current_proxy` (download proxy), `current_duration`, `current_out_dir` (`:2406-2408`).

### 3.1 Acquire browser
`warm_browser_manager.acquire(account, headless=False, use_extension=True, proxy=proxy, browser=browser)` (`:2411-2413`) →
§1.2. `task_stage.set_stage("page")` (`:2433`). If the reused page is closed → `fresh_page` (`:2434-2440`).

### 3.2 Navigate
- **New chat** (`:2461-2467`): if reused page already at `https://www.dola.com/chat` → `wait_for_timeout(1000)`;
  else `page.goto("https://www.dola.com/chat", timeout=60000, wait_until="domcontentloaded")` then
  `wait_for_timeout(3000 if is_reused else 5000)`.
- **Existing conversation** (`conversation_id` given, `:2444-2460`): `goto f"https://www.dola.com/chat/{conversation_id}"`
  (60000, domcontentloaded) + `3000/4000` ms; then `assert_logged_in`; then a probe `page.evaluate(POLL_JS, …)` (timeout 10)
  to collect `ignore_videos = set(prior["videos"])`, `ignore_links = set(prior["links"])`.

### 3.3 `assert_logged_in(page, context, account)` (`:1170-1209`)
1. `ensure_dola_japanese` (`:1143-1167`): `DOLA_LANG_JS` (`:1019-1025`):
   ```js
   () => {
     const ls = (() => { try { return localStorage.getItem('i18nextLng') || ''; } catch (e) { return ''; } })();
     const ck = (document.cookie.match(/(?:^|;\s*)i18next=([^;]+)/) || [])[1] || '';
     const ul = (document.cookie.match(/(?:^|;\s*)user_language_code=([^;]+)/) || [])[1] || '';
     const jp = !![...document.querySelectorAll('button')].find(b => /高速|プロ/.test((b.innerText || '').trim()));
     return {ls, ck, ul, jp};
   }
   ```
   `dola_lang_needs_switch` (`:1098-1106`): language not starting with `ja` → switch. Switch = UI path
   `switch_dola_language_ui` (`:1109-1140`: click last leaf of `nav|aside` (`SIDEBAR_LAST_LEAF_JS :1087-1095`) →
   `text=/^(Settings|設定|Cài đặt|…)$/` (`:1085`) → row `text=/^(Language|言語|…)$/` (`:1086`) → click at
   `x+width-40` → `text=/^日本語$/` → Escape); fallback `DOLA_SET_JA_JS` (`:1026-1033`) which deletes
   `i18nextLng` and the two cookies; then `page.reload(domcontentloaded, 60000)` + 4000 ms.
2. `wait_dola_settled(page, timeout=25)` (`:1058-1077`): every 1 s evaluate
   `MODE_BTN_JS = "() => !![...document.querySelectorAll('button')].find(b => /^(高速|プロ)/.test((b.innerText || '').trim()))"`
   (`:1055`) via `_eval_retry` (`:1036-1052`, tolerates "Execution context was destroyed"); done when the button exists
   and `page.url` unchanged for 2 consecutive ticks.
3. `cookies = context.cookies("https://www.dola.com")`; `has_sid = bool(sessionid)` (`:1178-1179`).
4. If `has_sid`: `SESSION_JS` (`:1005-1016`) with timeout 15:
   ```js
   () => {
     const texts = [];
     for (const el of document.querySelectorAll('button, a, [role="button"], h1, h2, h3, div, span')) {
       const t = (el.textContent || '').trim();
       if (t && t.length <= 40 && el.childElementCount === 0) texts.push(t);
       if (texts.length > 400) break;
     }
     const composer = !!(document.querySelector('textarea') || document.querySelector('[contenteditable="true"]'));
     return {texts, composer};
   }
   ```
5. `state = classify_session(has_sid, texts, composer)`. If `logged_out` **with** sessionid → reload
   `https://www.dola.com/chat` once (+4000 ms) and re-classify (`:1189-1204`). Still `logged_out` →
   `raise AccountLoggedOutError(f"{account}: {why}")` (`:1205-1208`). `unknown` → continue.

### 3.4 Remember existing conversations (new chat only)
`known_conv_ids = set(await recent_conv_ids(page, cookies, account))` (`:2472`) → `RECENT_IDS_JS` (`:635-664`,
timeout 10):
```js
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
```

### 3.5 Pre-flight balance (`_preflight_balance`, `:985-1002`, required = `VIDEO_REQUIRED_POINTS=2`, `config.py:160`)
`BALANCE_JS` (`:544-628`, `__KEYWORDS__` replaced by `DOLA_TEXT["balance_keywords"] = ["ポイント","积分","points","上限","limit"]`
`modes.py:68`) does `POST /im/chain/recent_conv` (cmd 3200, limit 20, message_count_per_conv 10) then
`POST /im/chain/batch_single` (cmd 3101, `batch_pull_singe_chain_uplink_body`, first 10 conversation ids, limit 1),
walks `downlink_body.batch_pull_singe_chain_downlink_body.message_map[*].messages[*]` with `create_time >= sinceTs`
(`quota_day_start()`, `modes.py:21-25`, Tokyo midnight) and collects strings containing a keyword (<1200 chars).
`_parse_balance_texts` (`:973-982`): `DAILY_LIMIT_PATTERN` hit (non-JSON) → `AccountLimitedError`; `DOLA_TEXT["balance"]`
regexes (`modes.py:62-67`) e.g. `本日は残り\s*(\d+)\s*のみ` → balance; `< required` → `CreditInsufficientError`.
Any other exception → proceed (`:1000-1002`). Whole evaluate has 30 s timeout.

### 3.6 Choose mode — `select_mode(page, target_mode)` (`:1970-2050`)
`target_mode = "pro" if mode in ("pro_max","agent") else mode` (`:2475`). Lock per context (`_MODE_LOCKS`, `:42-51`).
Inside `_select_mode_locked`:
1. Cookie banner: `page.locator('button:has-text("OK"), button:has-text("Đồng ý")').first`; if visible → click
   (timeout 1500) + 500 ms (`:1987-1993`).
2. `_wait_mode_button(page, 8000)` (`:1925-1945`), candidates in order:
   - `page.locator("button", has_text=MODE_BTN_RE).first` where
     `MODE_BTN_RE = re.compile(r"(高速|Nhanh|Fast|プロ|Pro)", re.IGNORECASE)` (`:1854-1858`)
   - `page.locator("button", has_text=re.compile(r"(?:^|\s|\n)(高速|Nhanh|Fast|プロ|Pro)", re.IGNORECASE)).first`
   - `page.locator('button:has-text("高速"), button:has-text("Fast"), button:has-text("Nhanh"), button:has-text("Pro"), button:has-text("プロ")').first`
   First pass requires `count() and is_visible()`; second pass `wait_for(state="attached", timeout=min(t,5000))`.
3. Not found → `_clear_ability_tag(page)` (§3.9) and if something was cleared: click `div[contenteditable='true'], textarea`,
   `Control+A`, `Delete`, 800 ms, retry `_wait_mode_button(5000)` (`:1996-2012`).
4. Still not found → `_close_open_menus` (Escape while `[role="menu"]` exists, `:1845-1851`), `page.reload(60000,
   domcontentloaded)`, 5000 ms, `_wait_mode_button(15000)`, again ability-tag clear (`:2013-2026`).
5. Still none → `RuntimeError(await _mode_unavailable_message(page))` (`:2027-2028`) =
   `"MODE_UNAVAILABLE: Dola page shows no 高速/Nhanh/Fast/プロ/Pro mode button even after reload (url …; buttons seen:
   [...]; screenshot logs/mode_fail_<ts>.png)"` (`:1951-1967`, after `_raise_if_dola_logged_out`).
6. Read-back loop (2 tries, `:2030-2045`): `current = btn.inner_text(timeout=8000).strip()`;
   `_is_mode_match(current, mode)` (`:1867-1871`: fast ⇔ any of `("高速","Nhanh","Fast")`, pro ⇔ `("プロ","Pro")`,
   case-insensitive substring); match → return. Else `btn.click(timeout=5000)`, 700 ms, item locator
   `'[role="menuitem"], [role="option"], div[class*="menu-item"], div[class*="option"], button'` with
   `has_text=re.compile(r"(?:^|\s|\n)(高速|Nhanh|Fast)")` (fast) or `r"(?:^|\s|\n)(プロ|Pro)"` (pro) → `.first.click(5000)`,
   1500 ms, `_close_open_menus`, re-read.
7. Final mismatch → `RuntimeError(f"Cannot switch Dola mode to {mode} ({want}); button shows {current!r}")` (`:2047-2049`).
   For `mode == "agent"` the caller swallows this and continues (`:2480-2485`).

### 3.7 Fast submit — `_submit_fast(account, page, prompt, ratio, duration, model_key, refs, add_ref_src)` (`:2264-2301`)
1. `open_video_composer(page, "fast")` (`:2053-2089`):
   - Already open if any of: `page.locator("button, div, span").filter(has_text=DUR_BTN_RE).count()`,
     `page.locator("button", has_text=RATIO_BTN_RE).count()`, or tag `re.compile(r"動画を作成\s*[✕x×]")` → return.
     `DUR_BTN_RE = r"^\s*\d+\s*(?:s|秒)(?:\s*\(.*?\))?\s*$"`, `RATIO_BTN_RE = r"^\s*(比率|Tỷ lệ|Tỉ lệ|Ratio)(?:\s*\d+\s*:\s*\d+)?\s*$"`
     (`:1862-1864`).
   - Else 3 attempts: `loc = page.locator("button", has_text=VIDEO_BTN_RE).first` with
     `VIDEO_BTN_RE = r"^\s*(動画を作成|Tạo video|Create Video)\s*$"` (`:1859`); attempts 1-2 `loc.click(timeout=5000*attempt)`,
     attempt 3 `loc.evaluate("e => e.click()")`; fallback `page.click("text=動画を作成", …)` (`VIDEO_BTN`, `:539`,
     `UI_LABELS["video_btn"]` `modes.py:39`); 1500 ms; success if duration or ratio button appears (or attempt 3).
     Between attempts: `_close_open_menus`, `1500*attempt` ms. Failure →
     `RuntimeError("MODE_UNAVAILABLE: không mở được composer Nhanh (nút 動画を作成) sau 3 lần (…)")` after
     `_raise_if_dola_logged_out`.
2. `_clear_composer_attachments(page)` (`:794-833`): `ATTACH_CARD_SEL = '[class*="image-container-"] [class*="container-"]'`
   (`:790`); for each thumbnail: hover, 250 ms, bounding box, click at `(x+width-dx, y+dy)` for `(4,4),(6,6),(2,2),(8,8)`
   until count decreases (350 ms between).
3. `_attach_refs` (`:1682-1689`) → `attach_reference_images(page, paths)` (`:836-892`), see §3.10.
4. `_select_model(page, model_key)` (`:2136-2169`): button `page.locator("button", has_text=MODEL_BTN_RE)` with
   `MODEL_BTN_RE = r"(モデル|Model)\b"` (`:1863`), fallbacks `get_by_text(label, exact=True)` over
   `UI_LABELS["model_current"] = ("モデル 2.0高速","モデル 2.5","モデル 1.0","2.0 Nhanh","Seedance 2.5","Seedance 1.0")`
   (`modes.py:44`), then `get_by_text("モデル")`. If the version string (`"2.0"`, `"2.5"`, `"1.0"`) already appears in
   the button text → skip. Else click (5000), 500 ms, click first visible `get_by_text(option)` over
   `UI_LABELS["model_options"][model_key]` (`modes.py:45-50`: 2.5 → `("Dreamina Seedance 2.5","Seedance 2.5")`;
   2.0 → `("Dreamina Seedance 2.0高速","Dreamina Seedance 2.0 Nhanh","Dreamina Seedance 2.0","Seedance2.0Fast","2.0 Nhanh","2.0")`;
   1.0 → `("Dreamina Seedance 1.0","Seedance 1.0","Seedance1.0Fast","1.0高速","1.0")`). All errors swallowed (prints).
5. `_close_open_menus`; if `ratio`: `_select_ratio(page, ratio, tries=3)` (`:2176-2207`): button
   `page.locator("button", has_text=RATIO_BTN_RE).first`; if its text already contains `ratio` → done; else click (3000)
   (fallback `page.click("text=比率")`), 600 ms, `page.locator('[role="menuitem"]', has_text=re.compile(rf"^\s*{re.escape(ratio)}(?:\b|\s|$)")).first.click(3000)`,
   400 ms, close menus, re-read (2000). 3 failures → `_raise_if_dola_logged_out` then
   `RatioUnavailableError("RATIO_UNAVAILABLE: không chọn được tỉ lệ {ratio} … sau 3 lần (…)")`.
6. Duration (`:2274-2287`): `select_fast_duration(page, duration, account)` (`:1879-1922`): locator
   `page.locator("button, div, span").filter(has_text=DUR_BTN_RE).first`; up to 10 × 500 ms wait for the label to match
   (`_is_duration_match` `:1874-1876` = first `(\d+)\s*(?:s|秒)` equals duration); else click it (3000), 600 ms, pick
   `'[role="menuitem"], [role="option"], div[class*="menu-item"], button'` with `has_text=re.compile(rf"^\s*{int(duration)}s(?:\b|\s|$)")`,
   400 ms, re-read; mismatch → `DurationUnavailableError("DURATION_UNAVAILABLE: requested {d}s, selector shows {cur!r}")`.
   In `_submit_fast`: on `DurationUnavailableError`, if `duration in (15, 30)` → `select_fast_duration(page, 10)` then
   `install_duration_hook(page, duration)` (§7); else re-raise. At HEAD the fast composer shows `モデル / 10s / 比率`
   buttons (comment `:1997-1998`, ADR-044 E2E on 17/9 passed through `select_fast_duration(page, 10)`), so for 10 s this
   is a label read that already matches. Caveat: ADR-036 (16/9) once observed **no** duration button at all; if that
   recurs, `dur_btn.count()==0` → `current=""` → `DURATION_UNAVAILABLE` even for 10 s (`:1909-1920`). A port should treat
   "no duration button" as acceptable when the requested duration is 10 (the request body defaults to 10).
7. `_type_and_send(page, prompt)` (§3.8).
8. If hooked: poll `data-lvc-hook` 20 × 0.25 s for `patched:<N>` (§7).

### 3.8 Type & send — `_type_and_send(page, text, append=False, delay=100)` (`:2117-2133`)
```
task_stage.set_stage("prompt")
_close_open_menus(page)                       # Escape while [role="menu"] exists (max 3)
box = page.locator("textarea, [contenteditable=\"true\"]").first
box.click(timeout=10000)
append ? keyboard.press("End") : keyboard.press("Control+A"); keyboard.press("Delete")
_enter_text(page, text, delay)                # :1231-1239
wait_for_timeout(600)
keyboard.press("Enter")
```
`_enter_text`: `len(text) > config.TYPE_PASTE_THRESHOLD` (default **0**, `config.py:280`) → **always**
`page.keyboard.insert_text(text)` (Playwright `Input.insertText` CDP, one shot; no per-char typing; `\n` inside the
text goes through `insertText` as a line break in the editor, never as a synthesised Enter, so it cannot trigger send).
Otherwise `keyboard.type(text, delay=100)`.
There is **no send-button fallback** in `_type_and_send`; only `_send_chat_message` (`:1242-1262`, used for agent replies)
checks whether the box still has text after Enter and then clicks
`button[aria-label*="Gửi" i], button[aria-label*="Send" i], button[aria-label*="送信" i], button:has(svg)`.last.

### 3.9 Pro / Pro Max / Agent variants
- **pro** `_submit_pro` (`:2304-2311`): `open_video_composer(page, "pro")` (`:2091-2103`): button
  `re.compile(r"^\s*(スキル|Kỹ năng|Skills?)\s*$")` → click (8000), 1200 ms, `[role="option"]` with
  `re.compile(r"^\s*(動画の生成|Tạo video|Create Video)")` → click (8000), 1200 ms, close menus (fallback: `VIDEO_BTN_RE`
  button). Then clear attachments, attach refs, `text = pro_prompt(prompt, ratio, duration)` (`modes.py:366-390`:
  `"{d}秒の{縦動画（9:16）…}を1本作ってください。内容: {prompt}" + "。BGMなし、環境音のみ" (PRO_NO_BGM=1) +
  "。確認や質問は不要です。最適な設定を自動選択し、直ちに動画生成を開始してください。"`; ratio map `_RATIO_JA` `:179-183`),
  `_type_and_send(page, " " + text, append=True, delay=60)`.
- **pro_max** `_submit_pro_max` (`:2314-2323`): no skill; `pro_max_prompt` = `config.PRO_MAX_COMMAND ("/Create Video") + " " +
  pro_prompt(...)` (`modes.py:398-403`, `config.py:240`); `append=False`.
- **agent** `_submit_agent` (`:2370-2381`): `_clear_ability_tag` (`:2349-2367`) using
  `ABILITY_TAG_SEL = '[data-input-engine-action-source], [class*="exit-skill-btn"]'` (`:2329`) and `CLEAR_ABILITY_TAG_JS`
  (`:2330-2346`):
  ```js
  () => {
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
  }
  ```
  then clear attachments, attach refs, `text = agent_prompt(prompt, ratio, duration, model=model_key, style=AGENT_VIDEO_STYLE)`
  (`modes.py:421-451`) — the "DIRECTIVE/PARAMETERS" block:
  ```
  Create a {r} video (duration: {d}s).
  [PARAMETERS: DISPLAY_FORMAT={r}, DURATION={d}s, SCENE_COUNT=1, MODEL={m}{style_param}, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]
  DIRECTIVE: Use {m}{cost_note}. Do NOT ask for missing parameters. Do NOT ask for confirmation. Do NOT provide options A/B/C. Automatically apply optimal parameters and generate the video directly in ONE SHOT now. Return the finished video as the standard video card only - do NOT paste the video URL, markdown link or HTML <video> tag.

  Content:
  {prompt}
  ```
  `_type_and_send(page, text, append=False, delay=60)`.
- **real_person** (setting `real_person`, default off `config.py:227`): `_dola_recreate_image` (`:1588-1638`) runs the
  `画像を作成` skill in fast mode with the user's photos and `SHEET_PROMPTS["person"]` (`modes.py:145-150`), waits for a
  new `<img>` ≥900 px (`NEW_IMAGES_JS :1529-1531`), then `_add_image_as_reference` (`:1641-1679`) hovers the image and
  clicks the icon whose tooltip matches `ADD_REF_TIP_RE = r"参照|参考|reference|tham khảo|참조|참고"` (`:1548`).

### 3.10 Reference image upload — `attach_reference_images(page, paths)` (`:836-892`)
- Listens `page.on("response")` for URLs containing `/alice/resource/prepare_upload` or `/upload/v1/` (`:850-858`).
- `thumbs0 = document.querySelectorAll('img').length` (`COUNT_IMG_JS :710`).
- If `page.locator('input[type="file"]').first.count() > 0` (fast composer) → `set_input_files(image_paths)` (all at once,
  `way="input"`); else (pro/agent) per file `paste_image_once` (`:767-787`) + 700 ms.
- `paste_image_once`: injects `PASTE_MAIN_WORLD_SCRIPT` (`:730-753`) via `PASTE_INJECT_JS` (`:754-758`,
  `<script>` tag → MAIN world; `data-dolaPasteReady`), triggers with `window.postMessage({type:'dola-paste', b64, name, mime})`
  (`PASTE_TRIGGER_JS :759-763`), reads `data-dolaPasteResult` 20 × 100 ms. The MAIN-world handler builds a `File`, a
  `DataTransfer`, dispatches `ClipboardEvent('paste', {bubbles:true, cancelable:true})` on
  `document.querySelector('textarea, [contenteditable="true"]')` with `clipboardData` as a **read-once Proxy** (first
  reader of `files`/`items` gets the real DataTransfer, later readers get an empty one) — ADR-014, prevents ProseMirror's
  double upload. CSP block → fallback `PASTE_IMAGE_JS` (`:713-722`, isolated world, may upload twice).
- Completion signal (`:874-886`): loop every 250 ms until `prepare >= 1 and tos >= expected and thumbs >= expected`
  (2xx responses counted), then +800 ms; deadline `max(10, REFERENCE_UPLOAD_TIMEOUT=90)` s (`config.py:287`).
  Timeout → `_raise_if_dola_logged_out` then
  `ReferenceUploadError("attached k/N via {way} within 90s (prepare=…, tos=…, thumbs=+…)")` (`src/media/media.py`).
- Max count enforced upstream: `REFERENCE_IMAGE_MAX_COUNT=30` (`config.py:165`), `REFERENCE_IMAGE_MAX_BYTES=15 MiB`.
- MIME by suffix `MIME_BY_SUFFIX` (`:709`).

### 3.11 After Enter — `_after_submit` (`:2533-2616`)
1. `task_stage.set_stage("sending")`.
2. **Captcha wait** (`:2540-2561`): 3 attempts; each attempt waits up to **20 × 1000 ms** for an iframe whose URL contains
   `CAPTCHA_FRAME_KEY = "bdcaptcha.html"` (`:540`, `find_captcha_frame :692-696`). No frame → `solved_or_absent`.
   **Net effect: a fixed 20 s pause after Enter when there is no captcha.** Frame found → `solve_slider` (`:911-966`):
   waits `img` in frame (15000), both images loaded (10 s), fetches bg (`.jpeg` or `-2.`) and piece (`.png` or `-1.`) via
   aiohttp through the account proxy, `find_gap_x` (OpenCV, `src/utils/gap.py`), drags `.captcha-slider-btn` with a
   humanised track (`_gen_track :895-908`), success = frame gone within 10 × 700 ms. 3 failures →
   `page.screenshot("solve_fail.png")`, `RiskControlError("Captcha failed 3 times")`.
3. **Conversation id** (`:2563-2600`): `for i in range(30)`: `wait_for_timeout(1000)`; `tail = page.url.rstrip("/").split("/")[-1]`;
   `pick_new_conv_id(tail, known, recent)` (`:678-689`): digits and not in `known_conv_ids` → id; digits but in `known`
   (page jumped back to an old chat) → every 3rd tick query `recent_conv_ids` and take the first id not in `known`
   (`jumped=True` → `goto https://www.dola.com/chat/{conv_id}` + 2000 ms). No id after 30 s → screenshot
   `logs/no_conv_<account>_<ts>.png`, `_raise_if_dola_logged_out(page, account, …)` (§6 LOGGED_OUT/revoked), else
   `TimeoutError("NO_CONVERSATION: Dola khong tao hoi thoai sau 30s (url=…, anh=…)")`.
   **Acceptance = URL becomes `https://www.dola.com/chat/<digits>` with a *new* id.** No network interception, no DOM check.
4. `task_stage.set_stage("generating", conv_id)`; `on_conversation_id(account, conv_id, deadline=time.time()+timeout)`
   (server stores `conversation_id`, `deadline_at`; `server.py:612-618`).
5. `[hub single]` `detach_browser` → `HttpSession`; `[hub wtabs]` `release_submit_lock`.
6. `poll_conversation(...)` (§4).

---

## 4. Poll flow

### 4.1 Loop — `poll_conversation(account, page, context, conversation_id, timeout, on_poll, on_balance, accept_links, ignore_videos, ignore_links)` (`:1475-1525`)
- `cookies = context.cookies("https://www.dola.com")`; `ms_token`, `fp` (`:1485-1486`).
- `state = {"confirmed": False, "last_latest": [], "last_activity": start, "ignore_videos", "ignore_links"}`.
- `while True`: stop when `elapsed >= timeout` **and not** `should_extend(elapsed, timeout, PRO_MAX_TIMEOUT=3600,
  silence, PRO_ACTIVITY_WINDOW=600)` (`modes.py:361-363`: extend while `elapsed < max_timeout and silence < window`).
- `await asyncio.sleep(5)` — **poll interval 5 s** (`:1501`).
- `poll = await asyncio.wait_for(page.evaluate(POLL_JS, {"conversationId": conversation_id, "msToken": ms_token, "fp": fp}), timeout=30)`
  (`:1503-1504`). Exception text containing `has been closed` / `target closed` →
  `RuntimeError("BROWSER_CLOSED: browser or tab was closed while generating (…)")`; other exceptions → continue.
- `not poll["ok"] and is_auth_status(poll["status"])` (401/403) → `AccountLoggedOutError` (`:1512-1514`).
- `on_poll(now)` at most every 30 s (`:1516-1518`) → server updates `last_poll_at`.
- `result = _handle_poll(...)`; truthy → return; else print `...Generating (Ns)`.
- Timeout → `TimeoutError("No video generated within Ns (conversation_id=…) | agent: <last 400 chars of latest>")`.

### 4.2 Timeouts per mode (`modes.video_timeout`, `modes.py:278-285`; base `config.VIDEO_TIMEOUT=300`, `config.py:92`)
| case | timeout |
|---|---|
| fast 10 s, no refs | 300 s |
| any mode with reference images | `max(t, REFERENCE_VIDEO_TIMEOUT=900)` (`video_worker_ui.py:2404-2405`) |
| pro / pro_max / agent | `max(t, PRO_VIDEO_TIMEOUT=1800)`; extendable to `PRO_MAX_TIMEOUT=3600` while agent still "talks" within 600 s |
| duration ≥ 30 | `max(t, LONG_VIDEO_TIMEOUT=1500)` (`modes.py:275`) |

### 4.3 `POLL_JS` (verbatim, `src/video/video_worker.py:19-115`)
```js
async ({conversationId, msToken, fp}) => {
  // Current protocol: uplink_body.pull_singe_chain_uplink_body
  const params = new URLSearchParams({
    version_code: "20800", language: "ja", device_platform: "web",
    doubao_device_platform: "web", aid: "495671", real_aid: "495671",
    pkg_type: "release_version", pc_version: "3.32.61", doubao_pc_version: "3.32.61",
    region: "JP", sys_region: "JP", samantha_web: "1", web_platform: "browser",
    "use-olympus-account": "1", web_tab_id: crypto.randomUUID(),
  });

  const resp = await fetch("/im/chain/single?" + params.toString(), {
    method: "POST",
    headers: {
      "Content-Type": "application/json; encoding=utf-8",
      "agw-js-conv": "str",
      "Accept": "*/*",
    },
    body: JSON.stringify({
      cmd: 3100,
      uplink_body: {
        pull_singe_chain_uplink_body: {
          conversation_id: conversationId,
          anchor_index: Number.MAX_SAFE_INTEGER,
          conversation_type: 3,
          direction: 1,
          limit: 20,
          ext: {},
          filter: {index_list: []},
          evaluate_ab_params: "",
          evaluate_common_params: "",
        },
      },
      sequence_id: crypto.randomUUID(),
      channel: 2,
      version: "1",
    }),
    credentials: "include",
  });
  if (!resp.ok) return {ok: false, status: resp.status, texts: [], videos: []};

  const data = await resp.json();
  const messages =
    (((data.downlink_body || {}).pull_singe_chain_downlink_body) || {}).messages || [];
  const texts = [];
  const videos = [];
  const videoModels = [];
  const links = [];   // Pro: link video nằm trong text_block dạng markdown [動画を再生する](http://v16-dola.dola.com/...)
  const latest = [];  // các khối chữ của TIN MỚI NHẤT CỦA BOT (API trả tin mới nhất trước, khối trong tin theo thời gian)
  // 2026-09-17: messages[0] có thể là prompt của CHÍNH TOOL (user_type 1) ngay sau khi gửi → chữ "missing parameters"/
  // "confirmation" trong DIRECTIVE bị needs_confirmation() coi là agent hỏi → tool tự trả lời "C…" 2 lần trước khi agent kịp nói.
  // Chỉ lấy tin mới nhất có user_type 2 (bot); không có trường user_type (dữ liệu cũ/giả) → như trước (messages[0]).
  const hasType = messages.some(m => m && m.user_type !== undefined && m.user_type !== null);
  let latestIdx = hasType ? messages.findIndex(m => m && Number(m.user_type) === 2) : 0;
  const linkRe = /https?:\/\/[^\s)\]"'<>]+/g;
  for (const [mi, msg] of messages.entries()) {
    let content = msg.content;
    if (typeof content === "string") {
      try { content = JSON.parse(content); } catch (e) { continue; }
    }
    if (!Array.isArray(content)) continue;
    for (const block of content) {
      const text = (((block.content || {}).text_block) || {}).text || "";
      if (text) {
        texts.push(text.slice(0, 1000));
        if (mi === latestIdx) latest.push(text.slice(0, 4000));
        let m;
        while ((m = linkRe.exec(text)) !== null) {
          const u = m[0];
          if (/mime_type=video_mp4|\.mp4(\?|$)|\/video\//i.test(u) && !links.includes(u)) links.push(u);
        }
      }
      if (block.block_type !== 2074) continue;
      const creations = (((block.content || {}).creation_block) || {}).creations || [];
      for (const cre of creations) {
        if (cre.type !== 2) continue;
        const url = ((cre.video || {}).download_url) || "";
        if (url.startsWith("http")) {
          videos.push(url);
          videoModels.push((cre.video || {}).video_model || "");
        }
      }
    }
  }
  if (latest.length === 0) {
    try {
      const items = document.querySelectorAll('[class*="message"], [class*="chat-item"], [data-message-id]');
      if (items && items.length > 0) {
        const lastEl = items[items.length - 1];
        const t = (lastEl.innerText || "").trim();
        if (t) latest.push(t.slice(0, 4000));
      }
    } catch (e) {}
  }
  return {ok: true, status: resp.status, texts, videos, videoModels, links, latest};
}
```
JSON fields read: `downlink_body.pull_singe_chain_downlink_body.messages[]` → `user_type` (2 = bot), `content`
(JSON string or array of blocks) → `block.block_type == 2074` → `block.content.creation_block.creations[]` with
`type == 2` → `video.download_url`, `video.video_model` (JSON string containing `video_list`, `fallback_api`, `key_seed`);
`block.content.text_block.text` for texts/links.

### 4.4 Per-poll decision — `_handle_poll(account, page, poll, state, conversation_id, on_balance, accept_links)` (`:1275-1429`)
Order of checks (first hit wins):
1. Activity: `latest` changed → `state["last_activity"] = now` (`:1280-1283`).
2. Balance: every text through `_parse_balance_texts` → `on_balance(balance, source)` (`:1285-1288`).
3. For each text in `latest` (skip `looks_like_json`, `modes.py:28-31`): `classify_dola_text` (`:84-95`):
   - `DAILY_LIMIT_PATTERN` (`modes.py:57-61`, e.g. `動画生成の\s*1日あたりの上限`) → `"limit"` → `AccountLimitedError`.
   - `GENERIC_REFUSAL_RE` (`:69-71`) or `policy_rejection` (`modes.py:333-340`, `DOLA_TEXT["policy_reject"] :109-126`)
     **and not** `EXPLICIT_CREDIT_RE` (`:72-73`) → `"refusal"` → `PromptViolationError(REFUSAL_NOTE + text[:200])`.
   - `CREDIT_FAIL_RE` (`:62-64` = `dola_client.CREDIT_FAIL_PATTERN` `src/api/dola_client.py:39-45` +
     `DOLA_TEXT["credit_fail"]` `modes.py:71-75` + `["生成できません","残高不足"]`) → `"credit"` → `CreditError("Insufficient quota: …")`.
4. DOM scan (`:1300-1320`, skipped when reusing a chat): `((document.querySelector('main') || document.body).innerText || '').slice(-3000)`
   → same classification (`(DOM)` suffix).
5. Filter old items (`:1321-1343`): `video_ids()` (`VIDEO_ID_RE = r"\b(v[0-9a-z]{10,})\b"`, `:1432-1437`) and `link_key()`
   (host + path from `/video/`, `:1440-1456`) against `ignore_*`.
6. No videos & no links: `policy_rejection(latest)` → `PromptViolationError` (portrait hint via `PORTRAIT_RE :76`) (`:1344-1353`).
7. No videos & `needs_confirmation(latest)` (`modes.py:288-305`: `confirm_request`, `choice_request`, `question`,
   `duration_request` regexes `modes.py:80-139`) → `_send_chat_message(page, get_confirmation_reply(latest, duration))`
   (`modes.py:308-322`; duration question → `"{d}秒 ({d} seconds). {d}秒の動画を生成してください。"` `:161-170`), keyed by
   `confirm_key` sha1 of latest, max `CONFIRM_MAX_REPLIES=3` (`config.py:292`), stage `confirm`, return None (`:1354-1368`).
8. `agent_question(latest)` (`modes.py:350-358`, regex `[？?]\s*$|…A[.．]…B[.．]`) held ≥ `AGENT_QUESTION_GRACE=60` s →
   `AgentQuestionError("AGENT_QUESTION: …")` (`:1369-1375`).
9. `AGENT_IDLE_FAIL=120` s (`config.py:208`): latest is text, not the tool's own prompt (`"DIRECTIVE:" in text and
   "Content:" in text`), idle ≥120 s, and no `DOLA_TEXT["generating"]` match (`modes.py:128-132`) →
   `RuntimeError("AGENT_NO_VIDEO: …")` (`:1376-1384`).
10. `videos` non-empty → **done** (§5).
11. `links` non-empty → `_raw_for_link` (§5) else wait `LINK_RAW_GRACE=45` s (`config.py:295`), optional preview ask
    (`LINK_PREVIEW_ASK=0` default), then download the link (`:1394-1428`).
12. else → None (keep polling).

"Still generating" is therefore the absence of all of the above; there is no explicit progress field read.

### 4.5 Network interception during polling
None. Polling is purely the in-page `fetch("/im/chain/single")` above (isolated world evaluate; the page's own
cookies/jar are used via `credentials: "include"`). No `page.route`, no CDP `Network.*` hooks.

### 4.6 `[hub single/wtabs-detached]` HTTP polling — `HttpSession.poll` (`hub_browser.py:853-873`)
`aiohttp.ClientSession(cookies={name: value}, timeout=40, headers={"User-Agent": POLL_UA, "Accept": "*/*",
"Accept-Language": "ja-JP,ja;q=0.9", "Origin": "https://www.dola.com", "agw-js-conv": "str"})` (`:817-823`,
`POLL_UA` `:747` = Chrome/153 UA); `POST https://www.dola.com/im/chain/single` with `POLL_PARAMS` (`:748-750`) +
`web_tab_id=uuid4`, same JSON body as POLL_JS (`anchor_index: 2**53-1`), headers `Content-Type: application/json;
encoding=utf-8`, `Referer: https://www.dola.com/chat/<id>`. HTTP ≠200 → `{ok:false,status}`; body `status_code`
∉ (0, None) → `{ok:false, status: 401 if code in (712012001,) else 500}` (`LOGIN_INVALID_CODES :746`); else
`parse_chain(data)` (`:755-793`, Python twin of POLL_JS). Cookie jar updates are merged back (`:867-868`).
**Proven 2026-09-15: `/im/chain/single` works with cookies alone (no a_bogus/msToken signing)** — ADR-029
(`docs/KIEN-TRUC.md:186-195`). This is the cleanest polling primitive for a Rust port (reqwest + cookie jar).

---

## 5. Download

### 5.1 URL selection (`_handle_poll`, `:1385-1428`)
1. `videos` present: `raw = await resolve_raw_video(video_models[0])` (`src/media/unwatermark.py:243-251`,
   `RAW_UNWATERMARK=1`): `find_fallback_apis(video_model)` (`:51-83`, key `fallback_api`, host
   `vod-urls-mya.byteintlapi.com/video/fplay/...`), for the first ≤3: `fetch_raw_video(api)` (`:225-240`) = GET
   `replace_query(api, {"channel":"no","codec_type":"8","logo_type":"unwatermarked"})` (`RAW_PARAMS :26`) with
   `headers={"accept":"application/json,text/plain,*/*"}`, **no cookies**, timeout 30, through the account proxy →
   `pick_raw_video` (`:202-222`) chooses best `video_list[*]` by `bitrate + w*h`, decoding `main_url` via base64 or the
   `qAAB` AES-CBC scheme (`decode_qaab :153-171`: key/iv = `SHA512(SHA512(key_seed[:32]) + SALT)[:16]/[16:32]`, payload
   after 4-byte `a8 00 01 00`, `QAAB_SALT_HEX :24-25`). Success → `_download_result(..., " (raw, no logo)")`.
   Fallback: `extract_unwatermarked_url(video_model, videos[0])` (`video_worker.py:118-141`: base64-decode each
   `video_list[*].main_url`, pick highest `bitrate`/`real_bitrate`; else `download_url`) → `" (unwatermarked priority)"`.
2. `links` only: `_raw_for_link(links[0], videoModels)` (`:1459-1472`) matches the `v…` id in the link against the
   `video_model` strings (newest last) → raw; else after 45 s grace download `links[0]` → `" (link in text)"`.

### 5.2 Bytes — `_download(url, account, proxy=None, out_dir=None)` (`src/video/video_worker.py:170-187`)
```python
dl_dir = Path(out_dir) if out_dir else Path(config.DOWNLOAD_DIR)      # "downloads"
fname = dl_dir / f"{account}_{time.strftime('%Y%m%d_%H%M%S')}.mp4"
timeout = aiohttp.ClientTimeout(total=300)
server, user, pw = aiohttp_proxy_parts(resolve_proxy(proxy))         # account proxy via ContextVar current_proxy
async with aiohttp.ClientSession(timeout=timeout) as session:
    async with session.get(url, proxy=server, proxy_auth=auth) as resp:
        resp.raise_for_status()
        ... iter_chunked(1 << 16) → f.write
```
- Plain aiohttp GET: **no cookies, no Referer, no User-Agent override** (CDN URLs are pre-signed: `x-expires`,
  `x-signature`). Not `page.request`, not the extension, not `<a download>`.
- Called from `_download_result` (`video_worker_ui.py:1265-1272`): stage `downloading` (dashboard label
  `web/lang/vi.json:281` "Có video — đang tải về" / `en.json:281` "Video ready — downloading", 92 % in
  `web/index.html:1673` `STAGE_PCT`) — this is the "receiving video data" step. Returns
  `{"video_url", "local_path", "conversation_id", "account"}`; `generate_video` adds `"watermarked": is_watermarked_url(url)`
  (`src/media/postprocess.py:58-63`: `"watermark" in url and not "unwatermarked"`).
- **Size sanity: none** (only `local.stat().st_size / 1e6` printed, `:1271`). `raise_for_status` is the only check.
- Output dir: `out_dir` from `server._task_out_dir` (`server.py:422+`; project dir or `settings.output_dir`), default
  `downloads/`.

### 5.3 Post-processing — `server._finish_task` (`src/core/server.py:442-487`)
- `should_remove_logo(logo, video_url)` (`postprocess.py:66+`): logo `auto` (default for API/scenes,
  `server.py:350`, `scenes.py:12`) always tries `remove_logo` (`src/media/dewatermark.py`: ffmpeg `fps=2` sampling +
  `cv2.matchTemplate` on `assets/dola_watermark.png`, threshold 0.24, then ffmpeg `delogo` segments; ADR-043) and
  re-verifies up to `LOGO_MAX_PASSES=2`.
- `WEB_H264=1` → `ensure_web_codec` (`postprocess.py:192-208`): ffprobe codec; HEVC → `<stem>_h264.mp4` via ffmpeg
  (`FFMPEG_BIN` = `<APP_ROOT>/bin/ffmpeg.exe` if present, `config.py:258-264`). Failures → still `completed` with
  `LOGO_SKIPPED`/`H264_SKIPPED` note.
- `store.update(status="completed", video_url=public_url, local_path, …)`.

---

## 6. Failure taxonomy → detection rule → what happens next

Mapping to `failure_code` happens in `server._run_task` (`server.py:596-719`): explicit `except` branches
(`:674-697`) and generic `_failure_code_from(e)` (`:413-419`) which scans the message for a known prefix `CODE:`
from `KNOWN_FAILURE_CODES` (`:404-406`). Pool-level rotation is in `BrowserPool.generate_video`
(`browser_pool.py:1090-1287`). Auto-retry (`server.py:722-777`): after any failure, `should_auto_retry(row, limit)`
→ `limit = settings.auto_retry` clamped 0..5, default `AUTO_RETRY_DEFAULT=3` (`config.py:224`); skipped when
`failure_code in AUTO_RETRY_SKIP_CODES = ("cancelled","429","DOLA_REVOKED_STREAK","ACCOUNT_UNAVAILABLE")` or
`auto_retries >= limit`; otherwise after `AUTO_RETRY_DELAY=8` s (`config.py:228`) → `store.mark_retry` (new chat,
`store.py:366-374`) + `_run_task(..., fresh_chat=True)`.

| code | trigger (exact) | where | worker/pool reaction | auto_retry |
|---|---|---|---|---|
| `cancelled` | user `DELETE`/stop → `store.mark_cancelled` (`store.py:254-260`); `_run_task` checks `is_cancelled` before opening browser (`server.py:608`) and in `on_conversation_id` (`:613-615`, raises `asyncio.CancelledError`) | server | coroutine cancelled; conv id kept for "Resume" | no (skip list) |
| `LOGGED_OUT` | (a) `assert_logged_in` → `AccountLoggedOutError` (no `sessionid` cookie, or a leaf label matching `LOGIN_MARKERS` after one reload; `video_worker_ui.py:1205-1208`); (b) `_raise_if_dola_logged_out` (`:1212-1228`): `page.url` contains `from_logout=1` **or** `SESSION_JS` classifies `logged_out` → `revoked=True`; called before RATIO/MODE/REF_UPLOAD/NO_CONVERSATION are raised; (c) polling HTTP 401/403 (`:1512-1514`); (d) `HttpSession` body `status_code 712012001` → 401 | worker | pool `mark_logged_out(name, reason, revoked)` (`browser_pool.py:643-666`): `login_ok=0`, close warm session; non-revoked → `_auto_verify` (`AUTO_VERIFY_RETRIES=2`, delay 20 s, `connect_account_flow` = Google silent relogin); revoked → `auto_verify_state='revoked'`, no verify, `_note_revoked` (circuit breaker `REVOKED_STREAK=3` in `REVOKED_WINDOW=600` s → pause new tasks `REVOKED_PAUSE=600` s, code `DOLA_REVOKED_STREAK`, `:668-686`). Pool then rotates to the next candidate (`:1192-1197`); none left → `AllAccountsLoggedOutError` → server `store.requeue(code LOGGED_OUT, retry_at=now+REQUEUE_INTERVAL=120)` up to `REQUEUE_MAX=3`, then failed `LOGGED_OUT` (`server.py:681-691`) | yes (if final failed) |
| `429` | pool exhausted: `AllAccountsLimitedError` (`all_accounts_limited_for(mode)`, `browser_pool.py:1013-1019`) / `AllAccountsQuotaBlockedError` (`:1026-1032`) raised at `:1279-1286`; also early 422/429 in `_create_task` (`server.py:985-989`) | pool | `_run_task` walks `mode_fallback_chain` (agent→pro→pro_max→fast, `modes.py:243-258`, `MODE_FALLBACK=1`) with `fit_duration_to_mode` (`:234-240`) before giving up (`server.py:651-672`); final → failed `429`. Retry endpoint requeues at `next_limit_reset()` (Tokyo 00:00, `browser_pool.py:448-457`; `server.py:2148`) | no |
| `ACCOUNT_UNAVAILABLE` | `PreferredAccountUnavailableError` (`browser_pool.py:169-173`): chosen account missing, or `_hard_blocked` (`login_ok==0`, scheduling off, cooling; `:916-919`) — soft states (limits/credit) are **ignored** when `PREFER_ACCOUNT_FORCE=1` (`:1108-1121`); or any per-account error when a preferred account was set (`:1271-1273`) | pool | failed, no rotation (ADR-011) | no |
| `PROMPT_VIOLATION` | `PromptViolationError` (`modes.py:325-330`) from `_handle_poll`: `classify_dola_text == "refusal"` (latest or DOM `<main>`), or `policy_rejection(latest)` when no video/link (`video_worker_ui.py:1295-1297, 1312-1314, 1348-1353`); regexes `DOLA_TEXT["policy_reject"]` (`modes.py:109-126`), `GENERIC_REFUSAL_RE` (`video_worker_ui.py:69-71`), `PORTRAIT_RE` (`:76`) | worker | `server.py:692-694` failed; `DEAD_CHAT_CODES` → any retry opens a new chat (`server.py:407`) | yes |
| `REF_UPLOAD` | `ReferenceUploadError` from `attach_reference_images` timeout (`:888-890`) or paste `nobox`/`error` (`:871-872`, `:785`) | worker | `server.py:695-697` failed | yes |
| `PROFILE_BUSY` | `ProfileBusyError` (`:110-125`): launch error text containing `"profile is already in use"`, `"Opening in existing browser session"`, `"already in use by another instance"` (`_PROFILE_BUSY_MARKERS :120`) | worker launch | pool sets `cooldown_until = now + PROFILE_BUSY_COOLDOWN=300` (`browser_pool.py:1244-1261`), rotates; none left → `RuntimeError("PROFILE_BUSY: no account with a free browser profile …")` (`:1274-1275`) | yes |
| `PROXY_DOWN` | `proxy_precheck` (`browser_pool.py:734-750`): GET `PROXY_TEST_URL` (`https://api.ipify.org?format=json`, timeout 15) through the account's own proxy fails (cached `PROXY_PRECHECK_TTL=600`) → `ProxyDownError` (`:33-40`) | pool | skip account without launching, rotate (`:1147-1150`) | yes |
| `DURATION_UNAVAILABLE` | `DurationUnavailableError` (`:535-536`): `select_fast_duration` label mismatch (`:1918-1920`) or click error (`:1913-1916`) for durations other than 15/30; hook not installable (`:2284-2285`); hook did not report `patched:<N>` within 5 s after Enter (`:2296-2299`) | worker (fast) | generic branch → failed with code; before Enter → no credit spent | yes |
| `RATIO_UNAVAILABLE` | `RatioUnavailableError` after 3 tries (`:2206-2207`) | worker (fast) | failed | yes |
| `AGENT_QUESTION` | `AgentQuestionError` (`:106-107`): `agent_question(latest)` non-empty for ≥ `AGENT_QUESTION_GRACE=60` s (`:1369-1373`) | worker poll | failed; dead chat → retry = new chat | yes |
| `AGENT_NO_VIDEO` | `RuntimeError("AGENT_NO_VIDEO: …")` idle ≥ `AGENT_IDLE_FAIL=120` s (`:1376-1384`) | worker poll | failed; dead chat | yes |
| `MODE_UNAVAILABLE` | `select_mode` finds no mode button after reload (`:2027-2028`, message `:1966-1967`); `open_video_composer` cannot open the fast composer after 3 tries (`:2089`). Note `"Cannot switch Dola mode to …"` (`:2049`) has **no code** → `failure_code=None` | worker | failed (agent mode ignores mode-switch errors, `:2480-2485`) | yes |
| `NO_CONVERSATION` | `TimeoutError("NO_CONVERSATION: …")` after 30 s without a new numeric id in URL (`:2588-2600`) (preceded by `_raise_if_dola_logged_out`) | worker | pool `except TimeoutError` → `_claim(account, mode)` (counts a use!) + re-raise (`browser_pool.py:1231-1239`) | yes |
| `RESTART` | at startup `store.requeue_orphans` (`store.py:385-397`): `processing` rows without `conversation_id` → `queued`, code `RESTART` (`server.py:903-904`); rows with conversation_id → `_resume_task` (`:823-859`, `resume_video` `video_worker_ui.py:1787-1842`) | server | re-dispatched immediately | n/a |
| `DOLA_REVOKED_STREAK` | `pool.revoked_pause_reason()` non-empty (`browser_pool.py:679-686`) raised as `RuntimeError` before/while picking (`:1103-1105, 1129-1131`) | pool | failed fast, no browser | no |
| `REAL_PERSON` | `RealPersonError` (`:1565-1566`) from `_dola_recreate_image`/`_add_image_as_reference` | worker | failed | yes |
| (no code) timeout | `TimeoutError("No video generated within …")` (`:1524-1525`) | worker poll | pool `_claim` + `last_used_at`, re-raise; server failed with `failure_code=None`; "Resume" possible (conversation_id kept) | yes |
| (no code) `RiskControlError` | captcha failed 3× (`:2561`), or old worker `_check_submit` 710022004/slide/shark/710022002 (`video_worker.py:148-167`, dead path) | worker | pool `cooldown_until = now + COOLDOWN_SEC=1800` (`browser_pool.py:26`, `:1218-1230`), rotate | yes |
| (no code) `AccountLimitedError` | `classify_dola_text == "limit"` in poll (`:1293-1294, 1310-1311`) or preflight (`:991-992`) | worker | pool `_mark_daily_limit(account, reason, mode)` = usage forced to limit + `rate_limited_until = next reset` (`browser_pool.py:476-488`), close session, rotate | (429 after exhaustion) |
| (no code) `CreditError` | `classify_dola_text == "credit"` (`:1298-1299, 1315-1316`) | worker | pool: if text matches `CREDIT_QUOTE_RE` (`browser_pool.py:161`: `クレジットが使用されます|本日は残り\s*\d+\s*のみ|パラメーターを変更してもう一度`) resend in a **new chat, same account** up to `CREDIT_QUOTE_RETRIES=2` (`:1164-1185`); else `_claim` + rotate (`:1213-1217`) | yes |
| (no code) `CreditInsufficientError` | preflight balance < 2 (`:993-996`) | worker | `_mark_quota_blocked` (until next reset), rotate (`:1198-1202`) | — |
| `BROWSER_CLOSED` | poll evaluate raises "has been closed"/"target closed" (`:1506-1509`) — not in `KNOWN_FAILURE_CODES` → `failure_code=None` | worker | failed | yes |

Prefix convention: every coded error's `str()` starts with `"<CODE>: "` (e.g. `AccountLoggedOutError` → `"LOGGED_OUT: …"`
`session_state.py:19-24`).

---

## 7. 30 s duration fetch hook (ADR-044, `docs/KIEN-TRUC.md:507-532`)

### 7.1 What is injected (verbatim `DURATION_HOOK_JS`, `video_worker_ui.py:2215-2242`; `__DUR__` → int)
```js
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
```
(Python raw string: the first regex literally targets the escaped form `\"duration\":10` inside the nested
`chat_ability.ability_param` JSON string; the second targets a bare `"duration":10`.)

### 7.2 World and moment
- `install_duration_hook(page, duration)` (`:2249-2254`):
  `page.evaluate("(src) => { const s = document.createElement('script'); s.textContent = src; document.documentElement.appendChild(s); s.remove(); }", duration_hook_script(duration))`
  → executes in the page **MAIN world** (script element), even though `evaluate` itself runs isolated.
- Called from `_submit_fast` **after** `select_fast_duration(page, 10)` and **before** `_type_and_send` (`:2282-2288`),
  i.e. when the Dola SDK is already loaded and has installed its own `fetch` interceptor (which computes `a_bogus`/signs
  the body). Our wrapper is therefore the *outermost* `fetch`, so the body is modified **before** the SDK signs it.
  Installing at `document_start` (add_init_script / page.route) does not work (ADR-044 items 1-2: replay or post-sign
  edits → `710022002`).
- Rewritten request: `POST /chat/completion` body `chat_ability.ability_param` (nested JSON string
  `{"ratio":"16:9","model":"seedance_v2.5","duration":10,"input_box_content":{…}}`, ADR-037) → `duration: N`.
- Verification: after Enter, poll `document.documentElement.getAttribute('data-lvc-hook')` 20 × 250 ms for
  `patched:<N>` (`:2289-2300`); otherwise `DURATION_UNAVAILABLE`.
- Server side: `validate_generation` → `modes.route_30s` (`modes.py:216-231`) rewrites any 30 s request to
  `mode=fast, model=seedance-2.5`; `allowed_durations(mode, model)` (`:206-213`) = fast `(10,15)` + 30 with 2.5;
  pro-like `(10,15)` (agent/pro 15 s are asked for in the prompt text, not hooked).

### 7.3 Needed for 10 s / 15 s?
- **10 s: NOT needed** — the UI's default selector shows `10s`; `select_fast_duration` returns without the hook.
- **15 s in fast mode: needed** (same code path as 30 s; `if duration not in (15, 30): raise` `:2279-2280`).
- 15 s in pro/agent: no hook; written into the prompt (`pro_prompt` / `agent_prompt`).
- Credits measured (ADR-044): 2.0 10 s = 2, 2.5 10 s = 4, 2.5 30 s = 2; 30 s ≈ 5 min without refs, ~12-15 min with refs.

---

## 8. Scheduler

- **Threads**: `DynamicLimiter(threads, accounts_fn)` (`browser_pool.py:53-158`); `threads` from `settings.threads`
  (`:273-277`) else `config.MAX_CONCURRENCY=3` (`config.py:86`, `server.py:67`); `effective() = max(1, min(threads,
  max(1, enabled_accounts), hub_browser.max_parallel()=100))` (`:78-88`); `enabled_accounts` = accounts_meta rows with
  `scheduling=1` whose dir exists (`:305-309`); recomputed on **every** `acquire` wait predicate (`:122`) and on
  `set_threads` notify (`:90-109`). FIFO by ticket `(created_at, seq)` (`:111-129`; `slot(order)` `:131-142`).
- **Dispatch**: every task (`POST /v1/videos/generations`, `/api/admin/generate`, `/api/admin/batch` scene) becomes one
  `asyncio.create_task(_run_task(...))` immediately (`server.py:1022-1025`, batch `:2260`); throttling happens inside
  `pool.generate_video` → `async with self.limiter.slot(queue_order)` (`browser_pool.py:1106`).
- **Account selection**: `start_account = pool.pick_account(mode)` (`server.py:633-634`) →
  `candidates_for(mode)` = `list_accounts()` filtered by `_schedulable` (`:909-914`: scheduling on, not cooling, not
  rate_limited, not quota_blocked, `used < limit` for mode, `login_ok != 0`, not `_credit_blocks`); `ACCOUNT_PICK`
  (`config.py:201`, default `rotate`) → `min(fresh, key=_spread_key)` where
  `_spread_key = (used_today(fast), last_used_at, name)` (`:935-938`), excluding accounts picked in the last
  `PICK_RESERVE=20` s (`:953-972`). Then `candidates_for(mode, start_account)` puts it first and sorts the rest by
  `_spread_key` (`:940-951`). `prefer_account` (scene `account`) → only that account (`:1108-1121`).
- **Per-account lock**: `self._locks[name] = asyncio.Lock()` held for the whole generate (`:1132-1138`); busy accounts
  skipped (unless preferred) and the loop waits up to `BUSY_WAIT_MAX=600` s polling every 0.5 s (`:1262-1267`).
- **Daily limits per mode** (`modes.daily_limit`, `modes.py:261-270`): fast `DAILY_LIMIT_FAST=30`, pro
  `DAILY_LIMIT_PRO=10`, pro_max `DAILY_LIMIT_PRO_MAX=10`, agent `DAILY_LIMIT_AGENT=30` (`config.py:175-176, 238, 243`).
  Usage table `usage(account, day, mode, used)`; day key = date in `LIMIT_RESET_TZ=Asia/Tokyo` (`:443-446`);
  `_claim` after success / CreditError / TimeoutError (`:1186, 1215, 1234`).
- **Cooldowns**: `rate_limited_until = next Tokyo midnight` on `AccountLimitedError` (`_mark_daily_limit :476-488`);
  `quota_blocked_until = next reset` on credit `< 2` (`_set_credit_balance :881-891`, `_mark_quota_blocked :469-474`);
  `cooldown_until = now+1800` on `RiskControlError`; `now+300` on `ProfileBusyError`; cleared by
  `_clear_expired_rate_limits` (`:459-467`) on each `list_accounts`.
- **Mode fallback** on exhaustion: `server.py:651-672` (§6 `429`).
- **Batch of scenes**: `/api/admin/batch` (`server.py:2222-2268`) normalises each scene (`src/utils/scenes.py`,
  defaults `ratio 9:16, duration 10, model seedance-2.0, mode agent, logo auto` `:12`) and creates one task per scene with
  `batch_id`/`scene_index`; scenes run in parallel up to the limiter; `BATCH_REUSE_CHAT=0` (default, `config.py:250`,
  setting `batch_reuse_chat`) so each scene opens a new chat on the least-used account; `_pick_batch_context`
  (`server.py:548-567`) only reuses the batch's chat/account when enabled and that account is idle.

---

## 9. Region / proxy

- Region block URL: `DOLA_REGION_RESTRICTED = "/security/region-restricted"`; `is_region_restricted(url)`,
  `check_dola_region(page)` (goto `/chat`, 3 s, check URL) — `browser.py:190, 227-240`. Used only for Opera VPN flows
  (`vpn_after_add :243-285`, `apply_opera_vpn :200-224` writes `freedom.proxy_switcher{enabled,country_code:"AS"}`).
  The worker itself does not check region; a region block simply surfaces as MODE_UNAVAILABLE/LOGGED_OUT.
- Locale/timezone are always `ja-JP` / `Asia/Tokyo` (all launch paths) — Dola labels are Japanese; the UI language is
  a **server-side profile setting** which the tool forces to 日本語 once (§3.3).
- Proxy per account: `accounts_meta.proxy` (`browser_pool.py:699-705`), `effective_proxy_for(name)` =
  own > `config.PROXY` (`DOLA_PROXY`, empty default) ; `"direct"` = none (`src/utils/proxies.py:73-80`).
  Formats accepted by `parse_proxy` (`:31-70`): `ip:port`, `ip:port:user:pass`, `scheme://user:pass@host:port`
  (http/https/socks5). Browser gets `proxy={"server","username","password"}` (`proxy_kwargs :83-92`) → Chromium
  auth handled by Playwright (CDP `Fetch.authRequired` under the hood — a Rust port must implement
  `Fetch.enable(handleAuthRequests)` + `Fetch.continueWithAuth`, or use an unauthenticated local forwarder).
- Downloads/captcha images/raw-video lookups use the same proxy via `current_proxy` ContextVar + `aiohttp_proxy_parts`
  (`proxies.py:95-110`; `video_worker.py:179`, `unwatermark.py:228`).
- Pre-check `proxy_precheck` (§6 PROXY_DOWN). Hub modes: `tabs` applies proxy per context; `single`/`wtabs` cannot
  (one browser) — proxy only applies to HTTP polling/download.

---

## 10. What changed on 2026-09-17 — MUST-have vs can-DEFER for the Rust port

Commits of the day (`git log`, 44 commits from `6df9166` 01:26 to `3d07c19` 21:47). Grouped:

### 10.1 MUST replicate for a correct single-account fast-10s run
1. **`latest` = newest *bot* message (`user_type == 2`)** — `b4f68ec`, `video_worker.py:71-72`, `hub_browser.py:760-763`.
   Without it the tool's own prompt is mis-read as an agent question. (Agent mode mostly, but cheap and affects
   refusal/limit detection for fast too.)
2. **Refusal vs credit classification** — `639ee7c`, `09a003a`: `classify_dola_text` (`video_worker_ui.py:84-95`),
   `GENERIC_REFUSAL_RE`/`EXPLICIT_CREDIT_RE`, `ポリシーに違反` in `policy_reject`. Needed so a content refusal does not
   burn the account as "out of credit".
3. **Scan `<main>` not `body` for DOM text** — `90a05ad` (`:1305-1308`); sidebar chat titles were false positives.
4. **Language handling** — `1d2a059` + `34c64af`: clear `user_language_code`/`i18next` at launch
   (`browser.py:178-183`), `ensure_dola_japanese` + `wait_dola_settled` before any label-based click. Required
   because every selector is Japanese text.
5. **Ratio button regex accepts `比率\n16:9`** and 3 retries → `RATIO_UNAVAILABLE` — `e22cfa3`, `c4f2369`
   (`:1862`, `:2176-2207`). Needed for any ratio ≠ default.
6. **Clear stale composer attachments before send** — `e22cfa3` (`_clear_composer_attachments :794-833`). Needed when
   reference images are used (Dola keeps drafts across chats/reloads).
7. **Open-composer retry ×3** — `a5f9fd5` (`:2064-2089`). Cheap robustness; keep.
8. **New-conversation detection vs page jumping back to an old chat** — `3191f19`: `known_conv_ids` from
   `recent_conv` before send + `pick_new_conv_id` (`:635-689`, `:2566-2587`). Needed or you poll the wrong chat.
9. **Revoked-session detection** (`87dfa32`): `from_logout=1` URL + `_raise_if_dola_logged_out` before
   RATIO/MODE/REF/NO_CONVERSATION (`:1212-1228`), `AccountLoggedOutError(revoked=True)`. Needed to stop retry loops
   on a dead account (do not auto-verify, do not rotate through the whole pool).
10. **Prompt always pasted in one shot** (`28a8454`, `TYPE_PASTE_THRESHOLD=0`) → `Input.insertText`. Simple.
11. **Cookie persistence rules** (`9547e41`, `f44fe7a`): `ensure_persistent` 60 d, `merge_keep_values` — only if the
    port uses `cookies.json` instead of the profile jar. For "login via existing profile" the jar is the truth and
    nothing is needed.

### 10.2 Can DEFER (one-line reason each)
- **30 s hook / `route_30s` / `allowed_durations(model)`** (`e2fbbe9`, `ff34567`, `d7015e6`): only for 15/30 s in fast;
  10 s needs nothing.
- **wtabs / lvc_relay 1.3-1.4 / write-back to profile** (`f44fe7a`, `9547e41`, `46dcc0f`): multi-account-in-one-window
  only; a per-account Chromium needs no extension.
- **Facebook login** (`becda1d`, `src/browser/facebook_login.py`), Google phone-verify stage (`e0ab088`), re-login
  buttons (`6df9166`), multi-file cookies.txt import (`ce8ce08`), bulk proxy column (`e677c51`): account onboarding,
  not generation.
- **auto_retry default 3** (`5f245a1`, `ad46bd2`): server-level policy; port can start with 0 and add later.
- **Credit-quote resend** (`00d9e63`, `CREDIT_QUOTE_RETRIES=2`): optimisation for a specific Dola message; defer.
- **Prefer-account force** (`78f5696`), **FIFO limiter** (`a62ad05`), **spread key / PICK_RESERVE** (`d656144`),
  **mode fallback**: multi-account scheduling; irrelevant for one account.
- **Real-person sheet / 9-view character sheet / image generation** (`28a8454`, `d054807`, `90a05ad`): separate
  feature (画像を作成 skill) — defer.
- **Duration-question auto-reply** (`a4eb168`, `duration_request`): only appears with agent/real-person flows.
- **Revoked circuit breaker `REVOKED_STREAK`** (`87dfa32`): pool-wide protection; single account can just stop.
- **Launcher port randomisation, Nuitka/PyInstaller build, pack_source, dashboard/UI tweaks, task log timestamps**
  (`bc79710`, `3d07c19`, `2430961`, `439abec`, `7724387`, `0fa93a1`, `dd33ef2`, `fee8519`, `c2b1b0a`, docs): tooling.
- **Logo removal (`dewatermark`) + H.264 transcode** (`_finish_task`): post-processing with ffmpeg/OpenCV; the raw
  `fallback_api` download (`unwatermark.py`) already yields a clean file in most cases; defer ffmpeg.
- **Balance pre-flight (`BALANCE_JS`)**: purely advisory (errors ignored); defer.
- **Captcha solver** (OpenCV slider): only on risk control; initially just detect `bdcaptcha.html` frame and fail.

### 10.3 Recommended minimal end-to-end path for the port (one account, fast, 10 s, mp4)
1. Launch bundled Chromium headed with `--user-data-dir=accounts/<name>` and the five `LAUNCH_ARGS`
   (`browser.py:12-18`), `ignore_default_args` `--enable-automation`, emulate `ja-JP` / `Asia/Tokyo` via CDP
   (`Emulation.setLocaleOverride`, `Emulation.setTimezoneOverride`) — or seed the profile from `cookies.json`
   with `Network.setCookies` + 60-day expiry when no profile exists.
2. `Network.deleteCookies` for `user_language_code`, `i18next`; navigate `https://www.dola.com/chat`, wait
   DOMContentLoaded + 5 s; run `DOLA_LANG_JS`, `MODE_BTN_JS` until stable; run `SESSION_JS` + `classify_session`
   (check `sessionid` via `Network.getCookies`) → abort `LOGGED_OUT`.
3. Run `RECENT_IDS_JS` → `known` set.
4. Click cookie `OK` if visible; find the mode button by text `高速`/`プロ`, ensure `高速` (menuitem click if needed).
5. Click `動画を作成` (retry 3×), verify a `\d+s` or `比率` element appeared; optionally set `比率` and model; confirm
   duration label is `10s`.
6. Focus `textarea, [contenteditable="true"]`, `Control+A`, `Delete`, `Input.insertText(prompt)`, 600 ms, `Enter`.
7. Wait 20 s watching for an iframe with `bdcaptcha.html` (fail if present); then up to 30 × 1 s until
   `location.href` = `https://www.dola.com/chat/<digits>` with a digits id not in `known` (fallback `RECENT_IDS_JS`).
8. Poll every 5 s with `POLL_JS` (evaluate) **or** — simpler and proven — reqwest `POST
   https://www.dola.com/im/chain/single` with the jar's cookies exactly as `HttpSession.poll` (`hub_browser.py:853-873`);
   parse per `parse_chain`. Stop on `videos` non-empty; classify `latest` text for limit/refusal/credit;
   overall timeout 300 s (900 s with refs).
9. Download: try `fallback_api` + `channel=no&codec_type=8&logo_type=unwatermarked` (no cookies) and decode
   `video_list[*].main_url` (base64 / qAAB-AES); else base64 `main_url` by bitrate; else `download_url`. Plain GET,
   64 KiB chunks, `downloads/<account>_<YYYYmmdd_HHMMSS>.mp4`.
10. Close the browser (or keep warm ≤900 s).
Everything else in this document (agent replies, hook, captcha solve, multi-account scheduling, hub modes,
post-processing) layers on top without changing these ten steps.
