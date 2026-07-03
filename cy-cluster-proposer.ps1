# RFC16 proposer (sealer) - shard CY lab. Sources cy-cluster-common.ps1 (genesis defaults: tmp\genesis-custom.json or $env:PWM_DEMO_GENESIS_PATH).
# UTF-8 with BOM - do not save as ANSI-only on Windows if you add non-ASCII text inside quoted strings.
param([switch]$Release, [switch]$Flamegraph)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
. (Join-Path $PSScriptRoot 'cy-cluster-common.ps1')
if ($Flamegraph) { $Release = $true }
$buildProfile = if ($Release) { 'release' } else { 'debug' }

foreach ($dir in @($CyStateProposer)) {
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
}

if ((Test-Path -LiteralPath $CyGenesis)) {
    echo "Using genesis template from $CyGenesis"
}
else {
    Write-Error "Missing genesis file: $CyGenesis - adjust cy-cluster-common or add tmp\genesis-custom.json"
}

Initialize-CyLabPeersFile

$clusterMembers = $CyInstanceProposer + ',' + $CyInstanceAttester

# Flamegraph: F:\pwm-test\shared\flamegraph\pwmd.exe (MSVC + debug symbols, run under samply).
# Release:    F:\pwm-test\shared\release\pwmd.exe   (MSVC PDB), fallback to GNU release.
# Debug:      rust-target-shared\debug\pwmd.exe.
if ($Flamegraph) {
    $pwmdExeAbs = "F:\pwm-test\shared\flamegraph\pwmd.exe"
} elseif ($Release) {
    $pwmdMsvc   = "F:\pwm-test\shared\release\pwmd.exe"
    $pwmdGnu    = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\rust-target-shared\release\pwmd.exe"))
    $pwmdExeAbs = if (Test-Path -LiteralPath $pwmdMsvc) { $pwmdMsvc } else { $pwmdGnu }
} else {
    $pwmdExeAbs = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\rust-target-shared\debug\pwmd.exe"))
}
$cargoReleaseFlag = if ($Release) { @('--release') } else { @() }

$cargoArgs = @(
    'run', '-p', 'pwmd', '--bin', 'pwmd') + $cargoReleaseFlag + @('--',
    '--listen', $CyRpcProposer,
    '--state-root', $CyStateProposer,
    '--data-file', (Join-Path $CyStateProposer 'pwm-data.json'),
    '--genesis-file', $CyGenesis,
    '--genesis-passphrase', $CyGenesisPass,
    '--network-id', $CyNetwork,
    '--domain-hi', $CyDomainHi,
    '--cluster-id', $CyClusterLabel,
    '--node-id', $CyNodeProposer,
    '--node-instance-id', $CyInstanceProposer,
    '--transport-real',
    '--transport-peer-listen', $CyPeerProposer,
    '--peers-list', $CyPeersFile,
    '--cluster-enabled',
    '--cluster-role', 'proposer',
    '--cluster-members', $clusterMembers,
    '--cluster-quorum-k', '1',
    '--cluster-quorum-n', '2',
    '--seal-lease-backend', 'process-local'
)

$pwmdArgs = @(
    '--listen', $CyRpcProposer,
    '--state-root', $CyStateProposer,
    '--data-file', (Join-Path $CyStateProposer 'pwm-data.json'),
    '--genesis-file', $CyGenesis,
    '--genesis-passphrase', $CyGenesisPass,
    '--network-id', $CyNetwork,
    '--domain-hi', $CyDomainHi,
    '--cluster-id', $CyClusterLabel,
    '--node-id', $CyNodeProposer,
    '--node-instance-id', $CyInstanceProposer,
    '--transport-real',
    '--transport-peer-listen', $CyPeerProposer,
    '--peers-list', $CyPeersFile,
    '--cluster-enabled',
    '--cluster-role', 'proposer',
    '--cluster-members', $clusterMembers,
    '--cluster-quorum-k', '1',
    '--cluster-quorum-n', '2',
    '--seal-lease-backend', 'process-local',
    '--cluster-attest-max-tip-lag', '2'
)

$modeLabel = if ($Flamegraph) { 'flamegraph' } else { $buildProfile }
Write-Host "Starting CY cluster proposer (sealer) [$modeLabel]. RPC=$CyRpcProposer peer=$CyPeerProposer peers-list=$CyPeersFile"
if ($Flamegraph) {
    if (-not (Test-Path -LiteralPath $pwmdExeAbs)) {
        Write-Error "Flamegraph binary not found: $pwmdExeAbs`nRun: build_release.cmd flamegraph"
        exit 1
    }
    Write-Host "Profiling with samply. Load the node with ramp, then Ctrl+C -- Firefox Profiler opens automatically."
    & samply record -- $pwmdExeAbs @pwmdArgs
} elseif (Test-Path -LiteralPath $pwmdExeAbs) {
    & $pwmdExeAbs @pwmdArgs
}
else {
    & cargo @cargoArgs
}
