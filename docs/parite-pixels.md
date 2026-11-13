# Parité pixels : produire les références sans écrire dans Fragment

Les références (`pixels/reference/*.png`) sont des captures de Fragment pilotée par son harnais e2e.
Fragment est en lecture seule : on exécute ses outils, on n'y écrit rien (décision du 06/11).

## Commande
`powershell -File scripts/pixels-reference.ps1 [scène]` — sans argument, les 11 scènes dans les deux thèmes (22 PNG).

## Ce que fait le script
1. **Serveur.** Le port 5123 est codé en dur dans Fragment (harnais e2e et processus principal) : il ne se
   surcharge pas. S'il répond déjà, c'est le serveur de l'utilisateur : on le réutilise et on ne l'arrête jamais.
   Sinon on lance Vite (binaire de `Fragment/app/node_modules`) avec la config enveloppe
   `pixels/vite.reference.config.ts`, qui reprend celle de Fragment et ne change que `root` et `cacheDir`
   (`C:/dev/fragment-rs-vite-cache`). Le serveur lancé par le script est arrêté en fin de passe (arbre de
   processus), et le script vérifie que le port est libéré.
2. **Pas de `.vite-temp`.** La config est chargée avec `--configLoader runner` (en mémoire) ; le chargeur par
   défaut écrirait un fichier temporaire dans `node_modules` de Fragment.
3. **Playwright** (celui de Fragment) joue `pixels/scenes/*.spec.ts` ; `userData` d'Electron et coffre vont dans
   `%TEMP%` (harnais) ; `FRAGMENT_NO_ONBOARDING=1`.
4. **Garde.** `git status --porcelain --ignored` de Fragment, plus l'empreinte (chemin, taille, date) de tout
   `app/` hors `node_modules` et de `node_modules/.vite`, `.vite-temp`, `.cache`, relevés avant et après ; un écart
   fait échouer le script. Une modification concurrente de Fragment par l'utilisateur pendant la passe
   déclenche donc aussi l'échec : relancer.
5. **Provenance.** Une passe complète réussie écrit `pixels/reference/PROVENANCE.txt` (commit, branche, état de
   l'arbre, mode du serveur).

## Si un PNG change
Comparer avec `git status` / `git diff --stat pixels/reference`. Un écart est à expliquer avant de commiter
(fixture ou décor réécrits, mise à jour de Fragment : le commit de PROVENANCE.txt le montre).

## Scènes instables (référence non déterministe)
★ Décision de l'orchestrateur (13/11) : une RÉFÉRENCE non déterministe est un défaut de la mire, pas un bruit
acceptable, car elle peut masquer un vrai écart. Une scène listée ici ne compte PAS comme verte, même à 0 pixel,
tant que son bead n'a pas trouvé la source (caret, transition, horloge, ordre de rendu…) et figé la scène :
critère de sortie, **1 seul MD5 sur 5 rejeux** en mode invisible.

| Scène | Symptôme | Bead |
|---|---|---|
| t11-accent | deux MD5 qui alternent, pixel (507,7) R 0x30/0x2F, dans les deux thèmes | fr-hwq |
| t7-chat-cowork-replie | deux passes, deux PNG différents | fr-914 |

## Dettes de parité : surfaces flottantes T1 (fr-t1.1)
Scènes `t1-{notice,notice-empilees,menu,infobulle,toolbar-sections}` en `"depuis": "T2"` ; `t1-toolbar-horizontale` à T1.
Mesures CI du 10/11, de 0,12 % à 1,07 % de pixels différents (contre 0,5 à 4,3 % avant) ; la mise en page est
désormais au pixel (arêtes du menu, séparateurs, bordures, hauteur de carte identiques).

**État au 11/11, catégorie ombre en place (run CI 42997199734, après #33)** : sur les 20 PNG des 10 scènes flottantes
et de palette, 1 passe (`t1-toolbar-horizontale-dark`, toléré, cliquet posé) ; les scènes restent en `"T2"` pour
garder le job vert. Ce qui échoue encore, par cause :
- **Coins arrondis** (toutes les surfaces) : 2 à 12 pixels d'anticrénelage de coin par surface, delta 1 à 6 (SDF de GPUI
  contre couverture analytique de Skia). Seuls responsables des échecs de `t1-toolbar-horizontale-light` et
  `t1-toolbar-sections-*` (12 px de delta 1). Rasterizeur, phase 4.
- **Ombres** (garde-fous, voir ADR-009) : passent le menu sombre, les barres d'outils et la notice sombre (étalement
  ≤ 0,07 px). Échouent : `--shadow-l` (modale, palette) avec un étalement de +1,4 à +1,7 px en sombre et −0,6 px en clair,
  encre +24 à +34 % en sombre, plus 26 à 76 pixels de delta 3 à 4 dans le halo ; le menu clair (−0,19 px), l'infobulle
  (+0,22 à +0,24 px), la notice claire (−0,16 px). Le signe qui change avec le thème désigne le mélange alpha (gamma), pas
  la géométrie. Rasterizeur, phase 4.
- **Notices empilées** : la carte à deux lignes (2 × 16,8 px) est décalée d'1 px en hauteur (résidu de `fr-t1.3`), ce
  qui déplace aussi l'ombre de la carte suivante (glissement −34 à +80 %, 710 et 1151 px hors tolérance).
- **Texte** : origines d'encre décalées d'1 px en x sur des traits plus fins (encre −10 à −14 %, sous le seuil de 10 % de
  couverture, sans décalage visible) ; flèches de repli « ↑↓ », « ↵ » (forme 67 à 80 %) ; libellé de barre d'outils :
  la RÉFÉRENCE a un anticrénelage sous-pixel (franges colorées), d'où une origine à +1 px.
- **Palette** : fond de ligne sélectionnée et de puce `kbd` à 1 niveau (composition, malgré `Couleur::sur`) sur
  ~17 000 px ; 2 pixels isolés du pied de légende sans équivalent (delta 63 et 112).

Corrigé ici : la puce `kbd` de la palette, 20 px au lieu de 21 (interligne de 16,5 arrondi par GPUI), calée par
`echelle::calees` ; le rectangle d'enregistrement des niveaux de menu (position statique d'un enfant absolu, 4 px de trop).

**État au 11/11, après la reprise (#46, run CI 43011857108)** : catégorie bord d'arrondi et exception `--shadow-l` en place
(chiffres dans ADR-009). 3 PNG sur 20 passent : `t1-toolbar-horizontale-{dark,light}` (scène repassée à T1) et
`t1-notice-dark` (cliquet posé, scène en T2 faute du clair). Corrigé : notices empilées calées comme Chromium (carte à deux
lignes de 55 px, 3e carte à 126 : 0 pixel hors tolérance en sombre), `↵` dessiné avec Cambria Math (repli de Chromium ;
forme 0 % au lieu de 67 à 80 %), légende de la palette calée (écart ramené de +2/+3 px à −1/−2 px), aplats de fr-t1.4
(ligne sélectionnée : 17 000 px hors tolérance à 0). Reste, par cause :
- **Origine d'encre à 1 px (14 PNG)** : seuil de 10 % de couverture. GPUI cale le fût sur la grille (1,0 px net, couverture
  1,0) quand Chromium l'étale (0,27 / 0,87 / 0,27, même centre) : la colonne de bord passe sous 10 % d'un seul côté. Ce n'est
  pas un décalage ; c'est le garde-fou qui le lit ainsi. À trancher par l'orchestrateur (seuil, ou origine au barycentre).
- **Ombres hors exception** : infobulle +0,22 à +0,24 px (petite boîte, 4 échantillons du shader), menu clair −0,19, notice
  claire −0,16 (étalement, borne 0,08). Rasterizeur, phase 4.
- **Coins clairs** : déplacement d'arc 0,20 à 0,25 px (borne 0,15) ; palette et modale claires : 60 px de delta > 6.
- **Légende de la palette** : `↵` en gras synthétique chez Chromium (1 px d'avance et d'épaisseur en plus), absent de GPUI ;
  `exécuter` à −2 px, `esc`/`fermer` à −1 px.
- **Titre de la modale** et champ de la palette : origine à +1 px en y (interligne).

**État au 11/11, étape 3 (PR #50, run CI 43024988037)** : les 10 scènes sont à T1, 20 PNG sur 20 tolérés, cliquet posé.
Origine d'encre au barycentre, exceptions nommées temporaires de T1 (`xtask/src/pixels/exceptions.rs`, 13 éléments, à
retirer par `fr-p4.1`), `↵` en gras synthétique, titre de modale remonté d'1 px. Deux causes trouvées en route :
- **Bordures translucides** : pré-composées en opaque, elles éloignaient l'anticrénelage des arcs de 2 à 4 niveaux ;
  laissées translucides, GPUI les mélange à l'octet d'opacité (26/255) quand Chromium prend l'opacité exacte (10 %),
  d'où 232 au lieu de 233 sur les côtés droits clairs. `flottant::bordure_translucide` déplace la teinte, canal par
  canal, pour que le mélange de GPUI tombe sur celui de Chromium.
- **Zoom qui fuit** : depuis #48, le zoom d'une scène d'app (110 %) survivait dans les scènes flottantes suivantes ;
  chaque scène repart à 100 % (`pixels/mod.rs`).

**Corrigé (fr-t1.1)**
- Lignes de menu de 29,5 px : GPUI arrondit les longueurs écrites AVANT la mise en page ; `echelle::calees` leur
  donne les hauteurs entières (30, 29, 30…) qui reproduisent l'arrondi arête par arête de Chromium (voir
  `constantes-rendu.md`, « Positions fractionnaires »). Icône de menu centrée à l'égalité haute (`echelle::moitie`).
- Ombres : le shader de GPUI prend `blur_radius` pour l'écart-type, Skia prend `flou / 2` ; `ombres()` divise par 2.
  L'ombre est passée de « un cran trop pâle sur plus de 100 px » à ≤ 2 niveaux d'écart (voir plus bas).
- Texte : le harnais force `TextRenderingMode::Grayscale` (Chromium n'a pas de franges colorées).
- Poignée de la barre d'outils : `min-width: 16px` + `padding: 0 1px` en boîte de contenu = 18 px (2 px de moins).
- Texte de la notice remonté d'un pixel (ligne de 16,8 px arrondie à 17 par GPUI).

**Reste, irréductible sans toucher GPUI**
- **Règle du sas** (`xtask/src/pixels.rs`) : le masque contient TOUT pixel de delta > 0, les zones sont ses composantes
  8-connexes (une par passage de texte, pas par glyphe), le pire delta se prend sur toute l'image. Un passage de texte
  (40 à 90 px de large) ou une ligne de bordure à 1 niveau d'écart fait donc échouer la scène, même si chaque pixel
  est ≤ 16.
- **Glyphes** : le candidat tourne sur Linux (cosmic-text, paramètres de contraste du renderer wgpu inaccessibles à
  l'application), la référence sur Chromium/DirectWrite : traits plus fins, delta jusqu'à 190 sur les arêtes. Largeur
  du texte : **résolue (fr-t1.3)**. Deux causes : DirectWrite, appelé par GPUI avec un objet de typographie, n'applique
  plus `kern` de lui-même (« notice » +0,24 px, « Enregistré » +0,76 px à 12 px) : `polices::caracteristiques()` le
  redemande, et les largeurs sont celles de HarfBuzz/Chromium à 0,001 px ; et GPUI arrondit au pixel SUPÉRIEUR la feuille
  de texte (`elements/text.rs`) quand Chromium garde la fraction : une boîte ajustée au texte (notice, infobulle) est
  posée à la largeur entière de Chromium par `polices::largeur_ajustee`.
- **Ombres** : après correction, écart ≤ 2 niveaux, sauf quelques pixels isolés (formule d'erf approchée et 4
  échantillons du shader contre le flou de Skia). Bordures et séparateurs : 1 niveau de plus ou de moins (mélange
  alpha).
- **Icônes Lucide de 1,5 px** : géométrie identique, anticrénelage différent, delta max 54 à 94 selon la scène, zones
  d'environ 16 × 16 px (> 16 de delta : ce n'est pas un écart « de glyphe » toléré par la règle actuelle).
- **Survol du menu** : non traité (cause côté Chromium non élucidée) ; pointeur toujours garé.
## Dettes de parité : modale et palette T1 (fr-t1-palette)
Scènes `t1-{modale,palette,palette-aucun-resultat,palette-selection}` (décor `_t1-flottantes` : la boîte sur le fond
du workspace). Requête de `t1-palette` : « panneau » (et non « sidebar », absent d'une interface en français) : sur
Fragment elle ne trouve que `Application: Afficher / masquer le panneau de droite|gauche`, deux commandes du cœur
(`app:toggle-left-sidebar`, `app:toggle-right-sidebar`), que la scène Rust déclare avec leurs noms et raccourcis.
Mesure sur l'artefact CI (run 42873546813), boîte et ombre comprises :
- **Géométrie** : bords de boîte, bordures, hauteurs de lignes, de champ et de légende identiques à Fragment au pixel
  (largeur `content-box` de Chromium reproduite : 700 + 2 de bordure ; 560 + 48 + 2 pour la modale).
- **Glyphes** : texte du champ, des lignes et de la légende à bords différents (zones de 40 à 90 px de large : hors du
  sas de 20 × 20) ; la chasse de DirectWrite diffère d'environ 1 px sur une ligne de 340 px. Flèches « ↑↓ » et « ↵ » :
  glyphes de repli, dessinés différemment.
- **Ombre** `--shadow-l` : écart de 1 à 2 niveaux sur ~108 000 pixels hors de la boîte (flou de GPUI contre Chromium).
- **Ligne sélectionnée** : le fond `background-modifier-active-hover` donne (41, 41, 43) côté candidat contre
  (40, 41, 43) côté Fragment, sur 688 × 35 px. ★ Corrigé par fr-t1.4 : Chromium prémultiplie la teinte par le pourcentage exact et atténue le fond par l'octet d'opacité (`Couleur::sur_chromium`, `Palette::aplat`).
- **Icône de fermeture** de la modale : trait de 2 px dans notre jeu Lucide, plus fin (1,5 px) dans la référence.
- Dettes de fonctionnement : champ de saisie minimal (pas de curseur déplaçable, sélection ni collage) ; `localeCompare`
  approché (sans casse ni accents) pour l'ordre alphabétique à requête vide.
- **Décision du chef (10/11)** : scènes en `"depuis": "T2"` (en attente du shell, à repasser à T1 par `fr-t1-binaire`) ;
  les écarts de rendu relèvent de `fr-t1.1`.

## Dettes de parité : shell assemblé T1 (fr-t1-binaire)

Les 21 scènes des tâches fermées (onglets, splits, réglages et zoom, `t1-fenetre-vide`) sont rejouées sur l'application réelle (boot, coquille, pont clavier) depuis fr-t1-binaire. Runs CI 43011739987 puis 43013547871 (après les corrections 7 et 8) : **0 PNG vert sur 42**. Causes, chiffrées sur le second run sauf mention (px = pixels divergents), aucune tolérance nouvelle :

1. **Texte et icônes, origine décalée de 1 px (strict)** : toutes les scènes. Titres « New Tab » décalés de +1 px en x (boîte (52,14) 48×12, encre −5 à −10 %) : dette de largeur/position `fr-t1.3`. Icône du ruban `square-pen` (9,48) décalée de +1 px en y (encre +15 %) : placement vertical du ruban (`docks`, fr-t1-splits). Barre d'état : « Fragment 0.0.0 » (1191,786) 49×11 hors boîte, la largeur du texte arrondie au pixel supérieur par GPUI déplace l'élément ancré à droite.
2. **Arcs (strict faute de déclaration)** : coins du contour de l'onglet actif (4 zones de 5×6 et 5×5 px par onglet actif, delta max 71) et coin haut-gauche de la barre d'état (1178,780) 5×5. La catégorie « bords d'arrondi » (ADR-009, amendement 11:15) exige des arcs DÉCLARÉS par le candidat : ni `onglets` ni `shell::barre_etat` ne les déclarent encore (crochet à ajouter dans `capture.rs`, possédé par fr-t1.1).
3. **Poignée de split en abscisse fractionnaire** : bande 7×800 px à x = 657 (`t1-split-vertical`, `t1-glisser-depot`, 2 120 px dont 1 662 de delta ≤ 2), 12×800 à x = 1160 (`t1-boutons-fenetre`, 3 354 px) : dernier pixel du contour d'une barre de largeur fractionnaire (dette notée par fr-t1-splits).
4. **`t1-split-proportions`** : zone 629×800 à partir de x = 346 (5 046 px, delta max 201) : largeurs des panes imbriqués 25/50/25 décalées, à reprendre dans `splits::geometrie`.
5. **Aplat translucide de la zone de dépôt en clair** : `t1-glisser-zone-light`, 472 133 px à delta 1 ((210,210,236) contre (209,210,236)) : le jeton n'est pas pré-composé sur son fond (ADR-009 c, fr-t1.4).
6. **Pied du dock gauche absent** (`VaultSwitcher`, T2) : `t1-docks-ouverts`, trait (36,761) 260×1 et libellé « coffre » + 2 icônes.
7. **Fenêtre de réglages** (`t1-reglages-*`, ~11 000 px hors boîtes) : rail et contenu 1 px plus haut que la référence ; description du zoom non coupée (largeur maximale du bloc d'information du réglage) ; corrigé ici : le schéma de couleurs affiché (« Adapt to system » → « Dark »/« Light », fixé par la scène comme le harnais de Fragment).
8. **Scènes de zoom et de réglages sans décor** : leurs JSON n'avaient pas le layout par défaut du harnais (`PILE_PAR_DEFAUT`, 4 onglets, le 2e actif) ; corrigé ici (action `layout` ajoutée) : `t1-zoom-150` passe de 16 916 à 329 px hors boîtes en sombre (1 561 en clair) ; restent les causes 1 à 3 mises à l'échelle, et `t1-zoom-80` (2 636 px) où les traits de 0,8 px tombent en fractionnaire.

Le cliquet (`pixels/cliquet.json`, fr-t1.1) n'a pas de base pour ces 42 PNG : une scène qui diverge dans ses boîtes sans base échoue même dans le sas. Sa mise à jour (`--maj-cliquet`) revient au chef après les corrections 1 à 7.

## Format du rapport du sas (ADR-009)
`cargo xtask pixels` écrit sa section entre `<!-- sas:debut -->
Généré par `cargo xtask pixels` (ADR-009). Hors des boîtes de texte et d'icônes : strict, 0 pixel. Dans une boîte : anticrénelage seul, sous quatre garde-fous (encre totale, boîte d'encre, forme, teinte). Dans le halo d'une ombre déclarée (hors intérieur de la surface) : delta ≤ 2, sous trois garde-fous (étalement, glissement, encre).

| Scène | Verdict | Boîtes (texte/icônes) | Hors boîtes (px) | Dans les boîtes (px) | % dans les boîtes | Delta max | Largeur ±1 (dette fr-t1.3) | Ombres | Halos (px tolérés) | % halos | Delta ombre | Cliquet |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| t1-activation-fermeture-dark.png | Echec | 5/9 | 497 | 1651 | 33.4481 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-activation-fermeture-light.png | Echec | 5/9 | 579 | 1642 | 33.2658 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-boutons-fenetre-dark.png | Echec | 4/10 | 3354 | 1783 | 38.1717 | 199 | 4 | 0 | 0 | 0.0000 | 0 | — |
| t1-boutons-fenetre-light.png | Echec | 4/10 | 3478 | 1775 | 38.0004 | 192 | 4 | 0 | 0 | 0.0000 | 0 | — |
| t1-dock-replie-dark.png | Echec | 4/10 | 495 | 1493 | 32.3580 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-dock-replie-light.png | Echec | 4/10 | 579 | 1484 | 32.1630 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-docks-ouverts-dark.png | Echec | 4/12 | 1220 | 1719 | 32.0112 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-docks-ouverts-light.png | Echec | 4/12 | 1299 | 1708 | 31.8063 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-entete-vue-dark.png | Echec | 9/11 | 1289 | 2749 | 33.7591 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-entete-vue-light.png | Echec | 8/11 | 1369 | 2720 | 33.8182 | 124 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-fenetre-vide-dark.png | Echec | 3/9 | 497 | 1109 | 31.3631 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-fenetre-vide-light.png | Echec | 3/9 | 579 | 1102 | 31.1652 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-glisser-depot-dark.png | Echec | 5/11 | 2120 | 1508 | 31.5745 | 194 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-glisser-depot-light.png | Echec | 5/11 | 2233 | 1498 | 31.3652 | 212 | 1 | 0 | 0 | 0.0000 | 0 | — |
| t1-glisser-zone-dark.png | Echec | 5/9 | 833 | 1651 | 33.4481 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-glisser-zone-light.png | Echec | 5/9 | 472133 | 1641 | 33.2455 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-menu-onglet-dark.png | Echec | 6/10 | 640 | 1924 | 33.4667 | 138 | 0 | 1 | 1931 | 9.7822 | 2 | — |
| t1-menu-onglet-light.png | Echec | 6/10 | 1108 | 1916 | 33.3275 | 121 | 0 | 1 | 4899 | 29.7053 | 2 | — |
| t1-onglet-epingle-dark.png | Echec | 7/9 | 497 | 1922 | 33.5077 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-onglet-epingle-light.png | Echec | 6/9 | 579 | 1912 | 33.9248 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-onglet-survol-dark.png | Echec | 7/10 | 505 | 1966 | 33.4354 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-onglet-survol-light.png | Echec | 6/10 | 587 | 1958 | 33.8754 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-onglets-dark.png | Echec | 7/9 | 497 | 1922 | 33.5077 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-onglets-light.png | Echec | 6/9 | 579 | 1912 | 33.9248 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-pile-active-inactive-dark.png | Echec | 7/11 | 2209 | 2060 | 33.3549 | 157 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-pile-active-inactive-light.png | Echec | 7/11 | 2255 | 2046 | 33.1282 | 151 | 1 | 0 | 0 | 0.0000 | 0 | — |
| t1-poignee-survol-dark.png | Echec | 5/11 | 556 | 1508 | 31.5745 | 194 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-poignee-survol-light.png | Echec | 5/11 | 780 | 1498 | 31.3652 | 212 | 1 | 0 | 0 | 0.0000 | 0 | — |
| t1-reglages-appearance-dark.png | Echec | 29/17 | 10982 | 21557 | 38.1857 | 206 | 14 | 0 | 0 | 0.0000 | 0 | — |
| t1-reglages-appearance-light.png | Echec | 29/17 | 11020 | 21351 | 37.7466 | 221 | 12 | 0 | 0 | 0.0000 | 0 | — |
| t1-reglages-zoom-dark.png | Echec | 29/17 | 10991 | 21520 | 38.1405 | 206 | 14 | 0 | 0 | 0.0000 | 0 | — |
| t1-reglages-zoom-light.png | Echec | 29/17 | 11029 | 21315 | 37.7030 | 221 | 12 | 0 | 0 | 0.0000 | 0 | — |
| t1-split-horizontal-dark.png | Echec | 6/11 | 3039 | 1765 | 32.2316 | 138 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-split-horizontal-light.png | Echec | 6/11 | 2333 | 1751 | 31.9759 | 121 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-split-proportions-dark.png | Echec | 10/15 | 5046 | 2473 | 32.0420 | 203 | 1 | 0 | 0 | 0.0000 | 0 | — |
| t1-split-proportions-light.png | Echec | 10/15 | 3750 | 2457 | 31.8347 | 221 | 4 | 0 | 0 | 0.0000 | 0 | — |
| t1-split-vertical-dark.png | Echec | 6/11 | 2120 | 1779 | 32.4872 | 194 | 0 | 0 | 0 | 0.0000 | 0 | — |
| t1-split-vertical-light.png | Echec | 6/11 | 2233 | 1768 | 32.2863 | 212 | 1 | 0 | 0 | 0.0000 | 0 | — |
| t1-zoom-150-dark.png | Echec | 6/10 | 329 | 4704 | 36.4962 | 203 | 8 | 0 | 0 | 0.0000 | 0 | — |
| t1-zoom-150-light.png | Echec | 6/10 | 1561 | 4673 | 36.2557 | 221 | 8 | 0 | 0 | 0.0000 | 0 | — |
| t1-zoom-80-dark.png | Echec | 10/7 | 2636 | 1937 | 40.1451 | 158 | 8 | 0 | 0 | 0.0000 | 0 | — |
| t1-zoom-80-light.png | Echec | 9/7 | 2635 | 1936 | 40.8870 | 192 | 7 | 0 | 0 | 0.0000 | 0 | — |
| fenetre-vide-dark.png | preuve (sans référence Fragment) | — | — | — | — | — | — | — | — | — | — | — |
| fenetre-vide-light.png | preuve (sans référence Fragment) | — | — | — | — | — | — | — | — | — | — | — |
| jouet-glisser-maintenu-dark.png | preuve (sans référence Fragment) | — | — | — | — | — | — | — | — | — | — | — |
| jouet-glisser-relache-dark.png | preuve (sans référence Fragment) | — | — | — | — | — | — | — | — | — | — | — |
| jouet-saisie-dark.png | preuve (sans référence Fragment) | — | — | — | — | — | — | — | — | — | — | — |
| mire-dark.png | preuve (sans référence Fragment) | — | — | — | — | — | — | — | — | — | — | — |
| t1-binaire-dark.png | preuve (sans référence Fragment) | — | — | — | — | — | — | — | — | — | — | — |
| annotation-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| annotation-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| base-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| base-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| menu-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| menu-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| notice-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| notice-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| palette-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| palette-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-appearance-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-appearance-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-community-plugins-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-community-plugins-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-editor-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-editor-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-hotkeys-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| reglages-hotkeys-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| selection-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| selection-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| survol-explorateur-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| survol-explorateur-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-infobulle-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-infobulle-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-menu-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-menu-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-modale-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-modale-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-notice-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-notice-empilees-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-notice-empilees-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-notice-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-palette-aucun-resultat-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-palette-aucun-resultat-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-palette-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-palette-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-palette-selection-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-palette-selection-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-toolbar-horizontale-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-toolbar-horizontale-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-toolbar-sections-dark.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |
| t1-toolbar-sections-light.png | hors tranche | — | — | — | — | — | — | — | — | — | — | — |

Échecs :

- t1-activation-fermeture-dark.png : 497 px HORS boîtes (delta max 71, dont 81 de delta ≤ 2)
- t1-activation-fermeture-dark.png : divergence HORS boîte en (245,4) 5×6 px
- t1-activation-fermeture-dark.png : divergence HORS boîte en (448,4) 5×6 px
- t1-activation-fermeture-dark.png : divergence HORS boîte en (242,31) 5×5 px
- t1-activation-fermeture-dark.png : divergence HORS boîte en (451,31) 5×5 px
- t1-activation-fermeture-dark.png : divergence HORS boîte en (1178,780) 5×5 px
- t1-activation-fermeture-dark.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-activation-fermeture-dark.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-activation-fermeture-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-activation-fermeture-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-activation-fermeture-dark.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-activation-fermeture-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-activation-fermeture-light.png : 579 px HORS boîtes (delta max 57, dont 139 de delta ≤ 2)
- t1-activation-fermeture-light.png : divergence HORS boîte en (242,3) 9×33 px
- t1-activation-fermeture-light.png : divergence HORS boîte en (447,3) 9×33 px
- t1-activation-fermeture-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-activation-fermeture-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-activation-fermeture-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-activation-fermeture-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-activation-fermeture-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-activation-fermeture-light.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-activation-fermeture-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-boutons-fenetre-dark.png : 3354 px HORS boîtes (delta max 125, dont 86 de delta ≤ 2)
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (1160,0) 12×800 px
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (1170,4) 54×32 px
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (1244,13) 7×14 px
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (1145,17) 10×6 px
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-boutons-fenetre-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-boutons-fenetre-dark.png : boîte icone (1201,10) 17×15 : encre totale +34 % (borne ±25 %) ; forme : 38 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (-3, 0) px (strict) ; boîte d'encre : bord droit -2, bord bas +1 px (borne ±1) [encre +34.1 %, forme 37.5 %, teinte 1.0000, bords -3,+0,-2,+1]
- t1-boutons-fenetre-dark.png : boîte icone (1118,12) 16×16 : encre totale +inf % (borne ±25 %) ; forme : 100 % des traits sans équivalent dans un mot (borne 5 %) ; teinte : cosinus 0.000 (min 0.995) ; encre présente d'un seul côté [encre +inf %, forme 100.0 %, teinte 0.0000, bords —]
- t1-boutons-fenetre-dark.png : boîte icone (1252,12) 18×12 : encre totale +46 % (borne ±25 %) ; forme : 31 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, 0) px (strict) ; boîte d'encre : bord droit +8, bord bas -3 px (borne ±1) [encre +46.0 %, forme 30.8 %, teinte 1.0000, bords +3,+0,+8,-3]
- t1-boutons-fenetre-dark.png : boîte icone (1229,13) 14×14 : origine de la boîte d'encre décalée de (-7, 1) px (strict) ; boîte d'encre : bord droit -3, bord bas -1 px (borne ±1) [encre -12.9 %, forme 0.0 %, teinte 1.0000, bords -7,+1,-3,-1]
- t1-boutons-fenetre-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-boutons-fenetre-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-boutons-fenetre-dark.png : boîte texte (1090,16) 12×8 : encre totale +inf % (borne ±25 %) ; forme : 100 % des traits sans équivalent dans un mot (borne 5 %) ; teinte : cosinus 0.000 (min 0.995) ; encre présente d'un seul côté [encre +inf %, forme 100.0 %, teinte 0.0000, bords —]
- t1-boutons-fenetre-dark.png : boîte texte (1182,21) 8×4 : encre totale -72 % (borne ±25 %) ; forme : 100 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, 3) px (strict) ; boîte d'encre : bord droit -3, bord bas -2 px (borne ±1) [encre -72.2 %, forme 100.0 %, teinte 1.0000, bords +3,+3,-3,-2]
- t1-boutons-fenetre-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-boutons-fenetre-light.png : 3478 px HORS boîtes (delta max 125, dont 188 de delta ≤ 2)
- t1-boutons-fenetre-light.png : divergence HORS boîte en (1160,0) 64×800 px
- t1-boutons-fenetre-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-boutons-fenetre-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-boutons-fenetre-light.png : divergence HORS boîte en (1244,13) 7×14 px
- t1-boutons-fenetre-light.png : divergence HORS boîte en (1145,17) 10×6 px
- t1-boutons-fenetre-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-boutons-fenetre-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-boutons-fenetre-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-boutons-fenetre-light.png : boîte icone (1201,10) 17×15 : encre totale +26 % (borne ±25 %) ; forme : 81 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (-3, 0) px (strict) ; boîte d'encre : bord droit -2, bord bas +1 px (borne ±1) [encre +25.6 %, forme 81.2 %, teinte 1.0000, bords -3,+0,-2,+1]
- t1-boutons-fenetre-light.png : boîte icone (1118,12) 16×16 : encre totale +inf % (borne ±25 %) ; forme : 100 % des traits sans équivalent dans un mot (borne 5 %) ; teinte : cosinus 0.000 (min 0.995) ; encre présente d'un seul côté [encre +inf %, forme 100.0 %, teinte 0.0000, bords —]
- t1-boutons-fenetre-light.png : boîte icone (1252,12) 18×12 : forme : 30 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, 0) px (strict) ; boîte d'encre : bord droit +8, bord bas -3 px (borne ±1) [encre +23.7 %, forme 30.0 %, teinte 1.0000, bords +3,+0,+8,-3]
- t1-boutons-fenetre-light.png : boîte icone (1229,13) 14×14 : encre totale -26 % (borne ±25 %) ; forme : 25 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (-7, 1) px (strict) ; boîte d'encre : bord droit -3, bord bas -1 px (borne ±1) [encre -26.4 %, forme 25.0 %, teinte 1.0000, bords -7,+1,-3,-1]
- t1-boutons-fenetre-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-boutons-fenetre-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-boutons-fenetre-light.png : boîte texte (1090,16) 12×8 : encre totale +inf % (borne ±25 %) ; forme : 100 % des traits sans équivalent dans un mot (borne 5 %) ; teinte : cosinus 0.000 (min 0.995) ; encre présente d'un seul côté [encre +inf %, forme 100.0 %, teinte 0.0000, bords —]
- t1-boutons-fenetre-light.png : boîte texte (1182,21) 8×4 : encre totale -75 % (borne ±25 %) ; forme : 100 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, 3) px (strict) ; boîte d'encre : bord droit -3, bord bas -2 px (borne ±1) [encre -74.9 %, forme 100.0 %, teinte 1.0000, bords +3,+3,-3,-2]
- t1-boutons-fenetre-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-dock-replie-dark.png : 495 px HORS boîtes (delta max 71, dont 79 de delta ≤ 2)
- t1-dock-replie-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-dock-replie-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-dock-replie-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-dock-replie-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-dock-replie-dark.png : divergence HORS boîte en (1178,780) 5×5 px
- t1-dock-replie-dark.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-dock-replie-dark.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-dock-replie-dark.png : boîte icone (1030,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +14.3 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-dock-replie-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-dock-replie-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-dock-replie-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-dock-replie-light.png : 579 px HORS boîtes (delta max 57, dont 142 de delta ≤ 2)
- t1-dock-replie-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-dock-replie-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-dock-replie-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-dock-replie-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-dock-replie-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-dock-replie-light.png : boîte icone (1030,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.7 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-dock-replie-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-dock-replie-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-dock-replie-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-docks-ouverts-dark.png : 1220 px HORS boîtes (delta max 175, dont 90 de delta ≤ 2)
- t1-docks-ouverts-dark.png : divergence HORS boîte en (300,3) 6×7 px
- t1-docks-ouverts-dark.png : divergence HORS boîte en (502,3) 6×7 px
- t1-docks-ouverts-dark.png : divergence HORS boîte en (297,31) 5×5 px
- t1-docks-ouverts-dark.png : divergence HORS boîte en (506,31) 5×5 px
- t1-docks-ouverts-dark.png : divergence HORS boîte en (36,761) 260×1 px
- t1-docks-ouverts-dark.png : divergence HORS boîte en (244,774) 15×15 px
- t1-docks-ouverts-dark.png : divergence HORS boîte en (270,774) 13×15 px
- t1-docks-ouverts-dark.png : divergence HORS boîte en (53,775) 8×5 px
- t1-docks-ouverts-dark.png : boîte icone (46,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +14.3 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-docks-ouverts-dark.png : boîte icone (1030,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +14.3 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-docks-ouverts-dark.png : boîte icone (1062,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +14.3 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-docks-ouverts-dark.png : boîte texte (313,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-docks-ouverts-dark.png : boîte texte (519,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-docks-ouverts-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-docks-ouverts-light.png : 1299 px HORS boîtes (delta max 181, dont 153 de delta ≤ 2)
- t1-docks-ouverts-light.png : divergence HORS boîte en (297,3) 9×33 px
- t1-docks-ouverts-light.png : divergence HORS boîte en (502,3) 9×33 px
- t1-docks-ouverts-light.png : divergence HORS boîte en (36,761) 260×1 px
- t1-docks-ouverts-light.png : divergence HORS boîte en (244,774) 15×15 px
- t1-docks-ouverts-light.png : divergence HORS boîte en (270,774) 13×15 px
- t1-docks-ouverts-light.png : divergence HORS boîte en (53,775) 8×5 px
- t1-docks-ouverts-light.png : divergence HORS boîte en (70,776) 34×10 px
- t1-docks-ouverts-light.png : divergence HORS boîte en (249,777) 5×6 px
- t1-docks-ouverts-light.png : boîte icone (46,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.7 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-docks-ouverts-light.png : boîte icone (1030,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.7 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-docks-ouverts-light.png : boîte icone (1062,9) 16×19 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.7 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+1]
- t1-docks-ouverts-light.png : boîte texte (313,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-docks-ouverts-light.png : boîte texte (519,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-docks-ouverts-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-entete-vue-dark.png : 1289 px HORS boîtes (delta max 144, dont 593 de delta ≤ 2)
- t1-entete-vue-dark.png : divergence HORS boîte en (245,4) 5×6 px
- t1-entete-vue-dark.png : divergence HORS boîte en (448,4) 5×6 px
- t1-entete-vue-dark.png : divergence HORS boîte en (242,31) 5×5 px
- t1-entete-vue-dark.png : divergence HORS boîte en (451,31) 5×5 px
- t1-entete-vue-dark.png : divergence HORS boîte en (1248,38) 26×26 px
- t1-entete-vue-dark.png : divergence HORS boîte en (36,71) 19×13 px
- t1-entete-vue-dark.png : divergence HORS boîte en (56,74) 27×10 px
- t1-entete-vue-dark.png : divergence HORS boîte en (1178,780) 5×5 px
- t1-entete-vue-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-entete-vue-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-entete-vue-dark.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-entete-vue-dark.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-entete-vue-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-entete-vue-light.png : 1369 px HORS boîtes (delta max 121, dont 659 de delta ≤ 2)
- t1-entete-vue-light.png : divergence HORS boîte en (242,3) 9×33 px
- t1-entete-vue-light.png : divergence HORS boîte en (447,3) 9×33 px
- t1-entete-vue-light.png : divergence HORS boîte en (1248,38) 26×26 px
- t1-entete-vue-light.png : divergence HORS boîte en (36,71) 19×13 px
- t1-entete-vue-light.png : divergence HORS boîte en (56,74) 27×10 px
- t1-entete-vue-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-entete-vue-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-entete-vue-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-entete-vue-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-entete-vue-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-entete-vue-light.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-entete-vue-light.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-entete-vue-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-fenetre-vide-dark.png : 497 px HORS boîtes (delta max 71, dont 81 de delta ≤ 2)
- t1-fenetre-vide-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-fenetre-vide-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-fenetre-vide-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-fenetre-vide-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-fenetre-vide-dark.png : divergence HORS boîte en (1178,780) 5×5 px
- t1-fenetre-vide-dark.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-fenetre-vide-dark.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-fenetre-vide-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-fenetre-vide-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-fenetre-vide-light.png : 579 px HORS boîtes (delta max 57, dont 139 de delta ≤ 2)
- t1-fenetre-vide-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-fenetre-vide-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-fenetre-vide-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-fenetre-vide-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-fenetre-vide-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-fenetre-vide-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-fenetre-vide-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-glisser-depot-dark.png : 2120 px HORS boîtes (delta max 71, dont 1662 de delta ≤ 2)
- t1-glisser-depot-dark.png : divergence HORS boîte en (657,0) 7×800 px
- t1-glisser-depot-dark.png : divergence HORS boîte en (662,3) 6×7 px
- t1-glisser-depot-dark.png : divergence HORS boîte en (864,3) 6×7 px
- t1-glisser-depot-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-glisser-depot-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-glisser-depot-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-glisser-depot-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-glisser-depot-dark.png : divergence HORS boîte en (868,31) 5×5 px
- t1-glisser-depot-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-glisser-depot-dark.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.5 %, forme 0.0 %, teinte 0.9997, bords +1,+0,+0,+0]
- t1-glisser-depot-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-glisser-depot-light.png : 2233 px HORS boîtes (delta max 57, dont 1751 de delta ≤ 2)
- t1-glisser-depot-light.png : divergence HORS boîte en (657,0) 11×800 px
- t1-glisser-depot-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-glisser-depot-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-glisser-depot-light.png : divergence HORS boîte en (864,3) 9×33 px
- t1-glisser-depot-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-glisser-depot-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-glisser-depot-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-glisser-depot-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-glisser-depot-light.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +8.7 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+1,+1]
- t1-glisser-depot-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-glisser-zone-dark.png : 833 px HORS boîtes (delta max 71, dont 409 de delta ≤ 2)
- t1-glisser-zone-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-glisser-zone-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-glisser-zone-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-glisser-zone-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-glisser-zone-dark.png : divergence HORS boîte en (663,36) 1×1 px
- t1-glisser-zone-dark.png : divergence HORS boîte en (1274,36) 1×1 px
- t1-glisser-zone-dark.png : divergence HORS boîte en (661,37) 1×1 px
- t1-glisser-zone-dark.png : divergence HORS boîte en (1276,37) 1×1 px
- t1-glisser-zone-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-glisser-zone-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-glisser-zone-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-glisser-zone-dark.png : boîte texte (1202,432) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-glisser-zone-light.png : 472133 px HORS boîtes (delta max 57, dont 471693 de delta ≤ 2)
- t1-glisser-zone-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-glisser-zone-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-glisser-zone-light.png : divergence HORS boîte en (658,36) 622×764 px
- t1-glisser-zone-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-glisser-zone-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-glisser-zone-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-glisser-zone-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-glisser-zone-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-glisser-zone-light.png : boîte texte (1202,432) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +10.9 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-menu-onglet-dark.png : 640 px HORS boîtes (delta max 71, dont 112 de delta ≤ 2)
- t1-menu-onglet-dark.png : divergence HORS boîte en (245,4) 5×6 px
- t1-menu-onglet-dark.png : divergence HORS boîte en (448,4) 5×6 px
- t1-menu-onglet-dark.png : divergence HORS boîte en (670,15) 5×5 px
- t1-menu-onglet-dark.png : divergence HORS boîte en (676,15) 11×5 px
- t1-menu-onglet-dark.png : divergence HORS boîte en (697,15) 7×5 px
- t1-menu-onglet-dark.png : divergence HORS boîte en (710,15) 7×5 px
- t1-menu-onglet-dark.png : divergence HORS boîte en (703,17) 7×3 px
- t1-menu-onglet-dark.png : divergence HORS boîte en (688,18) 6×2 px
- t1-menu-onglet-dark.png : 106 px de delta > 2 dans les halos d'ombre
- t1-menu-onglet-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-menu-onglet-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-menu-onglet-dark.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-menu-onglet-dark.png : boîte texte (594,34) 44×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -15.7 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-menu-onglet-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-menu-onglet-light.png : 1108 px HORS boîtes (delta max 57, dont 541 de delta ≤ 2)
- t1-menu-onglet-light.png : divergence HORS boîte en (242,3) 9×33 px
- t1-menu-onglet-light.png : divergence HORS boîte en (447,3) 9×33 px
- t1-menu-onglet-light.png : divergence HORS boîte en (563,15) 176×7 px
- t1-menu-onglet-light.png : divergence HORS boîte en (561,21) 1×1 px
- t1-menu-onglet-light.png : divergence HORS boîte en (555,22) 5×36 px
- t1-menu-onglet-light.png : divergence HORS boîte en (740,22) 3×3 px
- t1-menu-onglet-light.png : divergence HORS boîte en (743,26) 2×26 px
- t1-menu-onglet-light.png : divergence HORS boîte en (740,53) 4×5 px
- t1-menu-onglet-light.png : 101 px de delta > 2 dans les halos d'ombre
- t1-menu-onglet-light.png : surface ombrée (555,20) 190×40 : ombre : étalement -0.10 px (borne ±0.08) : flou différent [haut -0.06 px +0.1 %, bas -0.29 px -4.8 %, gauche -0.05 px -0.2 %, droite +0.01 px -1.3 %]
- t1-menu-onglet-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-menu-onglet-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-menu-onglet-light.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-menu-onglet-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-onglet-epingle-dark.png : 497 px HORS boîtes (delta max 71, dont 81 de delta ≤ 2)
- t1-onglet-epingle-dark.png : divergence HORS boîte en (245,4) 5×6 px
- t1-onglet-epingle-dark.png : divergence HORS boîte en (448,4) 5×6 px
- t1-onglet-epingle-dark.png : divergence HORS boîte en (242,31) 5×5 px
- t1-onglet-epingle-dark.png : divergence HORS boîte en (451,31) 5×5 px
- t1-onglet-epingle-dark.png : divergence HORS boîte en (1178,780) 5×5 px
- t1-onglet-epingle-dark.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-onglet-epingle-dark.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-onglet-epingle-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglet-epingle-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-onglet-epingle-dark.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglet-epingle-dark.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglet-epingle-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-onglet-epingle-light.png : 579 px HORS boîtes (delta max 57, dont 139 de delta ≤ 2)
- t1-onglet-epingle-light.png : divergence HORS boîte en (242,3) 9×33 px
- t1-onglet-epingle-light.png : divergence HORS boîte en (447,3) 9×33 px
- t1-onglet-epingle-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-onglet-epingle-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-onglet-epingle-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-onglet-epingle-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglet-epingle-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-onglet-epingle-light.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglet-epingle-light.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglet-epingle-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-onglet-survol-dark.png : 505 px HORS boîtes (delta max 71, dont 89 de delta ≤ 2)
- t1-onglet-survol-dark.png : divergence HORS boîte en (245,4) 5×6 px
- t1-onglet-survol-dark.png : divergence HORS boîte en (448,4) 5×6 px
- t1-onglet-survol-dark.png : divergence HORS boîte en (455,8) 2×2 px
- t1-onglet-survol-dark.png : divergence HORS boîte en (653,8) 2×2 px
- t1-onglet-survol-dark.png : divergence HORS boîte en (455,30) 2×2 px
- t1-onglet-survol-dark.png : divergence HORS boîte en (653,30) 2×2 px
- t1-onglet-survol-dark.png : divergence HORS boîte en (242,31) 5×5 px
- t1-onglet-survol-dark.png : divergence HORS boîte en (451,31) 5×5 px
- t1-onglet-survol-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglet-survol-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-onglet-survol-dark.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -8.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglet-survol-dark.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglet-survol-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-onglet-survol-light.png : 587 px HORS boîtes (delta max 57, dont 147 de delta ≤ 2)
- t1-onglet-survol-light.png : divergence HORS boîte en (242,3) 9×33 px
- t1-onglet-survol-light.png : divergence HORS boîte en (447,3) 9×33 px
- t1-onglet-survol-light.png : divergence HORS boîte en (455,8) 2×2 px
- t1-onglet-survol-light.png : divergence HORS boîte en (653,8) 2×2 px
- t1-onglet-survol-light.png : divergence HORS boîte en (455,30) 2×2 px
- t1-onglet-survol-light.png : divergence HORS boîte en (653,30) 2×2 px
- t1-onglet-survol-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-onglet-survol-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-onglet-survol-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglet-survol-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-onglet-survol-light.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +12.6 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglet-survol-light.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglet-survol-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-onglets-dark.png : 497 px HORS boîtes (delta max 71, dont 81 de delta ≤ 2)
- t1-onglets-dark.png : divergence HORS boîte en (245,4) 5×6 px
- t1-onglets-dark.png : divergence HORS boîte en (448,4) 5×6 px
- t1-onglets-dark.png : divergence HORS boîte en (242,31) 5×5 px
- t1-onglets-dark.png : divergence HORS boîte en (451,31) 5×5 px
- t1-onglets-dark.png : divergence HORS boîte en (1178,780) 5×5 px
- t1-onglets-dark.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-onglets-dark.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-onglets-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglets-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-onglets-dark.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglets-dark.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-onglets-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-onglets-light.png : 579 px HORS boîtes (delta max 57, dont 139 de delta ≤ 2)
- t1-onglets-light.png : divergence HORS boîte en (242,3) 9×33 px
- t1-onglets-light.png : divergence HORS boîte en (447,3) 9×33 px
- t1-onglets-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-onglets-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-onglets-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-onglets-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglets-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-onglets-light.png : boîte texte (464,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglets-light.png : boîte texte (670,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-onglets-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-pile-active-inactive-dark.png : 2209 px HORS boîtes (delta max 71, dont 1650 de delta ≤ 2)
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (657,0) 2×800 px
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (1048,3) 10×33 px
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (854,4) 10×32 px
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-pile-active-inactive-dark.png : divergence HORS boîte en (1178,780) 5×5 px
- t1-pile-active-inactive-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-pile-active-inactive-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-pile-active-inactive-dark.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -10.4 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-pile-active-inactive-dark.png : boîte texte (870,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.5 %, forme 0.0 %, teinte 0.9997, bords +1,+0,+0,+0]
- t1-pile-active-inactive-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-pile-active-inactive-light.png : 2255 px HORS boîtes (delta max 57, dont 1674 de delta ≤ 2)
- t1-pile-active-inactive-light.png : divergence HORS boîte en (657,0) 2×800 px
- t1-pile-active-inactive-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-pile-active-inactive-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-pile-active-inactive-light.png : divergence HORS boîte en (854,3) 10×33 px
- t1-pile-active-inactive-light.png : divergence HORS boîte en (1048,3) 10×33 px
- t1-pile-active-inactive-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-pile-active-inactive-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-pile-active-inactive-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-pile-active-inactive-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-pile-active-inactive-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-pile-active-inactive-light.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +10.1 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+1,+1]
- t1-pile-active-inactive-light.png : boîte texte (870,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.5 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-pile-active-inactive-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-poignee-survol-dark.png : 556 px HORS boîtes (delta max 71, dont 99 de delta ≤ 2)
- t1-poignee-survol-dark.png : divergence HORS boîte en (662,3) 6×7 px
- t1-poignee-survol-dark.png : divergence HORS boîte en (864,3) 6×7 px
- t1-poignee-survol-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-poignee-survol-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-poignee-survol-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-poignee-survol-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-poignee-survol-dark.png : divergence HORS boîte en (659,31) 5×5 px
- t1-poignee-survol-dark.png : divergence HORS boîte en (868,31) 5×5 px
- t1-poignee-survol-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-poignee-survol-dark.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.5 %, forme 0.0 %, teinte 0.9997, bords +1,+0,+0,+0]
- t1-poignee-survol-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-poignee-survol-light.png : 780 px HORS boîtes (delta max 57, dont 301 de delta ≤ 2)
- t1-poignee-survol-light.png : divergence HORS boîte en (656,0) 1×35 px
- t1-poignee-survol-light.png : divergence HORS boîte en (659,0) 9×36 px
- t1-poignee-survol-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-poignee-survol-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-poignee-survol-light.png : divergence HORS boîte en (864,3) 9×33 px
- t1-poignee-survol-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-poignee-survol-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-poignee-survol-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-poignee-survol-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-poignee-survol-light.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +8.7 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+1,+1]
- t1-poignee-survol-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-reglages-appearance-dark.png : 10982 px HORS boîtes (delta max 232, dont 215 de delta ≤ 2)
- t1-reglages-appearance-dark.png : divergence HORS boîte en (0,31) 1000×2 px
- t1-reglages-appearance-dark.png : divergence HORS boîte en (14,46) 208×7 px
- t1-reglages-appearance-dark.png : divergence HORS boîte en (261,52) 715×9 px
- t1-reglages-appearance-dark.png : divergence HORS boîte en (822,70) 138×33 px
- t1-reglages-appearance-dark.png : divergence HORS boîte en (14,74) 208×7 px
- t1-reglages-appearance-dark.png : divergence HORS boîte en (831,82) 1×9 px
- t1-reglages-appearance-dark.png : divergence HORS boîte en (70,106) 1×2 px
- t1-reglages-appearance-dark.png : divergence HORS boîte en (70,110) 1×4 px
- t1-reglages-appearance-dark.png : boîte icone (924,10) 14×14 : encre totale +40 % (borne ±25 %) [encre +40.2 %, forme 0.0 %, teinte 1.0000, bords +0,+0,+0,+0]
- t1-reglages-appearance-dark.png : boîte icone (972,12) 10×10 : encre totale +44 % (borne ±25 %) [encre +43.7 %, forme 0.0 %, teinte 1.0000, bords +0,+0,+0,+0]
- t1-reglages-appearance-dark.png : boîte icone (24,56) 14×14 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +14.8 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+1,-1]
- t1-reglages-appearance-dark.png : boîte texte (44,56) 103×15 : encre totale +41 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +41.4 %, forme 0.0 %, teinte 0.9996, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (277,70) 124×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -4.6 %, forme 0.0 %, teinte 0.9999, bords +1,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (833,79) 28×13 : origine de la boîte d'encre décalée de (3, -1) px (strict) [encre -11.9 %, forme 0.0 %, teinte 1.0000, bords +3,-1,+1,-1]
- t1-reglages-appearance-dark.png : boîte icone (940,79) 10×14 : encre totale +30 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +29.8 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (17,103) 52×12 : forme : 42 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas -1 px (borne ±1) [encre +2.0 %, forme 41.6 %, teinte 0.9988, bords +0,-1,-3,-1]
- t1-reglages-appearance-dark.png : boîte icone (18,128) 16×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +15.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (41,131) 48×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -3.0 %, forme 0.0 %, teinte 0.9993, bords +0,-1,-1,-1]
- t1-reglages-appearance-dark.png : boîte texte (276,148) 53×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -4.4 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte icone (902,156) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +14.7 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte icone (937,156) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (817,157) 44×13 : origine de la boîte d'encre décalée de (3, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas -1 px (borne ±1) [encre -8.2 %, forme 0.0 %, teinte 0.9999, bords +3,-1,+2,-1]
- t1-reglages-appearance-dark.png : boîte icone (868,157) 10×14 : encre totale +30 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +29.8 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte icone (17,160) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +15.8 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (42,162) 37×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -2.4 %, forme 0.0 %, teinte 0.9981, bords +1,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte icone (17,192) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (42,194) 83×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -3.9 %, forme 0.0 %, teinte 0.9983, bords +1,-1,-1,-1]
- t1-reglages-appearance-dark.png : boîte icone (17,223) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +12.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (41,226) 75×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -7.8 %, forme 0.0 %, teinte 0.9998, bords +0,-1,-1,-1]
- t1-reglages-appearance-dark.png : boîte texte (276,226) 85×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -4.6 %, forme 0.0 %, teinte 0.9999, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte icone (17,256) 18×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +14.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (42,257) 49×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +1.9 %, forme 0.0 %, teinte 0.9988, bords +1,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte icone (17,286) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.7 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (41,288) 118×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -1.0 %, forme 0.0 %, teinte 0.9989, bords +0,-1,-1,-1]
- t1-reglages-appearance-dark.png : boîte texte (263,300) 35×15 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -4.3 %, forme 0.0 %, teinte 1.0000, bords +1,+0,-1,+0]
- t1-reglages-appearance-dark.png : boîte icone (17,317) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.1 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (42,320) 54×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -2.3 %, forme 0.0 %, teinte 0.9982, bords +1,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (277,345) 58×14 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -4.8 %, forme 0.0 %, teinte 0.9999, bords +1,+0,-1,+0]
- t1-reglages-appearance-dark.png : boîte texte (17,359) 85×12 : forme : 65 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas -1 px (borne ±1) [encre +2.8 %, forme 65.3 %, teinte 0.9990, bords +0,-1,-3,-1]
- t1-reglages-appearance-dark.png : boîte texte (277,368) 313×14 : origine de la boîte d'encre décalée de (1, 1) px (strict) [encre +2.1 %, forme 0.0 %, teinte 0.9990, bords +1,+1,+0,+1]
- t1-reglages-appearance-dark.png : boîte icone (17,385) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.6 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-dark.png : boîte texte (41,388) 37×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +1.8 %, forme 0.0 %, teinte 0.9992, bords +0,-1,-1,-1]
- t1-reglages-appearance-dark.png : boîte texte (766,465) 37×13 : forme : 46 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, -1) px (strict) ; boîte d'encre : bord droit +4, bord bas -1 px (borne ±1) [encre +3.9 %, forme 46.2 %, teinte 0.9989, bords +3,-1,+4,-1]
- t1-reglages-appearance-dark.png : boîte texte (276,473) 71×13 : encre totale +29 % (borne ±25 %) ; forme : 131 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +0, bord bas +7 px (borne ±1) [encre +28.8 %, forme 130.8 %, teinte 0.9999, bords +0,+3,+0,+7]
- t1-reglages-appearance-dark.png : boîte texte (276,494) 429×15 : encre totale +47 % (borne ±25 %) ; forme : 167 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +42, bord bas -3 px (borne ±1) [encre +46.6 %, forme 166.7 %, teinte 0.9989, bords +0,+3,+42,-3]
- t1-reglages-appearance-dark.png : boîte icone (938,495) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +15.1 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : 11020 px HORS boîtes (delta max 221, dont 108 de delta ≤ 2)
- t1-reglages-appearance-light.png : divergence HORS boîte en (0,31) 1000×2 px
- t1-reglages-appearance-light.png : divergence HORS boîte en (14,46) 208×7 px
- t1-reglages-appearance-light.png : divergence HORS boîte en (261,52) 715×10 px
- t1-reglages-appearance-light.png : divergence HORS boîte en (822,70) 138×33 px
- t1-reglages-appearance-light.png : divergence HORS boîte en (14,74) 208×7 px
- t1-reglages-appearance-light.png : divergence HORS boîte en (831,82) 1×9 px
- t1-reglages-appearance-light.png : divergence HORS boîte en (70,106) 1×2 px
- t1-reglages-appearance-light.png : divergence HORS boîte en (70,110) 1×4 px
- t1-reglages-appearance-light.png : boîte icone (924,10) 14×14 : encre totale +25 % (borne ±25 %) [encre +25.1 %, forme 0.0 %, teinte 1.0000, bords +0,+0,+0,+0]
- t1-reglages-appearance-light.png : boîte icone (24,56) 14×14 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +10.6 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+1,-1]
- t1-reglages-appearance-light.png : boîte texte (44,56) 103×15 : encre totale +27 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +26.7 %, forme 0.0 %, teinte 0.9997, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (277,70) 124×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +5.9 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+0]
- t1-reglages-appearance-light.png : boîte texte (833,79) 31×15 : origine de la boîte d'encre décalée de (2, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas -1 px (borne ±1) [encre +5.0 %, forme 0.0 %, teinte 0.9988, bords +2,-1,+2,-1]
- t1-reglages-appearance-light.png : boîte icone (940,79) 10×14 : encre totale +40 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +40.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (17,103) 52×12 : forme : 31 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas +0 px (borne ±1) [encre +17.2 %, forme 31.2 %, teinte 0.9999, bords +0,-1,-3,+0]
- t1-reglages-appearance-light.png : boîte icone (18,128) 16×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +11.9 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (41,131) 48×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +8.8 %, forme 0.0 %, teinte 0.9997, bords +0,-1,-1,+0]
- t1-reglages-appearance-light.png : boîte texte (276,148) 53×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +5.9 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+0]
- t1-reglages-appearance-light.png : boîte icone (902,156) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +11.6 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte icone (937,156) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (817,157) 44×13 : origine de la boîte d'encre décalée de (2, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas +0 px (borne ±1) [encre +4.5 %, forme 0.0 %, teinte 0.9994, bords +2,-1,+2,+0]
- t1-reglages-appearance-light.png : boîte icone (868,157) 10×14 : encre totale +40 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +40.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte icone (17,160) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.7 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (42,162) 37×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +8.5 %, forme 0.0 %, teinte 0.9996, bords +1,-1,+0,+0]
- t1-reglages-appearance-light.png : boîte texte (276,169) 322×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +16.7 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+0]
- t1-reglages-appearance-light.png : boîte icone (17,192) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +9.9 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (42,194) 83×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +8.2 %, forme 0.0 %, teinte 0.9996, bords +1,-1,-1,+0]
- t1-reglages-appearance-light.png : boîte icone (17,223) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -1.1 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (41,226) 75×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +3.3 %, forme 0.0 %, teinte 0.9999, bords +0,-1,-1,-1]
- t1-reglages-appearance-light.png : boîte texte (276,226) 85×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +7.6 %, forme 0.0 %, teinte 0.9997, bords +0,-1,+0,+0]
- t1-reglages-appearance-light.png : boîte icone (17,256) 18×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (42,257) 49×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +14.9 %, forme 0.0 %, teinte 0.9995, bords +1,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte icone (17,286) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (41,288) 118×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +12.2 %, forme 0.0 %, teinte 0.9997, bords +0,-1,-1,-1]
- t1-reglages-appearance-light.png : boîte texte (263,300) 35×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +3.6 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+1]
- t1-reglages-appearance-light.png : boîte icone (17,317) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (42,320) 54×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +11.0 %, forme 0.0 %, teinte 0.9992, bords +1,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (277,345) 58×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +5.7 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+1]
- t1-reglages-appearance-light.png : boîte texte (17,359) 85×12 : forme : 51 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas +0 px (borne ±1) [encre +18.3 %, forme 50.5 %, teinte 0.9999, bords +0,-1,-3,+0]
- t1-reglages-appearance-light.png : boîte texte (277,368) 313×14 : origine de la boîte d'encre décalée de (1, 1) px (strict) [encre +15.9 %, forme 0.0 %, teinte 0.9996, bords +1,+1,+0,+1]
- t1-reglages-appearance-light.png : boîte icone (17,385) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-appearance-light.png : boîte texte (41,388) 37×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +14.8 %, forme 0.0 %, teinte 0.9998, bords +0,-1,-1,-1]
- t1-reglages-appearance-light.png : boîte texte (262,420) 45×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +4.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,+1]
- t1-reglages-appearance-light.png : boîte texte (766,465) 37×13 : forme : 41 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, -1) px (strict) ; boîte d'encre : bord droit +4, bord bas +0 px (borne ±1) [encre +16.7 %, forme 41.2 %, teinte 1.0000, bords +3,-1,+4,+0]
- t1-reglages-appearance-light.png : boîte texte (276,473) 71×13 : encre totale +43 % (borne ±25 %) ; forme : 154 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +0, bord bas +8 px (borne ±1) [encre +42.9 %, forme 153.6 %, teinte 0.9999, bords +0,+3,+0,+8]
- t1-reglages-appearance-light.png : boîte texte (276,494) 429×15 : encre totale +67 % (borne ±25 %) ; forme : 151 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +42, bord bas -3 px (borne ±1) [encre +66.5 %, forme 151.1 %, teinte 0.9997, bords +0,+3,+42,-3]
- t1-reglages-appearance-light.png : boîte icone (938,495) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +12.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : 10991 px HORS boîtes (delta max 232, dont 215 de delta ≤ 2)
- t1-reglages-zoom-dark.png : divergence HORS boîte en (0,31) 1000×2 px
- t1-reglages-zoom-dark.png : divergence HORS boîte en (14,46) 208×7 px
- t1-reglages-zoom-dark.png : divergence HORS boîte en (261,52) 715×9 px
- t1-reglages-zoom-dark.png : divergence HORS boîte en (822,70) 138×33 px
- t1-reglages-zoom-dark.png : divergence HORS boîte en (14,74) 208×7 px
- t1-reglages-zoom-dark.png : divergence HORS boîte en (831,82) 1×9 px
- t1-reglages-zoom-dark.png : divergence HORS boîte en (70,106) 1×2 px
- t1-reglages-zoom-dark.png : divergence HORS boîte en (70,110) 1×4 px
- t1-reglages-zoom-dark.png : boîte icone (924,10) 14×14 : encre totale +40 % (borne ±25 %) [encre +40.2 %, forme 0.0 %, teinte 1.0000, bords +0,+0,+0,+0]
- t1-reglages-zoom-dark.png : boîte icone (972,12) 10×10 : encre totale +44 % (borne ±25 %) [encre +43.7 %, forme 0.0 %, teinte 1.0000, bords +0,+0,+0,+0]
- t1-reglages-zoom-dark.png : boîte icone (24,56) 14×14 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +14.8 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+1,-1]
- t1-reglages-zoom-dark.png : boîte texte (44,56) 103×15 : encre totale +41 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +41.4 %, forme 0.0 %, teinte 0.9996, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (277,70) 124×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -4.6 %, forme 0.0 %, teinte 0.9999, bords +1,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (833,79) 28×13 : origine de la boîte d'encre décalée de (3, -1) px (strict) [encre -11.9 %, forme 0.0 %, teinte 1.0000, bords +3,-1,+1,-1]
- t1-reglages-zoom-dark.png : boîte icone (940,79) 10×14 : encre totale +30 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +29.8 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (17,103) 52×12 : forme : 42 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas -1 px (borne ±1) [encre +2.0 %, forme 41.6 %, teinte 0.9988, bords +0,-1,-3,-1]
- t1-reglages-zoom-dark.png : boîte icone (18,128) 16×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +15.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (41,131) 48×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -3.0 %, forme 0.0 %, teinte 0.9993, bords +0,-1,-1,-1]
- t1-reglages-zoom-dark.png : boîte texte (276,148) 53×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -4.4 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte icone (902,156) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +14.7 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte icone (937,156) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (817,157) 44×13 : origine de la boîte d'encre décalée de (3, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas -1 px (borne ±1) [encre -8.2 %, forme 0.0 %, teinte 0.9999, bords +3,-1,+2,-1]
- t1-reglages-zoom-dark.png : boîte icone (868,157) 10×14 : encre totale +30 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +29.8 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte icone (17,160) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +15.8 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (42,162) 37×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -2.4 %, forme 0.0 %, teinte 0.9981, bords +1,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte icone (17,192) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (42,194) 83×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -3.9 %, forme 0.0 %, teinte 0.9983, bords +1,-1,-1,-1]
- t1-reglages-zoom-dark.png : boîte icone (17,223) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +12.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (41,226) 75×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -7.8 %, forme 0.0 %, teinte 0.9998, bords +0,-1,-1,-1]
- t1-reglages-zoom-dark.png : boîte texte (276,226) 85×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -4.6 %, forme 0.0 %, teinte 0.9999, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte icone (17,256) 18×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +14.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (42,257) 49×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +1.9 %, forme 0.0 %, teinte 0.9988, bords +1,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte icone (17,286) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.7 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (41,288) 118×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -1.0 %, forme 0.0 %, teinte 0.9989, bords +0,-1,-1,-1]
- t1-reglages-zoom-dark.png : boîte texte (263,300) 35×15 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -4.3 %, forme 0.0 %, teinte 1.0000, bords +1,+0,-1,+0]
- t1-reglages-zoom-dark.png : boîte icone (17,317) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.1 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (42,320) 54×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -2.3 %, forme 0.0 %, teinte 0.9982, bords +1,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (277,345) 58×14 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -4.8 %, forme 0.0 %, teinte 0.9999, bords +1,+0,-1,+0]
- t1-reglages-zoom-dark.png : boîte texte (17,359) 85×12 : forme : 65 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas -1 px (borne ±1) [encre +2.8 %, forme 65.3 %, teinte 0.9990, bords +0,-1,-3,-1]
- t1-reglages-zoom-dark.png : boîte texte (277,368) 313×14 : origine de la boîte d'encre décalée de (1, 1) px (strict) [encre +2.1 %, forme 0.0 %, teinte 0.9990, bords +1,+1,+0,+1]
- t1-reglages-zoom-dark.png : boîte icone (17,385) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.6 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-dark.png : boîte texte (41,388) 37×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +1.8 %, forme 0.0 %, teinte 0.9992, bords +0,-1,-1,-1]
- t1-reglages-zoom-dark.png : boîte texte (768,465) 35×13 : forme : 89 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, -1) px (strict) ; boîte d'encre : bord droit +4, bord bas -1 px (borne ±1) [encre +3.8 %, forme 89.2 %, teinte 0.9981, bords +3,-1,+4,-1]
- t1-reglages-zoom-dark.png : boîte texte (276,473) 71×13 : encre totale +29 % (borne ±25 %) ; forme : 131 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +0, bord bas +7 px (borne ±1) [encre +28.8 %, forme 130.8 %, teinte 0.9999, bords +0,+3,+0,+7]
- t1-reglages-zoom-dark.png : boîte texte (276,494) 429×15 : encre totale +47 % (borne ±25 %) ; forme : 167 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +42, bord bas -3 px (borne ±1) [encre +46.6 %, forme 166.7 %, teinte 0.9989, bords +0,+3,+42,-3]
- t1-reglages-zoom-dark.png : boîte icone (938,495) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +15.1 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : 11029 px HORS boîtes (delta max 221, dont 108 de delta ≤ 2)
- t1-reglages-zoom-light.png : divergence HORS boîte en (0,31) 1000×2 px
- t1-reglages-zoom-light.png : divergence HORS boîte en (14,46) 208×7 px
- t1-reglages-zoom-light.png : divergence HORS boîte en (261,52) 715×10 px
- t1-reglages-zoom-light.png : divergence HORS boîte en (822,70) 138×33 px
- t1-reglages-zoom-light.png : divergence HORS boîte en (14,74) 208×7 px
- t1-reglages-zoom-light.png : divergence HORS boîte en (831,82) 1×9 px
- t1-reglages-zoom-light.png : divergence HORS boîte en (70,106) 1×2 px
- t1-reglages-zoom-light.png : divergence HORS boîte en (70,110) 1×4 px
- t1-reglages-zoom-light.png : boîte icone (924,10) 14×14 : encre totale +25 % (borne ±25 %) [encre +25.1 %, forme 0.0 %, teinte 1.0000, bords +0,+0,+0,+0]
- t1-reglages-zoom-light.png : boîte icone (24,56) 14×14 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +10.6 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+1,-1]
- t1-reglages-zoom-light.png : boîte texte (44,56) 103×15 : encre totale +27 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +26.7 %, forme 0.0 %, teinte 0.9997, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (277,70) 124×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +5.9 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+0]
- t1-reglages-zoom-light.png : boîte texte (833,79) 31×15 : origine de la boîte d'encre décalée de (2, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas -1 px (borne ±1) [encre +5.0 %, forme 0.0 %, teinte 0.9988, bords +2,-1,+2,-1]
- t1-reglages-zoom-light.png : boîte icone (940,79) 10×14 : encre totale +40 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +40.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (17,103) 52×12 : forme : 31 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas +0 px (borne ±1) [encre +17.2 %, forme 31.2 %, teinte 0.9999, bords +0,-1,-3,+0]
- t1-reglages-zoom-light.png : boîte icone (18,128) 16×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +11.9 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (41,131) 48×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +8.8 %, forme 0.0 %, teinte 0.9997, bords +0,-1,-1,+0]
- t1-reglages-zoom-light.png : boîte texte (276,148) 53×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +5.9 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+0]
- t1-reglages-zoom-light.png : boîte icone (902,156) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +11.6 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte icone (937,156) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (817,157) 44×13 : origine de la boîte d'encre décalée de (2, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas +0 px (borne ±1) [encre +4.5 %, forme 0.0 %, teinte 0.9994, bords +2,-1,+2,+0]
- t1-reglages-zoom-light.png : boîte icone (868,157) 10×14 : encre totale +40 % (borne ±25 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +40.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte icone (17,160) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +13.7 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (42,162) 37×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +8.5 %, forme 0.0 %, teinte 0.9996, bords +1,-1,+0,+0]
- t1-reglages-zoom-light.png : boîte texte (276,169) 322×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +16.7 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+0]
- t1-reglages-zoom-light.png : boîte icone (17,192) 18×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +9.9 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (42,194) 83×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +8.2 %, forme 0.0 %, teinte 0.9996, bords +1,-1,-1,+0]
- t1-reglages-zoom-light.png : boîte icone (17,223) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -1.1 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (41,226) 75×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +3.3 %, forme 0.0 %, teinte 0.9999, bords +0,-1,-1,-1]
- t1-reglages-zoom-light.png : boîte texte (276,226) 85×13 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +7.6 %, forme 0.0 %, teinte 0.9997, bords +0,-1,+0,+0]
- t1-reglages-zoom-light.png : boîte icone (17,256) 18×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (42,257) 49×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +14.9 %, forme 0.0 %, teinte 0.9995, bords +1,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte icone (17,286) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (41,288) 118×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +12.2 %, forme 0.0 %, teinte 0.9997, bords +0,-1,-1,-1]
- t1-reglages-zoom-light.png : boîte texte (263,300) 35×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +3.6 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+1]
- t1-reglages-zoom-light.png : boîte icone (17,317) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.3 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (42,320) 54×15 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +11.0 %, forme 0.0 %, teinte 0.9992, bords +1,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (277,345) 58×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +5.7 %, forme 0.0 %, teinte 0.9998, bords +0,-1,+0,+1]
- t1-reglages-zoom-light.png : boîte texte (17,359) 85×12 : forme : 51 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) ; boîte d'encre : bord droit -3, bord bas +0 px (borne ±1) [encre +18.3 %, forme 50.5 %, teinte 0.9999, bords +0,-1,-3,+0]
- t1-reglages-zoom-light.png : boîte texte (277,368) 313×14 : origine de la boîte d'encre décalée de (1, 1) px (strict) [encre +15.9 %, forme 0.0 %, teinte 0.9996, bords +1,+1,+0,+1]
- t1-reglages-zoom-light.png : boîte icone (17,385) 18×18 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +10.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-reglages-zoom-light.png : boîte texte (41,388) 37×14 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +14.8 %, forme 0.0 %, teinte 0.9998, bords +0,-1,-1,-1]
- t1-reglages-zoom-light.png : boîte texte (262,420) 45×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +4.2 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,+1]
- t1-reglages-zoom-light.png : boîte texte (768,465) 35×13 : forme : 86 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (3, -1) px (strict) ; boîte d'encre : bord droit +4, bord bas +0 px (borne ±1) [encre +16.5 %, forme 86.5 %, teinte 0.9995, bords +3,-1,+4,+0]
- t1-reglages-zoom-light.png : boîte texte (276,473) 71×13 : encre totale +43 % (borne ±25 %) ; forme : 154 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +0, bord bas +8 px (borne ±1) [encre +42.9 %, forme 153.6 %, teinte 0.9999, bords +0,+3,+0,+8]
- t1-reglages-zoom-light.png : boîte texte (276,494) 429×15 : encre totale +67 % (borne ±25 %) ; forme : 151 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, 3) px (strict) ; boîte d'encre : bord droit +42, bord bas -3 px (borne ±1) [encre +66.5 %, forme 151.1 %, teinte 0.9997, bords +0,+3,+42,-3]
- t1-reglages-zoom-light.png : boîte icone (938,495) 16×16 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +12.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-split-horizontal-dark.png : 3039 px HORS boîtes (delta max 71, dont 2580 de delta ≤ 2)
- t1-split-horizontal-dark.png : divergence HORS boîte en (245,4) 5×6 px
- t1-split-horizontal-dark.png : divergence HORS boîte en (448,4) 5×6 px
- t1-split-horizontal-dark.png : divergence HORS boîte en (242,31) 5×5 px
- t1-split-horizontal-dark.png : divergence HORS boîte en (451,31) 5×5 px
- t1-split-horizontal-dark.png : divergence HORS boîte en (36,399) 1244×2 px
- t1-split-horizontal-dark.png : divergence HORS boîte en (39,405) 5×6 px
- t1-split-horizontal-dark.png : divergence HORS boîte en (242,405) 5×6 px
- t1-split-horizontal-dark.png : divergence HORS boîte en (36,432) 5×5 px
- t1-split-horizontal-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-split-horizontal-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-split-horizontal-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-split-horizontal-dark.png : boîte texte (52,415) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-split-horizontal-light.png : 2333 px HORS boîtes (delta max 57, dont 1853 de delta ≤ 2)
- t1-split-horizontal-light.png : divergence HORS boîte en (242,3) 9×33 px
- t1-split-horizontal-light.png : divergence HORS boîte en (447,3) 9×33 px
- t1-split-horizontal-light.png : divergence HORS boîte en (36,399) 1244×2 px
- t1-split-horizontal-light.png : divergence HORS boîte en (36,405) 8×32 px
- t1-split-horizontal-light.png : divergence HORS boîte en (242,405) 8×32 px
- t1-split-horizontal-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-split-horizontal-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-split-horizontal-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-split-horizontal-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-split-horizontal-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-split-horizontal-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-split-horizontal-light.png : boîte texte (52,415) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-split-proportions-dark.png : 5046 px HORS boîtes (delta max 201, dont 3204 de delta ≤ 2)
- t1-split-proportions-dark.png : divergence HORS boîte en (346,0) 629×800 px
- t1-split-proportions-dark.png : divergence HORS boîte en (1048,3) 10×33 px
- t1-split-proportions-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-split-proportions-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-split-proportions-dark.png : divergence HORS boîte en (557,4) 5×6 px
- t1-split-proportions-dark.png : divergence HORS boîte en (760,4) 5×6 px
- t1-split-proportions-dark.png : divergence HORS boîte en (973,4) 5×6 px
- t1-split-proportions-dark.png : divergence HORS boîte en (1018,22) 2×2 px
- t1-split-proportions-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-split-proportions-dark.png : boîte texte (364,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -10.4 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-split-proportions-dark.png : boîte texte (570,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.5 %, forme 0.0 %, teinte 0.9997, bords +1,+0,+0,+0]
- t1-split-proportions-dark.png : boîte texte (986,14) 31×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) ; boîte d'encre : bord droit -3, bord bas +0 px (borne ±1) [encre -8.7 %, forme 4.7 %, teinte 0.9997, bords +1,+0,-3,+0]
- t1-split-proportions-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-split-proportions-dark.png : boîte texte (364,215) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.5 %, forme 0.0 %, teinte 0.9997, bords +1,+0,+0,+0]
- t1-split-proportions-light.png : 3750 px HORS boîtes (delta max 219, dont 1888 de delta ≤ 2)
- t1-split-proportions-light.png : divergence HORS boîte en (346,0) 633×224 px
- t1-split-proportions-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-split-proportions-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-split-proportions-light.png : divergence HORS boîte en (554,3) 9×33 px
- t1-split-proportions-light.png : divergence HORS boîte en (759,3) 9×33 px
- t1-split-proportions-light.png : divergence HORS boîte en (1048,3) 10×33 px
- t1-split-proportions-light.png : divergence HORS boîte en (1018,22) 2×2 px
- t1-split-proportions-light.png : divergence HORS boîte en (347,205) 9×33 px
- t1-split-proportions-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-split-proportions-light.png : boîte texte (364,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +10.1 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+1,+1]
- t1-split-proportions-light.png : boîte texte (570,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +8.7 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+1,+1]
- t1-split-proportions-light.png : boîte texte (986,14) 31×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) ; boîte d'encre : bord droit -3, bord bas +1 px (borne ±1) [encre +4.9 %, forme 2.6 %, teinte 0.9997, bords +1,+0,-3,+1]
- t1-split-proportions-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-split-proportions-light.png : boîte texte (364,215) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +8.7 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+1,+1]
- t1-split-vertical-dark.png : 2120 px HORS boîtes (delta max 71, dont 1662 de delta ≤ 2)
- t1-split-vertical-dark.png : divergence HORS boîte en (657,0) 7×800 px
- t1-split-vertical-dark.png : divergence HORS boîte en (662,3) 6×7 px
- t1-split-vertical-dark.png : divergence HORS boîte en (864,3) 6×7 px
- t1-split-vertical-dark.png : divergence HORS boîte en (39,4) 5×6 px
- t1-split-vertical-dark.png : divergence HORS boîte en (242,4) 5×6 px
- t1-split-vertical-dark.png : divergence HORS boîte en (36,31) 5×5 px
- t1-split-vertical-dark.png : divergence HORS boîte en (245,31) 5×5 px
- t1-split-vertical-dark.png : divergence HORS boîte en (868,31) 5×5 px
- t1-split-vertical-dark.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.1 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+0]
- t1-split-vertical-dark.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -9.8 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+0]
- t1-split-vertical-dark.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre -5.5 %, forme 0.0 %, teinte 0.9997, bords +1,+0,+0,+0]
- t1-split-vertical-dark.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +15.0 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-split-vertical-light.png : 2233 px HORS boîtes (delta max 57, dont 1751 de delta ≤ 2)
- t1-split-vertical-light.png : divergence HORS boîte en (657,0) 11×800 px
- t1-split-vertical-light.png : divergence HORS boîte en (36,3) 9×33 px
- t1-split-vertical-light.png : divergence HORS boîte en (241,3) 9×33 px
- t1-split-vertical-light.png : divergence HORS boîte en (864,3) 9×33 px
- t1-split-vertical-light.png : divergence HORS boîte en (1178,780) 6×6 px
- t1-split-vertical-light.png : divergence HORS boîte en (1191,786) 49×11 px
- t1-split-vertical-light.png : divergence HORS boîte en (1243,786) 25×9 px
- t1-split-vertical-light.png : boîte texte (52,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +9.9 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+0,+1]
- t1-split-vertical-light.png : boîte texte (258,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +11.0 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+0,+1]
- t1-split-vertical-light.png : boîte texte (675,14) 48×12 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +8.7 %, forme 0.0 %, teinte 0.9998, bords +1,+0,+1,+1]
- t1-split-vertical-light.png : boîte icone (9,48) 18×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -0.1 %, forme 0.0 %, teinte 1.0000, bords +0,+1,+0,+0]
- t1-zoom-150-dark.png : 329 px HORS boîtes (delta max 43, dont 57 de delta ≤ 2)
- t1-zoom-150-dark.png : divergence HORS boîte en (274,6) 13×48 px
- t1-zoom-150-dark.png : divergence HORS boîte en (492,6) 14×48 px
- t1-zoom-150-dark.png : divergence HORS boîte en (719,18) 1×24 px
- t1-zoom-150-dark.png : divergence HORS boîte en (1127,770) 10×30 px
- t1-zoom-150-dark.png : boîte texte (77,22) 71×16 : origine de la boîte d'encre décalée de (-1, 0) px (strict) ; boîte d'encre : bord droit -2, bord bas +0 px (borne ±1) [encre -9.3 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-2,+0]
- t1-zoom-150-dark.png : boîte texte (297,22) 71×16 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre -5.7 %, forme 0.0 %, teinte 0.9998, bords -1,+0,-1,+0]
- t1-zoom-150-dark.png : boîte icone (470,22) 15×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +12.0 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-zoom-150-dark.png : boîte texte (516,22) 71×16 : origine de la boîte d'encre décalée de (-1, 0) px (strict) ; boîte d'encre : bord droit -2, bord bas +0 px (borne ±1) [encre -9.4 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-2,+0]
- t1-zoom-150-dark.png : boîte texte (736,22) 71×16 : origine de la boîte d'encre décalée de (-1, 0) px (strict) ; boîte d'encre : bord droit -2, bord bas +0 px (borne ±1) [encre -9.2 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-2,+0]
- t1-zoom-150-dark.png : boîte icone (14,72) 26×26 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +13.5 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,+0]
- t1-zoom-150-dark.png : boîte icone (13,123) 27×27 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +13.9 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,+0]
- t1-zoom-150-dark.png : boîte icone (15,177) 23×21 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +15.6 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,+0]
- t1-zoom-150-dark.png : boîte texte (1146,778) 116×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre -3.4 %, forme 0.0 %, teinte 1.0000, bords +0,+1,-1,+1]
- t1-zoom-150-light.png : 1561 px HORS boîtes (delta max 39, dont 1297 de delta ≤ 2)
- t1-zoom-150-light.png : divergence HORS boîte en (54,5) 1226×49 px
- t1-zoom-150-light.png : divergence HORS boîte en (719,18) 1×24 px
- t1-zoom-150-light.png : divergence HORS boîte en (1127,770) 10×30 px
- t1-zoom-150-light.png : boîte texte (77,22) 71×16 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +4.2 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,+1]
- t1-zoom-150-light.png : boîte texte (297,22) 71×16 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +3.7 %, forme 0.0 %, teinte 0.9999, bords -1,+0,-1,+1]
- t1-zoom-150-light.png : boîte icone (470,22) 15×15 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +0.9 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-zoom-150-light.png : boîte texte (516,22) 71×16 : origine de la boîte d'encre décalée de (-2, 0) px (strict) ; boîte d'encre : bord droit -2, bord bas +1 px (borne ±1) [encre +4.2 %, forme 0.0 %, teinte 1.0000, bords -2,+0,-2,+1]
- t1-zoom-150-light.png : boîte texte (736,22) 71×16 : origine de la boîte d'encre décalée de (-2, 0) px (strict) ; boîte d'encre : bord droit -2, bord bas +1 px (borne ±1) [encre +4.2 %, forme 0.0 %, teinte 1.0000, bords -2,+0,-2,+1]
- t1-zoom-150-light.png : boîte icone (14,72) 26×26 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +0.5 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,+0]
- t1-zoom-150-light.png : boîte icone (13,123) 27×27 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +2.6 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,+0]
- t1-zoom-150-light.png : boîte icone (15,177) 23×21 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +3.1 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,+0]
- t1-zoom-150-light.png : boîte texte (1146,778) 116×18 : origine de la boîte d'encre décalée de (0, 1) px (strict) [encre +11.7 %, forme 0.0 %, teinte 1.0000, bords +0,+1,-1,+1]
- t1-zoom-80-dark.png : 2636 px HORS boîtes (delta max 99, dont 1246 de delta ≤ 2)
- t1-zoom-80-dark.png : divergence HORS boîte en (29,3) 1251×26 px
- t1-zoom-80-dark.png : divergence HORS boîte en (13,104) 7×1 px
- t1-zoom-80-dark.png : divergence HORS boîte en (1197,784) 6×16 px
- t1-zoom-80-dark.png : boîte texte (525,8) 3×16 : forme : 14 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -1) px (strict) [encre +16.7 %, forme 14.3 %, teinte 1.0000, bords +0,-1,+0,+1]
- t1-zoom-80-dark.png : boîte icone (1151,9) 13×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre +11.9 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+1,-1]
- t1-zoom-80-dark.png : boîte icone (698,10) 11×11 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -5.4 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-zoom-80-dark.png : boîte texte (40,11) 40×9 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre -15.6 %, forme 0.0 %, teinte 1.0000, bords -1,+0,-1,-1]
- t1-zoom-80-dark.png : boîte texte (346,11) 9×9 : encre totale +28 % (borne ±25 %) ; origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +27.9 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+1,+0]
- t1-zoom-80-dark.png : boîte texte (1128,12) 11×7 : encre totale +27 % (borne ±25 %) ; origine de la boîte d'encre décalée de (1, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas -1 px (borne ±1) [encre +26.6 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+2,-1]
- t1-zoom-80-dark.png : boîte icone (6,63) 16×16 : forme : 53 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -2) px (strict) ; boîte d'encre : bord droit +0, bord bas -2 px (borne ±1) [encre +17.1 %, forme 53.1 %, teinte 1.0000, bords +0,-2,+0,-2]
- t1-zoom-80-dark.png : boîte icone (8,91) 12×12 : forme : 90 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -3) px (strict) ; boîte d'encre : bord droit -1, bord bas -3 px (borne ±1) [encre +24.1 %, forme 90.0 %, teinte 1.0000, bords +0,-3,-1,-3]
- t1-zoom-80-dark.png : boîte texte (1207,788) 63×11 : origine de la boîte d'encre décalée de (-1, 1) px (strict) ; boîte d'encre : bord droit -2, bord bas +1 px (borne ±1) [encre -2.3 %, forme 0.0 %, teinte 1.0000, bords -1,+1,-2,+1]
- t1-zoom-80-light.png : 2635 px HORS boîtes (delta max 102, dont 2345 de delta ≤ 2)
- t1-zoom-80-light.png : divergence HORS boîte en (29,2) 1251×27 px
- t1-zoom-80-light.png : divergence HORS boîte en (526,9) 1×1 px
- t1-zoom-80-light.png : divergence HORS boîte en (526,22) 1×1 px
- t1-zoom-80-light.png : divergence HORS boîte en (13,104) 7×1 px
- t1-zoom-80-light.png : divergence HORS boîte en (1197,784) 5×16 px
- t1-zoom-80-light.png : boîte icone (1151,9) 13×13 : origine de la boîte d'encre décalée de (1, -1) px (strict) [encre -0.7 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+1,-1]
- t1-zoom-80-light.png : boîte icone (698,10) 11×11 : origine de la boîte d'encre décalée de (0, -1) px (strict) [encre -7.1 %, forme 0.0 %, teinte 1.0000, bords +0,-1,+0,-1]
- t1-zoom-80-light.png : boîte texte (40,11) 40×9 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +3.0 %, forme 0.0 %, teinte 1.0000, bords -1,+0,+0,+0]
- t1-zoom-80-light.png : boîte texte (205,11) 40×9 : origine de la boîte d'encre décalée de (-1, 0) px (strict) [encre +1.5 %, forme 0.0 %, teinte 0.9998, bords -1,+0,+0,+0]
- t1-zoom-80-light.png : boîte texte (346,11) 9×9 : origine de la boîte d'encre décalée de (1, 0) px (strict) [encre +6.3 %, forme 0.0 %, teinte 1.0000, bords +1,+0,+1,+0]
- t1-zoom-80-light.png : boîte texte (1128,12) 11×7 : origine de la boîte d'encre décalée de (1, -1) px (strict) ; boîte d'encre : bord droit +2, bord bas -1 px (borne ±1) [encre +11.1 %, forme 0.0 %, teinte 1.0000, bords +1,-1,+2,-1]
- t1-zoom-80-light.png : boîte icone (6,63) 16×16 : forme : 83 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -2) px (strict) ; boîte d'encre : bord droit +0, bord bas -2 px (borne ±1) [encre -2.0 %, forme 83.3 %, teinte 1.0000, bords +0,-2,+0,-2]
- t1-zoom-80-light.png : boîte icone (8,91) 12×12 : forme : 75 % des traits sans équivalent dans un mot (borne 5 %) ; origine de la boîte d'encre décalée de (0, -3) px (strict) ; boîte d'encre : bord droit -1, bord bas -3 px (borne ±1) [encre -0.7 %, forme 75.0 %, teinte 1.0000, bords +0,-3,-1,-3]
- t1-zoom-80-light.png : boîte texte (1207,788) 63×11 : encre totale +26 % (borne ±25 %) ; origine de la boîte d'encre décalée de (-1, 1) px (strict) ; boîte d'encre : bord droit -2, bord bas +1 px (borne ±1) [encre +25.8 %, forme 0.0 %, teinte 1.0000, bords -1,+1,-2,+1]

Ombres (distance au bord et encre, candidat moins référence) :

- t1-menu-onglet-dark.png : surface 0 (555,20) 190×40 : haut +0.08 px -0.8 %, bas -0.09 px -0.8 %, gauche -0.07 px +1.1 %, droite -0.09 px +1.4 %
- t1-menu-onglet-light.png : surface 0 (555,20) 190×40 ÉCHEC : haut -0.06 px +0.1 %, bas -0.29 px -4.8 %, gauche -0.05 px -0.2 %, droite +0.01 px -1.3 %
<!-- sas:fin -->` (en fin de fichier, ajoutée si les marqueurs manquent) : le reste de ce fichier, écrit à la main, n'est jamais touché. Une ligne par PNG comparé :

| Colonne | Sens |
|---|---|
| Verdict | `Strict` (0 pixel), `Tolere` (écarts seulement dans les boîtes, garde-fous tenus, cliquet tenu), `Echec` |
| Boîtes (texte/icônes) | nombre de boîtes déclarées par le candidat, par catégorie |
| Hors boîtes (px) | pixels divergents hors de toute boîte : doit être 0 |
| Dans les boîtes (px), % | pixels divergents dans l'union des boîtes (élargies de 1 px), et part de leur aire |
| Delta max | plus fort écart de canal dans les boîtes |
| Largeur ±1 | boîtes dont le bord droit d'encre diffère d'un pixel : la dette T1 (`fr-t1.3`) |
| Ombres | surfaces ombrées déclarées par le candidat (boîte de bordure, rayon, couches), version 2 de `.boites.json` |
| Halos (px tolérés), % halos, Delta ombre | pixels de delta ≤ 2 dans les halos, leur part de l'aire des halos, leur plus fort delta (cliquet `ombre_delta_max`, `ombre_pourcent`) |
| Cliquet | `ok`, `réécrit` (sous `--maj-cliquet`) ou la raison de l'aggravation |

Sous le tableau, « Échecs » liste, pour chaque scène en échec : les pixels hors boîtes (dont combien de delta ≤ 2), les zones, et pour chaque boîte fautive son rectangle, le garde-fou violé et ses chiffres (`encre`, `forme`, `teinte`, écart des quatre bords d'encre). Le candidat écrit `<scène>-<thème>.boites.json` à côté de chaque PNG ; sans ce fichier `xtask` déduit les boîtes de l'image. Les diffs `pixels/diff/*.png` montrent en orange l'écart toléré dans une boîte, en bleu celui toléré dans un halo d'ombre et en rouge le reste. Une section « Ombres » donne, pour chaque surface, l'écart (candidat moins référence) de distance au bord et d'encre de chacun de ses côtés droits.

<!-- sas:debut -->
(Section générée par le job `pixels`.)
<!-- sas:fin -->
