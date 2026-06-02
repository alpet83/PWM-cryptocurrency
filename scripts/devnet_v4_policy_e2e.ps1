# MVP V4 policy - live devnet smoke (PowerShell). UTF-8; avoid em-dash in strings (PS 5.1 lexer).
# Purpose: complement unit-test-only V4 gate with operator-level checks: genesis → 2 CY nodes →
# tx-init → policy set/activate (reversible) → optional deactivate → GET /v1/account policy fields.
#
# Limitations (documented also in docs/reviews):
# - Does NOT prove cosign_required witness path (pwm-cli only auto-cosigns emergency rescue).
# - Does NOT prove default_behavior / sender_filter / cross-domain without a 2nd funded account.
# - Emergency routing + rescue cosign needs a second initialized account; extend script or run manually.
#
# Prerequisites: Rust cargo on PATH; repo root = parent of scripts/.
#
# Example (from repo root):
#   ./scripts/devnet_v4_policy_e2e.ps1 -CleanState
#
# Optional offline bruteforce (pwm-testing): use -BruteDemoOnly -BruteMaxTry 1000000 (default)
# Full live policy smoke (background pwmd trees): use MCP cq_process_ctl (host=true) spawn + wait per docs/AGENT_PROMPT_testing.md.
#
param(
    [string]$RepoRoot = '',
    [string]$RpcUrl = 'http://127.0.0.1:3030',
    [int]$SmokeSeconds = 55,
    [int]$ProposerLeadSeconds = 12,
    [int]$StatusWaitSeconds = 120,
    [switch]$CleanState,
    [switch]$SkipArchive,
    [int]$MaxStateArchives = 30,
    [switch]$SkipGenesis,
    [switch]$SkipNodes,
    [switch]$BruteDemoOnly,
    # Upper bound on addr derivation trials (PWM phase1 profile defaults: flags mask 1023 ~= 10 low bits checked per try; brute still needs high attempt counts for CY domain lottery).
    [int]$BruteMaxTry = 1000000,
    [string]$ReportPath = ''
)
$ErrorActionPreference = 'Stop'
if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
Set-Location -LiteralPath $RepoRoot

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
if (-not $ReportPath) {
    $ReportPath = Join-Path $RepoRoot ("tmp\devnet_v4_policy_e2e_$ts.md")
}
$tmpDir = Join-Path $RepoRoot 'tmp'
if (-not (Test-Path -LiteralPath $tmpDir)) {
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
}

$report = New-Object System.Collections.Generic.List[string]
function Add-Rep([string]$s) { $script:report.Add($s); Write-Host $s }

Add-Rep "# Devnet V4 policy E2E ($ts)"
Add-Rep ""
Add-Rep "- Host: $env:COMPUTERNAME"
Add-Rep "- Repo: $RepoRoot"
Add-Rep "- RpcUrl: $RpcUrl"
Add-Rep ""

$DEMO_MASTER = '0000000000000000000000000000000000000000000000000000000000000001'
$WALLET = Join-Path $RepoRoot 'tmp\demo-genesis-wallet.yaml'
$GENESIS = Join-Path $RepoRoot 'tmp\genesis-custom.json'

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

if ($BruteDemoOnly) {
    Add-Rep "## addr-bruteforce (offline demo seed; max_try=$BruteMaxTry)"
    Add-Rep "Uses public demo master + domain CY; align with pwm-cli defaults --flags-mask 1023 --expected-flags 0."
    $bruteWallet = Join-Path $tmpDir 'e2e-brute-wallet.yaml'
    & cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master $DEMO_MASTER --domain CY --max-try $BruteMaxTry --flags-mask 1023 --expected-flags 0 --wallet-out $bruteWallet --overwrite-wallet
    $x = $LASTEXITCODE
    Add-Rep "- exit: $x"
    Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
    Write-Host "Report: $ReportPath"
    exit $x
}

if ($CleanState) {
    Add-Rep "## CleanState"
    . (Join-Path $PSScriptRoot '_devnet_clean_state.ps1')
    $cleanPatterns = Get-DevnetCleanStatePatterns -RepoRoot $RepoRoot -Profile CyCluster
    $null = Invoke-DevnetCleanStateWithArchive -RepoRoot $RepoRoot -PathPatterns $cleanPatterns `
        -Label 'devnet_v4_policy_e2e' -MaxArchives $MaxStateArchives -SkipArchive:$SkipArchive `
        -Log { param($m) Add-Rep $m }
}

if (-not $SkipGenesis) {
    Add-Rep "## demo genesis"
    $ds = Join-Path $PSScriptRoot 'demo-devnet-start.ps1'
    & $ds
    if ($LASTEXITCODE -ne 0) {
        Add-Rep "FATAL: demo-devnet-start failed"
        Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
        exit 1
    }
}

if (-not (Test-Path -LiteralPath $GENESIS) -or -not (Test-Path -LiteralPath $WALLET)) {
    Add-Rep "FATAL: missing $GENESIS or $WALLET - run demo-devnet-start first"
    Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
    exit 1
}

$proposerPs1 = Join-Path $RepoRoot 'cy-cluster-proposer.ps1'
$attesterPs1 = Join-Path $RepoRoot 'cy-cluster-attester.ps1'

$procs = @()
if (-not $SkipNodes) {
    $env:PWM_DEMO_GENESIS_PATH = $GENESIS
    $env:PWM_DEMO_GENESIS_PASSPHRASE = '12345'

    $logDir = Join-Path $RepoRoot ("tmp\cy-e2e-policy-$ts")
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    $proposerOut = Join-Path $logDir 'proposer.stdout.log'
    $proposerErr = Join-Path $logDir 'proposer.stderr.log'
    $attesterOut = Join-Path $logDir 'attester.stdout.log'
    $attesterErr = Join-Path $logDir 'attester.stderr.log'

    Add-Rep "## Spawn CY proposer + attester (logs under $logDir )"

    $pArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $proposerPs1)
    $procs += Start-Process -FilePath 'powershell.exe' -ArgumentList $pArgs `
        -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $proposerOut -RedirectStandardError $proposerErr

    Start-Sleep -Seconds $ProposerLeadSeconds

    $aArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $attesterPs1)
    $procs += Start-Process -FilePath 'powershell.exe' -ArgumentList $aArgs `
        -WorkingDirectory $RepoRoot -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $attesterOut -RedirectStandardError $attesterErr

    Add-Rep "## Wait RPC ready (max ${StatusWaitSeconds}s)"
    if (-not (Wait-RPCReady -Url $RpcUrl -MaxSec $StatusWaitSeconds)) {
        Add-Rep "FATAL: RPC not ready - see logs in $logDir"
        Get-Content -LiteralPath $proposerErr -Tail 30 -ErrorAction SilentlyContinue | ForEach-Object { Add-Rep " proposer-err: $_" }
        & taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
        Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
        exit 2
    }
    Add-Rep "- /v1/status OK"

    Start-Sleep -Seconds $SmokeSeconds
}

try {
    Add-Rep "## pwm tx-init (demo genesis index)"
    Run-Pwm @('tx-init', '--wallet', $WALLET, '--index', '287292', '--flags', '0')

    Add-Rep "## policy set dormant + activate (routing.same_domain_only, reversible)"
    Run-Pwm @('tx-policy-set', '--wallet', $WALLET, '--policy', 'routing.same_domain_only', '--activation', 'dormant', '--fee', '1000000')
    Run-Pwm @('tx-policy-activate', '--wallet', $WALLET, '--policy', 'routing.same_domain_only', '--fee', '1000000')

    Add-Rep "## GET /v1/accounts (inspect policy fields)"
    $accList = Invoke-RestMethod -Uri "$RpcUrl/v1/accounts" -TimeoutSec 15
    $ids = @($accList.accounts | ForEach-Object { $_.id })
    Add-Rep ("- account ids: " + ($ids -join ', '))

    foreach ($id in $ids) {
        $one = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$id" -TimeoutSec 15
        Add-Rep "### $id"
        Add-Rep ("- initialized: " + $one.initialized)
        if ($null -ne $one.PSObject.Properties['active_policies']) {
            Add-Rep ("- active_policies: " + $one.active_policies)
        }
        if ($null -ne $one.PSObject.Properties['dormant_policies']) {
            Add-Rep ("- dormant_policies: " + $one.dormant_policies)
        }
        if ($null -ne $one.PSObject.Properties['finalized']) {
            Add-Rep ("- finalized: " + $one.finalized)
        }
    }

    Add-Rep "## tx-policy-deactivate (same policy)"
    Run-Pwm @('tx-policy-deactivate', '--wallet', $WALLET, '--policy', 'routing.same_domain_only', '--fee', '1000000')

    Add-Rep "## Verdict: PASS (live policy lifecycle + account JSON inspection)"
}
catch {
    Add-Rep "## FAIL: $($_.Exception.Message)"
    Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
    & taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
    exit 3
}

if (-not $SkipNodes) {
    Add-Rep "## Stop pwmd"
    & taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
}

Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
Write-Host "Report written: $ReportPath"
exit 0
