# ADR-014 — Raster de page PDF : catégorie du sas limitée à la page rendue

Statut : **proposé** (fr-t4-raster, 12/11/2026). Divergence avec Fragment : à trancher par l'orchestrateur (program.md §8). Mesures : `docs/spikes/S7-raster-pdf.md`.

## Contexte

Fragment peint une page PDF avec pdf.js dans un canvas, fragment-rs avec pdfium. S7 compare les deux rendus d'une même fixture (texte embarqué ou non, traits, aplats, image) à 50, 100 et 200 %, à la même taille physique. Seules les images sont identiques. Le reste diverge pour quatre causes, et aucune option de pdfium ne les supprime :
- le texte, rastérisé en LCD par Chromium, en gris par pdfium (8 à 22 % des pixels de la zone, delta max jusqu'à 248) ;
- les rectangles pleins, accrochés à la grille par pdfium (delta jusqu'à 143 sur les bords fractionnaires) ;
- l'arrondi des couleurs à ,5, au pair chez pdf.js et vers le haut chez pdfium (un niveau sur l'intérieur des aplats) ;
- l'anticrénelage des courbes et des diagonales (delta jusqu'à 72).

Le strict demanderait un fork de pdfium. ADR-009 ne couvre pas ce cas : ses boîtes de texte sont détectées dans l'image, et sa garde d'encre (±25 %) est dépassée par le rendu honnête (−17 à −34 %).

## Décision proposée

1. **Le chrome reste STRICT** : barre d'outils, vignettes, marges, fond de la vue, position et taille de chaque page, surlignages de recherche et de sélection, liens au survol. Le bord de la page en fait partie.
2. **Catégorie « raster de page PDF », en DETTE**, dans le seul rectangle de chaque page rendue.
   - Le rectangle est déclaré par le candidat depuis la mise en page, via `pdf_geometry`, comme le halo des ombres (ADR-009, amendement du 11/11). Il n'est jamais déduit de l'image.
   - Les surlignages et la sélection posés sur la page restent stricts. Ils sont comparés sur une scène de contrôle qui rend la page en blanc des deux côtés, ou bien sous le seul masque de leurs propres boîtes.
3. **Garde-fous, par page et par échelle**, avec les seuils calés sur S7 :
   - part des pixels de delta > 32 : ≤ 12 % à 50 %, ≤ 10 % à 100 %, ≤ 6 % à 200 % ;
   - delta moyen sur la page : ≤ 10 / 8 / 5 ;
   - sur la fixture de S7, le rendu honnête est mesuré à 10,1 / 8,4 / 4,2 % et 8,0 / 6,9 / 3,5 dans la zone de texte, la pire.
4. **Contre-épreuve obligatoire** : la même scène, page décalée de 1 px en x puis en y, doit échouer. S7 mesure, dans la zone de texte, 18 à 19 % / 14 à 15 % / 8 à 9 % et un delta moyen de 21 / 19 à 21 / 11 à 13 : c'est au-dessus de chaque seuil.
5. **Cliquet** : la part des pixels de delta > 32 et le delta moyen, par page de scène, comme pour le texte et l'ombre.
6. **Test strict de non-régression du moteur**, à côté du sas : un test de `fragment-pdf` compare `Document::rendre_page` à des rasters dorés pdfium de la fixture S7. Un glyphe manquant, une couleur fausse ou une région décalée y échouent au pixel près, ce que la catégorie ne voit pas.

## Limite connue

La catégorie détecte un décalage, une échelle fausse ou une page manquante. Elle ne voit pas un glyphe isolé faux ou absent : moins de 0,1 point dans les mesures de zone, et la garde de forme d'ADR-009 est aveugle à 50 % et sous son seuil à 200 %. Le contenu du texte est donc vérifié ailleurs : par la couche texte (`Document::caracteres`, tests de `pdf_texte`) et par le test doré du point 6.

## Option à trancher : le texte LCD

Rendre en `BGRx` avec `FPDF_LCD_TEXT` (le jeu `lcd-bgrx`) baisse le delta moyen du texte de 25 à 37 %, mais touche un peu plus de pixels.
- Pour : plus proche de Fragment, qui est lui-même en LCD.
- Contre : la référence dépend du réglage ClearType de la machine qui la capture, et les franges deviennent visibles si l'image est étirée pendant un zoom.

Ce changement de `Document::rendre_region` n'est pas fait ici. Le chef ou l'orchestrateur décide ; les seuils du point 3 sont calés sur `lcd-bgrx` ; avec `defaut`, le texte les dépasse à 50 % (13,2 % de delta > 32) et à 100 % (10,9 %), et il faudrait les relever à 14 / 12 / 6 %.

## Conséquences

- `fr-t4-livraison` met en œuvre la catégorie : déclaration des rectangles de page par `fragment-pixels`, garde-fous dans `xtask/src/pixels/`, contre-épreuve et cliquet. Elle met aussi en œuvre le test doré du point 6.
- Les scènes PDF à 3 zooms passent ainsi sans relâcher le reste de la scène.
- Le fork de pdfium reste écarté. S'il devait être rouvert pour atteindre le strict, il lui faudrait un ADR propre.
