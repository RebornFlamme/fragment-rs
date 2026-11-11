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
