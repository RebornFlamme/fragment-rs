# ADR-013 — Ombres et bordures : émuler la composition de Chromium, sans fork

Statut : **accepté** par le chef (fr-p4.1, 11/11/2026), échange de cliquets à confirmer par l'orchestrateur (section « Échange daté du 11/11 »). Mesures : `docs/spikes/S8-melange-alpha.md`.

## Contexte

L'ADR-009 a accordé deux tolérances temporaires, en attendant de trouver la cause :
- l'exception nommée `--shadow-l` : étalement de +1,5 px en sombre, −0,6 px en clair ;
- les exceptions nommées de T1 : ombres de l'infobulle, du menu clair et des notices claires ; arcs clairs.

On pensait que ces écarts venaient d'un mélange alpha différent : « linéaire contre sRGB ». S8 montre que ce n'est pas le cas. GPUI et Skia composent tous deux en sRGB encodé, et le candidat est reproduit au bit près par une simulation du shader. Les causes réelles sont au nombre de quatre :
1. le calque composé de Chromium (conteneur `position: fixed`) ;
2. la forme du flou de Skia ;
3. la bordure que GPUI peint deux fois au bord extérieur ;
4. l'opacité des jetons ramenée à l'octet.

## Décision proposée

1. **Opacité exacte** (fait, fr-p4.1). Une bordure `color-mix(… p %, transparent)` sur un fond opaque est peinte en aplat exact (`rendu::melange::aplat_exact` : flottant, demie vers le haut). `flottant::bordure_translucide` est retiré ; `opacite_exacte` reste pour une couleur translucide sur fond inconnu.
2. **Calque émulé pour les surfaces `fixed`** (fait pour la modale et la palette, `rendu::ombre`).
   - L'ombre n'est plus le `BoxShadow` de GPUI. C'est une image RGBA8 calculée par le CPU, gaussienne exacte, σ = flou/2, et mise en cache par géométrie.
   - Pour la modale et la palette, l'image fusionne le voile et l'ombre, quantifiés comme le calque de Chromium. Le voile la peint avant la boîte, ce qui est possible parce que la boîte déclare sa géométrie au prépeint, qui précède toute peinture.
   - Menus, notices et infobulles gardent le `BoxShadow` de GPUI : la simulation de S8 avec la gaussienne exacte ne les améliore pas (infobulle +0,22 → +0,68 px). Ce qui les sépare de Chromium est la forme de Skia (dette (b)), leurs exceptions d'ombre restent.
   - Gain simulé : `--shadow-l` passe de 1,6 à 0,2 px au pire.
3. **Fond en retrait sous la bordure** (fait, `rendu::melange::bord_unique` : menu, notice, infobulle, modale, palette). La boîte est peinte de l'aplat opaque de sa bordure, sans quad de bordure, et le fond est un enfant en retrait, de rayon `r − bordure` (bordure opaque) ou `r − bordure + 0,4` (bordure translucide, mesuré sur la mire de S8). Sur la mire opaque, l'écart à Chromium passe de 31 à 8 niveaux au pire.
4. **Pas de fork de GPUI.** Un shader d'ombre à 64 échantillons ne corrigerait ni le calque (cause 1), ni la forme propre à Skia (cause 2). Il ne justifierait pas le fork n° 2.
5. **Le zéro sur `--shadow-l` est déclaré hors d'atteinte au bit près.**
   - Le flou des rectangles arrondis de Skia sur GPU dépend de son chemin (neuvaine, flou sous-échantillonné) et du filtrage bilinéaire du GPU de la référence.
   - Ce qui reste après les points 2 et 3 (simulé : étalement de 0,11 à 0,20 px, delta ≤ 2) est gardé en exception nommée, avec les nouvelles valeurs en cliquet, jusqu'à ce qu'une table de couverture mesurée par géométrie (sur la mire de S8) le résorbe ou que l'orchestrateur l'accepte comme dette définitive.

## Conséquences

- Exceptions retirées : arcs de l'infobulle, du menu, de la notice et des notices claires. Restent : `--shadow-l` (8 PNG), arcs de la modale et des palettes claires, ombres de l'infobulle, du menu clair et des notices claires. Leurs valeurs baissent, sauf l'échange daté ci-dessous.
- Retirée le 12/11 (chef, fr-t1.5) : l'ombre du menu clair. Le premier niveau du menu passe par l'ombre CPU du calque émulé ; étalement 0,185 → 0,020 px, sous la borne de 0,08.
- La mire de S8 (scripts hors dépôt, décrits dans le spike) peut se rejouer pour toute nouvelle géométrie d'ombre.
- Coût d'une image d'ombre de modale (758 × 250 px, σ 24) : 0,33 s en debug (profil de la CI), une fois par taille, jeton et thème, puis lue dans le cache (16 entrées au plus). Le flou passe par les formes de rangées du masque (séparable) : 2,2 s → 0,33 s en debug. Le profil release n'a pas été mesuré.

## Échange daté du 11/11

Mesures CI (run 43040649423, PR #56) contre `main` (run 43018474495). Accepté par le chef, à confirmer par l'orchestrateur. En cas de refus, on revient à GPUI pour les quatre PNG clairs (modale, palette, palette aucun-résultat, palette sélection).

| PNG | Clé | Avant | Après |
|---|---|---|---|
| modale sombre | `--shadow-l` étalement / encre / glissement | 1,60 px / 0,34 / 0,43 | **0,09 px** / 0,05 / 0,11 |
| palettes sombres | `--shadow-l` étalement | 1,42 px | **0,00 à 0,01 px** |
| modale claire | `--shadow-l` étalement | 0,60 px | **0,18 px** |
| palettes claires | `--shadow-l` étalement | 0,58 px | **0,21 px** |
| modale et palettes claires | arcs : déplacement / delta max / px > 6 | 0,20 px / 33 / 60 | **0,02 px / 9 / 22** |
| menu, notices, infobulle claires | arcs : déplacement | 0,23 à 0,35 px | **0,07 à 0,11 px**, exceptions retirées |
| tous les PNG à arcs, sauf modale et palettes claires | delta max / % des bandes d'arrondi | 4 à 6 / 55 à 83 % | **2 à 3 / 27 à 77 %** |
| modale et palettes claires | delta max d'ombre | 1 | 2 (hausse) |
| modale claire | `--shadow-l` encre / glissement | 0,033 / 0,024 | 0,035 / 0,071 (hausse) |
| palette aucun-résultat claire | `--shadow-l` glissement | 0,043 | 0,049 (hausse) |
| modale et palettes claires | % des bandes d'arrondi | 20 à 25 % | 35 à 46 % (hausse) |

**Justification.** La géométrie gagne un ordre de grandeur : étalement de `--shadow-l` divisé par 3 à 100, arcs ramenés sous la borne. Les hausses ne portent que sur l'anticrénelage, à 1 niveau. Elles viennent de deux sources. D'une part, le halo clair sort de la gaussienne exacte, et non de la forme de Skia. D'autre part, la couverture de GPUI (`0,5 − distance` au centre du pixel) n'est pas celle de Skia, alors que le double anticrénelage tombait avant juste, par hasard, sur certains pixels. Les garde-fous d'ADR-009 sont intacts : delta d'ombre ≤ 2, glissement ≤ 10 %, encre ±20 %, contre-épreuve. Toutes les autres clés du cliquet baissent ou restent égales (diff du cliquet dans la PR).

## Dette (b) : la forme du flou de Skia, chiffrée

Après le calque émulé, il reste ceci :
- `--shadow-l` en clair, étalement : +0,18 px pour la modale, +0,21 px pour les palettes (borne 0,08). En sombre : 0,09 px pour la modale, 0,00 à 0,01 px pour les palettes.
- Ombres de l'infobulle, du menu et des notices : 0,15 à 0,24 px.
- Ombre du menu d'un fichier de l'explorateur, sombre (`t2-explorateur-menu-fichier-dark`, chef du 13/11) : étalement −0,12 px, côté droit −0,31 px, déjà ombrée au calque émulé. La gaussienne simulée reproduit le candidat à 100 % et la référence à 95 % : même forme de flou, même dette.
- Arcs opaques de la modale et des palettes claires : 22 px de delta 7 à 9 à la jonction de l'arc et du côté droit.

Ce reste est hors d'atteinte au bit près sans la couverture de Skia sur GPU. Celle-ci dépend du chemin (neuvaine, flou sous-échantillonné) et du GPU de la référence. Seule une table de couverture mesurée par géométrie sur la mire de S8 le résorberait. Simulée, elle donne 0,01 px.