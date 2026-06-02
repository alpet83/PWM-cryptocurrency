param(
    [string]$WalletPath = "tmp/demo-genesis-wallet.yaml",
    [string]$OutputPath = "tmp/genesis-custom.json",
    [string]$Country = "CY",
    # -1: use deterministic public demo wallet seed/index (default, stable and fast).
    # >=0: explicit m/0/<N> (must satisfy pwm-cli recipient-domain policy; fixed 0 is often invalid).
    [int]$DerivationIndex = -1,
    # Public demo-only seed material for deterministic clean-clone path (NOT production secret).
    [string]$DemoMaster = "0000000000000000000000000000000000000000000000000000000000000001",
    [int]$DemoDerivationIndex = 287292,
    # Optional fallback mode: random seed + country brute-force (bounded by --max-try).
    [switch]$UseCountryBruteforce,
    [int]$MaxTry = 120000,
    [UInt64]$PremineRaw = 21000000000000000,
    [string]$WalletPassphrase,
    [string]$GenesisPassphrase,
    [switch]$ForceRecreateWallet,
    [switch]$SkipVerify,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot/..

if (-not $WalletPassphrase) {
    $WalletPassphrase = $env:PWM_WALLET_PASSPHRASE
}
if (-not $GenesisPassphrase) {
    $GenesisPassphrase = $env:PWM_GENESIS_PASSPHRASE
}
if (-not $GenesisPassphrase) {
    # Devnet-only default for compatibility with existing CY scripts.
    $GenesisPassphrase = "12345"
}

function Invoke-CheckedCommand {
    param(
        [string]$Name,
        [string[]]$CmdArgs
    )
    Write-Host "==> $Name"
    Write-Host "    cargo $($CmdArgs -join ' ')"
    if ($DryRun) {
        return
    }
    & cargo @CmdArgs
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

$walletDir = Split-Path -Parent $WalletPath
if ($walletDir -and -not (Test-Path -LiteralPath $walletDir)) {
    New-Item -ItemType Directory -Path $walletDir -Force | Out-Null
}

$outDir = Split-Path -Parent $OutputPath
if ($outDir -and -not (Test-Path -LiteralPath $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

if ((Test-Path -LiteralPath $WalletPath) -and $ForceRecreateWallet) {
    if ($DryRun) {
        Write-Host "DryRun: would remove existing wallet $WalletPath"
    } else {
        Remove-Item -LiteralPath $WalletPath -Force
    }
}

if (-not (Test-Path -LiteralPath $WalletPath)) {
    $walletArgs = @("run", "-p", "pwm-cli", "--bin", "pwm", "--")
    if ($WalletPassphrase) {
        $walletArgs += @("--wallet-passphrase", $WalletPassphrase)
    }

    $walletArgs += @("wallet", "init", "--country", $Country, "--wallet-out", $WalletPath)

    if ($DerivationIndex -ge 0) {
        Write-Host "Wallet init mode: explicit derivation index m/0/$DerivationIndex"
        $walletArgs += @("--derivation-index", "$DerivationIndex")
    } elseif ($UseCountryBruteforce) {
        Write-Host "Wallet init mode: country brute-force (bounded max_try=$MaxTry)"
        $walletArgs += @("--max-try", "$MaxTry")
    } else {
        Write-Host "Wallet init mode: deterministic public demo seed/index"
        Write-Host "  demo_master=$DemoMaster"
        Write-Host "  demo_derivation_index=$DemoDerivationIndex"
        $walletArgs += @("--master", $DemoMaster, "--derivation-index", "$DemoDerivationIndex")
    }
    # pwm-cli defaults to encrypted wallets; demo path without PWM_WALLET_PASSPHRASE must opt in explicitly.
    if (-not $WalletPassphrase) {
        $walletArgs += "--plaintext-dev"
    }
    Invoke-CheckedCommand -Name "wallet init" -CmdArgs $walletArgs
} else {
    Write-Host "Reusing existing wallet: $WalletPath"
}

$genArgs = @("run", "-p", "pwm-cli", "--bin", "pwm", "--")
if ($WalletPassphrase) {
    $genArgs += @("--wallet-passphrase", $WalletPassphrase)
}
$genArgs += @("--genesis-passphrase", $GenesisPassphrase)
$genArgs += @(
    "genesis-build",
    "--wallet", $WalletPath,
    "--out", $OutputPath,
    "--premine-bal", "$PremineRaw"
)
Invoke-CheckedCommand -Name "genesis-build" -CmdArgs $genArgs

# if (-not $SkipVerify) {
#      $verifyScript = Join-Path $PSScriptRoot "demo-genesis-verify.ps1"
#      $ExpectedPremine = $PremineRaw
#      Write-Host "==> verify premine"
#      Write-Host "    $verifyScript -GenesisPath $OutputPath -ExpectedPremineRaw $ExpectedPremine"
#      if (-not $DryRun) {
#          & $verifyScript -GenesisPath $OutputPath -ExpectedPremineRaw $ExpectedPremine
#          if ($LASTEXITCODE -ne 0) {
#              throw "Premine verification failed with exit code $LASTEXITCODE"
#          }
#      }
# }

Write-Host ""
Write-Host "Demo genesis package ready." -ForegroundColor Green
Write-Host "Output genesis: $OutputPath"
Write-Host "Premine raw: $PremineRaw (21,000,000,000 PWM at scale 1 PWM = 1,000,000 raw)"
Write-Host "Passphrase source: explicit parameter > PWM_GENESIS_PASSPHRASE > devnet fallback '12345'"
Write-Host "Wallet init defaults: deterministic demo seed/index (override via -UseCountryBruteforce or -DerivationIndex)"
