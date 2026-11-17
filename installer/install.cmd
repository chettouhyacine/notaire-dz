@echo off
setlocal EnableExtensions
rem NotaireDZ 1.0.1 — per-user install, no administrator rights required.
rem Installs to %LOCALAPPDATA%\Programs\NotaireDZ and creates Desktop + Start Menu shortcuts.

set "SRC=%~dp0"
set "TARGET=%LOCALAPPDATA%\Programs\NotaireDZ"
if not "%NOTAIRE_TARGET%"=="" set "TARGET=%NOTAIRE_TARGET%"

echo.
echo  NotaireDZ 1.0.1
echo  Installation dans : "%TARGET%"
echo.

if not exist "%TARGET%" mkdir "%TARGET%" >nul 2>&1

for %%F in (NotaireDZ.exe NotaireDZ.ico NotaireDZ-Manuel.pdf uninstall.cmd shortcuts.ps1) do (
  if exist "%SRC%%%F" copy /y "%SRC%%%F" "%TARGET%\" >nul
)

if not "%NOTAIRE_NO_SHORTCUTS%"=="1" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%SRC%shortcuts.ps1" -Target "%TARGET%"
)

reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v DisplayName /t REG_SZ /d "NotaireDZ 1.0.1" /f >nul
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v DisplayVersion /t REG_SZ /d "1.0.1" /f >nul
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v Publisher /t REG_SZ /d "NotaireDZ" /f >nul
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v DisplayIcon /t REG_SZ /d "%TARGET%\NotaireDZ.ico" /f >nul
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v InstallLocation /t REG_SZ /d "%TARGET%" /f >nul
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v UninstallString /t REG_SZ /d "\"%TARGET%\uninstall.cmd\"" /f >nul
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v NoModify /t REG_DWORD /d 1 /f >nul
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /v NoRepair /t REG_DWORD /d 1 /f >nul

echo.
echo  Installation terminee.
echo  Raccourci "NotaireDZ" sur le Bureau et dans le menu Demarrer.
echo  Les donnees de l'etude restent dans "%LOCALAPPDATA%\NotaireDZ".
echo.
if "%NOTAIRE_QUIET%"=="1" exit /b 0
pause
