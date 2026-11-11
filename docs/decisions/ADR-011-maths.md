# ADR-011 — Les maths sont rendues par MathJax 3.2.1 exécuté dans QuickJS

Statut : proposé (fr-t3-maths, 11/11/2026). Mesures : `docs/spikes/S5-maths.md`. Une dette de fidélité y est chiffrée : divergence à trancher par l'orchestrateur (program.md §8).

## Contexte

Fragment rend `$…$` et `$$…$$` avec `mathjax-full` 3.2.1 (TeX + tous les paquets, sortie SVG, cache de fontes local), dans le DOM de Chromium. program.md §4 autorisait `rex` ou `typst-math`, en supposant qu'un moteur natif suffirait. Le spike S5 a mesuré trois voies sur 12 formules × 2 thèmes, contre des captures de Fragment.

- `rex` n'existe pas sur crates.io sous ce nom : le nom appartient à un autre projet, et ReX, le moteur de maths, n'a jamais été publié. La voie B a donc été tenue par `latex-rust` 3.0.3.
- `typst-math` est un nom réservé, vide. La voie C a utilisé `typst` 0.15.1, avec des formules traduites à la main.

## Décision

1. **La voie A est retenue.** Le bundle `mathjax-full` 3.2.1 (même configuration que Fragment, adaptateur « lite » sans DOM) tourne dans QuickJS (`quickjs-jit` 0.14.1, déjà au lock par gpui-shell, utilisé SANS son JIT). Il produit le SVG, qui est peint par resvg.
   - Le SVG est identique à celui de Fragment pour les 24 captures (test `mathjax_dans_quickjs_produit_le_svg_de_fragment`, ids de cache renumérotés).
   - La boîte est identique au 1/64 px (test `mathjax_meme_boite_que_chromium`).
2. **Règles de mise en page, à reprendre telles quelles par `fr-t3-widgets-inline` et `fr-t3-widgets-blocs`.**
   - 1 ex = `sxHeight` de DM Sans (504/1000) × le corps du conteneur. Les longueurs `Nex` sont tronquées à 1/64 px, comme le LayoutUnit de Chromium.
   - Le `<svg>` est peint à son origine ARRONDIE au pixel, mais à sa taille de mise en page.
   - La ligne de base se lit dans le `vertical-align` du SVG.
   - `currentColor` prend la couleur du texte (jeton `--text-normal` via `fragment-theme`).
3. **Exécution.**
   - Une VM QuickJS par application, sur un FIL DÉDIÉ d'au moins 4 Mio de pile : `set_max_stack_size` de 2 Mio en release, 4 Mio en débogage, sinon « Maximum call stack size exceeded ».
   - Un cache LRU (source, mode) → SVG, comme `mathjax.ts`, est obligatoire. Un rendu à froid coûte de 4 à 57 ms selon la formule. Le démarrage de la VM et du bundle coûte environ 220 ms : il est donc paresseux, au premier `$` rencontré, hors du fil d'interface.
   - Le JIT de quickjs-jit est REFUSÉ : 2,8 s de démarrage, 1 s par formule.
4. **Licences.** MathJax et mhchemparser sont en Apache-2.0 (bundlés, `LICENSE-MathJax` et `LICENSE-mhchemparser` livrés avec le binaire et dans la notice). QuickJS-ng et quickjs-jit sont en MIT. Pas de copyleft.
5. **Dette de fidélité (DIVERGENCE, à l'orchestrateur).** Il ne reste que l'anticrénelage des tracés : tiny-skia (resvg) contre Skia (Chromium).
   - Dans les boîtes des formules : 19,1 % des pixels diffèrent, 5,7 % au-delà de 16, delta max 82, encre de −4 à −8 %.
   - Hors anticrénelage : 0 pixel de géométrie.
   - Elle relève de la même catégorie que les glyphes d'ADR-009 : boîte de formule = boîte de glyphes, origine stricte, encre bornée à ±25 %.
   - Elle va dans la tâche « rasterizeur » de la phase 4. Un suréchantillonnage 2 × divise par deux les pixels au-delà de 16 (mesuré) : piste pour cette tâche.

## Conséquences

- `fr-t3-widgets-inline` promeut `crates/spike-maths/src/voie_a.rs`, l'asset `mathjax-svg.js` et sa recette dans `fragment-ui`, ou dans une crate `fragment-maths` qui ne dépend pas de gpui-kit, puis supprime la crate du spike.
- Le binaire grossit de 3,0 Mo (bundle 1,8 Mo + QuickJS) si gpui-shell n'est pas déjà lié, et de 1,8 Mo s'il l'est (T10). À comparer aux 2,6 Mo de la voie B et aux 45,6 Mo de la voie C (surtout les polices de typst-assets), pour une fidélité très inférieure.
- program.md §4 : ajouter `quickjs-jit` (via gpui-shell) et l'asset MathJax aux dépendances autorisées. `rex` et `typst-math` en sortent : ils n'existent pas sous ces noms.
- Repli, si l'orchestrateur refuse QuickJS : la voie C (typst). Elle coûte un convertisseur LaTeX → typst (mitex) et une dette de géométrie, jusqu'à 17 px de largeur par formule et 18 % des pixels au-delà de 16.
