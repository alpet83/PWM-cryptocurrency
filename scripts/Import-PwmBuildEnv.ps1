# Mirror repo-root .build.env for native PowerShell launchers (CY cluster, etc.).
# Prepends MSYS2 UCRT64/MINGW64 and ~/.cargo/bin to PATH when tools are missing;
# sets RUSTFLAGS -Cdlltool=... when rustc cannot find dlltool.exe by name.
# Encoding: UTF-8 with BOM (PowerShell 5.1).

function Test-PathInPathEnv {
    param([string]$Dir)
    if (-not $Dir -or -not (Test-Path -LiteralPath $Dir)) { return $false }
    $norm = [System.IO.Path]::GetFullPath($Dir).TrimEnd('\')
    foreach ($part in ($env:PATH -split ';')) {
        if (-not $part) { continue }
        try {
            $p = [System.IO.Path]::GetFullPath($part).TrimEnd('\')
            if ($p -eq $norm) { return $true }
        }
        catch { }
    }
    return $false
}

function Add-PathIfMissing {
    param([string]$Dir)
    if (-not $Dir -or -not (Test-Path -LiteralPath $Dir)) { return }
    if (-not (Test-PathInPathEnv $Dir)) {
        $env:PATH = "$Dir;$env:PATH"
    }
}

function Initialize-PwmBuildEnv {
    param(
        [string]$RepoRoot = ''
    )
    if (-not $RepoRoot) {
        $RepoRoot = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }
    }

    $needDlltool = -not (Get-Command dlltool -ErrorAction SilentlyContinue)
    $needCargo = -not (Get-Command cargo -ErrorAction SilentlyContinue)
    $needRustDlltoolFlag = -not ($env:RUSTFLAGS -match '(?:^|\s)-Cdlltool=')

    if (-not $needDlltool -and -not $needCargo -and -not $needRustDlltoolFlag) {
        return
    }

    $msysRoot = if ($env:MSYS2_ROOT) { $env:MSYS2_ROOT } else { 'C:\msys64' }
  # Same dirs as .build.env: PATH="/ucrt64/bin:/mingw64/bin:$PATH"
    $ucrtBin = Join-Path $msysRoot 'ucrt64\bin'
    $mingwBin = Join-Path $msysRoot 'mingw64\bin'
    $dlltoolExe = Join-Path $ucrtBin 'dlltool.exe'
    $gccExe = Join-Path $ucrtBin 'gcc.exe'

    if ($needDlltool -or $needCargo) {
        Add-PathIfMissing $ucrtBin
        Add-PathIfMissing $mingwBin
        if ($env:USERPROFILE) {
            Add-PathIfMissing (Join-Path $env:USERPROFILE '.cargo\bin')
        }
    }

    if ($needRustDlltoolFlag -and (Test-Path -LiteralPath $dlltoolExe)) {
        $flag = "-Cdlltool=$dlltoolExe"
        if ($env:RUSTFLAGS) {
            $env:RUSTFLAGS = "$flag $($env:RUSTFLAGS)"
        }
        else {
            $env:RUSTFLAGS = $flag
        }
    }

    if (-not $env:CC_x86_64_pc_windows_gnu -and (Test-Path -LiteralPath $gccExe)) {
        $env:CC_x86_64_pc_windows_gnu = $gccExe
    }
}
