# Parse proposer log: suppression windows vs checkpoint rhythm and pending pressure.
# Usage:
#   .\scripts\analyze_seal_suppression_overnight.ps1 -LogPath logs\2026-05-30\pwmd-cy-proposer-*.log

param(
    [Parameter(Mandatory = $true)]
    [string]$LogPath
)

$ErrorActionPreference = 'Stop'
$files = @(Get-ChildItem -LiteralPath $LogPath -ErrorAction SilentlyContinue)
if ($files.Count -eq 0) {
    Write-Error "No log files: $LogPath"
}

$rows = New-Object System.Collections.Generic.List[object]
$lastSealed = $null

foreach ($f in $files) {
    Get-Content -LiteralPath $f.FullName -Encoding UTF8 | ForEach-Object {
        $line = $_
        if ($line -match 'build control marker=pwmd/([\d.]+)') {
            Write-Host "binary marker: pwmd/$($Matches[1]) ($($f.Name))"
        }
        if ($line -match 'sealed height=(\d+)') {
            $script:lastSealed = [int64]$Matches[1]
        }
        if ($line -match 'seal_suppression_summary window_sec=(\d+) slots=(\d+) slots_waited_att=(\d+) slots_timeout=(\d+) slots_struck=(\d+) suppression_pct=([\d.]+) sealed_in_window=(\d+)') {
            $h = $lastSealed
            $rows.Add([pscustomobject]@{
                file               = $f.Name
                tip_h              = $h
                h_mod_100          = if ($null -ne $h) { $h % 100 } else { $null }
                h_mod_1000         = if ($null -ne $h) { $h % 1000 } else { $null }
                near_chk_100       = if ($null -ne $h) { ($h % 100) -ge 95 -or ($h % 100) -le 5 } else { $false }
                near_epoch_1000    = if ($null -ne $h) { ($h % 1000) -ge 995 -or ($h % 1000) -le 5 } else { $false }
                slots              = [int]$Matches[2]
                slots_struck       = [int]$Matches[5]
                suppression_pct    = [double]$Matches[6]
                sealed_in_window   = [int]$Matches[7]
                slots_waited_att   = [int]$Matches[3]
                ratio_struck_slots = if ([int]$Matches[2] -gt 0) {
                    '{0:F4}' -f ([int]$Matches[5] / [double][int]$Matches[2])
                } else { '' }
            })
        }
        if ($line -match 'cluster_gate_pending_summary pending_ticks_since_last_sealed=(\d+) sealed_h=(\d+)') {
            $script:lastPending = [int]$Matches[1]
            $script:lastPendingH = [int64]$Matches[2]
        }
    }
}

if ($rows.Count -eq 0) {
    Write-Warning 'No seal_suppression_summary lines found.'
    exit 0
}

Write-Host "`n--- suppression windows ($($rows.Count)) ---"
$rows | Sort-Object tip_h | Format-Table -AutoSize tip_h, suppression_pct, slots, slots_struck, ratio_struck_slots, sealed_in_window, slots_waited_att, near_chk_100, near_epoch_1000

$exactThird = $rows | Where-Object {
    $_.slots -gt 0 -and ($_.slots_struck * 3) -eq $_.slots
}
if ($exactThird.Count -gt 0) {
    Write-Host "`nWindows with struck/slots = 1/3 exactly: $($exactThird.Count)"
    $exactThird | Format-Table tip_h, slots, slots_struck, suppression_pct
}

$near100 = $rows | Where-Object { $_.near_chk_100 }
$far100 = $rows | Where-Object { -not $_.near_chk_100 }
if ($near100.Count -gt 0 -and $far100.Count -gt 0) {
    $avgNear = ($near100 | Measure-Object -Property suppression_pct -Average).Average
    $avgFar = ($far100 | Measure-Object -Property suppression_pct -Average).Average
    Write-Host ("`nAvg suppression_pct near h%100 boundary (±5 blocks): {0:F2}" -f $avgNear)
    Write-Host ("Avg suppression_pct other windows: {0:F2}" -f $avgFar)
}
