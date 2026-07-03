# build_project.ps1 -- PowerShell wrapper for cargo with MSYS2/UCRT64 toolchain.
# Sets the same environment as .build.env so dlltool and gcc are found.
# Usage:
#   powershell -NonInteractive -File build_project.ps1                        # cargo build --workspace
#   powershell -NonInteractive -File build_project.ps1 build -p pwm-tui
#   powershell -NonInteractive -File build_project.ps1 check -p pwmd
#   powershell -NonInteractive -File build_project.ps1 clippy -p pwm-tui -- -D warnings
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ProjectRoot = $PSScriptRoot
$Msys64     = "C:\msys64"
$Ucrt64Bin  = "$Msys64\ucrt64\bin"
$Mingw64Bin = "$Msys64\mingw64\bin"

# -- PATH: prepend ucrt64/bin and mingw64/bin (dlltool, gcc, etc.) --
$env:PATH = "$Ucrt64Bin;$Mingw64Bin;$env:PATH"

# -- Cargo bin (Rust toolchain) --
if ($env:USERPROFILE -and (Test-Path "$env:USERPROFILE\.cargo\bin")) {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
}

# -- Toolchain env vars (mirror .build.env) --
$env:CC                          = "$Ucrt64Bin\gcc.exe"
$env:CC_x86_64_pc_windows_gnu    = "$Ucrt64Bin\gcc.exe"
$env:LIBRARY_PATH                = "$Msys64\ucrt64\lib"

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
if (-not (Test-Path "$Ucrt64Bin\dlltool.exe")) {
    Write-Error "[build_project] dlltool not found: $Ucrt64Bin\dlltool.exe -- install msys2 ucrt64 binutils"
    exit 1
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "[build_project] cargo not found in PATH"
    exit 1
}

Write-Host "[build_project] CARGO_TARGET_DIR: $env:CARGO_TARGET_DIR"
Write-Host "[build_project] RUSTFLAGS: $env:RUSTFLAGS"

Push-Location $ProjectRoot
try {
    if ($CargoArgs.Count -eq 0) {
        Write-Host "[build_project] running: cargo build --workspace"
        cargo build --workspace
    } else {
        Write-Host "[build_project] running: cargo $CargoArgs"
        cargo @CargoArgs
    }
} finally {
    Pop-Location
}
