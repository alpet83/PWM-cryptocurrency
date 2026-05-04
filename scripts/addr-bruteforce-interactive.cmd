@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "SH_SCRIPT=%SCRIPT_DIR%addr-bruteforce-interactive.sh"

if not exist "%SH_SCRIPT%" (
  echo [error] Script not found: "%SH_SCRIPT%"
  exit /b 1
)

set "BASH_EXE="
if exist "%ProgramFiles%\Git\bin\bash.exe" set "BASH_EXE=%ProgramFiles%\Git\bin\bash.exe"
if not defined BASH_EXE if exist "%ProgramFiles(x86)%\Git\bin\bash.exe" set "BASH_EXE=%ProgramFiles(x86)%\Git\bin\bash.exe"
if not defined BASH_EXE if exist "C:\Apps\Git\bin\bash.exe" set "BASH_EXE=C:\Apps\Git\bin\bash.exe"

if not defined BASH_EXE (
  for /f "delims=" %%B in ('where bash 2^>nul') do (
    echo %%B | findstr /i "\\Git\\bin\\bash.exe" >nul
    if not errorlevel 1 (
      set "BASH_EXE=%%B"
      goto :bash_found
    )
  )
)

:bash_found
if not defined BASH_EXE (
  where bash >nul 2>nul
  if not errorlevel 1 (
    set "BASH_EXE=bash"
  )
)

if not defined BASH_EXE (
  echo [error] bash is not found. Install Git Bash or add bash to PATH.
  exit /b 1
)

"%BASH_EXE%" "%SH_SCRIPT%" %*
exit /b %errorlevel%
