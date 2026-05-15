# Third node of CY lab: same-shard follower (no RFC16 quorum role).
# Peer mesh: общий tmp\cy-lab-peers.yaml (--peers-list), как у proposer/attester.
# Recommended start order (see cy-cluster-common.ps1): follower + attester first, then proposer.
# UTF-8 with BOM - safe for Windows PowerShell 5.1 on localized Windows.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
. (Join-Path $PSScriptRoot 'cy-cluster-common.ps1')

foreach ($dir in @($CyStateFollower)) {
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
}

if (-not (Test-Path -LiteralPath $CyGenesis)) {
    Write-Error "Missing genesis file: $CyGenesis - adjust cy-cluster-common or add tmp\genesis-custom.json"
}

Initialize-CyLabPeersFile

$cargoArgs = @(
    'run', '-p', 'pwmd', '--bin', 'pwmd', '--',
    '--listen', $CyRpcFollower,
    '--state-root', $CyStateFollower,
    '--data-file', (Join-Path $CyStateFollower 'pwm-data.json'),
    '--genesis-file', $CyGenesis,
    '--genesis-passphrase', $CyGenesisPass,
    '--network-id', $CyNetwork,
    '--domain-hi', $CyDomainHi,
    '--cluster-id', $CyClusterLabel,
    '--node-id', $CyNodeFollower,
    '--node-instance-id', $CyInstanceFollower,
    '--transport-real',
    '--transport-peer-listen', $CyPeerFollower,
    '--peers-list', $CyPeersFile,
    '--seal-lease-backend', 'process-local',
    '--debug-disable-seal-loop'
)

Write-Host "Starting CY cluster follower (3-node lab). RPC=$CyRpcFollower peer=$CyPeerFollower peers-list=$CyPeersFile; seal loop off; lease=process-local."
& cargo @cargoArgs
