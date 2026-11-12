# ADR-015 — Interligne exact de l'éditeur (fork n°1, extension du patch d'ADR-006)

- Statut : proposé (équipier `fr-t3.5`, 12/11/2026). À accepter par le chef.
- Contexte : le live preview pose `line_height(16 × 1,4 = 22,4 px)` (`editeur/constantes.rs`, thème de base de CodeMirror). Chromium garde 22,4 ; l'éditeur du fork rendait 22. Chaque ligne dérivait de 0,4 px : la ligne 100 était à 2 200 px de la ligne 0 au lieu de 2 240, et une scène T3 d'une note longue ne pouvait pas être stricte.

## Diagnostic

- **L'arrondi vient de GPUI, pas du fork.** `Window::line_height()` (gpui-pre 0.3.10 et 0.3.12, `window.rs`) rend `TextStyle::line_height_in_pixels`, qui fait `line_height.to_pixels(font_size, rem).round()` (`style.rs:554`). Rien en amont ne rend la valeur exacte.
- **Le fork le reçoit à cinq endroits** : `base/element.rs` (`request_layout`, `prepaint`, `paint`, défilement à la molette) et `base/state.rs` (défilement). Tout le reste de l'éditeur lit `last_layout.line_height`, donc la valeur arrondie se propage à la grille, au curseur, à la sélection, au hit-test et à `EditorState::line_height()`.
- **Le reste du fork tient le fractionnaire** : les hauteurs dictées (titres, blocs) passent déjà par `row_top`/`row_at_y` en `f32`. Les glyphes sont posés à l'ordonnée arrondie au pixel (`SUBPIXEL_VARIANTS_Y = 1`, `paint_glyph`), comme Chromium arrondit la ligne de base.

## Options

| Option | Coût | Risque |
|---|---|---|
| A. Dicter 22,4 à CHAQUE ligne par `set_row_model` | ~20 lignes, mais dans `editeur/modele.rs` et `element.rs` (fichiers de `fr-t3.4`) | Vecteur de n hauteurs à chaque projection ; `haut_de_ligne` de la surface passe en O(n log n) ; la grille de base reste à 22 (rangées non dictées, défilement à la molette, curseur des lignes vides du fork) : deux sources de vérité. |
| **B. Le fork lit l'interligne exact (retenue)** | 1 fonction + 5 remplacements d'une ligne dans `base/element.rs` et `base/state.rs`, déjà déclarés dans `NOTICE` | Faible : même arithmétique, en `f32`. Les fonds (sélection, ligne active) tombent à des ordonnées fractionnaires et sont anticrénelés par GPUI là où Chromium les cale au pixel : à surveiller aux scènes T3 avec sélection (correctif, s'il le faut : caler les quads au pixel dans `paint`). |
| C. Contourner sans fork (rem, échelle, interligne choisi pour que l'arrondi tombe juste) | — | Impossible : toute valeur passe par `.round()`. |

## Décision

Option B. `exact_line_height(window)` (dans `base/element.rs`, marquée `// ★ fragment-rs`) recalcule `text_style.line_height.to_pixels(font_size, rem_size)` sans arrondi ; les cinq appels à `window.line_height()` de l'éditeur l'utilisent. Les autres composants de gpui-base (`scroll_bounce`, `scrollable_mask`, `text/`) ne sont pas touchés. Pas de nouvelle API, pas de nouveau fichier modifié, toujours fork n°1 (`program.md` §10).

Preuve : `interligne_fractionnaire_ligne_100` (`crates/fragment-ui/tests/interligne.rs`) mesure la ligne 100 à 100 × 22,4 = 2 240 px de la ligne 0, puis 22,4 px entre les lignes 99 et 100 après défilement ; `titre_plus_grand` attend désormais 22,4.

## PR amont (non ouverte)

À joindre à la série de PR d'ADR-006 :

> **input: use the unrounded line height for layout**
>
> `Window::line_height()` rounds to whole pixels. For an editor with `line-height: 1.4` at 16 px the rows are 22 px instead of 22.4, so the input drifts 0.4 px per row from a browser rendering the same text (40 px after 100 lines). The input now derives its row height from the text style without rounding; glyph baselines are still snapped by the text system. Inputs whose line height is already whole are unchanged.

## Conséquences

- Les scènes T3 longues peuvent être strictes sur la géométrie des lignes.
- Le rebase du fork (procédure d'ADR-006) porte un remplacement de plus par point d'accroche ; le job `fork-patch` le voit.
