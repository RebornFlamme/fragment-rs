# pixels/ — parité au pixel avec Fragment

Voir `program.md`, « Parité au PIXEL ». Une scène = un coffre + des actions + une capture
(1280 × 800, DPR 1, sombre ET clair).

| Dossier | Rôle |
|---|---|
| `scenes/<scene>.spec.ts` | Playwright : pilote **Fragment** par son harnais e2e (`readerHarness`), écrit `reference/<scene>-<thème>.png` |
| `scenes/<scene>.json` | La même scène pour le candidat Rust (`fragment-pixels`) |
| `scenes/_commun.ts` | Décor partagé (note + PDF en split + chat + conversation ancrée), gel caret/animations |
| `vault/` | Coffre de fixture, copié de `visual-diff.spec.ts` (le harnais l'écrit dans `%TEMP%`, jamais ici) |
| `reference/` | PNG capturés sur Fragment (commités) ; `candidate/` et `diff/` : générés, ignorés |

Régénérer : `powershell -File scripts/pixels-reference.ps1 [scène]` (hors CI, Fragment doit être présent).
Prérequis dans `Fragment/app` : `node_modules` et `dist-electron/` ; serveur Vite sur 5123 (lancé par la
config s'il manque — son cache va alors dans `Fragment/app/node_modules/.vite`).

## Schéma JSON (écarts au schéma de base, nécessaires à la fidélité)
Actions de base : `command`, `open`, `click`, `key`, `type`, `wait-stable`. Ajouts : `hover {x,y}`,
`wait {ms}`, `click.button|count|target` (`target:"settings"` = fenêtre de réglages), `open.split`,
`command.args`, `settings_window` (1000 × 700, comme visual-diff). `wait-stable` = polices prêtes + 2 rAF
+ 150 ms, sans bouger le pointeur ; le pointeur est garé par `hover 2,2`. La capture est implicite en fin
d'actions, par thème. `pixels:notice` et `doc-widget:open-anchored` sont à fournir côté Rust (Fragment
les fait par JS : `new Notice(…, 0)`, `host.open(…)`). `fenetre-vide.json` : preuve CI Rust, sans spec.
Les scènes de réglages sont en 1000 × 700, pas 1280 × 800.

## État
22 captures (11 scènes × 2 thèmes) écrites, déterministes (2 passes, MD5 identiques sur base et selection).
Le plugin `e2e-chat` du coffre est un double de test JS : le candidat n'a pas à l'exécuter, il lui faut
seulement l'état visuel (conversation « Bonjour », conversation ancrée).
Note : `note.md` exige des fins de ligne LF (ajouter `pixels/vault/** -text` en `.gitattributes` racine).
Dates : non figées ; aucun écart observé entre deux passes.
