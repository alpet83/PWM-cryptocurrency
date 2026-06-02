# Shared devnet state archive + CleanState helpers for PowerShell harness scripts.
# Dot-source from scripts/*.ps1 — not invoked directly.

function Get-DevnetCleanStatePatterns {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [ValidateSet('CyCluster', 'FullTmpDevnet')]
        [string]$Profile = 'CyCluster'
    )
    $tmp = Join-Path $RepoRoot 'tmp'
    if ($Profile -eq 'FullTmpDevnet') {
        return @(
            (Join-Path $RepoRoot 'tmp\state-*'),
            (Join-Path $RepoRoot 'tmp\cy-*'),
            (Join-Path $RepoRoot 'tmp\genesis-custom.json'),
            (Join-Path $RepoRoot 'tmp\demo-genesis-wallet.yaml')
        )
    }
    return @(
        (Join-Path $tmp 'state-cy-proposer'),
        (Join-Path $tmp 'state-cy-attester'),
        (Join-Path $tmp 'state-cy-follower'),
        (Join-Path $tmp 'cy-lab-peers.yaml')
    )
}

function Resolve-DevnetCleanStatePaths {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$PathPatterns
    )
    $resolved = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($pattern in $PathPatterns) {
        if ($pattern -match '[\*\?]') {
            Get-ChildItem -Path $pattern -ErrorAction SilentlyContinue | ForEach-Object {
                [void]$resolved.Add($_.FullName)
            }
        }
        elseif (Test-Path -LiteralPath $pattern) {
            $full = (Resolve-Path -LiteralPath $pattern).Path
            [void]$resolved.Add($full)
        }
    }
    return @($resolved | Sort-Object)
}

function Remove-OldDevnetStateArchives {
    param(
        [Parameter(Mandatory = $true)][string]$ArchiveDir,
        [int]$MaxArchives = 30
    )
    if (-not (Test-Path -LiteralPath $ArchiveDir)) {
        return
    }
    $keep = [Math]::Max(1, $MaxArchives)
    Get-ChildItem -LiteralPath $ArchiveDir -Filter 'devnet-state_*.zip' -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -Skip $keep |
        ForEach-Object {
            Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
        }
}

function Save-DevnetStateArchive {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$SourcePaths,
        [string]$Label = 'manual',
        [int]$MaxArchives = 30,
        [scriptblock]$Log = { param($m) Write-Host $m }
    )
    if ($SourcePaths.Count -eq 0) {
        return $null
    }

    $archiveDir = Join-Path $RepoRoot 'tmp\archives'
    New-Item -ItemType Directory -Path $archiveDir -Force | Out-Null

    $stamp = Get-Date -Format 'yyyyMMdd_HHmmss'
    $safeLabel = ($Label -replace '[^\w\-]+', '_').Trim('_')
    if (-not $safeLabel) { $safeLabel = 'manual' }
    $zipName = "devnet-state_${stamp}_${safeLabel}.zip"
    $zipPath = Join-Path $archiveDir $zipName

    $staging = Join-Path $archiveDir ".staging_$stamp"
    if (Test-Path -LiteralPath $staging) {
        Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
    }
    New-Item -ItemType Directory -Path $staging -Force | Out-Null

    $repoRootNorm = (Resolve-Path -LiteralPath $RepoRoot).Path.TrimEnd('\')
    $manifest = New-Object System.Collections.Generic.List[string]
    $manifest.Add("archived_at=$((Get-Date).ToString('o'))")
    $manifest.Add("label=$Label")
    $manifest.Add("repo_root=$repoRootNorm")
    $manifest.Add('paths:')

    foreach ($src in $SourcePaths) {
        if (-not (Test-Path -LiteralPath $src)) {
            continue
        }
        $srcNorm = (Resolve-Path -LiteralPath $src).Path
        if (-not $srcNorm.StartsWith($repoRootNorm, [StringComparison]::OrdinalIgnoreCase)) {
            $rel = Split-Path -Leaf $srcNorm
        }
        else {
            $rel = $srcNorm.Substring($repoRootNorm.Length).TrimStart('\', '/')
        }
        $dest = Join-Path $staging $rel
        $destParent = Split-Path -Parent $dest
        if ($destParent -and -not (Test-Path -LiteralPath $destParent)) {
            New-Item -ItemType Directory -Path $destParent -Force | Out-Null
        }
        Copy-Item -LiteralPath $srcNorm -Destination $dest -Recurse -Force
        $manifest.Add("- $rel")
    }

    if ($manifest.Count -le 4) {
        Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
        return $null
    }

    Set-Content -LiteralPath (Join-Path $staging 'MANIFEST.txt') -Value ($manifest -join "`n") -Encoding utf8

    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }
    Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zipPath -CompressionLevel Optimal
    Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue

    Remove-OldDevnetStateArchives -ArchiveDir $archiveDir -MaxArchives $MaxArchives
    & $Log "- archived devnet state -> $zipPath"
    return $zipPath
}

function Invoke-DevnetCleanStateWithArchive {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$PathPatterns,
        [string]$Label = 'cleanstate',
        [int]$MaxArchives = 30,
        [switch]$SkipArchive,
        [scriptblock]$Log = { param($m) Write-Host $m }
    )

    $resolved = @(Resolve-DevnetCleanStatePaths -RepoRoot $RepoRoot -PathPatterns $PathPatterns)
    if ($resolved.Count -eq 0) {
        & $Log '- cleanstate: nothing to remove (archive skipped)'
        return [pscustomobject]@{
            Archived    = $false
            ArchivePath = $null
            Removed     = @()
        }
    }

    $archivePath = $null
    if (-not $SkipArchive) {
        $archivePath = Save-DevnetStateArchive -RepoRoot $RepoRoot -SourcePaths $resolved -Label $Label `
            -MaxArchives $MaxArchives -Log $Log
    }
    else {
        & $Log '- cleanstate: archive skipped (-SkipArchive)'
    }

    $removed = New-Object System.Collections.Generic.List[string]
    foreach ($p in $resolved) {
        Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue
        $removed.Add($p)
        & $Log "- removed $p"
    }

    return [pscustomobject]@{
        Archived    = ($null -ne $archivePath)
        ArchivePath = $archivePath
        Removed     = $removed.ToArray()
    }
}
