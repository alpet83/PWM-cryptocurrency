param(
    [string]$GenesisPath = "tmp/genesis-custom.json",
    [UInt64]$ExpectedPremineRaw = 42000000000000000
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot/..

if (-not (Test-Path -LiteralPath $GenesisPath)) {
    Write-Error "Genesis file not found: $GenesisPath"
}

$json = Get-Content -LiteralPath $GenesisPath -Raw | ConvertFrom-Json
$rows = @($json.gen_cfg.funding.accounts)
if ($rows.Count -eq 0) {
    Write-Error "gen_cfg.funding.accounts is empty in $GenesisPath"
}

$sum = [System.Numerics.BigInteger]::Zero
foreach ($row in $rows) {
    if ($null -eq $row.bal -or [string]::IsNullOrWhiteSpace([string]$row.bal)) {
        Write-Error "Found funding row with empty bal in $GenesisPath"
    }
    $sum += [System.Numerics.BigInteger]::Parse([string]$row.bal)
}

$expected = [System.Numerics.BigInteger]::Parse([string]$ExpectedPremineRaw)
if ($sum -ne $expected) {
    Write-Host "Premine mismatch." -ForegroundColor Red
    Write-Host "Expected raw: $expected"
    Write-Host "Actual raw:   $sum"
    exit 1
}

Write-Host "Premine verified: $sum raw (target: $expected)." -ForegroundColor Green
exit 0
