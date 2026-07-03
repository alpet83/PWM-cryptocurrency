#!/usr/bin/env pwsh
# cy-cluster-proposer-flamegraph.ps1
# Run the proposer under samply to capture a CPU flamegraph.
#
# Usage:
#   .\cy-cluster-proposer-flamegraph.ps1
#
# Workflow:
#   1. Builds pwmd with [profile.flamegraph] (release-speed + debug symbols)
#   2. Wraps the proposer with `samply record`
#   3. On Ctrl+C, samply opens Firefox Profiler in your browser automatically
#
# Run the benchmark (ramp) in a separate terminal while this is running.
# Let it run for at least 30-60 seconds under load, then Ctrl+C.

param()

. (Join-Path $PSScriptRoot 'cy-cluster-common.ps1')
. (Join-Path $PSScriptRoot 'scripts\Import-PwmBuildEnv.ps1')
Initialize-PwmBuildEnv -RepoRoot $PSScriptRoot
Initialize-CyLabPeersFile

# ------ build ------
Write-Host "[flamegraph] Building pwmd with profile=flamegraph ..."
$env:CARGO_PROFILE_FLAMEGRAPH_DEBUG = "true"
& cargo build --profile flamegraph -p pwmd
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$buildDir = "F:\pwm-test\shared"
$pwmdExe  = Join-Path $buildDir "flamegraph\pwmd.exe"
Write-Host "[flamegraph] Binary: $pwmdExe"

# ------ cluster members ------
$clusterMembers = "$CyInstanceProposer,$CyInstanceAttester"

$pwmdArgs = @(
    '--listen',               $CyRpcProposer,
    '--state-root',           $CyStateProposer,
    '--data-file',            (Join-Path $CyStateProposer 'pwm-data.json'),
    '--genesis-file',         $CyGenesis,
    '--genesis-passphrase',   $CyGenesisPass,
    '--network-id',           $CyNetwork,
    '--domain-hi',            $CyDomainHi,
    '--cluster-id',           $CyClusterLabel,
    '--node-id',              $CyNodeProposer,
    '--node-instance-id',     $CyInstanceProposer,
    '--transport-real',
    '--transport-peer-listen',$CyPeerProposer,
    '--peers-list',           $CyPeersFile,
    '--cluster-enabled',
    '--cluster-role',         'proposer',
    '--cluster-members',      $clusterMembers,
    '--cluster-quorum-k',     '1',
    '--cluster-quorum-n',     '2',
    '--seal-lease-backend',   'process-local'
)

# ------ samply ------
Write-Host ""
Write-Host "[flamegraph] Starting proposer under samply. Load it with the ramp script, then Ctrl+C."
Write-Host "[flamegraph] samply will open Firefox Profiler automatically on exit."
Write-Host ""

& samply record -- $pwmdExe @pwmdArgs
