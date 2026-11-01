# S1 — Éditeur Markdown live preview sur gpui-kit 0.8.1

- Tâche Beads `fr-p2-s1`, PR « fr-p2-s1 : spike éditeur live preview ».
- Code : `crates/fragment-markdown/tests/spike_s1.rs` (10 tests headless), `tests/spike_s1/live.rs` (modèle + vue), `examples/spike_s1.rs` (à lancer à la main : `cargo run -p fragment-markdown --example spike_s1`, Windows).
- Méthode : lecture des sources de `gpui-base` / `gpui-component` 0.8.1 (`input/`), puis tests sur l'`EditorState` stock avec `language("markdown")` (feature `tree-sitter-markdown`). Les 10 tests passent sur `test-windows` (CI de la PR). Les durées viennent d'un run CI jetable (`windows-latest`, branche supprimée depuis), débogage ET `--release`.
- ★ Limites de la mesure : la plateforme de test de GPUI utilise `NoopTextSystem` (chasse fixe 0,6 em = 7,8 px à 13 px, 20 px de ligne, **aucun pixel, aucune mise en forme DirectWrite**). Les tests prouvent le comportement et la géométrie logique, pas le rendu ; les durées excluent le coût réel des glyphes.

## Verdict global : **NO-GO sur l'éditeur stock comme base du live preview — repli GO-AVEC, option B (fork de `gpui-base::input`), chiffré ci-dessous**

Ce que l'éditeur stock fait bien (frappe, IME, undo, wrap, sélection, 10 000 lignes fluides, coordonnées) suffit à une édition de texte. Il ne peut pas être un live preview fidèle à Fragment pour trois raisons structurelles, établies à la lecture ET par test :

1. **Cacher ≠ retirer** : une décoration (`TextDecoration` = `HighlightStyle` : couleur, graisse, fond, soulignement, `fade_out`) ne modifie jamais la mise en page. `# ` « caché » (glyphes transparents) laisse un trou de 2 × 7,8 px ; Fragment fait disparaître le marqueur et le texte se décale.
2. **Une seule hauteur de ligne** : pas de titres plus grands (`hLine` de Fragment), pas de ligne plus haute pour un widget.
3. **Pas de widget de bloc ni de remplacement inline** : seuls des calques posés PAR-DESSUS (de la hauteur exacte des lignes sources) sont possibles ; un widget de hauteur libre (image, tableau à cellules multilignes, embed, math display) ne l'est pas.

Les trois sont dans la liste « pixel perfect » du programme (`program.md` §0). Il n'y a pas de contournement propre : il faut changer la couche d'affichage de l'éditeur (section « Repli »).

## Par point

### 1. Frappe et IME — GO (IME réelle : non testable)
- Hypothèse : l'éditeur stock gère la frappe et la composition.
- Essayé : `frappe_et_retour_chariot` (clic, `# Titre`, Entrée, `corps **gras**`) ; `composition_ime_simulee_et_unites_utf16` (`TestInput` : `compose_text` « に » puis « 日本», `commit_text` « 日本語»).
- Mesure : valeur finale exacte ; langage retenu `markdown` ; plage marquée `2..3` puis `2..4` (UTF-16) ; après commit, sélection `5..5` côté système (UTF-16) mais `11..11` côté éditeur (octets).
- Verdict : GO pour la frappe et le protocole de composition. ★ L'IME du système (fenêtre de candidats, TSF) ne se teste pas en headless (`TESTING.md` le dit) : à vérifier à la main avec l'exemple, en japonais. ★ Frontière d'unités : l'éditeur donne des **octets UTF-8** (`selected_range`, décorations, `range_to_bounds`) et parle UTF-16 au système ; ADR-004 impose UTF-16 aux contrats : la conversion (`fragment-text::utf16`) est à faire dans l'adaptateur `TextSurface`.

### 2. `#` masqué hors curseur, révélé dessous (`curseurActif`) — GO-AVEC (trou non résorbable)
- Hypothèse : des décorations pilotées par la règle `touches(from,to)` cachent/révèlent les marqueurs.
- Essayé : modèle `compute` (titre = ligne entière, `**…**` = étendue), `LiveView` qui réécrit deux `TextDecorationCollection` (glyphes transparents + gras). Test `diese_masque_hors_curseur_et_revele_dessous` : curseur en fin de document → `0..2, 15..17, 21..23` cachés ; dans le titre → `15..17, 21..23` ; dans le gras → `0..2` seul. Test `le_curseur_declenche_la_regle_par_observation` : la règle se rejoue sans appel explicite.
- Mesure : x du premier caractère après `# ` **identique** caché ou révélé (45,6 px) ; hauteur de ligne titre = corps = 20 px.
- Verdict : la règle et les ranges sont corrects ; le rendu ne l'est pas (trou, pas de titre agrandi). ★ Pièges relevés : `InputEvent` n'a pas d'événement « sélection changée » → observer l'entité (`cx.observe`) ; `TextDecorationCollection::set` **notifie toujours** (même inchangé) → sans garde de comparaison, l'observation boucle ; chaque `set` réindexe toute la collection.

### 3. Case à cocher INLINE cliquable — GO-AVEC (calque posé sur la source)
- Hypothèse : un élément GPUI cliquable posé sur `[ ]`.
- Essayé : (a) `InlineToken` (atome inline avec rendu GPUI et clic) : **refusé** — `install_tokens` exige `!M::CODE_EDITOR` (les jetons n'existent que pour `Input`/`Textarea`, qui n'ont ni décorations ni coloration), et `set_value` efface l'historique ; (b) calque `div` absolu placé aux coordonnées de `range_to_bounds(2..5)`, `occlude`, `on_click` → `replace("[x]")`. Test `case_a_cocher_inline_cliquable`.
- Mesure : widget (45,5 ; 8) 23,5 × 20 px contre source (45,6 ; 8) 23,4 × 20 px (à 0,1 px, arrondi) ; le clic bascule `- [ ]` → `- [x]`, journal `["task"]` ; Ctrl+Z restaure, Ctrl+Y rétablit (une seule transaction).
- Verdict : GO-AVEC. Limites : la géométrie lue est celle de l'image précédente (décalage d'un cadre pendant un défilement) ; le calque doit être (re)positionné pour chaque case visible ; la largeur est celle de la source (3 caractères), pas celle d'une vraie case.

### 4. Widget de BLOC atomique, clic dessous sur la bonne ligne — NO-GO pour la hauteur libre ; GO-AVEC pour une plaque de hauteur égale à ses lignes
- Hypothèse : un calque opaque posé sur les lignes d'un tableau se comporte en bloc atomique.
- Essayé : test `widget_de_bloc_et_clic_dessous` (tableau 3 lignes).
- Mesure : widget y = 48 px = y de la ligne 2 ; hauteur 60 px = exactement 3 × 20 px ; clic sur le widget → journal `["table"]`, curseur inchangé ; clic sous le widget (ligne « après ») → curseur dans la bonne ligne ; clic sur la ligne vide juste dessous → offset de cette ligne.
- Verdict : le clic dessous est juste **parce que les lignes sources existent toujours** ; le widget ne peut pas être plus haut ni plus bas que ses lignes (une image, un tableau à cellules enrichies, un embed ne tiennent pas). ★ Non atomique au clavier : Bas depuis la ligne 1 fait entrer le curseur dans la source cachée (octet 7, dans `7..36`) ; il faudrait intercepter chaque déplacement. Fragment fait l'inverse (CodeMirror : `Decoration.replace`/widget de bloc de hauteur libre).

### 5. Soft wrap — GO
- Essayé : test `soft_wrap` (480 caractères sur 800 px, puis wrap coupé).
- Mesure : 7 rangées visuelles (3 sans wrap : la ligne longue + « fin » + dernière) ; y du premier et du dernier caractère 8 px / 88 px ; Bas depuis le début → offset 96 (la rangée visuelle, pas la ligne logique).
- Verdict : GO. (Le wrap est calculé sur la police du rendu : avec un vrai texte les valeurs changent, le comportement non.)

### 6. Annuler / rétablir — GO
- Essayé : `annuler_et_retablir` et la bascule de case (point 3).
- Mesure : la frappe consécutive est groupée (annulation de `cd` seul d'abord, puis de `\n`), Ctrl+Y rétablit ; les décorations suivent les éditions (ranges ajustées). Les décorations ne font pas partie de l'historique (voir la doc de `create_decorations_collection`) : le modèle les recalcule.
- Verdict : GO. Un toggle de case = un pas.

### 7. 10 000 lignes, défilement — GO (sous réserve du rendu réel)
- Essayé : 570 890 octets (titres, tâches, lignes de tableau, paragraphes), `language("markdown")`, `dix_mille_lignes`.
- Mesure (CI `windows-latest`, `NoopTextSystem`) :

| | débogage | `--release` |
|---|---|---|
| modèle `compute` sur tout le document | 2,9 ms | 0,5 ms |
| création de l'éditeur + `set_value` + parse tree-sitter | 848 ms | 115 ms |
| 1re image | 5,8 ms | 1,8 ms |
| décorations du document entier (6 000 spans) | 13,4 ms | 3,4 ms |
| défilement : moyenne / max par pas de 3 lignes | 16,6 / 23,3 ms | 3,9 / 5,6 ms |
| frappe au milieu du document (moyenne / caractère) | 12,4 ms | 2,9 ms |

- Verdict : GO pour la structure (rendu virtualisé : lignes 300..326 seulement après défilement, frappe incrémentale). ★ Les glyphes ne sont pas mesurés (Noop) ; la doc de l'éditeur annonce « jusqu'à ~50 K lignes ». À rejouer avec l'exemple et un vrai texte (DirectWrite) avant de s'engager.

### 8. `coordsAtPos` / `posAtCoords` (contrat `TextSurface`) — GO-AVEC
- Essayé : test `coordonnees_et_positions`.
- Mesure : `coords_at_pos` = `range_to_bounds(k..k)` (publique, coordonnées fenêtre, `None` hors rangées posées) ; `pos_at_coords` n'a **pas d'API publique** (`index_for_mouse_position` est `pub(crate)`) : reconstruite par dichotomie sur `range_to_bounds` ; 4 points sur 4 identiques au clic réel (offsets exacts) ; chasse 7,8 px, ligne 20 px.
- Verdict : GO-AVEC : la dichotomie est O(log n) appels mais ne marche que pour les rangées posées (le défilement hors écran = `None`) ; un `TextSurface` complet (positions hors écran, UTF-16) demande soit un PR amont qui expose `index_for_mouse_position`, soit le fork.

### 9. Maths `$x^2$` → image — évalué sur papier, non testé
- Besoin : rendre `$…$` (inline) et `$$…$$` (bloc) par `typst`/`rex` → SVG → image GPUI, puis l'insérer DANS le flux du texte avec l'alignement de ligne de base, et le remplacer par la source sous le curseur (comme `MathInlineWidget` de Fragment).
- Fidélité : Fragment rend avec MathJax ; `rex` (moteur de mise en page TeX, police à table MATH) et `typst` (syntaxe de maths différente de LaTeX, compilateur lourd) ne reproduiront pas les glyphes de MathJax au pixel près : accepter une dette de fidélité, à noter en ADR. Aucune de ces crates n'a été ajoutée ni essayée.
- Insertion : sur l'éditeur stock, impossible proprement (remplacement inline = point 2/3 : le widget n'occupe pas la place de la source, hauteur libre = point 4). C'est un cas du même blocage, pas un risque nouveau.

## Repli chiffré

Estimations (pas des mesures) à partir des tailles lues dans `gpui-base-0.8.1/src/input` : 33 000 lignes au total, dont `base/state.rs` 12 400 (une bonne moitié de tests), `base/element.rs` 5 460 (peinture/prépeinture), `display_map/` ≈ 3 100 (`text_wrapper` 1 940, `fold_map`, `wrap_map`).

- **Option B (recommandée) — fork de `gpui-base::input`** (Apache-2.0, ADR « fork de crate » + `[patch.crates-io]`, §10 de `program.md`) : ajouter à `DisplayMap` une couche de **hauteurs de rangée variables + masquage de plages + blocs** (le `DisplayMap` est déjà en couches à la Zed : wrap → fold → display) et faire suivre `element.rs` (peinture, `line_and_position_for_offset`, `index_for_mouse_position`, déplacements verticaux). On garde IME, sélection, undo, recherche, accessibilité, scrollbars. Estimation : 3 à 4 semaines d'un équipier + dette de rebase à chaque montée de gpui-kit (ADR-001 : montée = PR dédiée). Risque : `element.rs` et `state.rs` sont monolithiques.
- **Option C — mise en page maison sur `gpui::TextSystem`** (le repli de `program.md`) : tampon (`ropey`) + undo, `DisplayMap` maison (masquage → wrap → blocs/hauteurs), élément GPUI qui peint des `ShapedLine`, `EntityInputHandler` (IME, UTF-16), sélection/mouvements/clavier, défilement virtualisé, accessibilité. Estimation ≈ 8 000 à 9 000 lignes Rust, 6 à 8 semaines d'un équipier ; aucune dette amont mais tout le comportement de base (IME en tête) est à retester.
- **Option A — rester sur le stock** avec calques : possible seulement en abandonnant le pixel perfect sur masquage, titres et blocs (non conforme au programme).

Recommandation : lancer B en premier (un seul sous-spike de 3 jours : masquage + hauteur de rangée dans `DisplayMap`, vérifiable avec les mêmes tests headless de `tests/spike_s1.rs`) ; basculer sur C si le sous-spike montre que `element.rs` résiste. Dans les deux cas, le modèle `compute`/règle `curseurActif` et la vue de calques de ce spike se réutilisent.

## À retenir pour T3
- Tests headless : tout se teste (clics réels, frappe, undo, composition simulée, géométrie) sans pixel ; `NoopTextSystem` = chasse 0,6 em, rien sur les vraies polices.
- Position d'un calque = `range_to_bounds` (image précédente) ; ne jamais supposer qu'il suit un défilement dans la même image.
- `TextDecorationCollection::set` notifie toujours : garder le dernier calcul.
- Cargo : GPUI n'est tirée que sous `cfg(windows)` dans les `dev-dependencies` de `fragment-markdown` (les jobs Linux ne l'installent pas) ; le `Cargo.lock` gagne `tree-sitter`, `tree-sitter-md`, `tree-sitter-json`, `tree-sitter-language`, `streaming-iterator` (feature `tree-sitter-markdown` de gpui-kit, dépendance autorisée d'avance).
