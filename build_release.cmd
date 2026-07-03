@echo off
rem build_release.cmd — MSVC x64 release build for pwmd.
rem
rem Produces PDB debug symbols readable by samply (Firefox Profiler).
rem Use this instead of build_project.sh when profiling with samply.
rem
rem Usage:
rem   build_release.cmd                   — release build
rem   build_release.cmd flamegraph        — flamegraph profile (release + debug symbols)
rem   build_release.cmd check             — cargo check only
rem
rem Output: F:\pwm-test\shared\<profile>\pwmd.exe + pwmd.pdb
rem Then run:  samply record -- F:\pwm-test\shared\flamegraph\pwmd.exe [args]

setlocal

set VCVARS="C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat"
if not exist %VCVARS% (
    echo [build_release] ERROR: vcvarsall.bat not found at %VCVARS%
    echo [build_release] Install Visual Studio 2022 with C++ workload.
    exit /b 1
)

rem Activate MSVC x64 environment
call %VCVARS% x64 >nul 2>&1
if errorlevel 1 (
    echo [build_release] ERROR: vcvarsall.bat failed.
    exit /b 1
)

rem Use F:\pwm-test\shared so MSVC artifacts don't fill the project disk
set CARGO_TARGET_DIR=F:\pwm-test\shared
set RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc

rem Parse argument
set PROFILE=release
set CARGO_SUBCMD=build
if /i "%1"=="flamegraph"  set PROFILE=flamegraph
if /i "%1"=="check"       set CARGO_SUBCMD=check & set PROFILE=release
if /i "%1"=="flamegraph"  set CARGO_SUBCMD=build

echo [build_release] toolchain=%RUSTUP_TOOLCHAIN% profile=%PROFILE%
echo [build_release] target-dir=%CARGO_TARGET_DIR%
echo.

if "%CARGO_SUBCMD%"=="check" (
    cargo +stable-x86_64-pc-windows-msvc check -p pwmd
) else if "%PROFILE%"=="release" (
    cargo +stable-x86_64-pc-windows-msvc build --release -p pwmd
    echo.
    echo [build_release] Done: %CARGO_TARGET_DIR%\release\pwmd.exe
) else (
    cargo +stable-x86_64-pc-windows-msvc build --profile %PROFILE% -p pwmd
    echo.
    echo [build_release] Done: %CARGO_TARGET_DIR%\%PROFILE%\pwmd.exe
    echo [build_release] PDB:  %CARGO_TARGET_DIR%\%PROFILE%\pwmd.pdb
    echo.
    echo [build_release] To profile:
    echo   samply record -- %CARGO_TARGET_DIR%\%PROFILE%\pwmd.exe [node args]
)

endlocal
