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

## Dettes de parité : surfaces flottantes T1 (fr-t1.1)
Scènes `t1-{notice,notice-empilees,menu,infobulle,toolbar-horizontale,toolbar-sections}`, toujours en `"depuis": "T2"`.
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

## Format du rapport du sas (ADR-009)
`cargo xtask pixels` écrit sa section entre `<!-- sas:debut -->` et `<!-- sas:fin -->` (en fin de fichier, ajoutée si les marqueurs manquent) : le reste de ce fichier, écrit à la main, n'est jamais touché. Une ligne par PNG comparé :

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
