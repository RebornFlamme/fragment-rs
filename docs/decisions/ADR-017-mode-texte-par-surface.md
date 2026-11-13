# ADR-017 — Mode du texte par surface : ClearType sur fond opaque, gris dans un calque ; fork n°2 de gpui-pre

Statut : **accepté**, option (B), par l'orchestrateur (13/11/2026, transmis par le chef), avec les conditions de la section « Décision ». Tâche fr-p4.1.4.

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

## Décision

Option (B), aux conditions de l'orchestrateur, toutes obligatoires :
1. **Le patch se limite à `should_use_subpixel_rendering`** : `vendor/gpui-pre.patch`, +5 lignes dans `src/window.rs` (une garde `if self.element_opacity < 1.0 { return false; }` et son commentaire `// ★ fragment-rs (ADR-017)`). Aucun autre changement dans ce fork sans un nouvel ADR. **Son test tourne hors du fork, en CI** (voir « Le test du patch ») : le test interne d'abord écrit dans `window.rs` a été retiré, car `vendor/` est hors du workspace et il ne tournait jamais.
2. **Job CI `fork-patch`** : l'arbre `vendor/gpui-pre` doit être ÉGAL à l'archive de crates.io de la version épinglée (sha256 vérifié contre `NOTICE`), plus `vendor/gpui-pre.patch`, et rien d'autre (`diff -r`). Il échoue aussi si la version vendorisée n'est plus celle du `Cargo.lock` et de `[workspace.dependencies]`, et il affiche la dette de rebase (dernière version publiée).
3. **NOTICE Apache-2.0 et provenance** (`vendor/gpui-pre/NOTICE` : version 0.3.12, URL et sha256 de l'archive). `cargo xtask audit-sources` exclut ce dossier par une exclusion nommée (`vendor/gpui-pre/`, entrée de dossier).
4. **Aucune PR amont** : décision de l'utilisateur du 13/11/2026.
5. **Contre-épreuves de scène** : un onglet au repos (opacité 0,6) en gris, le même texte à opacité 1 (onglet actif) en ClearType sur surface opaque ; patch retiré, la scène passe au rouge. ★ Remplacement (chef, 13/11) : `.clickable-icon` ne porte que des SVG (gris des deux côtés, ne prouve rien) ; le second texte atténué retenu est `.view-header-title-parent` (`workspace.css:779`, opacité 0,6), voir « Le test du patch ».
6. **Plafond : deux forks au plus** (`gpui-base`, ADR-006 ; `gpui-pre`, ici).

★ La version : gpui-kit 0.9.1 épingle `gpui-pre =0.3.12` (Cargo.lock) ; c'est elle qui est vendorisée.

## Le test du patch (CI)

★ Un test headless ne peut pas juger ce patch : la plateforme de test de GPUI refuse toujours le ClearType (`TestWindow::is_subpixel_rendering_supported` rend `false`), avec ou sans patch. Le test tourne donc sur la vraie plateforme Windows, dans le job `pixels` : une scène déclare `modes_texte` (zones et mode attendu, `gris` ou `cleartype`), et `fragment-pixels` lit dans la scène peinte le genre de chaque sprite de glyphe (`subpixel_sprites` = ClearType, `monochrome_sprites` = gris). Chaque zone doit contenir au moins un glyphe, tous du mode attendu ; sinon la capture échoue et le job passe au rouge (`capture::verifier_modes_texte`, test unitaire `le_mode_du_texte_est_controle_par_zone`).

Les zones, relevées sur les références de Fragment (texte gris : aucun pixel chromatique ; ClearType : 38 à 86 % de pixels chromatiques) :
- `t1-onglets` : onglets au repos (opacité 0,6) **gris**, onglet actif (opacité 1, surface opaque) **ClearType**. Patch retiré, les onglets au repos sortent en ClearType : rouge.
- `t1-menu` (cas limite 1, texte sur le fond opaque d'un calque composé) : **gris**.
- `t1-modale` (cas limite 2, surface opaque au-dessus d'un fond translucide, le voile) : **gris**.

Le chemin `.view-header-title-parent` n'apparaît que dans `t2-fil-ariane` (tranche T3, pas encore active) : sa zone y sera ajoutée à l'activation de T3.

## Mise en œuvre de la règle (sans fork)

`fragment_ui::rendu::mode_texte` :
- `installer(cx, opaque)` : le mode de base de l'App devient le gris ; `opaque` (ClearType dans `fragment-pixels`, celui du système dans l'app) est gardé dans un global.
- `surface_opaque(e)` : peint `e` dans le mode opaque, puis remet le mode d'avant. Posé à la racine de la coquille, de la fenêtre de réglages et du décor des flottantes.
- `pile(fond, calques)` : le fond en surface opaque, chaque calque en gris. La coquille y range la barre d'état, les notices, le menu, l'infobulle et la modale.
- Les dessins différés (`deferred` : menus, infobulles, listes déroulantes) sont peints après l'arbre, donc dans le mode de base : gris.

La règle et ses sources CSS sont dans `docs/constantes-rendu.md`, section « Texte ».
