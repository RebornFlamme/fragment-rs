<#
.SYNOPSIS
  Régénère la référence pixel : joue les scènes pixels/scenes/*.spec.ts sur Fragment
  (harnais e2e, Playwright de Fragment/app/node_modules) et écrit pixels/reference/*.png.
  ★ SECOURS LOCAL (fr-e9q, décision utilisateur du 13/11) : les références se capturent sur GitHub Actions
  (.github/workflows/references.yml, PR references/<date>). Ce script ne tourne sur le PC qu'en cas de panne
  DÉCLARÉE de la CI (au journal), sous le verrou de capture C:/dev/fragment-capture.lock.
.PARAMETER Scene
  Optionnel : une ou plusieurs scènes séparées par des virgules (ex. "menu,base"). Sans lui, toutes les scènes.
.NOTES
  Compatible Windows PowerShell 5.1. Fragment est en LECTURE SEULE : on exécute ses outils, on n'y
  écrit rien. Les sorties vont dans pixels/.out, le cache Vite dans C:/dev/fragment-rs-vite-cache,
  Electron et le coffre dans %TEMP% (le harnais s'en charge).
  - Serveur : la Vite enveloppe (pixels/vite.reference.config.ts) écoute sur 5123, port codé en dur
    dans Fragment. Si 5123 répond déjà, c'est le serveur de l'utilisateur : on le réutilise et on
    ne l'arrête jamais. Sinon on lance le nôtre et on l'arrête (arbre de processus) à la fin.
  - Garde : l'état de l'arbre de Fragment (git status --porcelain --ignored + empreinte des fichiers
    hors node_modules + node_modules/.vite et .vite-temp) est relevé avant et après ; tout écart
    fait échouer le script.
  - Écrit pixels/reference/PROVENANCE.txt (commit et branche de Fragment, serveur, date).
#>
param([string]$Scene = '')

$ErrorActionPreference = 'Stop'
$racine   = Split-Path -Parent $PSScriptRoot
$depots   = Split-Path -Parent $racine
$fragment = Join-Path $depots 'Fragment\app'
$config   = Join-Path $racine 'pixels\playwright.config.ts'
$viteCfg  = Join-Path $racine 'pixels\vite.reference.config.ts'
$pw       = Join-Path $fragment 'node_modules\.bin\playwright.cmd'
$viteJs   = Join-Path $fragment 'node_modules\vite\bin\vite.js'
$ref      = Join-Path $racine 'pixels\reference'
$sortie   = Join-Path $racine 'pixels\.out'
$cache    = 'C:/dev/fragment-rs-vite-cache'
$port     = 5123   # ★ codé en dur dans Fragment (e2e/harness.ts, dist-electron/main.js) : non surchargeable

if (-not (Test-Path $pw))     { throw "Playwright introuvable : $pw (node_modules de Fragment/app absent ?)" }
if (-not (Test-Path $viteJs)) { throw "Vite introuvable : $viteJs" }
if (-not (Test-Path (Join-Path $fragment 'dist-electron\main.js'))) { throw "Build Electron manquant : $fragment\dist-electron\main.js" }
New-Item -ItemType Directory -Force -Path $ref, $sortie, $cache | Out-Null

function Test-Port([int]$p) {
    # ★ « localhost » peut n'écouter qu'en IPv6 (::1) : on essaie les deux adresses explicitement.
    foreach ($ip in '::1', '127.0.0.1') {
        $c = New-Object System.Net.Sockets.TcpClient([System.Net.Sockets.AddressFamily]$(if ($ip -eq '::1') { 'InterNetworkV6' } else { 'InterNetwork' }))
        try {
            $a = $c.BeginConnect([System.Net.IPAddress]::Parse($ip), $p, $null, $null)
            if ($a.AsyncWaitHandle.WaitOne(1000) -and $c.Connected) { return $true }
        } catch { } finally { $c.Close() }
    }
    return $false
}

# Empreinte de l'arbre de Fragment : statut git (dont les ignorés) + liste (chemin, taille, mtime) de
# tout ce qui est sous app/ hors node_modules, plus node_modules/.vite et .vite-temp (ignorés, donc
# invisibles dans le statut git).
function Get-EtatFragment {
    $depot = (& git -C $fragment rev-parse --show-toplevel).Trim()
    $git = @(& git -C $depot status --porcelain --ignored)
    $fichiers = New-Object System.Collections.Generic.List[string]
    $racines = @(Get-ChildItem $fragment -Force | Where-Object { $_.Name -ne 'node_modules' } | ForEach-Object { $_.FullName })
    foreach ($d in '.vite', '.vite-temp', '.cache') {
        $x = Join-Path $fragment "node_modules\$d"
        if (Test-Path $x) { $racines += $x }
    }
    foreach ($r in $racines) {
        Get-ChildItem $r -Recurse -Force -File -ErrorAction SilentlyContinue | ForEach-Object {
            $fichiers.Add(('{0}|{1}|{2}' -f $_.FullName, $_.Length, $_.LastWriteTimeUtc.Ticks))
        }
        # les fichiers à la racine de app/ eux-mêmes
        if (Test-Path -PathType Leaf $r) { $f = Get-Item $r -Force; $fichiers.Add(('{0}|{1}|{2}' -f $f.FullName, $f.Length, $f.LastWriteTimeUtc.Ticks)) }
    }
    [pscustomobject]@{ Depot = $depot; Git = $git; Fichiers = @($fichiers | Sort-Object) }
}

function Assert-Identique($avant, $apres) {
    $d1 = Compare-Object $avant.Git $apres.Git
    $d2 = Compare-Object $avant.Fichiers $apres.Fichiers
    if ($d1 -or $d2) {
        Write-Host 'ÉCART dans l''arbre de Fragment (lecture seule violée) :' -ForegroundColor Red
        foreach ($d in @($d1) + @($d2)) { if ($d) { Write-Host ("  {0} {1}" -f $d.SideIndicator, $d.InputObject) } }
        throw 'Fragment a été modifié pendant la génération des références.'
    }
}

$avantEtat = Get-EtatFragment
$commit  = (& git -C $avantEtat.Depot rev-parse HEAD).Trim()
$branche = (& git -C $avantEtat.Depot branch --show-current).Trim()
$sale    = @($avantEtat.Git | Where-Object { $_ -notmatch '^!!' })

# --- Serveur Vite (5123) ---------------------------------------------------------------------------
$serveur = $null
$modeServeur = ''
if (Test-Port $port) {
    $modeServeur = "réutilisé (déjà en écoute sur $port avant le lancement, serveur de l'utilisateur : non arrêté)"
    Write-Host "Port $port déjà occupé : serveur existant réutilisé, il ne sera pas arrêté."
} else {
    $modeServeur = "lancé par le script (Vite enveloppe, cache $cache), puis arrêté"
    $logVite = Join-Path $sortie 'vite.log'
    $serveur = Start-Process -FilePath 'node' -WorkingDirectory $sortie -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $logVite -RedirectStandardError (Join-Path $sortie 'vite.err.log') `
        -ArgumentList @("`"$viteJs`"", '--config', "`"$viteCfg`"", '--configLoader', 'runner', '--host', 'localhost')
    $fin = (Get-Date).AddSeconds(90)
    while (-not (Test-Port $port)) {
        if ($serveur.HasExited -or (Get-Date) -gt $fin) {
            $msg = Get-Content (Join-Path $sortie 'vite.err.log') -Raw -ErrorAction SilentlyContinue
            if (-not $serveur.HasExited) { & taskkill /PID $serveur.Id /T /F | Out-Null }
            throw "La Vite enveloppe n'a pas démarré sur $port. $msg"
        }
        Start-Sleep -Milliseconds 500
    }
    Write-Host "Vite enveloppe lancée (PID $($serveur.Id)), port $port."
}

$code = 1
try {
    $env:FRAGMENT_NO_ONBOARDING = '1'
    $avant = Get-Date
    $args2 = @('test', '--config', $config)
    if ($Scene -ne '') {
        # ★ Un filtre par scène (positionnels) : un « | » dans une seule expression serait mangé par cmd.exe
        # (playwright.cmd est un shim).
        foreach ($nom in ($Scene -split ',')) { $args2 += ('[\/]' + [regex]::Escape($nom.Trim()) + '\.spec\.ts$') }
    }
    Push-Location $fragment   # le harnais fait « electron . » et lit e2e/fixtures/ en relatif
    try { & $pw @args2; $code = $LASTEXITCODE } finally { Pop-Location }
} finally {
    if ($serveur) {
        if (-not $serveur.HasExited) { & taskkill /PID $serveur.Id /T /F | Out-Null }
        $fin = (Get-Date).AddSeconds(15)
        while ((Test-Port $port) -and (Get-Date) -lt $fin) { Start-Sleep -Milliseconds 300 }
        if (Test-Port $port) { Write-Host "ATTENTION : le port $port est encore occupé après l'arrêt de notre serveur." -ForegroundColor Red; $code = 1 }
        else { Write-Host "Vite enveloppe arrêtée, port $port libéré." }
    }
}

# --- Garde : Fragment n'a pas bougé -----------------------------------------------------------------
$apresEtat = Get-EtatFragment
Assert-Identique $avantEtat $apresEtat
Write-Host ("Fragment inchangé ({0} entrées git, {1} fichiers suivis par la garde)." -f $apresEtat.Git.Count, $apresEtat.Fichiers.Count)

# --- Provenance (seulement après une passe complète : une scène isolée ne la réécrit pas) -----------
if ($Scene -eq '' -and $code -eq 0) {
    $prov = @(
        'Références pixels de fragment-rs, capturées sur Fragment (lecture seule).',
        "Commit de Fragment : $commit",
        "Branche de Fragment : $branche",
        ("Arbre de travail de Fragment : {0} chemin(s) modifié(s) ou non suivi(s) (hors ignorés) au lancement." -f $sale.Count),
        "Serveur Vite (port $port) : $modeServeur",
        'Cache Vite : C:/dev/fragment-rs-vite-cache (hors Fragment) ; userData Electron : dossier temporaire du harnais.',
        'Environnement : FRAGMENT_NO_ONBOARDING=1, 1280 x 800, DPR 1.',
        'Garde : git status --porcelain --ignored et empreinte des fichiers de Fragment/app identiques avant et après.'
    )
    [System.IO.File]::WriteAllLines((Join-Path $ref 'PROVENANCE.txt'), $prov, (New-Object System.Text.UTF8Encoding($false)))
}

Add-Type -AssemblyName System.Drawing
$pngs = Get-ChildItem $ref -Filter *.png | Where-Object { $_.LastWriteTime -ge $avant } | Sort-Object Name
Write-Host ''
Write-Host ("{0} PNG écrit(s) dans {1}" -f @($pngs).Count, $ref)
foreach ($f in $pngs) {
    $img = [System.Drawing.Image]::FromFile($f.FullName)
    try { Write-Host ("  {0,-48} {1}x{2}  {3} Ko" -f $f.Name, $img.Width, $img.Height, [int]($f.Length / 1KB)) } finally { $img.Dispose() }
}
exit $code
