# Spawn CY lab proposer + attester with log capture; wait; stop pwmd; print sync summary.
# Standby attester: no Sync progress (product policy); PASS uses snapshot ready + optional checkpoint/peer health.
# Legacy Sync progress summary still printed for info if present (e.g. older builds or Active).
# For operator "sync quieted down" on Active nodes see docs/blockchain-sync.md and -RequireQuietTail.
# For pwm-testing / CQDS host runs: non-interactive, no TUI.
param(
    [int] $SmokeSeconds = 120,
    [int] $ProposerLeadSeconds = 8,
    [int] $StatusWaitSeconds = 180,
    [int] $MinBlocks = 5,
    [int] $QuorumTimeoutMax = 5,
    [string] $RpcUrl = 'http://127.0.0.1:3030',
    [string] $RepoRoot = '',
    [switch] $RequireQuietTail,
    [switch] $SkipCluster,
    [switch] $NoStopCluster
)
$ErrorActionPreference = 'Stop'
if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
Set-Location -LiteralPath $RepoRoot

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$logDir = Join-Path $RepoRoot ("tmp\cy-e2e-s1-$ts")
New-Item -ItemType Directory -Path $logDir -Force | Out-Null

$proposerPs1 = Join-Path $RepoRoot 'cy-cluster-proposer.ps1'
$attesterPs1 = Join-Path $RepoRoot 'cy-cluster-attester.ps1'
if (-not (Test-Path -LiteralPath $proposerPs1)) {
    Write-Error "Missing $proposerPs1"
}
if (-not (Test-Path -LiteralPath $attesterPs1)) {
    Write-Error "Missing $attesterPs1"
}

function Wait-RPCReady([string] $Url, [int] $MaxSec) {
    $deadline = (Get-Date).AddSeconds($MaxSec)
    while ((Get-Date) -lt $deadline) {
        try {
            $null = Invoke-RestMethod -Uri "$Url/v1/status" -TimeoutSec 5
            return $true
        }
        catch {
            Start-Sleep -Seconds 2
        }
    }
    return $false
}

function Get-HeadHeight([string] $Url) {
    try {
        $resp = Invoke-RestMethod -Uri "$Url/v1/head" -TimeoutSec 10
        if ($null -ne $resp.height) {
            return [int64]$resp.height
        }
    }
    catch { }
    try {
        $resp = Invoke-RestMethod -Uri "$Url/v1/status" -TimeoutSec 10
        foreach ($key in @('head_height', 'height')) {
            if ($null -ne $resp.$key) {
                return [int64]$resp.$key
            }
        }
    }
    catch { }
    return [int64]-1
}

function Get-LatestClusterLogDir([string] $Root) {
    if (-not (Test-Path -LiteralPath $Root)) { return $null }
    $dir = Get-ChildItem -LiteralPath $Root -Directory -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($null -eq $dir) { return $null }
    return $dir.FullName
}

function Stop-CyLabPwmd([object] $ProposerProc, [object] $AttesterProc) {
    & taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
    Start-Sleep -Milliseconds 500
    foreach ($proc in @($ProposerProc, $AttesterProc)) {
        if ($null -ne $proc -and -not $proc.HasExited) {
            try { $proc.Kill() } catch { }
        }
    }
}

$proposerOut = Join-Path $logDir 'proposer.stdout.log'
$proposerErr = Join-Path $logDir 'proposer.stderr.log'
$attesterOut = Join-Path $logDir 'attester.stdout.log'
$attesterErr = Join-Path $logDir 'attester.stderr.log'

Write-Host "cy_cluster_two_node_smoke: logDir=$logDir smoke=${SmokeSeconds}s lead=${ProposerLeadSeconds}s statusWait=${StatusWaitSeconds}s minBlocks=$MinBlocks skipCluster=$SkipCluster noStopCluster=$NoStopCluster"

$p = $null
$a = $null
$attesterReady = $false
$proposerListen = $false
$headHeightStart = [int64]-1
$headHeightEnd = [int64]-1
$maxPct = 0
$rpcReady = $false

if (-not $SkipCluster) {
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
}

Write-Host "## wait RPC $RpcUrl"
if (-not (Wait-RPCReady $RpcUrl $StatusWaitSeconds)) {
    Write-Host 'SMOKE_PARTIAL: RPC not ready'
    if (-not $SkipCluster -and -not $NoStopCluster) {
        Stop-CyLabPwmd $p $a
    }
    exit 2
}

try {
    $rpcReady = $true
    $headHeightStart = Get-HeadHeight $RpcUrl
    if ($headHeightStart -lt 0) {
        Write-Host 'SMOKE_PARTIAL: could not read initial head_height'
    }
    else {
        Write-Host "initial head_height=$headHeightStart"
    }

    Start-Sleep -Seconds $SmokeSeconds
    $headHeightEnd = Get-HeadHeight $RpcUrl
    if ($headHeightEnd -ge 0) {
        Write-Host "final head_height=$headHeightEnd delta=$([int64]($headHeightEnd - $headHeightStart))"
    }
} catch {
    Write-Host "SMOKE_WARN: status sampling failed: $($_.Exception.Message)"
}

if (-not $SkipCluster) {
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
    foreach ($line in (Select-String -Path $attesterOut, $attesterErr -Pattern 'Sync progress (\d+)%' -ErrorAction SilentlyContinue)) {
        if ($line.Matches.Count -gt 0) {
            $v = [int]$line.Matches[0].Groups[1].Value
            if ($v -gt $maxPct) { $maxPct = $v }
        }
    }
    Write-Host "--- SUMMARY max Sync progress % observed: $maxPct (attester; 0 expected for Standby) (logDir=$logDir) ---"

    $attesterReady = $null -ne (Select-String -Path $attesterOut, $attesterErr -Pattern 'snapshot startup load ok|pwmd startup phase: ready \(snapshot loaded\)' -ErrorAction SilentlyContinue | Select-Object -First 1)
    $proposerListen = $null -ne (Select-String -Path $proposerOut, $proposerErr -Pattern 'pwmd listening on http://' -ErrorAction SilentlyContinue | Select-Object -First 1)
}

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

$scanRoot = if ($SkipCluster) { Get-LatestClusterLogDir (Join-Path $RepoRoot 'logs') } else { $logDir }
if ($null -ne $scanRoot) {
    Write-Host "--- log counters: $scanRoot ---"
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'scan_pwmd_log_counters.ps1') -LogDir $scanRoot
} else {
    Write-Host 'SMOKE_WARN: no log dir found for counter scan'
}

if ($SkipCluster) {
    $delta = [int64]($headHeightEnd - $headHeightStart)
    if ($headHeightStart -lt 0 -or $headHeightEnd -lt 0) {
        Write-Host 'SMOKE_PARTIAL: head height unavailable in verify-only mode'
        exit 2
    }
    if ($delta -lt $MinBlocks) {
        Write-Host "SMOKE_PARTIAL: head_height delta $delta < MinBlocks $MinBlocks"
        exit 2
    }
    Write-Host "SMOKE_PASS: verify-only mode head_height delta=$delta MinBlocks=$MinBlocks"
    exit 0
}

if ($attesterReady -and $proposerListen -and ($headHeightEnd -ge $headHeightStart) -and (($headHeightEnd - $headHeightStart) -ge $MinBlocks)) {
    Write-Host 'SMOKE_PASS: attester snapshot/ready + proposer listening + head_height advanced'
    if (-not $SkipCluster -and -not $NoStopCluster) {
        Stop-CyLabPwmd $p $a
    }
    exit 0
}
if (($headHeightEnd - $headHeightStart) -lt $MinBlocks) {
    Write-Host "SMOKE_PARTIAL: head_height delta $([int64]($headHeightEnd - $headHeightStart)) < MinBlocks $MinBlocks"
}
if (-not $attesterReady) {
    Write-Host 'SMOKE_PARTIAL: attester did not reach snapshot ready (inspect logs)'
}
if (-not $proposerListen) {
    Write-Host 'SMOKE_PARTIAL: proposer did not show listening line (inspect logs)'
}
Write-Host 'SMOKE_PARTIAL: smoke criteria not met (inspect logs)'
if (-not $NoStopCluster) {
    Stop-CyLabPwmd $p $a
}
exit 2
