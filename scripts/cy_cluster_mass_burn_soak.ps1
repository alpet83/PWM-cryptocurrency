# CY E2E s3 - mass BurnMark batch soak harness
# Simulates offchain-aggregated burn burst landing on-chain.
# Use -SkipCluster -NoStopCluster when operator cluster already live.
# Example: ./scripts/cy_cluster_mass_burn_soak.ps1 -SkipCluster -NoStopCluster

param(
    [string]$RepoRoot = '',
    [string]$RpcUrl = 'http://127.0.0.1:3030',
    [string]$WalletPath = 'tmp/demo-genesis-wallet.yaml',
    [int64]$TargetMarksBurned = 1000000000,
    [int64]$MaxMarkAmountPerTx = 200000000,
    [int]$MinBlocksDuringBurn = 3,
    [int]$TxIntervalSeconds = 3,
    [switch]$SkipCluster,
    [switch]$NoStopCluster
)

$ErrorActionPreference = 'Stop'

if (-not $RepoRoot) { $RepoRoot = Split-Path -Parent $PSScriptRoot }
Set-Location -LiteralPath $RepoRoot

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$ReportPath = Join-Path $RepoRoot ('tmp/cy-e2e-s3-' + $ts + '.md')
$tmpDir = Join-Path $RepoRoot 'tmp'
if (-not (Test-Path -LiteralPath $tmpDir)) { New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null }

$report = New-Object System.Collections.Generic.List[string]

function Add-Rep([string]$s) { $script:report.Add($s); Write-Host $s }

function Save-Report {
    $content = $script:report -join "`r`n"
    [System.IO.File]::WriteAllText($script:ReportPath, $content, [System.Text.Encoding]::UTF8)
}

function Get-HeadHeight {
    try {
        $resp = Invoke-RestMethod -Uri ($RpcUrl + '/v1/head') -TimeoutSec 5
        return [int64]$resp.height
    } catch { return [int64]-1 }
}

function Get-AccountSnapshot {
    param([string]$AccountId)
    try {
        $resp = Invoke-RestMethod -Uri ($RpcUrl + '/v1/account/' + $AccountId) -TimeoutSec 5
        return @{
            id = $AccountId
            marks_stored = [int64]$resp.marks
            marks_last_block = [int64]$resp.marks_last_block
            staked = [double]$resp.staked
            nonce = [int64]$resp.nonce
        }
    } catch { return $null }
}

function Get-AccountInfoCLI {
    param([string]$Wallet)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    try {
        $out = & cargo run --quiet -p pwm-cli --bin pwm -- account-info --rpc $RpcUrl --wallet $Wallet 2>&1
        if ($LASTEXITCODE -ne 0) { return $null }
        $fields = @{}
        $acctId = $null
        foreach ($line in @($out)) {
            if ([string]$line -match '^([A-Za-z_]+)=(.*)$') { $fields[$Matches[1]] = $Matches[2] }
            if ([string]$line -match 'account=pwm1-[A-Z]+/[0-9A-Za-z]+-t([0-9a-f]+)') { $acctId = $Matches[1] }
        }
        return @{
            account_id_suffix = $acctId
            marks_stored = [int64]$fields['marks_stored']
            marks_effective = [int64]$fields['marks_effective']
            marks_sat_pct = [int]$fields['marks_sat_pct']
            marks_last_block = [int64]$fields['marks_last_block']
            staked = [int64]$fields['staked']
        }
    } catch { return $null } finally { $ErrorActionPreference = $prev }
}

function Invoke-BurnMark {
    param([string]$Wallet, [int64]$Amount, [string]$Purpose)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    try {
        $burnArgs = @(
            'run', '--quiet', '-p', 'pwm-cli', '--bin', 'pwm', '--',
            '--rpc', $RpcUrl,
            'tx-burn-mark',
            '--wallet', $Wallet,
            '--mark-amount', $Amount.ToString(),
            '--purpose', $Purpose
        )
        $out = & cargo @burnArgs 2>&1
        $rc = $LASTEXITCODE
        $outStr = ($out | ForEach-Object { [string]$_ }) -join ' '
        return @{ ok = ($rc -eq 0); rc = $rc; output = $outStr }
    } catch {
        return @{ ok = $false; rc = -1; output = $_.Exception.Message }
    } finally {
        $ErrorActionPreference = $prev
    }
}

Add-Rep '# CY E2E s3 - mass BurnMark batch soak'
Add-Rep ''
Add-Rep ('- Started: ' + $ts)
Add-Rep ('- Host: ' + $env:COMPUTERNAME)
Add-Rep ('- Repo: ' + $RepoRoot)
Add-Rep ('- RpcUrl: ' + $RpcUrl)
Add-Rep ('- TargetMarksBurned: ' + $TargetMarksBurned)
Add-Rep ('- MaxMarkAmountPerTx: ' + $MaxMarkAmountPerTx)
Add-Rep ('- TxIntervalSeconds: ' + $TxIntervalSeconds)
Add-Rep ('- MinBlocksDuringBurn: ' + $MinBlocksDuringBurn)
Add-Rep ''

# --- Preflight ---
Add-Rep '## Preflight'
Add-Rep ''

$headStart = Get-HeadHeight
Add-Rep ('- head_height_start: ' + $headStart)
if ($headStart -lt 0) {
    Add-Rep 'FATAL: RPC unreachable'
    Save-Report; exit 1
}

$walletInfo = Get-AccountInfoCLI -Wallet $WalletPath
if (-not $walletInfo) {
    Add-Rep 'FATAL: account-info CLI failed for primary wallet'
    Save-Report; exit 1
}

$primaryId = $walletInfo.account_id_suffix
Add-Rep ('- primary_wallet_account_suffix: ' + $primaryId)
Add-Rep ('- marks_stored: ' + $walletInfo.marks_stored)
Add-Rep ('- marks_effective: ' + $walletInfo.marks_effective)
Add-Rep ('- marks_sat_pct: ' + $walletInfo.marks_sat_pct + '%')
Add-Rep ('- staked: ' + $walletInfo.staked)
Add-Rep ''

if ($walletInfo.marks_effective -lt 1) {
    Add-Rep 'FATAL: primary account has no effective marks to burn'
    Save-Report; exit 1
}

# Find all staked accounts for secondary participation attempt
$stakedAccounts = @()
try {
    $allAccts = Invoke-RestMethod -Uri ($RpcUrl + '/v1/accounts') -TimeoutSec 10
    $stakedAccounts = @($allAccts.accounts | Where-Object { [double]$_.staked -gt 0 })
} catch { }
Add-Rep ('- staked_accounts_found: ' + $stakedAccounts.Count)

# --- Warm-up touch ---
Add-Rep '## Warm-up touch'
Add-Rep ''
Add-Rep '(First tx materializes lazy effective marks into stored)'
Add-Rep ''

$touchAmount = 1
$touchResult = Invoke-BurnMark -Wallet $WalletPath -Amount $touchAmount -Purpose 's3-warmup-touch'
if (-not $touchResult.ok) {
    Add-Rep ('- touch_result: FAIL rc=' + $touchResult.rc + ' output=' + $touchResult.output)
    Add-Rep 'FATAL: warm-up touch failed; cannot proceed with bulk burn'
    Save-Report; exit 1
}
Add-Rep ('- touch_result: OK rc=0 amount=' + $touchAmount)
Start-Sleep -Seconds 3

$afterTouch = Get-AccountInfoCLI -Wallet $WalletPath
if ($afterTouch) {
    Add-Rep ('- after_touch: stored=' + $afterTouch.marks_stored + ' effective=' + $afterTouch.marks_effective + ' sat_pct=' + $afterTouch.marks_sat_pct + '%')
} else {
    $snapTouch = Get-AccountSnapshot -AccountId ($stakedAccounts | Where-Object { $_.id -like ('*' + $primaryId) } | Select-Object -First 1 -ExpandProperty id)
    if ($snapTouch) { Add-Rep ('- after_touch: stored=' + $snapTouch.marks_stored) }
}
Add-Rep ''

# --- Bulk burn loop ---
Add-Rep '## Bulk burn'
Add-Rep ''
Add-Rep ('| Tx# | Amount | TotalBurned | Head | Result | Notes |')
Add-Rep ('|-----|--------|-------------|------|--------|-------|')

$totalBurned = [int64]$touchAmount
$txCount = 1
$rejectCount = 0

# Resolve primary account full ID (highest staked)
$primaryFullId = $null
$prevEA = $ErrorActionPreference; $ErrorActionPreference = 'SilentlyContinue'
try {
    $allAcctsNow = Invoke-RestMethod -Uri ($RpcUrl + '/v1/accounts') -TimeoutSec 10
    $highestStaked = $allAcctsNow.accounts | Sort-Object { [double]$_.staked } -Descending | Select-Object -First 1
    if ($highestStaked) { $primaryFullId = [string]$highestStaked.id }
} catch {}
$ErrorActionPreference = $prevEA
Add-Rep ('- primary_full_id: ' + $(if ($primaryFullId) { $primaryFullId.Substring(0,16) } else { 'unknown' }))
$headBurnStart = Get-HeadHeight
Add-Rep ('- head_burn_start: ' + $headBurnStart)

$txIdx = 1
while ($totalBurned -lt $TargetMarksBurned) {
    # Re-check available stored marks from RPC (materialized after touch)
    $available = [int64]0
    if ($primaryFullId) {
        $prevEA2 = $ErrorActionPreference; $ErrorActionPreference = 'SilentlyContinue'
        try {
            $snapNow = Invoke-RestMethod -Uri ($RpcUrl + '/v1/account/' + $primaryFullId) -TimeoutSec 5
            $available = [int64]$snapNow.marks
        } catch {} finally { $ErrorActionPreference = $prevEA2 }
    }
    if ($available -le 0) {
        Add-Rep ('| ' + $txIdx + ' | - | ' + $totalBurned + ' | - | STOP: no_marks | primary exhausted |')
        break
    }
    $remaining = $TargetMarksBurned - $totalBurned
    $burnAmt = [math]::Min([math]::Min($available, $MaxMarkAmountPerTx), $remaining)
    if ($burnAmt -le 0) { break }

    $purposeStr = 's3-batch-' + $txIdx + '-{utc_timestamp}'
    $result = Invoke-BurnMark -Wallet $WalletPath -Amount $burnAmt -Purpose $purposeStr
    $headNow = Get-HeadHeight
    $txCount++
    $txIdx++

    if ($result.ok) {
        $totalBurned += $burnAmt
        Add-Rep ('| ' + ($txIdx-1) + ' | ' + $burnAmt + ' | ' + $totalBurned + ' | ' + $headNow + ' | OK | |')
    } else {
        $rejectCount++
        $note = if ($result.output -match 'InsufficientMarks|insufficient') { 'InsufficientMarks' } else { 'err' }
        Add-Rep ('| ' + ($txIdx-1) + ' | ' + $burnAmt + ' | ' + $totalBurned + ' | ' + $headNow + ' | FAIL | ' + $note + ' |')
        if ($rejectCount -gt 5) {
            Add-Rep ('| - | - | ' + $totalBurned + ' | - | STOP: too_many_rejects | rejects=' + $rejectCount + ' |')
            break
        }
        Start-Sleep -Seconds 2
    }

    if ($totalBurned -ge $TargetMarksBurned) { break }
    Start-Sleep -Seconds $TxIntervalSeconds
}

$headBurnEnd = Get-HeadHeight
$headDelta = $headBurnEnd - $headBurnStart
Add-Rep ''

# --- Secondary account (2c6ec9f5) ---
Add-Rep '## Secondary account burn attempt'
Add-Rep ''
$secondAcct = $stakedAccounts | Where-Object { $_.id -notlike ('*' + $primaryId) } | Select-Object -First 1
if ($secondAcct) {
    Add-Rep ('- second_account: ' + $secondAcct.id.Substring(0,16) + ' staked=' + $secondAcct.staked + ' stored=' + $secondAcct.marks)
    $secondEffective = [int64]$secondAcct.marks
    $headNow2 = Get-HeadHeight
    $deltaBlocks2 = $headNow2 - [int64]$secondAcct.marks_last_block
    $deltaHours2 = [long]($deltaBlocks2 / 3600)
    $wholePwm2 = [long]([double]$secondAcct.staked / 1000000)
    $lazy2 = [long]($wholePwm2 * $deltaHours2)
    $secondEffective = [math]::Min([int64]$secondAcct.marks + $lazy2, [int64]4294967295)
    Add-Rep ('- second_effective_approx: ' + $secondEffective + ' (lazy_delta=' + $lazy2 + ' delta_hours=' + $deltaHours2 + ')')
    if ($secondEffective -gt 0) {
        Add-Rep '- attempting touch+burn on secondary account (no separate wallet; skip)'
        Add-Rep '- NOTE: secondary account wallet not available in this genesis; documenting participation gap'
    } else {
        Add-Rep '- NOTE: secondary account has zero effective marks; skipping'
    }
} else {
    Add-Rep '- NOTE: only 1 staked account with separate wallet; genesis constraint documented'
}
Add-Rep ''

# --- Verdict ---
$rejectTotal = if ($txCount -gt 1) { $rejectCount } else { 0 }
$rejectPct = if ($txCount -gt 1) { [math]::Round($rejectCount * 100.0 / ($txCount - 1), 1) } else { 0 }
$headDeltaOk = $headDelta -ge $MinBlocksDuringBurn
$burnOk = $totalBurned -ge $TargetMarksBurned
$rejectOk = $rejectPct -lt 1.0 -or $rejectCount -eq 0

$pass = $burnOk -and $headDeltaOk -and $rejectOk

Add-Rep '## Summary'
Add-Rep ''
Add-Rep ('- total_marks_burned: ' + $totalBurned)
Add-Rep ('- tx_count: ' + ($txCount - 1))
Add-Rep ('- reject_count: ' + $rejectCount + ' (' + $rejectPct + '%)')
Add-Rep ('- head_start: ' + $headBurnStart + ' head_end: ' + $headBurnEnd + ' delta: ' + $headDelta)
Add-Rep ('- target_met: ' + $burnOk + ' (target=' + $TargetMarksBurned + ')')
Add-Rep ('- head_delta_ok: ' + $headDeltaOk + ' (min=' + $MinBlocksDuringBurn + ')')
Add-Rep ('- reject_rate_ok: ' + $rejectOk)
Add-Rep ''

if ($pass) {
    Add-Rep '## PASS'
    Add-Rep ''
    Add-Rep ('PASS_EVIDENCE: soak=s3 total_marks_burned=' + $totalBurned + ' tx_count=' + ($txCount-1) + ' head_delta=' + $headDelta + ' reject_pct=' + $rejectPct + '% duration_blocks=' + $headDelta)
} else {
    Add-Rep '## FAIL'
    Add-Rep ''
    if (-not $burnOk) { Add-Rep ('- FAIL: total_burned=' + $totalBurned + ' < target=' + $TargetMarksBurned) }
    if (-not $headDeltaOk) { Add-Rep ('- FAIL: head_delta=' + $headDelta + ' < min=' + $MinBlocksDuringBurn) }
    if (-not $rejectOk) { Add-Rep ('- FAIL: reject_rate=' + $rejectPct + '% >= 1%') }
}

Add-Rep ''
Add-Rep ('Report: ' + $ReportPath)

Save-Report
exit $(if ($pass) { 0 } else { 1 })
