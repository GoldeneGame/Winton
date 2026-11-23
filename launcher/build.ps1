# Compile Winton et copie Winton.exe à la racine du projet (..\Winton.exe)
# Usage : clic droit > Exécuter avec PowerShell, ou  .\build.ps1  depuis ce dossier.
Set-Location $PSScriptRoot

# Outils Rust + MinGW (dlltool) dans le PATH de cette session
$mingw = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Directory -Filter "BrechtSanders.WinLibs*" -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName "mingw64\bin" } | Where-Object { Test-Path $_ } | Select-Object -First 1
$env:PATH = "$env:USERPROFILE\.cargo\bin;$mingw;$env:PATH"

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "La compilation a échoué." }

Copy-Item target\release\winton.exe ..\Winton.exe -Force
Write-Host "OK -> $((Resolve-Path ..\Winton.exe).Path)"
