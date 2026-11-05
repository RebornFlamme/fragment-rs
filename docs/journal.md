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

## Vague 2 — 01/11/2026 15:20 UTC — usage 5h 40 % / semaine 34 % (autorisé 34,2 %)
Phase : 1 — **PORTE FRANCHIE**. Tâches fermées : fr-p1-core-b (#13), fr-p1-markdown (#14), fr-p1-parite (#15), fr-p1-divers (#16), puis l'épic fr-p1. En cours : fr-p2-s1b (fork de gpui-base::input, local). Bloqué : rien.
- CI verte sur main (run 41320167640). `cargo xtask parite` : 767/800 cas source portés = **95,9 %**, chaque non-porté justifié (`docs/parite-tests.md`).
- Contrat : `fragment-core/tests/contrats.rs` vérifie 48/68 noms du cœur ; les 20 restants (Vault, vues, objets du workspace, App) arrivent avec T1/T2.
Décisions prises par le chef :
- `menuModel`, `anchorLink`, `clusterFilter`, `Toolbar` (modèles) rangés provisoirement dans fragment-annotation ; à reclasser en T5/T8 si un autre consommateur apparaît.
- `SecretStorage` : trait dans fragment-core, implémentation (disque/chiffrement injectés) dans fragment-plugins.
- Disque C: plein (311 Mo libres) : dossiers cibles des tâches finies supprimés (2,6 Go rendus) ; chaque équipier supprime le sien en fin de tâche. Le reste du disque n'est pas à nous.
- Budget : semaine au plafond du prorata → pas de nouvel équipier avant que la marge remonte (≈ +0,5 %/h).
À trancher par l'orchestrateur : rien.
Prochaine vague : S1b (en cours) ; S2 et S3 dès que la marge hebdo le permet ; puis découpage de la phase 3 (T1 shell, ne dépend que de p1-core) en tâches Beads.

## Vague 2 (suite) — 01/11/2026 ~16:00 UTC — S1b GO
Tâches fermées : fr-p2-s1b (PR #17). Verdict **GO** (6/6 tests headless, sur NoopTextSystem) : fork n°1 `vendor/gpui-base` (+531/−30 lignes, 7 fichiers, API `set_row_model`), `[patch.crates-io]` + `exclude = ["vendor"]`, job CI `fork-patch` (version épinglée, sha256 de l'archive, liste exacte des fichiers modifiés). ADR-006 écrite.
Décisions prises par le chef : les dettes du fork (police par rangée — un H1 plus GROS —, gouttière/IME aux hauteurs variables, atomicité ←/→ des plages masquées) deviennent la tâche fr-p3-fork-input, prérequis de T3 ; la PR amont s'ouvre à sa fin (patch stable).
À trancher par l'orchestrateur : rien.

## Vague 3 — 01/11/2026 19:15 UTC — usage 5h 25 % / semaine 38 % (autorisé 36,4 %) — PAUSE
Phase : 2. Tâches fermées : fr-p2-s3 (PR #18). En cours : rien. Bloqué : budget hebdo au-dessus du prorata.
- S3 = **GO-AVEC** (QuickJS via gpui-shell) : rendu d'un arbre de 502 nœuds 13,8 ms, rechargement à chaud 23 ms, 12 évasions refusées, boucle infinie coupée à 2 s. Conditions pour T10 : une VM par plugin (~2,6 Mo, ~8 ms) ; Rust → JS jamais synchrone (contrats déclaratifs ou poussés) ; une vue racine par chargement ; gpui-shell `=0.8.1` sans la feature `webview` (elle tire `gpui-fast`, un autre GPUI) ; pile de 16 Mo pour l'hôte.
Décisions prises par le chef :
- Format des plugins : T10 fournit une coquille CommonJS où `require('fragment')` rend l'objet API, pour garder le format des plugins de Fragment (pas de divergence de contrat ; l'API en argument d'`onload` reste possible).
- `gpui-fast` : à trancher AVEC S2 (si la webview l'exige, l'activer partout ou isoler la webview).
- Cadence imposée par le plafond hebdomadaire : un spike ou une tranche lourde coûte 3 à 4 % de la semaine, le prorata en rend ≈ 0,54 %/h → au plus une grosse tâche locale toutes les 6-7 h. Pause jusqu'à ~22:20 UTC (prorata ≥ 38 %).
À trancher par l'orchestrateur : rien (le plafond n'est pas atteint ; c'est le prorata qui cadence).
Prochaine vague : S2 (pages HTML vivantes, WebView2) au retour sous le prorata ; puis fr-p3-fork-input et le découpage de T1.

## Incident — 02/11/2026 09:30 UTC — usage 5h 17 % / semaine 45 % (autorisé 44 %)
Le réveil programmé pour 22:25 UTC le 01/11 a ÉCHOUÉ : c'était une commande de fond qui dormait plusieurs heures, or une commande de fond est plafonnée à 10 minutes (code 4). Le chef est resté inactif ~11 h ; relancé par l'orchestrateur. Aucune perte : rien n'était en cours, rien n'était ouvert (`gh pr list` vide).
Décision du chef : tous les réveils passent désormais par l'outil de planification de l'app (`CronCreate` : un réveil ponctuel pour chaque échéance connue + un battement horaire de sécurité), jamais par une commande qui dort ; chaque fin de tour vérifie qu'un réveil est posé (`CronList`).
Prochaine vague : S2 dès que la semaine repasse sous le prorata (~11:20 UTC), puis fr-p3-fork-input et le découpage de T1.

## Décision budget — 02/11/2026 15:15 UTC — usage 5h 16 % / semaine 51 % (autorisé 47 %)
Remonté (§8) : l'hebdo monte d'≈ 1 point/h sans aucun équipier fragment-rs (travail de l'utilisateur sur Fragment, prioritaire). Tranché par l'orchestrateur : **(a) pause stricte sur l'abonnement** ; aucun lancement local au-dessus du prorata, et jamais au-dessus de **85 % hebdo** quel que soit le prorata.
Décisions du chef en conséquence :
- Tout ce qui tourne sous Linux part en CARTES CLOUD (crédits cloud, zéro abonnement), sans les attendre : `fr-p3-fork-input` (dettes du fork : police par rangée, gouttière/IME, atomicité ←/→, PR amont) et le découpage de T1 (plan `docs/tranches/T1.md` + `T1-seed.json` que le chef importe dans Beads).
- S2 (WebView2, Windows) reste local et attend le prorata.
- Battement du chef espacé à 2 h (CronCreate), réveil ponctuel à 23:37 heure de Paris.
À trancher par l'orchestrateur : rien.

## Reprise — 05/11/2026 13:25 UTC — usage 5h 34 % / semaine 67 %
Trois jours de pause : semaine repassée sous le prorata ; l'app a redémarré, réveils perdus (reposés). Aucune PR arrivée, les 2 cartes cloud n'ont pas été cliquées (conservées, travail fait en local).
Décisions de l'orchestrateur : jusqu'au reset du 06/11 18:00 UTC, plafond 77 % d'hebdo pour fragment-rs ; ordre S2, fr-p3-fork-input, découpage de T1 et sa 1re tâche. À partir du reset : PART FIXE de 30 points par semaine, 8 par jour au plus, fenêtre 5 h ≤ 80 % pour lancer, pause à 90 %, arrêt global à 85 % (program.md §2 mis à jour). Objectif : binaire de T1 + scènes pixels vertes.
Part : relevé de départ 67 %.
