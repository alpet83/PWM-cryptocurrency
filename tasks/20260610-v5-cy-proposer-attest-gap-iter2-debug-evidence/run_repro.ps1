
$ErrorActionPreference = 'Continue'
Set-Location 'P:\opt\docker\pwm-protocol'
$dir = 'P:\opt\docker\pwm-protocol\tasks\20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence'
$env:RUST_BACKTRACE = 'full'
$env:RUST_LIB_BACKTRACE = '1'
$env:RUST_LOG = 'pwmd::lifecycle=debug,pwmd::peer=debug,pwmd::sync=debug,pwm_core::state=info'
Get-Process pwmd -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
$attOut = Join-Path $dir 'repro-attester-stdout.log'
$attErr = Join-Path $dir 'repro-attester-stderr.log'
$propOut = Join-Path $dir 'repro-proposer-stdout.log'
$propErr = Join-Path $dir 'repro-proposer-stderr.log'
$att = Start-Process powershell.exe -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File','P:\opt\docker\pwm-protocol\cy-cluster-attester.ps1') -RedirectStandardOutput $attOut -RedirectStandardError $attErr -PassThru
Start-Sleep -Seconds 3
$prop = Start-Process powershell.exe -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File','P:\opt\docker\pwm-protocol\cy-cluster-proposer.ps1') -RedirectStandardOutput $propOut -RedirectStandardError $propErr -PassThru
Start-Sleep -Seconds 300
Get-Process pwmd -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
foreach ($proc in @($att, $prop)) {
    if ($proc -and -not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    }
}
'bounded_repro_done'
