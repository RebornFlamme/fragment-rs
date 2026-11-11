# Télécharge le binaire pdfium ÉPINGLÉ (bblanchon/pdfium-binaries) et vérifie son sha256.
#   powershell -File scripts/pdfium.ps1 [-Dest C:/dev/fragment-rs-pdfium-bin]
# Écrit le dossier de la bibliothèque dans la sortie standard, et PDFIUM_LIB_DIR dans $GITHUB_ENV en CI.
# ★ Même version et mêmes empreintes que scripts/pdfium.sh : les changer ensemble, avec la feature
#   `pdfium_XXXX` de pdfium-render (crates/fragment-pdf/Cargo.toml). Aucun binaire n'entre dans le dépôt.
param([string]$Dest = 'C:/dev/fragment-rs-pdfium-bin')
$ErrorActionPreference = 'Stop'

$Version = 'chromium/8125'
$Sha256 = '9a964261760cff52570aabf038ab346d3a5034dd8b30d0d5fe8861235d356b8b' # pdfium-win-x64.tgz
$Url = "https://github.com/bblanchon/pdfium-binaries/releases/download/$Version/pdfium-win-x64.tgz"

$Cible = Join-Path $Dest ($Version -replace '/', '-')
$Dll = Join-Path $Cible 'bin/pdfium.dll'
if (-not (Test-Path $Dll)) {
    New-Item -ItemType Directory -Force $Cible | Out-Null
    $Archive = Join-Path $Cible 'pdfium-win-x64.tgz'
    Invoke-WebRequest -Uri $Url -OutFile $Archive -UseBasicParsing
    $Obtenu = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Obtenu -ne $Sha256) { Remove-Item $Archive; throw "sha256 inattendu pour $Url : $Obtenu" }
    & "$env:SystemRoot/System32/tar.exe" -xzf $Archive -C $Cible
    if ($LASTEXITCODE -ne 0) { throw 'extraction impossible' }
    Remove-Item $Archive
}
$Lib = Join-Path $Cible 'bin'
if ($env:GITHUB_ENV) { [IO.File]::AppendAllText($env:GITHUB_ENV, "PDFIUM_LIB_DIR=$Lib`n", (New-Object Text.UTF8Encoding($false))) }
Write-Output $Lib
