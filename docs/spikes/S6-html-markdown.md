# S6 — Le HTML dans le Markdown (rendu natif de gpui-kit, WebView2 par bloc, ou mixte)

Tâche `fr-t3-html`. Verdict global : **GO-AVEC** l'option **mixte**, c'est-à-dire un traducteur HTML → éléments GPUI écrit pour ce dépôt (HTML assaini, CSS en ligne, `details`), le SVG peint par le rastériseur de GPUI, un interpréteur SMIL minimal pour les animations, et une carte de repli pour les `<iframe>`. Le rendu HTML natif de gpui-kit 0.8.1 TEL QUEL est **NO-GO** (il perd la moitié des cas). Une WebView2 par bloc est **NO-GO** comme moyen d'intégration dans le flux d'une note (S2). Décision proposée : `docs/decisions/ADR-012-html-markdown.md`.

Code : `crates/fragment-ui/tests/spike_s6.rs` (6 tests, headless), `tests/spike_s6/smil.rs` (interpréteur), `tests/spike_s6/fixtures/` (12 fixtures réécrites pour ce dépôt, ADR-007), `tests/spike_s6/capture/capture.mjs` et `tests/spike_s6/reference/` (Chromium). Commande : `cargo test -p fragment-ui --test spike_s6 -- --nocapture --test-threads=1` (Windows). Les lignes `S6|…` alimentent ce document.

## Hypothèse

Fragment rend le HTML du Markdown (`div` stylée, SVG animé à la couleur du thème, `details` ouvrable, `span` coloré, `mark`, `u`, `sub`, `br`, commentaires, `iframe`) par le DOM de Chromium, après DOMPurify. T3 (program.md §5) suppose un « rendu natif de gpui-kit » avec une politique d'assainissement. L'hypothèse à éprouver : le HTML natif de gpui-kit suffit-il, sinon que faut-il y ajouter, et une WebView2 par bloc est-elle une sortie de secours ?

## Méthode

- **Fixtures.** 12 fragments écrits pour le spike d'après les cas de la spec `markdown-html` (div stylée, SVG statique, SVG SMIL, `details`, balises en ligne, commentaire, table HTML, `iframe`, pièges d'assainissement, image, animation CSS, mise en page flex).
- **Natif.** Ce que `gpui-base` (vendu, fork n°1 : `text::format::html`) fait de chaque fixture est lu dans l'arbre `BlockNode` que `parse_markdown` produit (le même analyseur que `TextView`, html5ever + `markdown`). Chaque fixture a une liste de traits attendus ; un trait est « tenu » ou « perdu ». Pour le CSS en ligne, la mesure est différentielle : l'arbre de la source est comparé à celui de la même source dont les `style="…"` sont remplacés par des blancs de même longueur. Arbre identique = CSS ignoré.
  Quatre traits ne se lisent pas dans l'arbre et sont posés à « perdu » d'après le code : animation SMIL, ouverture d'un `details`, `sub`/`sup`, flex. Raison : `TextMark` n'a ni `vertical-align` ni couleur, et `BlockNode` n'a ni état d'ouverture ni mise en page en ligne.
- **SVG et SMIL.** `gpui_kit::SvgRenderer` (celui qui peint les `ImageFormat::Svg`, resvg 0.46) rastérise le SVG de la fixture, composé sur le fond de page. Les variables du thème (`var(--color-accent)`, `currentColor`) sont substituées dans le source. Un interpréteur SMIL de 120 lignes (`smil.rs` : `animate` numérique et `animateTransform translate`, interpolation linéaire des `values`, `repeatCount="indefinite"`) fige le SVG à l'instant `t`.
- **Référence.** `capture.mjs` charge chaque fixture dans Chromium (Chrome installé, Playwright de `Fragment/app/node_modules` en lecture seule), page sombre de Fragment (fond `#1e1e1e`, texte `#dcddde`, 16 px, accent `#7c83ff`), DPR 1. Mesures : rectangle du conteneur, texte, PNG du SVG, et les valeurs `animVal` aux instants 0,5, 1,0 et 1,5 s. NB : c'est Chromium sur une page de test, pas l'application Fragment elle-même ; les polices sont celles du système, ce qui compte pour `<text>`.
- **WebView2.** Non remesurée : les chiffres sont ceux de S2 (`docs/spikes/S2-webview.md`), relus pour le cas « un bloc dans le flux d'une note ».

## Résultats

### 1. Le rendu natif tel quel : 17 traits sur 34 (50 %)

| Fixture | Traits tenus | Ce qui est perdu (mesuré) |
|---|---|---|
| f01 div stylée | 2/3 | couleur, fond, bordure, padding, rayon : 5 déclarations, arbre identique sans `style` |
| f02 SVG statique | **0/2** | rien n'est dessiné ; le `<text>` du SVG FUIT en texte brut (« fin ») dans la note |
| f03 SVG SMIL | **0/2** | un paragraphe vide ; aucune animation |
| f04 `details` | 1/3 | toujours déplié (le texte « masqué » est visible), pas d'ouverture au clic |
| f05 en ligne | 3/7 | `span` coloré (style ignoré), `sub`, `sup`, `kbd` (rendu en texte simple) |
| f06 commentaire | 2/2 | tenu (bloc vide) |
| f07 table HTML | 2/3 | fond de cellule et alignement ignorés (2 déclarations) |
| f08 `iframe` | **0/1** | disparaît (bloc vide) |
| f09 pièges | 2/3 | `<script>` et `onclick` tombent d'eux-mêmes, MAIS `href="javascript:…"` survit en `LinkMark` : le natif n'assainit pas |
| f10 image | 2/2 | tenu (`data:`, largeur 80 px) |
| f11 animation CSS | 2/3 | le `<style>` est retiré ; l'animation ne joue pas |
| f12 mise en page flex | 1/3 | colonnes empilées, fonds ignorés (6 déclarations) |

14 déclarations CSS en ligne sur 4 fixtures : **0 honorée**. Seul `mark` lit une couleur (`color`, `background-color` hexadécimal et 4 noms) ; seules `img`/`td` lisent `width`/`height`.

★ Deux défauts plus graves que la perte de style : (1) un SVG inséré fait fuiter ses `<text>` dans le texte de la note, donc un schéma d'agent s'afficherait comme des mots en vrac ; (2) un lien `javascript:` reste cliquable côté données, donc l'assainissement doit précéder l'analyse, il ne peut pas la suivre.

### 2. SVG statique par le rastériseur de GPUI (mixte) : 4,3 % des pixels au-delà de 16

`f02`, 160 × 60 px, contre la capture de Chromium : 453 pixels sur 9 600 diffèrent (4,72 %), **416 de plus de 16** (4,33 %), delta max 222. La carte des différences (`diff-*.png`, écrite dans `$CARGO_TARGET_TMPDIR/spike-s6/`) ne montre que des contours (rectangle arrondi, cercle, trait de 3 px, glyphes de `<text>`) : de l'anticrénelage, pas de la géométrie. Même famille de dette que les glyphes d'ADR-009 et les maths d'ADR-011 (tiny-skia contre Skia). La taille est exacte (facteur 0,5 : `SvgRenderer` rastérise au double à 1,0).

### 3. SMIL interprété : valeurs exactes, 0,8 % de pixels différents

`f03` (cercle `animate cx` 10→110→10 sur 2 s, groupe `animateTransform translate` 0→60) :

| t | `cx` (nous / Chromium) | `tx` (nous / Chromium) | pixels ≠ | > 16 | delta max |
|---|---|---|---|---|---|
| 0,5 s | 60 / 60 | 15 / 15 | 0,81 % | 0,58 % | 181 |
| 1,0 s | 110 / 110 | 30 / 30 | 0,83 % | 0,58 % | 181 |
| 1,5 s | 60 / 60 | 45 / 45 | 0,81 % | 0,58 % | 181 |

Les valeurs animées sont identiques à 0,01 près (test d'assertion). Les pixels sont ceux du contour du cercle et du rectangle (anticrénelage).

**Coût par image** (figer + analyser + rastériser un SVG de 120 × 40, profil debug, portable de développement) : **1,35 ms**, soit 8 % du budget de 16,7 ms à 60 Hz. Mesure `release` non faite. Cela passe à l'échelle de la surface du SVG et du nombre de SVG animés visibles (dix SVG de cette taille : 13,5 ms en debug). Atténuations non éprouvées : garder le `ParsedSvg` de GPUI (`parse_svg` puis `render_parsed`) pour ne pas ré-analyser, limiter à 30 images/s, n'animer que ce qui est visible.

### 4. WebView2 par bloc : fidélité du moteur maximale, intégration impossible dans le flux

Le moteur est Chromium : tout ce que Fragment rend, la WebView le rend (non mesuré, c'est la référence). Mais S2 a établi que :

- `gpui-webview` ne montre RIEN sous Windows (la page est sous le rendu GPUI) ; il faut un POPUP possédé posé au-dessus de la fenêtre, et tout calque GPUI qui doit passer par-dessus est une fenêtre à part (S2 §6).
- Un bloc HTML dans une note défile avec le texte, est rogné par le viewport de l'éditeur, passe sous la barre d'outils, les menus et la sélection. Un popup n'est pas rogné par GPUI (clip partiel : `SetWindowRgn`, NON mesuré en S2) et ne défile pas avec lui (suivi de position à chaque image, NON mesuré).
- N blocs visibles = N popups et N contrôleurs WebView2. Premier message du pont : 142 ms à chaud (S2 §1). Mémoire par popup : NON mesurée.
- Le popup est invisible pour `render_to_image` et pour le harnais pixels : les scènes `t3-html-*` de `fr-t3-scenes-c` ne pourraient pas le juger. Pas de clic « dessous sur la bonne ligne » (S1 point 4) ni de curseur de texte autour.
- Le chat (T7) rend aussi du HTML : des dizaines de blocs par conversation.

Verdict : **NO-GO** pour un bloc dans le flux ; la WebView reste le bon outil pour une PAGE pleine feuille (T9, S2).

### 5. Tableau fixture × rendu

Légende : « natif » = gpui-kit 0.8.1 tel quel (mesuré, §1) ; « WebView » = WebView2 en popup par bloc (fidélité du moteur ; intégration : NO-GO, §4) ; « mixte » = traducteur écrit pour ce dépôt + rastériseur GPUI + SMIL minimal (SVG/SMIL : mesuré §2-3 ; les autres traits PROJETÉS d'après les capacités de GPUI et du traducteur à écrire, à confirmer par `fr-t3-html-rendu`).

| Fixture | Natif (mesuré) | WebView par bloc | Mixte |
|---|---|---|---|
| f01 div stylée | 2/3 : texte, gras ; CSS perdu | moteur identique | 3/3 projeté (`div().bg().border().p()` sur les déclarations lues) |
| f02 SVG statique | 0/2 ; texte du SVG fuit | moteur identique | 2/2 **mesuré** : 4,33 % px > 16 (contours) |
| f03 SVG SMIL | 0/2 | moteur identique | 2/2 **mesuré** : valeurs exactes, 0,58 % px > 16 ; 1,35 ms/image (debug) |
| f04 `details` | 1/3 : toujours ouvert | moteur identique | 3/3 projeté (état ouvert/fermé dans le widget) |
| f05 en ligne | 3/7 | moteur identique | 7/7 projeté (couleur, `sub`/`sup` par décalage de ligne de base, `kbd`) |
| f06 commentaire | 2/2 | moteur identique | 2/2 |
| f07 table HTML | 2/3 | moteur identique | 3/3 projeté |
| f08 `iframe` | 0/1 | moteur identique | **0/1** : carte-lien de repli (perte) |
| f09 pièges | 2/3 : `javascript:` survit | moteur identique après assainissement | 3/3 : la politique de `fragment-markdown` agit avant l'analyse (test) |
| f10 image | 2/2 | moteur identique | 2/2 |
| f11 animation CSS | 2/3 | moteur identique | 2/3 : texte tenu, animation CSS perdue |
| f12 flex | 1/3 | moteur identique | 3/3 projeté (`flex`, `gap`, `flex: n` → `div().flex()`), `grid` non couvert |
| **Total traits** | **17/34 (50 %)** | 34/34 présumé, intégration NO-GO | **32/34 (94 %)**, dont 4 traits SVG/SMIL mesurés |

### 6. Assainissement

`la_politique_assainit_les_pieges` : la politique portée (`fragment_markdown::sanitize_html`) refuse `script`, retire `onclick` et refuse l'URL `javascript:` d'un `href`, et garde `svg`. Elle doit s'appliquer AVANT toute analyse de rendu (§1 : le natif laisse passer `javascript:`).

## Écarts de fidélité à remonter (program.md §8)

1. **`<iframe>` (YouTube…) : perdu.** Fragment l'affiche (CSP `frame-src https:`). Le mixte met une carte-lien ; un lecteur vivant exigerait une WebView popup activée au clic (machinerie de T9), hors périmètre du bloc de note.
2. **Animation CSS (`@keyframes`, `animation:`), sélecteurs de `<style>` : perdus.** Le mixte rend l'état statique ; seules les animations SMIL sont jouées.
3. **SMIL au-delà du sous-ensemble : perdu** (`animateMotion`, `keyTimes`, `calcMode`, `begin`, `set`, `animateTransform` autre que `translate`). Couvrir `rotate` et `scale` coûte peu ; `animateMotion` plus.
4. **Anticrénelage des SVG : 4,3 % des pixels au-delà de 16** sur un SVG de test, de l'ordre des 5,7 % des maths (ADR-011). Même catégorie de dette que les glyphes d'ADR-009.
5. **CSS autre que le sous-ensemble** (`grid`, positions absolues, `transform`, filtres, dégradés CSS) : perdu.

## Coût estimé du mixte

À confirmer par `fr-t3-html-rendu` (tâche lourde de T3) : traducteur HTML assaini → éléments GPUI avec un lecteur de déclarations CSS (6 à 8 jours), SVG par le rastériseur et substitution des variables du thème (1 jour), interpréteur SMIL étendu (`rotate`, `scale`, `set`) et cadence (2 jours), `details` et bouton « </> » (1 jour), carte de repli de l'`iframe` (0,5 jour). Dépendance directe nouvelle probable : `html5ever` et `markup5ever_rcdom` (déjà au lock par `gpui-base`).

## Ce qui n'a pas pu être observé

Rendu réel dans une fenêtre (le spike lit l'arbre, il ne peint pas : hauteurs, retours à la ligne et polices ne sont pas comparés à Chromium) ; mesures `release` ; mémoire et coût d'une WebView2 par popup ; clic et sélection dans un bloc natif ; thème clair ; les fixtures du chat (réponses réelles de l'agent) ; `<svg>` avec `<filter>` ou `<image>` ; un SVG animé très grand.
