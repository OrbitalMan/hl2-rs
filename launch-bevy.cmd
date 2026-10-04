@echo off
setlocal
cd /d "%~dp0"
if not exist "bin\hl2-bevy.exe" (
  echo Build with scripts\build-bevy.ps1 first.
  exit /b 1
)
"bin\hl2-bevy.exe" %*
set "bevyExitCode=%errorlevel%"
if not "%bevyExitCode%"=="0" pause
exit /b %bevyExitCode%
