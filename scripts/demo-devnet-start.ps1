param(
    [string]$GenesisPath = "tmp/genesis-custom.json",
    [string]$WalletPath = "tmp/demo-genesis-wallet.yaml",
    [string]$GenesisPassphrase,
    [string]$WalletPassphrase,
    [int]$DerivationIndex = -1,
    [string]$DemoMaster = "0000000000000000000000000000000000000000000000000000000000000001",
    [int]$DemoDerivationIndex = 287292,
    [switch]$UseCountryBruteforce,
    [int]$MaxTry = 120000,
    [UInt64]$PremineRaw = 21000000000000000,
    [switch]$SkipBuild,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot/..

if (-not $GenesisPassphrase) {
    $GenesisPassphrase = $env:PWM_GENESIS_PASSPHRASE
}
if (-not $GenesisPassphrase) {
    $GenesisPassphrase = "12345"
}
if (-not $WalletPassphrase) {
    $WalletPassphrase = $env:PWM_WALLET_PASSPHRASE
}

if (-not $SkipBuild) {
    $buildScript = Join-Path $PSScriptRoot "demo-genesis-build.ps1"
    $buildArgs = @{
        WalletPath = $WalletPath
        OutputPath = $GenesisPath
        PremineRaw = $PremineRaw
        GenesisPassphrase = $GenesisPassphrase
        DerivationIndex = $DerivationIndex
        DemoMaster = $DemoMaster
        DemoDerivationIndex = $DemoDerivationIndex
        MaxTry = $MaxTry
    }
    if ($WalletPassphrase) {
        $buildArgs["WalletPassphrase"] = $WalletPassphrase
    }
    if ($UseCountryBruteforce) {
        $buildArgs["UseCountryBruteforce"] = $true
    }
    if ($DryRun) {
        $buildArgs["DryRun"] = $true
    }

    Write-Host "==> preparing demo genesis package"
    & $buildScript @buildArgs
} else {
    Write-Host "SkipBuild enabled. Expecting existing genesis at: $GenesisPath"
}

Write-Host ""
Write-Host "Near-one-command path complete." -ForegroundColor Green
Write-Host "Open three terminals from repo root and run:"
Write-Host ""
Write-Host "  # Terminal 1 (proposer)"
Write-Host "  `$env:PWM_DEMO_GENESIS_PATH='$GenesisPath'; `$env:PWM_DEMO_GENESIS_PASSPHRASE='$GenesisPassphrase'; ./cy-cluster-proposer.ps1"
Write-Host ""
Write-Host "  # Terminal 2 (attester)"
Write-Host "  `$env:PWM_DEMO_GENESIS_PATH='$GenesisPath'; `$env:PWM_DEMO_GENESIS_PASSPHRASE='$GenesisPassphrase'; ./cy-cluster-attester.ps1"
Write-Host ""
Write-Host "  # Terminal 3 (follower)"
Write-Host "  `$env:PWM_DEMO_GENESIS_PATH='$GenesisPath'; `$env:PWM_DEMO_GENESIS_PASSPHRASE='$GenesisPassphrase'; ./cy-cluster-follower.ps1"
Write-Host ""
Write-Host "After nodes are up, run API smoke:"
Write-Host "  Invoke-RestMethod http://127.0.0.1:3030/v1/status"
Write-Host "  Invoke-RestMethod http://127.0.0.1:3030/v1/head"
Write-Host "  `$resp = Invoke-RestMethod http://127.0.0.1:3030/v1/accounts"
Write-Host '  Invoke-RestMethod "http://127.0.0.1:3030/v1/account/$($resp.accounts[0].id)"'
Write-Host ""
Write-Host "Wallet defaults: deterministic public demo seed/index for clean-clone reproducibility."
Write-Host "Use -UseCountryBruteforce for bounded random search or -DerivationIndex for explicit m/0/N."
