# P0 — Lõi thuần + Tầng lưu trữ: Kế hoạch triển khai

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dựng workspace Rust với `lvc-core` (miền thuần, không I/O) và `lvc-store` (SQLite qua sqlx), cộng bộ nhập dữ liệu từ DB Python cũ — toàn bộ kiểm chứng được mà không cần trình duyệt.

**Architecture:** `lvc-core` không phụ thuộc crate I/O nào; mọi giới hạn cấu hình được *tiêm vào* qua struct `Limits` thay vì đọc global như bản Python. `lvc-store` phụ thuộc `lvc-core` một chiều. Không crate nào ở P0 biết gì về trình duyệt, Tauri hay ffmpeg.

**Tech Stack:** Rust edition 2024 · sqlx 0.8 (sqlite, runtime-tokio) · serde · thiserror · tokio · proptest

**Spec:** `docs/superpowers/specs/2026-09-16-lvc-seedance-rust-design.md`

## Global Constraints

- Edition `2024`, `rust-version = "1.85"` cho mọi crate.
- `[lints.rust] unsafe_code = "warn"`; `[lints.clippy] all = "warn"`, `pedantic = "warn"`.
- `lvc-core` **không được** phụ thuộc: sqlx, tokio, reqwest, chromiumoxide, tauri. Kiểm bằng Task 1 Step 6.
- Thông điệp lỗi phải **khớp từng ký tự** với bản Python — dashboard và test cũ dựa vào chúng.
- Trạng thái task đúng 4 giá trị: `queued` `processing` `completed` `failed`.
- `failure_code` đúng 10 giá trị: `cancelled` `LOGGED_OUT` `429` `ACCOUNT_UNAVAILABLE` `PROMPT_VIOLATION` `REF_UPLOAD` `PROFILE_BUSY` `PROXY_DOWN` `DURATION_UNAVAILABLE` `AGENT_QUESTION`.
- Tỉ lệ hợp lệ ở tầng cảnh đúng **5**: `9:16` `16:9` `1:1` `4:3` `3:4` (bản Python `scenes.RATIOS`; `21:9` chỉ tồn tại ở `_RATIO_JA` nên không tới được).
- Thời lượng hợp lệ: `10` `15` `30`.
- Mặc định cảnh: `ratio=9:16`, `duration=10`, `model=seedance-2.0`, `mode=fast`, `logo=off`, `reference_images=[]`, `account=""`.

---

### Task 1: Workspace scaffold

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`
- Create: `crates/lvc-core/Cargo.toml`, `crates/lvc-core/src/lib.rs`
- Create: `crates/lvc-store/Cargo.toml`, `crates/lvc-store/src/lib.rs`

**Interfaces:**
- Consumes: (nothing — first task)
- Produces: workspace biên dịch được; crate `lvc_core` và `lvc_store` tồn tại và rỗng.

- [ ] **Step 1: Khởi tạo git**

```bash
cd /a/ToolLVC/lvc_seedancev2
git init
git config core.autocrlf false
```

- [ ] **Step 2: Viết `.gitignore`**

```gitignore
/target
**/*.rs.bk
node_modules/
dist/
*.db
*.db-wal
*.db-shm
accounts/
downloads/
uploads/
logs/
.env.local
accounts_creds.json
```

- [ ] **Step 3: Viết `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 4: Viết `Cargo.toml` gốc (workspace)**

```toml
[workspace]
resolver = "3"
members = ["crates/lvc-core", "crates/lvc-store"]

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "proprietary"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
sqlx = { version = "0.8", default-features = false, features = ["sqlite", "runtime-tokio", "macros", "migrate"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs"] }
lvc-core = { path = "crates/lvc-core" }

[workspace.lints.rust]
unsafe_code = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
```

- [ ] **Step 5: Viết `crates/lvc-core/Cargo.toml` và lib rỗng**

```toml
[package]
name = "lvc-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
thiserror.workspace = true

[dev-dependencies]
proptest = "1"

[lints]
workspace = true
```

`crates/lvc-core/src/lib.rs`:

```rust
//! Miền thuần của LVC Seedance: chế độ, cảnh, định dạng TXT.
//! Không I/O, không DB, không trình duyệt — test được không tốn lượt Dola.
```

- [ ] **Step 6: Viết `crates/lvc-store/Cargo.toml` và lib rỗng**

```toml
[package]
name = "lvc-store"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
lvc-core.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
tokio.workspace = true

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "test-util"] }
tempfile = "3"

[lints]
workspace = true
```

`crates/lvc-store/src/lib.rs`:

```rust
//! Tầng lưu trữ: SQLite qua sqlx, migration, và bộ nhập dữ liệu Python cũ.
```

- [ ] **Step 7: Kiểm biên dịch và kiểm ràng buộc phụ thuộc**

```bash
cargo build
cargo tree -p lvc-core | grep -E "sqlx|tokio|reqwest|chromiumoxide|tauri" && echo "VI PHAM: lvc-core khong duoc phu thuoc I/O" && exit 1 || echo "OK: lvc-core thuan"
```
Expected: build thành công; dòng cuối in `OK: lvc-core thuan`.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore: khoi tao workspace Rust cho ban viet lai Tauri

Tach lvc-core (thuan) khoi lvc-store (I/O) ngay tu dau de rang buoc
"loi khong import browser" duoc cuong che luc bien dich.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Kiểu lỗi và `Limits`

**Files:**
- Create: `crates/lvc-core/src/error.rs`
- Create: `crates/lvc-core/src/limits.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Consumes: (nothing)
- Produces: `CoreError` (enum, `PartialEq`), `SceneError { line: Option<usize>, kind: CoreError }`, `Limits { prompt_max_chars, batch_max_scenes, reference_image_max_count, daily_limit_fast, daily_limit_pro, daily_limit_pro_max }` với `Default`.

- [ ] **Step 1: Viết test thất bại**

`crates/lvc-core/src/error.rs` (phần test ở cuối file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_error_co_so_dong_thi_hien_line() {
        let e = SceneError::new(Some(3), CoreError::EmptyPrompt);
        assert_eq!(e.to_string(), "line 3: prompt is empty");
    }

    #[test]
    fn scene_error_khong_co_dong_thi_hien_tran() {
        let e = SceneError::new(None, CoreError::EmptyPrompt);
        assert_eq!(e.to_string(), "prompt is empty");
    }

    #[test]
    fn thong_diep_khop_ban_python() {
        assert_eq!(
            CoreError::InvalidMode("xyz".into()).to_string(),
            r#"mode must be fast, pro or pro_max (got "xyz")"#
        );
        assert_eq!(
            CoreError::InvalidLogo("x".into()).to_string(),
            r#"logo must be off, crop or delogo (got "x")"#
        );
        assert_eq!(
            CoreError::PromptTooLong { actual: 12, limit: 10 }.to_string(),
            "prompt too long (12 > 10 chars)"
        );
    }
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-core`
Expected: FAIL — `cannot find type SceneError in this scope`.

- [ ] **Step 3: Viết `error.rs`**

```rust
use std::fmt;

/// Lỗi miền. Thông điệp khớp từng ký tự với bản Python.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error("mode must be fast, pro or pro_max (got {0:?})")]
    InvalidMode(String),
    #[error("ratio must be one of 9:16, 16:9, 1:1, 4:3, 3:4 (got {0:?})")]
    InvalidRatio(String),
    #[error("model must be 2.0 or 2.5 (got {0:?})")]
    InvalidModel(String),
    #[error("logo must be off, crop or delogo (got {0:?})")]
    InvalidLogo(String),
    #[error("dur must be 10, 15 or 30 (got {0:?})")]
    InvalidDuration(String),
    #[error("prompt is empty")]
    EmptyPrompt,
    #[error("prompt too long ({actual} > {limit} chars)")]
    PromptTooLong { actual: usize, limit: usize },
    #[error("too many reference images (max {0})")]
    TooManyReferenceImages(usize),
    #[error("acc must be an account name (letters, digits, _ -; got {0:?})")]
    InvalidAccount(String),
    #[error("too many scenes (max {0})")]
    TooManyScenes(usize),
    #[error("no scenes (text is empty)")]
    NoScenes,
    #[error("expected key=value, got {0:?}")]
    ExpectedKeyValue(String),
    #[error("unknown key {0:?} (allowed: ratio, dur, model, mode, logo, refs, acc)")]
    UnknownKey(String),
}

/// Lỗi một cảnh kèm số dòng TXT (1-based) hoặc số thứ tự cảnh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneError {
    pub line: Option<usize>,
    pub kind: CoreError,
}

impl SceneError {
    #[must_use]
    pub fn new(line: Option<usize>, kind: CoreError) -> Self {
        Self { line, kind }
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(n) => write!(f, "line {n}: {}", self.kind),
            None => write!(f, "{}", self.kind),
        }
    }
}

impl std::error::Error for SceneError {}
```

- [ ] **Step 4: Viết `limits.rs`**

```rust
/// Giới hạn cấu hình được TIÊM vào miền thuần.
///
/// Bản Python đọc thẳng `config` trong `scenes.py`, phá vỡ tính thuần.
/// Ở đây gọi hàm nào cần giới hạn thì truyền vào.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// 0 = không giới hạn.
    pub prompt_max_chars: usize,
    pub batch_max_scenes: usize,
    pub reference_image_max_count: usize,
    pub daily_limit_fast: u32,
    pub daily_limit_pro: u32,
    pub daily_limit_pro_max: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            prompt_max_chars: 0,
            batch_max_scenes: 100,
            reference_image_max_count: 30,
            daily_limit_fast: 2,
            daily_limit_pro: 10,
            daily_limit_pro_max: 10,
        }
    }
}
```

- [ ] **Step 5: Khai báo module trong `lib.rs`**

```rust
//! Miền thuần của LVC Seedance: chế độ, cảnh, định dạng TXT.
//! Không I/O, không DB, không trình duyệt — test được không tốn lượt Dola.

pub mod error;
pub mod limits;

pub use error::{CoreError, SceneError};
pub use limits::Limits;
```

- [ ] **Step 6: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-core`
Expected: PASS, 3 test.

- [ ] **Step 7: Commit**

```bash
git add crates/lvc-core
git commit -m "$(cat <<'EOF'
feat(core): kieu loi mien va Limits tiem vao

Thong diep loi khop tung ky tu voi ban Python vi dashboard doc chung.
Limits duoc tiem thay vi doc global de giu lvc-core thuan.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `Mode`

**Files:**
- Create: `crates/lvc-core/src/mode.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Consumes: `CoreError` (Task 2), `Limits` (Task 2)
- Produces: `Mode` enum `{ Fast, Pro, ProMax }` với `Mode::parse(&str) -> Result<Mode, CoreError>`, `is_pro_like() -> bool`, `allowed_durations() -> &'static [u16]`, `as_str() -> &'static str`, `button_label() -> &'static str`; `Limits::daily_limit(Mode) -> u32`.

- [ ] **Step 1: Viết test thất bại**

Cuối `crates/lvc-core/src/mode.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Limits;

    #[test]
    fn alias_dung_nhu_ban_python() {
        for v in ["fast", "nhanh", "高速", "quick", "default", "FAST", "  fast  "] {
            assert_eq!(Mode::parse(v).unwrap(), Mode::Fast, "{v}");
        }
        for v in ["pro", "プロ", "professional"] {
            assert_eq!(Mode::parse(v).unwrap(), Mode::Pro, "{v}");
        }
        for v in ["pro_max", "promax", "pro-max", "skill"] {
            assert_eq!(Mode::parse(v).unwrap(), Mode::ProMax, "{v}");
        }
    }

    #[test]
    fn rong_thi_ve_fast() {
        assert_eq!(Mode::parse("").unwrap(), Mode::Fast);
        assert_eq!(Mode::parse("   ").unwrap(), Mode::Fast);
        assert_eq!(Mode::default(), Mode::Fast);
    }

    #[test]
    fn la_thi_loi_co_thong_diep_dung() {
        let err = Mode::parse("bogus").unwrap_err();
        assert_eq!(err.to_string(), r#"mode must be fast, pro or pro_max (got "bogus")"#);
    }

    #[test]
    fn thoi_luong_cho_phep_theo_che_do() {
        assert_eq!(Mode::Fast.allowed_durations(), &[10, 30]);
        assert_eq!(Mode::Pro.allowed_durations(), &[10, 15, 30]);
        assert_eq!(Mode::ProMax.allowed_durations(), &[10, 15, 30]);
    }

    #[test]
    fn pro_max_dung_nut_pro() {
        assert_eq!(Mode::Fast.button_label(), "高速");
        assert_eq!(Mode::Pro.button_label(), "プロ");
        assert_eq!(Mode::ProMax.button_label(), "プロ");
        assert!(!Mode::Fast.is_pro_like());
        assert!(Mode::Pro.is_pro_like());
        assert!(Mode::ProMax.is_pro_like());
    }

    #[test]
    fn han_muc_ngay_lay_tu_limits() {
        let l = Limits::default();
        assert_eq!(l.daily_limit(Mode::Fast), 2);
        assert_eq!(l.daily_limit(Mode::Pro), 10);
        assert_eq!(l.daily_limit(Mode::ProMax), 10);
        let zero = Limits { daily_limit_fast: 0, ..Limits::default() };
        assert_eq!(zero.daily_limit(Mode::Fast), 1, "ban Python kep san >= 1");
    }
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-core mode`
Expected: FAIL — `cannot find type Mode`.

- [ ] **Step 3: Viết `mode.rs`**

```rust
use serde::{Deserialize, Serialize};

use crate::{CoreError, Limits};

/// Chế độ tạo video của Dola.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Fast,
    Pro,
    ProMax,
}

impl Mode {
    /// Chuỗi rỗng / chỉ khoảng trắng → `Fast` (giữ hành vi `normalize_mode`).
    ///
    /// # Errors
    /// `CoreError::InvalidMode` khi giá trị không thuộc danh sách alias.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let key = value.trim().to_lowercase();
        if key.is_empty() {
            return Ok(Self::Fast);
        }
        match key.as_str() {
            "fast" | "nhanh" | "高速" | "quick" | "default" => Ok(Self::Fast),
            "pro" | "プロ" | "professional" => Ok(Self::Pro),
            "pro_max" | "promax" | "pro-max" | "skill" => Ok(Self::ProMax),
            _ => Err(CoreError::InvalidMode(value.to_string())),
        }
    }

    /// `pro` và `pro_max` cùng dùng nút プロ, cùng nhận link mp4 trong text_block.
    #[must_use]
    pub fn is_pro_like(self) -> bool {
        matches!(self, Self::Pro | Self::ProMax)
    }

    /// fast chỉ có 10s (30s nhờ extension); pro nhận 15s qua prompt.
    #[must_use]
    pub fn allowed_durations(self) -> &'static [u16] {
        if self.is_pro_like() { &[10, 15, 30] } else { &[10, 30] }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Pro => "pro",
            Self::ProMax => "pro_max",
        }
    }

    /// Nhãn nút chế độ trên trang Dola (ja-JP). `pro_max` bấm cùng nút với `pro`.
    #[must_use]
    pub fn button_label(self) -> &'static str {
        match self {
            Self::Fast => "高速",
            Self::Pro | Self::ProMax => "プロ",
        }
    }
}

impl Limits {
    /// Hạn mức video/ngày/tài khoản theo chế độ, kẹp sàn 1 như bản Python.
    #[must_use]
    pub fn daily_limit(&self, mode: Mode) -> u32 {
        match mode {
            Mode::Fast => self.daily_limit_fast.max(1),
            Mode::Pro => self.daily_limit_pro.max(1),
            Mode::ProMax => self.daily_limit_pro_max.max(1),
        }
    }
}
```

- [ ] **Step 4: Khai báo trong `lib.rs`**

Thêm vào `lib.rs`:

```rust
pub mod mode;
pub use mode::Mode;
```

- [ ] **Step 5: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-core`
Expected: PASS, 9 test.

- [ ] **Step 6: Commit**

```bash
git add crates/lvc-core
git commit -m "$(cat <<'EOF'
feat(core): enum Mode thay chuoi che do

Mode la kieu chu khong phai chuoi -> khong con normalize_mode rai rac.
Giu nguyen alias tieng Nhat/Viet va quy tac pro_max bam nut プロ.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: `Ratio`, `Model`, `LogoMode`

**Files:**
- Create: `crates/lvc-core/src/value.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Consumes: `CoreError` (Task 2)
- Produces: `Ratio { R9x16, R16x9, R1x1, R4x3, R3x4 }` với `parse`, `as_str`, `japanese`; `Model { Seedance20, Seedance25 }` với `parse`, `as_str` (`"seedance-2.0"`), `wire_str` (`"seedance_v2.0"`); `LogoMode { Off, Crop, Delogo }` với `parse`, `as_str`.

- [ ] **Step 1: Viết test thất bại**

Cuối `crates/lvc-core/src/value.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_dung_5_gia_tri() {
        for s in ["9:16", "16:9", "1:1", "4:3", "3:4"] {
            assert_eq!(Ratio::parse(s).unwrap().as_str(), s);
        }
        // 21:9 chi ton tai o _RATIO_JA cua ban Python, khong phai gia tri canh hop le
        let err = Ratio::parse("21:9").unwrap_err();
        assert_eq!(
            err.to_string(),
            r#"ratio must be one of 9:16, 16:9, 1:1, 4:3, 3:4 (got "21:9")"#
        );
        assert_eq!(Ratio::default(), Ratio::R9x16);
    }

    #[test]
    fn ratio_tieng_nhat_cho_prompt_pro() {
        assert_eq!(Ratio::R9x16.japanese(), "縦動画（9:16）");
        assert_eq!(Ratio::R3x4.japanese(), "縦動画（3:4）");
        assert_eq!(Ratio::R16x9.japanese(), "横動画（16:9）");
        assert_eq!(Ratio::R4x3.japanese(), "横動画（4:3）");
        assert_eq!(Ratio::R1x1.japanese(), "正方形動画（1:1）");
    }

    #[test]
    fn model_alias_day_du() {
        for s in ["2.0", "2", "seedance-2.0", "seedance_2.0", "seedance_v2.0", "seedance-v2.0"] {
            assert_eq!(Model::parse(s).unwrap(), Model::Seedance20, "{s}");
        }
        for s in ["2.5", "seedance-2.5", "seedance_2.5", "seedance_v2.5", "seedance-v2.5"] {
            assert_eq!(Model::parse(s).unwrap(), Model::Seedance25, "{s}");
        }
        assert_eq!(Model::Seedance20.as_str(), "seedance-2.0");
        assert_eq!(Model::Seedance20.wire_str(), "seedance_v2.0");
        assert_eq!(Model::Seedance25.wire_str(), "seedance_v2.5");
        assert_eq!(
            Model::parse("3.0").unwrap_err().to_string(),
            r#"model must be 2.0 or 2.5 (got "3.0")"#
        );
    }

    #[test]
    fn logo_mode_ba_gia_tri_rong_la_off() {
        assert_eq!(LogoMode::parse("").unwrap(), LogoMode::Off);
        assert_eq!(LogoMode::parse("  ").unwrap(), LogoMode::Off);
        assert_eq!(LogoMode::parse("CROP").unwrap(), LogoMode::Crop);
        assert_eq!(LogoMode::parse("delogo").unwrap(), LogoMode::Delogo);
        assert_eq!(
            LogoMode::parse("blur").unwrap_err().to_string(),
            r#"logo must be off, crop or delogo (got "blur")"#
        );
        // Ban Python bao loi voi gia tri GOC, khong lowercase
        assert_eq!(
            LogoMode::parse("BLUR").unwrap_err().to_string(),
            r#"logo must be off, crop or delogo (got "BLUR")"#
        );
    }
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-core value`
Expected: FAIL — `cannot find type Ratio`.

- [ ] **Step 3: Viết `value.rs`**

```rust
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Tỉ lệ khung hình hợp lệ ở tầng cảnh — đúng 5 giá trị của `scenes.RATIOS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Ratio {
    #[default]
    #[serde(rename = "9:16")]
    R9x16,
    #[serde(rename = "16:9")]
    R16x9,
    #[serde(rename = "1:1")]
    R1x1,
    #[serde(rename = "4:3")]
    R4x3,
    #[serde(rename = "3:4")]
    R3x4,
}

impl Ratio {
    /// # Errors
    /// `CoreError::InvalidRatio` khi không thuộc 5 giá trị hợp lệ.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        match value.trim() {
            "9:16" => Ok(Self::R9x16),
            "16:9" => Ok(Self::R16x9),
            "1:1" => Ok(Self::R1x1),
            "4:3" => Ok(Self::R4x3),
            "3:4" => Ok(Self::R3x4),
            other => Err(CoreError::InvalidRatio(other.to_string())),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::R9x16 => "9:16",
            Self::R16x9 => "16:9",
            Self::R1x1 => "1:1",
            Self::R4x3 => "4:3",
            Self::R3x4 => "3:4",
        }
    }

    /// Cụm tiếng Nhật ghép vào prompt cho agent Pro.
    #[must_use]
    pub fn japanese(self) -> &'static str {
        match self {
            Self::R9x16 => "縦動画（9:16）",
            Self::R3x4 => "縦動画（3:4）",
            Self::R16x9 => "横動画（16:9）",
            Self::R4x3 => "横動画（4:3）",
            Self::R1x1 => "正方形動画（1:1）",
        }
    }
}

/// Model Seedance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Model {
    #[default]
    #[serde(rename = "seedance-2.0")]
    Seedance20,
    #[serde(rename = "seedance-2.5")]
    Seedance25,
}

impl Model {
    /// # Errors
    /// `CoreError::InvalidModel` khi alias không nhận ra.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let key = value.trim().to_lowercase();
        match key.as_str() {
            "2.0" | "2" | "seedance-2.0" | "seedance_2.0" | "seedance_v2.0" | "seedance-v2.0" => {
                Ok(Self::Seedance20)
            }
            "2.5" | "seedance-2.5" | "seedance_2.5" | "seedance_v2.5" | "seedance-v2.5" => {
                Ok(Self::Seedance25)
            }
            _ => Err(CoreError::InvalidModel(value.trim().to_lowercase())),
        }
    }

    /// Tên công khai (API, TXT, DB).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seedance20 => "seedance-2.0",
            Self::Seedance25 => "seedance-2.5",
        }
    }

    /// Tên Dola dùng trong `ability_param`.
    #[must_use]
    pub fn wire_str(self) -> &'static str {
        match self {
            Self::Seedance20 => "seedance_v2.0",
            Self::Seedance25 => "seedance_v2.5",
        }
    }
}

/// Cách xử lý logo/watermark sau khi tải video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogoMode {
    #[default]
    Off,
    Crop,
    Delogo,
}

impl LogoMode {
    /// Rỗng → `Off` (giữ hành vi `normalize_logo_mode`).
    ///
    /// # Errors
    /// `CoreError::InvalidLogo` khi không thuộc 3 giá trị.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let key = value.trim().to_lowercase();
        if key.is_empty() {
            return Ok(Self::Off);
        }
        match key.as_str() {
            "off" => Ok(Self::Off),
            "crop" => Ok(Self::Crop),
            "delogo" => Ok(Self::Delogo),
            // Bản Python báo lỗi với giá trị GỐC (`value`), không phải bản đã
            // lowercase — giữ đúng để thông điệp khớp.
            _ => Err(CoreError::InvalidLogo(value.to_string())),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Crop => "crop",
            Self::Delogo => "delogo",
        }
    }
}
```

- [ ] **Step 4: Khai báo trong `lib.rs`**

```rust
pub mod value;
pub use value::{LogoMode, Model, Ratio};
```

- [ ] **Step 5: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-core`
Expected: PASS, 13 test.

- [ ] **Step 6: Commit**

```bash
git add crates/lvc-core
git commit -m "$(cat <<'EOF'
feat(core): kieu gia tri Ratio, Model, LogoMode

Ratio dung 5 gia tri (21:9 trong _RATIO_JA cua ban Python la code chet).
Model tach as_str (cong khai) khoi wire_str (ability_param cua Dola).

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: `Scene` và chuẩn hoá

**Files:**
- Create: `crates/lvc-core/src/scene.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Consumes: `Mode` (Task 3), `Ratio`/`Model`/`LogoMode` (Task 4), `Limits`/`SceneError`/`CoreError` (Task 2)
- Produces: `RawScene` (mọi trường `Option<String>`, `reference_images: Option<Vec<String>>`), `Scene { prompt, ratio, duration: u16, model, mode, logo, reference_images: Vec<String>, account: String }`, `Scene::normalize(&RawScene, &Limits, Option<usize>) -> Result<Scene, SceneError>`, hằng `DURATIONS: [u16; 3]`, `DEFAULT_DURATION: u16`.

- [ ] **Step 1: Viết test thất bại**

Cuối `crates/lvc-core/src/scene.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn raw(prompt: &str) -> RawScene {
        RawScene { prompt: Some(prompt.into()), ..RawScene::default() }
    }

    #[test]
    fn dien_mac_dinh_dung() {
        let s = Scene::normalize(&raw("con meo"), &Limits::default(), None).unwrap();
        assert_eq!(s.prompt, "con meo");
        assert_eq!(s.ratio, Ratio::R9x16);
        assert_eq!(s.duration, 10);
        assert_eq!(s.model, Model::Seedance20);
        assert_eq!(s.mode, Mode::Fast);
        assert_eq!(s.logo, LogoMode::Off);
        assert!(s.reference_images.is_empty());
        assert_eq!(s.account, "");
    }

    #[test]
    fn prompt_rong_thi_loi() {
        let e = Scene::normalize(&raw("   "), &Limits::default(), Some(2)).unwrap_err();
        assert_eq!(e.to_string(), "line 2: prompt is empty");
    }

    #[test]
    fn prompt_qua_dai_chi_khi_limit_duong() {
        let l = Limits { prompt_max_chars: 5, ..Limits::default() };
        let e = Scene::normalize(&raw("abcdefgh"), &l, None).unwrap_err();
        assert_eq!(e.to_string(), "prompt too long (8 > 5 chars)");
        // 0 = khong gioi han
        assert!(Scene::normalize(&raw("abcdefgh"), &Limits::default(), None).is_ok());
    }

    #[test]
    fn thoi_luong_chap_nhan_hau_to_s() {
        let mut r = raw("x");
        r.duration = Some("30s".into());
        assert_eq!(Scene::normalize(&r, &Limits::default(), None).unwrap().duration, 30);
        r.duration = Some("15S".into());
        assert_eq!(Scene::normalize(&r, &Limits::default(), None).unwrap().duration, 15);
        r.duration = Some(String::new());
        assert_eq!(Scene::normalize(&r, &Limits::default(), None).unwrap().duration, 10);
    }

    #[test]
    fn thoi_luong_la_thi_loi() {
        let mut r = raw("x");
        r.duration = Some("7".into());
        let e = Scene::normalize(&r, &Limits::default(), None).unwrap_err();
        assert_eq!(e.to_string(), r#"dur must be 10, 15 or 30 (got "7")"#);
        r.duration = Some("abc".into());
        let e = Scene::normalize(&r, &Limits::default(), None).unwrap_err();
        assert_eq!(e.to_string(), r#"dur must be 10, 15 or 30 (got "abc")"#);
    }

    #[test]
    fn refs_bo_phan_tu_rong_va_kep_so_luong() {
        let mut r = raw("x");
        r.reference_images = Some(vec!["a.jpg".into(), "  ".into(), " b.png ".into()]);
        let s = Scene::normalize(&r, &Limits::default(), None).unwrap();
        assert_eq!(s.reference_images, vec!["a.jpg", "b.png"]);

        let l = Limits { reference_image_max_count: 1, ..Limits::default() };
        let e = Scene::normalize(&r, &l, None).unwrap_err();
        assert_eq!(e.to_string(), "too many reference images (max 1)");
    }

    #[test]
    fn ten_tai_khoan_theo_regex() {
        let mut r = raw("x");
        r.account = Some("acc_2-b".into());
        assert_eq!(Scene::normalize(&r, &Limits::default(), None).unwrap().account, "acc_2-b");
        r.account = Some("bad name".into());
        let e = Scene::normalize(&r, &Limits::default(), None).unwrap_err();
        assert_eq!(
            e.to_string(),
            r#"acc must be an account name (letters, digits, _ -; got "bad name")"#
        );
        r.account = Some("x".repeat(33));
        assert!(Scene::normalize(&r, &Limits::default(), None).is_err());
    }
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-core scene`
Expected: FAIL — `cannot find type RawScene`.

- [ ] **Step 3: Viết `scene.rs`**

```rust
use serde::{Deserialize, Serialize};

use crate::{CoreError, Limits, LogoMode, Mode, Model, Ratio, SceneError};

pub const DURATIONS: [u16; 3] = [10, 15, 30];
pub const DEFAULT_DURATION: u16 = 10;

/// Cảnh thô: đúng hình dạng dict của bản Python, mọi trường tuỳ chọn.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawScene {
    pub prompt: Option<String>,
    pub ratio: Option<String>,
    pub duration: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub logo: Option<String>,
    pub reference_images: Option<Vec<String>>,
    pub account: Option<String>,
}

/// Cảnh đã chuẩn hoá và kiểm hợp lệ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scene {
    pub prompt: String,
    pub ratio: Ratio,
    pub duration: u16,
    pub model: Model,
    pub mode: Mode,
    pub logo: LogoMode,
    pub reference_images: Vec<String>,
    /// `""` = để scheduler tự chọn.
    pub account: String,
}

fn valid_account(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl Scene {
    /// # Errors
    /// `SceneError` kèm số dòng khi bất kỳ trường nào không hợp lệ.
    pub fn normalize(
        raw: &RawScene,
        limits: &Limits,
        line: Option<usize>,
    ) -> Result<Self, SceneError> {
        let err = |k: CoreError| SceneError::new(line, k);

        let prompt = raw.prompt.as_deref().unwrap_or("").trim().to_string();
        if prompt.is_empty() {
            return Err(err(CoreError::EmptyPrompt));
        }
        if limits.prompt_max_chars > 0 && prompt.chars().count() > limits.prompt_max_chars {
            return Err(err(CoreError::PromptTooLong {
                actual: prompt.chars().count(),
                limit: limits.prompt_max_chars,
            }));
        }

        let ratio_raw = raw.ratio.as_deref().unwrap_or("").trim();
        let ratio = if ratio_raw.is_empty() {
            Ratio::default()
        } else {
            Ratio::parse(ratio_raw).map_err(err)?
        };

        let dur_raw = raw.duration.as_deref().unwrap_or("").trim();
        let dur_trimmed = dur_raw.trim_end_matches(['s', 'S']);
        let duration = if dur_trimmed.is_empty() {
            DEFAULT_DURATION
        } else {
            dur_trimmed
                .parse::<u16>()
                .map_err(|_| err(CoreError::InvalidDuration(dur_raw.to_string())))?
        };
        if !DURATIONS.contains(&duration) {
            return Err(err(CoreError::InvalidDuration(dur_raw.to_string())));
        }

        let model_raw = raw.model.as_deref().unwrap_or("").trim();
        let model = if model_raw.is_empty() {
            Model::default()
        } else {
            Model::parse(model_raw).map_err(err)?
        };

        let mode = Mode::parse(raw.mode.as_deref().unwrap_or("")).map_err(err)?;
        let logo = LogoMode::parse(raw.logo.as_deref().unwrap_or("")).map_err(err)?;

        let reference_images: Vec<String> = raw
            .reference_images
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();
        if reference_images.len() > limits.reference_image_max_count {
            return Err(err(CoreError::TooManyReferenceImages(
                limits.reference_image_max_count,
            )));
        }

        let account = raw.account.as_deref().unwrap_or("").trim().to_string();
        if !account.is_empty() && !valid_account(&account) {
            return Err(err(CoreError::InvalidAccount(account)));
        }

        Ok(Self { prompt, ratio, duration, model, mode, logo, reference_images, account })
    }
}
```

- [ ] **Step 4: Khai báo trong `lib.rs`**

```rust
pub mod scene;
pub use scene::{DEFAULT_DURATION, DURATIONS, RawScene, Scene};
```

- [ ] **Step 5: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-core`
Expected: PASS, 20 test.

- [ ] **Step 6: Commit**

```bash
git add crates/lvc-core
git commit -m "$(cat <<'EOF'
feat(core): Scene va chuan hoa co tiem Limits

Giu nguyen moi quy tac cua normalize_scene: hau to s cua dur, bo ref rong,
regex ten tai khoan, prompt_max_chars = 0 nghia la khong gioi han.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: TXT parse / format và tính chất round-trip

**Files:**
- Create: `crates/lvc-core/src/scene_txt.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Consumes: `Scene`/`RawScene` (Task 5), `Limits`/`SceneError` (Task 2)
- Produces: `parse_txt(&str, &Limits) -> Result<Vec<Scene>, SceneError>`, `format_txt(&[Scene]) -> String`

- [ ] **Step 1: Viết test thất bại**

Cuối `crates/lvc-core/src/scene_txt.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LogoMode, Mode, Model, Ratio};

    #[test]
    fn bo_qua_dong_trong_va_comment() {
        let txt = "\n# ghi chu\n  \nmot canh\n";
        let s = parse_txt(txt, &Limits::default()).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].prompt, "mot canh");
    }

    #[test]
    fn doc_day_du_khoa_va_alias() {
        let txt = "canh mot | ratio=16:9 | dur=30s | model=2.5 | mode=pro | logo=crop | acc=acc2 | refs=a.jpg,https://x/y.png";
        let s = parse_txt(txt, &Limits::default()).unwrap();
        assert_eq!(s[0].ratio, Ratio::R16x9);
        assert_eq!(s[0].duration, 30);
        assert_eq!(s[0].model, Model::Seedance25);
        assert_eq!(s[0].mode, Mode::Pro);
        assert_eq!(s[0].logo, LogoMode::Crop);
        assert_eq!(s[0].account, "acc2");
        assert_eq!(s[0].reference_images, vec!["a.jpg", "https://x/y.png"]);
    }

    #[test]
    fn dau_gach_dung_trong_prompt_phai_escape() {
        let s = parse_txt(r"a \| b | dur=30", &Limits::default()).unwrap();
        assert_eq!(s[0].prompt, "a | b");
        assert_eq!(s[0].duration, 30);
    }

    #[test]
    fn khoa_la_va_thieu_dau_bang_thi_loi() {
        let e = parse_txt("x | bogus=1", &Limits::default()).unwrap_err();
        assert_eq!(
            e.to_string(),
            r#"line 1: unknown key "bogus" (allowed: ratio, dur, model, mode, logo, refs, acc)"#
        );
        let e = parse_txt("x | noequals", &Limits::default()).unwrap_err();
        assert_eq!(e.to_string(), r#"line 1: expected key=value, got "noequals""#);
    }

    #[test]
    fn vuot_tran_so_canh_thi_loi() {
        let l = Limits { batch_max_scenes: 2, ..Limits::default() };
        let txt = "a\nb\nc\n";
        let e = parse_txt(txt, &l).unwrap_err();
        assert_eq!(e.to_string(), "line 3: too many scenes (max 2)");
        // dung bang tran thi khong loi
        assert_eq!(parse_txt("a\nb\n", &l).unwrap().len(), 2);
    }

    #[test]
    fn format_chi_ghi_khoa_khac_mac_dinh() {
        let s = parse_txt("chi mot prompt", &Limits::default()).unwrap();
        assert_eq!(format_txt(&s), "chi mot prompt\n");
    }

    #[test]
    fn format_theo_dung_thu_tu_ratio_dur_model_mode_logo_acc_refs() {
        let txt = "p | refs=a.jpg | acc=me | logo=crop | mode=pro | model=2.5 | dur=30 | ratio=16:9";
        let s = parse_txt(txt, &Limits::default()).unwrap();
        assert_eq!(
            format_txt(&s),
            "p | ratio=16:9 | dur=30 | model=seedance-2.5 | mode=pro | logo=crop | acc=me | refs=a.jpg\n"
        );
    }

    #[test]
    fn format_rong_thi_chuoi_rong() {
        assert_eq!(format_txt(&[]), "");
    }

    #[test]
    fn round_trip_voi_dau_gach_dung() {
        let original = parse_txt(r"a \| b | mode=pro", &Limits::default()).unwrap();
        let round = parse_txt(&format_txt(&original), &Limits::default()).unwrap();
        assert_eq!(original, round);
    }
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-core scene_txt`
Expected: FAIL — `cannot find function parse_txt`.

- [ ] **Step 3: Viết `scene_txt.rs`**

```rust
use crate::{CoreError, Limits, RawScene, Scene, SceneError, scene::DEFAULT_DURATION};

/// Tách theo `|` KHÔNG đứng ngay sau dấu `\`.
///
/// Sao đúng ngữ nghĩa regex `(?<!\\)\|` của bản Python: chỉ nhìn đúng một ký tự
/// phía trước, nên `\\|` cũng không phải điểm tách (giống Python).
fn split_unescaped_pipe(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '|' && (i == 0 || chars[i - 1] != '\\') {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    parts.push(current);
    parts
}

fn unescape(s: &str) -> String {
    s.replace("\\|", "|")
}

fn escape(s: &str) -> String {
    s.replace('|', "\\|")
}

/// TXT → danh sách cảnh đã chuẩn hoá.
///
/// # Errors
/// `SceneError` kèm số dòng (1-based) tại dòng đầu tiên sai.
pub fn parse_txt(text: &str, limits: &Limits) -> Result<Vec<Scene>, SceneError> {
    let mut scenes = Vec::new();
    for (no, raw_line) in text.lines().enumerate() {
        let no = no + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts = split_unescaped_pipe(line);
        let mut scene = RawScene {
            prompt: Some(unescape(parts[0].trim())),
            ..RawScene::default()
        };
        for part in &parts[1..] {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some((key, value)) = part.split_once('=') else {
                return Err(SceneError::new(
                    Some(no),
                    CoreError::ExpectedKeyValue(part.to_string()),
                ));
            };
            let key = key.trim().to_lowercase();
            let value = unescape(value.trim());
            match key.as_str() {
                "ratio" => scene.ratio = Some(value),
                "dur" | "duration" => scene.duration = Some(value),
                "model" => scene.model = Some(value),
                "mode" => scene.mode = Some(value),
                "logo" => scene.logo = Some(value),
                "ref" | "refs" => {
                    scene.reference_images =
                        Some(value.split(',').map(str::to_string).collect());
                }
                "acc" | "account" => scene.account = Some(value),
                _ => return Err(SceneError::new(Some(no), CoreError::UnknownKey(key))),
            }
        }
        scenes.push(Scene::normalize(&scene, limits, Some(no))?);
        if scenes.len() > limits.batch_max_scenes {
            return Err(SceneError::new(
                Some(no),
                CoreError::TooManyScenes(limits.batch_max_scenes),
            ));
        }
    }
    Ok(scenes)
}

/// Ngược lại `parse_txt`: chỉ ghi khoá khác mặc định, theo thứ tự
/// ratio, dur, model, mode, logo, acc, refs.
#[must_use]
pub fn format_txt(scenes: &[Scene]) -> String {
    let mut lines = Vec::with_capacity(scenes.len());
    for s in scenes {
        let mut parts = vec![escape(&s.prompt)];
        if s.ratio != crate::Ratio::default() {
            parts.push(format!("ratio={}", s.ratio.as_str()));
        }
        if s.duration != DEFAULT_DURATION {
            parts.push(format!("dur={}", s.duration));
        }
        if s.model != crate::Model::default() {
            parts.push(format!("model={}", s.model.as_str()));
        }
        if s.mode != crate::Mode::default() {
            parts.push(format!("mode={}", s.mode.as_str()));
        }
        if s.logo != crate::LogoMode::default() {
            parts.push(format!("logo={}", s.logo.as_str()));
        }
        if !s.account.is_empty() {
            parts.push(format!("acc={}", s.account));
        }
        if !s.reference_images.is_empty() {
            let refs: Vec<String> = s.reference_images.iter().map(|r| escape(r)).collect();
            parts.push(format!("refs={}", refs.join(",")));
        }
        lines.push(parts.join(" | "));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}
```

- [ ] **Step 4: Khai báo trong `lib.rs`**

```rust
pub mod scene_txt;
pub use scene_txt::{format_txt, parse_txt};
```

- [ ] **Step 5: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-core`
Expected: PASS, 29 test.

- [ ] **Step 6: Viết test tính chất round-trip**

Create `crates/lvc-core/tests/round_trip.rs`:

```rust
//! Tinh chat cot loi cua dinh dang TXT: parse(format(x)) == x.

use lvc_core::{Limits, LogoMode, Mode, Model, Ratio, Scene, format_txt, parse_txt};
use proptest::prelude::*;

fn any_ratio() -> impl Strategy<Value = Ratio> {
    prop_oneof![
        Just(Ratio::R9x16),
        Just(Ratio::R16x9),
        Just(Ratio::R1x1),
        Just(Ratio::R4x3),
        Just(Ratio::R3x4),
    ]
}

fn any_scene() -> impl Strategy<Value = Scene> {
    (
        // prompt: khong xuong dong, khong rong sau khi trim, co the chua dau gach dung
        "[a-zA-Z0-9 |]{1,40}",
        any_ratio(),
        prop_oneof![Just(10u16), Just(15u16), Just(30u16)],
        prop_oneof![Just(Model::Seedance20), Just(Model::Seedance25)],
        prop_oneof![Just(Mode::Fast), Just(Mode::Pro), Just(Mode::ProMax)],
        prop_oneof![Just(LogoMode::Off), Just(LogoMode::Crop), Just(LogoMode::Delogo)],
        prop::collection::vec("[a-z0-9.]{1,10}", 0..3),
        prop_oneof![Just(String::new()), "[a-z][a-z0-9_-]{0,10}"],
    )
        .prop_filter("prompt khong rong sau trim", |(p, ..)| !p.trim().is_empty())
        .prop_map(
            |(prompt, ratio, duration, model, mode, logo, reference_images, account)| Scene {
                prompt: prompt.trim().to_string(),
                ratio,
                duration,
                model,
                mode,
                logo,
                reference_images,
                account,
            },
        )
}

proptest! {
    #[test]
    fn parse_format_round_trip(scenes in prop::collection::vec(any_scene(), 1..6)) {
        let text = format_txt(&scenes);
        let back = parse_txt(&text, &Limits::default()).unwrap();
        prop_assert_eq!(scenes, back);
    }
}
```

- [ ] **Step 7: Chạy test tính chất**

Run: `cargo test -p lvc-core --test round_trip`
Expected: PASS. Nếu proptest tìm được phản ví dụ, nó ghi ra `crates/lvc-core/proptest-regressions/` — **sửa code, đừng nới test**.

- [ ] **Step 8: Commit**

```bash
git add crates/lvc-core
git commit -m "$(cat <<'EOF'
feat(core): TXT parse/format giu tinh chat round-trip

split_unescaped_pipe sao dung ngu nghia lookbehind mot ky tu cua regex
Python, ke ca truong hop \\| de khong lech hanh vi.
Proptest cuong che parse(format(x)) == x thay vi vai vi du le.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: `split_bulk`, `bulk_scenes`, `batch_progress`

**Files:**
- Create: `crates/lvc-core/src/bulk.rs`
- Create: `crates/lvc-core/src/progress.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Consumes: `Scene`/`RawScene` (Task 5), `Limits`/`SceneError` (Task 2)
- Produces: `split_bulk(&str, &str, &Limits) -> Result<Vec<String>, SceneError>`, `bulk_scenes(&str, &str, &RawScene, &Limits) -> Result<Vec<Scene>, SceneError>`, `TaskStatus` enum, `BatchProgress { total, queued, processing, completed, failed, percent }`, `batch_progress(&[TaskStatus]) -> BatchProgress`

- [ ] **Step 1: Viết test thất bại**

Cuối `crates/lvc-core/src/bulk.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimiter_rong_thi_moi_dong_la_mot_canh() {
        let out = split_bulk("a\n\n b \r\nc\n", "", &Limits::default()).unwrap();
        assert_eq!(out, vec!["a", "b", "c"]);
    }

    #[test]
    fn co_delimiter_thi_giu_xuong_dong_ben_trong() {
        let out = split_bulk("a\nb\n---\nc", "---", &Limits::default()).unwrap();
        assert_eq!(out, vec!["a\nb", "c"]);
    }

    #[test]
    fn vuot_tran_thi_loi() {
        let l = Limits { batch_max_scenes: 2, ..Limits::default() };
        let e = split_bulk("a\nb\nc", "", &l).unwrap_err();
        assert_eq!(e.to_string(), "line 3: too many scenes (max 2)");
    }

    #[test]
    fn bulk_scenes_ap_mac_dinh_chung() {
        let defaults = RawScene { mode: Some("pro".into()), duration: Some("15".into()), ..RawScene::default() };
        let out = bulk_scenes("a\nb", "", &defaults, &Limits::default()).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].mode, crate::Mode::Pro);
        assert_eq!(out[1].duration, 15);
        assert_eq!(out[0].prompt, "a");
    }

    #[test]
    fn bulk_scenes_rong_thi_loi_no_scenes() {
        let e = bulk_scenes("   ", "", &RawScene::default(), &Limits::default()).unwrap_err();
        assert_eq!(e.to_string(), "no scenes (text is empty)");
    }
}
```

Cuối `crates/lvc-core/src/progress.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_la_hoan_tat_cong_that_bai() {
        let tasks = [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Queued,
            TaskStatus::Processing,
        ];
        let p = batch_progress(&tasks);
        assert_eq!(p.total, 4);
        assert_eq!(p.completed, 1);
        assert_eq!(p.failed, 1);
        assert_eq!(p.queued, 1);
        assert_eq!(p.processing, 1);
        assert_eq!(p.percent, 50);
    }

    #[test]
    fn rong_thi_percent_0() {
        let p = batch_progress(&[]);
        assert_eq!(p.total, 0);
        assert_eq!(p.percent, 0);
    }

    #[test]
    fn trang_thai_parse_dung_4_gia_tri() {
        assert_eq!(TaskStatus::parse("queued"), Some(TaskStatus::Queued));
        assert_eq!(TaskStatus::parse("processing"), Some(TaskStatus::Processing));
        assert_eq!(TaskStatus::parse("completed"), Some(TaskStatus::Completed));
        assert_eq!(TaskStatus::parse("failed"), Some(TaskStatus::Failed));
        assert_eq!(TaskStatus::parse("cancelled"), None, "huy = failed + failure_code");
    }
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-core`
Expected: FAIL — `cannot find function split_bulk`.

- [ ] **Step 3: Viết `bulk.rs`**

```rust
use crate::{CoreError, Limits, RawScene, Scene, SceneError};

/// Văn bản nhiều prompt → danh sách prompt.
///
/// `delimiter` rỗng/chỉ khoảng trắng → mỗi dòng không trống là một cảnh.
/// Có `delimiter` → tách theo CHUỖI LITERAL (không phải regex), giữ xuống dòng
/// bên trong một cảnh.
///
/// # Errors
/// `CoreError::TooManyScenes` khi vượt `limits.batch_max_scenes`.
pub fn split_bulk(
    text: &str,
    delimiter: &str,
    limits: &Limits,
) -> Result<Vec<String>, SceneError> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let delim = delimiter.trim();
    let parts: Vec<String> = if delim.is_empty() {
        normalized.split('\n').map(str::to_string).collect()
    } else {
        normalized.split(delim).map(str::to_string).collect()
    };
    let out: Vec<String> = parts
        .iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if out.len() > limits.batch_max_scenes {
        return Err(SceneError::new(
            Some(out.len()),
            CoreError::TooManyScenes(limits.batch_max_scenes),
        ));
    }
    Ok(out)
}

/// Mỗi prompt tách được → một cảnh chuẩn hoá với mặc định chung.
///
/// # Errors
/// `CoreError::NoScenes` khi không có prompt nào; `SceneError` kèm số thứ tự
/// cảnh khi một giá trị mặc định sai.
pub fn bulk_scenes(
    text: &str,
    delimiter: &str,
    defaults: &RawScene,
    limits: &Limits,
) -> Result<Vec<Scene>, SceneError> {
    let prompts = split_bulk(text, delimiter, limits)?;
    if prompts.is_empty() {
        return Err(SceneError::new(None, CoreError::NoScenes));
    }
    prompts
        .into_iter()
        .enumerate()
        .map(|(i, prompt)| {
            let raw = RawScene { prompt: Some(prompt), ..defaults.clone() };
            Scene::normalize(&raw, limits, Some(i + 1))
        })
        .collect()
}
```

- [ ] **Step 4: Viết `progress.rs`**

```rust
use serde::{Deserialize, Serialize};

/// Đúng 4 trạng thái. Huỷ = `Failed` + `failure_code = "cancelled"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

impl TaskStatus {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "processing" => Some(Self::Processing),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BatchProgress {
    pub total: usize,
    pub queued: usize,
    pub processing: usize,
    pub completed: usize,
    pub failed: usize,
    pub percent: u8,
}

/// `percent = round(100 * (completed + failed) / total)`.
#[must_use]
pub fn batch_progress(statuses: &[TaskStatus]) -> BatchProgress {
    let mut p = BatchProgress { total: statuses.len(), ..BatchProgress::default() };
    for s in statuses {
        match s {
            TaskStatus::Queued => p.queued += 1,
            TaskStatus::Processing => p.processing += 1,
            TaskStatus::Completed => p.completed += 1,
            TaskStatus::Failed => p.failed += 1,
        }
    }
    if p.total > 0 {
        let done = p.completed + p.failed;
        let pct = (100.0 * done as f64 / p.total as f64).round();
        p.percent = u8::try_from(pct as u64).unwrap_or(100);
    }
    p
}
```

- [ ] **Step 5: Khai báo trong `lib.rs`**

```rust
pub mod bulk;
pub mod progress;
pub use bulk::{bulk_scenes, split_bulk};
pub use progress::{BatchProgress, TaskStatus, batch_progress};
```

- [ ] **Step 6: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-core`
Expected: PASS, 37 test.

- [ ] **Step 7: Commit**

```bash
git add crates/lvc-core
git commit -m "$(cat <<'EOF'
feat(core): nhap hang loat va tien do batch

TaskStatus la enum 4 gia tri; huy van la Failed + failure_code cancelled
dung nhu ban Python de khong pha vo du lieu cu.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Prompt tiếng Nhật cho Pro / Pro Max

**Files:**
- Create: `crates/lvc-core/src/pro_prompt.rs`
- Modify: `crates/lvc-core/src/lib.rs`, `crates/lvc-core/src/limits.rs`

**Interfaces:**
- Consumes: `Ratio` (Task 4), `Limits` (Task 2)
- Produces: `pro_prompt(&str, Option<Ratio>, Option<u16>, bool) -> String`, `pro_max_prompt(&str, Option<Ratio>, Option<u16>, bool, &str) -> String`; thêm trường `Limits::pro_no_bgm: bool` và `Limits::pro_max_command: String`.

> Ghi chú: `Limits` có thêm `pro_max_command: String` nên **không còn `Copy`**. Sửa mọi `#[derive(..., Copy)]` trên `Limits` thành chỉ `Clone`, và đổi `impl Limits { pub fn daily_limit(&self, ...) }` giữ nguyên (đã nhận `&self`).

- [ ] **Step 1: Viết test thất bại**

Cuối `crates/lvc-core/src/pro_prompt.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ratio;

    #[test]
    fn day_du_thoi_luong_va_ti_le() {
        let out = pro_prompt("con meo", Some(Ratio::R9x16), Some(15), true);
        assert_eq!(
            out,
            "15秒の縦動画（9:16）を1本作ってください。内容: con meo。BGMなし、環境音のみ"
        );
    }

    #[test]
    fn tat_bgm_thi_khong_them_duoi() {
        let out = pro_prompt("x", Some(Ratio::R16x9), Some(10), false);
        assert_eq!(out, "10秒の横動画（16:9）を1本作ってください。内容: x");
    }

    #[test]
    fn chi_co_thoi_luong() {
        let out = pro_prompt("x", None, Some(30), false);
        assert_eq!(out, "30秒の動画を1本作ってください。内容: x");
    }

    #[test]
    fn chi_co_ti_le() {
        let out = pro_prompt("x", Some(Ratio::R1x1), None, false);
        assert_eq!(out, "正方形動画（1:1）を1本作ってください。内容: x");
    }

    #[test]
    fn khong_co_gi_thi_giu_nguyen_prompt_khong_bia() {
        assert_eq!(pro_prompt("  x  ", None, None, true), "x");
    }

    #[test]
    fn pro_max_them_lenh_ky_nang() {
        let out = pro_max_prompt("x", None, Some(10), false, "/Create Video");
        assert_eq!(out, "/Create Video 10秒の動画を1本作ってください。内容: x");
    }
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-core pro_prompt`
Expected: FAIL — `cannot find function pro_prompt`.

- [ ] **Step 3: Viết `pro_prompt.rs`**

```rust
use crate::Ratio;

/// Ghép yêu cầu thời lượng/tỉ lệ (tiếng Nhật) vào đầu prompt cho agent Pro.
///
/// Không có cả hai → trả nguyên prompt (không bịa thêm).
/// Đã đo thật: `"15秒の縦動画（9:16）を1本作ってください: ..."` → agent tạo đúng 15s 9:16.
#[must_use]
pub fn pro_prompt(prompt: &str, ratio: Option<Ratio>, duration: Option<u16>, no_bgm: bool) -> String {
    let prompt = prompt.trim();
    let head = match (duration, ratio) {
        (Some(d), Some(r)) => format!("{d}秒の{}", r.japanese()),
        (Some(d), None) => format!("{d}秒の動画"),
        (None, Some(r)) => r.japanese().to_string(),
        (None, None) => return prompt.to_string(),
    };
    let mut text = format!("{head}を1本作ってください。内容: {prompt}");
    if no_bgm {
        // Tránh agent tự thêm nhạc → lỗi bản quyền âm thanh (IDEA-012).
        text.push_str("。BGMなし、環境音のみ");
    }
    text
}

/// Pro Max: gõ thẳng lệnh kỹ năng vào ô chat thay vì chọn スキル → 動画の生成.
#[must_use]
pub fn pro_max_prompt(
    prompt: &str,
    ratio: Option<Ratio>,
    duration: Option<u16>,
    no_bgm: bool,
    command: &str,
) -> String {
    format!("{} {}", command.trim(), pro_prompt(prompt, ratio, duration, no_bgm))
}
```

- [ ] **Step 4: Thêm trường vào `Limits` và bỏ `Copy`**

Trong `crates/lvc-core/src/limits.rs`, đổi derive và thêm 2 trường:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub prompt_max_chars: usize,
    pub batch_max_scenes: usize,
    pub reference_image_max_count: usize,
    pub daily_limit_fast: u32,
    pub daily_limit_pro: u32,
    pub daily_limit_pro_max: u32,
    /// Thêm "BGMなし、環境音のみ" vào prompt Pro.
    pub pro_no_bgm: bool,
    /// Lệnh kỹ năng của Pro Max.
    pub pro_max_command: String,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            prompt_max_chars: 0,
            batch_max_scenes: 100,
            reference_image_max_count: 30,
            daily_limit_fast: 2,
            daily_limit_pro: 10,
            daily_limit_pro_max: 10,
            pro_no_bgm: true,
            pro_max_command: "/Create Video".to_string(),
        }
    }
}
```

- [ ] **Step 5: Xác nhận bỏ `Copy` không làm gãy chỗ nào**

Run: `cargo build -p lvc-core`
Expected: PASS. Mọi chỗ hiện dùng `&Limits` hoặc `Limits { .. , ..Limits::default() }`
nên bỏ `Copy` là vô hại. Nếu có lỗi `use of moved value`, đổi chỗ đó sang mượn
`&limits` thay vì `.clone()` — `Limits` giờ có `String` nên clone là cấp phát thật.

- [ ] **Step 6: Khai báo trong `lib.rs`**

```rust
pub mod pro_prompt;
pub use pro_prompt::{pro_max_prompt, pro_prompt};
```

- [ ] **Step 7: Chạy toàn bộ test**

Run: `cargo test -p lvc-core && cargo clippy -p lvc-core -- -D warnings`
Expected: PASS, 43 test; clippy sạch.

- [ ] **Step 8: Commit**

```bash
git add crates/lvc-core
git commit -m "$(cat <<'EOF'
feat(core): prompt tieng Nhat cho Pro va Pro Max

Pro khong co menu thoi luong/ti le nen phai ghi vao prompt. Mau cau lay
dung tu ban da do that tren Dola.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: Schema và migration của `lvc-store`

**Files:**
- Create: `crates/lvc-store/migrations/0001_init.sql`
- Create: `crates/lvc-store/src/db.rs`
- Create: `crates/lvc-store/src/error.rs`
- Modify: `crates/lvc-store/src/lib.rs`

**Interfaces:**
- Consumes: (nothing from core yet)
- Produces: `StoreError`, `Db` (bọc `SqlitePool`), `Db::open(&Path) -> Result<Db, StoreError>` (tự chạy migration, bật WAL), `Db::open_memory() -> Result<Db, StoreError>`, `Db::pool(&self) -> &SqlitePool`

- [ ] **Step 1: Viết test thất bại**

Create `crates/lvc-store/tests/schema.rs`:

```rust
use lvc_store::Db;

#[tokio::test]
async fn migration_tao_du_bang() {
    let db = Db::open_memory().await.unwrap();
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    for expected in ["accounts_meta", "proxies", "settings", "tasks", "usage"] {
        assert!(names.contains(&expected), "thieu bang {expected}; co: {names:?}");
    }
}

#[tokio::test]
async fn co_du_index_bat_buoc() {
    let db = Db::open_memory().await.unwrap();
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    for expected in ["idx_tasks_status", "idx_tasks_batch", "idx_tasks_retry"] {
        assert!(names.contains(&expected), "thieu index {expected}; co: {names:?}");
    }
}

#[tokio::test]
async fn mo_lai_file_khong_loi_migration_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lvc.db");
    {
        let db = Db::open(&path).await.unwrap();
        drop(db);
    }
    let db = Db::open(&path).await.unwrap();
    let (mode,): (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-store`
Expected: FAIL — `cannot find type Db`.

- [ ] **Step 3: Viết `migrations/0001_init.sql`**

```sql
-- Gop tasks.db + pool_usage.db cua ban Python vao mot DB, them index.

CREATE TABLE IF NOT EXISTS tasks (
  id               TEXT PRIMARY KEY,
  prompt           TEXT    NOT NULL,
  ratio            TEXT    NOT NULL DEFAULT '9:16',
  duration         INTEGER NOT NULL DEFAULT 10,
  model            TEXT    NOT NULL DEFAULT 'seedance-2.0',
  mode             TEXT    NOT NULL DEFAULT 'fast',
  logo             TEXT    NOT NULL DEFAULT 'off',
  logo_applied     INTEGER NOT NULL DEFAULT 0,
  status           TEXT    NOT NULL DEFAULT 'queued',
  failure_code     TEXT,
  error            TEXT,
  account          TEXT,
  account_pref     TEXT,
  conversation_id  TEXT,
  video_url        TEXT,
  local_path       TEXT,
  reference_images TEXT    NOT NULL DEFAULT '[]',
  batch_id         TEXT,
  batch_name       TEXT,
  scene_index      INTEGER,
  project_name     TEXT,
  project_dir      TEXT,
  retry_count      INTEGER NOT NULL DEFAULT 0,
  retry_at         REAL    NOT NULL DEFAULT 0,
  deadline_at      REAL,
  last_poll_at     REAL,
  created_at       REAL    NOT NULL,
  updated_at       REAL    NOT NULL,
  started_at       REAL,
  finished_at      REAL
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_batch  ON tasks(batch_id, scene_index);
CREATE INDEX IF NOT EXISTS idx_tasks_retry  ON tasks(status, retry_at);

CREATE TABLE IF NOT EXISTS accounts_meta (
  name                TEXT PRIMARY KEY,
  scheduling          INTEGER NOT NULL DEFAULT 1,
  note                TEXT    NOT NULL DEFAULT '',
  email               TEXT    NOT NULL DEFAULT '',
  created_at          REAL    NOT NULL DEFAULT 0,
  last_used_at        REAL    NOT NULL DEFAULT 0,
  login_ok            INTEGER NOT NULL DEFAULT 1,
  login_checked_at    REAL    NOT NULL DEFAULT 0,
  cooldown_until      REAL    NOT NULL DEFAULT 0,
  rate_limited_until  REAL    NOT NULL DEFAULT 0,
  limit_reason        TEXT    NOT NULL DEFAULT '',
  quota_blocked_until REAL    NOT NULL DEFAULT 0,
  quota_reason        TEXT    NOT NULL DEFAULT '',
  credit_balance      INTEGER,
  credit_checked_at   REAL    NOT NULL DEFAULT 0,
  proxy               TEXT    NOT NULL DEFAULT '',
  logged_out_at       REAL    NOT NULL DEFAULT 0,
  cookie_expires_at   REAL    NOT NULL DEFAULT 0,
  browser             TEXT    NOT NULL DEFAULT '',
  auto_verify_state   TEXT    NOT NULL DEFAULT '',
  auto_verify_tries   INTEGER NOT NULL DEFAULT 0
);

-- PRIMARY KEY da tao san unique index cho (account, day, mode).
CREATE TABLE IF NOT EXISTS usage (
  account TEXT    NOT NULL,
  day     TEXT    NOT NULL,
  mode    TEXT    NOT NULL,
  used    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, day, mode)
);

CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS proxies (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  url          TEXT    NOT NULL UNIQUE,
  label        TEXT    NOT NULL DEFAULT '',
  enabled      INTEGER NOT NULL DEFAULT 1,
  health       TEXT    NOT NULL DEFAULT 'unknown',
  last_checked REAL    NOT NULL DEFAULT 0,
  fail_count   INTEGER NOT NULL DEFAULT 0,
  last_used_at REAL    NOT NULL DEFAULT 0,
  exit_ip      TEXT    NOT NULL DEFAULT ''
);
```

- [ ] **Step 4: Viết `error.rs`**

```rust
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("loi sqlite: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("loi migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("duong dan khong hop le: {0}")]
    BadPath(String),
    #[error("loi JSON: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 5: Viết `db.rs`**

```rust
use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::StoreError;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Bọc pool SQLite. Bản Python dùng MỘT connection + khoá toàn cục gọi đồng bộ
/// từ event loop; ở đây là pool async thật.
#[derive(Debug, Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Mở (tạo nếu chưa có) DB tại `path`, bật WAL, chạy migration.
    ///
    /// # Errors
    /// `StoreError::Sqlx` / `StoreError::Migrate` khi mở hoặc migrate thất bại.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    /// DB trong bộ nhớ, dùng cho test.
    ///
    /// # Errors
    /// Như [`Db::open`].
    pub async fn open_memory() -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::new().in_memory(true).foreign_keys(true);
        // max_connections=1: moi connection in-memory la mot DB rieng.
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
```

- [ ] **Step 6: Khai báo trong `lib.rs`**

```rust
//! Tầng lưu trữ: SQLite qua sqlx, migration, và bộ nhập dữ liệu Python cũ.

pub mod db;
pub mod error;

pub use db::Db;
pub use error::StoreError;
```

- [ ] **Step 7: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-store`
Expected: PASS, 3 test.

- [ ] **Step 8: Commit**

```bash
git add crates/lvc-store
git commit -m "$(cat <<'EOF'
feat(store): schema gop hai DB cu, them index, bat WAL

Ban Python khong co index nao va dung mot connection + khoa toan cuc goi
dong bo tu event loop. Doi sang sqlx pool + WAL + 3 index tren tasks.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 10: Repository `tasks`

**Files:**
- Create: `crates/lvc-store/src/tasks.rs`
- Modify: `crates/lvc-store/src/lib.rs`

**Interfaces:**
- Consumes: `Db` (Task 9), `Scene`/`TaskStatus` (Task 5, 7)
- Produces: `TaskRow` struct; `TaskRepo::new(&Db)`; `create(&self, id: &str, scene: &Scene, batch: Option<&BatchRef>) -> Result<TaskRow, StoreError>`; `get(&self, id) -> Result<Option<TaskRow>, StoreError>`; `set_status(&self, id, TaskStatus, Option<&str> failure_code, Option<&str> error) -> Result<(), StoreError>`; `list_recent(&self, limit: i64) -> Result<Vec<TaskRow>, StoreError>`; `BatchRef { batch_id, batch_name, scene_index }`

- [ ] **Step 1: Viết test thất bại**

Create `crates/lvc-store/tests/tasks_repo.rs`:

```rust
use lvc_core::{Limits, RawScene, Scene, TaskStatus};
use lvc_store::{BatchRef, Db, TaskRepo};

fn scene(prompt: &str) -> Scene {
    let raw = RawScene { prompt: Some(prompt.into()), ..RawScene::default() };
    Scene::normalize(&raw, &Limits::default(), None).unwrap()
}

#[tokio::test]
async fn tao_roi_doc_lai_giu_nguyen_truong() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let s = scene("con meo di bo");
    repo.create("video_a1", &s, None).await.unwrap();

    let got = repo.get("video_a1").await.unwrap().expect("phai co");
    assert_eq!(got.prompt, "con meo di bo");
    assert_eq!(got.status, TaskStatus::Queued);
    assert_eq!(got.ratio, "9:16");
    assert_eq!(got.duration, 10);
    assert_eq!(got.reference_images, Vec::<String>::new());
    assert!(got.created_at > 0.0);
}

#[tokio::test]
async fn luu_reference_images_dang_json() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let mut s = scene("x");
    s.reference_images = vec!["upload:a.jpg".into(), "https://x/y.png".into()];
    repo.create("video_b", &s, None).await.unwrap();

    let got = repo.get("video_b").await.unwrap().unwrap();
    assert_eq!(got.reference_images, vec!["upload:a.jpg", "https://x/y.png"]);
}

#[tokio::test]
async fn huy_la_failed_cong_failure_code_cancelled() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_c", &scene("x"), None).await.unwrap();
    repo.set_status("video_c", TaskStatus::Failed, Some("cancelled"), None).await.unwrap();

    let got = repo.get("video_c").await.unwrap().unwrap();
    assert_eq!(got.status, TaskStatus::Failed);
    assert_eq!(got.failure_code.as_deref(), Some("cancelled"));
}

#[tokio::test]
async fn batch_luu_index_va_ten() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let b = BatchRef { batch_id: "batch_1".into(), batch_name: "Du an A".into(), scene_index: 2 };
    repo.create("video_d", &scene("x"), Some(&b)).await.unwrap();

    let got = repo.get("video_d").await.unwrap().unwrap();
    assert_eq!(got.batch_id.as_deref(), Some("batch_1"));
    assert_eq!(got.scene_index, Some(2));
}

#[tokio::test]
async fn khong_co_thi_tra_none() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    assert!(repo.get("khong_ton_tai").await.unwrap().is_none());
}

#[tokio::test]
async fn list_recent_moi_nhat_truoc_va_kep_limit() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    for i in 0..5 {
        repo.create(&format!("video_{i}"), &scene(&format!("p{i}")), None).await.unwrap();
    }
    let rows = repo.list_recent(3).await.unwrap();
    assert_eq!(rows.len(), 3);
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-store --test tasks_repo`
Expected: FAIL — `cannot find type TaskRepo`.

- [ ] **Step 3: Viết `tasks.rs`**

```rust
use lvc_core::{Scene, TaskStatus};
use sqlx::Row;

use crate::{Db, StoreError};

/// Tham chiếu batch khi task thuộc một kịch bản nhiều cảnh.
#[derive(Debug, Clone)]
pub struct BatchRef {
    pub batch_id: String,
    pub batch_name: String,
    /// 1-based, giữ thứ tự cảnh.
    pub scene_index: i64,
}

#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: String,
    pub prompt: String,
    pub ratio: String,
    pub duration: i64,
    pub model: String,
    pub mode: String,
    pub logo: String,
    pub status: TaskStatus,
    pub failure_code: Option<String>,
    pub error: Option<String>,
    pub account: Option<String>,
    pub account_pref: Option<String>,
    pub conversation_id: Option<String>,
    pub video_url: Option<String>,
    pub local_path: Option<String>,
    pub reference_images: Vec<String>,
    pub batch_id: Option<String>,
    pub batch_name: Option<String>,
    pub scene_index: Option<i64>,
    pub retry_count: i64,
    pub retry_at: f64,
    pub created_at: f64,
    pub updated_at: f64,
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Result<TaskRow, StoreError> {
    let refs_json: String = row.try_get("reference_images")?;
    let status_str: String = row.try_get("status")?;
    Ok(TaskRow {
        id: row.try_get("id")?,
        prompt: row.try_get("prompt")?,
        ratio: row.try_get("ratio")?,
        duration: row.try_get("duration")?,
        model: row.try_get("model")?,
        mode: row.try_get("mode")?,
        logo: row.try_get("logo")?,
        status: TaskStatus::parse(&status_str).unwrap_or(TaskStatus::Failed),
        failure_code: row.try_get("failure_code")?,
        error: row.try_get("error")?,
        account: row.try_get("account")?,
        account_pref: row.try_get("account_pref")?,
        conversation_id: row.try_get("conversation_id")?,
        video_url: row.try_get("video_url")?,
        local_path: row.try_get("local_path")?,
        reference_images: serde_json::from_str(&refs_json)?,
        batch_id: row.try_get("batch_id")?,
        batch_name: row.try_get("batch_name")?,
        scene_index: row.try_get("scene_index")?,
        retry_count: row.try_get("retry_count")?,
        retry_at: row.try_get("retry_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[derive(Debug, Clone)]
pub struct TaskRepo {
    db: Db,
}

impl TaskRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại; `StoreError::Json` khi serialize refs thất bại.
    pub async fn create(
        &self,
        id: &str,
        scene: &Scene,
        batch: Option<&BatchRef>,
    ) -> Result<TaskRow, StoreError> {
        let ts = now();
        let refs = serde_json::to_string(&scene.reference_images)?;
        let account_pref = (!scene.account.is_empty()).then(|| scene.account.clone());
        sqlx::query(
            "INSERT INTO tasks (id, prompt, ratio, duration, model, mode, logo, status,
                 reference_images, account_pref, batch_id, batch_name, scene_index,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'queued', ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        )
        .bind(id)
        .bind(&scene.prompt)
        .bind(scene.ratio.as_str())
        .bind(i64::from(scene.duration))
        .bind(scene.model.as_str())
        .bind(scene.mode.as_str())
        .bind(scene.logo.as_str())
        .bind(&refs)
        .bind(account_pref)
        .bind(batch.map(|b| b.batch_id.clone()))
        .bind(batch.map(|b| b.batch_name.clone()))
        .bind(batch.map(|b| b.scene_index))
        .bind(ts)
        .execute(self.db.pool())
        .await?;

        self.get(id).await?.ok_or_else(|| {
            StoreError::BadPath(format!("task {id} vua tao nhung doc lai khong thay"))
        })
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get(&self, id: &str) -> Result<Option<TaskRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_to_task).transpose()
    }

    /// Huỷ task = `Failed` + `failure_code = "cancelled"` (không có trạng thái `cancelled`).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_status(
        &self,
        id: &str,
        status: TaskStatus,
        failure_code: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET status = ?2, failure_code = ?3, error = ?4, updated_at = ?5
             WHERE id = ?1",
        )
        .bind(id)
        .bind(status.as_str())
        .bind(failure_code)
        .bind(error)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<TaskRow>, StoreError> {
        let limit = limit.clamp(1, 200);
        let rows = sqlx::query("SELECT * FROM tasks ORDER BY created_at DESC LIMIT ?1")
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_to_task).collect()
    }
}
```

- [ ] **Step 4: Khai báo trong `lib.rs`**

```rust
pub mod tasks;
pub use tasks::{BatchRef, TaskRepo, TaskRow};
```

- [ ] **Step 5: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-store`
Expected: PASS, 9 test.

- [ ] **Step 6: Commit**

```bash
git add crates/lvc-store
git commit -m "$(cat <<'EOF'
feat(store): repository tasks

Nhan Scene da chuan hoa (kieu) thay vi dict, nen khong con duong nao ghi
gia tri ratio/mode la vao DB.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 11: Repository `accounts`, `usage`, `settings`

**Files:**
- Create: `crates/lvc-store/src/accounts.rs`
- Create: `crates/lvc-store/src/settings.rs`
- Modify: `crates/lvc-store/src/lib.rs`

**Interfaces:**
- Consumes: `Db` (Task 9), `Mode` (Task 3)
- Produces: `AccountRow`; `AccountRepo::new(&Db)`, `upsert(&self, name) `, `list(&self)`, `set_scheduling(&self, name, bool)`, `set_proxy(&self, name, &str)`, `bump_usage(&self, name, day, Mode) -> Result<i64, StoreError>`, `used_today(&self, name, day, Mode) -> Result<i64, StoreError>`; `SettingsRepo::new(&Db)`, `get(&self, key) -> Result<Option<String>, StoreError>`, `set(&self, key, value)`, `get_or(&self, key, default) -> Result<String, StoreError>`

- [ ] **Step 1: Viết test thất bại**

Create `crates/lvc-store/tests/accounts_repo.rs`:

```rust
use lvc_core::Mode;
use lvc_store::{AccountRepo, Db, SettingsRepo};

#[tokio::test]
async fn upsert_hai_lan_khong_nhan_ban() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.upsert("acc2").await.unwrap();
    assert_eq!(repo.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn mac_dinh_bat_lich() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    let rows = repo.list().await.unwrap();
    assert!(rows[0].scheduling);
    assert_eq!(rows[0].proxy, "");
}

#[tokio::test]
async fn tat_lich_va_dat_proxy() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_scheduling("acc2", false).await.unwrap();
    repo.set_proxy("acc2", "http://u:p@1.2.3.4:8080").await.unwrap();
    let rows = repo.list().await.unwrap();
    assert!(!rows[0].scheduling);
    assert_eq!(rows[0].proxy, "http://u:p@1.2.3.4:8080");
}

#[tokio::test]
async fn usage_dem_rieng_tung_che_do() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    assert_eq!(repo.bump_usage("acc2", "2026-09-16", Mode::Fast).await.unwrap(), 1);
    assert_eq!(repo.bump_usage("acc2", "2026-09-16", Mode::Fast).await.unwrap(), 2);
    assert_eq!(repo.bump_usage("acc2", "2026-09-16", Mode::Pro).await.unwrap(), 1);
    assert_eq!(repo.used_today("acc2", "2026-09-16", Mode::Fast).await.unwrap(), 2);
    assert_eq!(repo.used_today("acc2", "2026-09-17", Mode::Fast).await.unwrap(), 0);
}

#[tokio::test]
async fn settings_get_set() {
    let db = Db::open_memory().await.unwrap();
    let s = SettingsRepo::new(&db);
    assert!(s.get("threads").await.unwrap().is_none());
    s.set("threads", "5").await.unwrap();
    assert_eq!(s.get("threads").await.unwrap().as_deref(), Some("5"));
    s.set("threads", "8").await.unwrap();
    assert_eq!(s.get_or("threads", "3").await.unwrap(), "8");
    assert_eq!(s.get_or("khong_co", "mac_dinh").await.unwrap(), "mac_dinh");
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-store --test accounts_repo`
Expected: FAIL — `cannot find type AccountRepo`.

- [ ] **Step 3: Viết `accounts.rs`**

```rust
use lvc_core::Mode;
use sqlx::Row;

use crate::{Db, StoreError};

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub name: String,
    pub scheduling: bool,
    pub email: String,
    pub note: String,
    pub proxy: String,
    pub browser: String,
    pub login_ok: bool,
    pub last_used_at: f64,
    pub cooldown_until: f64,
    pub rate_limited_until: f64,
    pub quota_blocked_until: f64,
    pub cookie_expires_at: f64,
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[derive(Debug, Clone)]
pub struct AccountRepo {
    db: Db,
}

impl AccountRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Tạo nếu chưa có; đã có thì không đụng gì.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn upsert(&self, name: &str) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO accounts_meta (name, created_at) VALUES (?1, ?2)
             ON CONFLICT(name) DO NOTHING",
        )
        .bind(name)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn list(&self) -> Result<Vec<AccountRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM accounts_meta ORDER BY name")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter()
            .map(|r| {
                Ok(AccountRow {
                    name: r.try_get("name")?,
                    scheduling: r.try_get::<i64, _>("scheduling")? != 0,
                    email: r.try_get("email")?,
                    note: r.try_get("note")?,
                    proxy: r.try_get("proxy")?,
                    browser: r.try_get("browser")?,
                    login_ok: r.try_get::<i64, _>("login_ok")? != 0,
                    last_used_at: r.try_get("last_used_at")?,
                    cooldown_until: r.try_get("cooldown_until")?,
                    rate_limited_until: r.try_get("rate_limited_until")?,
                    quota_blocked_until: r.try_get("quota_blocked_until")?,
                    cookie_expires_at: r.try_get("cookie_expires_at")?,
                })
            })
            .collect()
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_scheduling(&self, name: &str, on: bool) -> Result<(), StoreError> {
        sqlx::query("UPDATE accounts_meta SET scheduling = ?2 WHERE name = ?1")
            .bind(name)
            .bind(i64::from(on))
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_proxy(&self, name: &str, proxy: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE accounts_meta SET proxy = ?2 WHERE name = ?1")
            .bind(name)
            .bind(proxy)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Tăng và trả về số lượt đã dùng trong ngày của chế độ đó.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn bump_usage(
        &self,
        account: &str,
        day: &str,
        mode: Mode,
    ) -> Result<i64, StoreError> {
        let row = sqlx::query(
            "INSERT INTO usage (account, day, mode, used) VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(account, day, mode) DO UPDATE SET used = used + 1
             RETURNING used",
        )
        .bind(account)
        .bind(day)
        .bind(mode.as_str())
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.try_get("used")?)
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn used_today(
        &self,
        account: &str,
        day: &str,
        mode: Mode,
    ) -> Result<i64, StoreError> {
        let row = sqlx::query(
            "SELECT used FROM usage WHERE account = ?1 AND day = ?2 AND mode = ?3",
        )
        .bind(account)
        .bind(day)
        .bind(mode.as_str())
        .fetch_optional(self.db.pool())
        .await?;
        match row {
            Some(r) => Ok(r.try_get("used")?),
            None => Ok(0),
        }
    }
}
```

- [ ] **Step 4: Viết `settings.rs`**

```rust
use sqlx::Row;

use crate::{Db, StoreError};

#[derive(Debug, Clone)]
pub struct SettingsRepo {
    db: Db,
}

impl SettingsRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(self.db.pool())
            .await?;
        match row {
            Some(r) => Ok(Some(r.try_get("value")?)),
            None => Ok(None),
        }
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get_or(&self, key: &str, default: &str) -> Result<String, StoreError> {
        Ok(self.get(key).await?.unwrap_or_else(|| default.to_string()))
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }
}
```

- [ ] **Step 5: Khai báo trong `lib.rs`**

```rust
pub mod accounts;
pub mod settings;
pub use accounts::{AccountRepo, AccountRow};
pub use settings::SettingsRepo;
```

- [ ] **Step 6: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-store`
Expected: PASS, 14 test.

- [ ] **Step 7: Commit**

```bash
git add crates/lvc-store
git commit -m "$(cat <<'EOF'
feat(store): repository accounts, usage, settings

usage dem rieng tung che do voi PRIMARY KEY (account, day, mode) giong
ban Python, de han muc fast/pro/pro_max khong lan nhau.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 12: Bộ nhập dữ liệu Python cũ

**Files:**
- Create: `crates/lvc-store/src/legacy.rs`
- Modify: `crates/lvc-store/src/lib.rs`

**Interfaces:**
- Consumes: `Db` (Task 9), `StoreError` (Task 9)
- Produces: `ImportReport { tasks, accounts, usage, settings, skipped: Vec<String> }`; `import_legacy(&Db, legacy_dir: &Path) -> Result<ImportReport, StoreError>`

- [ ] **Step 1: Viết test thất bại**

Create `crates/lvc-store/tests/legacy_import.rs`:

```rust
use std::path::Path;

use lvc_store::{Db, import_legacy};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Executor};

/// Dung DB cu toi thieu dung schema Python (chi cot ta doc).
async fn make_legacy(dir: &Path) {
    let tasks_db = dir.join("tasks.db");
    let opts = SqliteConnectOptions::new().filename(&tasks_db).create_if_missing(true);
    let mut conn = opts.connect().await.unwrap();
    conn.execute(
        "CREATE TABLE tasks (
            id TEXT PRIMARY KEY, model TEXT, prompt TEXT, ratio TEXT, duration INTEGER,
            status TEXT, video_url TEXT, error TEXT, created_at REAL, updated_at REAL,
            conversation_id TEXT, failure_code TEXT, reference_images TEXT,
            mode TEXT DEFAULT 'fast', account TEXT, retry_count INTEGER DEFAULT 0,
            retry_at REAL DEFAULT 0, logo TEXT DEFAULT 'off', logo_applied INTEGER DEFAULT 0,
            batch_id TEXT, scene_index INTEGER, account_pref TEXT,
            project_name TEXT, project_dir TEXT, batch_name TEXT, local_path TEXT
        )",
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, model, prompt, ratio, duration, status, created_at, updated_at,
             reference_images, mode, logo)
         VALUES ('video_old1', 'seedance-2.0', 'canh cu', '9:16', 10, 'completed',
             1000.0, 1001.0, '[\"upload:a.jpg\"]', 'pro', 'crop')",
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, model, prompt, ratio, duration, status, created_at, updated_at,
             reference_images, mode, logo)
         VALUES ('video_old2', 'seedance-2.5', 'canh hai', '16:9', 30, 'failed',
             1002.0, 1003.0, '[]', 'fast', 'off')",
    )
    .await
    .unwrap();

    let pool_db = dir.join("pool_usage.db");
    let opts = SqliteConnectOptions::new().filename(&pool_db).create_if_missing(true);
    let mut conn = opts.connect().await.unwrap();
    conn.execute(
        "CREATE TABLE accounts_meta (
            name TEXT PRIMARY KEY, scheduling INTEGER DEFAULT 1, note TEXT DEFAULT '',
            email TEXT DEFAULT '', created_at REAL DEFAULT 0, last_used_at REAL DEFAULT 0,
            login_ok INTEGER DEFAULT 1, proxy TEXT DEFAULT '', browser TEXT DEFAULT ''
        )",
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO accounts_meta (name, email, proxy, browser, scheduling)
         VALUES ('acc2', 'a@b.com', 'http://1.2.3.4:8080', 'opera', 1)",
    )
    .await
    .unwrap();
    conn.execute(
        "CREATE TABLE usage (account TEXT, day TEXT, mode TEXT, used INTEGER,
            PRIMARY KEY(account, day, mode))",
    )
    .await
    .unwrap();
    conn.execute("INSERT INTO usage VALUES ('acc2', '2026-09-15', 'fast', 2)").await.unwrap();
    conn.execute("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT)").await.unwrap();
    conn.execute("INSERT INTO settings VALUES ('threads', '5')").await.unwrap();
}

#[tokio::test]
async fn nhap_du_lieu_cu_day_du() {
    let dir = tempfile::tempdir().unwrap();
    make_legacy(dir.path()).await;

    let db = Db::open_memory().await.unwrap();
    let report = import_legacy(&db, dir.path()).await.unwrap();

    assert_eq!(report.tasks, 2);
    assert_eq!(report.accounts, 1);
    assert_eq!(report.usage, 1);
    assert_eq!(report.settings, 1);

    let repo = lvc_store::TaskRepo::new(&db);
    let t = repo.get("video_old1").await.unwrap().unwrap();
    assert_eq!(t.prompt, "canh cu");
    assert_eq!(t.mode, "pro");
    assert_eq!(t.logo, "crop");
    assert_eq!(t.reference_images, vec!["upload:a.jpg"]);

    let accounts = lvc_store::AccountRepo::new(&db).list().await.unwrap();
    assert_eq!(accounts[0].name, "acc2");
    assert_eq!(accounts[0].proxy, "http://1.2.3.4:8080");
}

#[tokio::test]
async fn chay_hai_lan_khong_nhan_ban() {
    let dir = tempfile::tempdir().unwrap();
    make_legacy(dir.path()).await;

    let db = Db::open_memory().await.unwrap();
    import_legacy(&db, dir.path()).await.unwrap();
    let second = import_legacy(&db, dir.path()).await.unwrap();

    assert_eq!(second.tasks, 0, "lan hai phai bo qua het");
    let rows = lvc_store::TaskRepo::new(&db).list_recent(100).await.unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn thieu_file_cu_thi_bao_skipped_chu_khong_loi() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_memory().await.unwrap();
    let report = import_legacy(&db, dir.path()).await.unwrap();
    assert_eq!(report.tasks, 0);
    assert_eq!(report.skipped.len(), 2, "thieu ca tasks.db lan pool_usage.db");
}
```

- [ ] **Step 2: Chạy test để xác nhận thất bại**

Run: `cargo test -p lvc-store --test legacy_import`
Expected: FAIL — `cannot find function import_legacy`.

- [ ] **Step 3: Viết `legacy.rs`**

```rust
use std::path::Path;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};

use crate::{Db, StoreError};

/// Kết quả nhập. `skipped` liệt kê file cũ không tìm thấy — thiếu file không
/// phải lỗi, vì người dùng có thể cài mới hoàn toàn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub tasks: usize,
    pub accounts: usize,
    pub usage: usize,
    pub settings: usize,
    pub skipped: Vec<String>,
}

async fn open_readonly(path: &Path) -> Result<SqlitePool, StoreError> {
    let opts = SqliteConnectOptions::new().filename(path).read_only(true);
    Ok(SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?)
}

/// Đọc cột có thể không tồn tại ở DB cũ (bản Python migrate bằng
/// `ALTER TABLE ... ADD COLUMN` nuốt lỗi, nên schema thực tế khác nhau tuỳ máy).
fn opt_str(row: &sqlx::sqlite::SqliteRow, col: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(col).ok().flatten()
}

fn opt_i64(row: &sqlx::sqlite::SqliteRow, col: &str) -> Option<i64> {
    row.try_get::<Option<i64>, _>(col).ok().flatten()
}

fn opt_f64(row: &sqlx::sqlite::SqliteRow, col: &str) -> Option<f64> {
    row.try_get::<Option<f64>, _>(col).ok().flatten()
}

/// Nhập `tasks.db` + `pool_usage.db` của bản Python vào DB mới.
///
/// Idempotent: chạy lại chỉ bỏ qua bản ghi đã có (`INSERT OR IGNORE`).
/// Chỉ ĐỌC DB cũ — không bao giờ sửa dữ liệu gốc.
///
/// # Errors
/// `StoreError::Sqlx` khi đọc/ghi thất bại trên file thực sự tồn tại.
pub async fn import_legacy(db: &Db, legacy_dir: &Path) -> Result<ImportReport, StoreError> {
    let mut report = ImportReport::default();

    let tasks_db = legacy_dir.join("tasks.db");
    if tasks_db.is_file() {
        let old = open_readonly(&tasks_db).await?;
        let rows = sqlx::query("SELECT * FROM tasks").fetch_all(&old).await?;
        for r in &rows {
            let Some(id) = opt_str(r, "id") else { continue };
            let res = sqlx::query(
                "INSERT OR IGNORE INTO tasks
                   (id, prompt, ratio, duration, model, mode, logo, logo_applied, status,
                    failure_code, error, account, account_pref, conversation_id, video_url,
                    local_path, reference_images, batch_id, batch_name, scene_index,
                    project_name, project_dir, retry_count, retry_at, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
                         ?20,?21,?22,?23,?24,?25,?26)",
            )
            .bind(&id)
            .bind(opt_str(r, "prompt").unwrap_or_default())
            .bind(opt_str(r, "ratio").unwrap_or_else(|| "9:16".into()))
            .bind(opt_i64(r, "duration").unwrap_or(10))
            .bind(opt_str(r, "model").unwrap_or_else(|| "seedance-2.0".into()))
            .bind(opt_str(r, "mode").unwrap_or_else(|| "fast".into()))
            .bind(opt_str(r, "logo").unwrap_or_else(|| "off".into()))
            .bind(opt_i64(r, "logo_applied").unwrap_or(0))
            .bind(opt_str(r, "status").unwrap_or_else(|| "failed".into()))
            .bind(opt_str(r, "failure_code"))
            .bind(opt_str(r, "error"))
            .bind(opt_str(r, "account"))
            .bind(opt_str(r, "account_pref"))
            .bind(opt_str(r, "conversation_id"))
            .bind(opt_str(r, "video_url"))
            .bind(opt_str(r, "local_path"))
            .bind(opt_str(r, "reference_images").unwrap_or_else(|| "[]".into()))
            .bind(opt_str(r, "batch_id"))
            .bind(opt_str(r, "batch_name"))
            .bind(opt_i64(r, "scene_index"))
            .bind(opt_str(r, "project_name"))
            .bind(opt_str(r, "project_dir"))
            .bind(opt_i64(r, "retry_count").unwrap_or(0))
            .bind(opt_f64(r, "retry_at").unwrap_or(0.0))
            .bind(opt_f64(r, "created_at").unwrap_or(0.0))
            .bind(opt_f64(r, "updated_at").unwrap_or(0.0))
            .execute(db.pool())
            .await?;
            report.tasks += usize::try_from(res.rows_affected()).unwrap_or(0);
        }
        old.close().await;
    } else {
        report.skipped.push("tasks.db".to_string());
    }

    let pool_db = legacy_dir.join("pool_usage.db");
    if pool_db.is_file() {
        let old = open_readonly(&pool_db).await?;

        let rows = sqlx::query("SELECT * FROM accounts_meta").fetch_all(&old).await?;
        for r in &rows {
            let Some(name) = opt_str(r, "name") else { continue };
            let res = sqlx::query(
                "INSERT OR IGNORE INTO accounts_meta
                   (name, scheduling, note, email, created_at, last_used_at, login_ok,
                    proxy, browser)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            )
            .bind(&name)
            .bind(opt_i64(r, "scheduling").unwrap_or(1))
            .bind(opt_str(r, "note").unwrap_or_default())
            .bind(opt_str(r, "email").unwrap_or_default())
            .bind(opt_f64(r, "created_at").unwrap_or(0.0))
            .bind(opt_f64(r, "last_used_at").unwrap_or(0.0))
            .bind(opt_i64(r, "login_ok").unwrap_or(1))
            .bind(opt_str(r, "proxy").unwrap_or_default())
            .bind(opt_str(r, "browser").unwrap_or_default())
            .execute(db.pool())
            .await?;
            report.accounts += usize::try_from(res.rows_affected()).unwrap_or(0);
        }

        let rows = sqlx::query("SELECT * FROM usage").fetch_all(&old).await?;
        for r in &rows {
            let res = sqlx::query(
                "INSERT OR IGNORE INTO usage (account, day, mode, used) VALUES (?1,?2,?3,?4)",
            )
            .bind(opt_str(r, "account").unwrap_or_default())
            .bind(opt_str(r, "day").unwrap_or_default())
            .bind(opt_str(r, "mode").unwrap_or_else(|| "fast".into()))
            .bind(opt_i64(r, "used").unwrap_or(0))
            .execute(db.pool())
            .await?;
            report.usage += usize::try_from(res.rows_affected()).unwrap_or(0);
        }

        if let Ok(rows) = sqlx::query("SELECT * FROM settings").fetch_all(&old).await {
            for r in &rows {
                let res = sqlx::query(
                    "INSERT OR IGNORE INTO settings (key, value) VALUES (?1,?2)",
                )
                .bind(opt_str(r, "key").unwrap_or_default())
                .bind(opt_str(r, "value").unwrap_or_default())
                .execute(db.pool())
                .await?;
                report.settings += usize::try_from(res.rows_affected()).unwrap_or(0);
            }
        }

        old.close().await;
    } else {
        report.skipped.push("pool_usage.db".to_string());
    }

    Ok(report)
}
```

- [ ] **Step 4: Khai báo trong `lib.rs`**

```rust
pub mod legacy;
pub use legacy::{ImportReport, import_legacy};
```

- [ ] **Step 5: Chạy test để xác nhận đạt**

Run: `cargo test -p lvc-store`
Expected: PASS, 17 test.

- [ ] **Step 6: Commit**

```bash
git add crates/lvc-store
git commit -m "$(cat <<'EOF'
feat(store): nhap du lieu tu tasks.db va pool_usage.db cua ban Python

Doc bang try_get cho tung cot vi ban Python migrate bang ALTER TABLE nuot
loi, nen schema thuc te khac nhau tuy may. INSERT OR IGNORE de chay lai
duoc nhieu lan. Chi DOC DB cu, khong bao gio sua du lieu goc.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 13: Cổng chất lượng P0

**Files:**
- Create: `scripts/check.ps1`
- Create: `README.md`

**Interfaces:**
- Consumes: mọi crate (Task 1–12)
- Produces: một lệnh chạy hết fmt + clippy + test + kiểm ràng buộc phụ thuộc.

- [ ] **Step 1: Viết `scripts/check.ps1`**

```powershell
# Cong chat luong P0: chay truoc moi commit lon.
$ErrorActionPreference = "Stop"

Write-Host "== fmt ==" -ForegroundColor Cyan
cargo fmt --all -- --check

Write-Host "== clippy ==" -ForegroundColor Cyan
cargo clippy --workspace --all-targets -- -D warnings

Write-Host "== test ==" -ForegroundColor Cyan
cargo test --workspace

Write-Host "== rang buoc: lvc-core phai thuan ==" -ForegroundColor Cyan
$tree = cargo tree -p lvc-core --prefix none --no-dedupe | Out-String
foreach ($bad in @("sqlx", "tokio", "reqwest", "chromiumoxide", "tauri")) {
    if ($tree -match "(?m)^\s*$bad\s") {
        Write-Error "VI PHAM: lvc-core phu thuoc $bad"
    }
}

Write-Host "TAT CA XANH" -ForegroundColor Green
```

- [ ] **Step 2: Chạy cổng chất lượng**

Run: `powershell -ExecutionPolicy Bypass -File scripts/check.ps1`
Expected: in `TAT CA XANH`. Nếu `cargo fmt --check` đỏ, chạy `cargo fmt --all` rồi chạy lại.

- [ ] **Step 3: Viết `README.md`**

```markdown
# LVC Seedance v2 (Rust + Tauri)

Bản viết lại của `lvcseedance` (Python) sang Rust + Tauri với workflow kéo thả.

## Trạng thái

Giai đoạn **P0**: lõi thuần + tầng lưu trữ. Chưa có trình duyệt, chưa có UI.

## Cấu trúc

| Crate | Trách nhiệm |
|---|---|
| `lvc-core` | Miền thuần: chế độ, cảnh, TXT, tiến độ. Không I/O. |
| `lvc-store` | SQLite qua sqlx, migration, nhập dữ liệu Python cũ. |

## Phát triển

```bash
cargo test --workspace
powershell -ExecutionPolicy Bypass -File scripts/check.ps1
```

`lvc-core` **không được** phụ thuộc sqlx/tokio/reqwest/chromiumoxide/tauri.
`scripts/check.ps1` cưỡng chế ràng buộc này.

## Nhập dữ liệu từ bản Python

```rust
let db = Db::open(Path::new("lvc.db")).await?;
let report = import_legacy(&db, Path::new(r"A:\ToolLVC\lvcseedance")).await?;
```

Chỉ đọc `tasks.db` và `pool_usage.db` cũ, không bao giờ sửa. Chạy lại được nhiều lần.

## Tài liệu

- Thiết kế: `docs/superpowers/specs/2026-09-16-lvc-seedance-rust-design.md`
- Kế hoạch P0: `docs/superpowers/plans/2026-09-16-p0-core-store.md`
```

- [ ] **Step 4: Commit**

```bash
git add scripts README.md
git commit -m "$(cat <<'EOF'
chore: cong chat luong P0 va README

check.ps1 cuong che rang buoc "lvc-core khong phu thuoc I/O" bang cargo
tree, vi day la rang buoc kien truc de vo am tham khi them dependency.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Cố ý KHÔNG làm ở P0

**Kiểu node graph** (`NodeKind`, `PortType`, `Edge`, `Graph`). Đặc tả xếp chúng vào
`lvc-core`, nhưng định nghĩa chúng lúc chưa có executor là thiết kế mù: hình dạng đúng
của `Graph` chỉ lộ ra khi viết DAG executor và node rào chắn ở P3. Định nghĩa sớm rồi
sửa lại tốn hơn là định nghĩa đúng lúc.

Những gì P0 xây — `Scene`, `Mode`, `Ratio`, `Model`, `LogoMode` — là dữ liệu mà node
`Tạo video` sẽ mang, nên không có gì phải làm lại.

## Nghiệm thu P0

- [ ] `cargo test --workspace` xanh, **không cần trình duyệt, không cần mạng**
- [ ] `scripts/check.ps1` in `TAT CA XANH`
- [ ] `lvc-core` không phụ thuộc sqlx/tokio/reqwest/chromiumoxide/tauri
- [ ] Tính chất `parse_txt(format_txt(x)) == x` được proptest cưỡng chế
- [ ] Nhập được `tasks.db` + `pool_usage.db` thật từ `A:\ToolLVC\lvcseedance`, chạy lại không nhân bản

## Sau P0

P1 (`BrowserDriver` + chromiumoxide + cookie + đăng nhập) là **cửa tử**: nếu Dola
chặn CDP thuần, ta biết ngay mà chưa mất công viết node graph. Trước khi bắt đầu P1,
viết một kế hoạch riêng.
