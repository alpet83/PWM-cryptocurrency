# RFC16 proposer (sealer) - shard CY lab. Sources cy-cluster-common.ps1 (genesis defaults: tmp\genesis-custom.json or $env:PWM_DEMO_GENESIS_PATH).
# UTF-8 with BOM - do not save as ANSI-only on Windows if you add non-ASCII text inside quoted strings.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
. (Join-Path $PSScriptRoot 'cy-cluster-common.ps1')

foreach ($dir in @($CyStateProposer)) {
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
}

if (-not (Test-Path -LiteralPath $CyGenesis)) {
    Write-Error "Missing genesis file: $CyGenesis - adjust cy-cluster-common or add tmp\genesis-custom.json"
}

Initialize-CyLabPeersFile

$clusterMembers = $CyInstanceProposer + ',' + $CyInstanceAttester

$cargoArgs = @(
    'run', '-p', 'pwmd', '--bin', 'pwmd', '--',
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

Write-Host "Starting CY cluster proposer (sealer). RPC=$CyRpcProposer peer=$CyPeerProposer peers-list=$CyPeersFile"
& cargo @cargoArgs
