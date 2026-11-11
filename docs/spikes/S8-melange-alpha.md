# S8 — Le mélange alpha de GPUI contre Chromium

Tâche `fr-p4.1`. Code promu : `crates/fragment-ui/src/rendu/melange.rs`. Décision proposée : `docs/decisions/ADR-013-ombres-chromium.md`.

## Hypothèse de départ, réfutée

« GPUI mélange en linéaire, Chromium en sRGB. » C'est faux.
- GPUI (`gpui-pre-windows 0.3.10`) peint dans une cible `DXGI_FORMAT_B8G8R8A8_UNORM`, pas `_SRGB`. Le mélange est `SRC_ALPHA, INV_SRC_ALPHA`, sur des canaux sRGB encodés. `hsla_to_rgba` ne linéarise rien (seuls les dégradés Oklab le font).
- Skia compose aussi en sRGB encodé, dans ses tuiles RGBA8.
- **Preuve** : une simulation en `f32` du shader d'ombre de GPUI (erf approchée, 4 échantillons en y, `gaussian(y, blur_radius)`, arrondi au plus proche) reproduit le candidat au bit près. Sur la modale sombre, 100 % des pixels du halo sont identiques ; sur la modale claire, 97,9 %, les autres à 1 niveau (aller-retour HSLA).

Les écarts viennent de ce qui est arrondi, de quand, et de la forme du flou. Il y a quatre causes, séparées par la mesure.

## Méthode

- **Mire Chromium** : l'Electron 43 de `Fragment/app/node_modules`, piloté par son Playwright. Un `main.js` jetable charge des pages `data:`, `--force-device-scale-factor=1`, et la capture se fait par `page.screenshot`, comme `pixels-reference.ps1`. `userData` va dans le dossier de travail, sous le verrou `C:/dev/fragment-rs-pixels-verrou`. Fragment n'est pas modifié : `git status --porcelain` est identique avant et après.
- **Mires** :
  - les géométries des scènes (modale 610 × 102, r 12 ; palette 702 × 168 ; menu 190 × 205, r 8 ; infobulle 63 × 26, r 6 ; notices), avec des ombres NOIRES opaques sur du blanc, pour lire la couverture du flou de Skia sur 255 niveaux ;
  - les mêmes ombres dans la structure exacte des scènes (voile, conteneur `position: fixed`) ;
  - des coins arrondis avec une bordure opaque de 1 et 2 px, avec une bordure translucide, et sans bordure.
- **Simulation du candidat** : le halo est repeint en Python selon chaque modèle, sur le PNG candidat du run 43018474495, puis `cargo xtask pixels` de `main` mesure les garde-fous (étalement, encre, glissement). C'est le vrai juge, sans build local de l'app.

## Les causes, chiffrées

### 1. Le calque composé de Chromium (`--shadow-l`, cause principale)

`.modal-container` est en `position: fixed` : Chromium en fait un **calque composé**. Le voile (`rgba(0,0,0,.5)`, octet 128) et l'ombre sont rastérisés ensemble dans une texture RGBA8 PRÉMULTIPLIÉE, quantifiée à l'octet. Cette texture est ensuite composée sur l'espace de travail : `L + fond × (1 − A)`. GPUI, lui, peint le voile, l'arrondit, puis peint l'ombre par-dessus. L'arrondi intermédiaire n'est donc pas le même.
- **Preuve** : la même ombre posée sur un fond plat `rgb(10,10,11)` donne 85,3 % de pixels égaux à la référence. Dans un conteneur `fixed` avec le voile, sur `#141517`, elle en donne **100 %**, en sombre comme en clair.
- **Modèle** : calque `A = q(a_voile + a_ombre (1 − a_voile))`, couleur prémultipliée `q(c × a_ombre)`, puis composition. Avec la couverture mesurée sur la mire, il reproduit la référence sombre à 100 % et la claire à 96,8 % (le reste à 1 niveau).
- **Effet sur les garde-fous** (simulation, couverture de Skia) : l'étalement de la modale passe de +1,60 à −0,01 px en sombre et de −0,60 à −0,01 px en clair. Pour la palette : +1,42 → −0,01 en sombre, −0,58 → +0,01 en clair. Encre et glissement sont à 0 %. **L'écart de signe entre les thèmes vient de là.**

### 2. La forme du flou de Skia

Sur les mires noir sur blanc, voici l'écart à la couverture de Chromium, en niveaux sur 255, dans la zone d'ombre hors boîte.

| Surface | GPUI (shader) | Gaussienne exacte (σ = flou/2) |
|---|---|---|
| modale 610×102 r12 σ24 | max 6, 57 % égaux | max 4, 61 % |
| palette 702×168 r12 σ24 | max 4, 63 % | **max 1**, 83 % |
| menu sombre σ12 | max 6, 64 % | max 3, 59 % |
| menu clair σ10 | max 4, 63 % | **max 1**, 74 % |
| infobulle 63×26 σ12 | max 3, 61 % | max 3, 61 % |
| notice 132×39 σ4 | max 4, 83 % | **max 1**, 82 % |
| rectangle sans rayon, σ24 | max 3, 94 % | **max 1**, 99,1 % |

- Pour un rectangle (analytique), Skia est une gaussienne d'écart-type flou/2, au niveau près.
- Pour un rectangle arrondi, Skia s'en écarte selon la taille. La modale (102 px de haut, σ24) et l'infobulle sont trop petites pour la neuvaine de `GrRRectBlurEffect` : elles passent par le flou générique du GPU, sous-échantillonné, de 2 à 3 niveaux plus clair sur les côtés. Le triple flou en boîte du chemin CPU (`SkMaskBlurFilter`, fenêtre ⌊σ·3√(2π)/4 + ½⌋) a été essayé : il est plus loin que la gaussienne (54 % de pixels égaux sur la modale).
- **Simulation avec la gaussienne exacte et le calque** : modale +0,11 px en sombre et +0,20 en clair ; palette +0,01 en sombre et +0,20 en clair. C'est mieux que +1,6 / −0,6, mais au-dessus de la borne de 0,08.
- **Avec la couverture de Skia et le calque** : 0,01 partout. Ce qui reste, c'est donc la seule forme du flou de Skia sur GPU. Elle dépend du chemin choisi (neuvaine, rectangle analytique, flou sous-échantillonné) et du filtrage bilinéaire du GPU de la référence (le portable), qui n'est pas celui de la CI (WARP).

### 3. La bordure peinte deux fois au bord (arcs de la modale et de la palette claires)

- GPUI (`Style::paint`) peint le quad du fond, puis les enfants, puis un SECOND quad pour la bordure. Les deux sont anticrénelés au bord extérieur : le pixel de bord reçoit deux fois la couverture.
- Chromium, devant une bordure opaque, évite ce débordement (*background bleed avoidance*) : le bord extérieur ne reçoit que la bordure.
- **Mire**, bordure opaque de 1 px, r 12 : le modèle « double » de GPUI s'écarte de Chromium de 31 niveaux au pire (8,9 en moyenne). « Une seule couverture » s'en écarte de 8 au pire, 0,28 en moyenne, sur 6 pixels par coin à la jonction de l'arc et du côté droit. Avec 2 px de bordure, l'écart est de 1 au pire.
- C'est la cause des « 60 px de delta 7 à 33 » de la modale et de la palette claires.
- **Correction possible, côté `fragment-ui`** : peindre le fond de la boîte de la couleur de la bordure, puis le vrai fond dans un enfant en retrait de la bordure (rayon `r − bordure`). Pas faite ici, voir l'ADR.

### 4. L'opacité ramenée à l'octet (bordures translucides)

- Un jeton `color-mix(in srgb, X 10 %, transparent)` est rangé en `Couleur` 8 bits (opacité 26/255). Chromium garde l'opacité exacte (0,1) : 232,5 → 233 sur le blanc du menu clair, quand GPUI donne 232.
- **Corrigé ici** : `rendu::melange::opacite_exacte` passe à GPUI l'opacité du pourcentage, en flottant, avec un biais d'un cent-millième pour que la demie monte comme chez Chromium. Le test unitaire refait le chemin de GPUI en `f32` (`Rgba` → `Hsla`, `hsla_to_rgba` du shader, mélange, octet) et retombe sur les trois relevés de fr-t1.1.
- Le contournement `flottant::bordure_translucide`, qui déplaçait la teinte, est retiré (menu, notice, infobulle).

### Ombres hors exception

- **Infobulle** (+0,22 px) : 4 échantillons en y du shader de GPUI, sur une boîte de 26 px de haut. Avec la couverture de Skia, on tombe à +0,04 en sombre et +0,16 en clair.
- **Menu clair et notices claires** : couleur d'ombre `(17,18,20)` et calque des surfaces fixes. Avec la couverture de Skia et une composition non calquée, on obtient +0,16 (haut du menu) et +0,08 (notices).

## Verdict

**GO-AVEC.**
- La cause de `--shadow-l` est trouvée et prouvée à 100 % des pixels : c'est le calque composé de Chromium, pas l'espace de mélange.
- Son émulation est à portée de `fragment-ui`, sans fork : une image d'ombre rastérisée par le CPU et peinte par le voile, avant la boîte. Elle ramène l'étalement de 1,6 px à 0,1 à 0,2 px.
- Le zéro exige en plus la couverture exacte du flou de Skia sur GPU. **Elle est hors d'atteinte au bit près** : elle dépend du chemin de Skia et du GPU de la référence. Approchée par une gaussienne exacte, il reste 1 niveau sur 255 au pire pour la palette, le menu et les notices, mais 4 pour la modale.
- Les arcs clairs de la modale et de la palette ont une cause locale (bordure peinte deux fois), corrigeable dans `fragment-ui`.

## Repli et suite

Voir l'ADR-013 :
- émulation du calque et ombres rastérisées par le CPU (gaussienne exacte) ;
- fond en retrait sous une bordure opaque ;
- l'exception `--shadow-l` ramenée à ses nouvelles valeurs en cliquet ;
- le résidu de forme de Skia accepté en dette chiffrée, ou réduit par une table de couverture par géométrie.

## Reprise (fr-p4.1, 11/11 après-midi) : bord unique et calque émulé

### Bord unique (causes 3 et 4 ensemble)

- `rendu::melange::bord_unique` : la boîte est peinte de l'aplat OPAQUE de sa bordure, sans quad de bordure (largeur gardée pour la mise en page, couleur absente) ; le fond est un enfant absolu en retrait, de rayon `r − bordure`. Un seul anticrénelage porte le bord extérieur.
- Bordure translucide : l'aplat est `fond + p × (teinte − fond)` en flottant, la demie vers le haut (`aplat_exact`, 232,5 → 233). `Palette::aplat` suit le chemin prémultiplié des remplissages et donne 232 : il ne convient pas aux bordures.
- ★ Arc intérieur d'une bordure translucide : sur la mire `a-transl` (1 px à 10 %, r 8), le modèle à `r − 1` reste à 4 niveaux de Chromium à l'intérieur de l'arc (bordure trop fine sur l'arc), le modèle « aire exacte » aussi. Le meilleur rayon peint par GPUI est `r − 1 + 0,4` : 2 niveaux au pire, somme signée nulle (0,3 à 0,5 : ≤ 3). Devant une bordure opaque (mires `a-opaque`, 1 et 2 px), le meilleur reste `r − bordure`.
- CI (run 43037958177) : arcs du menu, des notices et de l'infobulle claires de 0,23–0,35 px à 0,07–0,11 px (borne 0,15), delta d'arrondi ≤ 5 ; modale et palette claires de 0,20 à 0,02 px, delta max de 33 à 7–8, mais 20 à 22 px de delta 7–8 à la jonction de l'arc et du côté droit (la mire en prévoyait 6 par coin : l'anticrénelage propre à Skia).
### Calque émulé (cause 1)

- `rendu::ombre` : la modale et la palette déclarent leur boîte au prépeint (`ombrer_au_calque`, qui garde la déclaration pour `fragment-pixels` et retire le `BoxShadow`). Le voile (`voile`) peint, à sa peinture, une image calculée sur le CPU : gaussienne exacte, noyau intégré par pixel, voile et ombre quantifiés en RGBA8 prémultiplié comme le calque, puis rendus en alpha droit pour le mélange `SRC_ALPHA` des images de GPUI. Autour de l'image, le voile seul.
- CI (run 43040649423), étalement de `--shadow-l` : sombre +1,60 → +0,09 px (modale), +1,42 → 0,00 (palettes) ; clair −0,60 → +0,18 / +0,21. La simulation de S8 l'annonçait (0,11 / 0,20). Le reste est la forme du flou de Skia, dette (b) d'ADR-013.
- Coût : 0,33 s en debug pour une modale (758 × 250 px), une fois par géométrie, puis lu dans le cache. Le flou se fait par formes de rangées (le masque d'un rectangle arrondi n'a qu'une vingtaine de rangées distinctes), au lieu de deux noyaux de 145 sur toute l'image (2,2 s).