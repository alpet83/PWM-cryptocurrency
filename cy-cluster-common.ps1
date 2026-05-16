# Shared lab constants: RFC16 2-node quorum (k=1 attester ACK, n=2) on shard CY.
# Same genesis convention as legacy node-1.ps1.
# Encoding: UTF-8 with BOM (required for Windows PowerShell 5.1 + non-ASCII comments on RU locales).
#
# Start order: any; peers retry. For a clean lab: start attester and follower first, then proposer.
# Peer mesh: общий файл tmp\cy-lab-peers.yaml (генерируется из $CyPeer* / $CyNode*), все лаунчеры передают
# --peers-list; pwmd сам убирает из сидов адрес текущего --transport-peer-listen.
#
# PROPOSER + cluster: proposer opens round-state for tip_h+1, sends ClusterPropose on peer wire,
# and waits for ClusterAttest quorum before Chain::seal (RFC16 Variant A).
#
# ATTESTER: в RFC16 Variant A локальный seal не выполняется — роль `attester` даёт standby
# (см. `derive_seal_role` / RFC16 §8.2); лаунчер `cy-cluster-attester.ps1` не требует `--debug-disable-seal-loop`.
# FOLLOWER (без кластера): `--debug-disable-seal-loop` остаётся для replay-only / вне-кластерных harness.
#
# Lease backend in this lab is process-local to avoid stale shared file-lease CAS conflicts across
# repeated local runs. This is lab-only and assumes single active sealer + non-sealing attester.

$script:CyGenesis = if ($env:PWM_DEMO_GENESIS_PATH) { $env:PWM_DEMO_GENESIS_PATH } else { Join-Path $PSScriptRoot 'tmp\genesis-custom.json' }
$script:CyGenesisPass = if ($env:PWM_DEMO_GENESIS_PASSPHRASE) { $env:PWM_DEMO_GENESIS_PASSPHRASE } else { '12345' }
$script:CyNetwork = 'testnet-qa'
$script:CyDomainHi = '0x2C'
$script:CyClusterLabel = 'test-cluster-CY'

# Stable wire ids - must match --cluster-members on quorum nodes and each --node-instance-id.
$script:CyInstanceProposer = 'cy-quorum-proposer'
$script:CyInstanceAttester = 'cy-quorum-attester'
$script:CyInstanceFollower = 'cy-follow'

# Human-readable --node-id (logs, snapshot paths).
$script:CyNodeProposer = 'cy-proposer'
$script:CyNodeAttester = 'cy-attester'
$script:CyNodeFollower = 'cy-follower'

# Lab «как несколько хостов»: разные loopback-адреса, одни и те же номера портов — HTTP 3030, peer TCP 13030.
# На части Windows (Hyper-V/WSL) диапазон 313x может дать bind error 10013; тогда верните 33430–33432 здесь.
$script:CyRpcProposer = '127.0.0.1:3030'
$script:CyPeerProposer = '127.0.0.1:13030'
$script:CyRpcAttester = '127.0.0.2:3030'
$script:CyPeerAttester = '127.0.0.2:13030'
$script:CyRpcFollower = '127.0.0.3:3030'
$script:CyPeerFollower = '127.0.0.3:13030'

$script:CyStateProposer = Join-Path $PSScriptRoot 'tmp\state-cy-proposer'
$script:CyStateAttester = Join-Path $PSScriptRoot 'tmp\state-cy-attester'
$script:CyStateFollower = Join-Path $PSScriptRoot 'tmp\state-cy-follower'

# Shared multishard peers file (v2 YAML under tmp/ — gitignored with genesis/state).
$script:CyPeersFile = Join-Path $PSScriptRoot 'tmp\cy-lab-peers.yaml'

function Initialize-CyLabPeersFile {
    $tmpRoot = Split-Path -Parent $CyPeersFile
    if (-not (Test-Path -LiteralPath $tmpRoot)) {
        New-Item -ItemType Directory -Path $tmpRoot -Force | Out-Null
    }
    # Single source of truth: addresses below; format matches pwmd --peers-list (shards / domain-hi key).
    $lines = @(
        'shards:',
        ('  "' + $CyDomainHi + '":'),
        ('    - id: ' + $CyNodeProposer),
        ('      peer: ' + $CyPeerProposer),
        '      validator: true',
        ('    - id: ' + $CyNodeAttester),
        ('      peer: ' + $CyPeerAttester),
        '      validator: true',
        ('    - id: ' + $CyNodeFollower),
        ('      peer: ' + $CyPeerFollower),
        '      validator: false'
    )
    Set-Content -LiteralPath $CyPeersFile -Value $lines -Encoding utf8
}
