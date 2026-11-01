# Journal de fragment-rs

Une entrée par vague, format fixe (program.md §8). L'orchestrateur lit la dernière entrée.

## Vague 0 — 01/11/2026 12:28 UTC — usage 5h 78 % / semaine 28 % (autorisé 32,8 %)
Phase : 0 (amorçage) — **PORTE FRANCHIE**. Tâches fermées : fr-p0 (pas de PR : amorçage direct sur `main`, dépôt neuf). En cours : rien. Bloqué : rien.
- Dépôt public https://github.com/RebornFlamme/fragment-rs (MIT). Workspace : 16 crates (14 vides, `fragment-ui` vide, `fragment-app` = fenêtre gpui-kit « Fragment » + dock vide) + `xtask`. gpui-kit `=0.8.1` / gpui-pre `=0.3.10`, Rust 1.99.1.
- CI verte sur `main` : run 41306245205 (fmt, clippy, test-linux, test-windows, check-app, pixels). `build.yml` vert : run 41306245117, artifact `fragment-rs-win-x64` (7,5 Mo).
- Preuve pixels sur le runner Windows (rendu logiciel, adaptateur Hyper-V) : `fenetre-vide` et `mire` 1280×800, chacune = sa 2e capture, **0 pixel**. Références Fragment : 22 PNG (11 scènes × sombre/clair), deux passes identiques au MD5. `cargo xtask pixels` : strict / toléré / échec, 5 tests.
- `bd ready` : fr-p1-artefacts, -text, -core-a, -cli, -pdf, -agent (+ épics fr-p1, fr-p2). Base Beads hors OneDrive (`C:/dev/fragment-rs-beads`), export commité `.beads-export/issues.jsonl`.
- Docs : `contrats.md` (188/188 exports, 19/19 registres, relu), `constantes-rendu.md` (50/50 CSS), ADR-001 socle, ADR-002 registre, ADR-003 CI (preuve pixels confirmée).
- Sous-agents : 2 Sonnet (contrats, pixels) + 1 Haiku (constantes). Aucun équipier.
Décisions prises par le chef :
- Scripts en Windows PowerShell 5.1 (pas de `pwsh` sur le portable), `.ps1` en UTF-8 avec BOM ; `powershell -File scripts/x.ps1` remplace `pwsh scripts/x.ps1` du programme.
- Contrats : réglages déclaratifs (`SettingDefinition*`) → fragment-core ; `DocWidgetHost` → fragment-artefacts (aucune crate de modèle ne dépend de fragment-ui).
- Constantes : la valeur LUE dans le CSS fait foi — en-tête 36 px (`ui/index.css:63`), pas 37/54 ; icônes 14/16/18 px ; 38 px = barre du doc-widget seulement.
- Tables de parité : un `parite.toml` par crate (pas d'écriture concurrente dans `docs/parite-tests.md`).
- `scripts/pixels-reference.ps1` refuse de tourner si le Vite de Fragment n'est pas déjà lancé (sinon cache écrit dans `Fragment/app/node_modules/.vite`).
- Voie C retirée (décision utilisateur) : `port.yml` supprimé, tâches toutes en voie B.
À trancher par l'orchestrateur : rien.
Prochaine vague (1) : 3 sessions cloud Sonnet, environnement « Fragment » (clone de la référence dans `/tmp/Fragment`), une par groupe débloquant : **fr-p1-text** (débloque layers, chat), **fr-p1-core-a** (débloque core-b, markdown, divers), **fr-p1-artefacts** (le plus gros). La 1re session vérifie l'accès à Fragment et le chef lit `get_usage` avant/après (confirmation de la voie B). Démarrage après le reset de la fenêtre 5 h (12:30 UTC).

