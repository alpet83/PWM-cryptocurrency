@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem Repeated single-cycle companion runs for wait_peek_ticket statistics.
rem Defaults are chosen for Windows / cmd usage and can be overridden via env vars.

set "PYTHON=C:\Python314\python.exe"
set "PROJECT_ROOT=P:\opt\docker\pwm-protocol\"
set "COMPANION=P:\opt\docker\cqds\mcp-tools\cqds_companion.py"
set "CONFIG=%PROJECT_ROOT%\.cqds\cqds_companion.toml"
set "LOG_DIR=%PROJECT_ROOT%\scripts\logs\wait_peek_stats"

P:

cd  P:\opt\docker\pwm-protocol\
set "RUNS=%CQDS_WAIT_PEEK_RUNS%"
if not defined RUNS set "RUNS=2"

set "TRACE_DELAY_MS=%CQDS_WAIT_PEEK_TRACE_DELAY_MS%"
if not defined TRACE_DELAY_MS set "TRACE_DELAY_MS=25"

set "EXIT_WAIT_MS=%CQDS_SIGNAL_EXIT_WAIT_MS%"
if not defined EXIT_WAIT_MS set "EXIT_WAIT_MS=100"

if not exist "%LOG_DIR%" mkdir "%LOG_DIR%" >nul 2>nul

set "STAMP=%RANDOM%_%RANDOM%"
set "SUMMARY=%LOG_DIR%\wait_peek_stats_%STAMP%.log"

set "CQDS_SIGNAL_FAST_EXIT=1"
set "CQDS_SIGNAL_EXIT_WAIT_MS=%EXIT_WAIT_MS%"
set "CQDS_WAIT_PEEK_TRACE_DELAY_MS=%TRACE_DELAY_MS%"
set "CQDS_WAIT_PEEK_STAGE_DELAY_MS=%TRACE_DELAY_MS%"
set "CQDS_BASIC_LOG_PARENT_PROBE=0"
set "CQDS_BASIC_LOG_PARENT_TAG=companion"
set "CQDS_BASIC_LOG_FORCE_SIMPLE=1"
set "CQDS_CONSOLE_STAGE_TRACE=1"
set "CQDS_CONSOLE_STAGE_TRACE_HF_MS=250"
set "CQDS_CONSOLE_STAGE_TRACE_RING=300"
set "CQDS_CONSOLE_STAGE_TRACE_SCOPE=10.312-10.315"
set "CQDS_CTRL_C_CANARY_ENABLE=1"
set "CQDS_CTRL_C_CANARY_FILE=ctrl-c-canary_%STAMP%.jsonl"

echo wait_peek stats runner > "%SUMMARY%"
echo python=%PYTHON% >> "%SUMMARY%"
echo companion=%COMPANION% >> "%SUMMARY%"
echo config=%CONFIG% >> "%SUMMARY%"
echo runs=%RUNS% >> "%SUMMARY%"
echo trace_delay_ms=%TRACE_DELAY_MS% >> "%SUMMARY%"
echo signal_fast_exit=1 >> "%SUMMARY%"
echo signal_exit_wait_ms=%EXIT_WAIT_MS% >> "%SUMMARY%"
echo console_stage_trace=1 >> "%SUMMARY%"
echo console_stage_trace_hf_ms=250 >> "%SUMMARY%"
echo console_stage_trace_ring=300 >> "%SUMMARY%"
echo console_stage_trace_scope=10.312-10.315 >> "%SUMMARY%"
echo ctrl_c_canary=1 >> "%SUMMARY%"
echo ctrl_c_canary_file=ctrl-c-canary_%STAMP%.jsonl >> "%SUMMARY%"
echo. >> "%SUMMARY%"
echo starting wait_peek stats runner: %SUMMARY%

set "EXIT_CODE=0"
for /l %%I in (1,1,%RUNS%) do (
    set "RUN_LOG=%LOG_DIR%\run_%%I_%STAMP%.log"
    echo [%%I/%RUNS%] start >> "%SUMMARY%"
    "%PYTHON%" -X dev "%COMPANION%" --config "%CONFIG%" --run-worker-loop --asyncio-debug --worker-loop-iterations 1 --sigint-grace-sec 0 > "!RUN_LOG!" 2>&1
    set "EXIT_CODE=!ERRORLEVEL!"
    echo "Step done, parsing results..."    
    if errorlevel 1 (
        echo [%%I/%RUNS%] sigint_marker=none>> "%SUMMARY%"
    )
    if not "!EXIT_CODE!"=="0" (
        echo failed on run %%I, see !RUN_LOG!:
        type !RUN_LOG! | bash -c 'tail -n 10'
        goto :done
    )
    echo [%%I/%RUNS%] exit=!EXIT_CODE! log=!RUN_LOG!>> "%SUMMARY%"
    findstr /I /C:"sigint_stage" /C:"sigint_origin" "!RUN_LOG!" >> "%SUMMARY%"    
)

:done
echo summary=%SUMMARY%
echo last_exit=%EXIT_CODE%
exit /b %EXIT_CODE%
