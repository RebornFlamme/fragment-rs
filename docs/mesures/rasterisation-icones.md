# Rastérisation des icônes SVG : GPUI/resvg contre Chromium/Skia (fr-p4.1.5, 13/11/2026)

Question (fr-t2-montage, #102) : l'icône (165,315) du menu fichier de l'explorateur (T2) diffère d'un
pixel d'anticrénelage (delta 16/14) et gagne (sombre) ou perd (clair) une colonne d'encre. Une
exception nommée « anticrénelage SVG d'icône (rastériseur) » la couvre. L'écart est-il systématique
sur toutes les icônes Lucide des scènes ? Un paramètre exposé de GPUI peut-il le corriger sans fork ?

**Réponse courte.** La géométrie est la même des deux côtés : barycentre à ±0,1 px, boîte d'encre
identique sur 91 % des icônes. L'écart a deux parts.

1. **Une part systématique : le shader des sprites monochromes de GPUI.** Il applique aux masques
   d'icônes la correction gamma et contraste de DirectWrite, faite pour le texte (gamma 1,8,
   contraste gris 1,0, défauts du système). Cette correction dépend de la luminosité de la couleur
   RVB du jeton, sans tenir compte de l'opacité de l'élément. En sombre, l'encre gagne +15 %. En
   clair, elle est juste à 1 % près, mais sa courbe est déformée. Le modèle du shader reproduit les
   masses à 0,005 près dans les deux thèmes. **Aucun paramètre exposé ne la règle.**
2. **Une part non systématique : le rastériseur.** GPUI rastérise avec tiny-skia en 2x, puis
   l'échantillonnage bilinéaire fait une boîte 2x2. Skia, sur le GPU de la référence, donne une
   couverture d'aire exacte. Même sans la correction, il reste un delta max médian de 20 niveaux par
   icône. Les bords d'encre ne changent pas. C'est cette part qui fait l'exception de T2.

Recommandation : **garder l'exception** (sa cause est la part 2, sans paramètre exposé) et ne pas
faire le contournement sans fork de la part 1 (chiffré au § 5). Un correctif complet demande un
fork : à remonter.

## 1. Méthode

- Références : `pixels/reference/` (main a7dca46, captures du 10/11). Candidats : artefact `pixels`
  du run 43425070041 (main 0d5318b, `pixels/candidate/`). Boîtes : les `*.boites.json` du candidat,
  catégorie `icone` (sprites de la scène GPUI).
- 949 boîtes d'icône dans 65 PNG. On en mesure **906** (453 par thème, 31 PNG par thème, 131
  positions distinctes). Les 43 autres ont un fond non uni ou différent d'un côté à l'autre.
- Par boîte, élargie de 2 px : le fond est la médiane de l'anneau extérieur. L'encre est le pixel le
  plus éloigné du fond, commun aux deux images. La couverture α est la projection sur l'axe fond →
  encre, la même pour la référence et le candidat. On compare la masse (Σα cand / Σα réf), le
  barycentre, le delta max, et la boîte d'encre au seuil du sas (`SEUIL_ENCRE` 0,10, L1 normé par
  le contraste de la référence).
- ★ **La référence donne une couverture d'aire exacte.** À 15 px, l'échelle Lucide vaut 0,625 : les
  bords droits tombent sur des huitièmes de pixel. 75 % des pixels de bord de la référence sont sur
  16 valeurs, presque toutes des k/8. Le candidat n'en a que 40 % : la correction déplace ces pics.
  Ces pixels en k/8 servent d'étalon pour lire la courbe de transfert du candidat (§ 3).
- Scripts hors dépôt (Python, numpy et Pillow), dans le dossier de scratch de la session :
  `icones.py` (mesure), `analyse.py`, `huitiemes.py`, `fit3.py`, `residu.py`, `bords.py`. Une sonde
  Rust qui rastérisait l'icône de T2 par tiny-skia (1x, 2x+boîte, 8x) n'a pas été compilée : la
  permission de compiler hors dépôt a été refusée, comme en fr-p4.1.3.

## 2. Chiffres globaux

| | sombre | clair |
|---|---|---|
| icônes mesurées | 453 | 453 |
| identiques au pixel | 24 | 24 |
| delta max par icône (méd. / p90 / max) | 30 / 41 / 76 | 20 / 30 / 68 |
| delta moyen sur les pixels teintés (méd.) | 12,3 | 7,2 |
| encre cand/réf (méd. ; p10 à p90) | **1,150** (1,07 à 1,20) | **1,011** (0,93 à 1,10) |
| barycentre dx, dy (méd. ; p10 à p90) | 0,00 (−0,07 à +0,09) ; 0,00 (−0,06 à +0,13) | 0,00 (−0,06 à +0,08) ; 0,00 (−0,05 à +0,06) |
| boîte d'encre identique (seuil 0,10) | 411 (91 %) | 411 (91 %) |
| écarts de bord | hauteur −1 : 30, +1 : 7 ; largeur ±1 : 5 | hauteur −1 : 30, +1 : 7 ; largeur ±1 : 5 |

Les 24 icônes identiques sont les glyphes peints en quads (boutons de fenêtre, `boites.rs`), pas des
SVG. **Sur les SVG, l'écart touche toutes les icônes.** Le signe de l'encre suit le thème. La
géométrie, elle, ne bouge pas : barycentre nul, mêmes écarts de bord dans les deux thèmes. Les écarts
de bord tiennent à l'icône, pas au thème. Par exemple, (9,48) perd une rangée dans les 23 scènes du
rail, dans les deux thèmes.

## 3. Cause 1 : la correction du shader des sprites monochromes (systématique)

Source : `gpui-pre-0.3.10/src/window.rs::paint_svg` et `svg_renderer.rs`, `gpui-pre-windows-0.3.10/src/shaders.hlsl`
(`monochrome_sprite_fragment`) et `alpha_correction.hlsl`, `directx_renderer.rs::get_font_info`.

- `svg()` peint un `MonochromeSprite`. Son fragment appelle `apply_contrast_and_gamma_correction(
  sample, color.rgb, grayscale_enhanced_contrast, gamma_ratios)`. C'est la même fonction que pour les
  glyphes gris (code repris de Windows Terminal).
- Les paramètres viennent de `IDWriteFactory::CreateRenderingParams()`, lu une seule fois
  (`OnceLock`). Sans `Avalon.Graphics\DISPLAY1`, ni sur le runner ni sur le portable (fr-p4.1.3 § 4),
  ce sont les défauts : gamma 1,8, contraste gris 1,0.
- ★ La luminosité vient de `color.rgb`, la couleur du jeton. L'opacité de l'élément ne passe que
  dans `color.a` (`color.opacity(element_opacity)`). Or les icônes de Fragment sont `--text-normal`
  sous opacité : 0,7 dans les menus (`menu.rs`), 0,5 ailleurs. Le shader voit donc une luminosité
  de 0,87 en sombre (#dfded9) et de 0,105 en clair (#1a1b1e), alors que l'encre visible est grise.
  - En sombre, le terme « clair sur sombre » coupe le contraste. Il ne reste que la correction gamma,
    qui épaissit fortement : +15 %.
  - En clair, le contraste plein (+) et la correction gamma (−) se compensent presque.
- Chromium ne fait rien de tel pour un chemin SVG : couverture linéaire, composée en sRGB (ADR-013).

Vérification : transfert mesuré α_réf (pixels en k/8) → α_cand moyen, contre le modèle du shader
(contraste 1,0, gamma 1,8, luminosité du jeton) :

| α_réf | 1/8 | 2/8 | 3/8 | 4/8 | 5/8 | 6/8 | 7/8 |
|---|---|---|---|---|---|---|---|
| sombre mesuré | 0,205 | 0,437 | 0,508 | 0,626 | 0,706 | 0,839 | 0,907 |
| sombre modèle (b = 0,87) | 0,219 | 0,394 | 0,533 | 0,644 | 0,738 | 0,822 | 0,907 |
| clair mesuré | 0,160 | 0,314 | 0,388 | 0,511 | 0,600 | 0,764 | 0,869 |
| clair modèle (b = 0,105) | 0,159 | 0,275 | 0,385 | 0,499 | 0,619 | 0,744 | 0,872 |

Masse prédite : 1,147 en sombre pour 1,146 mesuré, 1,009 en clair pour 1,014. Prendre la
luminosité de l'encre visible (0,54 en sombre, 0,44 en clair) ne colle pas en clair (1,135). C'est
l'indice que le shader lit le jeton, pas la couleur composée.

## 4. Cause 2 : tiny-skia en 2x et boîte 2x2 contre Skia (non systématique)

- GPUI rastérise le masque à `ceil(2 × côté)` (`SMOOTH_SVG_SCALE_FACTOR`), avec resvg 0.46 et
  tiny-skia 0.11 (anticrénelage par sur-échantillonnage). Il le dessine ensuite dans des bornes 1x,
  calées au pixel, avec un échantillonneur `MIN_MAG_MIP_LINEAR`. Chaque pixel moyenne donc 2×2
  texels.
- On inverse la correction du § 3 sur le candidat. Ce qui reste est la part du rastériseur :

| | sombre | clair |
|---|---|---|
| delta max par icône, actuel (méd. / p90 / max) | 29,7 / 39,7 / 74,7 | 19,3 / 29,7 / 57,0 |
| delta max sans la correction du shader | **19,9 / 31,0 / 60,2** | **19,2 / 30,3 / 59,2** |
| encre cand/réf sans la correction (méd.) | **1,00** | **1,00** |
| icônes à delta ≤ 8 sans la correction | 29 / 453 | 28 / 453 |
| boîtes d'encre identiques, actuel → sans correction | 411 → 410 | 411 → 411 |

- L'écart résiduel moyen vaut 0,065 de couverture. Il est non signé (−0,005 en moyenne), et aussi
  fort sur les bords droits en k/8 que sur les courbes. Il ne dépend presque pas du décalage de
  barycentre (corrélation 0,35 en sombre, 0,23 en clair ; 0,060 de résidu à décalage nul).
  - Ce n'est donc ni un demi-pixel ni une échelle. C'est la façon de couvrir les pixels, qui diffère
    d'un rastériseur à l'autre.
- **L'icône de T2** (`separator-vertical`, 15 px, menu à 0,7) : la colonne 166, pointe du chevron
  gauche, vaut 0 dans la référence. Le candidat y met 0,06 avant correction (part 2). Le shader porte
  cette valeur à 0,12 en sombre (au-dessus du seuil 0,10 : +1 colonne) et à 0,09 en clair (en
  dessous ; la colonne 178 tombe aussi : −1). Sans la correction, les deux thèmes perdraient la
  colonne 178. **L'exception ne tomberait donc pas.** Elle changerait de signe en sombre.

## 5. Paramètres exposés et chiffrage

| levier | exposé ? | effet mesuré ou attendu |
|---|---|---|
| gamma et contraste de DirectWrite | non (`OnceLock` sur le système) | — |
| clé `Avalon.Graphics\DISPLAY1` du runner (gamma 1,0, contraste 0) | réglage système, pas l'app | annule la part 1 en CI seulement. Change aussi le texte gris, juste à 1 % aujourd'hui (fr-p4.1.3). Mesurerait un Fragment que l'utilisateur ne voit pas. **Écarté** |
| couleur pré-composée (jeton mélangé au fond) au lieu de l'opacité | oui | change la luminosité vue par le shader, sans supprimer la correction (`fit3.py` : encre 1,156 en sombre, 1,135 en clair). **Sans effet utile** |
| `svg().with_transformation(…)` (échelle, demi-pixel) | oui | transforme le sprite déjà rastérisé (affine dans le vertex shader), ne change pas la couverture ; barycentre déjà juste. **Sans effet** |
| sprite polychrome : `img(ImageSource::Render(…))`, SVG colorié à la volée et rastérisé par `cx.svg_renderer().render_single_frame` | oui | pas de correction dans `polychrome_sprite_fragment` : encre 1,00. Delta max médian 30 → 20 en sombre, inchangé en clair. Bords d'encre inchangés. **L'exception reste** |
| rastérisation 1x ou 8x par resvg dans `fragment-ui` | oui (dépendance `resvg` à ajouter) | non mesuré (sonde refusée). tiny-skia sur-échantillonne : il n'atteint pas l'aire exacte de Skia, donc le strict 0 pixel n'est pas garanti |

Chiffrage du contournement polychrome, sans fork :
- un élément `icone(nom, jeton, côté)` dans `fragment-ui`, avec un cache (nom, couleur sRGB 8 bits,
  côté, échelle) de `RenderImage`, vidé au changement de thème. L'opacité passe par l'élément ;
  `currentColor` est remplacé dans les octets du SVG. Environ 120 lignes.
- les 69 appels `svg()` de 31 fichiers à migrer (plusieurs équipiers), et un atlas polychrome plus
  chargé (une entrée par couleur).
- gain : la part systématique seulement (encre en sombre). L'exception de T2 n'est pas levée, et les
  icônes restent hors du strict.

**Le coût dépasse le gain : non proposé dans cette PR.**

Pour le 0 pixel, il faudrait un fork :
- `gpui-pre-windows` : un drapeau qui saute `apply_contrast_and_gamma_correction` pour les sprites
  SVG (environ 10 lignes dans `shaders.hlsl` et `MonochromeSprite`) ;
- **et** un rastériseur d'aire exacte comme celui de Skia. tiny-skia n'en a pas : il faudrait
  `skia-safe` (lourd) ou un rastériseur analytique maison.

C'est à remonter à l'orchestrateur. D'ici là, l'exception nommée reste en cliquet, à ±1 px de
largeur, sur son seul PNG. Les icônes restent sous le garde-fou d'ADR-009 (même règle que les
glyphes).

## 6. Chiffres par icône

Une ligne par position distincte, avec la première scène où elle paraît. PNG/thème = nombre de PNG
où elle est mesurée. Encre = Σα cand / Σα réf (médiane). Δ boîte d'encre au seuil 0,10 du sas, pour
le premier PNG. S = sombre, C = clair.

| première scène | x,y | sprite | PNG/thème | delta max S / C | encre cand/réf S / C | Δ largeur,hauteur d'encre S / C |
|---|---|---|---|---|---|---|
| t1-activation-fermeture | 9,9 | 18×18 | 23 | 29 / 19 | 1,18 / 1,01 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 1158,10 | 14×14 | 22 | 0 / 0 | 1,00 / 1,00 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 1204,10 | 14×14 | 22 | 34 / 16 | 1,26 / 1,04 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 1250,10 | 14×14 | 22 | 41 / 27 | 1,07 / 0,91 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 665,12 | 16×16 | 1 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 1088,12 | 16×16 | 21 | 38 / 18 | 1,19 / 1,01 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 1118,12 | 16×16 | 21 | 20 / 20 | 1,11 / 0,94 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 430,13 | 14×14 | 7 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 9,48 | 18×18 | 23 | 36 / 21 | 1,15 / 1,00 | +0,-1 / +0,-1 |
| t1-activation-fermeture | 9,82 | 18×18 | 23 | 37 / 20 | 1,13 / 0,99 | +0,+0 / +0,+0 |
| t1-activation-fermeture | 9,116 | 18×18 | 23 | 24 / 17 | 1,16 / 1,00 | +0,+0 / +0,+0 |
| t1-boutons-fenetre | 459,12 | 16×16 | 6 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-boutons-fenetre | 1228,12 | 16×16 | 1 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-boutons-fenetre | 224,13 | 14×14 | 9 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-dock-replie | 1029,9 | 18×18 | 2 | 30 / 20 | 1,14 / 0,99 | +0,+0 / +0,+0 |
| t1-dock-replie | 1118,10 | 16×16 | 2 | 20 / 20 | 1,11 / 0,94 | +0,+0 / +0,+0 |
| t1-dock-replie | 995,12 | 16×16 | 2 | 38 / 18 | 1,19 / 1,01 | +0,+0 / +0,+0 |
| t1-docks-ouverts | 45,9 | 18×18 | 7 | 39 / 24 | 1,12 / 1,02 | +0,+0 / +0,+0 |
| t1-docks-ouverts | 1061,9 | 18×18 | 1 | 30 / 20 | 1,14 / 0,99 | +0,+0 / +0,+0 |
| t1-docks-ouverts | 720,12 | 16×16 | 1 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-docks-ouverts | 485,13 | 14×14 | 7 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-docks-ouverts | 50,774 | 14×14 | 7 | 31 / 29 | 1,19 / 1,16 | +0,+1 / +0,+1 |
| t1-docks-ouverts | 244,774 | 15×15 | 7 | 20 / 12 | 1,15 / 1,03 | +0,+0 / +0,+0 |
| t1-docks-ouverts | 269,774 | 15×15 | 7 | 44 / 28 | 1,20 / 1,05 | +0,+0 / +0,+0 |
| t1-entete-vue | 871,12 | 16×16 | 5 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-entete-vue | 46,42 | 18×18 | 1 | 10 / 10 | 1,18 / 0,99 | +0,+0 / +0,+0 |
| t1-entete-vue | 74,42 | 18×18 | 1 | 10 / 10 | 1,17 / 0,99 | +0,+0 / +0,+0 |
| t1-entete-vue | 1252,42 | 18×18 | 1 | 23 / 6 | 1,12 / 0,99 | +0,+0 / +0,+0 |
| t1-fenetre-vide | 253,12 | 16×16 | 4 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-glisser-depot | 634,12 | 16×16 | 4 | 38 / 18 | 1,19 / 1,01 | +0,+0 / +0,+0 |
| t1-glisser-depot | 876,12 | 16×16 | 3 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-glisser-depot | 847,13 | 14×14 | 3 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-menu | 435,272 | 15×15 | 1 | 37 / 28 | 1,17 / 1,02 | +0,+0 / +0,+0 |
| t1-menu | 583,400 | 14×14 | 1 | 19 / 8 | 1,19 / 1,03 | +0,+0 / +0,+0 |
| t1-menu | 435,438 | 15×15 | 1 | 74 / 68 | 1,16 / 1,12 | +0,+0 / +0,+0 |
| t1-menu-onglet | 570,32 | 15×15 | 1 | 29 / 12 | 1,15 / 1,00 | +0,+0 / +0,+0 |
| t1-modale | 914,111 | 16×16 | 1 | 41 / 38 | 1,18 / 1,12 | +0,+0 / +0,+0 |
| t1-onglet-survol | 636,13 | 14×14 | 1 | 17 / 11 | 1,22 / 1,06 | +0,+0 / +0,+0 |
| t1-pile-active-inactive | 1060,12 | 16×16 | 2 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-pile-active-inactive | 1031,13 | 14×14 | 2 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 878,10 | 14×14 | 2 | 0 / 0 | 1,00 / 1,00 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 924,10 | 14×14 | 2 | 33 / 17 | 1,25 / 1,04 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 970,10 | 14×14 | 2 | 40 / 27 | 1,07 / 0,90 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 23,57 | 14×14 | 2 | 29 / 24 | 1,15 / 1,11 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 937,79 | 16×16 | 2 | 24 / 19 | 1,16 / 1,16 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,130 | 16×16 | 2 | 44 / 38 | 1,15 / 1,12 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 865,157 | 16×16 | 2 | 24 / 19 | 1,16 / 1,16 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 902,157 | 16×16 | 2 | 42 / 43 | 1,15 / 1,12 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 938,157 | 16×16 | 2 | 45 / 45 | 1,13 / 1,10 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,162 | 16×16 | 2 | 46 / 42 | 1,16 / 1,14 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,193 | 16×16 | 2 | 46 / 43 | 1,13 / 1,10 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,225 | 16×16 | 2 | 63 / 52 | 1,12 / 0,99 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 938,234 | 16×16 | 2 | 52 / 50 | 1,15 / 1,12 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,256 | 16×16 | 2 | 35 / 30 | 1,14 / 1,10 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,288 | 16×16 | 2 | 55 / 49 | 1,14 / 1,10 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,319 | 16×16 | 2 | 40 / 34 | 1,13 / 1,10 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 18,387 | 16×16 | 2 | 33 / 30 | 1,14 / 1,10 | +0,+0 / +0,+0 |
| t1-reglages-appearance | 938,496 | 16×16 | 2 | 52 / 50 | 1,15 / 1,12 | +0,+0 / +0,+0 |
| t1-split-horizontal | 253,413 | 16×16 | 1 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-split-horizontal | 1256,413 | 16×16 | 1 | 38 / 18 | 1,19 / 1,01 | +0,+0 / +0,+0 |
| t1-split-horizontal | 224,414 | 14×14 | 1 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-split-proportions | 323,12 | 16×16 | 1 | 38 / 18 | 1,19 / 1,01 | +0,+0 / +0,+0 |
| t1-split-proportions | 771,12 | 16×16 | 1 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-split-proportions | 945,12 | 16×16 | 1 | 38 / 18 | 1,19 / 1,01 | +0,+0 / +0,+0 |
| t1-split-proportions | 742,13 | 14×14 | 1 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-split-proportions | 565,213 | 16×16 | 1 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t1-split-proportions | 945,213 | 16×16 | 1 | 38 / 18 | 1,19 / 1,01 | +0,+0 / +0,+0 |
| t1-split-proportions | 536,214 | 14×14 | 1 | 29 / 21 | 1,19 / 1,04 | +0,+0 / +0,+0 |
| t1-toolbar-horizontale | 313,261 | 16×16 | 2 | 54 / 62 | 1,20 / 1,20 | +0,+0 / +0,+0 |
| t1-toolbar-horizontale | 347,261 | 16×16 | 2 | 43 / 38 | 1,14 / 1,10 | +0,+0 / +0,+0 |
| t1-toolbar-horizontale | 381,261 | 16×16 | 1 | 33 / 30 | 1,15 / 1,12 | +0,+0 / +0,+0 |
| t1-toolbar-horizontale | 426,261 | 16×16 | 1 | 31 / 26 | 1,15 / 1,12 | +0,+0 / +0,+0 |
| t1-toolbar-horizontale | 460,261 | 16×16 | 1 | 11 / 9 | 1,16 / 1,13 | +0,+0 / +0,+0 |
| t1-toolbar-horizontale | 490,262 | 14×14 | 1 | 7 / 5 | 1,05 / 1,02 | +0,+0 / +0,+0 |
| t1-toolbar-sections | 392,261 | 16×16 | 1 | 39 / 35 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t1-toolbar-sections | 437,261 | 16×16 | 1 | 26 / 24 | 1,14 / 1,10 | +0,+0 / +0,+0 |
| t1-toolbar-sections | 531,262 | 14×14 | 1 | 7 / 5 | 1,05 / 1,02 | +0,+0 / +0,+0 |
| t1-zoom-150 | 14,13 | 27×27 | 1 | 22 / 22 | 1,16 / 1,03 | +0,+0 / +0,+0 |
| t1-zoom-150 | 1097,15 | 21×21 | 1 | 10 / 9 | 0,99 / 0,98 | +0,+0 / +0,+0 |
| t1-zoom-150 | 1166,15 | 21×21 | 1 | 20 / 9 | 1,07 / 0,98 | +0,+0 / +0,+0 |
| t1-zoom-150 | 1235,15 | 21×21 | 1 | 38 / 23 | 1,19 / 1,03 | +0,+0 / +0,+0 |
| t1-zoom-150 | 950,18 | 24×24 | 1 | 7 / 2 | 1,01 / 1,00 | +0,+0 / +0,+0 |
| t1-zoom-150 | 992,18 | 24×24 | 1 | 28 / 19 | 1,08 / 1,02 | +0,+0 / +0,+0 |
| t1-zoom-150 | 1037,18 | 24×24 | 1 | 24 / 9 | 1,01 / 1,00 | +0,+0 / +0,+0 |
| t1-zoom-150 | 467,20 | 21×21 | 1 | 36 / 22 | 1,12 / 1,01 | +0,+0 / +0,+0 |
| t1-zoom-150 | 14,72 | 27×27 | 1 | 36 / 28 | 1,14 / 1,00 | +0,+0 / +0,+0 |
| t1-zoom-150 | 14,123 | 27×27 | 1 | 44 / 28 | 1,14 / 1,03 | +0,+0 / +0,+0 |
| t1-zoom-150 | 14,174 | 27×27 | 1 | 36 / 21 | 1,16 / 1,03 | +0,+0 / +0,+0 |
| t1-zoom-80 | 7,7 | 14×14 | 1 | 76 / 56 | 1,16 / 0,97 | +0,+0 / +0,+0 |
| t1-zoom-80 | 1182,8 | 12×12 | 1 | 18 / 18 | 0,96 / 0,95 | -1,+1 / -1,+0 |
| t1-zoom-80 | 1219,8 | 11×11 | 1 | 33 / 23 | 1,14 / 1,02 | -1,-1 / -1,-1 |
| t1-zoom-80 | 1256,8 | 11×11 | 1 | 52 / 35 | 1,14 / 0,94 | +0,+0 / +0,+0 |
| t1-zoom-80 | 344,10 | 11×11 | 1 | 65 / 50 | 1,28 / 1,06 | +0,+0 / +0,+0 |
| t1-zoom-80 | 697,10 | 13×13 | 1 | 32 / 31 | 0,95 / 0,93 | +0,+0 / +0,+0 |
| t1-zoom-80 | 1126,10 | 13×13 | 1 | 40 / 23 | 1,27 / 1,11 | +1,+0 / +1,+0 |
| t1-zoom-80 | 1150,10 | 13×13 | 1 | 50 / 31 | 1,12 / 0,99 | +0,+0 / +0,+0 |
| t1-zoom-80 | 7,38 | 14×14 | 1 | 75 / 58 | 1,20 / 0,99 | +0,-1 / +0,-1 |
| t1-zoom-80 | 7,66 | 14×14 | 1 | 66 / 57 | 1,17 / 0,98 | +0,+0 / +0,+0 |
| t1-zoom-80 | 7,93 | 14×14 | 1 | 54 / 45 | 1,25 / 1,00 | -1,+0 / -1,+0 |
| t2-explorateur | 514,12 | 16×16 | 6 | 9 / 7 | 1,09 / 0,93 | +0,+0 / +0,+0 |
| t2-explorateur | 106,50 | 18×18 | 6 | 36 / 21 | 1,15 / 1,00 | +0,-1 / +0,-1 |
| t2-explorateur | 140,50 | 18×18 | 6 | 31 / 22 | 1,14 / 1,00 | +0,+0 / +0,+0 |
| t2-explorateur | 174,50 | 18×18 | 6 | 24 / 30 | 1,18 / 0,99 | +0,+0 / +0,+0 |
| t2-explorateur | 208,50 | 18×18 | 6 | 24 / 18 | 1,20 / 1,03 | +0,+0 / +0,+0 |
| t2-explorateur | 48,86 | 10×10 | 6 | 20 / 19 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur | 48,112 | 10×10 | 6 | 28 / 20 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur | 48,138 | 10×10 | 5 | 20 / 19 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur | 48,164 | 10×10 | 5 | 20 / 19 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,129 | 15×15 | 1 | 45 / 33 | 1,18 / 1,02 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,159 | 15×15 | 1 | 49 / 39 | 1,16 / 1,03 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,197 | 15×15 | 1 | 37 / 28 | 1,17 / 1,02 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,227 | 15×15 | 1 | 34 / 36 | 1,17 / 1,02 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,265 | 15×15 | 1 | 39 / 40 | 1,15 / 1,03 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 385,266 | 14×14 | 1 | 19 / 8 | 1,19 / 1,03 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,304 | 15×15 | 1 | 34 / 22 | 1,18 / 1,05 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,342 | 15×15 | 1 | 48 / 34 | 1,23 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur-menu-dossier | 165,372 | 15×15 | 1 | 74 / 68 | 1,16 / 1,12 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,285 | 15×15 | 1 | 39 / 26 | 1,16 / 1,03 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,315 | 15×15 | 1 | 39 / 22 | 1,17 / 1,03 | +1,+0 / -1,+0 |
| t2-explorateur-menu-fichier | 165,353 | 15×15 | 1 | 37 / 28 | 1,17 / 1,02 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,383 | 15×15 | 1 | 34 / 36 | 1,17 / 1,02 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,421 | 15×15 | 1 | 39 / 40 | 1,15 / 1,03 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 385,422 | 14×14 | 1 | 19 / 8 | 1,19 / 1,03 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,460 | 15×15 | 1 | 34 / 22 | 1,18 / 1,05 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,489 | 15×15 | 1 | 34 / 22 | 1,18 / 1,05 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,528 | 15×15 | 1 | 48 / 34 | 1,23 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur-menu-fichier | 165,557 | 15×15 | 1 | 74 / 68 | 1,16 / 1,12 | +0,+0 / +0,+0 |
| t2-explorateur-ouvert | 65,138 | 10×10 | 1 | 20 / 19 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur-ouvert | 65,216 | 10×10 | 1 | 20 / 19 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur-ouvert | 48,268 | 10×10 | 1 | 20 / 19 | 1,12 / 1,09 | +0,+0 / +0,+0 |
| t2-explorateur-ouvert | 48,294 | 10×10 | 1 | 20 / 19 | 1,12 / 1,09 | +0,+0 / +0,+0 |
