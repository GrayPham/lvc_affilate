# P8 (auto-update): dong bo version o 3 noi (Cargo.toml workspace.package, app/package.json,
# crates/lvc-app/tauri.conf.json) roi commit + tao git tag "v<version>" CUC BO (khong tu push).
# Vi du: powershell -ExecutionPolicy Bypass -File scripts/bump-version.ps1 -Version 1.1.0
# Sau khi chay: tu kiem tra `git log -1`/`git tag`, roi `git push && git push --tags` khi san sang
# (push tag kich hoat .github/workflows/release.yml).
param(
    [Parameter(Mandatory = $true)][string]$Version
)

$ErrorActionPreference = "Stop"

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Host "Version phai dang x.y.z (semver, khong tien to v)" -ForegroundColor Red
    exit 1
}

$root = Split-Path -Parent $PSScriptRoot
$cargoToml = Join-Path $root "Cargo.toml"
$packageJson = Join-Path $root "app\package.json"
$tauriConf = Join-Path $root "crates\lvc-app\tauri.conf.json"

function Write-Utf8NoBom([string]$Path, [string]$Text) {
    $enc = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Text, $enc)
}

# Cargo.toml: chi doi dong "version = ..." BEN TRONG khoi [workspace.package] (khong dung cho
# member crate nao vi ho dung version.workspace = true).
$cargo = Get-Content $cargoToml -Raw
if ($cargo -notmatch '(?ms)(\[workspace\.package\][^\[]*?version\s*=\s*)"[^"]*"') {
    Write-Host "Khong tim thay version trong [workspace.package] cua Cargo.toml" -ForegroundColor Red
    exit 1
}
$cargo = [regex]::Replace($cargo, '(?ms)(\[workspace\.package\][^\[]*?version\s*=\s*)"[^"]*"', { param($m) $m.Groups[1].Value + "`"$Version`"" }, 1)
Write-Utf8NoBom $cargoToml $cargo

# app/package.json: truong "version" dau tien (top-level, nam truoc "type"/"scripts").
$pkg = Get-Content $packageJson -Raw
$pkg = [regex]::Replace($pkg, '"version"\s*:\s*"[^"]*"', "`"version`": `"$Version`"", 1)
Write-Utf8NoBom $packageJson $pkg

# tauri.conf.json: truong "version" top-level.
$conf = Get-Content $tauriConf -Raw
$conf = [regex]::Replace($conf, '"version"\s*:\s*"[^"]*"', "`"version`": `"$Version`"", 1)
Write-Utf8NoBom $tauriConf $conf

Write-Host "Da ghi version $Version vao Cargo.toml / app/package.json / tauri.conf.json" -ForegroundColor Green

git -C $root add "Cargo.toml" "app/package.json" "crates/lvc-app/tauri.conf.json"
git -C $root commit -m "chore(release): v$Version"
git -C $root tag "v$Version"

Write-Host "Da commit + tao tag v$Version (CUC BO, chua push)." -ForegroundColor Green
Write-Host "Kiem tra lai roi chay: git push && git push --tags" -ForegroundColor Cyan
