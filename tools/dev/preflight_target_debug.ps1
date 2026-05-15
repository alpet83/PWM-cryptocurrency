# Preflight for pwm-testing (fallback when bash/git_bash_exec unavailable).
# Compares sum of file Length under target/debug to threshold bytes (default = 4096 MiB).
# Run from repository root: pwsh -File tools/dev/preflight_target_debug.ps1
# Override: $env:PWM_PREFLIGHT_TARGET_DEBUG_MIB = '<integer>'

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath 'Cargo.toml')) {
    Write-Error 'preflight_target_debug.ps1: run from repository root (Cargo.toml not found)'
    exit 1
}

$mib = 4096
if ($env:PWM_PREFLIGHT_TARGET_DEBUG_MIB -match '^\d+$') {
    $mib = [int]$env:PWM_PREFLIGHT_TARGET_DEBUG_MIB
}
$thresholdBytes = [long]$mib * 1024L * 1024L

$rel = 'target/debug'
if (-not (Test-Path -LiteralPath $rel)) {
    Write-Host "pwm-testing preflight: no $rel"
    exit 0
}

$sum = (Get-ChildItem -LiteralPath $rel -Recurse -File -ErrorAction SilentlyContinue |
    Measure-Object -Property Length -Sum).Sum
if ($null -eq $sum) { $sum = 0 }

if ($sum -gt $thresholdBytes) {
    # ASCII hyphen only (Unicode em-dash breaks Windows PowerShell 5.1 parser on some hosts).
    Write-Host ("pwm-testing preflight: {0} {1} bytes > {2}MiB ({3} bytes) - Remove-Item -Recurse -Force" -f $rel, $sum, $mib, $thresholdBytes)
    Remove-Item -LiteralPath $rel -Recurse -Force
}
else {
    Write-Host ("pwm-testing preflight: {0} {1} bytes (threshold {2}MiB)" -f $rel, $sum, $mib)
}
