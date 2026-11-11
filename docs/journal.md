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

## Vague 4 — 05/11/2026 13:45 UTC — usage 5h 67 % / semaine 72 % (plafond 77 %)
Tâches fermées : fr-p3-fork-input (PR #19, CI verte dont fork-patch). En cours : fr-p2-s2 (PR #20 ouverte). Part consommée depuis 67 % : +5 points (dont travail hors fragment-rs possible).
Décisions du chef : PR amont du fork REPORTÉE à la montée vers gpui-kit 0.9 (amont en v0.9.0, arborescence changée, « une PR, une chose », patch pas encore éprouvé sur DirectWrite) ; texte prêt dans ADR-006. Dette pour T3 : le soft wrap mesure au corps de base (une ligne H1 peut déborder).

## Vague 4 (fin, avant clear) — 05/11/2026 14:05 UTC — usage 5h 69 % / semaine 72 % (plafond 77 % jusqu'au reset)
**État des portes.** Phase 0 et phase 1 franchies (767/800 cas, 95,9 %). Phase 2 : S1 NO-GO → repli B fork GO (S1b, #17), S3 GO-AVEC (#18), S4 GO (ADR-005), **S2 GO-AVEC mais PR #20 NON fusionnée** (voir ci-dessous) ; l'épic `fr-p2` reste ouverte tant que #20 n'est pas fusionnée. Phase 3 pas commencée. Aucune livraison.
**En cours.** Aucun équipier vivant, aucun worktree. PR ouverte : #20, branche `task/fr-p2-s2`, CI verte (8 jobs), bead `fr-p2-s2` en in_progress. `bd ready` : `fr-p2` (épic), `fr-p3-gpui-kit-0.9`.
**S2, résultat.** `gpui-webview` n'exige pas `gpui-fast` (feature facultative, macOS/Linux). JS, pont hauteur/lien/touche, thème avant la 1re image, sandbox : GO. ★ `gpui-webview` tel quel rend la page INVISIBLE sous Windows (HWND enfant sous le rendu GPUI) et un calque GPUI par-dessus n'existe pas. Prouvé par captures réelles du bureau : webview dans un popup possédé + menus/calques en fenêtres GPUI distinctes (4 à 6 j, T9). DirectComposition non essayée (8 à 15 j, risque élevé). Non observé : GPU réel, saisie et focus réels, suivi de la fenêtre.
**Copies de sources de Fragment (erreur du chef, briefs de phase 0 et de S2 ; §2 l'interdit : « jamais une copie des sources de Fragment dans fragment-rs »).**
- `pixels/vault/.fragment/plugins/e2e-chat/main.js` : identique à `app/e2e/fixtures/chat-provider-plugin.cjs` (89 lignes), sur `main` et donc public depuis le 01/11.
- `crates/fragment-html/tests/fixtures/htmlBridge.js` : `HTML_BRIDGE_SCRIPT` tel quel (270 lignes), seulement sur la branche publique `task/fr-p2-s2` et la PR #20. C'est pourquoi #20 n'est pas fusionnée.
- Vérifié : `crates/fragment-plugins/tests/fixtures/plugins/e2e-globe/main.js` diffère de `gutter-globe-plugin.cjs` (73 lignes de diff) ; il n'est pas identique, sa proximité reste à relire. La recherche a porté sur ces fixtures, pas sur tout le dépôt.
- Remonté à l'orchestrateur : garder (la plupart du pont devra de toute façon vivre dans le produit, T9), réécrire, ou purger (réécrire l'historique d'un dépôt public : décision de l'utilisateur). Rien n'est retiré ni réécrit en attendant.
**Budget, part.** Jusqu'au reset du 06/11 18:00 UTC : plafond 77 %, relevé de départ 67 % ; cumul à ce relevé +5 points (S2 + fork-input + chef, avec le travail éventuel de l'utilisateur dedans). À partir du reset : part fixe de 30 points/semaine, 8/jour, 5 h ≤ 80 % pour lancer, pause à 90 %, arrêt global à 85 % (program.md §2) ; relever `get_usage` avant et après chaque tâche, cumul ici.
**Décisions à retenir pour T3/T9/T10.** Dettes du fork pour T3 : soft wrap au corps de base (un H1 peut déborder) ; dessin des numéros de ligne non observé en headless. `fr-p3-gpui-kit-0.9` (montée, rebase, PR amont avec le texte d'ADR-006) doit bloquer la FIN de T3 : **poser cette dépendance quand les tâches de T3 existent** ; `fork-patch` reste obligatoire sur chaque PR. T10 : VM par plugin, Rust → JS asynchrone, coquille CommonJS `require('fragment')`, pile 16 Mo. Carte cloud du découpage de T1 encore posée (id `task_da5e0f81`), non attendue : si elle reste non cliquée, à retirer quand le découpage est fait en local.
**Prochaine action exacte.** (1) Appliquer la réponse de l'orchestrateur sur les copies, puis corriger et fusionner #20 (`bd close fr-p2-s2`, puis `bd close fr-p2` : 3 verdicts écrits) ; retirer `spike-s2.yml` si ce workflow n'a plus d'usage. (2) Découper T1 : `docs/tranches/T1.md` + `T1-seed.json`, importer dans Beads, lancer la 1re tâche (squelette `fragment-ui` + `fragment-theme::Theme` d'ADR-005). Objectif : binaire de T1 avec ses scènes pixels vertes. (3) Reposer un réveil `CronCreate` (battement 2 h), jamais une commande de fond qui dort.

## Incident sources — 06/11/2026 09:25 UTC — usage 5h 1 % / semaine 78 % (plafond 77 % jusqu'au reset)
Tâches fermées : fr-t0x (PR #21). En cours : PR #20, corrigée, audit à 0, CI relancée après mise à jour depuis `main`. Bloqué : rien.
L'audit trouve 25 fichiers copiés de Fragment, pas 2 (détail : ADR-007) : 24 sur `main` depuis le 01/11, dont 22 ports Rust par leurs blocs de commentaires, et `htmlBridge.js` sur la seule branche de #20. Tout est réécrit ; rien n'est purgé de l'historique.
Décisions du chef : `cargo xtask audit-sources --rev origin/task/<id>` obligatoire avant chaque fusion ; commentaires et fixtures se réécrivent (program.md §3, §7).
À trancher : la purge de l'historique, soumise à l'utilisateur par l'orchestrateur. Non vérifié : fixture `e2e-chat` et `_commun.ts` non rejoués sur Fragment (Vite absent) ; à confirmer à la prochaine régénération des références.
Budget : plafond hebdo dépassé d'un point, signalé à l'orchestrateur (§8) ; rien de nouveau avant le reset de 18:00 UTC.
Prochaine vague : fusion de #20, puis découpage de T1 et sa 1re tâche après le reset, sur la part fixe.

## Vague 5 (avant clear) — 06/11/2026 19:10 UTC — usage 5h 20 % / semaine 2 %
Phase : 3, T1. Tâches fermées : fr-t0x (#21), fr-p2-s2 (#20), fr-p2, fr-60v (#22). En cours : fr-t1-squelette, PR #23, audit à 0, CI relancée après mise à jour depuis `main`, équipier fini, bead encore ouvert. Bloqué : rien.
**Part fixe** : relevé de départ 0 % au reset du 06/11 18:00 UTC. Cumul 2 points : découpage de T1, squelette et chef compris. C'est moins que l'estimation de 3,5 pour le seul squelette ; un seul relevé, à confirmer. Sonnet : 215k tokens pour le découpage, 247k pour le squelette.
Décisions de l'orchestrateur : `audit-sources` à 6 lignes (fait, 16 plages réécrites et non 5, car ce seuil en signalait 16) ; « lecture seule » = rien d'écrit dans l'arbre de Fragment, outils permis avec caches et sorties redirigés dehors ; `DockArea` abandonné (ADR-008) ; historique et visibilité du dépôt chez l'utilisateur, ne rien réécrire ni changer d'ici sa réponse.
Décisions du chef : l'audit se lance avec le binaire de la branche mise à jour depuis `main`, car les exclusions sont compilées et qu'une branche antérieure à une correction est faussement signalée ; dette `fr-nj9` (JetBrains Mono absente de Fragment et de la machine). Revue de #23 par périmètre, audit et CI, le code lu seulement par échantillon (`echelle.rs`).
Cloud : `fr-t1-arbre` et `fr-t1-workspace` RÉSERVÉES, brief unique chez l'orchestrateur, qui les lancera au prochain tour de l'utilisateur et doit le dire. Sans visite au 08/11 18:00 UTC, les reprendre en local après l'avoir prévenu. Ne pas les lancer en local avant.
**Prochaine action exacte.**
1. Si `gh pr checks 23` est verte : `cargo xtask audit-sources --rev origin/task/fr-t1-squelette` puis `gh pr merge 23 --squash --delete-branch` puis `bd close fr-t1-squelette` puis `bd export -o .beads-export/issues.jsonl`, commit et push.
2. Lancer en équipiers locaux Sonnet 5.5, en worktree, chacun avec son propre `CARGO_TARGET_DIR` : `fr-t1-references` (déjà réservée ; vérifier D'ABORD si le port 5123 est codé en dur côté Fragment, `e2e/harness.ts:27` ; si oui, remonter à l'orchestrateur la commande du repli) et `fr-t1-lecteur` (à réserver). Dans chaque brief : « un port TRADUIT le code et RÉÉCRIT les commentaires dans ses mots ; il ne recopie ni en-tête ni diagramme ; aucune copie de source de Fragment, une fixture se réécrit ». Modèle de brief : bead qui fait foi, règles du §9, audit avant push, ni fusion ni fermeture.
3. `fr-t1-references` régénère les 22 PNG : c'est aussi la vérification de la fixture `e2e-chat` et de `_commun.ts` réécrits (ADR-007, non vérifiés à ce jour).
4. Vagues suivantes : `docs/tranches/T1.md` §4. Relever `get_usage` avant et après chaque tâche, cumul ici.
5. Reposer les réveils par `CronCreate` : un battement toutes les 2 h ; aucun réveil n'est actif au moment du clear.

## Vague 6 (début) — 06/11/2026 19:15 UTC — usage 5h 21 % / semaine 2 %
Phase : 3, T1. Tâches fermées : fr-t1-squelette (#23, CI verte, audit à 0 avec le binaire de la branche). En cours : fr-t1-references et fr-t1-lecteur, équipiers locaux Sonnet, worktrees `../fragment-rs-references` et `../fragment-rs-lecteur`. Réservées au cloud : fr-t1-arbre, fr-t1-workspace. Bloqué : rien.
**Part fixe** : relevé de départ de cette vague 2 points (cumul depuis le reset du 06/11 18:00 UTC). Par jour : 2 sur 8.
Décisions du chef : le port 5123 est codé en dur côté Fragment (`e2e/harness.ts:27`, `dist-electron/main.js`), mais ce n'est pas un motif de repli : la Vite enveloppe écoute sur 5123 et le serveur de l'utilisateur est réutilisé s'il tourne, sans écriture dans Fragment. Rien à remonter. Worktrees frères de `Fragment`, car `pixels/` y accède par `../../Fragment`.
Orchestrateur : clear du chef désormais par lui-même (§10, `docs/brief-reprise.md`), non éprouvé. À trancher : rien.
**Prochaine action exacte.** Aux comptes rendus : relever `get_usage`, lire le diff, `cargo xtask audit-sources --rev origin/task/<id>` avec le binaire de la branche à jour de `main`, fusion, `bd close`, export du registre, retrait du worktree. Puis fr-t1-menus et fr-t1-palette (T1.md §4), sous 8 points par jour. fr-t1-arbre et fr-t1-workspace : en local à partir du 08/11 18:00 UTC seulement, si le cloud n'est pas venu, en prévenant l'orchestrateur.

## Vague 6 (lecteur fusionné) — 06/11/2026 20:00 UTC — usage 5h 28 % / semaine 3 %
Tâches fermées : fr-t1-lecteur (#24, CI verte, audit à 0 ; 271k tokens). En cours : fr-t1-references. **Part fixe** : cumul 3 points, soit environ 1 point pour le lecteur contre 2,5 estimés, relevé arrondi et partagé avec `fr-t1-references` en parallèle.
Preuve du geste continu : `drag` tient en headless (`Window::dispatch_event`, redessin entre événements), risque 4 de T1.md levé côté lecteur. Non vérifié : `_t1.ts` jamais exécuté ; réglages = fond vide.
Décisions du chef : revue de #24 par périmètre, audit, CI et lecture des dépendances ; code non relu ligne à ligne. Écart de périmètre accepté : `pas_de_litteral.rs`, l'exception suit la mire déplacée. `fragment-app` tire désormais `image`, `serde`, `thiserror` et les crates du cœur hors feature `pixels`, car le lecteur est aussi une cible de test. fr-t1-menus et fr-t1-palette attendent `fr-t1-references` : leurs scènes exigent le script de références et l'exécution de `_t1.ts`.
**Prochaine action exacte.** Au compte rendu de `fr-t1-references` : usage, diff, audit, fusion, `bd close`, export, retrait du worktree `../fragment-rs-references`. Puis fr-t1-menus et fr-t1-palette en équipiers locaux Sonnet, sous 8 points par jour. fr-t1-arbre et fr-t1-workspace : en local à partir du 08/11 18:00 UTC seulement, si le cloud n'est pas venu, en prévenant l'orchestrateur.

## Vague 6 (références fusionnées) — 10/11/2026 17:00 UTC — usage 5h 9 % / semaine 12 %
Tâches fermées : fr-t1-references (#25, CI verte, audit à 0). En cours : fr-t1-menus et fr-t1-palette, équipiers locaux Sonnet, worktrees `../fragment-rs-menus` et `../fragment-rs-palette`. Cloud (lancé par l'orchestrateur) : fr-t1-arbre puis fr-t1-workspace, pas encore de branche ; surveiller leurs PR, ne pas les prendre en local. Bloqué : rien.
**Part fixe** : la semaine est passée de 3 % à 11 % entre le 06/11 20:00 et le 10/11 16:20 sans travail de fragment-rs (usage de l'utilisateur, hors part). Cumul de fragment-rs : environ 4 points, dont 1 pour clore les références. Par jour : 1 sur 8 le 10/11.
Références : le jeu du 01/11 est remplacé par celui de Fragment 89c7a80 (16 PNG changent : texte du PDF après 4f16922, plus une commande dans `reglages-hotkeys` ; 6 restent identiques). L'arbre de travail de Fragment avait des fichiers non suivis (doc, specs), aucun dans `app/src`. Garde « Fragment inchangé » tenue sur les trois lots. La fixture `e2e-chat` et `_commun.ts` réécrits sont vérifiés : aucun écart dans leurs zones.
★ Une passe de `pixels-reference.ps1` coûte ~7 min de garde (`git status --ignored` de Fragment sous OneDrive) ; la commande de fond, plafonnée à 10 min, a échoué (code 4). Désormais : premier plan avec un délai de 600 s, par lots, et verrou `C:/dev/fragment-rs-pixels-verrou` entre équipiers (port 5123 unique). ★ Un push sur une PR peut ne pas déclencher la CI (8f8c38a) : `gh workflow run ci --ref <branche>`.
Décisions du chef : les équipiers capturent eux-mêmes les références de leurs scènes (le bead disait « fournies par l'utilisateur ») ; `fr-t1-menus` est seul autorisé à corriger `_t1.ts`, jamais exécuté avant lui.
**Prochaine action exacte.** Aux comptes rendus de menus et palette : usage, diff, audit, CI, fusion, `bd close`, export, retrait des worktrees. Aux PR cloud d'arbre et de workspace, même revue. Puis fr-t1-onglets (dépend de workspace et menus), puis reglages, splits et binaire (T1.md §4).

## Décision de l'utilisateur — 10/11/2026 17:10 UTC
`fragment-rs` reste public, historique non purgé (ADR-007, section finale). Question close, plus de remontée. L'audit `audit-sources` au seuil de 6 avant chaque fusion reste obligatoire. La session cloud `fr-t1-arbre` → `fr-t1-workspace` a démarré (orchestrateur) ; surveiller ses PR.

## Vague 6 (menus fusionné) — 10/11/2026 18:20 UTC — usage 5h 27 % / semaine 14 %
Tâches fermées : fr-t1-menus (#26, CI verte, audit à 0), en deux équipiers : le premier s'est arrêté à 300k tokens, un neuf a repris. ★ Arrêter explicitement (`TaskStop`) l'équipier qui passe la main : le premier a continué en parallèle du second sur la même branche. En cours : fr-t1-palette ; fr-t1.1 (bug, parité des surfaces flottantes), équipier local, worktree `../fragment-rs-flottantes`. Cloud : arbre puis workspace, pas encore de branche.
**Part fixe** : cumul environ 6 points (menus ≈ 2). Par jour : environ 3 sur 8 le 10/11.
Décisions du chef :
- Pour les surfaces flottantes, le décor `_t1-flottantes.ts` (racine de Fragment masquée par `#root *`) est accepté. Sinon, une scène T1 en fenêtre entière est rouge avant le binaire.
- Palette garde la fenêtre entière, en `"depuis": "T2"`, avec sa zone mesurée au rognage.
- Les 6 scènes de menus divergent sur de la géométrie (notice +2 px, lignes de menu de 29,5 px arrondies à 29 par GPUI), sur les ombres et sur les icônes. Elles sont fusionnées en T2, et la dette devient `fr-t1.1`, qui bloque `fr-t1-binaire` (program.md §6 : la tranche n'est pas finie sans elle).
- Le 29,5 px est systémique (onglets, splits, réglages) : fr-t1.1 doit en tirer une règle réutilisable, notée dans `docs/constantes-rendu.md`.
À trancher : rien encore. L'écart d'ombre (flou de GPUI contre Skia) remontera à l'orchestrateur s'il est irréductible.
**Prochaine action exacte.** Aux comptes rendus de palette et de fr-t1.1 : revue habituelle, puis fusion. Ensuite fr-t1-onglets, dès que workspace (cloud) est fusionné, sous 8 points par jour.

## Vague 6 (palette, arbre, workspace fusionnés) — 10/11/2026 18:50 UTC — usage 5h 32 % / semaine 15 %
Tâches fermées : fr-t1-palette (#29), fr-t1-arbre (#27, cloud), fr-t1-workspace (#28, cloud). Toutes ont la CI verte et un audit à 0 ; `cargo test -p fragment-core` passe (276 + 49).
- #28 était empilée sur #27. Après la fusion de #27, je l'ai rebasée par `git rebase --onto origin/main <tête de #27>`, puis poussée en `--force-with-lease`, CI verte. ★ Cloud : demander une PR par tâche, partie de `main` après la fusion de la précédente, ou accepter ce rebase.
- Palette : boîte, bordures et hauteurs sont exactes. Les glyphes (40 à 90 px de large), l'ombre (environ 108k px), le fond de la ligne sélectionnée (41,41,43 contre 40,41,43, couleur stricte) et l'icône × sont ajoutés au bead fr-t1.1. Requête de `t1-palette` : « panneau » (« sidebar » ne trouvait pas que des commandes du cœur).
En cours : fr-t1.1, élargi aux 4 scènes de palette.
**Part fixe** : cumul environ 7 points ; 10/11 : environ 4 sur 8.
Décisions du chef : fr-t1-onglets et fr-t1-reglages dépendent maintenant de fr-t1.1. La règle des positions fractionnaires (29,5 px) doit exister avant qu'on dessine onglets et réglages, sinon chaque tâche réinvente l'arrondi.
**Prochaine action exacte.** Au compte rendu de fr-t1.1 : revue habituelle ; l'écart d'ombre, s'il est irréductible, va à l'orchestrateur (divergence, §8). Puis fr-t1-onglets, et fr-t1-reglages si le budget du jour le permet (lourds, environ 3,5 points chacun), en équipiers locaux Sonnet. Le contexte du chef approche les 300k : faire le clear (§10) dès que fr-t1.1 est fusionnée, avant de lancer onglets.

## Vague 6 (fin, avant clear du chef) — 10/11/2026 19:15 UTC — usage 5h ≈ 33 % / semaine 15 %
Tâches : fr-t1.1, PR #30 fusionnée (CI verte, audit à 0), bead rouvert et EN ATTENTE de la décision de l'orchestrateur. Détail dans les notes du bead et dans `docs/parite-pixels.md`.
- **Réglé :** le 29,5 px. GPUI arrondit chaque longueur avant la mise en page ; la règle est dans `echelle::calees`/`moitie` (`docs/constantes-rendu.md`, « Positions fractionnaires »). Aussi l'ombre (le flou de GPUI est un écart-type, celui de Skia vaut flou/2) et la poignée de 18 px.
- **Restant :** 0,1 à 1 % de pixels. Glyphes (traits plus fins, delta jusqu'à 190), texte environ 1 % plus large (+1 à 2 px), icônes (delta 54 à 94), bordures (1 niveau, alpha). Les 4 scènes de palette ne sont pas traitées.
Remontée à l'orchestrateur (§8, divergence) : ADR « sas par catégorie » proposé. Strict hors des boîtes de texte et d'icônes, tolérance dedans, largeur du texte à ±1 px, tâche de rasterizeur en phase 4.
Décision du chef : fr-t1-onglets et fr-t1-reglages ne dépendent plus de fr-t1.1, puisque la règle du fractionnaire existe. fr-t1-binaire reste bloquée par fr-t1.1.
Équipiers : aucun actif. Worktrees : aucun hors `main`. Battement annulé, réveil d'après-clear posé (§10). **Part fixe** : cumul environ 8 points ; 10/11 : environ 5 sur 8.
**Prochaine action exacte.**
1. Lancer fr-t1-onglets en équipier local Sonnet, dans un worktree frère `../fragment-rs-onglets` avec son propre `CARGO_TARGET_DIR`. Brief sur le modèle des précédents :
   - règle `echelle::calees` pour toute hauteur fractionnaire ;
   - scènes en fenêtre entière en `"depuis": "T2"`, zone mesurée au rognage sur un premier commit à T1 ;
   - captures au premier plan avec le verrou `C:/dev/fragment-rs-pixels-verrou` ;
   - pas de build local de l'app ;
   - arrêt et notes au bead au-delà de 250k tokens.
2. fr-t1-reglages en parallèle si le budget du jour le permet (8 par jour, lourds).
3. À la réponse de l'orchestrateur sur le sas : ADR, puis tâche xtask (masque de catégories exporté par le candidat), puis reprise de fr-t1.1.
4. ★ Arrêter (`TaskStop`) tout équipier qui a rendu son compte rendu, sinon il continue en tâche de fond.

## Décision de l'orchestrateur, sas — 10/11/2026 18:55 UTC
Le « sas par catégorie » est ACCORDÉ AVEC CONDITIONS, consignées dans ADR-009 :
- strict hors des boîtes de texte et d'icônes, avec l'origine de chaque boîte vérifiée strictement ;
- tolérance d'anticrénelage dans une boîte, sous garde-fous (encre totale bornée, boîte d'encre à ±1 px), avec contre-épreuve rouge obligatoire ;
- icônes traitées comme les glyphes ;
- largeur du texte à ±1 px pour T1 SEULEMENT ;
- rasterizeur en phase 4, avec un cliquet sur le delta max et le pourcentage dans les boîtes.
Beads créés : `fr-t1.2` (outillage du sas, dont dépend désormais fr-t1.1) et `fr-t1.3` (largeur du texte, environ 1 %). ★ fr-t1.3 doit être fermé AVANT T2 : à la création de l'épic fr-t2, faire `bd dep add fr-t2 fr-t1.3`. Le binaire de T1 peut être livré avec cette dette déclarée, à mentionner dans le compte rendu de livraison.
Le clear demandé à 18:49 n'a pas eu lieu : l'arrivée de ce message l'a annulé. Je le refais en fin de ce tour, et le réveil de 19:55 (heure locale) tient toujours.
**Prochaine action exacte** (remplace celle de l'entrée précédente) :
1. fr-t1-onglets en équipier local (voir l'entrée précédente pour le brief).
2. fr-t1.2 en parallèle : xtask et `fragment-app/src/pixels/{capture,mod}.rs`, fichiers disjoints d'onglets.
3. fr-t1-reglages si le budget du jour le permet.
4. fr-t1.3 après fr-t1.2, puis fr-t1.1.
Au plus 3 équipiers.

## Vague 7 (reprise) — 11/11/2026 08:30 UTC — usage 5h 3 % / semaine 16 %
Reprise : le réveil d'après-clear posé le 10/11 n'a PAS survécu (aucune tâche Cron à la reprise). C'est l'orchestrateur qui m'a relancé à 08:20 UTC. ★ Procédure §10 constatée en échec : au prochain clear, revenir à l'ancienne (réveils annulés, « prêt pour le clear » à l'orchestrateur, fin de tour).
Phase : 3, T1. En cours :
- fr-t1-onglets, équipier local Sonnet, worktree `../fragment-rs-onglets`, target `C:/dev/fragment-rs-target-onglets` ;
- fr-t1.2 (sas, ADR-009), équipier local Sonnet, worktree `../fragment-rs-sas`, target `C:/dev/fragment-rs-target-sas`.
Les fichiers des deux tâches sont disjoints. Bloqué : rien.
**Part fixe** : cumul environ 8 points au 10/11. Le 11/11 : 0 sur 8 au lancement, avec environ 6 points prévus (onglets ≈ 3,5, sas ≈ 2,5). La semaine est passée de 15 % à 16 % hors travail.
Battement recréé toutes les 2 h (CronCreate, à la minute 17).
**Prochaine action exacte.** Aux comptes rendus d'onglets et du sas : revue habituelle (diff, audit à 0, CI, fusion, `bd close`, export, retrait du worktree, `TaskStop`). Puis :
- fr-t1-reglages si la part du jour le permet, sinon le 12/11 ;
- fr-t1.3 après fr-t1.2 ;
- reprise de fr-t1.1.
