@echo off
setlocal
cd /d "%~dp0"
if exist "bin\hl2-rs.exe" (
  "bin\hl2-rs.exe" view %*
  goto done
)
if exist "target\debug\hl2-rs.exe" (
  "target\debug\hl2-rs.exe" view %*
  goto done
)
echo Build with scripts\build.ps1 first.
exit /b 1
:done
if errorlevel 1 pause
