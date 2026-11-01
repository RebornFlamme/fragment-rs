# S3 — Runtime de plugins sur gpui-shell (QuickJS)

- Tâche Beads `fr-p2-s3`, PR « fr-p2-s3 : spike runtime de plugins ». Phase 2 de `program.md` (S3).
- Code : `crates/fragment-plugins/tests/spike_s3.rs` (14 tests headless), `tests/spike_s3/host.rs` (l'hôte, partagé avec l'exemple), `tests/spike_s3/bootstrap.js` (le pont JS), `tests/fixtures/plugins/{e2e-globe,demo,evil,leak-a,leak-b,spin}`, `examples/spike_s3.rs` (fenêtre réelle). Windows seulement (`[target.'cfg(windows)'.dev-dependencies]`, comme S1) : aucun code de production ajouté, `fragment-core` intact.
- Méthode : `gpui-shell` 0.8.1 téléchargé de crates.io et LU (README, `lib.rs`, `host_modules.rs`, `policy.rs`, `sandbox.rs`, tests d'intégration), jamais compilé en local. Tout est itéré par la CI ; les mesures viennent d'un run jetable (`windows-latest`, branche `spike-s3-mesures`, supprimée depuis), en débogage ET `--release`. `test-windows` de la PR exécute les mêmes tests (verts). Plateforme de test de GPUI : pas de GPU, pas de pixel ; les temps de « dessin » sont ceux de la mise en page + préparation de scène, sans rasterisation.
- ★ Les mesures de ce document sont celles du **second** run release. Le premier avait donné 240 à 890 ms par chargement (runner froid ?) ; non reproduit (8 à 11 ms), à surveiller si la latence de chargement devient un critère.

## Verdict global : **GO-AVEC** — le runtime tient, sous cinq conditions de contrat (ci-dessous)

QuickJS via gpui-shell charge un plugin depuis `.fragment/plugins/<id>/main.js`, expose une API `fragment` qui branche `addCommand` sur le vrai `CommandManager`, `registerDocumentProvider` sur le vrai `DocumentManager`, rend un arbre déclaratif JSON en composants gpui-component, et le plugin jumeau de `gutter-globe-plugin.cjs` se charge, se décharge et se recharge à chaud (23 ms, 22 en passant par `Plugins::reload_plugin`). L'isolation est celle de gpui-shell (tout est refusé par défaut) et tient face à un plugin hostile. Le repli WASM n'est pas déclenché ; voir « Repli » pour son coût.

Les cinq conditions, par ordre d'impact sur T10 :

1. **Un `ShellRuntime` (une VM) PAR plugin.** Le tas est partagé dans une VM : un plugin pose `globalThis.__fuite`, un autre le lit (mesuré). Coût : ~2,6 Mo et ~8 ms de création par VM (release).
2. **Rust ne peut jamais appeler le script de façon synchrone.** Le pont de gpui-shell ne passe que des données (`HostValue` : null, booléen, nombre, chaîne, tableau, objet), jamais une fonction ; l'hôte n'a aucun `call` public. Tout ce que le cœur lit en synchrone doit donc être **déclaratif** (`appliesTo: { viewType }`, fournisseur de documents à gabarit de titre, disponibilité d'une commande) ou **poussé** par le plugin (cache côté Rust). Rust → JS passe par une boîte aux lettres lue au rendu suivant (une trame de latence).
3. **Une application gpui-shell = UNE vue racine, montable UNE fois.** Un plugin qui a N onglets/widgets = N chargements du module (~10 ms chacun), N copies de l'état du plugin. L'état partagé entre feuilles doit vivre côté Rust (ou être poussé), pas dans la classe du plugin.
4. **Contrat nouveau, pas de compatibilité Obsidian.** ES module, `export default class` ; l'API arrive en ARGUMENT d'`onload(fragment)` ; pas de `require`, pas de DOM. Les bundles CommonJS écrits contre `obsidian` ne tournent pas (ils ne tourneraient pas non plus sans DOM).
5. **Dépendances.** `gpui-shell = { version = "=0.8.1", default-features = false, features = ["quickjs", "components"] }` : la feature par défaut `webview` tire `gpui-fast`, un AUTRE GPUI (types incompatibles avec ceux de gpui-kit). ★ Si S2 (webview) active `gpui-fast`, il faut l'activer partout (`gpui-kit/gpui-fast` ET `gpui-shell/gpui-fast`) : à traiter ensemble. gpui-shell se dit « experimental » : épinglé exactement, comme gpui-kit (ADR-001).

## Architecture essayée (ce que fait `host.rs`)

```text
.fragment/plugins/<id>/main.js ──copie──▶ <staging>/<id>/plugin-main.js
                                          <staging>/<id>/main.js  = bootstrap.js (View racine + objet `fragment`)
Policy { aucune capacité, HostModule "fragment-host" }  ── une politique PAR plugin
ShellRuntime::load_application(<staging>/<id>, "main.js") ─▶ mount_application ─▶ Entity<ScriptView>
JS ─▶ Rust : fonctions de données de `fragment-host` ─▶ Op ─▶ sync() ─▶ CommandManager / DocumentManager / Registry
Rust ─▶ JS : Shared::post(message) ─▶ boîte aux lettres lue par `render` ─▶ ScriptView::refresh
```

- Pourquoi un bootstrap : gpui-shell veut qu'un `main.js` exporte par défaut une sous-classe de `View` ; un plugin Fragment exporte une classe `Plugin`. Le staging (une copie par chargement) donne aussi une **génération** de modules neuve à chaque rechargement ; il évite d'écrire dans le coffre.
- `onload` peut tourner au chargement du module (`Mode::Load`, **sans fenêtre**) ou dans `View.init` (`Mode::Init`, `fragment.spawn` disponible). Les deux marchent.
- Le plugin est un `PluginBase`/`Plugin` de `fragment-core` : ses commandes passent par `Plugin::add_command` (id préfixé, nom préfixé), ses retraits par la pile de nettoyages (`unload()`), sans `onunload` côté plugin.

## Par point

### P1 — Charger `.fragment/plugins/<id>/main.js` et exposer l'API `fragment` — GO-AVEC
- Hypothèse : un `HostModule` suffit à exposer `registerView`, `addCommand`, `registerDocumentProvider`, `addWidget`.
- Essai : bootstrap JS + module hôte `fragment-host` (15 fonctions de données) ; tests `jumeau_*`, `onload_sans_fenetre`, `cycle_de_vie_par_plugins_et_community_loader`.
- Mesure : chargement complet du jumeau (release) : staging 2,3 ms + création de la VM 8,0 ms + `load_application` 10,6 ms + montage 0,4 ms + 1er dessin ~0 ms = **21,4 ms** (débogage : 46,4 ms). `onload` sans fenêtre : OK (`load_ms` 10,3).
- Verdict : GO-AVEC. L'API passe en ARGUMENT d'`onload` ; le module hôte ne porte que des données ; une fonction dans un objet échouerait à la frontière (le bootstrap garde les callbacks en JS et n'envoie que des descriptions JSON). ★ `JSON.stringify` jette silencieusement une fonction placée dans un arbre : le bootstrap devra la refuser (dette T10).

### P2 — `registerView(type, {render})` : arbre déclaratif JSON → composants gpui-kit — GO
- Hypothèse : le plugin renvoie des données (`{t, style, children}`), pas des éléments ; le pont les rejoue en `div/h_flex/v_flex` et en composants du catalogue (`Text`, `Label`, `Button`, `Progress`).
- Essai : `vue_declarative_rendue_en_composants_gpui_kit` : vue de 3 lignes (17 nœuds, 669 octets de JSON). Instantané : `Button`, `Text`, `Label`, `Progress` présents ; un clic sur le `Button` exécute `demo:inc` dans le `CommandManager`, le message revient au script au rendu suivant, le compteur passe de 0 à 1.
- Mesure : voir P7. L'arbre traverse `JSON.stringify/parse` à chaque rendu (c'est aussi la frontière d'un hôte WASM : le repli garderait le même arbre).
- Verdict : GO. ★ Écarts constatés avec le README de gpui-shell : (1) `bg("primary")` est **refusé** (« not a color value ») alors que le README dit accepter les noms de jetons : le pont résout les jetons par `cx.theme().colors[nom]` au rendu, l'arbre garde des NOMS de jetons (portable) ; (2) `.on_click(...)` n'existe plus après un `.w()`/`.h()` sur un composant : l'ordre des appels compte ; (3) l'instantané de test (`debug_tree`) ne montre pas le texte des composants enregistrés (`Text`, `Label`), seulement celui des chaînes nues.

### P3 — `addCommand` branché sur le CommandManager de fragment-core — GO-AVEC
- Essai : `commande_branchee_sur_le_command_manager`. `demo:inc` apparaît dans `find_command` (nom « Demo: Incrémenter »), `list_commands`, s'exécute par `execute_command_by_id` ; un `throw` du plugin est rattrapé et journalisé sans toucher l'hôte ni les autres commandes ; `unload()` vide le `CommandManager`.
- Mesure : aller-retour (exécution → boîte aux lettres → rendu → callback JS → re-rendu) : **0,75 ms** (release), 3,7 ms (débogage), 100 itérations.
- Verdict : GO-AVEC. La commande JS s'exécute **au rendu suivant**, jamais en synchrone ; elle ne peut pas renvoyer de valeur à Rust. `check_callback`/`editor_check_callback` (disponibilité calculée par le plugin) sont donc à déclarer (`available: …`) ou à pousser, pas à calculer à la demande.

### P4 — `registerDocumentProvider` — GO-AVEC (déclaratif)
- Essai : `fournisseur_de_documents_declaratif`. `DocumentProvider` (cœur) est synchrone (`title`, `exists`, `open`) : le fournisseur JS est un gabarit `{ scheme, view, title: "Démo {rest}", icon }`. `view_state_for("demo:abc")` rend `ViewState { type: "demo:list", state: { key: "demo:abc" } }`, `title` rend « Démo abc ».
- Verdict : GO-AVEC. Un fournisseur dynamique (titre d'une conversation, existence) doit **pousser** ses données (`provider.setTitle(key, …)`) vers un cache Rust ; il ne sera jamais interrogé.

### P5 — `addWidget({ mode: "gutter", anchor })` et `registerLayer` — GO
- Essai : `widget_de_marge_declaratif`, et le calque du jumeau. Le widget s'inscrit dans un registre (`mode: "gutter"`, `side: "right"`, `pos`, `dy`), son contenu est un arbre rendu par la même chaîne que P2 ; le retrait (`handle.remove()`, détachement du calque, `unload`) le désinscrit.
- Verdict : GO. `appliesTo` est une DONNÉE (`{ viewType }`) : l'hôte l'évalue en Rust (un PDF est exclu, un markdown convient) ; le prédicat `(view) => …` de Fragment ne passe pas la frontière.

### P6 — Le jumeau de `gutter-globe-plugin.cjs`, chargé puis rechargé à chaud — GO
- Essai : `jumeau_gutter_globe_charge_puis_recharge_a_chaud` (`tests/fixtures/plugins/e2e-globe`). Le calque est inscrit, s'attache à une vue markdown, pose le widget (hauteur 200), se détache ; puis le `main.js` est modifié sur le disque (200 → 260) et rechargé 5 fois : exactement un calque, la version 260 gagne ; et la même chose via `Plugins::reload_plugin`.
- Mesure (release, moyenne de 5) : rechargement avec **VM neuve 23,0 ms** (22,4–23,8) ; en **gardant la VM 13,0 ms** (11,3–19,0). Décomposition : staging 2,3 + VM 7,8 + module 9,7 + montage 0,2 ms. Via `Plugins::reload_plugin` : 22,8 ms. Débogage : 50,0 / 20,4 / 48,6 ms.
- Verdict : GO. Garder la VM est plus rapide mais laisse survivre les écritures sur `globalThis` d'une génération à l'autre : préférer la VM neuve (23 ms) pour un rechargement fidèle. Le rechargement natif de gpui-shell (`watch`) conserverait l'état de la vue mais surveille le dossier de staging : non utilisé.

### P7 — Coût d'un rendu de vue — GO
- Essai : `cout_d_un_rendu_de_vue`, vue `demo:list` de 1, 20 et 100 lignes (7, 102, 502 nœuds ; 330 octets, 3,6 Ko, 17 Ko de JSON), 100 itérations.

| Nœuds | Redessin SANS invalidation | Invalidation + dessin | dont script (rendu + JSON + pont) | dont matérialisation |
|---|---|---|---|---|
| 7 | 141 µs | **476 µs** | 163 µs | 20 µs |
| 102 | 1,14 ms | **3,20 ms** | 826 µs | 143 µs |
| 502 | 5,38 ms | **13,8 ms** | 3,45 ms | 641 µs |

  (release ; en débogage : 2,7 / 23,9 / 98,7 ms pour les mêmes tailles.)
- Verdict : GO. Un redessin sans invalidation **n'entre jamais dans la VM** (0 rendu de script sur 100 redessins, assertion du test) : le coût du script suit ce que fait le plugin, pas la cadence des trames. ~7 µs de script par nœud ; une vue de 100 nœuds rend en 3 ms (un budget de 16,6 ms à 60 Hz), une de 500 nœuds consomme presque tout : les longues listes passent par un composant virtualisé (`MessageScroller`, `List`), pas par un arbre à plat.

### P8 — Isolation : ni système de fichiers, ni réseau — GO
- Essai : `isolation_aucune_capacite` (plugin `evil`, aucune capacité accordée), `isolation_temoin_avec_lecture_accordee`, `isolation_du_tas_entre_plugins`, `plugin_qui_boucle_est_coupe_par_le_chien_de_garde`.
- Mesure (12 sondes, toutes dans le journal imprimé `S3|isolation|…`) : **REFUSÉES** — `fs.readFile` d'un fichier témoin (« filesystem read is not granted »), `readFile("C:/Windows/win.ini")`, `readdir`, `writeFile` (le fichier n'est pas créé), `fetch("http://127.0.0.1:port/")` (le `TcpListener` du test ne reçoit AUCUNE connexion), `import("../escape.js")` (« resolves outside the application directory »), `require(fs | child_process | net | http | os)` (« `require` is not available in the shell »), `process.run` (« not granted »). Globaux `std`, `os`, `Deno`, `XMLHttpRequest` : `undefined` ; `eval` : absent ; `new Function` : « disabled » ; `process` existe (objet) mais gated.
- Le témoin : avec une SEULE racine de lecture accordée, la même sonde lit « SECRET-CANARY » et liste le dossier, alors que l'écriture et la lecture hors racine restent refusées : le test n'est pas vacueux, et l'octroi est granulaire.
- Tas : une VM par plugin → le second plugin voit `undefined` ; **une VM partagée → il lit `secret-de-A`**. D'où la condition 1.
- Chien de garde : récursion sans fond → `RangeError` rattrapable en 0,7 ms ; boucle infinie → coupée à **2 s** (budget de rendu, privé), avec un message clair, et le plugin/l'hôte repartent (la commande suivante s'exécute). ★ La fenêtre est gelée pendant ces 2 s (5 s pour un gestionnaire d'événement, 10 s pour l'évaluation du module) : budgets non configurables par l'hôte.
- Verdict : GO. Limites connues : pas de plafond mémoire par VM constaté ; en release gpui-shell compile le JS chaud en code natif (JIT) sans option publique pour le désactiver (en débogage il est coupé) : surface à garder à l'œil.

### P9 — Cycle de vie par `Plugins` + `CommunityLoader` — GO
- Essai : `cycle_de_vie_par_plugins_et_community_loader`. `FsStorage` (un `PluginStorage` sur un dossier) + `SpikeLoader` (un `CommunityLoader` qui range un plugin inerte ; `on_load` évalue le JS, `on_unload` rejoue les nettoyages). Sans `community-plugins.json` : mode restreint, rien de chargé ; `enable_plugin_and_save` : chargé (21,9 ms release), 2 commandes, un fournisseur ; `reload_plugin` : 22,8 ms, toujours 2 commandes (pas de doublon), nom mis à jour ; `disable_plugin_and_save` : tout est retiré.
- Verdict : GO. Le trait `CommunityLoader` (synchrone, sans contexte GPUI) suffit parce que `onload` n'a pas besoin de fenêtre (`Mode::Load`).

### P10 — Plusieurs feuilles d'un même plugin — GO-AVEC
- Essai : `feuilles_multiples_d_un_meme_plugin`. Un onglet de plus = un `load_application` de plus (mode `Leaf` : le module est réévalué, rien n'est ré-inscrit).
- Mesure (release) : **20,6 ms** par feuille avec une VM neuve (8 + 9,6 + 0,3 ms) ; ~10 ms dans une VM déjà là. Affiché : `parent_voit=true, feuille_voit=false` : après `demo:inc`, le compteur du plugin parent vaut 1 et celui d'une feuille reste 0.
- Verdict : GO-AVEC. L'état d'un plugin vit dans la VM de chaque feuille : tout état partagé (liste de conversations, compteur) doit passer par un magasin côté Rust (`fragment.store`) ou par des messages, pas par des champs de la classe du plugin. C'est la plus grosse contrainte de modèle de T10.

## Mémoire
`memoire_par_vm` : 20 chargements du jumeau (sans fenêtre). **~2,6 Mo par VM** (2 577 Ko release, 2 621 Ko débogage) ; chaque module de plus dans une VM déjà là : 0 à 77 Ko. 30 plugins = ~80 Mo de VM : acceptable pour du natif, à garder en tête si on voulait « un plugin par fenêtre/feuille » (une feuille ne doit PAS créer de VM : la réutiliser).

## À demander à l'amont (gpui-shell) ou à contourner
- Un appel hôte → script (au moins un `dispatch` sur une vue) : supprimerait la boîte aux lettres et les fournisseurs déclaratifs ; sans lui, le contrat de T10 reste « données seulement ».
- Budgets du chien de garde (2 s / 5 s / 10 s) et configuration du JIT : privés ; un patch minimal les rendrait configurables (fork n°2 = ADR, à n'envisager que si 2 s de gel pose problème).
- README/versions : les noms de jetons de couleur sont refusés en 0.8.1 malgré le README ; relire les tests d'intégration de la version épinglée, pas la documentation.

## Repli WASM (non déclenché, chiffré pour mémoire)
`wasmtime` + le MÊME arbre déclaratif JSON (P2 le garantit : l'arbre passe déjà `JSON.stringify/parse`) et les mêmes fonctions de données (15). Ce qui change : le plugin est compilé en WASM (Rust/AssemblyScript/JS via Javy), l'hôte implémente l'import `fragment-host` et une exportation `render`/`on_message` (un appel hôte → plugin SYNCHRONE, ce que QuickJS via gpui-shell n'offre pas : `check_callback` et `DocumentProvider` redeviennent possibles). Estimation : 2 à 3 semaines pour l'hôte + ABI, 1 semaine pour un SDK JS (Javy) ; perte : l'écosystème « un `main.js` en clair » et le catalogue communautaire actuel (à recompiler). À n'ouvrir que si l'une des conditions 2 ou 3 devient bloquante pour T10.

## Reproduire
- Tests : `cargo test -p fragment-plugins --test spike_s3 -- --nocapture --test-threads=1` (Windows ; ajouter `--release` pour les mesures). Les lignes `S3|…` alimentent ce document.
- Exemple : `cargo run -p fragment-plugins --example spike_s3 [dossier-du-plugin]` — fenêtre réelle, vue `demo:list` (12 lignes), bouton « +1 » → `CommandManager` → script. Compilé et lancé 8 s sur le runner (la fenêtre reste vivante) ; non piloté en CI.
