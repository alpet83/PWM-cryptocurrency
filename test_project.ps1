# test_project.ps1 -- PowerShell wrapper for cargo test with MSYS2/UCRT64 toolchain.
# Sets the same environment as .build.env so dlltool and gcc are found.
# Usage:
#   powershell -NonInteractive -File test_project.ps1 -p pwm-tui --lib
#   powershell -NonInteractive -File test_project.ps1 -p pwmd --features clickhouse-snapshot --lib
#   powershell -NonInteractive -File test_project.ps1          # default: cargo test --workspace
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ProjectRoot = $PSScriptRoot
$Msys64 = "C:\msys64"
$Ucrt64Bin = "$Msys64\ucrt64\bin"
$Mingw64Bin = "$Msys64\mingw64\bin"

# -- PATH: prepend ucrt64/bin and mingw64/bin (dlltool, gcc, etc.) --
$env:PATH = "$Ucrt64Bin;$Mingw64Bin;$env:PATH"

# -- Cargo bin (Rust toolchain) --
if ($env:USERPROFILE -and (Test-Path "$env:USERPROFILE\.cargo\bin")) {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
}

# -- Toolchain env vars (mirror .build.env) --
$env:CC = "$Ucrt64Bin\gcc.exe"
$env:CC_x86_64_pc_windows_gnu = "$Ucrt64Bin\gcc.exe"
$env:LIBRARY_PATH = "$Ucrt64Bin\..\lib;$env:PATH"  # approx; linker finds it via PATH

# -- RUSTFLAGS: inject dlltool path, avoid duplicates --
$DlltoolFlag = "-Cdlltool=$Ucrt64Bin\dlltool.exe"
if (-not ($env:RUSTFLAGS -like "*$DlltoolFlag*")) {
    $env:RUSTFLAGS = if ($env:RUSTFLAGS) { "$DlltoolFlag $env:RUSTFLAGS" } else { $DlltoolFlag }
}

# -- CARGO_TARGET_DIR --
if (-not $env:CARGO_TARGET_DIR) {
    $env:CARGO_TARGET_DIR = "$ProjectRoot\target-codex"
}
$TmpDir = "$env:CARGO_TARGET_DIR\tmp"
New-Item -ItemType Directory -Force $TmpDir | Out-Null
$env:TMPDIR = $TmpDir
$env:TMP    = $TmpDir
$env:TEMP   = $TmpDir

# -- Sanity checks --
$DlltoolExe = "$Ucrt64Bin\dlltool.exe"
if (-not (Test-Path $DlltoolExe)) {
    Write-Error "[test_project] dlltool not found: $DlltoolExe -- install msys2 ucrt64 binutils"
    exit 1
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "[test_project] cargo not found in PATH"
    exit 1
}

Write-Host "[test_project] dlltool: $DlltoolExe"
Write-Host "[test_project] CARGO_TARGET_DIR: $env:CARGO_TARGET_DIR"
Write-Host "[test_project] RUSTFLAGS: $env:RUSTFLAGS"

Push-Location $ProjectRoot
try {
    if ($CargoArgs.Count -eq 0) {
        Write-Host "[test_project] running: cargo test --workspace"
        cargo test --workspace
    } else {
        Write-Host "[test_project] running: cargo test $CargoArgs"
        cargo test @CargoArgs
    }
} finally {
    Pop-Location
}
