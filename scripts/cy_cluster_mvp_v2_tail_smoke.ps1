# MVP v2 tail / V2-9 lab: preflight, CY cluster smoke (2 or 3 nodes), head convergence, optional relay burn via attester RPC.
# Encoding: UTF-8. Avoid double-quoted strings containing '>=', '%' sequences (Windows PowerShell 5.x ParserError).
# Attach mode: assume pwmd already running (--Attach); no spawn/taskkill.
param(
    [int] $SmokeSeconds = 90,
    [int] $ProposerLeadSeconds = 8,
    [int] $AttesterLeadSeconds = 6,
    [ValidateSet('2', '3')]
    [string] $NodeCount = '3',
    [switch] $Attach,
    [string] $RepoRoot = '',
    [switch] $RelayBurn,
    [string] $WalletPath = '',
    [string] $BurnAccountHex = '2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e',
    [int] $MaxHeadSpread = 5,
    [int] $MarkBurnAmount = 1
)
$ErrorActionPreference = 'Stop'
if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
Set-Location -LiteralPath $RepoRoot

$rpc0 = 'http://127.0.0.1:3030'
$rpc1 = 'http://127.0.0.2:3030'
$rpc2 = 'http://127.0.0.3:3030'

Write-Host "cy_cluster_mvp_v2_tail_smoke: NodeCount=$NodeCount Attach=$Attach smoke=${SmokeSeconds}s RelayBurn=$RelayBurn"

$preflight = Join-Path $RepoRoot 'tools\dev\preflight_target_debug.ps1'
if (-not (Test-Path -LiteralPath $preflight)) {
    Write-Host 'TAIL_SMOKE_FAIL: missing preflight script'
    exit 5
}
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $preflight
if ($LASTEXITCODE -ne 0) {
    Write-Host 'TAIL_SMOKE_FAIL: preflight_target_debug exit non-zero'
    exit 5
}

$logDir = $null
$p = $null
$a = $null
$f = $null

function Stop-CyLabProcesses {
    param($ProposerProc, $AttesterProc, $FollowerProc)
    & taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
    Start-Sleep -Milliseconds 600
    foreach ($proc in @($ProposerProc, $AttesterProc, $FollowerProc)) {
        if ($null -ne $proc -and -not $proc.HasExited) {
            try { $proc.Kill() } catch { }
        }
    }
}

function Get-HeadHeight {
    param([string]$Uri)
    try {
        $h = Invoke-RestMethod -Uri ($Uri + '/v1/head') -TimeoutSec 15
        return [int64]$h.height
    } catch {
        return -1
    }
}

$exitCode = 0
try {
    if (-not $Attach) {
        $ts = Get-Date -Format 'yyyyMMdd_HHmmss'
        $logDir = Join-Path $RepoRoot ("tmp\cy-tail-smoke-$ts")
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null

        $proposerPs1 = Join-Path $RepoRoot 'cy-cluster-proposer.ps1'
        $attesterPs1 = Join-Path $RepoRoot 'cy-cluster-attester.ps1'
        $followerPs1 = Join-Path $RepoRoot 'cy-cluster-follower.ps1'
        foreach ($req in @($proposerPs1, $attesterPs1)) {
            if (-not (Test-Path -LiteralPath $req)) { Write-Error "Missing $req" }
        }
        if ($NodeCount -eq '3' -and -not (Test-Path -LiteralPath $followerPs1)) {
            Write-Error "Missing $followerPs1"
        }

        $pOut = Join-Path $logDir 'proposer.stdout.log'
        $pErr = Join-Path $logDir 'proposer.stderr.log'
        $aOut = Join-Path $logDir 'attester.stdout.log'
        $aErr = Join-Path $logDir 'attester.stderr.log'
        $fOut = Join-Path $logDir 'follower.stdout.log'
        $fErr = Join-Path $logDir 'follower.stderr.log'

        $pArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $proposerPs1)
        $p = Start-Process -FilePath 'powershell.exe' -ArgumentList $pArgs `
            -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput $pOut -RedirectStandardError $pErr

        Start-Sleep -Seconds $ProposerLeadSeconds

        $aArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $attesterPs1)
        $a = Start-Process -FilePath 'powershell.exe' -ArgumentList $aArgs `
            -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput $aOut -RedirectStandardError $aErr

        Start-Sleep -Seconds $AttesterLeadSeconds

        if ($NodeCount -eq '3') {
            $fArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $followerPs1)
            $f = Start-Process -FilePath 'powershell.exe' -ArgumentList $fArgs `
                -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
                -RedirectStandardOutput $fOut -RedirectStandardError $fErr
        }

        Start-Sleep -Seconds $SmokeSeconds
        Write-Host "logDir=$logDir"
    }

    $heights = @()
    $h0 = Get-HeadHeight $rpc0
    $h1 = Get-HeadHeight $rpc1
    Write-Host "head proposer(.1)=$h0 attester(.2)=$h1"
    $heights += $h0
    $heights += $h1
    if ($NodeCount -eq '3') {
        $h2 = Get-HeadHeight $rpc2
        Write-Host "head follower(.3)=$h2"
        $heights += $h2
    }

    $bad = ($heights | Where-Object { $_ -lt 0 }).Count
    if ($bad -gt 0) {
        Write-Host 'TAIL_SMOKE_FAIL: one or more /v1/head unreachable (start lab or use Attach after boot)'
        $exitCode = 2
        throw 'head-unreachable'
    }

    $minH = ($heights | Measure-Object -Minimum).Minimum
    $maxH = ($heights | Measure-Object -Maximum).Maximum
    $spread = $maxH - $minH
    Write-Host "head spread=$spread (max allowed $MaxHeadSpread)"
    if ($spread -gt $MaxHeadSpread) {
        Write-Host 'TAIL_SMOKE_FAIL: head heights diverged beyond tolerance'
        $exitCode = 3
        throw 'head-spread'
    }

    if ($RelayBurn) {
        if (-not $WalletPath) {
            $WalletPath = Join-Path $RepoRoot 'tmp\cy-wallet.yaml'
        }
        if (-not (Test-Path -LiteralPath $WalletPath)) {
            Write-Host 'TAIL_SMOKE_FAIL: RelayBurn set but wallet file missing'
            $exitCode = 4
            throw 'no-wallet'
        }
        $pass = $env:PWM_WALLET_PASSPHRASE
        if (-not $pass) {
            Write-Host 'TAIL_SMOKE_FAIL: set PWM_WALLET_PASSPHRASE for RelayBurn'
            $exitCode = 4
            throw 'no-pass'
        }
        $marksBefore = -1
        try {
            $acc = Invoke-RestMethod -Uri ($rpc0 + '/v1/account/' + $BurnAccountHex) -TimeoutSec 15
            $marksBefore = [int64]$acc.marks
        } catch {
            Write-Host 'TAIL_SMOKE_WARN: could not read account before burn'
        }
        $burnArgs = @(
            'run', '-q', '-p', 'pwm-cli', '--bin', 'pwm', '--',
            '--rpc', $rpc1,
            '--wallet-passphrase', $pass,
            'tx-burn-mark',
            '--wallet', $WalletPath,
            '--mark-amount', "$MarkBurnAmount",
            '--purpose', 'mvp-v2-tail-smoke'
        )
        Push-Location $RepoRoot
        try {
            & cargo @burnArgs
            if ($LASTEXITCODE -ne 0) {
                Write-Host 'TAIL_SMOKE_FAIL: cargo pwm tx-burn-mark non-zero'
                $exitCode = 4
                throw 'burn-fail'
            }
        } finally {
            Pop-Location
        }
        Start-Sleep -Seconds 3
        try {
            $acc2 = Invoke-RestMethod -Uri ($rpc0 + '/v1/account/' + $BurnAccountHex) -TimeoutSec 15
            $marksAfter = [int64]$acc2.marks
            $nonceAfter = [int64]$acc2.nonce
            Write-Host "account after burn marks=$marksAfter nonce=$nonceAfter (before $marksBefore)"
            if ($marksBefore -ge 0 -and $marksAfter -ne ($marksBefore - $MarkBurnAmount)) {
                Write-Host 'TAIL_SMOKE_FAIL: marks delta mismatch after relay burn'
                $exitCode = 4
                throw 'burn-verify'
            }
        } catch {
            if ($exitCode -eq 0) {
                Write-Host 'TAIL_SMOKE_FAIL: could not verify account after burn'
                $exitCode = 4
            }
            throw
        }
    }

    Write-Host 'TAIL_SMOKE_PASS: preflight ok, heads within spread, optional burn ok'
    $exitCode = 0
} catch {
    if ($exitCode -eq 0) { $exitCode = 1 }
} finally {
    if (-not $Attach) {
        Stop-CyLabProcesses -ProposerProc $p -AttesterProc $a -FollowerProc $f
    }
}

exit $exitCode
