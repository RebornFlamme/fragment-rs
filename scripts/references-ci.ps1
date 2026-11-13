<#
.SYNOPSIS
  Capture des références pixels sur GitHub Actions (fr-e9q), appelée par .github/workflows/references.yml.
.DESCRIPTION
  Fragment est déjà cloné (SHA épinglé) dans <Capture>/Fragment par le workflow. Ce script :
    1. copie pixels/ de fragment-rs dans <Capture>/fragment-rs/pixels : les scènes importent
       '../../../Fragment/app/…', il faut donc que Fragment soit le VOISIN de fragment-rs ;
    2. installe Fragment (npm ci) et prépare ce que le harnais e2e attend (pdfjs, cli, main Electron) ;
    3. lance la Vite enveloppe (port 5123), joue les scènes avec le Playwright de Fragment ;
    4. recopie les PNG capturés dans <Depot>/pixels/reference et écrit PROVENANCE.json.
  ★ Dépôt PUBLIC, Fragment PRIVÉ : aucune sortie de npm, tsc, Vite ou Playwright n'est imprimée telle
  quelle (une trace d'erreur peut citer une ligne de source de Fragment). Tout va dans <Capture>/journaux,
  jamais affiché, supprimé en fin de job. On n'imprime que des codes de sortie, des titres de scène
  (écrits dans fragment-rs) et la PREMIÈRE ligne des erreurs, tronquée.
.PARAMETER Capture
  Dossier de travail hors de l'espace du runner ($RUNNER_TEMP/capture), contenant déjà Fragment/.
.PARAMETER Depot
  Racine de fragment-rs (l'espace de travail du runner).
.PARAMETER Scenes
  Scènes séparées par des virgules (nom du .spec.ts sans extension). Vide = toutes.
#>
param(
    [Parameter(Mandatory = $true)][string]$Capture,
    [Parameter(Mandatory = $true)][string]$Depot,
    [string]$Scenes = ''
)

$ErrorActionPreference = 'Stop'
$fragment = Join-Path $Capture 'Fragment'
$app      = Join-Path $fragment 'app'
$rs       = Join-Path $Capture 'fragment-rs'
$pixels   = Join-Path $rs 'pixels'
$journaux = Join-Path $Capture 'journaux'
$refsMain = Join-Path $Capture 'refs-main'
$refsCi   = Join-Path $Capture 'refs-ci'
$port     = 5123   # ★ codé en dur dans le harnais de Fragment
$toutes   = ($Scenes.Trim() -eq '')

New-Item -ItemType Directory -Force -Path $journaux, $refsMain, $refsCi | Out-Null

function Invoke-Muet([string]$nom, [scriptblock]$bloc) {
    # Lance une commande native, sortie vers un journal JAMAIS imprimé ; seul le code de sortie remonte.
    $journal = Join-Path $journaux ($nom + '.log')
    $debut = Get-Date
    & $bloc *> $journal
    $code = $LASTEXITCODE
    Write-Host ("{0} : code {1} ({2:N0} s)" -f $nom, $code, ((Get-Date) - $debut).TotalSeconds)
    if ($code -ne 0) { throw "$nom a échoué (code $code). Journal non publié (sources privées)." }
}

function Test-Port([int]$p) {
    foreach ($ip in '::1', '127.0.0.1') {
        $famille = if ($ip -eq '::1') { 'InterNetworkV6' } else { 'InterNetwork' }
        $c = New-Object System.Net.Sockets.TcpClient([System.Net.Sockets.AddressFamily]$famille)
        try {
            $a = $c.BeginConnect([System.Net.IPAddress]::Parse($ip), $p, $null, $null)
            if ($a.AsyncWaitHandle.WaitOne(1000) -and $c.Connected) { return $true }
        } catch { } finally { $c.Close() }
    }
    return $false
}

# --- 1. pixels/ voisin de Fragment -----------------------------------------------------------------
New-Item -ItemType Directory -Force -Path $rs | Out-Null
Copy-Item -Recurse -Force (Join-Path $Depot 'pixels') $rs
Get-ChildItem (Join-Path $Depot 'pixels\reference') -Filter *.png | Copy-Item -Destination $refsMain
if ($toutes) {
    # Passe complète : on part d'un dossier vide, une scène qui n'écrit rien se voit.
    Get-ChildItem (Join-Path $pixels 'reference') -Filter *.png | Remove-Item -Force
}

# --- 2. Fragment installé et préparé (dans le clone jetable, jamais dans l'arbre de l'utilisateur) --
Push-Location $fragment
try {
    Invoke-Muet 'npm-ci' { npm ci --no-audit --no-fund --loglevel=error }
} finally { Pop-Location }
Push-Location $app
try {
    Invoke-Muet 'sync-pdfjs' { npm run --silent sync:pdfjs }
    Invoke-Muet 'build-cli' { npm run --silent build:cli }
    Invoke-Muet 'transpile-electron' { npm run --silent transpile:electron }
} finally { Pop-Location }

$pw     = Join-Path $app 'node_modules\.bin\playwright.cmd'
$viteJs = Join-Path $app 'node_modules\vite\bin\vite.js'
foreach ($f in $pw, $viteJs) { if (-not (Test-Path $f)) { throw "Introuvable après npm ci : $(Split-Path -Leaf $f)" } }

# --- 3. Vite enveloppe + scènes -------------------------------------------------------------------
$env:FRAGMENT_RS_VITE_CACHE = (Join-Path $Capture 'vite-cache')
$env:FRAGMENT_NO_ONBOARDING = '1'
$sortieVite = Join-Path $pixels '.out'
New-Item -ItemType Directory -Force -Path $sortieVite | Out-Null
$vite = Start-Process -FilePath 'node' -WorkingDirectory $sortieVite -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput (Join-Path $journaux 'vite.log') -RedirectStandardError (Join-Path $journaux 'vite.err.log') `
    -ArgumentList @("`"$viteJs`"", '--config', "`"$(Join-Path $pixels 'vite.reference.config.ts')`"", '--configLoader', 'runner', '--host', 'localhost')
Set-Content -Path (Join-Path $Capture 'vite.pid') -Value $vite.Id
$fin = (Get-Date).AddSeconds(180)
while (-not (Test-Port $port)) {
    if ($vite.HasExited -or (Get-Date) -gt $fin) { throw "La Vite enveloppe n'a pas démarré sur $port (journal non publié)." }
    Start-Sleep -Milliseconds 500
}
Write-Host "Vite enveloppe en écoute sur $port."

$debut = (Get-Date).ToUniversalTime()
$json = Join-Path $journaux 'playwright.json'
$env:PLAYWRIGHT_JSON_OUTPUT_NAME = $json
$argsPw = @('test', '--config', (Join-Path $pixels 'playwright.config.ts'), '--reporter=json')
if (-not $toutes) {
    # ★ Un filtre positionnel par scène : un « | » serait mangé par cmd.exe (playwright.cmd est un shim).
    foreach ($nom in ($Scenes -split ',')) {
        $n = $nom.Trim()
        if ($n -notmatch '^[A-Za-z0-9._-]+$') { throw "Nom de scène refusé : $n" }
        $argsPw += ('[\/]' + [regex]::Escape($n) + '\.spec\.ts$')
    }
}
$codePw = 1
Push-Location $app   # le harnais fait « electron . » et lit e2e/fixtures/ en relatif
try {
    & $pw @argsPw *> (Join-Path $journaux 'playwright.log')
    $codePw = $LASTEXITCODE
} finally {
    Pop-Location
    if (-not $vite.HasExited) { & taskkill /PID $vite.Id /T /F *> $null }
}

# Résumé : titres (écrits dans fragment-rs) et statut ; d'une erreur, sa première ligne seulement.
$esc = [char]27
$echecs = 0; $total = 0
function Lire-Suite($suite) {
    foreach ($spec in @($suite.specs)) {
        if (-not $spec) { continue }
        foreach ($t in @($spec.tests)) {
            $r = @($t.results) | Select-Object -Last 1
            $statut = if ($r) { $r.status } else { 'non joué' }
            $script:total++
            if ($statut -ne 'passed') {
                $script:echecs++
                $msg = ''
                if ($r -and $r.error -and $r.error.message) {
                    $msg = (($r.error.message -replace "$esc\[[0-9;]*m", '') -split "`n")[0]
                    if ($msg.Length -gt 200) { $msg = $msg.Substring(0, 200) + '…' }
                }
                Write-Host ("  ÉCHEC  {0} [{1}] {2}" -f $spec.title, $statut, $msg)
            }
        }
    }
    foreach ($s in @($suite.suites)) { if ($s) { Lire-Suite $s } }
}
if (Test-Path $json) {
    $rapport = Get-Content $json -Raw | ConvertFrom-Json
    foreach ($s in @($rapport.suites)) { Lire-Suite $s }
    foreach ($e in @($rapport.errors)) {
        if ($e -and $e.message) { Write-Host ("  ERREUR globale : {0}" -f ((($e.message -replace "$esc\[[0-9;]*m", '') -split "`n")[0])) }
    }
}
Write-Host ("Playwright : code {0}, {1} test(s), {2} échec(s)." -f $codePw, $total, $echecs)

# --- 4. PNG capturés -> dépôt, provenance ----------------------------------------------------------
$captures = @(Get-ChildItem (Join-Path $pixels 'reference') -Filter *.png | Where-Object { $_.LastWriteTimeUtc -ge $debut } | Sort-Object Name)
$captures | Copy-Item -Destination $refsCi
Write-Host ("{0} PNG capturé(s)." -f $captures.Count)
if ($codePw -ne 0 -or $total -eq 0 -or $captures.Count -eq 0) { throw 'Capture incomplète : aucune référence remplacée.' }

$refDepot = Join-Path $Depot 'pixels\reference'
if ($toutes) {
    # Passe complète : le dépôt reflète exactement les scènes (une scène retirée perd son PNG).
    $noms = $captures | ForEach-Object { $_.Name }
    Get-ChildItem $refDepot -Filter *.png | Where-Object { $noms -notcontains $_.Name } | ForEach-Object {
        Write-Host "  retirée : $($_.Name)"; Remove-Item $_.FullName -Force
    }
}
$captures | Copy-Item -Destination $refDepot -Force

function Version-Paquet([string]$paquet) {
    $f = Join-Path $app "node_modules\$paquet\package.json"
    if (Test-Path $f) { (Get-Content $f -Raw | ConvertFrom-Json).version } else { $null }
}
$bureau = Get-ItemProperty 'HKCU:\Control Panel\Desktop' -ErrorAction SilentlyContinue
$adaptateur = (Get-CimInstance Win32_VideoController | Select-Object -First 1)
$shaFragment = (& git -C $fragment rev-parse HEAD).Trim()
$dateFragment = (& git -C $fragment log -1 --format=%cI).Trim()
$maintenant = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')

$provFichier = Join-Path $refDepot 'PROVENANCE.json'
$parPng = [ordered]@{}
if (Test-Path $provFichier) {
    $ancien = Get-Content $provFichier -Raw | ConvertFrom-Json
    if ($ancien.pngs) { foreach ($p in $ancien.pngs.PSObject.Properties) { $parPng[$p.Name] = $p.Value } }
}
foreach ($c in $captures) {
    $parPng[$c.Name] = [ordered]@{ fragment = $shaFragment; fragment_rs = $env:GITHUB_SHA; date = $maintenant }
}
$tri = [ordered]@{}
foreach ($k in ($parPng.Keys | Sort-Object)) { if (Test-Path (Join-Path $refDepot $k)) { $tri[$k] = $parPng[$k] } }

$prov = [ordered]@{
    commentaire  = 'Références capturées sur GitHub Actions par .github/workflows/references.yml (fr-e9q). « derniere_capture » décrit le dernier run ; « pngs » donne, par PNG, le run qui l''a produit.'
    derniere_capture = [ordered]@{
        date        = $maintenant
        run         = "$($env:GITHUB_SERVER_URL)/$($env:GITHUB_REPOSITORY)/actions/runs/$($env:GITHUB_RUN_ID)"
        declencheur = $env:GITHUB_EVENT_NAME
        scenes      = $(if ($toutes) { 'toutes' } else { $Scenes })
        pngs        = $captures.Count
        fragment    = [ordered]@{ depot = 'RebornFlamme/Fragment'; sha = $shaFragment; date_commit = $dateFragment }
        fragment_rs = [ordered]@{ sha = $env:GITHUB_SHA }
        outils      = [ordered]@{
            node       = (& node --version).Trim()
            electron   = (Version-Paquet 'electron')
            playwright = (Version-Paquet '@playwright/test')
            vite       = (Version-Paquet 'vite')
        }
        runner      = [ordered]@{
            os          = $env:ImageOS
            image       = $env:ImageVersion
            adaptateur  = $adaptateur.Name
            ecran       = "$($adaptateur.CurrentHorizontalResolution)x$($adaptateur.CurrentVerticalResolution)"
            cleartype   = "FontSmoothing=$($bureau.FontSmoothing) FontSmoothingType=$($bureau.FontSmoothingType) FontSmoothingGamma=$($bureau.FontSmoothingGamma) FontSmoothingOrientation=$($bureau.FontSmoothingOrientation)"
        }
        environnement = 'FRAGMENT_NO_ONBOARDING=1, 1280 x 800 (réglages : 1000 x 700), DPR 1, Vite enveloppe (cache hors Fragment).'
    }
    pngs = $tri
}
$texte = ($prov | ConvertTo-Json -Depth 6) -replace "`r`n", "`n"
[System.IO.File]::WriteAllText($provFichier, $texte + "`n", (New-Object System.Text.UTF8Encoding($false)))
# L'ancienne provenance texte (captures du PC) est remplacée par PROVENANCE.json.
$ancienTxt = Join-Path $refDepot 'PROVENANCE.txt'
if (Test-Path $ancienTxt) { Remove-Item $ancienTxt -Force }
Write-Host 'PROVENANCE.json écrit.'
