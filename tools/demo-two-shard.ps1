param(
    [string]$RepoRoot = ".",
    [int]$PortA = 3030,
    [int]$PortB = 3031,
    [string]$StateRootA = "state-shard-a",
    [string]$StateRootB = "state-shard-b"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = Resolve-Path $RepoRoot
$pwmdA = "cargo run -p pwmd --bin pwmd -- --shard A --listen 127.0.0.1:$PortA --state-root $StateRootA"
$pwmdB = "cargo run -p pwmd --bin pwmd -- --shard B --listen 127.0.0.1:$PortB --state-root $StateRootB"

Write-Host "Repo: $repo"
Write-Host "Start shard A in terminal #1:"
Write-Host "  $pwmdA"
Write-Host ""
Write-Host "Start shard B in terminal #2:"
Write-Host "  $pwmdB"
Write-Host ""
Write-Host "Health checks:"
Write-Host "  Invoke-RestMethod -Uri `"http://127.0.0.1:$PortA/v1/head`""
Write-Host "  Invoke-RestMethod -Uri `"http://127.0.0.1:$PortB/v1/head`""
Write-Host ""
Write-Host "CLI target switch:"
Write-Host "  `$env:PWM_RPC=`"http://127.0.0.1:$PortA`""
Write-Host "  `$env:PWM_RPC=`"http://127.0.0.1:$PortB`""
