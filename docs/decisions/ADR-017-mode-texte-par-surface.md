# ADR-017 — Mode du texte par surface : ClearType sur fond opaque, gris dans un calque (BROUILLON)

Statut : **brouillon** (fr-p4.1.4, 13/11/2026), à trancher par l'orchestrateur avant tout code (condition 3 du bead). Numéro à confirmer par le chef.

## Contexte

fr-p4.1.3 (`docs/mesures/rasterisation-glyphes.md`) a montré que Chromium peint en ClearType le texte d'une surface opaque non composée, et en gris le reste. Le mode `Subpixel` de GPUI reproduit les mots ClearType (encre 0,99, largeur +0,04 px), le gris reproduit les mots gris (1,02). Décision de l'utilisateur : le mode découle d'une PROPRIÉTÉ de la surface, jamais d'une liste de noms.

## Ce que dit Fragment (CSS, lecture seule)

Deux propriétés font passer Chromium au gris :
1. **Calque composé** : surfaces `position: fixed` qui se superposent au contenu. `core/menu.css:5`, `core/modal.css:8`, `core/notice.css:5` (plus `transform` animé, l. 33-39), `core/tooltip.css:5`, la palette (modale), `workspace.css:1071` (`.status-bar`, `fixed` et `z-index: 10` : c'est pour cela que « Fragment 0.0.0 » est gris).
2. **Opacité < 1 sur un ancêtre** (effet, donc surface intermédiaire transparente) : `workspace.css:411` (`.workspace-tab-header { opacity: 0.6 }`, remis à 1 par `.is-active`, l. 538, et au survol dans les docks, l. 1019), `.clickable-icon` (l. 375), `.view-header-title-parent` (l. 779). C'est pour cela que seuls l'onglet ACTIF et le texte plein sont ClearType sur `t1-onglets`.

## Chiffrage du crochet GPUI (gpui-pre 0.3.12, Cargo.lock)

**Le mode se règle par élément, sans fork.** `TextRenderingMode` est un `Rc<Cell<_>>` partagé entre `App` (`app.rs:972`, `set_text_rendering_mode` l. 1852) et chaque `Window` (`window.rs:1231`). Il est lu glyphe par glyphe dans `Window::paint_glyph` (`window.rs:5071`) par `should_use_subpixel_rendering` (l. 5149-5166). Le sprite (`SubpixelSprite` ou `MonochromeSprite`, l. 5124) et la clé d'atlas (`RenderGlyphParams::subpixel_rendering`, `text_system.rs:1113`) suivent : gris et ClearType ne se mélangent pas. Un élément de `fragment-ui` peut donc poser le mode dans `Element::paint`, peindre ses enfants et le remettre. Les dessins différés (`paint_deferred_draws`, l. 4184) sont peints après l'arbre, donc dans le mode de base. `reuse_paint` (l. 4310) recopie les sprites déjà choisis. `vendor/gpui-base` n'y touche pas.

**Mais la propriété 2 (opacité) n'est pas lisible hors de GPUI.** `Div` applique `style.opacity` (hover compris) par `with_element_opacity` (`elements/div.rs:2449`, `2598`), et `Window::element_opacity` est `pub(crate)` (`window.rs:4554`). Aucun élément de `fragment-ui` ne peut savoir qu'un ancêtre est à 0,6, ni qu'il repasse à 1 au survol (`t1-onglet-survol`).

## Options

**(A) Sans fork.** Mode de base de l'App = gris. Un élément `SurfaceOpaque` à la racine de la fenêtre peint l'arbre principal en `Subpixel` ; `fragment_ui::flottant` (menus, modale, palette, notices, infobulle) et la barre d'état repassent au gris ; les dessins différés le sont d'office. Pour l'opacité : un élément `Attenue { repos, survol }` qui tient son propre `Hitbox` et pose le gris tant que l'opacité effective est < 1, substitué aux 50 appels `.opacity(` de 15 fichiers de `fragment-ui` (dont des fichiers possédés par d'autres tâches), avec `disallowed-methods` sur `Styled::opacity` dans `clippy.toml` pour qu'une surface nouvelle ne puisse pas l'oublier. Coût : ~150 lignes d'élément, ~50 sites touchés, `group_hover` et opacités d'état (`md_html/bloc.rs:865`, `chat_fil/message.rs:422`) à traiter un par un. Risque : la règle 2 est réimplantée à côté de GPUI, donc deux sources de vérité pour l'opacité.

**(B) Fork n° 2 : patch de 3 lignes dans `gpui-pre`.** Dans `should_use_subpixel_rendering` : `if self.element_opacity < 1.0 { return false; }`. La règle 2 devient exacte et automatique (hover compris, toute surface nouvelle). La règle 1 reste en `fragment-ui` sans fork (élément racine + `flottant`). Coût : vendoriser `gpui-pre 0.3.12` (148 fichiers `.rs`, 8,6 Mo) sous `vendor/gpui-pre` avec `NOTICE` et le contrôle `fork-patch` de la CI, comme ADR-006 ; une entrée `[patch.crates-io]`. Aucune PR amont (décision utilisateur du 13/11). Une montée de gpui-kit (ADR-001) oblige à rejouer le patch.

## Proposition

(B) : 3 lignes contre ~50 sites et une règle dupliquée. À défaut, (A) en deux temps : règle 1 seule (calques), puis l'opacité.

## Conséquences (si accepté)

- `docs/constantes-rendu.md`, section texte : les deux règles et leurs sources CSS ci-dessus.
- Scènes des deux cas limites (ligne de menu survolée ; surface opaque sur fond translucide).
- Retrait de `--disable-lcd-text` des décors `_t2.ts`, `_t5a.ts`, `_t7a.ts` et recapture en LCD sous verrou, candidat et références dans la même PR.
