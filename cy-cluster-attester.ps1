# RFC16 attester - shard CY lab. Pair with cy-cluster-proposer.ps1 and cy-cluster-follower.ps1.
# UTF-8 with BOM - safe for Windows PowerShell 5.1 on localized Windows.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
. (Join-Path $PSScriptRoot 'cy-cluster-common.ps1')

foreach ($dir in @($CyStateAttester)) {
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
    '--listen', $CyRpcAttester,
    '--state-root', $CyStateAttester,
    '--data-file', (Join-Path $CyStateAttester 'pwm-data.json'),
    '--genesis-file', $CyGenesis,
    '--genesis-passphrase', $CyGenesisPass,
    '--network-id', $CyNetwork,
    '--domain-hi', $CyDomainHi,
    '--cluster-id', $CyClusterLabel,
    '--node-id', $CyNodeAttester,
    '--node-instance-id', $CyInstanceAttester,
    '--transport-real',
    '--transport-peer-listen', $CyPeerAttester,
    '--peers-list', $CyPeersFile,
    '--cluster-enabled',
    '--cluster-role', 'attester',
    '--cluster-members', $clusterMembers,
    '--cluster-quorum-k', '1',
    '--cluster-quorum-n', '2',
    '--seal-lease-backend', 'process-local'
)

# RFC16 §8.2: cluster-role=attester derives standby (replay-only, no competing local seal loop).
Write-Host "Starting CY cluster attester. RPC=$CyRpcAttester peer=$CyPeerAttester peers-list=$CyPeersFile (seal loop off; proposer seals)."
& cargo @cargoArgs
