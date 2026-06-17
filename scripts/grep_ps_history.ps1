#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Grep PowerShell history and print unique matches.

.DESCRIPTION
  Searches PSReadLine history and returns unique matching commands with first-seen metadata.
  Default PSReadLine history has no per-line timestamps, so FirstLine is the reliable ordering key.

.PARAMETER Pattern
  Pattern to search in command history.

.PARAMETER HistoryPath
  Optional explicit path to a PSReadLine history file.

.PARAMETER Regex
  Treat Pattern as a regex. Default is literal substring match.

.PARAMETER CaseSensitive
  Enable case-sensitive matching.

.PARAMETER Limit
  Maximum number of unique results to output.

.EXAMPLE
  .\scripts\grep_ps_history.ps1 -Pattern 'cy-cluster'

.EXAMPLE
  .\scripts\grep_ps_history.ps1 -Pattern 'cargo test' -Regex

.EXAMPLE
  .\scripts\grep_ps_history.ps1 -Pattern 'pwmd' -HistoryPath $env:APPDATA\Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Pattern,
    [string]$HistoryPath,
    [switch]$Regex,
    [switch]$CaseSensitive,
    [Parameter()]
    [ValidateRange(1, 1000000)]
    [int]$Limit
)

function Resolve-HistoryPath {
    param([string]$ExplicitPath)
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        return $ExplicitPath
    }
    $candidates = [System.Collections.Generic.List[string]]::new()
    try {
        $opt = Get-PSReadLineOption -ErrorAction Stop
        if ($null -ne $opt -and -not [string]::IsNullOrWhiteSpace($opt.HistorySavePath)) {
            $candidates.Add($opt.HistorySavePath)
        }
    } catch {}
    if (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) {
        $candidates.Add((Join-Path $env:APPDATA "Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt"))
        $candidates.Add((Join-Path $env:APPDATA "Microsoft\PowerShell\PSReadLine\ConsoleHost_history.txt"))
    }
    foreach ($path in $candidates) {
        if (-not [string]::IsNullOrWhiteSpace($path) -and (Test-Path -LiteralPath $path -PathType Leaf)) {
            return $path
        }
    }
    return $null
}

function Get-FirstSeenTimestamp {
    param([string]$CommandLine)
    if ([string]::IsNullOrWhiteSpace($CommandLine)) {
        return $null
    }
    $match = [regex]::Match(
        $CommandLine,
        '^\s*(?<ts>\d{4}-\d{2}-\d{2}(?:[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?)'
    )
    if (-not $match.Success) {
        return $null
    }
    $raw = $match.Groups['ts'].Value
    $dto = [datetimeoffset]::MinValue
    if ([datetimeoffset]::TryParse($raw, [ref]$dto)) {
        return $dto.ToString("yyyy-MM-dd HH:mm:ss zzz")
    }
    $dt = [datetime]::MinValue
    if ([datetime]::TryParse($raw, [ref]$dt)) {
        return $dt.ToString("yyyy-MM-dd HH:mm:ss")
    }
    return $null
}

$resolvedPath = Resolve-HistoryPath -ExplicitPath $HistoryPath
if ([string]::IsNullOrWhiteSpace($resolvedPath)) {
    Write-Error "PSReadLine history file not found. Provide -HistoryPath explicitly."
    exit 2
}

if (-not (Test-Path -LiteralPath $resolvedPath -PathType Leaf)) {
    Write-Error "History file does not exist: $resolvedPath"
    exit 2
}

$cmp = if ($CaseSensitive) { [System.StringComparison]::Ordinal } else { [System.StringComparison]::OrdinalIgnoreCase }
$rxOpt = if ($CaseSensitive) { [System.Text.RegularExpressions.RegexOptions]::None } else { [System.Text.RegularExpressions.RegexOptions]::IgnoreCase }
$rows = [System.Collections.Generic.List[object]]::new()
$seen = [System.Collections.Generic.Dictionary[string, object]]::new()

$lineNo = 0
$lines = Get-Content -LiteralPath $resolvedPath -Encoding UTF8
$rows = [System.Collections.Generic.List[object]]::new()
foreach ($line in $lines) {
    $lineNo++
    if ($null -eq $line) {
        continue
    }
    $command = $line.TrimEnd()
    if ([string]::IsNullOrWhiteSpace($command)) {
        continue
    }
    $isMatch = if ($Regex) {
        [regex]::IsMatch($command, $Pattern, $rxOpt)
    } else {
        $command.IndexOf($Pattern, $cmp) -ge 0
    }
    if (-not $isMatch) {
        continue
    }
    if ($seen.ContainsKey($command)) {
        $seen[$command].Count++
    } else {
        $row = [pscustomobject]@{
            Command = $command
            FirstLine = $lineNo
            FirstSeen = (Get-FirstSeenTimestamp -CommandLine $command)
            Count = 1
        }
        $seen[$command] = $row
        $rows.Add($row)
    }
}

if ($PSBoundParameters.ContainsKey('Limit')) {
    $rows | Select-Object -First $Limit
} else {
    $rows
}
