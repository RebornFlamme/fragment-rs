# État CI d'une PR ou d'un run + log des échecs seulement (program.md §7).
#   powershell -File scripts/ci.ps1 <numéro de PR | id de run>
param([Parameter(Mandatory = $true)][string]$Cible)
$ErrorActionPreference = 'Stop'
$Depot = 'RebornFlamme/fragment-rs'

if ($Cible.Length -lt 7) {
    # Numéro de PR → dernier run de sa branche.
    $branche = gh pr view $Cible -R $Depot --json headRefName -q .headRefName
    gh pr checks $Cible -R $Depot
    $run = gh run list -R $Depot --branch $branche --workflow ci.yml --limit 1 --json databaseId -q '.[0].databaseId'
} else {
    $run = $Cible
}
gh run view $run -R $Depot
$echec = gh run view $run -R $Depot --json jobs -q '[.jobs[] | select(.conclusion == \"failure\")][0].databaseId'
if ($echec) {
    Write-Host "`n== log des échecs (job $echec, 200 lignes max)"
    gh run view $run -R $Depot --job $echec --log-failed | Select-Object -Last 200
}
