# S7 — Le raster d'une page PDF : pdfium contre pdf.js

Tâche `fr-t4-raster`. Code : `crates/fragment-pdf/examples/spike_s7.rs` (fixture, rendus, comparaison), `crates/fragment-pdf/tests/spike_s7.rs`. Décision proposée : `docs/decisions/ADR-014-raster-pdf.md`.

**Verdict : le strict est inatteignable avec pdfium sans fork.** Les images (0 pixel) le sont ; le texte, les traits et les aplats ne le sont pas, pour quatre causes identifiées. La catégorie « raster de page PDF » est à demander, chiffrée plus bas, avec une limite : à 50 et 100 %, ses garde-fous voient mal un glyphe manquant.

## Bug trouvé en route, corrigé ici : rouge et bleu inversés

`Document::rendre_region` sortait le rouge en bleu. pdfium-render pose `FPDF_REVERSE_BYTE_ORDER` par défaut, donc le tampon « BGRA » sort déjà en RGBA ; le code échangeait encore les canaux 0 et 2. Les tests de `document.rs` ne rendaient que du noir et du blanc, où l'inversion ne se voit pas.
- Test : `rendu_page_coloree_en_rgba` et `region_coloree_en_rgba` (bandes rouge, vert, bleu purs) : rouges avant la correction, verts après.
- Correction minimale : l'échange est retiré, le drapeau est rendu explicite (`set_reverse_byte_order(true)`), l'alpha reste forcé à 255. `fragment-ui` (`pdf_pages/fil.rs::vers_image`) attendait bien du RGBA : rien à changer là-bas. `cargo test -p fragment-ui --test pdf_pages --test pdf_texte` reste vert (8 + 8).

## Méthode

- **Fixture** (`spike_s7 fixture`) : une page de 400 × 300 pt écrite à la main, plus deux lignes ajoutées par pdfium-render. Zones déclarées depuis la MISE EN PAGE (constante `ZONES`), jamais déduites des pixels :
  - texte : Helvetica NON embarquée (12 et 9 pt, noir et bleu ; pdf.js prend ses `standard_fonts`, pdfium sa police de repli) et DM Sans EMBARQUÉE (`fragment-ui/assets/fonts/DMSans-400.ttf`, 11 et 20 pt) ;
  - traits : horizontaux et verticaux de 0,5 et 1 pt, un 0,5 pt à x fractionnaire, une diagonale de 0,75 pt, un cercle bleu en Bézier ;
  - aplats : un rectangle à coordonnées entières, un à coordonnées fractionnaires, un gris ;
  - image : 8 × 8 px RGB peints sur 80 × 80 pt (sans `/Interpolate`).
  - Limite : les lignes DM Sans générées par pdfium-render perdent leurs espaces et l'accent du « é ». C'est identique dans les deux moteurs, la comparaison reste valable.
- **Référence pdf.js** : un `main.js` jetable (`C:/dev/fragment-rs-s7/capture/`) dans l'Electron 43 de `Fragment/app/node_modules`, avec le pdf.js de Fragment (pdfjs-dist 6.2.108, `pdf.min.mjs` et son worker). Options de `PdfView` : `standardFontDataUrl`, `cMapUrl`, `cMapPacked`, `wasmUrl`, `iccUrl` ; `enableHWA` par défaut (faux), donc canvas en CPU. Canvas de `round(pt × échelle)`, transform `[w/vw, 0, 0, h/vh, 0, 0]` comme `PdfView`, densité 1. Lecture par `getImageData`, RGBA brut. `userData` dans le dossier de travail. Fragment n'est pas modifié : `git status --short` est identique avant et après.
- **Candidat** : `spike_s7 rendre` (pdfium lié par l'exemple, 11 jeux d'options) et `spike_s7 document` (le vrai `Document::rendre_page`, dans un autre processus). `document` est identique au pixel près au jeu `defaut`.
- **Comparaison** : `spike_s7 comparer` donne, par échelle et par zone, les pixels différents, le delta max d'un canal et le delta moyen des pixels différents ; il écrit aussi une carte `carte-sNNN.ppm`. Les zones sont arrondies vers l'extérieur.

## Chiffres, configuration de `Document` (`defaut`)

Pixels différents / pixels de la zone, delta max, delta moyen sur les pixels différents.

| Zone | 50 % (200 × 150) | 100 % (400 × 300) | 200 % (800 × 600) |
|---|---|---|---|
| page | 14,6 %, max 172, moy 17,7 | 11,5 %, max 248, moy 19,8 | 9,1 %, max 209, moy 13,3 |
| texte | 21,5 %, max 172, moy 30,3 | 15,6 %, max 248, moy 40,1 | 10,0 %, max 209, moy 33,4 |
| traits | 8,3 %, max 59, moy 13,1 | 4,2 %, max 69, moy 12,7 | 1,7 %, max 72, moy 7,8 |
| aplats | 41,5 %, max 116, moy 3,7 | 41,0 %, max 132, moy 2,3 | 40,7 %, max 143, moy 1,6 |
| image | **0** | **0** | **0** |

## Les causes

1. **Texte : deux rasterizeurs, et pdf.js est en LCD.** Le canvas de pdf.js est opaque, donc Chromium y rastérise le texte en sous-pixel ClearType (franges colorées à l'agrandissement, `zoom-texte-s200.png`). pdfium le rend en niveaux de gris, avec une encre plus maigre : −34 % (Helvetica 12), −27 % (Helvetica 9), −23 % (DM Sans 11), −17 % (DM Sans 20) à 100 %. Les boîtes d'encre (couverture ≥ 10 %) coïncident à ±1 px : les glyphes sont au même endroit. La position est juste, c'est l'anticrénelage qui diffère.
   - ★ Le texte de la RÉFÉRENCE dépend du réglage ClearType de la machine qui la capture (portable de l'utilisateur).
2. **Aplats : pdfium accroche les rectangles à la grille.** Un rectangle plein aligné aux axes, à bord fractionnaire (210,3), sort sans anticrénelage chez pdfium : le pixel de bord est plein (51, 179, 77), alors que pdf.js le mélange (111, 201, 129). C'est l'origine du delta max de 116 à 143. Aucun drapeau de rendu n'y touche.
3. **Aplats : l'arrondi des couleurs.** Une composante dont `c × 255` tombe sur ,5 est arrondie au pair par pdf.js (`Uint8ClampedArray`) et vers le haut par pdfium : 0,3 → 76 contre 77, 0,7 → 178 contre 179. Cela fait un niveau sur tout l'intérieur d'un aplat, d'où les 41 % de pixels et la moyenne de 1,6 à 3,7.
4. **Traits : l'anticrénelage, au niveau près sur les droites, jusqu'à 72 sur les courbes.** Les deux moteurs élargissent à 1 pixel physique un trait plus fin, et les traits droits coïncident à un niveau près (128 contre 127, 192 contre 191). La diagonale et le cercle divergent davantage : leur couverture n'est pas calculée de la même façon.

## Options de pdfium essayées (même tableau, colonnes « texte » et « traits »)

| Jeu | Texte 50 / 100 / 200 % | Effet |
|---|---|---|
| `defaut` | 21,5 / 15,6 / 10,0 %, moy 30 / 40 / 33 | référence |
| `lcd` (BGRA) | identique à `defaut` | ★ le LCD de pdfium n'agit pas sur un bitmap avec alpha |
| `bgrx` | identique à `defaut` | le format seul ne change rien |
| **`lcd-bgrx`** | 22,2 / 16,1 / 10,4 %, **moy 23 / 28 / 21, max 121 / 237 / 187** | le meilleur : delta moyen −25 à −37 %, mais un peu plus de pixels touchés |
| `sans-lissage-texte` | 19,1 / 13,1 / 7,7 %, max 255, moy 89 / 90 / 77 | moins de pixels, glyphes crénelés : pire |
| `sans-lissage-chemins` | traits 10,5 / 5,3 / 1,8 %, moy 96 / 93 / 62 | pire |
| `sans-lissage-image` | identique (image déjà à 0) | — |
| `sans-texte-natif`, `impression`, `demi-teinte` | identiques à `defaut` | — |
| `gris` | aplats moy 66, image 77 % | hors sujet (perd la couleur) |
| gamma émulé sur la couverture du texte (γ 0,8 à 1,8) | moy 30 à 46 % au mieux égale | aucun γ ne rapproche : l'écart n'est pas un gamma |

Aucune option ne touche aux causes 2 et 3 (aplats). Le strict exigerait de modifier pdfium : accrocher ou non les rectangles, arrondir au pair, rastériser le texte en LCD comme Skia. C'est un fork, refusé sans ADR et coûteux à tenir.

## Ce qu'une catégorie verrait (contre-épreuves)

Mesure par zone (`spike_s7 epreuves`) : part des pixels de delta > 32 et delta moyen sur TOUTE la zone, `lcd-bgrx` contre pdf.js. On la compare au même rendu décalé d'un pixel, et au même rendu sans le « H » de « Helvetica 12 ».

| Zone | Honnête 50 / 100 / 200 % | Décalé de 1 px (x ; y) | Sans le H |
|---|---|---|---|
| texte | 10,1 / 8,4 / 4,2 %, moy 8,0 / 6,9 / 3,5 | 18–19 / 14–15 / 8–9 %, moy 21 / 19–21 / 11–13 | 10,2 / 8,5 / 4,2 % |
| traits | 1,6 / 0,8 / 0,1 %, moy 1,2 / 0,6 / 0,2 | 8,3 / 4,2 / 2,2–2,4 % | inchangé |
| aplats | 2,0 / 1,5 / 0,8 %, moy 2,2 / 1,4 / 1,0 | 5,6–7,1 / 3,0–4,0 / 1,6–2,0 % | inchangé |
| image | 0 | 17,4 / 8,7 / 4,4 % | inchangé |

- Un décalage d'1 px (page mal placée, échelle fausse) double au moins chaque mesure : il est détecté.
- Un glyphe manquant ne se voit pas dans une mesure de zone (moins de 0,1 point). La garde de forme d'ADR-009 (orphelins de cœur, ±3 / ±1 px), par ligne (script jetable `C:/dev/fragment-rs-s7/capture/epreuve.py`, comme les encres et les sondes de pixels), est aveugle à 50 % (aucun cœur ≥ 0,8 dans un texte si fin), faible à 100 % (7,1 % contre 1,5 % honnête sur la ligne touchée) et juste sous le seuil à 200 % (4,6 % contre 0, pour un seuil de 5 %). L'encre totale (±25 % dans ADR-009) est hors d'atteinte en honnête : −17 à −34 %.

## Reproduire

```text
set PDFIUM_LIB_DIR=C:/dev/fragment-rs-pdfium-bin/chromium-8125/bin
cargo run -p fragment-pdf --example spike_s7 -- fixture
<Fragment>/app/node_modules/electron/dist/electron.exe C:/dev/fragment-rs-s7/capture/main.js
cargo run -p fragment-pdf --example spike_s7 -- rendre
cargo run -p fragment-pdf --example spike_s7 -- document
cargo run -p fragment-pdf --example spike_s7 -- comparer
cargo run -p fragment-pdf --example spike_s7 -- epreuves
```

Le `main.js` et la `page.html` de capture sont hors dépôt, dans `C:/dev/fragment-rs-s7/capture/` : ils lisent le pdf.js de Fragment par chemin, sans le copier.
