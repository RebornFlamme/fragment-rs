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


## Vague 1 (démarrage) — 01/11/2026 12:55 UTC — usage 5h 4 % / semaine 29 % (autorisé 33 %)
Phase : 1. En cours : fr-p1-text, fr-p1-core-a, fr-p1-artefacts — 3 sessions cloud Sonnet démarrées par l'orchestrateur (environnement « Default » : aucun environnement « Fragment » n'existe), briefs du chef. Bloqué : rien.
Décisions prises par le chef :
- Règle du commanditaire (01/11) : le chef gère tout jusqu'à la livraison d'un binaire de tranche finie ; personne ne sollicite l'utilisateur avant. Le chef programme ses propres réveils et ne remonte que le §8.
- Vagues suivantes : une session cloud exige un tour déclenché par l'utilisateur → repli autonome = équipiers LOCAUX Sonnet (Agent Team) sous les seuils du §2 ; cartes cloud facultatives, jamais attendues.
- Voie B : usage relevé avant (5h 79 %, sem. 29 %) et juste après le départ (5h 4 % après reset, sem. 29 %) ; confirmation sur le compteur hebdomadaire à la fin de la vague.
À trancher par l'orchestrateur : rien.

## Vague 1 (suite) — 01/11/2026 15:00 UTC — usage 5h 27 % / semaine 32 % (autorisé 34 %)
Phase : 1 et 2. Tâches fermées : fr-p1-cli (#1), fr-p1-text (#2), fr-p1-pdf (#3), fr-p1-artefacts (#4), fr-p1-core-a (#5), fr-p1-text-utf16 (#6), fr-p1-agent (#7), fr-p2-s1 (#8), fr-p1-layers (#9), fr-p1-chat (#10), fr-p1-artefacts-utf16 (#11), fr-p1-core-utf16 (#12), fr-p2-s4 (ADR-005). En cours (local) : fr-p1-core-b, fr-p1-markdown, fr-p1-divers. Bloqué : rien.
- Voie B confirmée : 3 sessions cloud (text, core-a, artefacts) ont livré sans bouger le compteur hebdomadaire ; la référence Fragment se clone par l'app GitHub de Claude (le jeton n'est pas dans l'environnement « Default »). Les vagues suivantes tournent en local (une session cloud exige un clic de l'utilisateur).
Décisions prises par le chef :
- ADR-004 : toute position de texte publique ou persistée en unités UTF-16 (compat des journaux de Fragment, API JS) ; correctifs fr-p1-text-utf16, -artefacts-utf16, -core-utf16 fusionnés.
- ADR-005 (S4) : struct Theme aux valeurs d'index.css, color-mix résolus à la construction, theme.json plat, DM Sans embarquée, garde-fou anti-littéraux.
- Un CARGO_TARGET_DIR par worktree (dossier cible partagé = dépendances par chemin cassées).
- Acceptés : panique de plugin rattrapée (catch_unwind), traits d'hôte au lieu de l'App, API synchrones, deps regex / time / unicode-normalization / log / sha2 / uuid (MIT/Apache, Unicode-3.0).
Remonté et tranché par l'orchestrateur :
- S1 NO-GO sur l'éditeur stock (masquage sans retrait, hauteur de ligne unique, pas de bloc). ACCORD pour le 1er fork (≤ 2) : gpui-base::input via [patch.crates-io] → vendor/gpui-base, patch minimal, PR amont, test CI « le patch s'applique encore ». Sous-spike B fr-p2-s1b, 3 jours max, six tests ; sinon bascule sur C (mise en page maison) avec ADR, sans remonter. Hauteur de rangée dictée par le modèle. T3 attend le verdict B.
À trancher par l'orchestrateur : rien.
Prochaine vague : fr-p2-s1b (prioritaire), S2, S3 au fil des places ; puis porte de la phase 1 (parite-tests.md via cargo xtask parite).
