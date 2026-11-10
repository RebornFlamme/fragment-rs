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
