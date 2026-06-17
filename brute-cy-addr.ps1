. (Join-Path $PSScriptRoot 'scripts\Import-PwmBuildEnv.ps1')
Initialize-PwmBuildEnv -RepoRoot $PSScriptRoot


cargo run -p pwm-cli --bin pwm -- --rpc http://127.0.0.1:3030 addr-bruteforce --wallet-out .\tmp\demo-genesis-wallet.yaml --max-try 1500200 --domain CY --flags-mask 1023 --expected-flags 0