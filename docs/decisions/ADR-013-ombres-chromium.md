# ADR-013 — Ombres et bordures : émuler la composition de Chromium, sans fork

Statut : **proposé** (fr-p4.1, 11/11/2026). Mesures : `docs/spikes/S8-melange-alpha.md`.

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

1. **Opacité exacte** (fait, fr-p4.1). Une couleur `color-mix(… p %, transparent)` passe à GPUI son opacité en flottant (`rendu::melange::opacite_exacte`). `flottant::bordure_translucide` est retiré.
2. **Calque émulé pour les surfaces `fixed`** (à faire, bead de suite).
   - L'ombre n'est plus le `BoxShadow` de GPUI. C'est une image RGBA8 calculée par le CPU, gaussienne exacte, σ = flou/2, et mise en cache par géométrie.
   - Pour la modale et la palette, l'image fusionne le voile et l'ombre, quantifiés comme le calque de Chromium. Le voile la peint avant la boîte, ce qui est possible parce que la boîte déclare sa géométrie au prépeint, qui précède toute peinture.
   - Les menus, notices et infobulles reçoivent une image d'ombre peinte par une toile placée en tête de leur couche.
   - Gain simulé : `--shadow-l` passe de 1,6 à 0,2 px au pire.
3. **Fond en retrait sous une bordure opaque** (à faire, même bead). La boîte est peinte de la couleur de la bordure, et le fond est un enfant en retrait de la bordure, de rayon `r − bordure`. Sur la mire, l'écart à Chromium passe de 31 à 8 niveaux au pire.
4. **Pas de fork de GPUI.** Un shader d'ombre à 64 échantillons ne corrigerait ni le calque (cause 1), ni la forme propre à Skia (cause 2). Il ne justifierait pas le fork n° 2.
5. **Le zéro sur `--shadow-l` est déclaré hors d'atteinte au bit près.**
   - Le flou des rectangles arrondis de Skia sur GPU dépend de son chemin (neuvaine, flou sous-échantillonné) et du filtrage bilinéaire du GPU de la référence.
   - Ce qui reste après les points 2 et 3 (simulé : étalement de 0,11 à 0,20 px, delta ≤ 2) est gardé en exception nommée, avec les nouvelles valeurs en cliquet, jusqu'à ce qu'une table de couverture mesurée par géométrie (sur la mire de S8) le résorbe ou que l'orchestrateur l'accepte comme dette définitive.

## Conséquences

- Les exceptions nommées de T1 et `--shadow-l` restent dans le cliquet jusqu'au bead de suite. Elles ne sont pas relâchées : leurs valeurs ne peuvent que baisser.
- La mire de S8 (scripts hors dépôt, décrits dans le spike) peut se rejouer pour toute nouvelle géométrie d'ombre.
- Le coût CPU des images d'ombre, avec cache par géométrie (une modale : environ 200 000 pixels), est à mesurer dans le bead de suite.
