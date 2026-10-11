# Ghi nhớ kỹ thuật — `lvc-browser::veo3`

## Vì sao KHÔNG dùng `BrowserDriver`/`ChromiumDriver` như `dola/`

`BrowserDriver` (trait ở `driver.rs`) có `submit`/`poll_and_download` gắn cứng với mô hình
conversation của Dola (`ConversationId`, `SubmitRequest`, `STAGE_*`…) — không có khái niệm tương tự
cho "mint captcha" hay "harvest phiên account". Ép vào trait đó sẽ phải thêm method vô nghĩa cho
Dola hoặc tách trait — không đáng, vì `veo3::captcha`/`veo3::harvest` chỉ cần vài lệnh CDP đơn giản
(mở trang, chạy JS), không cần bộ máy Session/Job-Object/proxy-auth phức tạp của `ChromiumSession`.

## Vì sao KHÔNG cần "pool"/luồng thợ riêng như Python

Python (`flow_cdp_captcha/pool.py`) phải tự dựng 1 luồng thợ + hàng đợi lệnh vì Selenium WebDriver
KHÔNG thread-safe — driver tạo ở luồng nào chỉ dùng được đúng luồng đó. `chromiumoxide` là async
thuần (không có khái niệm "luồng sở hữu driver"), nên chỉ cần bọc `GuestCaptchaBrowser` trong
`tokio::sync::Mutex` ở tầng gọi (`lvc-engine`) để serialize các lời gọi `mint()` đồng thời — không
cần tự chế queue/worker-thread. Đây là ĐƠN GIẢN HOÁ có chủ đích so với bản Python, không phải thiếu
sót khi port.

## Vì sao mint captcha PHẢI ở trình duyệt KHÁCH, không phải phiên account

Đo thật 2 lần (dự án ảnh khác + xác nhận lại 24-25/09/2026 trên chính dự án video): mint token bằng
code chạy TRONG phiên đã đăng nhập (dù qua extension hay Selenium mở thẳng profile account) → LUÔN
`PUBLIC_ERROR_UNUSUAL_ACTIVITY`, bất kể payload đúng/sai. Mint ở trình duyệt KHÁCH (không cookie
tài khoản, trang tĩnh `flow.google.com/about`, không có widget reCAPTCHA của Flow nào để token bị
ràng vào) → Google chấp nhận, tạo video thật được (`abra_t2v_4s/6s/8s`). Cookie tài khoản THẬT
không bao giờ được nạp vào trình duyệt này — nó tách biệt hoàn toàn khỏi bước harvest phiên account
(`harvest.rs`, Task 8).

## Vì sao KHÔNG headless / KHÔNG minimize

Cả hai đều là tín hiệu xấu cho reCAPTCHA Enterprise: headless đổi User-Agent
(`HeadlessChrome`) và một số đặc điểm render bị nhận diện ngay; minimize đổi
`document.visibilityState` thành `'hidden'` — đúng lúc cần điểm tin cậy cao nhất lại tự hạ điểm.
Giải pháp: chạy headed (`--window-position` đẩy ra ngoài màn hình âm) — vẫn "visible" với trang,
người dùng không thấy cửa sổ.

## Chưa port (biết trước, không phải thiếu sót)

- **UA/kích thước cửa sổ xoay theo profile** (Python `UA_VARIANTS`/`WINDOW_SIZE_VARIANTS`, xoay
  theo `variant_seed` mỗi lần dựng profile mới) — v1 dùng 1 UA/kích thước cố định. Thêm sau nếu
  thấy Google bắt đầu để ý các guest profile "giống hệt nhau".
- **Dọn profile khách mồ côi** (Python `kill_orphan_guest_browsers`/`sweep_stale_profiles` — dọn
  Chrome/profile sót lại khi app tắt ngang không qua `close()`). v1: nếu `open()` thành công rồi
  process bị kill cứng (không qua `close()`), thư mục profile bị rò (không tự dọn). Cần thêm một
  hàm quét-và-xoá thư mục `guest_*` cũ ở tầng gọi (`lvc-engine`/`lvc-app`, lúc khởi động app) nếu
  việc này gây rò đĩa thật trong thực tế.
- **Proxy cho trình duyệt khách** — Python hỗ trợ mint qua proxy (để mint đúng IP sẽ gửi request
  sau đó, tránh mint-1-IP/gửi-1-IP-khác bị coi là bất thường). v1 luôn mint trực tiếp (không proxy).
  Nếu tài khoản dùng proxy riêng, ĐÂY LÀ RỦI RO — cần thêm proxy vào `GuestCaptchaOpts` khi có
  account dùng proxy thật.
- Không dùng `respect_https_errors()`/Job Object Windows kiểu `ChromiumSession` đầy đủ — guest
  browser sống ngắn, không giữ dữ liệu quan trọng, kill cứng khi lỗi là chấp nhận được (khác phiên
  account, nơi thoát sạch để không hỏng cookie).

## Token dùng một lần, đừng để dành

Token reCAPTCHA sống ~90-120s và bị "tiêu" ngay khi đưa vào payload gửi đi (Google coi là dùng rồi)
— `mint()` phải được gọi NGAY TRƯỚC lúc cần, không cache/tái sử dụng, không mint trước một lô rồi
phát dần (đây đúng là lý do tránh gọi cơ chế này là "pool" — không có kho token nào để lấy ra dùng).

## `harvest.rs` (Task 8) — quirk phát hiện khi port `flow_account_runner.py`

- **`--window-position` KHÔNG được dùng cho profile ACCOUNT** (khác hẳn `captcha::GUEST_ARGS`).
  Python (`flow_account_runner.py` dòng 142-144, 201-205) ghi rõ: Chrome LƯU toạ độ cửa sổ vào
  `Preferences` của profile — đẩy cửa sổ ra ngoài màn hình MỘT LẦN sẽ "đầu độc" profile vĩnh viễn,
  lần sau mở TAY (không qua tool) cũng ra ngoài màn hình, không thấy gì. Guest profile của
  `captcha.rs` không bị ảnh hưởng vì nó bị xoá sau mỗi lần dùng — profile account thì sống lâu dài.
  `harvest.rs::ACCOUNT_ARGS` cố tình KHÔNG có cờ này; có test ghim
  (`account_args_khong_dua_cua_so_ra_ngoai_man_hinh`) để không ai lỡ thêm lại.
- **Không tái dùng `chromium::LAUNCH_ARGS`/`chrome_args()` của Dola.** `chrome_args()` luôn tự
  thêm `--lang=ja-JP` + Accept-Language/timezone Nhật (đặc thù sản phẩm Dola) — vô lý cho một tài
  khoản Google Flow bất kỳ (thường không phải người dùng Nhật). `harvest.rs` tự có `ACCOUNT_ARGS`
  riêng, tối giản, giống cách `captcha.rs` cũng không tái dùng các hằng số đó.
- **Guard cookie bắt buộc** (Python `get_shared_flow_client` dòng 552-564): đọc được `at` KHÔNG
  đủ — phải có ít nhất 1 trong `SID`/`__Secure-1PSID`/`__Secure-3PSID`/`__Secure-1PSIDTS` với GIÁ
  TRỊ KHÔNG RỖNG. Lỗi thật Python đã gặp: AV/khoá profile chặn đọc cookie DB → `at` đọc được (JS
  in-page, không qua DB) nhưng cookie rỗng → cache "half-baked" → mọi request sau đều 401 tới khi
  restart app. `harvest.rs::has_auth_cookie` port đúng guard này, trả `BrowserError::MissingAuthCookie`
  (tên 2 biến thể mới thêm vào `error.rs`: `NotLoggedIn`, `MissingAuthCookie`) — KHÔNG bao giờ cache
  "at có, cookie thiếu".
- **Không gọi `profile::disable_session_restore`** trước khi mở (khác `ChromiumDriver::launch` của
  Dola) — quyết định có chủ ý để tối giản Task 8 (Python cũng không làm việc này trong
  `FlowAccountRunner.open()`). Nếu sau này thấy popup "khôi phục trang" cản việc đọc
  `window.WIZ_global_data` lúc harvest, đây là nơi cần thêm.
- **Không dò `ProfileBusy`/lockfile** như `ChromiumDriver::prepare`/`probe_lockfile_busy` (hai hàm
  đó là private của `chromium.rs`, không export). Nếu 2 tiến trình cùng mở 1 profile account lúc
  harvest, lỗi sẽ rơi vào `BrowserError::Launch`/`LaunchExit` chung (thông tin kém chính xác hơn
  Dola) — cân nhắc thêm khi có bằng chứng đây là vấn đề thật (đa-tab/đa-tiến-trình gọi harvest cùng
  lúc cho cùng 1 account).
- **Đọc cookie đúng 1 lần** cho cả guard và header gửi đi (`join_cookie_header` viết lại công thức
  của `ops::cookie_header` thay vì gọi lại nó) — tránh gọi `Network.getCookies` 2 lần/lần harvest.
