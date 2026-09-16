# Cong chat luong P0: chay truoc moi commit lon.
# LUU Y: $ErrorActionPreference KHONG bat exit code cua exe native trong PS 5.1,
# nen phai kiem $LASTEXITCODE sau MOI lenh cargo. Thieu no thi cong luon xanh gia.
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

Write-Host "TAT CA XANH" -ForegroundColor Green
exit 0
