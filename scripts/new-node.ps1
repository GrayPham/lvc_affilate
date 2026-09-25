# Tao khung MOT node module moi (ADR-R014) - du file luat/ghi nho/README/manifest, bien dich duoc
# ngay. Vi du:
#   powershell -ExecutionPolicy Bypass -File scripts/new-node.ps1 -Kind gemini_video -Category video
# Sau khi chay: them 1 dong vao crates/lvc-app/Cargo.toml + 1 dong vao crates/lvc-app/src/registry.rs
# (script in ra dung 2 dong can them), roi sinh manifest:
#   $env:LVC_WRITE_FIXTURES=1; cargo test -p node-<ten>; Remove-Item Env:LVC_WRITE_FIXTURES
param(
    [Parameter(Mandatory = $true)][string]$Kind,
    [ValidateSet("input", "text", "image", "video", "audio", "output")][string]$Category = "video",
    [string]$Label = ""
)

$ErrorActionPreference = "Stop"

if ($Kind -notmatch '^[a-z][a-z0-9_]*$') {
    Write-Host "Kind phai la snake_case chu thuong (vd gemini_video)" -ForegroundColor Red
    exit 1
}
if ($Label -eq "") { $Label = $Kind }

$root = Split-Path -Parent $PSScriptRoot
$crate = "node-" + ($Kind -replace '_', '-')
$crateUnderscore = $crate -replace '-', '_'
$dir = Join-Path $root "crates\nodes\$crate"
if (Test-Path $dir) {
    Write-Host "Da ton tai: $dir" -ForegroundColor Red
    exit 1
}
$catPascal = (Get-Culture).TextInfo.ToTitleCase($Category)
$today = Get-Date -Format "yyyy-MM-dd"

New-Item -ItemType Directory -Force (Join-Path $dir "src") | Out-Null

function Write-Utf8([string]$Path, [string]$Text) {
    $enc = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, ($Text -replace "`r`n", "`n"), $enc)
}

Write-Utf8 (Join-Path $dir "Cargo.toml") @"
[package]
name = "$crate"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

# Node "$Kind" - xem CLAUDE.md cung thu muc. Chi duoc phu thuoc lvc-node-api, lvc-core,
# crates/providers/*. KHONG node khac / lvc-engine / lvc-store / lvc-browser / tauri.
[dependencies]
lvc-node-api.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true

[dev-dependencies]
lvc-core.workspace = true
tokio = { workspace = true }
tokio-util.workspace = true

[lints]
workspace = true
"@

$lib = @'
//! Node `__KIND__` - TODO: mo ta 1-2 dong (dau vao -> dau ra).

use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Category, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor, NodeManifest, NodeResult,
    Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "__KIND__";

#[derive(Debug, Deserialize)]
#[allow(dead_code, reason = "khung moi tao - xoa allow khi run() dung tham so")]
struct NodeParams {
    #[serde(default)]
    prompt: String,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::__CAT__,
    label_vi: "__LABEL__".to_string(),
    // TODO: cong vao/ra that cua node.
    inputs: vec![PortSpec::new("prompt", PortType::Text).many()],
    outputs: vec![PortSpec::new("out", PortType::Text).many()],
    require_any: vec![],
    credential: None,
    default_params: json!({ "prompt": "" }),
    params_schema: json!({
        "type": "object",
        "properties": { "prompt": { "type": "string", "title": "Prompt" } }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(ThisNode)
}

struct ThisNode;

#[async_trait]
impl NodeExecutor for ThisNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        _inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let _p: NodeParams = parse_params(cx.node_id, params)?;
        // TODO: kiem tham so + du doan dau ra (THUAN, khong I/O).
        Ok(Preview::single(Outputs::new()))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        _inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let _p: NodeParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        // TODO: lam viec that - chi cham the gioi ngoai qua `ctx` (services, cancel, limits).
        Err(NodeError::new(format!("node {}: chua cai dat", ctx.node_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_khop_file_manifest_json() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json");
        let got = serde_json::to_string_pretty(node().manifest()).unwrap() + "\n";
        if std::env::var_os("LVC_WRITE_FIXTURES").is_some() {
            std::fs::write(path, &got).unwrap();
        }
        let want = std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(got, want, "manifest.json lech - chay lai voi LVC_WRITE_FIXTURES=1");
    }
}
'@
$lib = $lib.Replace("__KIND__", $Kind).Replace("__CAT__", $catPascal).Replace("__LABEL__", $Label)
Write-Utf8 (Join-Path $dir "src\lib.rs") $lib

Write-Utf8 (Join-Path $dir "CLAUDE.md") @"
# Node ``$Kind`` - luat

Hop dong chung: ``crates/lvc-node-api`` | Luat repo: ``CLAUDE.md`` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, crates/providers/* (+ thu vien ngoai). **Cam** node khac, ``lvc-engine``,
``lvc-store``, ``lvc-browser``, ``tauri``.

## Hop dong vao/ra
- Vao: TODO
- Ra: TODO
- Tai khoan: TODO (``credential`` trong manifest, vd ``{ provider: "gemini", kinds: ["api_key"] }``)

## Tham so
TODO

## Luat rieng
- ``preview`` THUAN (khong I/O). ``run`` chi cham the gioi ngoai qua ``NodeCtx``.
- That bai mot phan -> ``Ok`` + ``failure``; that bai hoan toan -> ``NodeError`` (``fatal`` chi khi phai dung ca run).

## Kiem tra
- ``cargo test -p $crate``
- Doi manifest: ``LVC_WRITE_FIXTURES=1 cargo test -p $crate`` roi sinh lai fixture UI
  ``LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures``.
"@

Write-Utf8 (Join-Path $dir "MEMORY.md") @"
# Ghi nho ky thuat - node ``$Kind``

Ghi nhung gi hoc duoc khi lam/chay node nay (quirk nha cung cap, ma loi, quyet dinh nho). Moi muc
mot dong, co ngay.

- ${today}: tao khung bang scripts/new-node.ps1.
"@

Write-Utf8 (Join-Path $dir "README.md") @"
# $Label

TODO: mo ta cho nguoi dung (lam gi, dau vao, dau ra, chi phi).

- Loai: ``$Kind``
- Hop dong day du: ``manifest.json``
"@

# Workspace: them member + dependency (chen truoc dong dong ngoac cua members / sau lvc-dola).
$ws = Join-Path $root "Cargo.toml"
$text = [System.IO.File]::ReadAllText($ws)
$member = "    `"crates/nodes/$crate`",`n]"
$text = [regex]::Replace($text, '(?s)(members = \[.*?)\n\]', { param($m) $m.Groups[1].Value + "`n" + $member }, 1)
$dep = "$crate = { path = `"crates/nodes/$crate`" }"
$text = [regex]::Replace($text, '(?m)^(lvc-dola = .*)$', { param($m) $m.Groups[1].Value + "`n" + $dep }, 1)
Write-Utf8 $ws $text

cargo fmt -p $crate
if ($LASTEXITCODE -ne 0) {
    Write-Host "cargo fmt that bai (exit $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}

Write-Host "Da tao $dir" -ForegroundColor Green
Write-Host "Them vao crates/lvc-app/Cargo.toml [dependencies]:" -ForegroundColor Cyan
Write-Host "  $crate.workspace = true"
Write-Host "Them vao crates/lvc-app/src/registry.rs (chuoi .with):" -ForegroundColor Cyan
Write-Host "  .with(${crateUnderscore}::node())"
Write-Host "Roi sinh manifest:" -ForegroundColor Cyan
Write-Host "  `$env:LVC_WRITE_FIXTURES=1; cargo test -p $crate; Remove-Item Env:LVC_WRITE_FIXTURES"
exit 0
