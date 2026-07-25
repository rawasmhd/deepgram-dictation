@echo off
title Deepgram Dictation - Setup
cd /d "%~dp0"

echo.
echo   ============================================
echo     Deepgram Dictation  -  one-time setup
echo   ============================================
echo.

rem ---- 1. locate Python -------------------------------------------------
set "PY="
py -3 --version >nul 2>&1
if not errorlevel 1 set "PY=py -3"
if defined PY goto haspython

python --version >nul 2>&1
if not errorlevel 1 set "PY=python"
if defined PY goto haspython

echo   Python is not installed, or is not on your PATH.
echo.
echo   Get it from  https://www.python.org/downloads/
echo   IMPORTANT: tick "Add python.exe to PATH" on the first
echo   screen of the installer, then run this setup again.
echo.
pause
exit /b 1

:haspython
for /f "delims=" %%v in ('%PY% --version 2^>^&1') do set "PYVER=%%v"
echo   Found %PYVER%

rem the windowless interpreter, so no black box sits on your taskbar
set "PYW="
for /f "usebackq delims=" %%p in (`%PY% -c "import sys,os;print(os.path.join(os.path.dirname(sys.executable),'pythonw.exe'))"`) do set "PYW=%%p"
if not exist "%PYW%" for /f "usebackq delims=" %%p in (`%PY% -c "import sys;print(sys.executable)"`) do set "PYW=%%p"
echo   Runtime: %PYW%
echo.

rem ---- 2. packages ------------------------------------------------------
echo   Installing packages, this takes a minute...
%PY% -m pip install --quiet --upgrade pip
%PY% -m pip install --quiet sounddevice numpy requests pynput pyperclip
if errorlevel 1 goto pipfailed
echo   Packages installed.
echo.
goto askkey

:pipfailed
echo.
echo   Package installation failed. Try again with a right-click
echo   on setup.bat and "Run as administrator".
echo.
pause
exit /b 1

rem ---- 3. API key -------------------------------------------------------
:askkey
if not exist ".env" goto enterkey
set "REPLACE="
set /p "REPLACE=  An API key is already saved. Replace it? (y/N): "
if /i "%REPLACE%"=="y" goto enterkey
echo.
goto autostart

:enterkey
echo   Paste your Deepgram API key below.
echo   (Get one at https://console.deepgram.com)
echo.
set "DGKEY="
set /p "DGKEY=  API key: "
if "%DGKEY%"=="" goto nokey
> ".env" echo DEEPGRAM_API_KEY=%DGKEY%
echo.
echo   Key saved.
echo.
goto autostart

:nokey
echo.
echo   Nothing entered - run setup.bat again once you have a key.
echo.
pause
exit /b 1

rem ---- 4. autostart -----------------------------------------------------
:autostart
set "WD=%~dp0"
set "TARGET=%PYW%"
powershell -NoProfile -Command "$p=Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Startup\Deepgram Dictation.lnk'; $s=(New-Object -ComObject WScript.Shell).CreateShortcut($p); $s.TargetPath=$env:TARGET; $s.Arguments='dictate.py'; $s.WorkingDirectory=$env:WD; $s.Description='Alt+M dictation'; $s.Save()" >nul 2>&1
if errorlevel 1 goto nostartup
echo   Autostart enabled - it will run from every login onwards.
goto launch

:nostartup
echo   Could not register autostart. You can still launch it with
echo   "Start Dictation.bat".

rem ---- 5. start it now --------------------------------------------------
:launch
start "" "%PYW%" dictate.py
echo   Started.
echo.
echo   ============================================
echo     Ready. Press ALT+M anywhere to dictate.
echo     Press it again to send the text.
echo.
echo     Quit any time with CTRL+ALT+Q
echo   ============================================
echo.
echo   You can close this window.
pause
