# Cong chat luong P0: chay truoc moi commit lon.
# LUU Y: $ErrorActionPreference KHONG bat exit code cua exe native trong PS 5.1,
# nen phai kiem $LASTEXITCODE sau MOI lenh cargo. Thieu no thi cong luon xanh gia.
param(
    # P3 Task 8: bo qua 3 buoc frontend (pnpm) du app/package.json da co — dung khi may
    # khong co Node/pnpm hoac chi muon kiem phan Rust.
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"

function Invoke-Gate([string]$Label, [scriptblock]$Cmd) {
    Write-Host "== $Label ==" -ForegroundColor Cyan
    & $Cmd
    if ($LASTEXITCODE -ne 0) {
        Write-Host "THAT BAI: $Label (exit $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

Invoke-Gate "fmt"    { cargo fmt --all -- --check }
Invoke-Gate "clippy" { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Gate "test"   { cargo test --workspace }

Write-Host "== rang buoc: lvc-core phai thuan ==" -ForegroundColor Cyan
$tree = cargo tree -p lvc-core --prefix none --no-dedupe | Out-String
if ($LASTEXITCODE -ne 0) {
    Write-Host "THAT BAI: cargo tree (exit $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}
foreach ($bad in @("sqlx", "tokio", "reqwest", "chromiumoxide", "tauri")) {
    if ($tree -match "(?m)^\s*$bad\s") {
        Write-Host "VI PHAM: lvc-core phu thuoc $bad" -ForegroundColor Red
        exit 1
    }
}

# P3 Task 8: frontend (app/, Task 11) chua ton tai tren checkout hien tai — chi chay 3 buoc
# pnpm khi da co app/package.json, va co the tat het bang -SkipFrontend.
if ($SkipFrontend) {
    Write-Host "== frontend (app/) BO QUA (-SkipFrontend) ==" -ForegroundColor Yellow
}
elseif (Test-Path "app/package.json") {
    Invoke-Gate "frontend typecheck" { pnpm -C app typecheck }
    Invoke-Gate "frontend test"      { pnpm -C app test }
    Invoke-Gate "frontend build"     { pnpm -C app build }
}
else {
    Write-Host "== frontend (app/) BO QUA (chua co app/package.json) ==" -ForegroundColor Yellow
}

Write-Host "TAT CA XANH" -ForegroundColor Green
exit 0
