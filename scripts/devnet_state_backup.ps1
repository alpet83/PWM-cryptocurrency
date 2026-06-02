# Manual devnet state backup (no CleanState). Creates zip under tmp/archives/.
#
# Examples (repo root):
#   ./scripts/devnet_state_backup.ps1
#   ./scripts/devnet_state_backup.ps1 -Profile CyCluster -Label before_experiment
#   ./scripts/devnet_state_backup.ps1 -Profile FullTmpDevnet -Label nightly_cy

param(
    [string]$RepoRoot = '',
    [ValidateSet('CyCluster', 'FullTmpDevnet')]
    [string]$Profile = 'FullTmpDevnet',
    [string]$Label = 'manual',
    [int]$MaxArchives = 30
)

$ErrorActionPreference = 'Stop'
if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
Set-Location -LiteralPath $RepoRoot

. (Join-Path $PSScriptRoot '_devnet_clean_state.ps1')

$patterns = Get-DevnetCleanStatePatterns -RepoRoot $RepoRoot -Profile $Profile
$resolved = @(Resolve-DevnetCleanStatePaths -RepoRoot $RepoRoot -PathPatterns $patterns)
if ($resolved.Count -eq 0) {
    Write-Host "Nothing to archive under profile $Profile (patterns: $($patterns -join ', '))"
    exit 0
}

$zip = Save-DevnetStateArchive -RepoRoot $RepoRoot -SourcePaths $resolved -Label $Label `
    -MaxArchives $MaxArchives -Log { param($m) Write-Host $m }
if (-not $zip) {
    Write-Error 'Archive failed or produced empty archive.'
}
Write-Host "Backup OK: $zip"
exit 0
