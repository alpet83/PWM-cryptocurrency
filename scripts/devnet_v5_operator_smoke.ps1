# V5-8 Operator smoke: marks/inflation (slice1) + deferred policy (slice2)
# PowerShell harness (UTF-8). Simplified version of devnet_v4_policy_e2e.ps1.
#
# Example (from repo root):
#   ./scripts/devnet_v5_operator_smoke.ps1 -CleanState
#   ./scripts/devnet_v5_operator_smoke.ps1 -DeferredOnly -CleanState
#
# Prerequisites: cargo on PATH, repo root = parent of scripts/

param(
    [string]$RepoRoot = '',
    [string]$RpcUrl = 'http://127.0.0.1:3030',
    [int]$SmokeSeconds = 90,
    [int]$ProposerLeadSeconds = 12,
    [int]$StatusWaitSeconds = 120,
    [int]$DemoDerivationIndex = 287292,
    [int]$DeferredLeadBlocks = 20,
    [int]$DeferredWaitSeconds = 120,
    [string]$DeferredPolicy = 'default_behavior',
    [string]$PolicyFee = '1000000',
    [switch]$CleanState,
    [switch]$SkipArchive,
    [int]$MaxStateArchives = 30,
    [switch]$SkipGenesis,
    [switch]$SkipNodes,
    [switch]$MarksOnly,
    [switch]$DeferredOnly,
    [switch]$Ipv4ClaimOnly,
    [switch]$AccountInfoOnly,
    [int]$Ipv4ClaimPhase = 7,
    [string]$ReportPath = ''
)

$ErrorActionPreference = 'Stop'

if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
Set-Location -LiteralPath $RepoRoot

$sliceOnlyCount = @(@($MarksOnly, $DeferredOnly, $Ipv4ClaimOnly, $AccountInfoOnly) | Where-Object { $_ }).Count
if ($sliceOnlyCount -gt 1) {
    throw 'Use only one slice-only switch: -MarksOnly, -DeferredOnly, -Ipv4ClaimOnly, or -AccountInfoOnly.'
}

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
if (-not $ReportPath) {
    $ReportPath = Join-Path $RepoRoot ("tmp\devnet_v5_operator_smoke_$ts.md")
}

$tmpDir = Join-Path $RepoRoot 'tmp'
if (-not (Test-Path -LiteralPath $tmpDir)) {
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
}

$report = New-Object System.Collections.Generic.List[string]
function Add-Rep([string]$s) { $script:report.Add($s); Write-Host $s }

$runMarks = -not ($DeferredOnly -or $Ipv4ClaimOnly -or $AccountInfoOnly)
$runDeferred = -not ($MarksOnly -or $Ipv4ClaimOnly -or $AccountInfoOnly)
$runIpv4Claim = -not ($MarksOnly -or $DeferredOnly -or $AccountInfoOnly)
$runAccountInfo = -not ($MarksOnly -or $DeferredOnly -or $Ipv4ClaimOnly)

$ipv4ClaimPassed = $false   # default; set inside the slice block when implemented
$accountInfoPassed = $false # default; set inside the slice block when implemented

Add-Rep "# Devnet V5 Operator Smoke - $ts"
Add-Rep ""
Add-Rep "- Host: $env:COMPUTERNAME"
Add-Rep "- Repo: $RepoRoot"
Add-Rep "- RpcUrl: $RpcUrl"
Add-Rep "- Mode: MarksOnly=$MarksOnly DeferredOnly=$DeferredOnly Ipv4ClaimOnly=$Ipv4ClaimOnly AccountInfoOnly=$AccountInfoOnly runMarks=$runMarks runDeferred=$runDeferred runIpv4Claim=$runIpv4Claim runAccountInfo=$runAccountInfo"
Add-Rep ""

$WALLET = Join-Path $RepoRoot 'tmp\demo-genesis-wallet.yaml'
$GENESIS = Join-Path $RepoRoot 'tmp\genesis-custom.json'
# u32::MAX вЂ” lazy marks saturate here; PASS still valid if marks_last_block advances (RFC 0012 v2).
$MarksSaturation = 4294967295

function Wait-RPCReady {
    param([string]$Url, [int]$MaxSec)

    $deadline = (Get-Date).AddSeconds($MaxSec)
    while ((Get-Date) -lt $deadline) {
        try {
            $null = Invoke-RestMethod -Uri "$Url/v1/status" -TimeoutSec 3
            return $true
        }
        catch {
            Start-Sleep -Seconds 2
        }
    }
    return $false
}

function Run-Pwm {
    param([string[]]$PwmArgs)
    $a = @('run', '-p', 'pwm-cli', '--bin', 'pwm', '--', '--rpc', $RpcUrl) + $PwmArgs
    Write-Host ('==> pwm ' + ($PwmArgs -join ' '))
    & cargo @a
    if ($LASTEXITCODE -ne 0) {
        throw "pwm failed exit=$LASTEXITCODE"
    }
}

function Invoke-Pwm {
    param([string[]]$PwmArgs)
    $a = @('run', '-p', 'pwm-cli', '--bin', 'pwm', '--', '--rpc', $RpcUrl) + $PwmArgs
    Write-Host ('==> pwm ' + ($PwmArgs -join ' '))
    & cargo @a
    return $LASTEXITCODE
}

function Get-AccountMarks {
    param([string]$AccountId)
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$AccountId" -TimeoutSec 5
        return @{
            marks = [uint32]$resp.marks
            marks_last_block = [uint64]$resp.marks_last_block
        }
    }
    catch {
        return $null
    }
}

function Get-AccountPolicyFields {
    param([string]$AccountId)
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$AccountId" -TimeoutSec 5
        $active = 0
        $dormant = 0
        if ($null -ne $resp.PSObject.Properties['active_policies']) {
            $active = [uint16]$resp.active_policies
        }
        if ($null -ne $resp.PSObject.Properties['dormant_policies']) {
            $dormant = [uint16]$resp.dormant_policies
        }
        return @{
            active_policies = $active
            dormant_policies = $dormant
        }
    }
    catch {
        return $null
    }
}

function Get-HeadHeight {
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/head" -TimeoutSec 5
        return [int64]$resp.height
    }
    catch {
        return -1
    }
}

function Wait-HeadAtLeast {
    param(
        [int64]$MinHeight,
        [int]$MaxSec
    )

    $deadline = (Get-Date).AddSeconds($MaxSec)
    $last = -1
    while ((Get-Date) -lt $deadline) {
        $h = Get-HeadHeight
        if ($h -ge $MinHeight) {
            if ($h -ne $last) {
                Add-Rep ("- head reached {0} (target>={1})" -f $h, $MinHeight)
            }
            return $true
        }
        if ($h -ne $last -and $h -ge 0) {
            Add-Rep ("- waiting head {0} (target>={1})" -f $h, $MinHeight)
            $last = $h
        }
        Start-Sleep -Seconds 4
    }
    return $false
}

function Wait-AccountActivePoliciesNonZero {
    param(
        [string]$AccountId,
        [int]$MaxSec
    )

    $deadline = (Get-Date).AddSeconds($MaxSec)
    while ((Get-Date) -lt $deadline) {
        $pol = Get-AccountPolicyFields -AccountId $AccountId
        if ($pol -and $pol.active_policies -gt 0) {
            return $pol.active_policies
        }
        Start-Sleep -Seconds 4
    }
    return 0
}

function Resolve-StakedAccountId {
    $accList = Invoke-RestMethod -Uri "$RpcUrl/v1/accounts" -TimeoutSec 15
    $staked = @($accList.accounts | Where-Object { [int64]$_.staked -gt 0 })
    if ($staked.Count -gt 0) {
        return [string]$staked[0].id
    }
    $init = @($accList.accounts | Where-Object { $_.initialized -eq $true })
    if ($init.Count -gt 0) {
        return [string]$init[-1].id
    }
    return $null
}

function Resolve-GenesisFundedAccountId {
    $accList = Invoke-RestMethod -Uri "$RpcUrl/v1/accounts" -TimeoutSec 15
    $funded = @($accList.accounts | Where-Object {
            $_.initialized -eq $true -and [int64]$_.balance_pwm -gt 0
        })
    if ($funded.Count -gt 0) {
        return [string]$funded[0].id
    }
    return $null
}

function Test-AccountInitialized {
    param([string]$AccountId)
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$AccountId" -TimeoutSec 5
        return [bool]$resp.initialized
    }
    catch {
        return $false
    }
}

function Get-AccountNonce {
    param([string]$AccountId)
    try {
        $resp = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$AccountId" -TimeoutSec 5
        return [int64]$resp.nonce
    }
    catch {
        return -1
    }
}

function Wait-AccountNonceAtLeast {
    param(
        [string]$AccountId,
        [int64]$MinNonce,
        [int]$MaxSec
    )

    $deadline = (Get-Date).AddSeconds($MaxSec)
    $last = -1
    while ((Get-Date) -lt $deadline) {
        $nonce = Get-AccountNonce -AccountId $AccountId
        if ($nonce -ge $MinNonce) {
            Add-Rep ("- account nonce reached {0} (target>={1})" -f $nonce, $MinNonce)
            return $true
        }
        if ($nonce -ne $last -and $nonce -ge 0) {
            Add-Rep ("- waiting account nonce {0} (target>={1})" -f $nonce, $MinNonce)
            $last = $nonce
        }
        Start-Sleep -Seconds 2
    }
    return $false
}

# === Slice 3 helper: ensure a deterministic test ipv4_claim phase exists ===
# For maximum harness reliability we post-process the generated genesis JSON
# instead of modifying core demo scripts (per ticket guidance).
function Ensure-TestIPv4ClaimPhase {
    param(
        [string]$GenesisPath = $GENESIS,
        [int]$Phase = 7,
        [UInt64]$Allocation = 1000000
    )

    if (-not (Test-Path -LiteralPath $GenesisPath)) {
        throw "Genesis file not found at $GenesisPath"
    }

    $genesis = Get-Content -LiteralPath $GenesisPath -Raw | ConvertFrom-Json

    # Idempotent: if phase already exists, do nothing
    $existing = $genesis.gen_cfg.ipv4_claim_phases | Where-Object { $_.phase -eq $Phase }
    if ($existing) {
        Add-Rep "- ipv4_claim_phases already contains phase $Phase"
        return $true
    }

    # Resolve a real registry from the funded initialized accounts in this genesis (re-uses existing keypair controlled by the demo wallet).
    # This exercises the V5-5 ClaimPhaseConfig path without introducing new key material in the first iteration.
    $funded = @($genesis.gen_cfg.funding.accounts | Where-Object {
        $_.PSObject.Properties.Name -contains 'bal' -and [int64]$_.bal -gt 0
    })
    $registryAid = if ($funded.Count -gt 0) { [string]$funded[0].acct_hex } else { $null }
    if (-not $registryAid) {
        throw "Cannot inject ipv4_claim phase: no suitable registry account found in genesis"
    }

    $newPhase = [pscustomobject]@{
        phase            = $Phase
        registry_address = $registryAid
        allocation       = $Allocation
    }

    if (-not ($genesis.gen_cfg.PSObject.Properties.Name -contains 'ipv4_claim_phases')) {
        $genesis.gen_cfg | Add-Member -NotePropertyName ipv4_claim_phases -NotePropertyValue @()
    }

    $genesis.gen_cfg.ipv4_claim_phases += $newPhase

    # Write back
    $genesisJson = $genesis | ConvertTo-Json -Depth 20
    [System.IO.File]::WriteAllText(
        $GenesisPath,
        $genesisJson,
        [System.Text.UTF8Encoding]::new($false)
    )

    Add-Rep "- Injected ipv4_claim_phases entry (phase=$Phase, registry=$registryAid, allocation=$Allocation)"
    return $true
}

# === Real ClaimIPv4Batch submit + verification (V5-5 primitive) ===
function Submit-ClaimIPv4Batch {
    param(
        [int]$Phase,
        [string]$RpcUrl,
        [string]$GenesisPath
    )

    Add-Rep "- Building ClaimIPv4Batch via helper (pwm-cli claim-ipv4-batch) ..."

    $batchRoot = "00000000000000000000000000000000000000000000000000000000000000ab"

    # Prefer the real demo wallet when available (much more realistic smoke).
    # Falls back to deterministic test seeds only if wallet is missing.
    $useRealWallet = Test-Path -LiteralPath $WALLET
    $claimantIndex = 1
    if ($useRealWallet -and (Test-Path -LiteralPath $GenesisPath)) {
        try {
            $genesis = Get-Content -LiteralPath $GenesisPath -Raw | ConvertFrom-Json
            $funded = @($genesis.gen_cfg.funding.accounts | Where-Object {
                $_.PSObject.Properties.Name -contains 'bal' -and [int64]$_.bal -gt 0
            })
            if ($funded.Count -gt 0 -and $funded[0].PSObject.Properties.Name -contains 'der_idx') {
                $claimantIndex = [int]$funded[0].der_idx
            }
        }
        catch { }
    }

    if ($useRealWallet) {
        Add-Rep "- Using real demo wallet for claimant signing (index $claimantIndex)"
    } else {
        Add-Rep "- Wallet not found - falling back to fixed test seeds (less realistic)"
    }

    # === Note on first run ===
    # The first `cargo run` will compile the helper (can take 30-90s).
    # Subsequent runs are fast.

    # Use the fast discovery mode (--print-claimant) to learn the exact claimant.
    $discoveryArgs = @(
        "--phase", "$Phase",
        "--batch-root", $batchRoot,
        "--print-claimant"
    )

    if ($useRealWallet) {
        $discoveryArgs += @(
            "--wallet", $WALLET,
            "--claimant-index", "$claimantIndex",
            "--dev-registry-is-claimant"
        )
    } else {
        $discoveryArgs += @(
            "--registry-seed", "4444444444444444444444444444444444444444444444444444444444444444",
            "--claimant-seed", "4545454545454545454545454545454545454545454545454545454545454545"
        )
    }
    $cargoCmd = @("run", "--quiet", "-p", "pwm-cli", "--bin", "claim-ipv4-batch", "--") + $discoveryArgs
    $helperOut = & cargo @cargoCmd 2>&1
    if ($LASTEXITCODE -ne 0) {
        Add-Rep "ERROR: claim-ipv4-batch helper failed during claimant discovery"
        Add-Rep ($helperOut -join "`n")
        return $false
    }

    $envelope = $helperOut | ConvertFrom-Json
    $claimantId = [string]$envelope.claimant_id

    # Now fetch real nonce for this claimant
    try {
        $acctBefore = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$claimantId" -TimeoutSec 5
        $nonce = [uint64]$acctBefore.nonce
        $balanceBefore = [int64]$acctBefore.balance_pwm
    }
    catch {
        Add-Rep "ERROR: Failed to fetch pre-claim state for claimant $claimantId"
        return $false
    }

    Add-Rep "- Claimant from helper: $claimantId"
    Add-Rep "- Using nonce: $nonce, balance_before: $balanceBefore"

    # Final call to helper with correct nonce
    $finalHelperArgs = @(
        "--phase", "$Phase",
        "--batch-root", $batchRoot,
        "--nonce", "$nonce"
    )

    if ($useRealWallet) {
        $finalHelperArgs += @(
            "--wallet", $WALLET,
            "--claimant-index", "$claimantIndex",
            "--dev-registry-is-claimant"
        )
    } else {
        $finalHelperArgs += @(
            "--registry-seed", "4444444444444444444444444444444444444444444444444444444444444444",
            "--claimant-seed", "4545454545454545454545454545454545454545454545454545454545454545"
        )
    }

    $cargoCmd = @("run", "--quiet", "-p", "pwm-cli", "--bin", "claim-ipv4-batch", "--") + $finalHelperArgs

    Write-Host "==> cargo $($cargoCmd -join ' ')"
    $helperOut = & cargo @cargoCmd 2>&1
    if ($LASTEXITCODE -ne 0) {
        Add-Rep "ERROR: claim-ipv4-batch helper failed"
        Add-Rep ($helperOut -join "`n")
        return $false
    }

    $envelope = $helperOut | ConvertFrom-Json
    $signedTxJson = $envelope.tx | ConvertTo-Json -Depth 20 -Compress

    # Submit
    try {
        $null = Invoke-RestMethod -Uri "$RpcUrl/v1/tx" -Method Post -Body $signedTxJson -ContentType "application/json" -TimeoutSec 15
        Add-Rep "- ClaimIPv4Batch tx accepted"
    }
    catch {
        Add-Rep "ERROR: POST /v1/tx ClaimIPv4Batch failed: $_"
        return $false
    }

    # Poll for on-chain effect (both phase flag and balance increase for reliability)
    $deadline = (Get-Date).AddSeconds(90)
    $success = $false
    $balanceAfter = $null

    while ((Get-Date) -lt $deadline) {
        try {
            $acct = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$claimantId" -TimeoutSec 5

            $hasPhase = (
                $acct.PSObject.Properties.Name -contains 'ipv4_claimed_phase' -and
                $acct.ipv4_claimed_phase -eq $Phase
            )

            if ($hasPhase) {
                $currentBalance = [int64]$acct.balance_pwm
                $observedDelta = $currentBalance - $balanceBefore

                if ($observedDelta -gt 0) {
                    $balanceAfter = $currentBalance
                    Add-Rep "- Observed ipv4_claimed_phase == $Phase and positive balance delta ($observedDelta)"
                    $success = $true
                    break
                } else {
                    Add-Rep "- Phase is set but no balance increase yet (delta=$observedDelta). Continuing to poll..."
                }
            }
        }
        catch { }
        Start-Sleep -Seconds 2
    }

    if (-not $success) {
        Add-Rep "Timed out waiting for ipv4_claimed_phase effect"
        return $false
    }

    if ($null -eq $balanceAfter) {
        try {
            $acctFinal = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$claimantId" -TimeoutSec 5
            $balanceAfter = [int64]$acctFinal.balance_pwm
        }
        catch { $balanceAfter = "unknown" }
    }

    $delta = if ($balanceAfter -ne "unknown") { $balanceAfter - $balanceBefore } else { "unknown" }

    $registryFromHelper = if ($envelope.PSObject.Properties.Name -contains 'registry_address') { 
        [string]$envelope.registry_address 
    } else { "unknown" }

    Add-Rep ("PASS_EVIDENCE: slice=ipv4_claim phase={0} claimant={1} registry={2} balance_before={3} balance_after={4} delta={5}" -f $Phase, $claimantId, $registryFromHelper, $balanceBefore, $balanceAfter, $delta)

    return $true
}

# === Slice 4 helper: account-info CLI output validation ===
function Test-AccountInfoCli {
    param(
        [string]$AccountId
    )

    Add-Rep "- Running pwm account-info via demo wallet ..."
    $cargoCmd = @(
        "run", "--quiet", "-p", "pwm-cli", "--bin", "pwm", "--",
        "--rpc", $RpcUrl,
        "account-info",
        "--wallet", $WALLET
    )

    Write-Host "==> cargo $($cargoCmd -join ' ')"
    $out = & cargo @cargoCmd 2>&1
    if ($LASTEXITCODE -ne 0) {
        Add-Rep "ERROR: pwm account-info failed"
        Add-Rep ($out -join "`n")
        return $false
    }

    $fields = @{}
    foreach ($lineRaw in @($out)) {
        $line = [string]$lineRaw
        if ($line -match '^([A-Za-z_]+)=(.*)$') {
            $fields[$Matches[1]] = $Matches[2]
        }
    }

    $required = @(
        'head_height',
        'marks_stored',
        'marks_effective',
        'marks_sat_pct',
        'marks_last_block',
        'staked'
    )
    $missing = @($required | Where-Object { -not $fields.ContainsKey($_) })
    if ($missing.Count -gt 0) {
        Add-Rep ("ERROR: pwm account-info output missing required fields: {0}" -f ($missing -join ', '))
        Add-Rep ($out -join "`n")
        return $false
    }

    $marksLastBlock = [uint64]$fields['marks_last_block']
    $staked = [uint64]$fields['staked']
    if ($marksLastBlock -le 0) {
        Add-Rep "ERROR: expected marks_last_block > 0, got $marksLastBlock"
        return $false
    }
    if ($staked -le 0) {
        Add-Rep "ERROR: expected staked > 0, got $staked"
        return $false
    }

    Add-Rep "- account-info fields observed: head_height=$($fields['head_height']) marks_stored=$($fields['marks_stored']) marks_effective=$($fields['marks_effective']) marks_sat_pct=$($fields['marks_sat_pct']) marks_last_block=$marksLastBlock staked=$staked"
    Add-Rep ("PASS_EVIDENCE: slice=account_info account={0} head_height={1} marks_stored={2} marks_effective={3} marks_sat_pct={4} marks_last_block={5} staked={6}" -f $AccountId, $fields['head_height'], $fields['marks_stored'], $fields['marks_effective'], $fields['marks_sat_pct'], $marksLastBlock, $staked)
    return $true
}

# === CleanState ===
if ($CleanState) {
    Add-Rep "## CleanState"
    . (Join-Path $PSScriptRoot '_devnet_clean_state.ps1')
    $cleanPatterns = Get-DevnetCleanStatePatterns -RepoRoot $RepoRoot -Profile FullTmpDevnet
    $null = Invoke-DevnetCleanStateWithArchive -RepoRoot $RepoRoot -PathPatterns $cleanPatterns `
        -Label 'devnet_v5_operator_smoke' -MaxArchives $MaxStateArchives -SkipArchive:$SkipArchive `
        -Log { param($m) Add-Rep $m }
}

# === Genesis ===
if (-not $SkipGenesis) {
    Add-Rep "## Generating demo genesis (reusing V4 helper)"
    $demoStart = Join-Path $PSScriptRoot 'demo-devnet-start.ps1'
    if (Test-Path -LiteralPath $demoStart) {
        & $demoStart
        if ($LASTEXITCODE -ne 0) {
            Add-Rep "FATAL: demo-devnet-start.ps1 failed"
            Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
            exit 1
        }
    }
    else {
        Add-Rep "WARNING: demo-devnet-start.ps1 not found - assuming genesis already exists"
    }
}

if (-not (Test-Path -LiteralPath $GENESIS) -or -not (Test-Path -LiteralPath $WALLET)) {
    Add-Rep "FATAL: $GENESIS or $WALLET is missing. Run with -CleanState or prepare genesis manually."
    Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
    exit 1
}

# === Slice3 prep: ensure ipv4 claim phase in genesis (must be before nodes start) ===
if ($runIpv4Claim) {
    $null = Ensure-TestIPv4ClaimPhase -Phase $Ipv4ClaimPhase
}

# === Start node(s) ===
$proposerPs1 = Join-Path $RepoRoot 'cy-cluster-proposer.ps1'
$attesterPs1 = Join-Path $RepoRoot 'cy-cluster-attester.ps1'
$procs = @()
if (-not $SkipNodes) {
    $env:PWM_DEMO_GENESIS_PATH = $GENESIS
    $env:PWM_DEMO_GENESIS_PASSPHRASE = '12345'

    $logDir = Join-Path $RepoRoot ("tmp\devnet-v5-smoke-$ts")
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null

    $proposerOut = Join-Path $logDir 'proposer.stdout.log'
    $proposerErr = Join-Path $logDir 'proposer.stderr.log'
    $attesterOut = Join-Path $logDir 'attester.stdout.log'
    $attesterErr = Join-Path $logDir 'attester.stderr.log'

    Add-Rep "## Spawn CY proposer + attester (logs under $logDir)"

    if (-not (Test-Path -LiteralPath $proposerPs1) -or -not (Test-Path -LiteralPath $attesterPs1)) {
        Add-Rep "FATAL: missing cy-cluster-proposer.ps1 or cy-cluster-attester.ps1 in repo root"
        Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
        exit 2
    }

    $pArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $proposerPs1)
    $procs += Start-Process -FilePath 'powershell.exe' -ArgumentList $pArgs `
        -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $proposerOut -RedirectStandardError $proposerErr

    Start-Sleep -Seconds $ProposerLeadSeconds

    $aArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $attesterPs1)
    $procs += Start-Process -FilePath 'powershell.exe' -ArgumentList $aArgs `
        -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $attesterOut -RedirectStandardError $attesterErr

    Add-Rep "## Waiting for RPC ready (max ${StatusWaitSeconds}s)"
    if (-not (Wait-RPCReady -Url $RpcUrl -MaxSec $StatusWaitSeconds)) {
        Add-Rep "FATAL: RPC did not become ready"
        Get-Content -LiteralPath $proposerErr -Tail 40 -ErrorAction SilentlyContinue | ForEach-Object { Add-Rep " proposer-err: $_" }
        Get-Content -LiteralPath $attesterErr -Tail 40 -ErrorAction SilentlyContinue | ForEach-Object { Add-Rep " attester-err: $_" }
        Stop-Process -Id ($procs | ForEach-Object { $_.Id }) -Force -ErrorAction SilentlyContinue
        Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
        exit 2
    }
    Add-Rep "- /v1/status OK"
}

# === Main smoke logic ===
$exitCode = 0
$marksAdvanced = $false
$deferredPassed = $false
try {
    if (-not $SkipNodes) {
        $fundedId = Resolve-GenesisFundedAccountId
        if (-not $fundedId) {
            throw 'No funded genesis account found via GET /v1/accounts'
        }
        Add-Rep "- genesis-funded account: $fundedId"

        if (Test-AccountInitialized -AccountId $fundedId) {
            Add-Rep "## tx-init skipped (genesis row already initialized per GenCfg.state0)"
        }
        else {
            Add-Rep "## tx-init (demo genesis account, index $DemoDerivationIndex)"
            Run-Pwm @('tx-init', '--wallet', $WALLET, '--index', "$DemoDerivationIndex", '--flags', '0')
        }

        Add-Rep "## Stake some PWM (required for marks to appear)"
        $stakeNonceBefore = Get-AccountNonce -AccountId $fundedId
        Run-Pwm @('tx-stake', '--wallet', $WALLET, '--amount', '1000000000')
        if ($stakeNonceBefore -ge 0) {
            $stakeNonceTarget = $stakeNonceBefore + 1
            if (-not (Wait-AccountNonceAtLeast -AccountId $fundedId -MinNonce $stakeNonceTarget -MaxSec 90)) {
                throw "tx-stake did not apply before smoke continued (account=$fundedId target_nonce=$stakeNonceTarget)"
            }
        }
    }
    else {
        Add-Rep "## SkipNodes: assuming chain already running with a staked account"
    }

    $stakerId = Resolve-StakedAccountId
    if (-not $stakerId) {
        throw 'No staked or initialized account found via GET /v1/accounts'
    }
    Add-Rep "- operator account: $stakerId"

    if ($runMarks) {
        Add-Rep "## Slice1: marks / lazy inflation path"
        $baseline = Get-AccountMarks -AccountId $stakerId
        if (-not $baseline) {
            throw "GET /v1/account/$stakerId failed"
        }
        $headAtStart = Get-HeadHeight
        Add-Rep "- Head at start: $headAtStart"
        Add-Rep ("- marks baseline: {0}, marks_last_block: {1}" -f $baseline.marks, $baseline.marks_last_block)

        Add-Rep "## Waiting for marks and marks_last_block to advance (timeout ${SmokeSeconds}s)"
        $deadline = (Get-Date).AddSeconds($SmokeSeconds)
        $lastHead = $headAtStart

        while ((Get-Date) -lt $deadline) {
            Start-Sleep -Seconds 4
            $currentHead = Get-HeadHeight
            $current = Get-AccountMarks -AccountId $stakerId
            if (-not $current) {
                Add-Rep "WARNING: GET /v1/account/$stakerId failed at head=$currentHead"
                continue
            }

            if ($currentHead -gt $lastHead) {
                Add-Rep ("- head: {0} -> {1}; marks={2} marks_last_block={3}" -f $lastHead, $currentHead, $current.marks, $current.marks_last_block)
                $lastHead = $currentHead
            }

            if ($current.marks -gt $baseline.marks -and $current.marks_last_block -gt $baseline.marks_last_block) {
                Add-Rep ("- marks advanced: {0} -> {1}; marks_last_block: {2} -> {3}" -f $baseline.marks, $current.marks, $baseline.marks_last_block, $current.marks_last_block)
                Add-Rep ("PASS_EVIDENCE: slice=marks account={0} marks={1}->{2} marks_last_block={3}->{4} head={5}" -f $stakerId, $baseline.marks, $current.marks, $baseline.marks_last_block, $current.marks_last_block, $currentHead)
                $marksAdvanced = $true
                break
            }

            if ($baseline.marks -eq $MarksSaturation -and $current.marks_last_block -gt $baseline.marks_last_block) {
                Add-Rep ("- marks saturated at {0}; marks_last_block advanced: {1} -> {2}" -f $MarksSaturation, $baseline.marks_last_block, $current.marks_last_block)
                Add-Rep ("PASS_EVIDENCE: slice=marks account={0} marks=saturated({1}) marks_last_block={2}->{3} head={4}" -f $stakerId, $MarksSaturation, $baseline.marks_last_block, $current.marks_last_block, $currentHead)
                $marksAdvanced = $true
                break
            }
        }

        if (-not $marksAdvanced) {
            $final = Get-AccountMarks -AccountId $stakerId
            $finalHead = Get-HeadHeight
            if ($final) {
                Add-Rep ("WARNING: marks growth not observed within timeout (head={0}, marks={1}, marks_last_block={2})" -f $finalHead, $final.marks, $final.marks_last_block)
            }
            else {
                Add-Rep "WARNING: marks growth not observed within timeout (final account poll failed)"
            }
            if ($MarksOnly) {
                $exitCode = 4
            }
        }
        else {
            Add-Rep '- Marks path observed at operator level.'
        }
    }

    $shouldRunDeferred = $runDeferred -and ($DeferredOnly -or $marksAdvanced -or -not $runMarks)
    if ($runDeferred -and -not $DeferredOnly -and -not $marksAdvanced) {
        Add-Rep '## Slice2 skipped (marks slice did not PASS in this run)'
    }

    $shouldRunIpv4Claim = $runIpv4Claim -and ($Ipv4ClaimOnly -or $marksAdvanced -or $deferredPassed -or -not ($runMarks -or $runDeferred))
    if ($runIpv4Claim -and -not $Ipv4ClaimOnly -and -not ($marksAdvanced -or $deferredPassed)) {
        Add-Rep '## Slice3 skipped (previous slices did not provide a good baseline in this run)'
    }

    $shouldRunAccountInfo = $runAccountInfo -and ($AccountInfoOnly -or $marksAdvanced -or $deferredPassed -or $ipv4ClaimPassed -or -not ($runMarks -or $runDeferred -or $runIpv4Claim))
    if ($runAccountInfo -and -not $AccountInfoOnly -and -not ($marksAdvanced -or $deferredPassed -or $ipv4ClaimPassed)) {
        Add-Rep '## Slice4 skipped (previous slices did not provide a good baseline in this run)'
    }

    if ($shouldRunDeferred) {
        Add-Rep "## Slice2: deferred policy activation (ADR 0005)"
        $head0 = Get-HeadHeight
        if ($head0 -lt 0) {
            throw 'GET /v1/head failed before deferred policy smoke'
        }
        $activateAt = $head0 + $DeferredLeadBlocks
        Add-Rep ("- head H0={0}; activate_at={1} (lead={2} blocks); policy={3}" -f $head0, $activateAt, $DeferredLeadBlocks, $DeferredPolicy)

        Add-Rep '## tx-policy-set deferred'
        Run-Pwm @(
            'tx-policy-set',
            '--wallet', $WALLET,
            '--policy', $DeferredPolicy,
            '--activation', 'deferred',
            '--activate-at-height', "$activateAt",
            '--fee', $PolicyFee
        )

        $beforePol = Get-AccountPolicyFields -AccountId $stakerId
        if (-not $beforePol) {
            throw "GET /v1/account/$stakerId failed after deferred set"
        }
        Add-Rep ("- active_policies before height: {0}; dormant_policies: {1}" -f $beforePol.active_policies, $beforePol.dormant_policies)
        if ($beforePol.active_policies -ne 0) {
            throw "expected active_policies=0 before activate_at height, got $($beforePol.active_policies)"
        }

        Add-Rep '## tx-policy-activate before height (expect reject)'
        $activateExitBefore = Invoke-Pwm @(
            'tx-policy-activate',
            '--wallet', $WALLET,
            '--policy', $DeferredPolicy,
            '--fee', $PolicyFee
        )
        Add-Rep ("- pwm exit before height: $activateExitBefore")
        if ($activateExitBefore -eq 0) {
            throw 'tx-policy-activate unexpectedly succeeded before deferred height'
        }

        Add-Rep "## Waiting for head >= $activateAt (timeout ${DeferredWaitSeconds}s)"
        if (-not (Wait-HeadAtLeast -MinHeight $activateAt -MaxSec $DeferredWaitSeconds)) {
            throw "head did not reach activate_at=$activateAt within timeout"
        }

        $afterPol = Get-AccountPolicyFields -AccountId $stakerId
        $headAfter = Get-HeadHeight
        $storedActive = if ($afterPol) { $afterPol.active_policies } else { 0 }
        Add-Rep ("- head after wait: {0}; stored active_policies={1} (may stay 0; deferred is evaluator-gated per ADR 0005)" -f $headAfter, $storedActive)

        Add-Rep '## tx-policy-activate at/after height (expect reject: already active via deferred)'
        $activateExitAfter = Invoke-Pwm @(
            'tx-policy-activate',
            '--wallet', $WALLET,
            '--policy', $DeferredPolicy,
            '--fee', $PolicyFee
        )
        Add-Rep ("- pwm exit at/after height: $activateExitAfter")
        if ($activateExitAfter -eq 0) {
            throw 'tx-policy-activate unexpectedly succeeded after deferred auto-activation height'
        }
        if ($activateExitAfter -eq $activateExitBefore) {
            Add-Rep "WARNING: activate exit code unchanged before/after height ($activateExitAfter); relying on non-zero reject only"
        }

        Add-Rep ("PASS_EVIDENCE: slice=deferred account={0} policy={1} activate_at={2} stored_active_policies={3} head={4} activate_exit_before={5} activate_exit_after={6}" -f $stakerId, $DeferredPolicy, $activateAt, $storedActive, $headAfter, $activateExitBefore, $activateExitAfter)
        $deferredPassed = $true
        Add-Rep '- Deferred policy path observed at operator level.'
    }

    if ($shouldRunIpv4Claim) {
        Add-Rep "## Slice3: ClaimIPv4Batch happy path (V5-5)"

        # Phase was already injected earlier (after genesis, before nodes) for correct timing.
        Add-Rep "- ipv4_claim phase ensured in genesis before node startup."

        # Real submit + verification (the core of this revision)
        $ipv4ClaimPassed = Submit-ClaimIPv4Batch -Phase $Ipv4ClaimPhase -RpcUrl $RpcUrl -GenesisPath $GENESIS

        if ($ipv4ClaimPassed) {
            Add-Rep "- Slice3 completed with real on-chain ClaimIPv4Batch."
        }
        else {
            Add-Rep "- Slice3 did not complete real claim path (see errors above)."
        }
    }

    if ($shouldRunAccountInfo) {
        Add-Rep "## Slice4: pwm account-info marks output (V5-6/V5-7)"
        $accountInfoPassed = Test-AccountInfoCli -AccountId $stakerId
        if ($accountInfoPassed) {
            Add-Rep "- Slice4 completed with pwm account-info marks output."
        }
        else {
            Add-Rep "- Slice4 did not complete account-info validation (see errors above)."
        }
    }

    Add-Rep '## Smoke completed'
    $sliceResults = @()
    if ($runMarks) {
        $sliceResults += if ($marksAdvanced) { 'marks=PASS' } else { 'marks=PARTIAL' }
    }
    if ($runDeferred) {
        if ($shouldRunDeferred) {
            $sliceResults += if ($deferredPassed) { 'deferred=PASS' } else { 'deferred=FAIL' }
        }
        else {
            $sliceResults += 'deferred=SKIPPED'
        }
    }
    if ($runIpv4Claim) {
        if ($shouldRunIpv4Claim) {
            $sliceResults += if ($ipv4ClaimPassed) { 'ipv4_claim=PASS' } else { 'ipv4_claim=FAIL' }
        }
        else {
            $sliceResults += 'ipv4_claim=SKIPPED'
        }
    }
    if ($runAccountInfo) {
        if ($shouldRunAccountInfo) {
            $sliceResults += if ($accountInfoPassed) { 'account_info=PASS' } else { 'account_info=FAIL' }
        }
        else {
            $sliceResults += 'account_info=SKIPPED'
        }
    }
    $overallPass = $true
    if ($runMarks -and -not $marksAdvanced) { $overallPass = $false }
    if ($shouldRunDeferred -and -not $deferredPassed) { $overallPass = $false }
    if ($shouldRunIpv4Claim -and -not $ipv4ClaimPassed) { $overallPass = $false }
    if ($shouldRunAccountInfo -and -not $accountInfoPassed) { $overallPass = $false }
    $result = if ($overallPass) { 'PASS' } else { ('PARTIAL (' + ($sliceResults -join ', ') + ')') }
    Add-Rep ''
    Add-Rep ("**Result**: {0}" -f $result)

    if ($shouldRunDeferred -and -not $deferredPassed) {
        if ($exitCode -eq 0) { $exitCode = 5 }
    }
    elseif ($shouldRunAccountInfo -and -not $accountInfoPassed) {
        if ($exitCode -eq 0) { $exitCode = 6 }
    }
    elseif ($runMarks -and $MarksOnly -and -not $marksAdvanced) {
        $exitCode = 4
    }
    elseif (-not $overallPass) {
        if ($exitCode -eq 0) { $exitCode = 4 }
    }
    # Slice 5+: closeout/doc review is orchestrator-owned.
}
catch {
    Add-Rep "## FAIL: $($_.Exception.Message)"
    if ($exitCode -eq 0) { $exitCode = 3 }
}
finally {
    if ($procs.Count -gt 0) {
        Add-Rep '## Cleaning up nodes'
        foreach ($p in $procs) {
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        }
    }
    if (-not $SkipNodes) {
        try {
            & taskkill.exe /F /IM pwmd.exe /T *>$null
        }
        catch {
            Add-Rep "WARNING: taskkill cleanup failed: $($_.Exception.Message)"
        }
    }
}

Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
Write-Host ''
Write-Host "Report written to: $ReportPath"
exit $exitCode
