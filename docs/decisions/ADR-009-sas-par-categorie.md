# ADR-009 — Sas pixel par catégorie (texte et icônes)

Statut : accepté, avec conditions (orchestrateur, 10/11/2026), sur la proposition du chef du même jour. Le cadre est la règle de l'utilisateur : géométrie et couleurs en strict ; seul l'anticrénelage des glyphes est toléré, et il est suivi comme une dette.

## Contexte

Après #30, les 10 scènes flottantes de T1 ont des boîtes, des bordures et des hauteurs exactes, mais divergent encore de 0,1 à 1 % des pixels :
- glyphes aux traits plus fins côté GPUI (delta jusqu'à 190) ;
- icônes Lucide de même géométrie mais d'anticrénelage différent (delta de 54 à 94) ;
- texte mesuré environ 1 % plus large (+1 à 2 px sur les boîtes ajustées au texte).

Le sas de `program.md` (delta ≤ 16, ≤ 0,3 %, zones < 20 × 20) est mesuré par `xtask pixels` en composantes 8-connexes, une zone par passage de texte, avec le pire delta pris sur toute l'image. Avec cette mesure, aucune scène textuelle ne passe.

## Décision

1. **Hors des boîtes de texte et d'icônes : strict, 0 pixel.**
   - Le candidat (`fragment-pixels`) exporte ses boîtes, avec leur catégorie (texte ou icône) et leur rectangle.
   - L'ORIGINE (x, y) de chaque boîte est vérifiée strictement contre la référence. Une boîte qui commence ailleurs est un échec de géométrie.
2. **Dans une boîte : seul l'anticrénelage est toléré**, jamais un glyphe faux ou absent. Deux garde-fous par boîte, chiffres à fixer par la tâche d'outillage puis reportés ici :
   - l'encre totale (couverture cumulée) reste dans un écart borné ;
   - la boîte d'encre réelle tient à ±1 px de celle de la référence.

   Une lettre manquante ou changée, une autre police ou une autre graisse font échouer le test. La contre-épreuve est obligatoire : une scène dont une lettre est changée doit sortir rouge.
3. **Les icônes Lucide suivent la même règle que les glyphes**, puisque leur géométrie est identique.
4. **Largeur du texte : ±1 px accepté pour T1 SEULEMENT, comme dette, pas comme règle.** L'écart systématique d'environ 1 % a un bead propre, à fermer AVANT T2. Pistes : taille de police arrondie par GPUI ; avances entières contre positionnement sous-pixel ; crénage et fonctionnalités OpenType ; métriques DirectWrite « natural » contre « GDI-compatible ».
5. **Le rasterizeur (gamma, contraste) va en phase 4.** En attendant, chaque run du job `pixels` publie, par scène, le delta max et le pourcentage de pixels divergents DANS les boîtes, dans `docs/parite-pixels.md` et au journal. Ces chiffres font cliquet : une PR qui les aggrave échoue.

## Conséquences

- La tâche d'outillage `fr-t1.2` (xtask, export des boîtes par `fragment-pixels`, garde-fous, contre-épreuve, cliquet) précède la reprise de `fr-t1.1`. La largeur du texte est le bug `fr-t1.3`, à fermer avant T2.
- Le binaire de T1 peut être livré avec cette dette déclarée, que le compte rendu de livraison mentionne.
- Le seuil d'une scène ne se relâche toujours que par ADR.

## Chiffres des garde-fous (fr-t1.2, outillage livré)

Fixés sur les captures candidates de la palette et de la modale (run CI 42873546813, avant #30) contre la référence : 15 boîtes de texte et une d'icône, mesures réelles ci-dessous. Les constantes sont dans `xtask/src/pixels/sas.rs` (garde-fous) et `boites.rs` (détection).

- **Boîte** : candidat seul, jamais la référence. Pixel « de bord » : écart ≥ 14 avec un voisin ; traits droits ≥ 24 px (bordures, séparateurs) et composantes qui les touchent (arcs des coins) écartés comme géométrie ; composantes restantes regroupées à 5 px en x et 2 px en y (une ligne, une icône) ; moins de 8 pixels de bord : poussière, laissée au strict. Icône : 10 à 28 px de côté, presque carrée ; sinon texte.
- **Hors boîte** (élargie de 1 px, l'anticrénelage qui déborde) : 0 pixel de différence.
- **Origine** de la boîte d'encre (couverture ≥ 10 %) : identique à la référence, 0 px d'écart en x et en y.
- **Boîte d'encre** : bord droit et bord bas à ±1 px (la largeur à ±1 est la dette T1 `fr-t1.3`, comptée dans la colonne « Largeur ±1 » du rapport).
- **Encre totale** (distance au fond normalisée par le contraste de la référence) : écart relatif ≤ ±25 %. Mesuré : −15 % à +18 % pour un même texte, −37 % pour les flèches « ↑↓ » et « ↵ » (glyphes de repli différents).
- **Forme**, mot par mot : un pixel de cœur de trait (couverture ≥ 0,8) doit trouver un trait (≥ 0,2) au même endroit de l'autre image, au meilleur décalage de ±3 px en x et ±1 px en y ; part d'orphelins ≤ 5 %. Mesuré : ≤ 2,3 % pour un même texte, 67 % pour les flèches. Limite connue : une lettre changée est vue dans environ deux cas sur trois sur du texte réel de 13 px (les glyphes voisins partagent leurs traits) ; un mot absent, un autre mot, une autre couleur ou une autre graisse le sont toujours. La contre-épreuve (`une_lettre_changee_fait_echouer`) porte sur des glyphes agrandis.
- **Teinte** : cosinus entre les couleurs d'encre ≥ 0,995 (mesuré ≥ 0,999).
- **Cliquet** : `pixels/cliquet.json`, par PNG : delta max et % de pixels divergents dans les boîtes (arrondi à 4 décimales). Une mesure au-dessus de la base échoue ; une scène qui diverge dans ses boîtes sans base échoue aussi. `cargo xtask pixels --maj-cliquet` réécrit la base (la PR montre la différence).

### Limite de l'export des boîtes

GPUI ne laisse pas lire les sprites de glyphes et d'icônes d'une scène peinte (`Window::rendered_frame` est privé, `painted_quads` ne rend que les quads) : `fragment-pixels` déduit donc les boîtes de l'IMAGE candidate (`<scène>-<thème>.boites.json` à côté du PNG), par le même code que `xtask`. L'export exact (chaque `div` de texte et chaque icône déclarent leur rectangle) demanderait un crochet dans `fragment-ui` ; le format de fichier est prêt à le recevoir.

### Ce que le sas montre sur les captures avant #30

Aucune scène de palette ni de modale ne passe, et ce n'est pas le texte : plus de 99 % des pixels divergents HORS boîtes ont un delta ≤ 2 (ombre `--shadow-l`, mélange alpha), le reste est l'arrondi des coins de la boîte (delta jusqu'à 200), un libellé de pied absent côté candidat et la ligne sélectionnée. Dans les boîtes : décalages d'origine de 1 à 3 px sur les libellés du pied (dette de largeur en cascade) et les flèches de repli. L'ombre est le point à trancher : le sas strict hors boîte la fait échouer.

## Amendement du 11/11/2026 (orchestrateur) : ombre et boîtes

**Ombre : catégorie accordée, EN DETTE, pas en règle permanente.**
- Tolérance : delta ≤ 2, dans le seul HALO GÉOMÉTRIQUE de l'ombre. Le halo, c'est la boîte de l'élément (déjà stricte), plus le décalage, plus 3 σ du flou. Il est calculé depuis la mise en page (déclaré par le candidat), jamais déduit de l'image.
- Échouent : tout pixel divergent hors du halo, et tout delta > 2 dans le halo.
- Contre-épreuve : un rayon d'ombre ou un décalage changé d'1 px fait échouer.
- Cliquet : le delta max et le % de pixels de l'ombre font cliquet, comme le texte.
- Le mélange alpha et le flou (écart-type de GPUI contre flou/2 de Skia) rejoignent la tâche rasterizeur de la phase 4.
- Mise en œuvre : dans `fr-t1.1`, qui possède les surfaces flottantes et leurs ombres.

**Boîtes déduites de l'image : confirmées pour T1.** 2 lettres changées détectées sur 3, c'est insuffisant pour l'éditeur. L'export exact des boîtes par `fragment-ui` est le bead `fr-p3.1`, à mener le plus tôt possible dans T3 ; il bloque la FIN de T3.

### Chiffres de la catégorie ombre (fr-t1.1, outillage livré)

Constantes dans `xtask/src/pixels/ombres.rs`, mesures sur le run CI 42997199734.
- **Déclaration** : `fragment_ui::flottant::ombrer` pose l'ombre ET l'enregistre (boîte de bordure lue au prépeint, rayon, couches en pixels physiques) ; `capture.rs` vide le registre avant la peinture capturée et l'écrit dans `.boites.json` (version 2, clé `ombres`). Sans déclaration, aucune tolérance d'ombre.
- **Halo** : par couche, la boîte décalée, gonflée de l'étendue puis de 3 σ (σ = `blur_radius` de GPUI = flou CSS / 2) ; union des couches et de la boîte, moins l'intérieur (pixels entièrement couverts par la boîte arrondie, strict). Les boîtes de texte gardent leurs propres garde-fous.
- **Constat** : le delta seul ne suffit pas à la contre-épreuve. Une ombre décalée d'1 px ou au flou changé d'1 px ne bouge aucun pixel de plus de 2 niveaux (pente de 0,4 niveau par pixel pour `--menu-shadow` sombre). D'où trois garde-fous sur les côtés droits de la surface, mesurés sur les mêmes pixels des deux images :
  - **étalement** : moyenne des écarts de distance de l'ombre au bord, pondérée par l'encre, ≤ 0,08 px ;
  - **glissement** : écart d'encre relatif entre côtés opposés ≤ 10 % ;
  - **encre** moyenne à ±20 % près.
- **Calage** : ce qui passe en vrai a un étalement ≤ 0,07 px (menu sombre +0,02, barres d'outils +0,06, notice sombre +0,02). Sur la référence du menu sombre (`contre_epreuve` dans `ombres.rs`), un décalage de ±1 px donne un glissement de ±20 % ou plus, et un flou CSS de ±1 px un étalement de −0,10 à +0,15 px : tout cela échoue.
- **Cliquet** : `ombre_delta_max` et `ombre_pourcent` (part de pixels tolérés dans l'aire des halos) par PNG, dans `pixels/cliquet.json`.
- **Ce qui ne passe pas** (détail dans `docs/parite-pixels.md`) : `--shadow-l` (étalement +1,4 à +1,7 px en sombre, −0,6 px en clair, quelques pixels de delta 3 à 4), le menu clair (−0,19 px), l'infobulle (+0,23 px). Le signe change avec le thème, ce qui désigne le mélange (gamma) et non la géométrie : c'est pour la phase 4. Les coins arrondis (delta 1 à 6 sur 2 à 12 pixels par surface) restent stricts et font échouer seuls trois PNG de barre d'outils.

## Amendement du 11/11/2026, 11:15 UTC (orchestrateur) : arrondis, --shadow-l, aplats translucides

**Garde-fous d'ombre de fr-t1.1 (#35) : APPROUVÉS.** Étalement ≤ 0,08 px, glissement entre côtés opposés ≤ 10 %, encre ±20 %. Ils resserrent la règle de l'amendement précédent.

**a. Bords d'arrondi : catégorie accordée, en dette de phase 4.** L'anticrénelage d'une courbe est le même phénomène que pour les glyphes ; ce n'est pas de la géométrie.
- Ne couvre que les pixels à ≤ 1 px de l'arc THÉORIQUE, calculé depuis la mise en page (boîte et rayon) déclarée par le candidat, jamais déduit de l'image.
- Delta ≤ 6, en cliquet.
- Contre-épreuve : un rayon changé d'1 px ou une boîte décalée d'1 px font échouer.
- Le reste du bord et l'intérieur restent stricts.

**b. `--shadow-l` : exception NOMMÉE, pas tolérance générale.** L'étalement mesuré (+1,5 px en sombre, −0,6 px en clair, signe inversé selon le thème) vient du mélange alpha (espace linéaire contre sRGB).
- Les valeurs mesurées font cliquet, par thème.
- Les garde-fous du point précédent restent valables pour toutes les autres ombres.
- Tâche de phase 4 « mélange alpha GPUI contre Chromium », bloquante pour la fin de la phase 4.

**c. Aplats translucides : aucune tolérance (une couleur est stricte).**
- Correction à la source : pour un aplat dont le fond est connu, `fragment-theme` pré-compose le jeton translucide en couleur opaque, par le calcul de Chromium (sRGB, 8 bits).
- Vaut une fois pour tous les jetons translucides posés sur un fond connu : survol, sélection, modifier.

Dettes que l'orchestrateur listera à l'utilisateur à la livraison : texte, icônes, ombre, `--shadow-l`, bords d'arrondi.

**Prévu pour T4 (en attente du chiffrage de S7)** : une catégorie « raster de page PDF » limitée à la surface de la page rendue. Le chrome du PDF reste strict : barre, vignettes, marges, position des pages, surlignages, sélection.

### Chiffres des bords d'arrondi et de l'exception `--shadow-l` (fr-t1.1, outillage livré)

Constantes dans `xtask/src/pixels/arrondis.rs` et `ombres.rs`, mesures sur le run CI 43011857108.
- **Déclaration** : `fragment_ui::flottant::arrondir` pose l'arrondi ET l'enregistre (boîte de bordure lue au prépeint, rayon, bordure) ; `ombrer` l'appelle et nomme son jeton (`.boites.json` version 3, clés `arrondis` et `jeton`). Déclarés : surfaces ombrées, puce `kbd`, ligne sélectionnée de la palette, bouton actif de la barre d'outils. Sans déclaration, les coins restent stricts.
- **Arcs** : quatre extérieurs (rayon `r`) et, avec une bordure, quatre intérieurs (`r − bordure`). **Bande** : pixels dont le centre est à ≤ 1 px de l'arc, dans le quart de disque du coin. Delta ≤ 6 (cliquet `arrondi_delta_max`, `arrondi_pourcent`).
- **Constat** : le delta seul ne fait pas la contre-épreuve (un rayon changé d'1 px ne déplace l'arc que de 0,41 px au plus : sous 6 niveaux sur une bordure de 12 niveaux de contraste). D'où un garde-fou de **position** : par arc, Σ (candidat − référence) projeté sur la couleur « dedans − dehors » (lue au milieu du côté droit voisin), divisé par la longueur de l'arc = déplacement radial moyen, **≤ 0,15 px**. Un rayon ±1 donne 0,27 px quel que soit `r` (contre-épreuve `un_rayon_change_d_un_pixel_fait_echouer`, déclaré au rayon dessiné ou non, dont une bordure peu contrastée) ; une boîte décalée d'1 px échoue par ses côtés stricts.
- **Mesuré** : sombre et barres d'outils ≤ 0,10 px (barres ≤ 0,024) ; clair 0,20 à 0,25 px (menu, notices, infobulle : contraste de bordure de 7 niveaux, un niveau = 0,14 de couverture ; palette et modale : arc de GPUI 0,2 px plus à l'extérieur, 60 px de delta 7 à 33 sur une bordure de 92 niveaux). Ces coins clairs échouent.
- **`--shadow-l`** (`ombres::JETON_EXCEPTION`) : garde-fous et delta > 2 non bloquants, valeurs en cliquet par PNG (clé `shadow_l`, valeur absolue). Mesuré : sombre étalement +1,42 à +1,60 px, encre +24 à +34 %, glissement −28 à −43 % ; clair −0,58 à −0,60 px, encre −3 à −7 %, glissement +2 à +7 % ; 0 pixel de delta > 2 hors des bandes d'arrondi (les 26 à 76 pixels de delta 3-4 étaient les coins).

## Amendement du 11/11/2026, 12:00 UTC (orchestrateur) : origine d'encre, garde d'arc, résidus de T1

**Garde d'arc (fr-t1.1, #46) : APPROUVÉE.** Le glissement moyen de chaque arc est limité à 0,15 px.

**Origine d'encre, au barycentre.** Elle remplace le bord de couverture à 10 %, que le calage des fûts de GPUI décalait d'1 px sur un seul côté.
- L'origine se mesure au BARYCENTRE de l'encre, à ±0,5 px, en x ET en y.
- La boîte d'encre à ±1 px reste exigée en plus.
- Barycentre par passage et, si la boîte est assez longue, par moitié gauche et moitié droite : un décalage d'un côté ne peut pas être compensé de l'autre.
- Contre-épreuve : un passage déplacé d'1 px fait échouer.

**Résidus sous le pixel : extension TEMPORAIRE, limitée à la livraison de T1.**
- Liste NOMMÉE des scènes et des éléments, avec leurs valeurs mesurées en cliquet : ombres de l'infobulle, du menu clair et de la notice claire ; arcs clairs ; palette et modale claires.
- Aucune tolérance générique par thème.
- Ces exceptions et celle de `--shadow-l` sont cinq symptômes d'une seule cause, le mélange alpha. `fr-p4.1` est lancée MAINTENANT, sans attendre la phase 4. Son critère de fin : toutes ces exceptions retirées, cliquets ramenés à 0.
