# CY E2E s2 — marks saturation soak harness
# Owner cluster stays up; use -SkipCluster -NoStopCluster for live soak.
# Example: ./scripts/cy_cluster_marks_soak.ps1 -SkipCluster -NoStopCluster

param(
    [string]$RepoRoot = '',
    [string]$RpcUrl = 'http://127.0.0.1:3030',
    [int]$SampleIntervalMinutes = 15,
    [int]$SoakHours = 2,
    [string]$WalletPath = 'tmp/demo-genesis-wallet.yaml',
    [switch]$SkipCluster,
    [switch]$NoStopCluster
)

$ErrorActionPreference = 'Stop'

if (-not $RepoRoot) { $RepoRoot = Split-Path -Parent $PSScriptRoot }
Set-Location -LiteralPath $RepoRoot

$MarksCap = 4294967295

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$ReportPath = Join-Path $RepoRoot "tmp/cy-e2e-s2-$ts.md"
$tmpDir = Join-Path $RepoRoot 'tmp'
if (-not (Test-Path -LiteralPath $tmpDir)) { New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null }

$report = New-Object System.Collections.Generic.List[string]

function Add-Rep([string]$s) { $script:report.Add($s); Write-Host $s }

Add-Rep "# CY E2E s2 — marks saturation soak"
Add-Rep ""
Add-Rep "- Started: $ts"
Add-Rep "- Host: $env:COMPUTERNAME"
Add-Rep "- Repo: $RepoRoot"
Add-Rep "- RpcUrl: $RpcUrl"
Add-Rep "- SampleIntervalMinutes: $SampleIntervalMinutes"
Add-Rep "- SoakHours: $SoakHours"
Add-Rep "- Mode: EarlyExitOnPass"
Add-Rep ""

function Get-HeadHeight {
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/head" -TimeoutSec 5
        return [int64]$resp.height
    } catch { return -1 }
}

function Get-StakedAccounts {
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/accounts" -TimeoutSec 10
        return @($resp.accounts | Where-Object { [int64]$_.staked -gt 0 })
    } catch { return @() }
}

function Get-AccountSnapshot {
    param([string]$AccountId)
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$AccountId" -TimeoutSec 5
        return @{
            marks_stored = [int64]$resp.marks
            marks_last_block = [int64]$resp.marks_last_block
            staked = [double]$resp.staked
        }
    } catch { return $null }
}

function Write-CycleRow {
    param($c, $tsNow, $elapsed, $head, $acct)
    $elapsedStr = ([int]$elapsed).ToString() + 'min'
    $row = '| ' + $c + ' | ' + $tsNow + ' | ' + $elapsedStr + ' | ' + $head + ' | ' + $acct.Id.Substring(0, 16) + ' | ' + $acct.MarksStored + ' | ' + $acct.MarksEffective + ' | ' + $acct.MarksSatPct + '% | ' + $acct.MarksLastBlock + ' | ' + $acct.Staked + ' |'
    Add-Rep $row
}

function Get-AccountInfoCLI {
    param([string]$Wallet)
    try {
        $cargoCmd = @("run", "--quiet", "-p", "pwm-cli", "--bin", "pwm", "--", "account-info", "--rpc", $RpcUrl, "--wallet", $Wallet)
        $out = & cargo @cargoCmd 2>&1
        if ($LASTEXITCODE -ne 0) { return $null }
        $fields = @{}
        foreach ($line in @($out)) {
            if ([string]$line -match '^([A-Za-z_]+)=(.*)$') {
                $fields[$Matches[1]] = $Matches[2]
            }
        }
        # Extract short account id prefix from "account=pwm1-CY/XX-..." line
        $acctLine = @($out) | Where-Object { [string]$_ -match '^account=' } | Select-Object -First 1
        $acctId = $null
        if ($acctLine -match 'account=pwm1-[A-Z]+/[0-9A-Za-z]+-t([0-9a-f]+)') {
            $acctId = $Matches[1]
        }
        return @{
            account_id_suffix = $acctId
            marks_effective = [int64]$fields['marks_effective']
            marks_sat_pct = [int]$fields['marks_sat_pct']
            marks_last_block = [int64]$fields['marks_last_block']
            staked = [int64]$fields['staked']
        }
    } catch { return $null }
}

# Compute lazy marks_effective from RPC snapshot using pwm-core formula:
# delta_hours = delta_blocks / blocks_per_hour (integer)
# whole_pwm = staked_raw / 1_000_000
# per_hour = whole_pwm * marks_per_hour (default 1)
# generated = per_hour * delta_hours; effective = min(stored + generated, MARKS_CAP)
function Compute-LazyEffective {
    param([int64]$StoredMarks, [int64]$MarksLastBlock, [int64]$CurrentHead, [double]$StakedRaw,
          [int]$BlocksPerHour = 3600, [int]$MarksPerHour = 1)
    if ($StoredMarks -ge $script:MarksCap -or $CurrentHead -le $MarksLastBlock) { return [int64]$StoredMarks }
    if ($BlocksPerHour -eq 0) { return [int64]$StoredMarks }
    $deltaBlocks = $CurrentHead - $MarksLastBlock
    $deltaHours = [long]($deltaBlocks / $BlocksPerHour)
    if ($deltaHours -le 0) { return [int64]$StoredMarks }
    $wholePwm = [long]($StakedRaw / 1000000)
    if ($wholePwm -le 0 -or $MarksPerHour -le 0) { return [int64]$StoredMarks }
    $perHour = [long]($wholePwm * $MarksPerHour)
    $remaining = [long]($script:MarksCap - $StoredMarks)
    $saturHours = [long][math]::Ceiling($remaining / $perHour)
    $effectiveHours = [math]::Min($deltaHours, $saturHours)
    $generated = [long]($perHour * $effectiveHours)
    $eff = [long]$StoredMarks + $generated
    if ($eff -ge $script:MarksCap) { return [int64]$script:MarksCap }
    return [int64]$eff
}

$startEpoch = Get-Date
$deadline = $startEpoch.AddHours($SoakHours)
$cycle = 0
$pass = $false
$targetAccounts = @()

Add-Rep "## Preflight"
Add-Rep ""

$head0 = Get-HeadHeight
Add-Rep "- Head at start: $head0"

$staked = Get-StakedAccounts
Add-Rep "- Staked accounts found: $($staked.Count)"

if ($staked.Count -lt 1) {
    Add-Rep "FATAL: No staked accounts found."
    Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
    exit 1
}

$walletExists = Test-Path -LiteralPath $WalletPath -PathType Leaf

# Fetch wallet account info once (applies only to that wallet's account)
$walletInfo = $null
if ($walletExists) {
    $walletInfo = Get-AccountInfoCLI -Wallet $WalletPath
}

$targetAccounts = @()
foreach ($acct in $staked) {
    $id = [string]$acct.id
    $snap = Get-AccountSnapshot -AccountId $id
    if (-not $snap) { continue }
    $eff = 0
    $satPct = 0
    # Apply CLI info only if this account matches the wallet account
    $usedCLI = $false
    if ($walletInfo -and $walletInfo.account_id_suffix -and $id -like ('*' + $walletInfo.account_id_suffix)) {
        $eff = $walletInfo.marks_effective
        $satPct = $walletInfo.marks_sat_pct
        $snap.marks_last_block = $walletInfo.marks_last_block
        $snap.staked = $walletInfo.staked
        $usedCLI = $true
    }
    if (-not $usedCLI) {
        # Compute lazy effective from RPC snapshot data
        $eff = Compute-LazyEffective -StoredMarks $snap.marks_stored -MarksLastBlock $snap.marks_last_block -CurrentHead $head0 -StakedRaw $snap.staked
        $satPct = if ($eff -ge $MarksCap) { 100 } else { [int]($eff * 100 / $MarksCap) }
    }
    $targetAccounts += [pscustomobject]@{
        Id = $id
        MarksStored = $snap.marks_stored
        MarksEffective = $eff
        MarksSatPct = $satPct
        MarksLastBlock = $snap.marks_last_block
        Staked = $snap.staked
    }
}

if ($targetAccounts.Count -gt 0) {
    Add-Rep ("- Primary sample: id={0} eff={1} sat={2}% last_block={3}" -f $targetAccounts[0].Id, $targetAccounts[0].MarksEffective, $targetAccounts[0].MarksSatPct, $targetAccounts[0].MarksLastBlock)
}

Add-Rep ""
Add-Rep "## Time series"
Add-Rep ""
Add-Rep "| Cycle | Timestamp | Elapsed | Head | AccountId | Stored | Effective | Sat% | LastBlock | Staked |"
Add-Rep "|-------|-----------|---------|------|-----------|--------|-----------|------|-----------|--------|"

foreach ($acct in $targetAccounts) {
    Write-CycleRow -c 0 -tsNow $ts -elapsed 0 -head $head0 -acct $acct
}

while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds ($SampleIntervalMinutes * 60)
    $cycle++
    $now = Get-Date
    $elapsed = [math]::Round(($now - $startEpoch).TotalMinutes, 1)
    $tsNow = Get-Date -Format 'HH:mm:ss'
    $head = Get-HeadHeight

    $allPass = $true
    $updated = @()
    # Refresh wallet CLI once per cycle
    $cycleWalletInfo = $null
    if ($walletExists) { $cycleWalletInfo = Get-AccountInfoCLI -Wallet $WalletPath }
    foreach ($acct in $targetAccounts) {
        $snap = Get-AccountSnapshot -AccountId $acct.Id
        if (-not $snap) { $allPass = $false; continue }
        $eff = $acct.MarksEffective
        $satPct = $acct.MarksSatPct
        # Apply CLI info only to matching wallet account
        $usedCLI = $false
        if ($cycleWalletInfo -and $cycleWalletInfo.account_id_suffix -and $acct.Id -like ('*' + $cycleWalletInfo.account_id_suffix)) {
            $eff = $cycleWalletInfo.marks_effective
            $satPct = $cycleWalletInfo.marks_sat_pct
            $usedCLI = $true
        }
        if (-not $usedCLI) {
            $eff = Compute-LazyEffective -StoredMarks $snap.marks_stored -MarksLastBlock $snap.marks_last_block -CurrentHead $head -StakedRaw $snap.staked
            $satPct = if ($eff -ge $MarksCap) { 100 } else { [int]($eff * 100 / $MarksCap) }
        }
        $newAcct = [pscustomobject]@{ Id=$acct.Id; MarksStored=$snap.marks_stored; MarksEffective=$eff; MarksSatPct=$satPct; MarksLastBlock=$snap.marks_last_block; Staked=$snap.staked }
        Write-CycleRow -c $cycle -tsNow $tsNow -elapsed $elapsed -head $head -acct $newAcct
        if ($newAcct.MarksSatPct -lt 100 -or $newAcct.MarksEffective -lt $MarksCap) { $allPass = $false }
        $updated += $newAcct
    }
    $targetAccounts = $updated

    if ($allPass) {
        Add-Rep ""
        Add-Rep "## PASS — all accounts reached MARKS_CAP"
        Add-Rep ""
        Add-Rep ('PASS_EVIDENCE: soak=s2 elapsed=' + [int]$elapsed + 'min head=' + $head + ' accounts=' + $targetAccounts.Count + ' all_sat_pct=100 all_eff=cap')
        $pass = $true
        break
    }

    Add-Rep ('- waiting for saturation (cycle=' + $cycle + ', elapsed=' + [int]$elapsed + 'min)')
}

if (-not $pass) {
    Add-Rep ""
    Add-Rep "## FAIL — timeout without saturation"
    Add-Rep ""
    foreach ($acct in $targetAccounts) {
        $status = if ($acct.MarksSatPct -ge 100) { "cap" } else { "partial" }
        Add-Rep ('- ' + $acct.Id.Substring(0,16) + ': eff=' + $acct.MarksEffective + ' sat=' + $acct.MarksSatPct + '% status=' + $status)
    }
}

$exitCode = if ($pass) { 0 } else { 1 }

Add-Rep ""
$verdictStr = if ($exitCode -eq 0) { 'PASS' } else { 'FAIL' }
Add-Rep "**Verdict: $verdictStr**"
Add-Rep ""
Add-Rep ("Report: $ReportPath")

Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
exit $exitCode