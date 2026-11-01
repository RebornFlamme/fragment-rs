<#
.SYNOPSIS
  Régénère la référence pixel : joue les scènes pixels/scenes/*.spec.ts sur Fragment
  (harnais e2e, Playwright de Fragment/app/node_modules) et écrit pixels/reference/*.png.
.PARAMETER Scene
  Optionnel : nom d'une scène (ex. "menu"). Sans lui, toutes les scènes.
.NOTES
  Compatible Windows PowerShell 5.1. Fragment est en LECTURE SEULE : on l'exécute, on n'y écrit pas
  (sorties dans pixels/.out ; Electron et le coffre vont dans %TEMP%). Si le serveur Vite (5123)
  ne tourne pas, la config le lance (cache Vite dans Fragment/app/node_modules/.vite).
#>
param([string]$Scene = '')

$ErrorActionPreference = 'Stop'
$racine   = Split-Path -Parent $PSScriptRoot
$fragment = Join-Path (Split-Path -Parent $racine) 'Fragment\app'
$config   = Join-Path $racine 'pixels\playwright.config.ts'
$pw       = Join-Path $fragment 'node_modules\.bin\playwright.cmd'
$ref      = Join-Path $racine 'pixels\reference'

if (-not (Test-Path $pw))                              { throw "Playwright introuvable : $pw (npm install fait dans Fragment/app ?)" }
if (-not (Test-Path (Join-Path $fragment 'dist-electron\main.js'))) { throw "Build Electron manquant : $fragment\dist-electron\main.js" }
# ★ Lecture seule stricte : sans serveur Vite déjà lancé, la config le démarrerait et Vite écrirait
# son cache dans Fragment/app/node_modules/.vite. On refuse plutôt (décision du chef, 01/11).
$vite = Test-NetConnection -ComputerName localhost -Port 5123 -InformationLevel Quiet -WarningAction SilentlyContinue
if (-not $vite) { throw "Serveur Vite de Fragment absent (port 5123) : l'utilisateur lance « npm run dev:react » dans Fragment\app, puis relance ce script." }
New-Item -ItemType Directory -Force -Path $ref | Out-Null

$avant = Get-Date
$args2 = @('test', '--config', $config)
if ($Scene -ne '') { $args2 += ('[\/]' + [regex]::Escape($Scene) + '\.spec\.ts$') }

Push-Location $fragment   # le harnais fait « electron . » et lit e2e/fixtures/ en relatif
try { & $pw @args2; $code = $LASTEXITCODE } finally { Pop-Location }

Add-Type -AssemblyName System.Drawing
$pngs = Get-ChildItem $ref -Filter *.png | Where-Object { $_.LastWriteTime -ge $avant } | Sort-Object Name
Write-Host ''
Write-Host ("{0} PNG écrit(s) dans {1}" -f @($pngs).Count, $ref)
foreach ($f in $pngs) {
    $img = [System.Drawing.Image]::FromFile($f.FullName)
    try { Write-Host ("  {0,-48} {1}x{2}  {3} Ko" -f $f.Name, $img.Width, $img.Height, [int]($f.Length / 1KB)) } finally { $img.Dispose() }
}
exit $code
