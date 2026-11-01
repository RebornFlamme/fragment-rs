# État d'une vague, à coller dans docs/journal.md avec l'usage (program.md §7).
# Régénère aussi l'export commité du registre (ADR-002) : à lancer AVANT de démarrer une vague.
#   powershell -File scripts/wave.ps1 [-SansExport]
param([switch]$SansExport)
$ErrorActionPreference = 'Continue'
$Racine = Split-Path -Parent $PSScriptRoot
Set-Location $Racine
$Depot = 'RebornFlamme/fragment-rs'

Write-Host '== bd ready'; bd ready
Write-Host "`n== en cours"; bd list --status in_progress
Write-Host "`n== PR ouvertes"; gh pr list -R $Depot
Write-Host "`n== derniers runs sur main"; gh run list -R $Depot --branch main --limit 4

if (-not $SansExport) {
    bd export -o .beads-export/issues.jsonl
    git add .beads-export/issues.jsonl
    git diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        git commit -q -m "docs : export du registre Beads`n`nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
        git push -q origin HEAD
        Write-Host "`nexport du registre commité et poussé"
    }
}
