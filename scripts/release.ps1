<#
  Publie une nouvelle version du launcher ou d'un jeu (équivalent du scripts/release.sh de Ronnie).

  Usage (depuis n'importe où) :
      .\scripts\release.ps1 winton 1.1.0
      .\scripts\release.ps1 la-taverne 1.2.0

  Avant : écris les nouveautés dans le CHANGELOG.md du composant, sous un titre « ## 1.2.0 ».

  Le script :
    1. vérifie la version, la branche et le CHANGELOG ;
    2. met la version dans Cargo.toml (launcher) ou package.json (jeux) ;
    3. crée le commit et le tag <composant>-vX.Y.Z ;
    4. demande confirmation, puis pousse sur GitHub.
  La GitHub Action « Release » construit, signe et publie : les launchers proposent alors la mise à jour.
#>
param(
    [Parameter(Mandatory = $true)][string]$Composant,
    [Parameter(Mandatory = $true)][string]$Version
)

function Stop-Release([string]$message) {
    Write-Host "Erreur : $message" -ForegroundColor Red
    exit 1
}

function Write-Utf8([string]$path, [string]$text) {
    [IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding $false))
}

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

if ($Version -notmatch '^\d+\.\d+\.\d+$') { Stop-Release "la version doit être au format X.Y.Z (ex. 1.2.0)" }
if ($Composant -eq 'launcher') { $Composant = 'winton' }  # ancien nom
$dir = if ($Composant -eq 'winton') { 'launcher' } else { "jeux\$Composant" }
if (-not (Test-Path $dir)) { Stop-Release "le dossier $dir n'existe pas" }
if ((git branch --show-current) -ne 'main') { Stop-Release "il faut être sur la branche main" }
$tag = "$Composant-v$Version"
if (git tag -l $tag) { Stop-Release "la version $tag existe déjà" }

$changelog = Join-Path $dir 'CHANGELOG.md'
if (-not (Test-Path $changelog) -or -not (Select-String -Path $changelog -Pattern "^## $([regex]::Escape($Version))(\s|$)" -Quiet)) {
    Stop-Release "ajoute d'abord une section « ## $Version » dans $changelog"
}

$others = git status --porcelain | Where-Object { $_.Substring(3) -notlike "$($dir.Replace('\','/'))/*" }
if ($others) {
    Write-Host "Attention : ces fichiers modifiés hors de $dir ne feront PAS partie de la version :" -ForegroundColor Yellow
    $others | ForEach-Object { Write-Host "   $_" }
}

# ---------------------------------------------------------------- numéro de version
$versionRegex = [regex]'"version":\s*"[^"]*"'
if ($Composant -eq 'winton') {
    $cargo = Join-Path $dir 'Cargo.toml'
    $text = [IO.File]::ReadAllText($cargo)
    Write-Utf8 $cargo ([regex]::new('(?m)^version = "[^"]*"').Replace($text, "version = `"$Version`"", 1))
    # Met à jour Cargo.lock avec le nouveau numéro
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    Push-Location $dir
    cargo update --workspace --offline 2>$null
    Pop-Location
} else {
    $pkg = Join-Path $dir 'package.json'
    Write-Utf8 $pkg ($versionRegex.Replace([IO.File]::ReadAllText($pkg), "`"version`": `"$Version`"", 1))
    $lock = Join-Path $dir 'package-lock.json'
    if (Test-Path $lock) {
        # Les deux premières occurrences sont la version du projet lui-même.
        Write-Utf8 $lock ($versionRegex.Replace([IO.File]::ReadAllText($lock), "`"version`": `"$Version`"", 2))
    }
}

# ---------------------------------------------------------------- commit + tag
git add -- $dir
git commit -m "$Composant $Version"
if ($LASTEXITCODE -ne 0) { Stop-Release "le commit a échoué" }
git tag -a $tag -m "$Composant $Version"
Write-Host ""
Write-Host "Commit et tag $tag créés en local." -ForegroundColor Green

$answer = Read-Host "Publier maintenant sur GitHub ? (o/N)"
if ($answer -match '^[oOyY]') {
    git push origin main $tag
    Write-Host "Envoyé. Suivi de la construction : https://github.com/GoldeneGame/Winton/actions" -ForegroundColor Green
} else {
    Write-Host "Rien n'a été envoyé. Pour publier plus tard :  git push origin main $tag"
}
