@echo off
title Deepgram Dictation - Troubleshoot
cd /d "%~dp0"
echo Running with the console visible so you can see any errors.
echo Close this window or press CTRL+C to stop.
echo.

powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name like 'python%%'\" | Where-Object { $_.CommandLine -like '*dictate.py*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }" >nul 2>&1

set "PY="
py -3 --version >nul 2>&1
if not errorlevel 1 set "PY=py -3"
if not defined PY set "PY=python"

%PY% dictate.py
echo.
pause
