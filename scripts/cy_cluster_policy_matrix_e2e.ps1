# Live E2E on CY cluster from repo root scripts: genesis premine + N CY brute wallets + policy checks.
# Use cy-cluster-proposer.ps1 + cy-cluster-attester.ps1 while pwmd listens on 127.0.0.1:3030.
# ASCII-only in double-quoted strings (PowerShell 5.1).
#
# pwm-cli tx-init supports V4 rescue: --rescue-address <pretty-or-bech32> plus full V4 meta
# (--owner-kind, --owner-name, --owner-country, --metadata-commitment, ...).
#
# Usage (from repo root):
#   ./scripts/cy_cluster_policy_matrix_e2e.ps1 -CleanState
#
param(
    [string]$RepoRoot = '',
    [string]$RpcUrl = 'http://127.0.0.1:3030',
    [int]$CyWalletCount = 3,
    [int]$BruteMaxTry = 1000000,
    [int]$SmokeSeconds = 55,
    [int]$ProposerLeadSeconds = 12,
    [int]$StatusWaitSeconds = 180,
    [switch]$CleanState,
    [switch]$SkipArchive,
    [int]$MaxStateArchives = 30,
    [switch]$SkipGenesis,
    [switch]$SkipCluster,
    [string]$RpcBruteDead = 'http://127.0.0.1:59999',
    [string]$ReportPath = ''
)
$ErrorActionPreference = 'Stop'
if (-not $RepoRoot) { $RepoRoot = Split-Path -Parent $PSScriptRoot }
Set-Location -LiteralPath $RepoRoot

function Child-MasterHex([int]$k) {
    $b = New-Object byte[] 32
    if ($k -lt 1 -or $k -gt 255) { throw "child master index must be 1..255" }
    $b[31] = [byte]$k
    -join ($b | ForEach-Object { $_.ToString('x2') })
}

function Wait-RPC([string]$Url, [int]$MaxSec) {
    $deadline = (Get-Date).AddSeconds($MaxSec)
    while ((Get-Date) -lt $deadline) {
        try {
            $null = Invoke-RestMethod -Uri "$Url/v1/status" -TimeoutSec 3
            return $true
        }
        catch { Start-Sleep -Seconds 2 }
    }
    return $false
}

function Wait-AccountBalanceAtLeast([string]$Url, [string]$AccountHex, [int64]$MinRaw, [int]$MaxSec) {
    $deadline = (Get-Date).AddSeconds($MaxSec)
    while ((Get-Date) -lt $deadline) {
        try {
            $acc = Invoke-RestMethod -Uri "$Url/v1/account/$AccountHex" -TimeoutSec 5
            $rawText = if ($null -ne $acc.balance_pwm_raw) {
                $acc.balance_pwm_raw
            } elseif ($null -ne $acc.spendable_on_this_shard) {
                $acc.spendable_on_this_shard
            } elseif ($null -ne $acc.balance_pwm) {
                $acc.balance_pwm
            } else {
                $acc.local_state_balance
            }
            $raw = [int64]$rawText
            if ($raw -ge $MinRaw) { return $true }
        }
        catch { }
        Start-Sleep -Seconds 2
    }
    return $false
}

function Wait-AccountActivePoliciesNonZero([string]$Url, [string]$AccountHex, [int]$MaxSec) {
    $deadline = (Get-Date).AddSeconds($MaxSec)
    while ((Get-Date) -lt $deadline) {
        try {
            $acc = Invoke-RestMethod -Uri "$Url/v1/account/$AccountHex" -TimeoutSec 5
            if ([int]$acc.active_policies -gt 0) { return $true }
        }
        catch { }
        Start-Sleep -Seconds 2
    }
    return $false
}

function Run-Pwm([string]$Rpc, [string[]]$Tail) {
    $a = @('run', '-p', 'pwm-cli', '--bin', 'pwm', '--', '--rpc', $Rpc) + $Tail
    Write-Host ('==> pwm ' + ($Tail -join ' '))
    & cargo @a
}

function Run-Pwm-Ok([string]$Rpc, [string[]]$Tail) {
    Run-Pwm $Rpc $Tail
    if ($LASTEXITCODE -ne 0) { throw "pwm failed exit=$LASTEXITCODE" }
}

function Parse-BruteLog([string]$Path) {
    $raw = Get-Content -LiteralPath $Path -Raw
    if (-not ($raw -match 'derivation_index (\d+)')) { throw "no derivation_index in $Path" }
    $d = [int]$Matches[1]
    $f = 0
    if ($raw -match 'flags_derived_u32 (\d+)') { $f = [uint32]$Matches[1] }
    @{ Der = $d; Flags = $f }
}

# Cargo writes progress to stderr; 2>&1 | Tee-Object under $ErrorActionPreference Stop becomes NativeCommandError
# and terminates the script. Pipe also breaks LASTEXITCODE for native cargo.
function Invoke-CargoRunLog([string[]]$CargoArgs, [string]$LogPath) {
    $prevEa = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    try {
        $lines = @( & cargo @CargoArgs 2>&1 )
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prevEa
    }
    $lines | Out-File -LiteralPath $LogPath -Encoding utf8
    if ($null -eq $code) { return -1 }
    return [int]$code
}

$ts = Get-Date -Format 'yyyyMMdd_HHmmss'
$tmp = Join-Path $RepoRoot 'tmp'
if (-not (Test-Path -LiteralPath $tmp)) { New-Item -ItemType Directory -Path $tmp -Force | Out-Null }
$cargoTargetWasPreset = [bool]$env:CARGO_TARGET_DIR
if (-not $cargoTargetWasPreset) {
    $env:CARGO_TARGET_DIR = Join-Path $tmp "cy-policy-target-$ts"
}
if (-not $ReportPath) { $ReportPath = Join-Path $tmp "cy_policy_matrix_e2e_$ts.md" }
$GENESIS = Join-Path $tmp 'genesis-custom.json'
$PREM = Join-Path $tmp 'demo-genesis-wallet.yaml'
$LOGD = Join-Path $tmp "cy-matrix-e2e-$ts"
New-Item -ItemType Directory -Path $LOGD -Force | Out-Null

$report = New-Object System.Collections.Generic.List[string]
function Add-R([string]$s) { $null = $report.Add($s); Write-Host $s }

Add-R "# CY cluster policy matrix E2E ($ts)"
Add-R ""
if ($cargoTargetWasPreset) {
    Add-R "cargo target dir: $env:CARGO_TARGET_DIR (preset)"
} else {
    Add-R "cargo target dir: $env:CARGO_TARGET_DIR (script default)"
}
Add-R ""

if (-not $SkipGenesis) {
    Add-R "## demo genesis"
    & (Join-Path $PSScriptRoot 'demo-devnet-start.ps1')
    if ($LASTEXITCODE -ne 0) { throw "demo-devnet-start failed" }
}
if (-not (Test-Path -LiteralPath $GENESIS) -or -not (Test-Path -LiteralPath $PREM)) {
    throw "missing genesis or demo wallet under tmp/"
}

if ($CleanState) {
    Add-R "## CleanState"
    . (Join-Path $PSScriptRoot '_devnet_clean_state.ps1')
    $cleanPatterns = Get-DevnetCleanStatePatterns -RepoRoot $RepoRoot -Profile CyCluster
    $null = Invoke-DevnetCleanStateWithArchive -RepoRoot $RepoRoot -PathPatterns $cleanPatterns `
        -Label 'cy_cluster_policy_matrix_e2e' -MaxArchives $MaxStateArchives -SkipArchive:$SkipArchive `
        -Log { param($m) Add-R $m }
}

$childWallets = @()
Add-R "## offline CY bruteforce (dead RPC skips auto-init)"
for ($k = 1; $k -le $CyWalletCount; $k++) {
    # Keep generated matrix wallets away from the deterministic demo genesis master (...0001).
    $masterK = $k + 10
    $cm = Child-MasterHex $masterK
    $wf = Join-Path $tmp ("cy-matrix-" + $k + ".yaml")
    if (Test-Path -LiteralPath $wf) { Remove-Item -LiteralPath $wf -Force }
    $bl = Join-Path $LOGD ("brute-" + $k + ".log")
    $arg = @('run', '-p', 'pwm-cli', '--bin', 'pwm', '--', '--rpc', $RpcBruteDead, 'addr-bruteforce',
        '--master', $cm, '--domain', 'CY', '--max-try', "$BruteMaxTry", '--flags-mask', '1023', '--expected-flags', '0',
        '--wallet-out', $wf, '--overwrite-wallet')
    Write-Host "## bruteforce k=$k"
    $cargoExit = Invoke-CargoRunLog -CargoArgs $arg -LogPath $bl
    if ($cargoExit -ne 0) { throw "addr-bruteforce failed kid=$k exit=$cargoExit" }
    $meta = Parse-BruteLog $bl
    $idHexPattern = '^\s*id_hex\s+([0-9a-fA-F]+)\s*$'
    $lastHit = @(Select-String -LiteralPath $bl -Pattern $idHexPattern) | Select-Object -Last 1
    if (-not $lastHit) { throw "no id_hex in brute log kid=$k" }
    $null = $lastHit.Line -match $idHexPattern
    $idhx = [string]$Matches[1]
    $childWallets += , [ordered]@{ K = $k; MasterK = $masterK; Path = $wf; Hex = ($idhx.ToLowerInvariant()); Der = $meta.Der; Flags = $meta.Flags }
}

if (-not $SkipCluster) {
    $env:PWM_DEMO_GENESIS_PATH = $GENESIS
    $env:PWM_DEMO_GENESIS_PASSPHRASE = '12345'
    $po = Join-Path $LOGD 'proposer.stdout.log'; $pe = Join-Path $LOGD 'proposer.stderr.log'
    $ao = Join-Path $LOGD 'attester.stdout.log'; $ae = Join-Path $LOGD 'attester.stderr.log'
    $peers = Join-Path $LOGD 'cy-lab-peers.yaml'
    Set-Content -LiteralPath $peers -Encoding utf8 -Value @(
        'shards:',
        '  "0x2C":',
        '    - id: cy-proposer',
        '      peer: 127.0.0.1:13030',
        '      validator: true',
        '    - id: cy-attester',
        '      peer: 127.0.0.1:13031',
        '      validator: true'
    )
    $clusterMembers = 'cy-quorum-proposer,cy-quorum-attester'
    $attArgs = @(
        'run', '-p', 'pwmd', '--bin', 'pwmd', '--',
        '--listen', '127.0.0.1:3031',
        '--state-root', (Join-Path $tmp 'state-cy-attester'),
        '--data-file', (Join-Path (Join-Path $tmp 'state-cy-attester') 'pwm-data.json'),
        '--genesis-file', $GENESIS,
        '--genesis-passphrase', '12345',
        '--network-id', 'testnet-qa',
        '--domain-hi', '0x2C',
        '--cluster-id', 'test-cluster-CY',
        '--node-id', 'cy-attester',
        '--node-instance-id', 'cy-quorum-attester',
        '--transport-real',
        '--transport-peer-listen', '127.0.0.1:13031',
        '--peers-list', $peers,
        '--cluster-enabled',
        '--cluster-role', 'attester',
        '--cluster-members', $clusterMembers,
        '--cluster-quorum-k', '1',
        '--cluster-quorum-n', '2',
        '--seal-lease-backend', 'process-local'
    )
    $propArgs = @(
        'run', '-p', 'pwmd', '--bin', 'pwmd', '--',
        '--listen', '127.0.0.1:3030',
        '--state-root', (Join-Path $tmp 'state-cy-proposer'),
        '--data-file', (Join-Path (Join-Path $tmp 'state-cy-proposer') 'pwm-data.json'),
        '--genesis-file', $GENESIS,
        '--genesis-passphrase', '12345',
        '--network-id', 'testnet-qa',
        '--domain-hi', '0x2C',
        '--cluster-id', 'test-cluster-CY',
        '--node-id', 'cy-proposer',
        '--node-instance-id', 'cy-quorum-proposer',
        '--transport-real',
        '--transport-peer-listen', '127.0.0.1:13030',
        '--peers-list', $peers,
        '--cluster-enabled',
        '--cluster-role', 'proposer',
        '--cluster-members', $clusterMembers,
        '--cluster-quorum-k', '1',
        '--cluster-quorum-n', '2',
        '--seal-lease-backend', 'process-local'
    )
    $null = Start-Process -FilePath 'cargo' -ArgumentList $attArgs `
        -WorkingDirectory $RepoRoot -WindowStyle Hidden -RedirectStandardOutput $ao -RedirectStandardError $ae
    Start-Sleep -Seconds $ProposerLeadSeconds
    $null = Start-Process -FilePath 'cargo' -ArgumentList $propArgs `
        -WorkingDirectory $RepoRoot -WindowStyle Hidden -RedirectStandardOutput $po -RedirectStandardError $pe
    Add-R "## wait RPC $RpcUrl"
    if (-not (Wait-RPC $RpcUrl $StatusWaitSeconds)) {
        Add-R 'FATAL: RPC not ready'
        & taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
        Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
        exit 2
    }
    Start-Sleep -Seconds $SmokeSeconds
}

$fee = '1000000'
$fund = '2000000000000'
$V4 = @(
    '--owner-kind', 'matrix', '--owner-name', 'E2E', '--owner-country', 'CY',
    '--metadata-commitment', '0000000000000000000000000000000000000000000000000000000000000000',
    '--verification-ref', 'matrix-e2e'
)

$premDer = 287292
try {
    Add-R '## init premine'
    $premInitPath = Join-Path $LOGD 'prem-init.log'
    $premInitArgs = @('run', '-p', 'pwm-cli', '--bin', 'pwm', '--', '--rpc', $RpcUrl,
        'tx-init', '--wallet', $PREM, '--index', "$premDer", '--flags', '0')
    $premInitExit = Invoke-CargoRunLog -CargoArgs $premInitArgs -LogPath $premInitPath
    if ($premInitExit -ne 0) {
        Add-R "premine tx-init exit=$premInitExit; continuing with genesis premine"
    }
    $premListPath = Join-Path $LOGD 'prem-accounts.txt'
    Write-Host ('==> pwm wallet account list')
    $premArgs = @('run', '-p', 'pwm-cli', '--bin', 'pwm', '--', '--rpc', $RpcUrl, 'wallet', 'account', 'list', '--wallet', $PREM)
    $listExit = Invoke-CargoRunLog -CargoArgs $premArgs -LogPath $premListPath
    if ($listExit -ne 0) { throw "wallet account list exit=$listExit" }
    $genesisJson = Get-Content -LiteralPath $GENESIS -Raw | ConvertFrom-Json
    $premGenesis = @($genesisJson.gen_cfg.funding.accounts) | Where-Object { [int]$_.der_idx -eq $premDer } | Select-Object -First 1
    if (-not $premGenesis) { throw "premine der_idx $premDer not found in genesis funding accounts" }
    $premHex = [string]$premGenesis.acct_hex
    $detP = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$premHex" -TimeoutSec 25
    $premPretty = [string]$detP.id_pretty
    if (-not $premPretty) {
        $premListRaw = Get-Content -LiteralPath $premListPath -Raw
        foreach ($m in [regex]::Matches($premListRaw, 'id_hex=([0-9a-fA-F]+)\s+id_pretty=([^\s]+)')) {
            if ($m.Groups[1].Value.ToLowerInvariant() -eq $premHex.ToLowerInvariant()) {
                $premPretty = $m.Groups[2].Value
                break
            }
        }
    }
    if (-not $premPretty) { throw "premine id_pretty not found for $premHex" }
    Add-R "## premine id_pretty=$premPretty"

    $cw1 = $childWallets | Where-Object { $_.K -eq 1 } | Select-Object -First 1
    $cw2 = $childWallets | Where-Object { $_.K -eq 2 } | Select-Object -First 1
    $cw3 = $childWallets | Where-Object { $_.K -eq 3 } | Select-Object -First 1
    if (-not $cw1) { throw 'need CyWalletCount>=1' }

    Add-R '## tx-init CY wallet #1 rescue + dormant emergency_redirect'
    Run-Pwm-Ok $RpcUrl (@('tx-init', '--wallet', $cw1.Path, '--index', ([string]$cw1.Der), '--flags', ([string]$cw1.Flags)) `
            + $V4 + @('--rescue-address', $premPretty, '--initial-policy', 'routing.emergency_redirect:dormant'))
    if (-not (Wait-AccountBalanceAtLeast $RpcUrl $cw1.Hex 0 60)) { throw 'CY wallet #1 init not visible' }

    Add-R '## tx-init CY wallet #2 (+ V4 meta; default_behavior applied after funding)'
    if ($cw2) {
        Run-Pwm-Ok $RpcUrl (@('tx-init', '--wallet', $cw2.Path, '--index', ([string]$cw2.Der), '--flags', ([string]$cw2.Flags)) + $V4)
        if (-not (Wait-AccountBalanceAtLeast $RpcUrl $cw2.Hex 0 60)) { throw 'CY wallet #2 init not visible' }
    }

    if ($cw3) {
        Add-R '## tx-init CY wallet #3 plain'
        Run-Pwm-Ok $RpcUrl @('tx-init', '--wallet', $cw3.Path, '--index', ([string]$cw3.Der), '--flags', ([string]$cw3.Flags))
        if (-not (Wait-AccountBalanceAtLeast $RpcUrl $cw3.Hex 0 60)) { throw 'CY wallet #3 init not visible' }
    }

    foreach ($cw in $childWallets) {
        Run-Pwm-Ok $RpcUrl @('tx-send', '--wallet', $PREM, '--to', $cw.Hex, '--amount', $fund, '--fee', $fee)
        if (-not (Wait-AccountBalanceAtLeast $RpcUrl $cw.Hex ([int64]$fund) 60)) {
            throw "funding not visible for wallet #$($cw.K)"
        }
    }

    if ($cw2) {
        Add-R '## tx-policy-set default_behavior immediately on wallet #2'
        Run-Pwm-Ok $RpcUrl @('tx-policy-set', '--wallet', $cw2.Path, '--policy', 'default_behavior', '--activation', 'immediately', '--fee', $fee)
        if (-not (Wait-AccountActivePoliciesNonZero $RpcUrl $cw2.Hex 60)) { throw 'default_behavior not visible on wallet #2' }
    }

    if ($cw3) {
        Add-R "## policy routing.same_domain_only on wallet3"
        Run-Pwm-Ok $RpcUrl @('tx-policy-set', '--wallet', $cw3.Path, '--policy', 'routing.same_domain_only', '--activation', 'immediately', '--fee', $fee)
        if (-not (Wait-AccountActivePoliciesNonZero $RpcUrl $cw3.Hex 60)) { throw 'routing.same_domain_only not visible on wallet #3' }
        Run-Pwm-Ok $RpcUrl @('tx-send', '--wallet', $PREM, '--to', $cw3.Hex, '--amount', '1000000', '--fee', $fee)
    }

    if ($cw2) {
        Add-R '## expect tx-send to default_behavior recipient to FAIL'
        Run-Pwm $RpcUrl @('tx-send', '--wallet', $PREM, '--to', $cw2.Hex, '--amount', '1000000', '--fee', $fee)
        if ($LASTEXITCODE -eq 0) { throw 'expected default_behavior reject but pwm exit 0' } else { Add-R ('negative-case pwm-exit:' + [string]$LASTEXITCODE + ' (ok)') }
    }

    Add-R '## activate emergency_redirect (rescuer cosign = premine wallet)'
    Run-Pwm-Ok $RpcUrl @(
        'tx-policy-activate',
        '--wallet', $cw1.Path,
        '--policy', 'routing.emergency_redirect',
        '--fee', $fee,
        '--rescue-wallet', $PREM,
        '--rescue-account-index', "$premDer"
    )
    if (-not (Wait-AccountActivePoliciesNonZero $RpcUrl $cw1.Hex 60)) { throw 'routing.emergency_redirect activation not visible on wallet #1' }

    $premRawBefore = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$premHex" -TimeoutSec 25
    $b0 = $premRawBefore.balance_pwm
    Run-Pwm-Ok $RpcUrl @('tx-send', '--wallet', $PREM, '--to', $cw1.Hex, '--amount', '5000000000', '--fee', $fee)
    $premRawAfter = Invoke-RestMethod -Uri "$RpcUrl/v1/account/$premHex" -TimeoutSec 25
    $b1 = $premRawAfter.balance_pwm
    Add-R "## premine raw before redirect demo: $b0 after: $b1"

    Add-R '## verdict: reached end (inspect pwmd stderr if anomalies)'
}
catch {
    Add-R "## FAIL: $($_.Exception.Message)"
}
finally {
    # Ensure pwmd trees are gone before writing the report (matches pwm-testing cleanup policy).
    if (-not $SkipCluster) {
        & taskkill.exe /F /IM pwmd.exe /T 2>$null | Out-Null
    }
    Set-Content -LiteralPath $ReportPath -Value ($report -join "`n") -Encoding utf8
    Write-Host "Report written: $ReportPath"
}
