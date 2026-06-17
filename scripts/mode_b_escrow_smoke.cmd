@echo off
setlocal EnableExtensions

set "PROJECT_ROOT=%~dp0.."
pushd "%PROJECT_ROOT%" >nul

cmd /c .\build_project.cmd test -p pwm-core escrow_
set "EXIT_CODE=%ERRORLEVEL%"

popd >nul
exit /b %EXIT_CODE%
