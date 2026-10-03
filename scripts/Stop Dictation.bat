@echo off
title Stop Deepgram Dictation
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name like 'python%%'\" | Where-Object { $_.CommandLine -like '*dictate.py*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }"
rem the Rust version (dictation.exe), if it runs
taskkill /IM dictation.exe /F >nul 2>&1
echo Dictation stopped.
timeout /t 2 >nul
