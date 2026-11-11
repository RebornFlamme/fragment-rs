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
  du texte : environ 1 % de plus côté GPUI, plus l'arrondi vers le haut des feuilles mesurées (+1 px sur
  l'infobulle, +2 px sur la carte de notice).
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
  (40, 41, 43) côté Fragment, sur 688 × 35 px : arrondi de composition, malgré la composition à l'octet (`Couleur::sur`).
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
| Cliquet | `ok`, `réécrit` (sous `--maj-cliquet`) ou la raison de l'aggravation |

Sous le tableau, « Échecs » liste, pour chaque scène en échec : les pixels hors boîtes (dont combien de delta ≤ 2), les zones, et pour chaque boîte fautive son rectangle, le garde-fou violé et ses chiffres (`encre`, `forme`, `teinte`, écart des quatre bords d'encre). Le candidat écrit `<scène>-<thème>.boites.json` à côté de chaque PNG ; sans ce fichier `xtask` déduit les boîtes de l'image. Les diffs `pixels/diff/*.png` montrent en orange l'écart toléré dans une boîte et en rouge celui hors boîte.

<!-- sas:debut -->
(Section générée par le job `pixels`.)
<!-- sas:fin -->
