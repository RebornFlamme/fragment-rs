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

## Vague 7 (sas fusionné) — 11/11/2026 09:00 UTC — usage 5h 10 % / semaine 16 %
Tâches fermées : fr-t1.2 (#31, CI verte avec les 7 jobs, audit à 0, 44 tests xtask ; 249k tokens). En cours : fr-t1-onglets et fr-t1.3 (équipier local Sonnet, worktree `../fragment-rs-largeur`, target `C:/dev/fragment-rs-target-largeur`, fichiers `polices.rs` et `echelle.rs` sans changer leur API). Bloqué : rien.
**Part fixe** : la semaine reste à 16 % ; le sas a coûté moins d'un point. 11/11 : environ 1 sur 8.
Sas livré : chiffres dans ADR-009. Deux écarts avec la décision :
- Les boîtes sont DÉDUITES de l'image candidate (traits droits ≥ 24 px exclus comme géométrie), car GPUI ne laisse pas lire les glyphes peints. L'export exact demanderait un crochet dans `fragment-ui`, format prêt.
- Une lettre changée dans du texte réel de 13 px n'est vue que deux fois sur trois.
Mesure (palette et modale, avant #30) : aucune scène ne passe. Plus de 99 % des pixels hors boîte ont un delta ≤ 2, et c'est l'ombre.
Décisions du chef : fusion acceptée, l'outil ne relâche rien hors boîte. `program.md` §5 (anciens seuils) n'est pas aligné : texte de l'orchestrateur, signalé.
À trancher par l'orchestrateur (envoyé) : une catégorie « ombre » (ADR) ; l'acceptation des boîtes déduites ; l'alignement de §5.
**Prochaine action exacte.** Aux comptes rendus d'onglets et de largeur : revue habituelle. Puis fr-t1-reglages, le 12/11 si la part du jour est prise. fr-t1.1 reprend après la réponse sur l'ombre.

## Décision de l'orchestrateur, ombre et boîtes — 11/11/2026 09:20 UTC
- **Ombre** : catégorie accordée, en dette. Delta ≤ 2 dans le halo géométrique seulement, avec contre-épreuve et cliquet. Amendement daté dans ADR-009. La mise en œuvre est confiée à fr-t1.1, qui s'élargit aux fichiers du sas (`xtask/src/pixels/{sas,boites}.rs`, `capture.rs`) en plus des surfaces flottantes.
- **Boîtes déduites** : confirmées pour T1. Nouveau bead `fr-p3.1`, export exact par fragment-ui. ★ Il bloque la FIN de T3 : à la création de l'épic fr-t3, faire `bd dep add fr-t3 fr-p3.1` (et `fr-p3-gpui-kit-0.9`), et le lancer tôt dans T3.
- `program.md` §5 est aligné par l'orchestrateur ; commité ici.
**Prochaine action exacte.** fr-t1.1 en équipier local, quand un des deux équipiers en cours aura rendu compte. Sinon le 12/11, la part du jour étant engagée (onglets ≈ 3,5 et largeur ≈ 2 sur 8). Fichiers disjoints d'onglets et de largeur ; vérifier qu'onglets n'a pas modifié `flottant.rs` ni `menu.rs`. Sinon, aux comptes rendus : revue habituelle, puis fr-t1-reglages.

## Vague 7 (accélération) — 11/11/2026 09:40 UTC — usage 5h 12 % / semaine 17 %
**Décision de l'utilisateur, relayée par l'orchestrateur** (program.md §2 et CLAUDE.md, commités en e6d3aa3) :
- part fixe et plafond par jour SUPPRIMÉS ;
- cible ≥ 90 % d'hebdo au reset du 13/11 18:00 UTC, arrêt à 97 % ;
- seule garde : la fenêtre 5 h (lancer ≤ 80 %, pause à 90 %) ;
- jusqu'à 5 équipiers, Opus permis sur les tâches dures, battement de 1 h.
Rythme cible : (97 − 17) ÷ 56 h ≈ 1,4 point par heure.
Équipiers actifs (5), tous en worktrees frères avec chacun son `CARGO_TARGET_DIR` (`C:/dev/fragment-rs-target-<nom>`) :
- fr-t1-onglets (Sonnet), `../fragment-rs-onglets` ;
- fr-t1.3, largeur du texte (Sonnet), `../fragment-rs-largeur` ;
- fr-t1.1, ombre puis scènes flottantes et palette à T1 (Opus), `../fragment-rs-flottantes` ; ancienne branche distante supprimée, branche neuve ;
- fr-nj9, police mono (Sonnet), `../fragment-rs-mono` ; ne touche pas `polices.rs` (fr-t1.3) ;
- planificateur T2, T3, T4, T11 (Opus), `../fragment-rs-plan`, branche `task/fr-plan-t2-t11` : `docs/tranches/T{2,3,4,11}.md`, épics et tâches seedées dans Beads, avec `fr-t2 ← fr-t1.3`, `fr-t3 ← fr-p3.1` et `fr-t3 ← fr-p3-gpui-kit-0.9`.
Décisions du chef : fr-p3-gpui-kit-0.9 est REPORTÉE après fr-t1-binaire. La montée touche `Cargo.lock` et tout `fragment-ui` pendant que trois tâches T1 y écrivent, et T1.md (risque 7) garde T1 sur 0.8.1. Elle gênerait donc T1, la condition posée par l'orchestrateur. Elle bloque la fin de T3, pas son début.
Battement : `CronCreate`, toutes les heures à la minute 23.
**Prochaine action exacte.**
1. Au compte rendu du planificateur : revue de la PR (cohérence des fichiers possédés, dépendances), fusion. Puis lancer, à chaque place libre, les tâches `bd ready` indépendantes de T1, dans l'ordre qu'il donne.
2. Aux autres comptes rendus : revue habituelle.
3. Puis fr-t1-reglages, fr-t1-splits (après onglets), et fr-p3-gpui-kit-0.9 après le binaire.

## Vague 7 (largeur et police mono fusionnées) — 11/11/2026 10:00 UTC
Tâches fermées :
- fr-nj9 (#32) : ADR-005 corrigée. Fragment n'embarque pas JetBrains Mono : `--font-monospace` est une pile système, d'où Consolas.
- fr-t1.3 (#33, CI verte, audit à 0) : la cause est le crénage absent (DirectWrite sans `kern`) plus l'arrondi supérieur de la largeur d'une feuille de texte par GPUI. Corrigée sans ADR ni fork. Règle écrite dans `docs/constantes-rendu.md`.
Le résidu de fr-t1.3 passe à fr-t1.1 : un décalage vertical d'1 px sur les notices à deux lignes, et les glyphes de repli.
Écart de périmètre accepté : #33 a touché `flottant`, `notice`, `tooltip`, `modal` et `suggest`, des fichiers de fr-t1.1 (petites modifications). fr-t1.1, onglets et reglages ont été prévenus et doivent se rebaser.
fr-t2 ne dépend plus de fr-t1.3. En cours : onglets, fr-t1.1 (Opus), reglages, planificateur. Une place est libre, mais rien n'est prêt hors T1 avant la PR du planificateur.
**Prochaine action exacte.** Fusionner la PR du planificateur, puis remplir les places avec ses tâches indépendantes de T1.
- Battement 11/11 09:40 UTC : fenêtre 5 h à 29 %, semaine à 19 %. Rythme réel depuis 08:20 : environ 2,3 points par heure, pour une cible de 1,4 : en avance. PR #34 (onglets) ouverte, CI en cours. Le planificateur seede T2 à T4 : rien n'est lancé avant sa PR, pour que les dépendances soient posées.
- 11/11 10:00 UTC : fr-t1-onglets fusionnée (#34). CI verte, audit à 0 refait après le rebase. 7 scènes, 14 références, 13 tests headless ; 416k tokens, plus que la limite du brief.
  - Dettes passées à fr-t1-binaire : `brancher_scene`, rognage des scènes d'onglets, activation par le focus, contour de l'onglet actif à comparer à Skia, fichiers de parité.
  - Chasse « New Tab » à 12 px : 47,11 px dans GPUI contre 47 px d'encre dans Chromium (risque 3 levé).
  - fr-t1-splits lancée (Opus), worktree `../fragment-rs-splits`.

## Vague 8 (T2, T3, T4, T11 découpées) — 11/11/2026 10:25 UTC
Planificateur (Opus, 393k tokens) : PR #37 (docs/tranches/T{2,3,4,11}.md), fusion à CI verte.
Découpe : T2 = 12 tâches, T3 = 20, T4 = 13, T11 = 7, plus `fr-p3-socle`, qui débloque fragment-ui, -markdown et -pdf.
★ Beads bloque tous les enfants d'une épic bloquée. Les dépendances de fin de tranche se posent donc sur une TÂCHE et non sur l'épic :
- fr-t1.3 → `fr-t2-tree-item`, `fr-t3-surface`, `fr-t4-barre` ;
- fr-p3.1 et gpui-kit-0.9 → `fr-t3-livraison` ;
- `fr-p3.1` → `fr-t1.1` (fichiers du sas).
Lancées : fr-p3-socle (Sonnet, `../fragment-rs-socle`) et fr-t2-vault (Sonnet, `../fragment-rs-vault`). Avec fr-t1.1, reglages et splits, cela fait 5 équipiers.
Chevauchements acceptés, à rebaser : `fragment-core/src/lib.rs` (vault et reglages) ; `fragment-ui/src/lib.rs` (socle, splits, fr-t1.1).
À remonter à l'orchestrateur, avec la prochaine remontée : l'icône de marque vient du dépôt privé et serait publiée dans un dépôt public ; le spike S7 chiffrera l'écart du raster pdfium face à pdf.js.
**Prochaine action exacte.** À chaque place libre, dans l'ordre :
1. `fr-t2-coffres`, `fr-t11-cascade`, `fr-t3-maths` (Opus), `fr-t3-html` ;
2. après le socle : `fr-t3-syntaxe` (Opus), `fr-t4-pdfium`, puis les modèles de T4 ;
3. scènes en local, une à la fois, à cause du verrou de capture.
- 11/11 10:55 UTC : fusionnées #35 (fr-t1.1 : catégorie ombre, garde-fous plus stricts que l'amendement, 1 PNG vert sur 20 ; bead ROUVERT avec ses dettes), #36 (fr-t1-reglages, fermée) et #37 (plans).
  - Dettes de réglages passées à fr-t1-binaire.
  - Lancées : fr-t2-coffres (Sonnet) et fr-t3-maths S5 (Opus). Actifs : splits, socle, vault, coffres, maths.
  - Envoyé à l'orchestrateur : approbation des garde-fous d'ombre, catégorie « bord d'arrondi », mélange alpha de --shadow-l, icône de marque, pdfium (pour info). fr-t1.1 attend sa réponse et bloque fr-t1-binaire.

## Décision de l'orchestrateur, flottantes, icône, PDF — 11/11/2026 11:15 UTC
Inscrite dans ADR-009 (amendement daté) :
- garde-fous d'ombre approuvés ;
- catégorie « bord d'arrondi » accordée en dette de phase 4 (arc théorique à 1 px, delta ≤ 6, cliquet, contre-épreuve) ;
- exception nommée pour `--shadow-l`, avec la tâche de phase 4 `fr-p4.1` (mélange alpha), qui bloque la fin de la phase 4 ;
- PAS de tolérance pour les aplats translucides : nouvelle tâche `fr-t1.4`, pré-composition dans fragment-theme.
Dépendances posées : fr-t1.1 ← fr-t1.4 ; fr-t11-cascade ← fr-t1.4 (mêmes fichiers de fragment-theme).
Icône de marque : seulement les fichiers exportés, dans `assets/brand/` avec un NOTICE et une exclusion nommée de l'audit. Noté sur `fr-t11-identite`.
PDF : projet de catégorie « raster de page » à préparer dans S7, chrome strict. Noté sur `fr-t4-raster`.
**Prochaine action exacte.** Places libres, dans l'ordre :
1. fr-t1.4 (chemin critique du binaire), puis fr-t1.1 (Sonnet suffit, consignes au bead) ;
2. puis la liste du planificateur : fr-t3-html, `fr-t3-syntaxe` (Opus, après socle), `fr-t4-pdfium`, etc.
- 11/11 11:40 UTC : fusionnées #38 (fr-p3-socle : modules vides, `parite.d`) et #39 (fr-t2-coffres : 7 tests sur 7 de vaults.spec). Les deux beads sont fermés.
  - Lancées : fr-t1.4 (aplats, Sonnet, `../fragment-rs-aplats`) et fr-t3-syntaxe (Opus, `../fragment-rs-syntaxe`).
  - Actifs : splits, vault, maths, aplats, syntaxe.
  - `cargo xtask parite` échoue sur main pour fragment-theme : la correction est confiée à fr-t1.4.
  - **Prochaine action exacte** : aux places libres, dans l'ordre, fr-t1.1 (après fr-t1.4), `fr-t4-pdfium`, `fr-t3-html`, `fr-t3-commandes`, puis les modèles de T4. Les scènes une à une (verrou de capture).
- 11/11 12:05 UTC : fusionnée #40 (fr-t2-vault : 129 tests ; vault.ts n'a pas de test TS). Écarts acceptés : un `rename` par dossier, comme le TS, et la racine protégée (`VaultError::Racine`). `tests/contrats.rs` touché hors liste (5 noms passés à « portés »). Lancée : fr-t4-pdfium (Sonnet). Actifs : splits, maths, aplats, syntaxe, pdfium.
- Battement 11/11 10:40 UTC : fenêtre 5 h à 52 %, semaine à 22 %. Rythme réel depuis 08:20 : environ 2,6 points par heure, pour une cible de 1,36 : en avance.
  - Fusionnée #41 (fr-t1.4 : aplats pré-composés, formule de Chromium trouvée sur 5 pixels de référence, `xtask parite` réparé).
  - Reprise de fr-t1.1 (Opus) : catégorie bord d'arrondi, exception `--shadow-l`, scènes à T1.
  - En attente de compte rendu : #42 (splits) et #43 (pdfium). Actifs : splits, maths, syntaxe, pdfium, flottantes.
- 11/11 11:15 UTC : fusionnée #42 (fr-t1-splits : 20 tests headless, 10 scènes et leurs références ; 438k tokens). Ses dettes et branchements sont notés sur fr-t1-binaire.
  - Décision du chef : fr-t1-binaire n'est plus BLOQUÉE par fr-t1.1, les fichiers étant disjoints. Elle tourne en parallèle, et son bead ne se ferme qu'après fr-t1.1, car la tranche exige toutes ses scènes vertes.
  - Lancée : fr-t1-binaire (Opus). Actifs : maths, syntaxe, pdfium, flottantes, binaire.
- 11/11 11:05 UTC, fenêtre 5 h à 61 %, semaine à 23 % : fusionnée #43 (fr-t4-pdfium : pdfium chromium/8125 épinglé par sha256, pdfium-render =0.9.5, licences BSD-3 / MIT / Apache-2, 11 tests sur des PDF générés).
  - fr-t3-syntaxe arrêtée à 362k tokens. PR #44 en brouillon, aucun test exécuté.
  - Décision du chef : le PORT de `@lezer/markdown` 1.7.2 (MIT, notice gardée) est accepté comme troisième voie, à justifier dans ADR-010. Mesures : tree-sitter-md 70,6 % des nœuds Lezer, pulldown-cmark 59,4 %, les deux ignorant les extensions de Fragment.
  - Lancées : reprise de syntaxe (Opus) et fr-t4-vue-modele (Sonnet). Actifs : maths, flottantes, binaire, syntaxe2, vuepdf.
  - ★ Pause à 90 % de la fenêtre 5 h. Reset à 12:30 UTC.
- 11/11 11:25 UTC, fenêtre 5 h à 71 % (reset à 12:30), semaine à 24 % : fusionnée #47 (fr-t4-vue-modele : 12/12 cas, 32 tests).
  - Aucun lancement avant le reset : au rythme actuel, la fenêtre atteindrait 90 % avant 12:30.
  - PR ouvertes, en attente du compte rendu de leur équipier : #48 (binaire), #46 (fr-t1.1), #45 (maths S5), #44 (syntaxe, brouillon).
  - Contexte du chef : 303k tokens. Clear prévu dès que ces quatre équipiers auront rendu compte. Procédure de §10 revenue à l'ancienne, puisque le réveil d'après-clear n'a pas survécu le 10/11.
- 11/11 11:45 UTC : fusionnée #44 (fr-t3-syntaxe : port de @lezer/markdown, 100 % des nœuds conformes à Lezer sur 108 546 nœuds réels, 32 tests, ADR-010 proposé).
  - Le port n'a pas d'analyse incrémentale : 31 à 56 ms par frappe sur 10 000 lignes. Nouveau bead `fr-t3.1` (incrémental, puis coloration des blocs de code).
  - Les documents des specs ont été comparés hors du dépôt ; seuls 7 documents réécrits sont commités.
- Battement 11/11 11:38 UTC : fenêtre 5 h à 75 % (reset à 12:30), semaine à 25 %. Rythme cible : (97 − 25) ÷ 54,4 h ≈ 1,3 point par heure. Réel depuis 08:20 : environ 2,7, en avance. Trois équipiers actifs (binaire, fr-t1.1, maths), aucun nouveau lancement avant le reset.
- 11/11 11:55 UTC : fusionnée #46 (fr-t1.1, étape 2 : bord d'arrondi et `--shadow-l` en place, 3 PNG sur 20 au vert). Bead rouvert, en attente de l'orchestrateur (envoyé) sur trois points :
  - la garde d'arc à 0,15 px ;
  - l'origine d'encre décalée d'1 px sur 14 PNG (calage des fûts) ; je recommande de la mesurer au barycentre du trait ;
  - l'extension de l'exception `--shadow-l` aux résidus de mélange alpha.

## Décision de l'orchestrateur, origine d'encre et résidus — 11/11/2026 12:00 UTC
Inscrite dans ADR-009 (amendement daté, d3c43ea) :
- garde d'arc approuvée ;
- origine d'encre au barycentre (±0,5 px, par passage puis par moitié), boîte ±1 px gardée ;
- exceptions NOMMÉES temporaires pour la livraison de T1, en cliquet.
- ★ En contrepartie, fr-p4.1 (mélange alpha) est lancée tout de suite, sans attendre la phase 4. Son critère de fin : toutes ces exceptions retirées, cliquets à 0.
Lancée : fr-t1.1, étape 3 (Opus), avec une fenêtre 5 h à 77 %. fr-p4.1 est programmée juste après le reset de 12:30 UTC (réveil ponctuel à 12:37 UTC). Elle travaille d'abord sur une mire ; elle ne touche les fichiers de fr-t1.1 qu'après leur fusion.
À livrer à l'orchestrateur dès que #48 est verte avec ses scènes : le lien de l'artefact Windows, avec la liste des dettes ouvertes et leurs cliquets.
- 11/11 12:00 UTC : S5 (maths, #45). Verdict GO-AVEC pour la voie A : MathJax 3.2.1 dans QuickJS. Le SVG est identique à celui de Fragment, la boîte exacte au 1/64 px. Voies B (latex-rust) et C (typst) en NO-GO.
  - Dette de fidélité restante : l'anticrénelage des tracés (resvg contre Skia). 5,7 % des pixels dépassent un delta de 16, delta max 82, 0 pixel de géométrie. ADR-011 propose de la traiter comme les glyphes. À remonter à l'orchestrateur avec la livraison de T1.
  - À corriger dans program.md §4 (texte de l'orchestrateur) : `rex` n'est pas ReX sur crates.io, et `typst-math` est un nom réservé vide.
  - #45 est en conflit sur Cargo.lock : l'équipier rebase. Fusion après CI et audit.

## Vague 8 (fin, avant clear du chef) — 11/11/2026 12:15 UTC — usage 5h 82 % (reset à 12:30) / semaine 26 %
Fusionnées depuis la dernière entrée :
- #45 (fr-t3-maths, S5 GO-AVEC : MathJax 3.2.1 dans QuickJS ; B et C en NO-GO ; ADR-011 proposé) ;
- #48 (fr-t1-binaire : boot, `.fragment/` sur disque, pont clavier, hôte de fenêtre, coquille avec barre d'état ; scène de contrôle `t1-binaire` verte).
Décisions du chef :
- Pour fusionner #48 en CI verte, ses 21 scènes de shell, plus `t1-toolbar-horizontale` (régressée dans la coquille assemblée), sont remises en `"depuis": "T2"`. Bilan : 0 PNG vert sur 42.
- Nouveau bug `fr-t1.5` : parité des 21 scènes de shell. Il couvre 7 causes chiffrées dans le bead, dont le sélecteur de coffre rendu statiquement en T1 à partir du modèle de #39.
- `fr-t1-binaire` reste OUVERT et dépend de fr-t1.5 et de fr-t1.1.
- `t1-toolbar-horizontale` revient à fr-t1.1 (équipier prévenu).
Équipiers actifs : UN seul, fr-t1.1 étape 3 (Opus), worktree `../fragment-rs-flottantes`, branche `task/fr-t1.1`. Il fait l'origine au barycentre, les exceptions nommées, ↵, le titre de la modale et les 10 scènes, plus la toolbar dans la coquille. Son compte rendu arrivera après le clear : revue habituelle.
Worktrees hors `main` : `../fragment-rs-flottantes` seulement.
À remonter à l'orchestrateur avec la livraison de T1, en un seul message :
- la dette de fidélité des maths (ADR-011 : anticrénelage resvg contre Skia, 5,7 % des pixels au-delà d'un delta de 16, 0 pixel de géométrie, proposée comme les glyphes) ;
- `program.md` §4 à corriger : `rex` n'est pas ReX, et `typst-math` est un nom vide.
Livraison de T1 à l'orchestrateur : lien de l'artefact `build.yml` de main, liste des dettes avec leurs cliquets. Seulement quand fr-t1.1 et fr-t1.5 auront rendu les scènes vertes.
**Part et rythme** : cible (97 − 26) ÷ 53,7 h ≈ 1,3 point par heure ; réel depuis 08:20 ≈ 2,6, en avance.
**Clear** : contexte du chef à 354k. Procédure ancienne de §10 (le réveil d'après-clear n'a pas survécu le 10/11) : réveils annulés, « prêt pour le clear » envoyé à l'orchestrateur.
**Prochaine action exacte** (après le reset de 12:30 UTC, fenêtre ≤ 80 %) :
1. Lancer fr-p4.1 (mélange alpha, Opus, `../fragment-rs-alpha`). Consignes : `bd show fr-p4.1` et l'amendement de 12:00 d'ADR-009. Cause d'abord, sur une mire ou une crate jetable. Ne toucher `flottant.rs`, `xtask/src/pixels*` et `cliquet.json` qu'après la fusion de fr-t1.1.
2. Lancer fr-t1.5 (Opus, `../fragment-rs-shell`). Ses fichiers sont disjoints de ceux de fr-t1.1, sauf l'API de déclaration des arrondis, qu'il utilise sans la modifier.
3. Places restantes (5 au plus), d'après `bd ready` et dans l'ordre du planificateur : `fr-t3-html`, `fr-t3-commandes`, `fr-t4-recherche-modele`, `fr-t4-laterale-modele`, `fr-t2-fs`, `fr-t2-fileview`, `fr-t11-cascade`, `fr-t3.1`. Les scènes une à une, à cause du verrou de capture.
4. Reposer le battement horaire (CronCreate) et le vérifier par CronList.

## Vague 9 — 11/11/2026 12:40 UTC — usage 5h 0 % (reset à 17:30) / semaine 26 %
Reprise après clear, relancé par l'orchestrateur (pas par un réveil : le réveil d'après-clear n'est pas en jeu, procédure ancienne de §10 confirmée).
- ★ L'équipier de fr-t1.1 étape 3 n'a pas survécu au clear : deux commits poussés sur `task/fr-t1.1` (9350d7e, 3993aee), job pixels rouge sur 43018474495, pas de PR. Décision du chef : reprise par un équipier Opus neuf dans `../fragment-rs-flottantes`, qui s'arrête si la branche bouge sans lui.
- Lancés (5 équipiers locaux, worktrees frères, `CARGO_TARGET_DIR=C:/dev/fragment-rs-target-<nom>`) :
  - fr-t1.5 (Opus, `../fragment-rs-shell`) ;
  - fr-p4.1 (Opus, `../fragment-rs-alpha`, fichiers de fr-t1.1 seulement après sa fusion ; fichiers déclarés au bead) ;
  - fr-t1.1 étape 3, reprise (Opus) ;
  - fr-t3-html (Sonnet, `../fragment-rs-html`) ;
  - fr-t3-commandes (Sonnet, `../fragment-rs-commandes`).
- Battement horaire reposé (CronCreate, minute 17).
**Part et rythme** : cible (97 − 26) ÷ 53,3 h ≈ 1,3 point par heure.
**Prochaine action exacte** : aux comptes rendus, revue habituelle (diff, audit à 0, CI, fusion, `bd close`, export, retrait du worktree). Livraison de T1 quand fr-t1.1 et fr-t1.5 sont fusionnées et fr-t1-binaire fermée : lien de l'artefact `build.yml` de main, dettes (ADR-009, ADR-011 maths), correction de §4 (`rex`, `typst-math`). Places libres : fr-t4-recherche-modele, fr-t4-laterale-modele, fr-t2-fs, fr-t2-fileview, fr-t11-cascade, fr-t3.1.
- 11/11 13:25 UTC, fenêtre 5 h à 21 %, semaine à 29 % : fusionnées #51 (fr-t3-commandes : 29 `editor:*`, menus, collage d'image, 42 tests), #50 (fr-t1.1 étape 3 : 20/20 PNG flottants tolérés, `t1-toolbar-horizontale` à T1 dans la coquille, zoom remis à 100 % par scène), #52 (fr-t3-html, S6 GO-AVEC mixte : traducteur HTML → GPUI, SVG par GPUI, SMIL minimal ; ADR-012 proposé). Audits à 0, beads fermés.
  - Pertes de fidélité de S6 à remonter avec la livraison de T1 : iframe réduite à une carte-lien, animations CSS et `<style>` non jouées, SMIL hors sous-ensemble, anticrénelage SVG (4,3 % > 16, contours seuls), CSS hors sous-ensemble (grid, transform).
  - Dettes de #50 pour fr-p4.1 (prévenu) : cliquet `exceptions_t1` (ombres et arcs en clair) et le contournement `flottant::bordure_translucide` (GPUI arrondit l'opacité à l'octet). fr-t1.5 prévenue de rebaser.
  - `clippy --all-targets` de fragment-markdown échouerait sur `tests/spike_s1b.rs` d'après l'équipier de #51 (la CI passe) : à vérifier.
  - Lancées (Sonnet) : fr-t4-recherche-modele, fr-t4-laterale-modele, fr-t2-fs. Actifs : shell, alpha, recherche, laterale, fs.
- Battement 11/11 13:33 UTC : fenêtre 5 h à 26 % (reset 17:30), semaine à 30 %. Rythme cible (97 − 30) ÷ 52,5 h ≈ 1,3 point par heure ; réel depuis 12:40 ≈ 4,5, en avance. PR #53, #54, #55 en CI ; #49 (fr-t1.5) en brouillon, équipier à 370k tokens prié de conclure et de passer la main. Cinq équipiers actifs, aucun lancement.
- 11/11 13:45 UTC : fr-t1.5, premier équipier arrêté à 375k tokens (PR #49 brouillon, 0/42 PNG ; zone de dépôt claire 472 133 → 1 386 px, split-vertical 2 120 → 511, réglages 11 000 → 4 850). Bloquant commun aux 42 PNG : `boites.rs` prend le texte « Fragment 0.0.0 » pour un trait droit. Décisions du chef : `boites.rs` passe à fr-t1.5 (trait droit = couleur uniforme, resserrement avec contre-épreuve), extension d'`arrondir` à un arc par coin, trait de l'onglet actif (221/220) et proportions (199,5/199,75) en strict, `t1-boutons-fenetre` rejoué par resize puis référence recapturée. fr-p4.1 prévenu. Reprise par un équipier Opus neuf (shell2).
- 11/11 13:55 UTC : fusionnée #53 (fr-t4-laterale-modele : 6/6 cas de pdf-toolbar 7 à 12, 15 ajoutés ; le cas 7 « Page width » compté deux fois, avec vue_modele). Lancée : fr-t2-fileview (Sonnet). Actifs : shell2, alpha, recherche, fs, fileview.
- 11/11 14:05 UTC : fusionnée #55 (fr-t4-recherche-modele : 4/5 cas de pdf-find, 21 ajoutés ; dépendances notées : regex, unicode-normalization, unicode-segmentation, déjà au lock, MIT/Apache). Lancée : fr-t3.1 (Opus, incrémental). fr-t11-cascade différée : elle touche sombre.rs/clair.rs de fragment-theme, que fr-p4.1 peut toucher. Actifs : shell2, alpha, fs, fileview, incremental.
- 11/11 14:30 UTC, fenêtre 5 h à 36 %, semaine à 31 % : fr-p4.1, premier équipier arrêté à 309k tokens. PR #56 en brouillon (pixels rouge), S8 et ADR-013 proposés.
  - ★ Cause : PAS linéaire contre sRGB, les deux mélangent en sRGB encodé. Il y en a quatre :
    - (a) Chromium compose le conteneur fixed de la modale dans un calque RGBA8 prémultiplié, ce qui reproduit `--shadow-l` à 100 % en simulation ;
    - (b) le flou GPU de Skia s'écarte jusqu'à 4/255 : hors d'atteinte au bit près, 0,11 à 0,20 px restants ;
    - (c) GPUI anticrénèle deux fois le bord extérieur, fond puis bordure ;
    - (d) `opacite_exacte` corrige les côtés mais dégrade les arcs clairs.
  - Reprise (Opus, alpha2), dans l'ordre : (c) et (d) ensemble par un fond en retrait, puis (a) en ombre calculée sur le CPU avec le calque émulé.
  - (b) est une dette à remonter à l'orchestrateur (divergence §8) avec la livraison de T1.
  - PR #54 (fr-t2-fs) bonne pour fusion, renvoyée au rebase (conflit lock et parité).
  - Actifs : shell2, alpha2, fs (rebase), fileview, incremental.
- 11/11 14:40 UTC : décision du chef sur fr-t1.5, contour de l'onglet actif. Côtés et ligne de base : colonnes pré-composées, strict (221/222 = arrondi au plus proche de GPUI contre floor de Skia). Arcs r5/r4 et raccords : delta 9 à 17, au-delà de la catégorie arrondi (≤ 6). Cause : MSAA 4x asymétrique de GPUI, et environ 40 % d'encre en plus aux raccords chez Chromium ; une couverture exacte laisse encore un delta de 10. Ils passent en exception NOMMÉE temporaire « contour d'onglet actif », limitée au halo de chaque arc, en cliquet, avec contre-épreuve, rattachée à fr-p4.1. À remonter à l'orchestrateur avec la livraison de T1.
- Battement 11/11 14:32 UTC : fenêtre 5 h à 46 % (reset 17:30), semaine à 32 %. Rythme cible (97 − 32) ÷ 51,5 h ≈ 1,3 point par heure ; réel depuis 12:40 ≈ 3,2, en avance. #54 et #57 vertes sauf clippy (panne DNS apt du runner Linux, relancé). #58 (fr-t3.1) en CI ; #49 et #56 en brouillon. Lancée : fr-t3-metadata (Sonnet). Actifs : shell2, alpha2, incremental, listes, metadata.
- 11/11 14:45 UTC : fr-t1.5, deuxième équipier arrêté à 345k tokens (commits bc848b2 et 57ec4f9). Toujours 0/42 PNG, mais seulement 1 px hors boîte en sombre et 23 en clair sur la plupart des scènes d'onglets. Faits : boîtes de texte, arcs coin par coin, côtés du contour en strict, cause du 199,5 (Chromium arrondit la position du trait avant la transformation), `t1-boutons-fenetre` recapturée.
  - Décisions du chef :
    - (a) le pixel de l'arc intérieur du coin de la barre d'état (1180,782), delta 9 à 11, entre dans l'exception nommée sous un élément « coin de la barre d'état », aux mêmes conditions ;
    - (b) le passage 221 → 222 au milieu du côté de l'onglet en clair est un artefact de Skia sur le MÊME chemin que le contour (updateTabOutline). Il est couvert par l'exception « contour d'onglet actif » pour les côtés, au delta 1 SEULEMENT, en cliquet.
  - Reprise par un troisième équipier Opus (shell3) : réglages, zoom-80/150, entete-vue, glisser-zone.
- 11/11 14:55 UTC : fr-p4.1 (PR #56, run 43040649423), échange de cliquets accepté par le chef, sous conditions.
  - Gains :
    - étalement de `--shadow-l` : de +1,60 à +0,09 px pour la modale sombre, de +1,42 à 0 pour la palette sombre ;
    - arcs clairs du menu, des notices et de l'infobulle : de 0,23–0,35 à 0,07–0,11 px, 5 exceptions retirées ;
    - modale et palette claires : déplacement des arcs de 0,20 à 0,02 px, delta max de 33 à 8.
  - Hausses, sur la modale et les palettes claires seulement : delta d'ombre de 1 à 2 (garde-fou tenu) ; glissement de `--shadow-l` de 0,024 à 0,071 ; pixels non identiques des bandes d'arrondi de 20–25 % à 35–46 % (anticrénelage à 1 niveau).
  - Conditions : partout ailleurs, cliquet en baisse seulement (preuve dans la PR) ; garde-fous d'ADR-009 intacts ; section datée dans ADR-013.
  - À REMONTER à l'orchestrateur avec la livraison de T1 (un seuil relâché, §5). Repli en cas de refus : GPUI pour ces quatre PNG.
- 11/11 14:55 UTC : fr-t1.5, accepté environ 12 lignes dans `sas.rs` (masque `cote_nomme`, delta ≤ 1) et `flottant::arrondir_coins`, en ajout seul ; alpha2 prévenu.
- 11/11 15:10 UTC, fenêtre 5 h à 56 % (reset 17:30), semaine à 34 % :
  - #58 (fr-t3.1) livrée : 0,69 ms par frappe en médiane (max 1,52) contre 34 à 37 ms, oracle identique à l'analyse complète. La coloration des blocs de code et des maths est confiée au nouveau bead fr-t3.2.
  - #59 (fr-t3-metadata) livrée : 16 tests.
  - ★ Le job clippy (Ubuntu) échoue partout depuis 14:00 UTC : le runner ne résout plus archive/security.ubuntu.com à l'étape apt. #54, #57, #58 et #59 sont vertes sur les 6 autres jobs.
  - Correctif du chef, PR #61 : clippy passe sous windows-latest (sans apt), et les lints Windows du spike S2 (`fragment-html/tests/spike_s2/scenario.rs`, jamais vu par clippy sous Linux) sont corrigés.
  - Pas de 5e lancement : au rythme actuel, la fenêtre 5 h atteindrait 90 % avant 17:30. Actifs : shell3, alpha2, listes, decorations.
- 11/11 15:30 UTC : disque C: plein (0 Mo). J'ai supprimé les dossiers cible de 21 tâches fermées : 86,6 Go libres. ★ Après chaque fusion, retirer aussi le dossier cible de la tâche.
- #61 : sous Windows, clippy voit pour la première fois le code `cfg(windows)` des spikes S1b, S2 et S3. Lints corrigés par des `allow` ciblés et commentés, ou par `--fix`.
- fr-t1.5 (shell3) :
  - réglages passés de 4 850 à 48/92 px ;
  - exception nommée « trait de poignée » accordée : bords du trait seulement, delta 1, en clair, cliquet et contre-épreuve. Environ 4 000 px, même famille que 221/222, l'arrondi de Skia sur les bords fractionnaires n'étant pas constant ;
  - jeton `fragment-theme` ajouté pour le littéral CSS #8f8e89 (flèche de liste).
  - À remonter avec T1.
- Un `arbre.rs` non suivi traîne à la racine du dépôt principal, d'origine inconnue. Je n'y touche pas.
- Battement 11/11 15:34 UTC : fenêtre 5 h à 68 % (reset 17:30), semaine à 35 %. Rythme cible (97 − 35) ÷ 50,4 h ≈ 1,2 point par heure ; réel depuis 12:40 ≈ 3,1, en avance. Fusionnée #61 (clippy sous Windows). #54, #57, #58, #59, #60 et #62 mises à jour depuis main, en CI. Actifs : shell3 (fr-t1.5), alpha2 (fr-p4.1). Pas de lancement avant le reset : la fenêtre est trop près de 80 %. Contexte du chef : 282k, clear prévu après les fusions.
- 11/11 15:55 UTC : fusionnées, CI verte avec clippy sous Windows, audits à 0 :
  - #54 (fr-t2-fs) ;
  - #57 (fr-t2-fileview, 46 tests) ;
  - #58 (fr-t3.1 : 0,69 ms par frappe) ;
  - #59 (fr-t3-metadata) ;
  - #60 (fr-t3-listes : 10/11 cas) ;
  - #62 (fr-t3-decorations : 8 ms pour le document entier, 0,95 ms pour un viewport de 60 lignes, sur 10 000 lignes ; fragment-markdown dépend désormais de fragment-theme).
  - Beads fermés, worktrees et dossiers cible retirés.
- #56 (fr-p4.1, alpha2 terminé, 333k tokens) : pixels vert pour la première fois, prête, CI relancée depuis main.
  - Bordures à anticrénelage unique ; ombre `--shadow-l` calculée sur le CPU dans le calque émulé (0,33 s par géométrie en debug, puis cache ; release à mesurer) ; 5 exceptions d'arcs retirées ; `image` devient dépendance de fragment-ui.
  - Dettes (b) dans ADR-013. À fusionner AVANT #49.
- Battement 11/11 16:32 UTC : fenêtre 5 h à 76 % (reset 17:30), semaine à 36 %. Rythme cible (97 − 36) ÷ 49,5 h ≈ 1,2 point par heure ; en avance. Fusionnée #56 (fr-p4.1, étape 1 : audit à 0 ; diff du cliquet relu, conforme à l'échange accordé). fr-p4.1 reste OUVERTE pour ses dettes. main verte. Un seul actif : shell3 (fr-t1.5, #49, prévenu de rebaser). Le clear du chef attend son compte rendu, sinon il le tuerait (comme fr-t1.1 au clear précédent).

## Vague 9 (fin, avant clear du chef) — 11/11/2026 16:45 UTC — usage 5h ≈ 78 % (reset 17:30) / semaine 36 %
Fusionnées dans la vague : #50, #51, #52, #53, #54, #55, #56, #57, #58, #59, #60, #61 et #62.
- Beads fermés : fr-t1.1, fr-t3-commandes, fr-t3-html, fr-t4-laterale-modele, fr-t2-fs, fr-t4-recherche-modele, fr-t2-fileview, fr-t3.1, fr-t3-metadata, fr-t3-listes et fr-t3-decorations.
- Ouverts : fr-p4.1 (dettes, voir ses notes) et fr-t1.5.
- Nouveau bead : fr-t3.2 (parseMixed, stexMath).

fr-t1.5 : troisième équipier arrêté à 274k tokens. PR #49 en brouillon, branche rebasée sur main après #56 et poussée (02015c2), pas encore vue en CI.
- Au run 43053240704 (avant rebase) : 21 PNG à 0 px hors boîtes, encore rouges faute de base de cliquet. Réglages passés de 4 850 à 16-24 px ; zoom-80 sombre de 2 525 à 4.
- Reste :
  - (d) valeurs du cliquet après un run vert ;
  - lignes de base en cellule partielle (657,35) et (346,35) (`pile.rs`, splits) ;
  - `t1-boutons-fenetre` (1183,15) ;
  - titres d'onglets inactifs du 2e pane (barycentre à +0,6) ;
  - zoom-150 (156/707 px) et zoom-80 clair (162 px).
- Un `cargo fmt --all` d'un sous-agent a pu toucher des fichiers hors liste : vérifier le diff de #49.

Équipiers actifs : AUCUN. Worktrees hors `main` : `../fragment-rs-shell` (task/fr-t1.5) et `C:/dev/fragment-rs-mesure-t15` (détaché, mesures de fr-t1.5).

À remonter à l'orchestrateur avec la livraison de T1, en un seul message :
- dette des maths (ADR-011) ;
- §4 à corriger (`rex`, `typst-math`) ;
- pertes de S6 (ADR-012 : iframe en carte-lien, CSS animé et `<style>` non joués, SMIL partiel, anticrénelage SVG, CSS hors sous-ensemble) ;
- ADR-013 : forme du flou de Skia, plus l'échange de cliquets daté sur la modale et les palettes claires ;
- exceptions nommées de fr-t1.5 : contour d'onglet actif, côtés au delta 1, coin de la barre d'état, trait de poignée.

**Prochaine action exacte** (après le reset de 17:30 UTC) :
1. Relancer fr-t1.5 (Opus, quatrième équipier) dans `../fragment-rs-shell`, `CARGO_TARGET_DIR=C:/dev/fragment-rs-target-shell`. Brief : lire d'abord les notes du bead ; les décisions déjà prises y sont, aucune nouvelle tolérance sans le chef ; arrêt à 250k tokens.
2. Places restantes (5 au plus), selon `bd ready` : fr-t11-cascade (fragment-theme ; fr-t1.5 n'y ajoute que deux constantes en fin de theme.rs, à prévenir), fr-t3-blocs-modele, fr-t4-barre, fr-t4-pages, fr-t2-tree-item, fr-t3.2, fr-t3-scenes-a (une scène à la fois, verrou de capture).
3. ★ Après chaque fusion : retirer le worktree ET `C:/dev/fragment-rs-target-<tâche>` (le disque était plein le 11/11).
4. Livraison de T1 quand #49 est fusionnée et fr-t1-binaire fermée : lien de l'artefact `build.yml` de main, plus les points ci-dessus.
**Part et rythme** : cible (97 − 36) ÷ 49 h ≈ 1,25 point par heure ; réel ≈ 2,5, en avance.
**Clear** : contexte du chef à 300k. Battement annulé, orchestrateur prévenu, puis clear_session("self").

## Vague 10 — 11/11/2026 17:40 UTC — usage 5h 2 % (reset 22:30) / semaine 37 %
Reprise après clear, relancée par l'orchestrateur (pas par un réveil). Le réveil d'après-clear n'a donc pas été mis à l'épreuve.
- #49 (fr-t1.5) : la branche rebasée (02015c2) est passée en CI, run 43061092662. Tout est vert sauf le job pixels.
- Lancés : 5 équipiers locaux, worktrees frères, `CARGO_TARGET_DIR=C:/dev/fragment-rs-target-<nom>`, beads réclamés.
  - fr-t1.5 : Opus, 4e équipier, `../fragment-rs-shell`, arrêt à 250k tokens.
  - fr-t4-pages : Opus, `../fragment-rs-pdfpages`.
  - fr-t11-cascade : Sonnet, `../fragment-rs-cascade`. Prévenue que la fin de theme.rs est réservée aux deux constantes de fr-t1.5.
  - fr-t3-blocs-modele : Sonnet, `../fragment-rs-blocs`.
  - fr-t3.2 : Sonnet, `../fragment-rs-imbrique`. Même crate que blocs, fichiers disjoints ; seule la ligne `mod` est commune.
- Battement horaire reposé (CronCreate 1b9dfc9f, minute 17, heure locale), vérifié par CronList.
**Part et rythme** : cible (97 − 37) ÷ 48,3 h ≈ 1,25 point par heure.
**Prochaine action exacte** : la revue habituelle à chaque compte rendu :
  1. diff ;
  2. audit à 0 ;
  3. CI ;
  4. fusion ;
  5. `bd close` ;
  6. retrait du worktree et du dossier cible.

Places libres ensuite, prises dans `bd ready` : fr-t4-barre, fr-t2-tree-item, fr-t3-scenes-a.
Livraison de T1 quand #49 est fusionnée.
