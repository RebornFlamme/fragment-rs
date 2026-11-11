# S5 — Les maths au plus près de MathJax

Tâche `fr-t3-maths`. Code jetable : `crates/spike-maths` (promu ou supprimé par `fr-t3-widgets-inline`). Décision proposée : `docs/decisions/ADR-011-maths.md`.

## Hypothèse

Fragment rend `$…$` (`MathInlineWidget`) et `$$…$$` (`MathBlockWidget`) avec `mathjax-full` 3.2.1 : TeX + `AllPackages`, sortie SVG, `fontCache: "local"`, `currentColor`, et la règle `.cm-math-*` d'`editor.css`. Le seul moyen d'avoir les mêmes pixels est d'exécuter MathJax lui-même (voie A). Un moteur natif (B : TeX en Rust ; C : typst) laisserait une dette de géométrie. Il fallait la chiffrer.

## Méthode

- **Formules** : 12 formules écrites pour le spike, d'après les cas d'`editor-hell` et de `markdown-html`. Elles comprennent puissance, lettres grecques, racine, Euler, fraction en ligne, `\mathbb`, fraction, somme, intégrale, `pmatrix`, `\ce` et `cases`, soit 6 en ligne et 6 en bloc. Elles sont listées dans `src/formules.rs` et `capture/formules.ts`.
- **Référence**
  - `capture/maths.spec.ts` pilote Fragment par son harnais e2e (`lancerLecteur`), avec le Playwright de `Fragment/app/node_modules`. Le serveur est la Vite enveloppe de `pixels/` (cache hors de Fragment). La capture se fait sous le verrou `C:/dev/fragment-rs-pixels-verrou`, à DPR 1, en sombre et en clair.
  - Par formule : un PNG rogné à ±2 px autour du `<svg>`, et `mesures.json`, qui contient le rectangle exact, l'ex et l'em du conteneur, les couleurs, et le `<svg>` du DOM. Ces données sont dans `crates/spike-maths/reference/`.
  - Fragment n'a pas été modifié : `git status --porcelain` est identique avant et après, et les 334 entrées de `node_modules/.vite` sont inchangées.
  - Mesuré dans Fragment : corps 16 px, DM Sans, 1 ex = 8,0639 px, texte `rgb(223,222,217)` sur `rgb(20,21,23)` en sombre, et `rgb(26,27,30)` sur `rgb(247,246,242)` en clair. Le conteneur n'a pas de `font-size` propre.
- **Candidat** : chaque voie produit un SVG, que resvg 0.45 (celui de GPUI) peint sur le même fond, dans le même rognage. La comparaison est faite pixel à pixel (`tests/mesures.rs`). Les images candidates et les diffs sont écrites dans `$CARGO_TARGET_TMPDIR/spike-maths/<voie>/`.
- **Placement**
  - A : la boîte exacte de la référence.
  - B et C : la règle de l'application, c'est-à-dire la ligne de base de la référence, à gauche en ligne et centré en bloc.
  - Origine arrondie au pixel pour toutes les voies.
- **Temps** : build release, Windows (portable), une VM ou un moteur chaud, moyenne sur 10 passes des 12 formules.
- **Poids** : exemple `taille` (rend une formule) en release, comparé au même exemple sans voie.

## Essais et mesures

Colonnes : pixels qui diffèrent sur les 70 548 px des 24 rognages (B : 63 366, `\ce` non rendu), pixels dont l'écart dépasse 16, delta max, et écart de la boîte (largeur, hauteur) au pire.

| Voie | ≠ | > 16 | max | Δ boîte | 1er démarrage | 1re formule | chaud (moy.) | Poids | Couverture |
|---|---|---|---|---|---|---|---|---|---|
| **A** MathJax 3.2.1 / QuickJS | 19,1 % | **5,7 %** | **82** | **0** | 220 ms | 4 ms | 9,9 ms (3,6 à 57) | +3,0 Mo | 12/12 |
| A, origine fractionnaire | 19,7 % | 12,3 % | 172 | 0 | | | | | |
| A + JIT de quickjs-jit | | | | | 2,8 s | | 984 ms | | |
| **B** latex-rust, STIX Two Math | 25,3 % | 19,7 % | 221 | 19,3 × 8,8 px | 0,3 ms | 1,4 ms | 0,33 ms | +2,6 Mo | 11/12 (`\ce` refusé) |
| B, New Computer Modern Math | 25,7 % | 20,3 % | 221 | 13,4 × 7,5 px | | | | | 11/12 |
| **C** typst 0,15.1, NewCM Math | 25,2 % | 17,7 % | 221 | 16,8 × 3,9 px | 9,7 ms | 11 ms | 1,2 ms | +45,6 Mo | 12/12 (traduites à la main) |

Un delta max de 203 à 221 est le contraste plein entre le texte et le fond : des glyphes sont à un autre endroit.

### A — MathJax dans QuickJS : GO

- **Bundle** (`assets/mathjax-svg.js`, 1,8 Mo minifié)
  - Il est construit par esbuild 0.24.2 depuis `assets/mathjax-entree.js`. L'entrée reprend les imports de Fragment, avec `liteAdaptor` au lieu de `browserAdaptor`.
  - `mathjax-full@3.2.1` et `esbuild@0.24.2` ont été téléchargés depuis registry.npmjs.org dans un dossier vide du scratchpad.
  - ★ `--define:PACKAGE_VERSION="3.2.1"` est obligatoire : sinon `version.js` fait `require('../../package.json')` à l'exécution.
  - Recette : `esbuild mathjax-entree.js --bundle --minify --format=iife --target=es2020 --legal-comments=none --define:PACKAGE_VERSION="3.2.1"`.
  - Empreinte : sha256 `7df429e3…c23e`.
- **Moteur** : QuickJS-ng via `quickjs-jit` 0.14.1, déjà au lock par gpui-shell. Le nom de bibliothèque reste `rquickjs`. La crate est appelée en direct : le pont de gpui-shell ne permet pas d'appel synchrone (S3).
- **Résultats**
  - Le SVG est **identique** à celui du DOM de Fragment pour les 24 captures, une fois renumérotés les ids `MJX-<n>-` du cache local et triés les attributs de la balise.
  - La boîte est identique au 1/64 px.
  - La géométrie est donc exacte. La seule différence restante est l'anticrénelage des tracés : tiny-skia contre Skia.
- ★ **La pile.**
  - La pile JS par défaut de QuickJS (256 Kio) déborde dès `x^2`. Les constructeurs des mixins de MathJax s'empilent par nœud.
  - Il faut 2 Mio en release et 4 Mio en débogage ; 1 Mio échoue en release.
  - Le fil appelant a besoin d'autant de pile native. Le fil principal de Windows n'a que 1 Mio : il faut un fil dédié. Les tests tournent sur un fil de 16 Mio.
- ★ **Le calage, mesuré au 1/8 px** (`explorer_calage_voie_a`, ignoré par défaut)
  - Chromium tronque les longueurs à 1/64 px (LayoutUnit) : 2,282 ex × 8,0639 = 18,4018 donne 18,390625.
  - Chromium peint le `<svg>` à son origine arrondie au pixel, à sa taille de mise en page non accrochée. Accrocher la taille, ou garder l'origine fractionnaire, aggrave le diff.
  - 1 ex = `sxHeight` de DM Sans (504/1000) × 16 = 8,064 px, contre 8,0639 côté Chromium (écart de 1e-4 px, sans effet après la troncature).
- **Anticrénelage restant**
  - L'encre du candidat est de 4 à 8 % plus légère.
  - Peindre à 2 × puis moyenner divise par deux les pixels au-delà de 16, sans aller sous ~16 % de pixels différents. Au-delà de 2 ×, c'est pire.
  - Un trait fictif de 5 unités (`stroke-width`) ramène l'encre à ±3 %, sans gain net.
  - Conclusion : l'écart est un écart de rasterizeur (couverture analytique de Skia contre suréchantillonnage 4 × quantifié de tiny-skia), pas de géométrie.
- **Temps**
  - Démarrage : 220 ms (évaluation du bundle).
  - Par formule, à chaud et sans cache : de 3,6 ms (`x^2`) à 57 ms (`\ce`), 9,9 ms en moyenne.
  - resvg : 0,5 ms pour la somme.
  - Le cache LRU de Fragment (`mathjax.ts`, 500 entrées) est donc indispensable.
  - Le JIT de quickjs-jit est contre-productif ici : 2,8 s de démarrage, 1 s par formule (compilation cranelift à chaque palier).

### B — Moteur TeX en pur Rust : NO-GO

- ★ `rex` (program.md §4) n'est pas ReX : le nom `rex` de crates.io appartient à un langage fonctionnel. ReX, le moteur de maths, n'a jamais été publié (dépôt git seul, abandonné).
- La voie B a été tenue par `latex-rust` 3.0.3 (MIT OR Apache-2.0, Appendix G de TeX, polices OpenType MATH). C'est le plus rapide des trois : 0,33 ms par formule.
- Il n'a pas les fontes TeX de MathJax : avec STIX Two Math comme avec New Computer Modern Math, les boîtes diffèrent jusqu'à 19 px, et un cinquième des pixels s'écarte de plus de 16.
- `\ce` (mhchem) est refusé, et le moteur n'a ni `\newcommand` ni la couverture d'`AllPackages`.
- `mathtex` (XeTeX traduit) exige une installation TeX Live : écarté.

### C — typst : NO-GO (repli possible)

- typst 0.15.1 + typst-svg, avec New Computer Modern Math de typst-assets. `typst-math` sur crates.io est un nom réservé, vide.
- typst ne lit pas le LaTeX : les 12 formules ont été traduites à la main. En production, il faudrait mitex (Apache-2.0), dont la sortie appelle les définitions de son paquet typst. Ce coût n'a pas été mesuré.
- La fidélité est du même ordre que B : boîtes jusqu'à 17 px d'écart, 17,7 % des pixels au-delà de 16.
- Poids : +45,6 Mo, surtout les polices de typst-assets ; avec la seule police de maths, ce serait plusieurs Mo de moins, mais le compilateur reste lourd.

## Licences

| Composant | Licence | Où |
|---|---|---|
| MathJax (`mathjax-full` 3.2.1) | Apache-2.0 | bundle, `assets/LICENSE-MathJax` |
| mhchemparser 4.2.1 (dans le bundle, `\ce`) | Apache-2.0 | `assets/LICENSE-mhchemparser` |
| fontes TeX de MathJax (tracés SVG dans le bundle) | Apache-2.0 (avec MathJax) | |
| QuickJS-ng / `quickjs-jit` 0.14.1 | MIT | déjà au lock (gpui-shell) |
| resvg 0.45 | Apache-2.0 OR MIT | déjà au lock (gpui) |
| `latex-rust` 3.0.3 (B) ; STIX Two Math | MIT OR Apache-2.0 ; OFL-1.1 | spike seulement |
| `typst` 0.15.1, typst-svg, typst-layout, typst-assets (C) | Apache-2.0 ; polices OFL/GUST | spike seulement |

Aucune licence copyleft. Les dépendances nouvelles sont notées dans la PR.

## Verdict

**GO-AVEC pour la voie A.** C'est MathJax 3.2.1 lui-même dans QuickJS : le SVG de Fragment à l'octet près (aux ids de cache près), une géométrie exacte, toutes les formules, y compris `\ce`. Trois conditions :

1. MathJax tourne sur un fil dédié, avec au moins 4 Mio de pile native, une VM démarrée paresseusement (220 ms) et un cache LRU des SVG.
2. **Dette de fidélité, à remonter à l'orchestrateur.** L'anticrénelage des tracés diffère de Skia :
   - dans les boîtes des formules : 19,1 % de pixels différents, 5,7 % au-delà de 16, delta max 82, encre de −4 à −8 % ;
   - 0 pixel de géométrie.
   - Proposé : la traiter comme les glyphes d'ADR-009 (boîte de formule = boîte de texte, origine stricte, encre ±25 %), dans la tâche « rasterizeur » de la phase 4. Le suréchantillonnage 2 × est une première piste mesurée.
3. program.md §4 est corrigé : `rex` et `typst-math` sortent, `quickjs-jit` et l'asset MathJax entrent.

B et C sont NO-GO pour la fidélité : la géométrie diffère partout, et l'écart ne se réduira pas sans les fontes TeX de MathJax et ses règles.

## Repli

Si QuickJS est refusé, le repli est la voie C. Il coûte un convertisseur LaTeX → typst (mitex), +45 Mo (à réduire en n'embarquant que la police de maths) et une dette de géométrie déclarée (jusqu'à 17 px de largeur par formule), à faire accepter par l'utilisateur.

Si le poids de A gêne : le bundle peut se précompiler en bytecode QuickJS (non mesuré), ou se charger depuis un fichier à côté du binaire.

## Reproduire

- `cargo test -p spike-maths` : les trois tests de la voie A, la couverture des captures, et les tableaux en `--nocapture`. Pour des temps représentatifs, ajouter `--release`.
- `cargo test --release -p spike-maths -- --ignored --nocapture explorer` : le calage au 1/8 px, le trait et le suréchantillonnage.
- `cargo test --release -p spike-maths --no-default-features --features voie-a-jit -- --nocapture mesures_voie_a` : la variante avec le JIT.
- Captures : prendre le verrou, lancer la Vite enveloppe (`pixels/vite.reference.config.ts`, `--configLoader runner`), puis, depuis `Fragment/app`, `node_modules/.bin/playwright test --config <fragment-rs>/crates/spike-maths/capture/playwright.config.ts`. Contrôler `git status` de Fragment avant et après.
