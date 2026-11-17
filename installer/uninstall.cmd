@echo off
setlocal EnableExtensions
set "TARGET=%~dp0"
if "%TARGET:~-1%"=="\" set "TARGET=%TARGET:~0,-1%"

echo Removing NotaireDZ from "%TARGET%" ...
del "%USERPROFILE%\Desktop\NotaireDZ.lnk" >nul 2>&1
del "%USERPROFILE%\Desktop\NotaireDZ (demo).lnk" >nul 2>&1
del "%APPDATA%\Microsoft\Windows\Start Menu\Programs\NotaireDZ\NotaireDZ.lnk" >nul 2>&1
del "%APPDATA%\Microsoft\Windows\Start Menu\Programs\NotaireDZ\NotaireDZ (demo).lnk" >nul 2>&1
rd "%APPDATA%\Microsoft\Windows\Start Menu\Programs\NotaireDZ" >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NotaireDZ" /f >nul 2>&1

echo.
echo  Programme desinstalle.
echo.
echo  ATTENTION : les donnees de l'etude sont conservees dans
echo      "%LOCALAPPDATA%\NotaireDZ"
echo  et les donnees de demonstration dans
echo      "%LOCALAPPDATA%\NotaireDZ-demo"
echo  Supprimez ces dossiers vous-meme si vous en etes certain.
echo.

rem The running script cannot delete its own folder; hand that to a detached shell.
start "" /min cmd /c "timeout /t 2 >nul & rd /s /q ""%TARGET%"""
if "%NOTAIRE_QUIET%"=="1" exit /b 0
pause
