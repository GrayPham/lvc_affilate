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
