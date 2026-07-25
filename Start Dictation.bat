@echo off
rem Launch dictation silently. Normally not needed - setup.bat registers
rem this to run at login - but useful after CTRL+ALT+Q.
cd /d "%~dp0"

set "PY="
py -3 --version >nul 2>&1
if not errorlevel 1 set "PY=py -3"
if not defined PY set "PY=python"

set "PYW="
for /f "usebackq delims=" %%p in (`%PY% -c "import sys,os;print(os.path.join(os.path.dirname(sys.executable),'pythonw.exe'))"`) do set "PYW=%%p"
if not exist "%PYW%" set "PYW=pythonw"

start "" "%PYW%" dictate.py
exit
