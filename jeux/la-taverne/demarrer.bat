@echo off
chcp 65001 >nul
title La Taverne - serveur JDR
cd /d "%~dp0"

rem --- Trouve Node.js (systeme, sinon celui installe par Ronnie)
where node >nul 2>nul
if errorlevel 1 (
  for /d %%D in ("%APPDATA%\ronnie\config\node\versions\v*") do set "NODEDIR=%%D"
)
if defined NODEDIR set "PATH=%NODEDIR%;%PATH%"
where node >nul 2>nul
if errorlevel 1 (
  echo Node.js est introuvable. Installe-le depuis https://nodejs.org puis relance.
  pause
  exit /b 1
)

if not exist node_modules (
  echo Installation des dependances...
  call npm install --omit=dev
)

node server.js
pause
