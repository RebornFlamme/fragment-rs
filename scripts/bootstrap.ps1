# Phase 0 de fragment-rs en une commande (program.md §5, §7). IDEMPOTENT : relancer ne casse rien.
#   powershell -ExecutionPolicy Bypass -File scripts/bootstrap.ps1 [-SansPush] [-SansBeads]
# Compatible Windows PowerShell 5.1 (pwsh n'est pas installé sur le portable).
#
# Étapes : outils → crates vides du §4 → dépôt git → dépôt GitHub public + push → Beads hors OneDrive
# (épics fr-p0…fr-p4 + 11 tâches de phase 1, depuis scripts/beads-seed.json) → export du registre.
# Le contenu « riche » (fragment-app, xtask, workflows, docs) est commité tel quel : ce script ne
# le réécrit jamais, il ne crée que ce qui MANQUE.

param(
    [switch]$SansPush,
    [switch]$SansBeads
)

# ★ 'Continue' et non 'Stop' : sous PowerShell 5.1, la moindre ligne sur stderr d'un exe natif
# (cargo, gh, git) devient une erreur fatale avec 'Stop'. Les échecs sont détectés par $LASTEXITCODE (Natif).
$ErrorActionPreference = 'Continue'
$Racine = Split-Path -Parent $PSScriptRoot
Set-Location $Racine
$Depot = 'RebornFlamme/fragment-rs'
$BeadsReel = 'C:\dev\fragment-rs-beads\.beads'
$Utf8 = New-Object System.Text.UTF8Encoding($false)

function Etape($texte) { Write-Host "`n== $texte" -ForegroundColor Cyan }
function Ecrire-SiAbsent($chemin, $contenu) {
    if (-not (Test-Path $chemin)) {
        $dossier = Split-Path -Parent $chemin
        if ($dossier -and -not (Test-Path $dossier)) { New-Item -ItemType Directory -Force $dossier | Out-Null }
        if (-not $contenu.EndsWith("`n")) { $contenu += "`n" }
        [System.IO.File]::WriteAllText((Join-Path $Racine $chemin), $contenu, $Utf8)
        Write-Host "  créé : $chemin"
    }
}
function Natif($commande) {
    # Lance une commande native, échoue franchement sur code de sortie ≠ 0.
    # ★ Splatting (@reste) : le shim npm de `bd` (bd.ps1) recevait sinon un seul argument concaténé.
    $exe = $commande[0]
    $reste = @($commande | Select-Object -Skip 1)
    & $exe @reste
    if ($LASTEXITCODE -ne 0) { throw "échec ($LASTEXITCODE) : $($commande -join ' ')" }
}

# ---------------------------------------------------------------------------
Etape 'Outils'
foreach ($outil in 'git', 'gh', 'cargo', 'node', 'npm') {
    if (-not (Get-Command $outil -ErrorAction SilentlyContinue)) { throw "outil manquant : $outil" }
}
if (-not $SansBeads -and -not (Get-Command bd -ErrorAction SilentlyContinue)) {
    Natif @('npm', 'install', '-g', '@beads/bd')
}
New-Item -ItemType Directory -Force 'C:\dev\fragment-rs-target' | Out-Null

# ---------------------------------------------------------------------------
Etape 'Crates vides (program.md §4)'
$crates = [ordered]@{
    'fragment-core'       = 'Component/register*, registres, événements, portées clavier, commandes, plugins, Vault, documents, modèle du workspace, utilitaires purs.'
    'fragment-text'       = 'TextSurface/DocumentSurface/SurfaceText, positions et plages, mapRange/mapAnchor, rangeHighlights, viewport, findPassage, revealPassage, textDelta.'
    'fragment-artefacts'  = 'Artefacts : types, horloge HLC, pli, journal, objets, cache, ancres, ArtefactManager.'
    'fragment-layers'     = 'Calques (modèle) : WidgetAnchor/Handle/LazyWidget, WidgetLayer, GutterArbiter, gutterStack, WidgetActivation.'
    'fragment-markdown'   = 'Modèles Markdown : tableaux, frontmatter, transformations, listes, live preview (modèle), politique de sanitize.'
    'fragment-pdf'        = 'PDF : géométrie, zoom, outline, recherche, modèle PdfEditor (pdfium-render en phase 3).'
    'fragment-chat'       = 'Chat (modèles) : ChatSession, chatSurfaceModel, followModel, rail, recherche, historique, format.'
    'fragment-agent'      = 'Agent (modèles) : événements, trace, politiques, permissions, actions, skills, harness claude stream-json.'
    'fragment-annotation' = 'Annotation : géométrie, formes, session, persistance.'
    'fragment-snap'       = 'Snap : snapModel.'
    'fragment-theme'      = "Thème : jetons (noms d'Obsidian, valeurs de Fragment), themeModel, cascade (S4)."
    'fragment-cli'        = "CLI : protocole, index d'instances, shim, registre des coffres, client ``fragment``."
    'fragment-html'       = 'Pages HTML : pont (htmlBridge, htmlTextModel), hôte webview (S2).'
    'fragment-plugins'    = 'Plugins : hôte gpui-shell + API JS fragment (S3), catalogue, activation.'
    'fragment-ui'         = "L'app GPUI : shell du workspace sur gpui-kit, vues, calques, barres (phase 3)."
}
foreach ($nom in $crates.Keys) {
    $desc = $crates[$nom]
    Ecrire-SiAbsent "crates/$nom/Cargo.toml" @"
[package]
name = "$nom"
description = "$($desc -replace '"', '\"')"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]

[lints]
workspace = true
"@
    Ecrire-SiAbsent "crates/$nom/src/lib.rs" @"
//! $desc
//!
//! Crate VIDE en phase 0 : les modules arrivent avec les tâches de phase 1 (program.md §5).
"@
}
if (-not (Test-Path 'crates/fragment-app/src/main.rs')) { throw 'crates/fragment-app manque : il est commité, pas généré.' }

# Cargo.lock à jour (ne compile rien).
Natif @('cargo', 'generate-lockfile')

# ---------------------------------------------------------------------------
Etape 'Dépôt git'
if (-not (Test-Path '.git')) { Natif @('git', 'init', '-b', 'main') }
Natif @('git', 'add', '-A')
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) {
    $message = "feat : amorçage de la phase 0 (workspace, CI, outillage)`n`nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
    Natif @('git', 'commit', '-q', '-m', $message)
}

# ---------------------------------------------------------------------------
if (-not $SansPush) {
    Etape "Dépôt GitHub $Depot (public)"
    gh repo view $Depot --json name 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Natif @('gh', 'repo', 'create', $Depot, '--public', '--source', '.', '--push', '--description', "Fragment (clone d'Obsidian pour humains et agents) réécrit en Rust sur GPUI")
    } else {
        git remote get-url origin 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) { Natif @('git', 'remote', 'add', 'origin', "https://github.com/$Depot.git") }
        Natif @('git', 'push', '-u', 'origin', 'main')
    }
}

# ---------------------------------------------------------------------------
if (-not $SansBeads) {
    Etape "Beads (base hors OneDrive : $BeadsReel)"
    # ★ La base Dolt vit dans C:\dev ; le dépôt n'a qu'un `.beads/redirect` LOCAL (non commité,
    # il contient un chemin absolu propre à la machine).
    if (-not (Test-Path (Join-Path $BeadsReel 'config.yaml'))) {
        New-Item -ItemType Directory -Force $BeadsReel | Out-Null
        $env:BEADS_DIR = $BeadsReel
        Push-Location (Split-Path -Parent $BeadsReel)
        try { Natif @('bd', 'init', '--prefix', 'fr', '--non-interactive', '--quiet', '--skip-agents', '--skip-hooks') }
        finally { Pop-Location; Remove-Item Env:\BEADS_DIR }
    }
    New-Item -ItemType Directory -Force '.beads' | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $Racine '.beads\redirect'), $BeadsReel, $Utf8)
    Natif @('git', 'config', 'beads.role', 'maintainer')
    Natif @('bd', 'setup', 'claude')

    $seed = Get-Content 'scripts/beads-seed.json' -Raw -Encoding UTF8 | ConvertFrom-Json
    $existants = @{}
    $liste = bd list --all --json 2>$null
    if ($LASTEXITCODE -eq 0 -and $liste) { # ★ PS 5.1 : ConvertFrom-Json rend un tableau JSON comme UN objet ; passer par une variable pour l'itérer.
        $objets = ($liste | Out-String) | ConvertFrom-Json
        foreach ($i in $objets) { if ($i.id) { $existants[[string]$i.id] = $true } } }
    $tmp = [System.IO.Path]::GetTempFileName()
    foreach ($e in $seed.epics) {
        if (-not $existants.ContainsKey($e.id)) {
            [System.IO.File]::WriteAllText($tmp, $e.description, $Utf8)
            Natif @('bd', 'create', '--id', $e.id, '-t', 'epic', '-p', '1', '--title', $e.title, '--body-file', $tmp, '--silent')
        }
    }
    foreach ($t in $seed.tasks) {
        if (-not $existants.ContainsKey($t.id)) {
            $ref = $t.reference -replace '\bR\\', 'C:\Users\julie\OneDrive\Documents\Projects\Fragment\app\src\'
            $corps = "Référence : $ref`n`nFichiers possédés : $($t.owns)`n`nVérification : $($t.verify)`n`nFini quand : $($t.done)`n`nVoie : $($t.voie) (program.md §2)"
            [System.IO.File]::WriteAllText($tmp, $corps, $Utf8)
            Natif @('bd', 'create', '--id', $t.id, '-t', 'task', '-p', '2', '--title', $t.title, '--body-file', $tmp, '-l', "phase1,voie-$($t.voie)", '--silent')
            # bd refuse --id avec --parent : le parent est posé ensuite.
            Natif @('bd', 'update', $t.id, '--parent', $t.parent)
        }
    }
    Remove-Item $tmp
    # Dépendances (bd ignore un doublon ; on tolère l'erreur « existe déjà »).
    foreach ($x in @($seed.epics) + @($seed.tasks)) {
        foreach ($d in @($x.deps)) {
            if ($d) { bd dep add $x.id $d 2>$null | Out-Null }
        }
    }
    # Export commité : les sessions cloud lisent les tâches ici (ADR-002).
    New-Item -ItemType Directory -Force '.beads-export' | Out-Null
    Natif @('bd', 'export', '-o', '.beads-export/issues.jsonl')
    Natif @('git', 'add', '.beads-export/issues.jsonl', '.claude', 'CLAUDE.md')
    git diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        Natif @('git', 'commit', '-q', '-m', "docs : registre Beads initial (épics fr-p0…fr-p4, tâches de phase 1)`n`nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>")
        if (-not $SansPush) { Natif @('git', 'push', 'origin', 'main') }
    }
    Write-Host "`nbd ready :"
    bd ready
}

Etape 'Fini'
Write-Host "Rappel (utilisateur) : un environnement cloud doit exister dans l'app et pouvoir lire RebornFlamme/Fragment (program.md §2). Aucune clé API."
