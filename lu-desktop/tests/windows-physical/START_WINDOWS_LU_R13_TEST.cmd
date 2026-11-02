@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Run-LuR13-WindowsAcceptance.ps1"
echo.
echo Physical Windows test finished. The results are stored under LocalAppData\LuAcceptance.
pause
