@echo off
setlocal EnableExtensions

set "PROJECT_ROOT=%~dp0"
if "%PROJECT_ROOT:~-1%"=="\" set "PROJECT_ROOT=%PROJECT_ROOT:~0,-1%"

set "MSYS_BASH=C:\msys64\usr\bin\bash.exe"
if not exist "%MSYS_BASH%" (
  echo [test_project] MSYS2 bash not found: %MSYS_BASH%
  exit /b 1
)

if not defined HOME if defined USERPROFILE set "HOME=%USERPROFILE%"
set "MSYSTEM=UCRT64"
set "CHERE_INVOKING=1"

pushd "%PROJECT_ROOT%" >nul
"%MSYS_BASH%" -lc "bash ./scripts/test_project.sh ""$@""" -- %*
set "EXIT_CODE=%ERRORLEVEL%"
popd >nul
exit /b %EXIT_CODE%
