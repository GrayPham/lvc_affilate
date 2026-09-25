# Cong chat luong P0: chay truoc moi commit lon.
# LUU Y: $ErrorActionPreference KHONG bat exit code cua exe native trong PS 5.1,
# nen phai kiem $LASTEXITCODE sau MOI lenh cargo. Thieu no thi cong luon xanh gia.
param(
    # P3 Task 8: bo qua 3 buoc frontend (pnpm) du app/package.json da co - dung khi may
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
# Task 12a: che do dev "FakeDriver" (khong ton credit) la mot cargo feature rieng cua lvc-app -
# clippy mac dinh o tren KHONG bien dich no, nen kiem rieng de feature khong bi moc (rot).
Invoke-Gate "clippy (fake-driver)" { cargo clippy -p lvc-app --features fake-driver --all-targets -- -D warnings }
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

# P4a (ADR-R014): node doc lap - moi crate trong crates/nodes/* (va lvc-node-api, providers) KHONG
# duoc phu thuoc (dependency thuong, khong tinh dev-dependency) vao node khac / engine / store /
# browser / app / tauri. Doc ten crate tu Cargo.toml cua tung thu muc.
Write-Host "== rang buoc: node doc lap ==" -ForegroundColor Cyan
$forbiddenForAll = @("lvc-engine", "lvc-store", "lvc-browser", "lvc-app", "tauri", "chromiumoxide", "sqlx")
$nodeCrates = @(Get-ChildItem -Directory "crates/nodes" | ForEach-Object {
    ((Get-Content (Join-Path $_.FullName "Cargo.toml")) -match '^name\s*=' | Select-Object -First 1) -replace '^name\s*=\s*"(.*)"', '$1'
})
$providerCrates = @(Get-ChildItem -Directory "crates/providers" | ForEach-Object {
    ((Get-Content (Join-Path $_.FullName "Cargo.toml")) -match '^name\s*=' | Select-Object -First 1) -replace '^name\s*=\s*"(.*)"', '$1'
})
$checks = @()
foreach ($c in $nodeCrates) { $checks += , @($c, ($forbiddenForAll + ($nodeCrates | Where-Object { $_ -ne $c }))) }
foreach ($c in $providerCrates) { $checks += , @($c, ($forbiddenForAll + $nodeCrates + @("lvc-node-api"))) }
$checks += , @("lvc-node-api", ($forbiddenForAll + $nodeCrates + $providerCrates))
foreach ($pair in $checks) {
    $name = $pair[0]
    $deps = cargo tree -p $name -e normal --prefix none --no-dedupe | Out-String
    if ($LASTEXITCODE -ne 0) {
        Write-Host "THAT BAI: cargo tree -p $name (exit $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
    foreach ($bad in $pair[1]) {
        if ($deps -match "(?m)^\s*$([regex]::Escape($bad))\s") {
            Write-Host "VI PHAM: $name phu thuoc $bad (node phai doc lap - xem CLAUDE.md)" -ForegroundColor Red
            exit 1
        }
    }
}

# P3 Task 8: frontend (app/, Task 11) chua ton tai tren checkout hien tai - chi chay 3 buoc
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
