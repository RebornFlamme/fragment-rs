# Programme — Fragment en Rust (`fragment-rs`)

> Ce fichier est LA source de vérité de l'organisation. Le chef de projet le relit au début de chaque vague.
> Il est écrit par l'orchestrateur (session Fable, dossier `Fragment`) le 1er novembre 2026, après validation de l'utilisateur.
> Il ne décrit pas Fragment : la spec de Fragment est son code, son `CLAUDE.md` (≈ 600 lignes d'invariants et de décisions) et ses 96 specs e2e.

## 0. But, non-buts, décisions figées

**But.** Recréer Fragment (clone d'Obsidian pour humains et agents : coffres Markdown, PDF, calques, marges à widgets, chat agentique, plugins) en Rust sur GPUI, avec les primitives qui manquent pour les trois points bloquants : l'éditeur Markdown en live preview, les pages HTML vivantes, et les plugins qui créent des objets complets (vues, calques, widgets).

**Non-buts (ne pas ouvrir).** Compatibilité binaire avec les plugins/thèmes Obsidian existants. Collaboration temps réel. Autre plateforme que Windows d'abord (Linux compile en CI, macOS plus tard). Toute « amélioration » du design de Fragment : on reproduit, on ne redessine pas (une divergence = un ADR validé par l'orchestrateur).

**Décisions figées (validées par l'utilisateur le 01/11/2026).**

| Sujet | Décision | Pourquoi |
|---|---|---|
| Dossier | `C:\Users\julie\OneDrive\Documents\Projects\fragment-rs` | Demande utilisateur. `target/` et la base Beads sont DÉPORTÉS hors OneDrive (§4). |
| Dépôt | GitHub `RebornFlamme/fragment-rs`, **public**, MIT | Runners Actions gratuits sans limite de minutes. Licence du cœur de Fragment (`packages/core/LICENSE`). |
| Socle UI | `gpui-kit` 0.8.x (Longbridge, Apache-2) et son `gpui-pre` ÉPINGLÉ (`=0.3.10` au 01/11) | 75+ composants, dock/onglets/splits, listes virtuelles, Markdown/HTML natifs, éditeur tree-sitter, crate `gpui-webview` (WebView2), runtime JS `gpui-shell` (QuickJS), tests headless, AccessKit. Deux des trois points bloquants ont une base. |
| Éditeur de Zed | **NON** vendorisé (GPL-3, incompatible MIT). GPUI seul est Apache-2. | On construit l'éditeur live preview sur les primitives texte de gpui-kit / GPUI. |
| Registre des tâches | **Beads** (`bd`), préfixe `fr-` | Dépendances, `bd ready`, claim atomique, hooks Claude Code, Windows. |
| Orchestration | Agent Teams natifs de Claude Code (lead = chef, équipiers Sonnet, sous-agents Haiku) | Gas Town exige tmux/WSL et vise 20-30 agents : hors budget. Pas d'équipes imbriquées : un équipier délègue à des SOUS-AGENTS, pas à une équipe. |
| Budget | **Deux voies** (§2) : l'abonnement Max (fenêtres 5 h / hebdo) pour l'orchestrateur et le chef, **250 $ de crédits cloud** pour les équipiers en sessions cloud (volume ET mécanique). **Pas de clé API** : la voie « GitHub Actions + `claude -p` » est ABANDONNÉE (décision utilisateur 01/11 : « l'API ça me fait peur »). Sur l'abonnement : pause stricte, usage supplémentaire payant DÉSACTIVÉ. | Les crédits cloud ne touchent pas aux fenêtres de l'abonnement : c'est là que va tout le volume. GitHub Actions ne sert qu'à COMPILER et TESTER. |
| Compilation lourde | GitHub Actions (Windows + Linux). En local : `cargo check -p` / `cargo test -p` des crates pures seulement. | Un portable sous OneDrive ne compile pas GPUI en boucle pour 10 agents. |
| Référence | Le dépôt Fragment : `C:\Users\julie\OneDrive\Documents\Projects\Fragment` — **LECTURE SEULE** pour toute session de ce programme. | On lit la spec, on n'y écrit jamais. |

## 1. Rôles

```
Orchestrateur (Fable 5.1, session « Fragment »)
  └─ Chef de projet (Opus 5.5, session « fragment-rs », lead d'une Agent Team)
       ├─ Équipier A (Sonnet 5.5) ── sous-agents Haiku 5.5 (mécanique)
       ├─ Équipier B (Sonnet 5.5) ── sous-agents Haiku 5.5
       └─ Équipier C (Sonnet 5.5) ── sous-agents Haiku 5.5
```

**Orchestrateur = COMMANDITAIRE (règle de l'utilisateur, 01/11 : « tu n'es pas censé gérer ce que le chef doit faire »).** Ne code pas, ne lit pas de diff, ne compose pas les vagues, ne relance pas le chef à chaque run. Démarre ou redémarre la session du chef, tranche ce que le §8 remonte, et sollicite l'utilisateur UNE fois : à la livraison d'un binaire (§5, fin de tranche, pixels verts), avec le lien de l'artifact. Entre deux livraisons, ni l'orchestrateur ni le chef ne posent de question à l'utilisateur.

**Chef de projet (Opus 5.5, effort `medium`).** GÈRE TOUT : lit ce programme, tient le registre Beads, découpe en tâches, compose les vagues (§2), rédige les briefs, démarre les équipiers, revoit les PR (lecture du diff, pas réécriture), fusionne, écrit `docs/journal.md`, programme SES PROPRES RÉVEILS (minuteur) pour relire `gh pr list` / `gh run list` et avancer : il ne termine un tour que s'il n'y a rien à faire avant un délai connu, et l'a programmé. Il ne code que pour débloquer en moins de 10 minutes ; au-delà il crée une tâche. Il n'écrit JAMAIS dans le dépôt Fragment.

**Équipiers (Sonnet 5.5).** Une tâche Beads = une branche = une PR. Ils lisent la source TS de référence, écrivent les tests d'abord, l'implémentation ensuite, poussent, lisent le résultat CI, corrigent, demandent la fusion. Ils possèdent des FICHIERS déclarés dans la tâche ; deux tâches ouvertes ne partagent jamais un fichier. Ils délèguent le mécanique à des sous-agents Haiku (traduction d'un fichier de tests, `cargo fmt`, correction de `clippy`, rédaction d'une table).

**Sous-agents (Haiku 5.5).** Tâches fermées, vérifiables par une commande, brief ≤ 200 mots.

**Interdits à tous.** `cargo build` du binaire GPUI en local (CI seulement). `--no-verify`. Écrire dans `Fragment/`. Un `unwrap()` dans une crate `lib` hors tests. `unsafe` sans ADR. Ajouter une dépendance sans la noter dans la PR. Forker une crate externe sans ADR (§10).

## 2. Budget et cadence (la contrainte réelle)

État au départ (01/11 11:30 UTC) : fenêtre 5 h à 57 %, **semaine à 25 %** (reset le 06/11 18:00 UTC). Le plafond HEBDOMADAIRE cadence l'abonnement, pas les 5 h. D'où la répartition :

**Voie A — Abonnement Max (fenêtres).** Orchestrateur + chef + au plus UN équipier local quand la marge le permet. C'est la voie RARE : la coordination, pas le volume.

**Voie B — Crédits cloud (250 $).** Les équipiers de phases 1 à 3 tournent en **sessions cloud** (`start_session` avec `target: "cloud"`, `repository: "RebornFlamme/fragment-rs"`, dépôt public — fait). ★ CONSTATÉ (01/11) : l'app ne crée une session cloud que sur un tour déclenché par l'UTILISATEUR (pas sur un tour né d'un message de session ni d'un réveil) ; les environnements cloud s'appellent « Default » (il n'y a pas d'environnement « Fragment ») et une carte ne peut pas en choisir un. La vague 1 a été demandée par l'orchestrateur sur un tour utilisateur. Pour les suivantes, sans tour utilisateur disponible : REPLI AUTONOME = équipiers LOCAUX (Agent Team, Sonnet 5.5, sous-agents Haiku) sous les seuils ci-dessous ; le chef peut en plus poser des cartes cloud que l'utilisateur cliquera s'il passe, sans jamais attendre après elles. Une session cloud continue quand le portable dort et n'envoie pas de note de fin : le point de synchronisation est le registre (Beads) et `gh pr list`. ★ À la PREMIÈRE session cloud, le chef lit `get_usage` avant et après : si les fenêtres Max ne bougent pas, la voie B est confirmée et prend tout le volume ; sinon il le note au journal et l'orchestrateur tranche. Modèle : Sonnet 5.5. Les sessions cloud partent de la branche par défaut du dépôt (`main`) : une tâche = une branche = une PR, comme en local.

**Voie C — ABANDONNÉE (01/11).** Il n'y a PAS de clé API dans ce programme : pas de `port.yml`, pas de `claude -p` en Actions, pas de secret `ANTHROPIC_API_KEY`. Si un `port.yml` existe, le supprimer. Le mécanique (phase 1 : les 99 modules tests-d'abord ; phase 4 : les tables) va en voie B, par LOTS : une session cloud Sonnet reçoit un groupe entier du §5.1 (ex. `p1-chat`, 8 modules) et enchaîne les modules dans la même session, un commit par module, une PR par groupe.

**La référence TS depuis le cloud.** Le dépôt Fragment est PRIVÉ sur GitHub et n'est pas sur la machine d'une session cloud. ★ FAIT (01/11) : l'utilisateur a posé un jeton fine-grained en lecture seule (`Contents: read`, Fragment uniquement, 90 jours, nom `fragment-rs-cloud-read`) comme variable `FRAGMENT_READ_TOKEN` de son environnement cloud (nommé « Default » dans l'app ; l'utilisateur l'appelle « Fragment »). Les sessions cloud partent sur l'environnement par défaut, et chaque brief cloud commence par : `git clone --depth 1 https://x-access-token:$FRAGMENT_READ_TOKEN@github.com/RebornFlamme/Fragment /tmp/Fragment` (lecture seule ; ne jamais afficher la variable, ne jamais l'écrire dans un fichier ou un commit). La référence se lit dans `/tmp/Fragment/app/src/…`. Un secret Actions du même nom existe aussi sur le dépôt, inutilisé. Jamais une copie des sources de Fragment dans `fragment-rs` : le dépôt est public, Fragment est privé.

**Règle d'affectation.** Tout ce qui écrit du code → voie B (cloud, Sonnet). Coordination, revue, fusion, décision → voie A. Un équipier LOCAL seulement si le cloud est indisponible et que la marge de la fenêtre 5 h le permet.

**Lecture (voie A).** Le chef lit l'usage par l'outil de l'app `mcp__ccd_session_mgmt__get_usage` (champs `percentUsed`, `resetsIn`, `resetsAt` par fenêtre) : AVANT chaque vague, et à chaque fin de tâche d'un équipier.

**Seuils (pause stricte).**
- Fenêtre 5 h : aucune vague ne DÉMARRE au-dessus de **80 %** (décision utilisateur du 01/11, relevé de 60 %) ; au-dessus de **90 %**, le chef demande à tous les équipiers de terminer leur étape en cours et de se mettre en pause ; reprise au reset.
- Semaine, à partir du reset du 06/11 18:00 UTC (décision de l'orchestrateur du 05/11, remplace le prorata qui affamait le programme dès que l'utilisateur travaille) : fragment-rs dispose d'une **PART FIXE de 30 points d'hebdo par semaine**, comptée par le chef (relevé `get_usage` avant et après chaque tâche lancée, cumul dans `docs/journal.md`) et étalée à **8 points par jour au plus**. Conditions : fenêtre 5 h ≤ 80 % pour lancer, pause à 90 %, **arrêt global à 85 % d'hebdo**.
- Jusqu'à ce reset : plafond de 77 % d'hebdo pour fragment-rs (10 points à partir de 67 %).
- Le chef note l'usage dans chaque entrée de `docs/journal.md`.

**Modèles.** Chef : Opus 5.5 `medium`. Équipiers : **Sonnet 5.5** toujours (nommé dans le prompt de spawn). Sous-agents : Haiku 5.5 pour le mécanique, Sonnet pour le reste. Jamais Opus pour un équipier, jamais Fable.

**Hygiène de contexte (c'est là que part l'argent).**
- Un équipier = une tâche puis fin de session. Pas d'équipier « à demeure ».
- Brief ≤ 400 mots : la tâche, les fichiers possédés, les fichiers TS de référence (chemins), la commande de vérification, la définition de « fini ». Jamais de code collé : des chemins.
- Le chef compacte sa conversation à chaque fin de vague (`/compact` avec pour consigne : garder décisions, vague en cours, blocages).
- Les résultats CI se lisent par `gh run view <id> --log-failed` (le log des échecs), jamais le log complet.
- Un équipier qui dépasse 300k tokens de contexte s'arrête, écrit où il en est dans la tâche Beads (`bd update <id> --notes`), et le chef relance un équipier neuf.

**Vague** = l'unité de travail : ≤ 3 équipiers, ≤ 5 tâches chacun, se termine par CI verte sur `main` et une entrée de journal. Durée cible 2 à 4 h de travail d'agents.

## 3. Registre Beads : conventions

Installation (phase 0) : `npm install -g @beads/bd` puis `bd init` dans `fragment-rs` (préfixe `fr-`), `bd setup claude` (hooks). La base Dolt est déportée hors OneDrive (voir `bd init --help` ; sinon mode serveur sur `C:\dev\fragment-rs-beads`).

**Structure.** Une ÉPIC par phase (`fr-p0` … `fr-p4`), une épic par tranche verticale en phase 3, des tâches sous les épics. Dépendances par `bd dep add <enfant> <parent>` : le chef ne lance que ce que `bd ready` rend.

**Toute tâche porte, dans sa description :**
1. `Référence :` les fichiers TS de Fragment à lire (chemins absolus).
2. `Fichiers possédés :` la liste des fichiers Rust que la tâche crée ou modifie (exclusifs tant qu'elle est ouverte).
3. `Vérification :` la commande (ex. `cargo test -p fragment-artefacts`) et/ou le workflow CI attendu vert.
4. `Fini quand :` critères observables (nombre de cas de test portés, PR fusionnée, mesure).
5. La règle, dans le brief aussi : **aucune copie de source de Fragment ; une fixture se réécrit. Un port TRADUIT le code et RÉÉCRIT les commentaires dans ses mots ; il ne recopie ni en-tête ni diagramme** (ADR-007).

**Cycle d'un équipier.** `bd ready` → `bd update <id> --claim` → branche `task/<id>` → travail → `gh pr create` → CI → `bd update <id> --notes "PR #n"` → le chef lance l'audit des sources (code 0 obligatoire, §7) puis fusionne → `bd close <id>`. Une tâche fermée sans PR fusionnée est une erreur.

**Bugs.** Un échec CI répété deux fois sur une tâche → la tâche est mise en pause et un bead `bug` est créé avec le log des échecs. Un bug ne se corrige pas dans la tâche qui l'a trouvé si ses fichiers sont ailleurs.

## 4. Dépôt : structure et outillage

```
fragment-rs/
  Cargo.toml                 # workspace, edition 2024, resolver 3
  rust-toolchain.toml        # stable épinglé (channel noté au bootstrap)
  .cargo/config.toml         # [build] target-dir = "C:/dev/fragment-rs-target" (hors OneDrive) ; [alias] xtask
  .github/workflows/ci.yml   # fmt, clippy -D warnings, test (ubuntu + windows), cache
  .github/workflows/build.yml# binaire Windows en artifact sur main
  .claude/settings.json      # agent teams, modèle des sous-agents, permissions
  CLAUDE.md                  # court : renvoie ici + conventions Rust
  program.md                 # CE FICHIER
  docs/
    journal.md               # une entrée par vague (§8)
    decisions/ADR-NNN-*.md   # décisions d'architecture
    contrats.md              # table : type TS public → trait/struct Rust → crate (phase 0)
    parite-tests.md          # phase 1 : module TS → fichier Rust, cas portés / cas source
    spikes/S1-editeur.md, S2-webview.md, S3-plugins.md, S4-theme.md
    parite-e2e.md            # phase 4 : les 96 specs, ligne par ligne
  crates/
    fragment-core/           # Component/register*, Registry/OrderedRegistry/RegistryHub, Events, Scope/Keymap/Hotkey, Command, App (traits), Plugin, Vault (TFile/TFolder, events origin user|disk), DocumentKey/DocumentProvider/DocumentManager, modèle du workspace (WorkspaceItem/Parent/Split/Tabs/Leaf/ViewState), LeafHistory, layout.ts, breadcrumb, fuzzySearch, lru, debounce, diffMinimal, frontmatter, resolveLink, semver
    fragment-text/           # TextSurface/DocumentSurface/SurfaceText, EditorPosition/Range/Change, mapRange/mapAnchor, rangeHighlights (table), viewport (bande, constantes VP), findPassage, revealPassage (modèle), textDelta, locatePassage
    fragment-artefacts/      # types, clock (HLC format Actual), fold, journal/{writer,reader,import,compact}, objects, cache (trait + mémoire), anchor (citation + cascade), device, ArtefactManager
    fragment-layers/         # WidgetAnchor/Handle/LazyWidget, WidgetLayer (modèle), GutterArbiter, gutterStack (arrangeGutter, computeRooms, clusterGutter, gutterGap), WidgetActivation
    fragment-markdown/       # tableModel, frontmatterModel, editorTransforms, listes, modèle du live preview (décorations, widgets), sanitize (politique)
    fragment-pdf/            # pdfium-render : pages, texte, outline (pdfOutline), pdfGeometry, pdfZoom, pdfFindOffsets, PdfEditor (modèle)
    fragment-chat/           # ChatSession, chatSurfaceModel, followModel, rail, searchModel, historyModel, postModel, format
    fragment-agent/          # harness (registry), events (SDK → AgentEvent), traceModel, policyModel/permissionModel, quickModel, actionsModel, skillsModel, screenModel, attachmentBlocks, engine/engineModel ; harness Claude = `claude` CLI en `--output-format stream-json` (processus enfant)
    fragment-annotation/     # geometry, shapes, AnnotationSession, AnnotationPersistence
    fragment-snap/           # snapModel
    fragment-theme/          # jetons (noms d'Obsidian, valeurs de Fragment), themeModel, cascade (S4)
    fragment-cli/            # protocole (protocol.ts), index d'instances, shim, registre des coffres, client `fragment`
    fragment-html/           # pont (htmlBridge, htmlTextModel), hôte webview (S2)
    fragment-plugins/        # hôte gpui-shell + API JS `fragment` (S3), catalogue (GithubRegistrySource), plugins.ts
    fragment-ui/             # l'app GPUI : shell du workspace sur gpui-kit, vues (markdown, pdf, chat, explorer, html, image, palette, réglages), doc-widget, calques, menu Calques, barres, snap (présentation)
    fragment-app/            # main.rs : fenêtres, boot, coffres
  xtask/                     # `cargo xtask parite` (tables), `cargo xtask pixels` (diff d'images), `cargo xtask bench`
  pixels/
    vault/                   # le coffre de fixture, identique des deux côtés
    scenes/<scene>.spec.ts   # pilote Fragment (harnais e2e importé) → référence
    scenes/<scene>.json      # rejoue la même scène dans fragment-rs → candidat
    reference/, candidate/, diff/   # PNG (référence commitée ; candidat et diff = artifacts CI)
    playwright.config.ts     # testDir = scenes, lance Fragment par son harnais, sans le modifier
  scripts/
    bootstrap.ps1            # phase 0 en une commande (§7)
    ci.ps1                   # `scripts/ci.ps1 <pr|run-id>` : état + log des échecs seulement
    wave.ps1                 # `scripts/wave.ps1` : bd ready, PR ouvertes, dernier CI, usage → résumé pour le journal
```

**CI (`ci.yml`).** Déclenchée sur PR et `main`. Jobs : `fmt` (ubuntu), `clippy` (ubuntu, `-D warnings`), `test-linux` (ubuntu, `cargo test --workspace --exclude fragment-ui --exclude fragment-app` + crates UI en `--no-run` si GPU absent), `test-windows` (windows-latest, tout le workspace, tests headless gpui-kit), `check-app` (windows, `cargo check -p fragment-app`), `pixels` (windows : `cargo run -p fragment-app --features pixels --bin fragment-pixels -- pixels/scenes` puis `cargo xtask pixels` ; artifact `pixels/diff/` + `docs/parite-pixels.md` ; rouge hors tolérance). `Swatinem/rust-cache@v2` partout ; `sccache` (`mozilla-actions/sccache-action`) si le cache dépasse 10 Go. `concurrency: group: ${{ github.ref }}, cancel-in-progress: true`. Un `cargo check` seul suffit sur PR pour les crates UI tant qu'un test headless n'existe pas.

**`build.yml`.** Sur `main` seulement : `cargo build --release -p fragment-app` (windows), artifact `fragment-rs-win-x64.zip`. C'est le binaire que l'utilisateur essaie.

**Dépendances externes autorisées d'avance** (toute autre : notée dans la PR) : `gpui-kit` (+ `gpui-pre*` épinglés), `gpui-webview`, `gpui-shell`, `pdfium-render`, `tree-sitter`, `tree-sitter-md`, `pulldown-cmark` (lecture seule / rendu), `serde`, `serde_json`, `yaml-rust2` ou `serde_yaml`, `notify` (watcher), `sha2`, `uuid`, `time`, `anyhow`/`thiserror`, `tokio` (si gpui-kit l'exige), `similar` (diff), `fuzzy-matcher`, `typst-math` ou `rex` (maths, décision S1), `windows` (via gpui).

## 5. Phases, livrables, portes

### Phase 0 — Amorçage (chef + 1 équipier, ≤ 1 vague)

Livrables :
0. L'utilisateur vérifie qu'un environnement cloud existe dans l'app et que les sessions cloud peuvent lire le dépôt Fragment (§2, « La référence TS depuis le cloud ») ; le chef le lui demande via l'orchestrateur dès que le dépôt existe, et n'attend pas pour le reste. Aucune clé API, jamais.
1. Dépôt créé (`gh repo create RebornFlamme/fragment-rs --public --source . --push`), workspace Cargo avec toutes les crates VIDES (un `lib.rs` par crate), `fragment-app` = une fenêtre gpui-kit « Fragment » avec un dock vide. CI verte, `build.yml` produit l'artifact.
2. Beads initialisé, épics `fr-p0`…`fr-p4`, tâches de la phase 1 créées (une par groupe du §5.1) avec dépendances.
3. `docs/contrats.md` : pour CHAQUE export de `Fragment/app/src/ui/core/public-api.ts` (≈ 150 noms) et chaque registre `*@1` (19, déclarés dans les `api.ts`/`contract.ts` des plugins), la forme Rust (trait / struct / enum / « présentation, pas de contrat ») et la crate. C'est le contrat gelé : un changement = ADR.
4. `docs/decisions/ADR-001-socle.md` (gpui-kit, versions épinglées, licence), `ADR-002-ledger.md`, `ADR-003-ci.md`.
5. `CLAUDE.md` du dépôt complété : conventions Rust (§9), commande de vérification, où est la spec.
6. **Pixels, dès la phase 0** : `pixels/` amorcé avec les 22 scènes de `visual-diff.spec.ts` (référence capturée par `scripts/pixels-reference.ps1` sur le portable), `docs/constantes-rendu.md` (constantes extraites des CSS de Fragment), `cargo xtask pixels` qui compare deux dossiers, et une PREUVE que la capture offscreen marche sur le runner Windows (une scène « fenêtre vide » de fragment-app comparée à elle-même : 0 pixel). Si WARP ne rend pas, ADR « runner auto-hébergé » avant la phase 3.

Porte : CI verte sur `main`, `bd ready` rend les tâches de phase 1, `docs/contrats.md` relu par le chef sans trou. L'orchestrateur valide par lecture du journal.

### Phase 1 — Les modules purs, tests d'abord (sessions cloud Sonnet, un groupe par session, 3 en parallèle, 2 à 3 vagues)

99 paires `x.ts` / `x.test.ts` dans Fragment = 15 766 lignes TS, 9 096 lignes de tests. Règle : **traduire le fichier de tests d'abord** (un `#[test]` par `it`/`test`, MÊME nom en snake_case, mêmes données), puis l'implémentation jusqu'au vert. Un test TS non portable (DOM) est listé dans `docs/parite-tests.md` avec la raison. Les sous-agents Haiku traduisent les tests ; l'équipier Sonnet implémente.

Groupes (= tâches Beads, chemins sous `Fragment/app/src/`) :

| Tâche | Crate | Modules TS (source / test LOC) |
|---|---|---|
| p1-artefacts | fragment-artefacts | `ui/core/artefacts/fold.ts` (51/173), `anchor.ts` (236/260), `ArtefactManager.ts` (1080/611) + `clock`, `journal/*`, `objects`, `cache` (lire `app/documentation/journal-artefacts.pdf` §III) |
| p1-text | fragment-text | `ui/core/editor/viewport.ts` (102/59), `findPassage.ts` (82/32), `revealPassage.ts` (133/36, modèle `locatePassage`), `rangeHighlights.ts` (163/92, la table), `mapRange.test.ts`, `layers/mapAnchor.test.ts`, `shared/htmlBridge.ts` (338/44, partie pure), `views/html/htmlTextModel.ts` (93/67) |
| p1-layers | fragment-layers | `ui/core/layers/gutterStack.ts` (221/200), `GutterArbiter.ts` (258/66), `WidgetActivation.ts` (211/136) ; `WidgetLayer.ts` (840/45) : MODÈLE seulement (ancres, remappage, bande), la présentation va en phase 3 |
| p1-core-a | fragment-core | `CommandManager.ts` (197/298), `RegistryHub.ts` (75/86), `plugins.ts` (503/360, modèle d'activation), `keymap/keymap.ts` (203/483), `captureHotkey.ts`, `swipe.ts` (59/43), `fuzzySearch.ts`, `lru.ts`, `diffMinimal.ts`, `frontmatter.ts`, `links/resolveLink.ts`, `plugins/registry/semver.ts` |
| p1-core-b | fragment-core | `workspace/layout.ts` (105/136), `LeafHistory.ts` (86/66), `breadcrumb.ts` (45/50), `documents/DocumentManager.ts` (77/60), `documentKey.ts`, `embed/embedDocument.ts` (123/44, résolution), `vault/attachments.ts`, `settings/theme.ts`, `window/zoom.ts`, `DragManager.ts` (185/76 ; 2 échecs préexistants connus : les reproduire ou les documenter), `external/dispatch.ts`, `cli/dispatch.ts`, `api.ts`/`public-api.ts` (parité des exports → test de parité de `docs/contrats.md`) |
| p1-cli | fragment-cli | `shared/cli/protocol.ts` (196/90), `instanceIndex.ts`, `shim.ts`, `vaultRegistry.ts`, `shared/vaultPath.ts`, `cli/clientPur.ts`, `coreCommands/cliServer.ts` (217/74), `electron/secrets.ts` (modèle) |
| p1-markdown | fragment-markdown | `views/markdown/table/tableModel.ts` (234/124), `properties/frontmatterModel.ts` (170/97), `editorTransforms.ts` (494/344), `sanitizeHtml.ts` (75/48, la POLITIQUE : balises/attributs permis), `readOnlyMarkdown.ts` (modèle), `settings/readmeMarkdown.ts` (350/70, rendu → arbre) |
| p1-pdf | fragment-pdf | `views/pdf/pdfGeometry.ts` (226/77), `pdfZoom.ts` (74/43), `pdfOutline.ts` (89/34), `pdfFindOffsets.ts` (90/51), `PdfEditor.ts` (571/82) modèle des positions `(page−1)·1e6+1+car.` |
| p1-chat | fragment-chat | `ChatSession.ts` (131/94), `chatSurfaceModel.ts` (174/118), `scroll/followModel.ts` (109/60), `rail.ts` (154/36), `searchModel.ts`, `components/historyModel.ts`, `postModel.ts`, `format.ts` (98/42) |
| p1-agent | fragment-agent | `events.ts` (196/156), `trace/traceModel.ts` (313/197), `policy/policyModel.ts` (301/114), `permissionModel.ts`, `quick/quickModel.ts` (214/71), `actions/actionsModel.ts` (139/84), `skillsModel.ts` (132/108), `screenModel.ts` (123/47), `attachmentBlocks.ts` (85/50), `engine/engine.ts` + `engineModel.ts` (TarReader), `harness/registry.ts`, `oneShotOptions.ts`, `controls/controlsModel.ts`, `side-chat/cardModel.ts`, `textAccess.ts`, `lasso/lassoPrompt.ts` |
| p1-divers | fragment-annotation, fragment-snap, fragment-theme, fragment-plugins | `views/annotation/{geometry,shapes,AnnotationSession,AnnotationPersistence}.ts`, `views/artefacts/menuModel.ts`, `views/doc-widget/{anchorLink,clusterFilter}.ts` (trace pure), `snap/snapModel.ts` (219/208), `theme/themeModel.ts`, `plugins/registry/GithubRegistrySource.ts` (103 tests : faux dépôt), `coreCommands/WorkspaceCommandsPlugin.ts` (modèle de marge), `views/command-palette/CommandPalettePlugin.ts` (filtrage), `core/Toolbar.ts` (670/68 : le MODÈLE des sections, contrat de Menu), `settings/Setting.ts`, `SettingPage.ts`, `SecretStorage.ts`, `definitionTree.test.ts` |

Porte : toutes les crates non-UI vertes en CI ; `docs/parite-tests.md` donne, par module, cas portés / cas source, avec un total ≥ 90 % des cas (le reste justifié).

### Phase 2 — Les spikes (3 équipiers en parallèle, ≤ 3 jours, verdict écrit)

Chaque spike vit dans `crates/<crate>/examples/spike_*.rs` + `docs/spikes/Sn-*.md` (hypothèse, ce qui a été essayé, mesures, verdict GO / NO-GO / GO-AVEC, repli). Un spike ne cherche pas la propreté : il cherche la réponse. Son code est jeté ou promu par une tâche de phase 3.

**S1 — Éditeur live preview (le plus risqué).** Sur les primitives texte de gpui-kit (son `editor`/`Input`, `tree-sitter-md`). Doit démontrer, dans un exemple à lancer : frappe et IME (composition), `#` masqué hors curseur et révélé dessous (règle `curseurActif` de Fragment), une case à cocher INLINE cliquable, un widget de BLOC (tableau ou image) atomique sous lequel un clic tombe sur la bonne ligne, soft wrap, undo/redo, défilement fluide sur 10 000 lignes, mesure `coordsAtPos`/`posAtCoords` exposée (contrat `TextSurface`). Maths : rendre `$x^2$` par `typst` (math) ou `rex` → SVG → image GPUI ; noter la fidélité. NO-GO ⇒ repli : mise en page texte maison sur `gpui::TextSystem` (ShapedLine, `DisplayMap` à la Zed réécrite).

**S2 — Pages HTML vivantes.** `gpui-webview` : une page du coffre dans une feuille, JS qui tourne, pont `postMessage` (hauteur, lien, touche), injection des variables de thème avant la 1re image, sandbox (pas d'accès fichier hors coffre), coexistence avec un calque dessiné PAR-DESSUS (c'est le point dur : WebView2 est un HWND enfant). NO-GO sur la superposition ⇒ repli : la page en pleine feuille sans calques (perte acceptée, ADR) ou rendu HTML natif de gpui-kit pour les pages statiques.

**S3 — Runtime de plugins.** `gpui-shell` (QuickJS) : un hôte qui expose une API `fragment` à un script : `registerView(type, {render})` avec un arbre déclaratif (JSON → composants gpui-kit du `components::registry()`), `addCommand`, `registerDocumentProvider`, `addWidget(mode: gutter, anchor)` dans la marge d'une vue. Un plugin exemple (jumeau de `Fragment/app/e2e/fixtures/gutter-globe-plugin.cjs`) chargé à chaud depuis `.fragment/plugins/<id>/main.js`. Mesurer : coût d'un rendu, isolation (le script ne voit pas le FS), rechargement. NO-GO ⇒ repli : WASM (wasmtime) + même arbre déclaratif.

**S4 — Thème (chef, sans équipier).** ADR : jetons de Fragment (noms d'Obsidian, valeurs d'`index.css` : palette sombre/claire, forme, polices DM Sans) → `struct Theme` + fichier `theme.json` par coffre (`.fragment/themes/<Nom>/theme.json`) rechargé à chaud ; un composant lit un jeton, jamais un littéral (garde-fou = test qui grep les littéraux de couleur hors `theme/`). Le `theme.css` de Fragment n'est PAS supporté (non-but).

Porte : 3 verdicts écrits, lus par l'orchestrateur ; tout NO-GO déclenche l'ADR du repli AVANT la phase 3.

### Phase 3 — Tranches verticales (3 équipiers, ordre des dépendances)

Chaque tranche = une épic Beads, des tâches ≤ 1 jour, des tests headless (gpui-kit « headless UI testing », `TestAppContext`) pour chaque comportement nommé dans les specs e2e correspondantes de `Fragment/app/e2e/` (la spec dit QUOI observer ; on n'en recopie pas le DOM). Le `CLAUDE.md` de Fragment donne, section par section, les décisions à respecter au pixel (en-tête de vue, icônes, 1/4–3/4, grappes…).

| # | Tranche | Dépend de | Specs e2e de référence (Fragment/app/e2e) |
|---|---|---|---|
| T1 | Shell du workspace : fenêtre, docks, piles d'onglets, splits, feuille active, historique ← →, portées clavier (`Scope.pourVue/echap/capture`, `insertScope`), commandes + palette, Notice/Menu/Modal/Toolbar, réglages (déclaratif), zoom | p1-core | `workspace-structure`, `workspace-layout`, `activeleaf-tracking`, `boot-scope`, `tab-drag-split`, `app-zoom`, `keymap.test` |
| T2 | Coffre : watcher `notify`, TFile/TFolder, `.fragment/` JSON, explorateur (arbre, dossiers ouverts), documents (`DocumentManager`, fournisseurs par schéma), position de lecture (3 étages) | T1 | `scroll-persistence`, `vaults` |
| T3 | Éditeur Markdown (promotion de S1) : live preview, listes, titres, inline, blocs, tableaux, properties, HTML dans le Markdown (rendu natif gpui-kit, politique de sanitize), collage d'image, menu contextuel | T1, S1, p1-markdown | `editor-hell`, `markdown-html`, `properties` |
| T4 | PDF : `pdfium-render`, pages virtualisées, zoom (modes + pinch), barre d'outils, barre latérale (vignettes, TOC), recherche, liens, couche texte → `TextSurface` | T1, p1-pdf | `pdf-toolbar`, `pdf-layers`, `pdf-find`, `pdf-geometry` |
| T5 | Calques : `OverlayHost`, plan document / viewport, `WidgetLayer` (LazyWidget, bande), arbitre de marge, activation, grappes, marge 1/4–3/4 (geste = touche `SwipeRight`), menu « Calques », `rangeHighlights` dessinés | T3 ou T4 | `viewport`, `gutter-arbiter`, `gutter-room`, `gutter-cluster`, `margin-mode`, `doc-widget-views`, `widget-anchor`, `anchor-link` |
| T6 | Artefacts : journal sur disque (`.fragment/artefacts/journal/<device>/`), cache, réconciliateur (`rehydrate`), doc-widget (`doc-widget:pin` v2, en-tête standard, replié/ouvert), objets HTML, menu des objets | T5, p1-artefacts | `artefact-*`, `artefacts-html`, `artefacts-menu`, `doc-widget-collapse` |
| T7 | Chat + agent : vue chat (modes page/panel/anchored/bar, cowork, défilement `followModel`, rail), `ChatSurface`, fournisseur agent, harness = processus `claude --output-format stream-json` (sessions, resume, fork), trace, permissions (garde + carte), pièces jointes, barre de l'agent | T5 | `chat`, `chat-scroll`, `chat-column`, `chat-document`, `chat-reply`, `chat-attachments`, `agent-chat`, `agent-harness`, `agent-trace`, `agent-permissions`, `agent-bar` |
| T8 | Side chat, Entourer/Encadrer, annotation (traits, barre horizontale, Ctrl+Z aux traits), actions rapides, actions perso | T6, T7 | `reader`, `reader-pdf`, `lasso-ask`, `annotation-layer`, `toolbar-incrustee`, `quick-actions`, `agent-actions`, `chat-active`, `widget-active` |
| T9 | Vue HTML (promotion de S2) + embeds (`embedDocument`, cadre commun, HostedView) | T5, S2 | `html-view`, `html-text`, `chat-origin` |
| T10 | Plugins (promotion de S3) : hôte, API `fragment` JS, catalogue GitHub (format Obsidian), réglages déclarés, CLI `fragment` (serveur sur tuyau nommé, shim), snap | T1…T9 partiel, S3 | `community-plugins`, `agent-plugins`, `workspace-snap`, `cli` |
| T11 | Thème (promotion de S4) + identité visuelle (icône, police DM Sans embarquée) | T1 | `theme-plugin`, `light-theme`, `visual-diff` |

Ordre de lancement : T1 → (T2, T3, T4 en parallèle) → T5 → (T6, T7) → (T8, T9, T11) → T10. Le binaire `build.yml` de chaque fin de tranche est essayé par l'utilisateur (l'orchestrateur le lui dit).

### Parité au PIXEL (exigence utilisateur, 01/11 : « je veux des pixels diff avec le projet initial, pixel perfect »)

Le rendu de fragment-rs est jugé **par comparaison d'images avec Fragment**, scène par scène, en CI, et c'est une PORTE de chaque tranche : une tranche n'est pas finie tant que ses scènes ne passent pas.

**Les scènes.** Une scène = un coffre de fixture + une suite d'actions + une capture (fenêtre 1280 × 800, DPR 1, sombre ET clair). Les 22 scènes de `Fragment/app/e2e/visual-diff.spec.ts` sont le point de départ ; chaque tranche AJOUTE les siennes (un état par comportement visible de ses specs e2e : note riche, PDF à 3 zooms, chat page/panel/ancré, grappe repliée/ouverte, menu Calques, barre d'annotation, marge 1/4–3/4, snap…). Objectif ≥ 150 scènes en fin de phase 3.

**Côté Fragment (référence).** Les scènes vivent dans `fragment-rs/pixels/scenes/*.spec.ts` et PILOTENT Fragment par son harnais e2e importé par chemin (`../../../Fragment/app/e2e/harness`), lancées avec le Playwright de `Fragment/app/node_modules` et une config qui fixe `testDir` sur `pixels/scenes` ; sorties dans `pixels/reference/<scene>-<theme>.png`. Fragment n'est PAS modifié : on l'exécute, on ne l'écrit pas. Déterminisme : caret masqué, animations coupées (`prefers-reduced-motion`), horloges figées (dates des conversations fixes dans la fixture), même coffre `pixels/vault/`, même DM Sans. ★ « Lecture seule » (orchestrateur, 06/11) : rien d'écrit dans l'arbre de Fragment ; exécuter ses outils avec caches et sorties redirigés dehors est permis, avec contrôle de `git status` avant et après. La référence est régénérée par `pwsh scripts/pixels-reference.ps1` (sur le portable de l'utilisateur, hors CI : Fragment n'y est pas) et COMMITÉE (PNG, ~150 × 2 × ~300 Ko : acceptable ; `git lfs` si > 200 Mo).

**Côté fragment-rs (candidat).** Un binaire `fragment-pixels` (dans `fragment-app`, feature `pixels`) rejoue CHAQUE scène depuis un script JSON (`pixels/scenes/<scene>.json` : coffre, fenêtre, thème, actions = commandes + clics + frappes aux mêmes coordonnées, attente de stabilité) et capture le framebuffer (rendu offscreen GPUI ; sur runner Windows sans GPU : WARP, le rasterizeur logiciel de DirectX — à vérifier en phase 0 ; repli : runner auto-hébergé sur la machine de l'utilisateur, `scripts/pixels-runner.ps1`). Même DM Sans embarquée, mêmes jetons de thème. Le spec Playwright de la scène et son JSON sont écrits ENSEMBLE, par la même tâche.

**Le diff.** `cargo xtask pixels` compare `pixels/reference/` et `pixels/candidate/`, écrit `pixels/diff/<scene>.png` (pixels divergents en rouge) et `docs/parite-pixels.md` (par scène : pixels différents, % de l'image, pire delta de canal, boîtes des zones divergentes). Deux niveaux :
- **Strict (la cible)** : 0 pixel différent.
- **Toléré (le sas)** : delta de canal ≤ 16 ET ≤ 0,3 % des pixels, divergences confinées à des GLYPHES (zones < 20 × 20 px). ★ Honnêteté technique : Chromium rasterise le texte avec Skia + DirectWrite, GPUI avec son propre pipeline ; l'anticrénelage des bords de glyphes peut différer d'un ou deux niveaux de gris sans qu'AUCUNE géométrie ne diffère. Tout ce qui n'est pas un bord de glyphe (boîtes, traits de 1 px, rayons, couleurs de fond, position de chaque glyphe, lignes de base) est en STRICT dès la première scène. Les divergences de glyphes sont une dette suivie (tâche « gamma / hinting / sous-pixel » sur le rasterizeur, phase 4) ; si elles ne tombent pas à 0, l'orchestrateur remonte à l'utilisateur avec les images.
- La CI (`pixels` job, windows) ÉCHOUE sur toute scène hors tolérance et publie `pixels/diff/` en artifact. Un seuil par scène ne se relâche que par ADR.

**Ce que ça impose en amont.** Mêmes jetons (`fragment-theme` = valeurs d'`index.css` au bit près, `--radius-*`, `--size-*`, graisses) ; mêmes polices (fichiers woff2 de Fragment convertis en TTF, embarqués) ; mêmes constantes de mise en page (hauteurs d'en-tête 37/54 px, écart 8, icônes 22 px, `COUDE` 40, 38 px de bandeau…) lues DANS le CSS de Fragment, jamais réestimées. La table des constantes est `docs/constantes-rendu.md` (phase 0, extraite des 50 `.css` : 9 551 lignes).

### Phase 4 — Parité et mesure

`docs/parite-e2e.md` : les 96 specs e2e de Fragment, une ligne par `test(...)` (titre), état `pass` / `fail` / `n.a.` (raison). Objectif : ≥ 80 % `pass` hors `n.a.` justifiés. `cargo xtask bench` : démarrage à froid, mémoire au repos avec un coffre réel, défilement d'un PDF de 300 pages, 40 conversations en marge (les bancs de `Fragment/app/e2e/perf-*.spec.ts` disent quoi mesurer) ; comparer à Electron dans le journal.

## 6. Qu'est-ce qu'un travail « fini »

Une tâche est finie quand : (1) sa PR est fusionnée sur `main` avec CI verte ; (2) ses tests existent et sont nommés d'après les cas TS ; (3) `docs/parite-tests.md` ou `docs/parite-e2e.md` est mis à jour ; (4) aucune dépendance nouvelle non notée ; (5) les commentaires en français expliquent les décisions reprises de Fragment (marqueur `★` comme dans Fragment, renvoi au fichier TS d'origine en tête de fichier : `// Port de app/src/ui/core/layers/gutterStack.ts`) ; (6) **si la tâche touche au rendu** : ses scènes (spec Playwright côté Fragment + JSON côté fragment-rs) existent, la référence est commitée, et le job `pixels` est vert (strict, ou toléré avec la dette notée dans `docs/parite-pixels.md`).

Une TRANCHE est finie quand toutes ses scènes passent. Pas de « on verra le pixel après » : la correction d'un écart de rendu est dans la tranche.

## 7. Les commandes (une par opération, rien à la main)

| Opération | Qui | Commande |
|---|---|---|
| Phase 0 complète | chef | `pwsh scripts/bootstrap.ps1` (crée le dépôt public, le workspace, la CI, Beads, pousse ; idempotent ; écrit `docs/journal.md` entrée 0). Le chef ÉCRIT ce script en premier, puis le lance. |
| Lire l'état d'une vague | chef | `pwsh scripts/wave.ps1` → `bd ready`, `bd list --status in_progress`, PR ouvertes (`gh pr list`), dernier run CI, à coller dans le journal avec l'usage (`get_usage`). |
| Lire un échec CI | tous | `pwsh scripts/ci.ps1 <pr-number>` → statut des jobs + `--log-failed` du premier job rouge, tronqué à 200 lignes. |
| Lancer un équipier en voie B | chef | `start_session` (`target: "cloud"`, `model: claude-sonnet-5-5`, brief ≤ 400 mots, `bd update <id> --claim` fait par le chef AVANT ; un groupe entier du §5.1 par session en phase 1). |
| Vérifier une crate pure | équipier | `cargo test -p <crate>` (local ou cloud, rapide). |
| Vérifier le reste | équipier | `git push` → CI. Jamais `cargo build -p fragment-app` en local. |
| Audit des sources | chef | `cargo xtask audit-sources --rev origin/task/<id>` AVANT chaque fusion : code ≠ 0 = pas de fusion, on réécrit (ADR-007). Local : Fragment n'est pas sur les runners. `--historique` pour tout l'historique. |
| Fusionner | chef | `gh pr merge <n> --squash --delete-branch` après lecture du diff (`gh pr diff <n>`) et audit des sources vert. |
| Table de parité | équipier | `cargo xtask parite` (régénère les tables depuis les attributs `#[test]` et un fichier `parite.toml` par crate). |
| Référence pixels | utilisateur ou chef (portable) | `pwsh scripts/pixels-reference.ps1 [scene]` → capture Fragment, écrit `pixels/reference/`, commit. |
| Diff pixels | tous | `cargo xtask pixels [scene]` → `pixels/diff/`, `docs/parite-pixels.md`, code de sortie ≠ 0 hors tolérance. |
| Rapport à l'orchestrateur | chef | une entrée dans `docs/journal.md` + un message de fin de tour (l'app relaie les 400 premiers caractères : commencer par l'essentiel). |

## 8. Journal et escalade

`docs/journal.md`, une entrée par vague, format fixe :

```
## Vague N — <date heure UTC> — usage 5h xx % / semaine yy %
Phase : … Tâches fermées : fr-…, fr-… (PR #…). En cours : … Bloqué : …
Décisions prises par le chef : … (ADR si structurel)
À trancher par l'orchestrateur : … (ou « rien »)
Prochaine vague : …
```

**Remonte à l'orchestrateur (et à lui seul)** : un NO-GO de spike, une coupe de périmètre, une divergence avec Fragment, une dépendance à licence non MIT-compatible, un fork de crate externe, le plafond hebdo atteint avant la fin de la semaine, et toute question à laquelle le `CLAUDE.md` de Fragment ne répond pas. Tout le reste est au chef. L'orchestrateur répond par message dans la session du chef ; une décision structurelle devient un ADR.

## 9. Conventions de code

- Rust 2024, `clippy -D warnings`, `rustfmt` par défaut. Pas d'`unwrap`/`expect` hors tests et `main.rs` ; erreurs typées (`thiserror`) dans les crates, `anyhow` dans l'app.
- Noms : les CONCEPTS gardent le vocabulaire d'Obsidian/Fragment (`WorkspaceLeaf`, `TextSurface`, `GutterArbiter`, `coords_at_pos`), en snake_case Rust. Les commentaires sont en français, abondants, avec `★` pour ce qui a été appris chèrement, et le renvoi au fichier TS d'origine.
- Le cœur ne connaît aucune vue par son nom et ne connaît pas gpui-kit au-delà de `gpui` (invariant de Fragment transposé : `fragment-core`, `-text`, `-layers`, `-artefacts` ne dépendent pas de `gpui-kit`). Les vues sont des plugins internes de `fragment-ui`.
- Un jeton de thème, jamais un littéral de couleur hors `fragment-theme`.
- Tests : un module pur = ses tests unitaires à côté (`#[cfg(test)]` ou `tests/`), nommés d'après le TS. Un comportement d'interface = un test headless nommé d'après le `test(...)` e2e.
- Commits : `feat : …` / `fix : …` / `port : <module>` / `spike : …` / `docs : …`, message en français, Co-Authored-By de Claude à la fin.

## 10. Pannes et politiques

- **CI rouge deux fois** sur la même tâche → pause de la tâche, bead `bug` avec le log, le chef réassigne ou re-brief.
- **API GPUI manquante** → d'abord chercher dans gpui-kit (`components::registry()`, exemples du dépôt `longbridge/gpui-kit`), puis `docs/decisions/ADR-…` ; un fork n'est permis que par `[patch.crates-io]` vers `vendor/<crate>` avec un patch minimal ET une PR amont ouverte (lien dans l'ADR). Compter les forks dans le journal : objectif ≤ 2.
- **Équipier à contexte plein** → notes dans le bead, fin de session, équipier neuf.
- **Contexte du chef** (orchestrateur, 06/11) : dès que son contexte dépasse 300k tokens (`get_usage`), le chef se vide LUI-MÊME, l'orchestrateur ne peut plus le faire. Dans l'ordre : équipiers finis ; entrée de journal complète poussée (état, branches, `bd ready`, cumul de la part, prochaine action exacte) ; `docs/brief-reprise.md` à jour ; battement annulé ; réveil ponctuel `CronCreate` 5 minutes plus tard dont le texte EST ce brief ; `clear_session` avec `session_id: "self"` en dernière action, rien après. ★ Non éprouvé : ces réveils sont liés à la session et rien ne garantit qu'ils survivent au clear. Si le chef est relancé par l'orchestrateur ou l'utilisateur au lieu du réveil, il le note au journal et revient à l'ancienne procédure : réveils annulés, message « prêt pour le clear » à l'orchestrateur, fin de tour ; l'orchestrateur vide la conversation et renvoie le brief.
- **Chef redémarré** (session perdue) : l'orchestrateur relance une session avec le même brief ; le chef relit `program.md`, `docs/journal.md` (dernière entrée) et `bd ready`. Rien d'autre n'est nécessaire : tout l'état est dans Beads, git et le journal.
- **OneDrive** : si Dolt ou cargo lèvent `EPERM`/verrous, déplacer la base/le target hors OneDrive (déjà prévu) ; ne JAMAIS mettre `.beads/` ou `target/` en « toujours conserver sur cet appareil ».
- **Plafond atteint** : pause (§2), entrée de journal, fin de tour du chef. L'orchestrateur relance au reset.

## 11. Démarrage (orchestrateur)

1. L'orchestrateur ouvre une session Claude Code dans `fragment-rs` : modèle `claude-opus-5-5`, effort `medium`, même mode de permissions que la sienne, brief : « Tu es le chef de projet. Lis `program.md` en entier, puis `C:\Users\julie\OneDrive\Documents\Projects\Fragment\CLAUDE.md`. Exécute la phase 0 : écris `scripts/bootstrap.ps1`, lance-le, puis `docs/contrats.md`. Respecte le §2 (budget) avant d'ouvrir une vague. Termine ton tour par l'entrée 0 du journal. »
2. Le `.claude/settings.json` du dépôt active les Agent Teams et fixe le modèle des sous-agents (Sonnet) ; le chef nomme Haiku dans le prompt pour le mécanique.
3. À chaque fin de tour du chef, l'orchestrateur lit la note relayée et, si nécessaire, le journal ; il relance avec « vague suivante » ou tranche ce qui lui est remonté.
