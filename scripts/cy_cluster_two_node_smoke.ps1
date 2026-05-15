# Spawn CY lab proposer + attester with log capture; wait; stop pwmd; print sync summary.
# Standby attester: no Sync progress (product policy); PASS uses snapshot ready + optional checkpoint/peer health.
# Legacy Sync progress summary still printed for info if present (e.g. older builds or Active).
# For operator "sync quieted down" on Active nodes see docs/blockchain-sync.md and -RequireQuietTail.
# For pwm-testing / CQDS host runs: non-interactive, no TUI.
param(
    [int] $SmokeSeconds = 120,
    [int] $ProposerLeadSeconds = 8,
    [string] $RepoRoot = '',
    [switch] $RequireQuietTail
)
$ErrorActionPreference = 'Stop'
if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
Set-Location -LiteralPath $RepoRoot

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$logDir = Join-Path $RepoRoot ("tmp\cy-smoke-$ts")
New-Item -ItemType Directory -Path $logDir -Force | Out-Null

$proposerPs1 = Join-Path $RepoRoot 'cy-cluster-proposer.ps1'
$attesterPs1 = Join-Path $RepoRoot 'cy-cluster-attester.ps1'
if (-not (Test-Path -LiteralPath $proposerPs1)) {
    Write-Error "Missing $proposerPs1"
}
if (-not (Test-Path -LiteralPath $attesterPs1)) {
    Write-Error "Missing $attesterPs1"
}

$proposerOut = Join-Path $logDir 'proposer.stdout.log'
$proposerErr = Join-Path $logDir 'proposer.stderr.log'
$attesterOut = Join-Path $logDir 'attester.stdout.log'
$attesterErr = Join-Path $logDir 'attester.stderr.log'

Write-Host "cy_cluster_two_node_smoke: logDir=$logDir smoke=${SmokeSeconds}s lead=${ProposerLeadSeconds}s"

$pArgs = @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $proposerPs1
)
$p = Start-Process -FilePath 'powershell.exe' -ArgumentList $pArgs `
    -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput $proposerOut -RedirectStandardError $proposerErr

Start-Sleep -Seconds $ProposerLeadSeconds

$aArgs = @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $attesterPs1
)
$a = Start-Process -FilePath 'powershell.exe' -ArgumentList $aArgs `
    -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput $attesterOut -RedirectStandardError $attesterErr

Start-Sleep -Seconds $SmokeSeconds

# Stop pwmd trees (cargo run children); ignore errors.
& taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
Start-Sleep -Milliseconds 500
# Wrapper PowerShell may still be running; close if possible
foreach ($proc in @($p, $a)) {
    if ($null -ne $proc -and -not $proc.HasExited) {
        try { $proc.Kill() } catch { }
    }
}

Write-Host "--- attester: Sync progress lines ---"
Select-String -Path $attesterOut, $attesterErr -Pattern 'Sync progress' -ErrorAction SilentlyContinue |
    ForEach-Object { $_.Line }

Write-Host "--- attester: snapshot + wait-for-init (if any) ---"
Select-String -Path $attesterOut, $attesterErr -Pattern 'snapshot startup load ok|loading_snapshot|peer session waiting for init ready|standby sync checkpoint' -ErrorAction SilentlyContinue |
    ForEach-Object { $_.Line }

Write-Host "--- attester stderr tail (pwmd often here) ---"
Get-Content -LiteralPath $attesterErr -Tail 40 -ErrorAction SilentlyContinue | ForEach-Object { $_ }
Write-Host "--- attester stdout tail ---"
Get-Content -LiteralPath $attesterOut -Tail 20 -ErrorAction SilentlyContinue | ForEach-Object { $_ }

$maxPct = 0
foreach ($line in (Select-String -Path $attesterOut, $attesterErr -Pattern 'Sync progress (\d+)%' -ErrorAction SilentlyContinue)) {
    if ($line.Matches.Count -gt 0) {
        $v = [int]$line.Matches[0].Groups[1].Value
        if ($v -gt $maxPct) { $maxPct = $v }
    }
}
Write-Host "--- SUMMARY max Sync progress % observed: $maxPct (attester; 0 expected for Standby) (logDir=$logDir) ---"

$attesterReady = $null -ne (Select-String -Path $attesterOut, $attesterErr -Pattern 'snapshot startup load ok|pwmd startup phase: ready \(snapshot loaded\)' -ErrorAction SilentlyContinue | Select-Object -First 1)
$proposerListen = $null -ne (Select-String -Path $proposerOut, $proposerErr -Pattern 'pwmd listening on http://' -ErrorAction SilentlyContinue | Select-Object -First 1)

if ($RequireQuietTail) {
    $tailLines = 28
    $maxProgInTail = 4
    $tailText = @()
    foreach ($path in @($attesterErr, $attesterOut)) {
        if (Test-Path -LiteralPath $path) {
            $tailText += @(Get-Content -LiteralPath $path -Tail $tailLines -ErrorAction SilentlyContinue)
        }
    }
    $progInTail = ($tailText | Select-String -Pattern 'Sync progress' -AllMatches).Count
    Write-Host "--- QUIET_CHECK tailLines~(stderr+stdout)=$tailLines Sync progress count in tail=$progInTail (max allowed=$maxProgInTail) ---"
    if ($progInTail -gt $maxProgInTail) {
        Write-Host "SMOKE_NOISY_TAIL: too many Sync progress lines in attester log tail (see docs/blockchain-sync.md)"
        exit 4
    }
    $standbyQuiet = $attesterReady -and $maxPct -eq 0
    if (-not $standbyQuiet -and $maxPct -lt 95) {
        Write-Host ('SMOKE_PARTIAL_QUIET: max progress ' + $maxPct + '% < 95 - did not near-complete catch-up')
        exit 5
    }
}

$logRoot = Join-Path $RepoRoot 'logs'
if (Test-Path -LiteralPath $logRoot) {
    $peerAtt = Get-ChildItem -LiteralPath $logRoot -Recurse -Filter 'pwmd-peer-cy-attester*.log' -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($null -ne $peerAtt) {
        Write-Host "--- latest peer log: $($peerAtt.FullName) ---"
        Select-String -LiteralPath $peerAtt.FullName -Pattern 'wire_decode_failed|u128 is not supported|peer sync on_tip|catchup|nack node_id' -ErrorAction SilentlyContinue |
            Select-Object -Last 25 | ForEach-Object { $_.Line }
    }
}

if ($attesterReady -and $proposerListen) {
    Write-Host 'SMOKE_PASS: attester snapshot/ready + proposer listening (Standby has no Sync progress by design)'
    exit 0
}
if ($maxPct -ge 5) {
    if ($RequireQuietTail) {
        Write-Host 'SMOKE_PASS: ge 5% and quiet-tail / near-full criteria met'
    } else {
        Write-Host 'SMOKE_PASS: reached ge 5% sync progress (legacy attester criterion)'
    }
    exit 0
}
if (-not $attesterReady) {
    Write-Host 'SMOKE_PARTIAL: attester did not reach snapshot ready (inspect logs)'
}
if (-not $proposerListen) {
    Write-Host 'SMOKE_PARTIAL: proposer did not show listening line (inspect logs)'
}
Write-Host 'SMOKE_PARTIAL: smoke criteria not met (inspect logs)'
exit 2
