# Placement des glyphes : GPUI contre Chromium (fr-p4.1.1, 13/11/2026)

Question de l'orchestrateur : l'écart sur « fermer » (fr-p3.1, jusqu'à −0,60 px sur le « m ») et la
largeur d'encre de −1 à −2 px sur T2 (fr-t2-montage) viennent-ils d'un placement SYSTÉMATIQUE des glyphes
par GPUI, avec une origine calée sur le pixel entier par GPUI et au sous-pixel par Chromium ?

**Réponse : NON. La cause est ailleurs.** GPUI place et rastérise les glyphes au quart de pixel sous
DirectWrite, comme Chromium, et ses avances sont celles de Chromium. L'écart vient de l'**origine des
boîtes de texte**. La mise en page de GPUI (`gpui-pre-0.3.12/src/taffy.rs`) cale chaque boîte au pixel
physique, et elle arrondit au pixel supérieur la taille mesurée d'une feuille de texte. Chromium garde
l'origine fractionnaire (LayoutUnit, 1/64 px). Il n'y a donc pas de correctif dans le périmètre
pré-accordé (« positionnement sous-pixel horizontal des glyphes sous DirectWrite ») : ce
positionnement-là est déjà juste. Aucun patch n'a été écrit (consigne : remonter avant de toucher).

## Méthode

- **Référence** : relevé `Range.getClientRects` caractère par caractère, avec la police calculée
  (famille, taille, graisse, interlettrage). Il a été fait sur Fragment, pendant une passe des scènes
  sous verrou de capture, sur 50 scènes T1, T2 et T11 : 98 relevés. Les PNG de cette passe ont le
  **même MD5** que les 98 de `pixels/reference/` : le relevé décrit exactement les références. La
  garde de Fragment (« Fragment inchangé ») est passée.
- **Candidat** : les PNG de l'artefact `pixels` du run CI 43356976195 (PR #102, T1 et T2 actives).
- **Mesure** : `cargo xtask mesure-glyphes` (`xtask/src/mesure_glyphes.rs`). On prend la même fenêtre
  dans les deux images et on compare le barycentre de l'encre ; `dx` et `dy` valent candidat − référence.
  Deux fenêtres sont utilisées. (a) La boîte d'avance du caractère. (b) Le mot entier, élargi de 2 px.
  ★ Il faut (b) : la fenêtre d'un caractère seul coupe l'encre déplacée. Le « m » de « fermer » mesure
  −0,21 px dans sa boîte, et −0,73 px sur son profil complet.
- **Sonde DirectWrite** (hors dépôt, `C:/dev/fragment-rs-mesure-glyphes/sonde-dwrite`) : elle rejoue
  les appels de rastérisation de `gpui-pre-windows-0.3.12/src/direct_write.rs` sur `DMSans-400.ttf`
  12 px. Ces appels sont `GetRecommendedRenderingMode`, puis `CreateGlyphRunAnalysis` à l'origine
  `q/4`, puis une texture `ALIASED_1x1` en niveaux de gris.

Reproduire (une passe sous verrou ; voir « Outil de relevé » plus bas) :

```
powershell -File C:/dev/fragment-rs-mesure-glyphes/preparer.ps1          # copie des scènes + accroche du relevé
powershell -File C:/dev/fragment-rs-mesure-glyphes/relever.ps1 -Scene <t1-…,t2-…,t11-…>   # SOUS VERROU
gh run download 43356976195 -n pixels -D C:/dev/fragment-rs-mesure-glyphes/art-montage
cargo run --release -p xtask -- mesure-glyphes --releves C:/dev/fragment-rs-mesure-glyphes/pixels/reference \
  --candidate C:/dev/fragment-rs-mesure-glyphes/art-montage/pixels/candidate --prefixe t1- --prefixe t2- \
  --report <rapport.md> --csv <mesures.tsv>
```

## Résultats

### 1. Avances et crénage : identiques

La pente de `dx` le long d'un mot vaut **+0,006 px par caractère** (998 mots de 3 lettres ou plus,
5 546 caractères), et elle est comprise entre −0,019 et +0,018 selon la police. Sur dix lettres, cela
fait 0,06 px. Les polices sont les mêmes octets : `hmtx`, `glyf` et `GPOS` des TTF de `fragment-ui`
sont identiques aux woff2 de Fragment (MD5 des tables, graisses 400, 500 et 600).

### 2. Rastérisation sous-pixel : juste dans GPUI

Sonde, DM Sans 400 à 12 px, mode recommandé `NATURAL_SYMMETRIC` (5), grille `ENABLED` (2) :

| glyphe | barycentre q = 0 | q = 1/4 | q = 1/2 | q = 3/4 |
|---|---|---|---|---|
| `m` | 5,273 | 5,537 | 5,781 | 6,017 |
| `e` | 3,335 | 3,588 | 3,839 | 4,082 |
| `r` | 1,755 | 2,033 | 2,267 | 2,491 |

Le glyphe avance d'environ 0,25 px par quart : DirectWrite respecte l'origine sous-pixel que
`Window::paint_glyph` lui passe (`SUBPIXEL_VARIANTS_X = 4`, arrondi au quart le plus proche, comme Skia).
Désactiver la grille n'y change rien horizontalement. Le profil de colonnes d'un `m` en q = 1/2 est
`[1146, 606, …]`, avec ses fûts étalés sur deux colonnes. Il n'y a donc **pas** de calage au pixel
entier dans le rastériseur.

### 3. Pas de dent de scie selon la phase du stylo

Si GPUI calait le stylo au pixel entier, `dx` suivrait `round(x) − x`, soit ±0,44 px vers la phase
1/2. Mesuré par caractère : corrélation de `dx` avec `round(x) − x` = **0,18**, et avec la quantification
au quart = −0,05. Par huitième de phase, la moyenne de `dx` reste entre −0,12 et +0,06 px (5 546
caractères). Les écarts sont déterministes : la corrélation entre le sombre et le clair d'une même
scène vaut 0,91 (2 750 paires).

### 4. Ce qui bouge : le mot entier, avec l'origine de sa boîte

Au mot, `dx` a une moyenne de −0,049 et un écart-type de **0,325** px, et 35 % des mots dépassent
0,25 px (1 130 mots, T1 : 830, T2 : 300). L'écart est porté **par la boîte de texte** : un même texte
garde le même écart d'une scène à l'autre (« 0.0.0 » : −0,24, é.-t. 0,11 ; « Tab » : −0,32). Il est
plus fort quand Chromium place le début de ligne au sous-pixel : 48 % des mots au-dessus de 0,25 px,
contre 31 % pour une origine entière, avec une moyenne de −0,23 px en phase [1/8 ; 1/4[.

« fermer » (palette, 12 px 400) : stylo de Chromium en x = 503,406 ; mot entier **−0,56 px** en sombre
et −0,51 px en clair. C'est l'écart attendu si GPUI pose la boîte en 503,0 : le stylo est quantifié
en 503,0 au lieu de 503,5, soit −0,5 px, et chaque lettre suit avec les mêmes avances. Profil du `m` :
référence `654 496 | 673 486 | 595 453` (fûts entre deux colonnes, phase ≈ 1/2), candidat
`342 723 161 | … 787 … | … 765` (fûts sur une colonne, phase ≈ 0). La lettre n'est pas mal placée
dans son mot : c'est le mot qui part d'un pixel entier.

Mécanisme dans `gpui-pre-0.3.12/src/taffy.rs` (commentaire « We pixel-snap by rounding in
device-pixel space ») : (1) avant la mise en page, les longueurs sont arrondies, et une feuille mesurée
(un texte) voit sa taille **arrondie au pixel supérieur** ; (2) après la mise en page, chaque bord est
arrondi au pixel en coordonnées absolues. Un texte centré, aligné à droite, ou placé après une boîte
de largeur fractionnaire, est ainsi décalé d'une fraction de pixel. Cette fraction est quantifiée au
quart, et le texte entier la subit. Chromium, lui, cale fonds et bordures au pixel mais garde l'origine
du texte fractionnaire.

### 5. Deux cas qui ne relèvent pas de GPUI

- **Interlettrage** (titres de groupe des réglages, 12 px, 0,48 px) : `dx` alterne de −0,45 à +0,95 px
  d'une lettre à l'autre (« Options » : 0, +0,54, −0,44, −0,45, +0,13, +0,85, −0,37). GPUI n'a pas de
  `letter-spacing` : `fragment-ui/src/reglages/fenetre.rs` pose une boîte par lettre, à l'abscisse
  arrondie, et taffy la cale au pixel. C'est un calage par lettre dans `fragment-ui`.
- **Grands écarts d'un pixel ou plus** (« Afficher », « système » : −1,1 à −1,25 px dans les menus de
  T2 ; « New » : +3,8 sur t1-boutons-fenetre) : ce sont des écarts de mise en page de l'entrée elle-même
  (déjà relevés par fr-t2-montage, par exemple l'entrée à −0,85 px), pas de glyphes.

### Par police (mot entier, fenêtre élargie)

| police (DM Sans) | mots | moy dx | é.-t. dx | abs dx > 0,25 | moy dy |
|---|---|---|---|---|---|
| 11 px 400 | 108 | −0,214 | 0,186 | 69 % | +0,005 |
| 12 px 400 | 638 | −0,022 | 0,348 | 34 % | −0,047 |
| 12 px 400, interlettrage 0,48 px | 12 | +0,199 | 0,257 | 33 % | −0,119 |
| 12 px 500 | 14 | −0,007 | 0,076 | 0 % | +0,011 |
| 12 px 600 | 18 | +0,010 | 0,244 | 33 % | −0,056 |
| 13 px 400 | 188 | −0,065 | 0,388 | 39 % | −0,026 |
| 14 px 400 | 122 | −0,067 | 0,141 | 16 % | −0,008 |
| 14 px 600 | 8 | +0,139 | 0,005 | 0 % | −0,037 |
| 16 px 600 | 8 | −0,298 | 0,204 | 75 % | −0,146 |
| 18 px 600 | 8 | +0,092 | 0,067 | 0 % | −0,000 |

Il n'y a pas de dépendance à la taille ni à la graisse qui signerait le rastériseur. L'écart suit les
boîtes. Le 11 px (−0,21), par exemple, est pour moitié la version « 0.0.0 » (54 mots sur 108, −0,24
px à chaque occurrence).

## Recommandation (pour l'orchestrateur, qui décide)

1. **Pas de fork pour le placement des glyphes** : le positionnement sous-pixel de DirectWrite est
   déjà juste dans GPUI. Patcher `direct_write.rs` ou `paint_glyph` n'enlèverait rien.
2. Le levier, c'est l'origine des boîtes de texte. Il y a deux voies.
   - **(a) Fork de `gpui-pre` (`taffy.rs`)** : ne plus arrondir au pixel supérieur la taille mesurée d'un
     texte, et peindre le texte à l'origine non arrondie de sa boîte, tout en gardant fonds et bordures
     calés au pixel (ce que fait Chromium). Ce serait le fork n°2 sur 2 (`program.md` §10) et il toucherait
     toute la mise en page. Les géométries strictes, aujourd'hui vertes, sont à re-vérifier.
   - **(b) Côté `fragment-ui`** : là où Fragment place un texte au sous-pixel (centrage, alignement à
     droite, texte après une boîte de largeur fractionnaire), peindre le texte par un élément qui garde
     l'abscisse fractionnaire. Pour l'interlettrage, un seul élément qui pose ses glyphes à `x + i × ls`
     non arrondi. C'est sans fork, mais au cas par cas.
3. En attendant : la dette nommée « placement des glyphes (fr-p4.1.1) » et l'exception « fermer »
   restent ce qu'elles sont. Le critère de fin de fr-p4.1.1 (dettes à 0 sur T1 et T2) n'est pas
   atteignable sans (a) ou (b).

## Outil de relevé (hors dépôt, `C:/dev/fragment-rs-mesure-glyphes`)

- `preparer.ps1` copie `pixels/` (sans `reference/`) vers `C:/dev/fragment-rs-mesure-glyphes/pixels`. Il
  réécrit les imports relatifs vers Fragment en chemins absolus, et, après chaque `screenshot` ou
  `capturePage`, il ajoute un appel à `releverGlyphes(page, <scène>-<thème>.glyphes.json)`. Il produit
  `relever.ps1`, qui est `scripts/pixels-reference.ps1` avec sa racine déplacée : même serveur Vite et
  même garde de Fragment.
- `_releve.ts` (code propre à fragment-rs) parcourt les nœuds texte visibles avec un `TreeWalker`. Pour
  chaque caractère non blanc, il relève `Range.getClientRects` en pixels de l'image (×
  `devicePixelRatio` pour `capturePage`) et l'indice de police, et il incrémente un compteur de mot à
  chaque blanc et à chaque nœud.
- `sonde-dwrite` est le binaire Rust de la section 2 (crate `windows` 0.62.2, déjà dans `Cargo.lock`).
