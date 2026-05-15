#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Count high-signal pwmd log substrings (cluster, sync, seal) in log files.

.DESCRIPTION
  For CY lab / long runs: grep-style counts without JSON parsing. Use -LogDir to scan all *.log
  in a folder, or pass explicit file paths. Add -PerFile for one table per file (large dirs:
  still reads full files).

.PARAMETER LogDir
  Directory containing *.log files (non-recursive).

.PARAMETER Paths
  Explicit log file paths (UTF-8).

.PARAMETER PerFile
  Print pattern counts per file when using -LogDir or multiple Paths.

.EXAMPLE
  .\scripts\scan_pwmd_log_counters.ps1 -LogDir .\logs\2026-05-14
.EXAMPLE
  .\scripts\scan_pwmd_log_counters.ps1 -LogDir .\logs\2026-05-14 -PerFile
#>
[CmdletBinding(DefaultParameterSetName = 'Paths')]
param(
    [Parameter(ParameterSetName = 'Dir', Mandatory = $true)]
    [string]$LogDir,

    [Parameter(ParameterSetName = 'Paths', Mandatory = $true, Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$Paths,

    [Parameter()]
    [switch]$PerFile
)

$patterns = [ordered]@{
    "sealed height"             = "sealed height="
    "seal_suppressed_by_cluster" = "seal_suppressed_by_cluster"
    "reason=quorum_timeout"     = "reason=quorum_timeout"
    "reason=quorum_pending"    = "reason=quorum_pending"
    "binding_mismatch"         = "binding_mismatch"
    "cluster attest dropped"   = "cluster attest dropped"
    "Sync progress"            = "Sync progress"
    "sync_tip_divergence"      = "sync_tip_divergence"
    "TipDivergence"            = "TipDivergence"
    "seal_lease_acquired"      = "seal_lease_acquired"
    "seal_lease_renewed"       = "seal_lease_renewed"
    "ERROR"                    = "#ERROR"
}

function Count-PatternsInText([string]$text) {
    $out = @{}
    foreach ($key in $patterns.Keys) {
        $needle = $patterns[$key]
        $out[$key] = [regex]::Matches($text, [regex]::Escape($needle)).Count
    }
    return $out
}

function Show-Counts([hashtable]$counts, [string]$label) {
    Write-Host "---- $label ----"
    foreach ($key in $patterns.Keys) {
        Write-Host ("  {0,-28} {1}" -f $key, $counts[$key])
    }
}

$resolved = [System.Collections.Generic.List[string]]::new()
if ($PSCmdlet.ParameterSetName -eq 'Dir') {
    if (-not (Test-Path -LiteralPath $LogDir -PathType Container)) {
        Write-Error "Not a directory: $LogDir"
        exit 2
    }
    Get-ChildItem -LiteralPath $LogDir -Filter '*.log' -File | Sort-Object Name | ForEach-Object {
        $resolved.Add($_.FullName)
    }
    if ($resolved.Count -eq 0) {
        Write-Warning "No *.log under $LogDir"
        exit 0
    }
} else {
    foreach ($item in $Paths) {
        $resolved.Add($item)
    }
}

if ($PerFile -and $resolved.Count -gt 0) {
    foreach ($fp in $resolved) {
        if (-not (Test-Path -LiteralPath $fp)) {
            Write-Error "Missing file: $fp"
            continue
        }
        $raw = Get-Content -LiteralPath $fp -Raw -Encoding UTF8
        $lines = ($raw -split "`n").Count
        Write-Host ""
        Write-Host "file=$([IO.Path]::GetFileName($fp)) lines~=$lines bytes=$($raw.Length)"
        $c = Count-PatternsInText $raw
        Show-Counts $c "counts"
    }
    exit 0
}

$sb = [System.Text.StringBuilder]::new()
foreach ($fp in $resolved) {
    if (-not (Test-Path -LiteralPath $fp)) {
        Write-Error "Missing file: $fp"
        continue
    }
    $raw = Get-Content -LiteralPath $fp -Raw -Encoding UTF8
    [void]$sb.AppendLine("=== $fp ===")
    [void]$sb.Append($raw)
}
$text = $sb.ToString()
$lines = ($text -split "`n").Count

Write-Host "scan_pwmd_log_counters: files=$($resolved.Count) approx_lines=$lines bytes=$($text.Length)"
$all = Count-PatternsInText $text
Show-Counts $all "aggregate"
