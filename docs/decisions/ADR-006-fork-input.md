# ADR-006 — Fork n°1 : `gpui-base::input` (live preview)

- Statut : accepté (équipier `fr-p2-s1b`, 01/11/2026), sur décision de l'orchestrateur (option B du spike S1). Preuve : `docs/spikes/S1b-fork-input.md`, PR « fr-p2-s1b : sous-spike fork de gpui-base::input ».
- Contexte : S1 a établi que l'éditeur stock de gpui-kit 0.8.1 ne peut pas être un live preview fidèle à Fragment : une décoration cache sans retirer (trou), toutes les rangées ont la même hauteur, pas de bloc de hauteur libre. Ces trois points sont dans la liste « pixel perfect » (`program.md` §0).

## Décision

`vendor/gpui-base/` = gpui-base **0.8.1** (la version qu'épingle gpui-kit 0.8.1, lue dans `Cargo.lock`, Apache-2.0, `LICENSE-APACHE` et en-têtes conservés) **+ un patch minimal**. Branchement : `[patch.crates-io] gpui-base = { path = "vendor/gpui-base" }` au `Cargo.toml` racine, et `exclude = ["vendor"]` (le fork n'est pas membre du workspace : ni `fmt`, ni `clippy`, ni ses propres tests). Pas de dépendance nouvelle ; `Cargo.lock` ne change que par la source de `gpui-base` (chemin local).

**Fork n°1 sur ≤ 2** (`program.md` §10). Le suivant éventuel est réservé (S2/S3 devraient tenir sans).

### Périmètre du patch (+531 / −30 lignes, 7 fichiers, `vendor/gpui-base/NOTICE`)

Une seule API publique ajoutée : `InputBaseState::set_row_model(hidden, line_heights, blocks, cx)`.

| Fonction | Où | Mécanisme |
|---|---|---|
| Masquage de plages **sans trou** | `display_map/display_map.rs`, `base/element.rs` (`layout_hidden_lines`) | La plage est un « élément » de largeur 0 pour le wrapper (le mécanisme `inline_metrics` des jetons, réutilisé tel quel) et un fragment sans glyphes à la mise en forme. Le texte suivant se décale ; les offsets restent en octets source ; la source n'est jamais modifiée. |
| Hauteur de rangée **dictée** par ligne logique | `display_map/row_model.rs` (nouveau), `text_wrapper.rs` (`LineLayout`) | `LineLayout` porte une hauteur de rangée optionnelle ; le modèle d'affichage convertit rangée ↔ ordonnée par sommes préfixées (`row_top`, `row_at_y`, O(log n)). Modèle vide = grille uniforme `ligne_de_base × rang`, comportement d'origine. |
| **Bloc** de hauteur libre, atomique au clavier | idem | Le bloc est de la HAUTEUR AJOUTÉE sous la dernière rangée d'une ligne, pas une rangée : le curseur ne peut ni y entrer ni s'y arrêter, Haut/Bas le traversent d'un coup. « À la place de » une ligne = ligne masquée + hauteur dictée (une rangée, un arrêt). |
| Points d'accroche | `element.rs` (défilement, fenêtre visible, curseur, sélection, icônes de pli), `state.rs` (hit-test, `scroll_to`, `range_to_bounds`), `movement.rs` | Remplacent `line_height × rang` par `row_top(rang)` et `line_height` par la hauteur de la rangée. Chaque ajout est marqué `// ★ fragment-rs`. |

**Dettes soldées** (tâche `fr-p3-fork-input`, +API `InputBaseState::set_line_font_sizes`) : (1) taille de police dictée par ligne logique (un H1 est plus GROS : `layout_hidden_lines` mis en forme au corps dicté, même chemin que les plages masquées) ; (2) gouttière (numéros, fond de ligne active) et panneau IME (`bounds_for_range`) suivent les hauteurs de rangée et les blocs ; (3) Gauche/Droite (et Maj+Gauche/Droite) franchissent une plage masquée d'un pas (`previous_boundary_over_hidden`/`next_boundary_over_hidden`). Un test headless chacun (`police_dictee_par_rangee`, `gouttiere_et_ime_suivent_les_hauteurs`, `gauche_droite_franchissent_la_plage_masquee`). Le dessin des numéros n'est pas observable en headless : il est couvert par construction (même arithmétique que le texte) et sera vu au premier scénario pixels du live preview.

**Dette restante** : le retour à la ligne (soft wrap) mesure encore au corps de base (un titre en gros caractères plus long qu'une rangée peut déborder avant de passer à la ligne) ; Retour arrière/Suppr ne sont pas atomiques sur une plage masquée ; les plages masquées et les lignes à police dictée ne se mettent pas en forme avec `expand_tabs`, les repères d'espaces ni les fonds de couleur de document ; le numéro de ligne garde le corps de base.

## Procédure de rebase (montée de gpui-kit)

La montée de gpui-kit est déjà une PR dédiée (ADR-001). Le fork en fait une étape de plus :

1. Le job CI `fork-patch` télécharge gpui-base **à la version épinglée** (`Cargo.lock`), vérifie la somme `sha256` de l'archive (consignée dans `NOTICE`), fait échouer la CI si (a) la version vendored n'est pas celle épinglée, (b) un fichier non déclaré dans `NOTICE` diverge de l'original, (c) le patch ne s'applique plus à l'original. La dette de rebase se voit donc dès la PR de montée.
2. Rebaser : télécharger le `.crate` de la nouvelle version ; recopier **tout** `src/` dans `vendor/gpui-base/` ; ré-appliquer le patch (`gpui-base-fork.patch`, artefact du job `fork-patch` de la PR précédente : `patch -p1 -d vendor/gpui-base < gpui-base-fork.patch`) ; corriger les rejets à la main (les points d'accroche sont des remplacements d'une ligne, marqués `★ fragment-rs`) ; mettre à jour la version et le `sha256` dans `NOTICE` ; faire passer `spike_s1.rs` ET `spike_s1b.rs` (ils sont la preuve que le patch fonctionne et que le stock n'a pas régressé).
3. Estimation : une demi-journée tant que `element.rs`/`state.rs` ne sont pas réécrits en amont ; si l'amont ajoute son propre support de hauteurs variables, supprimer le patch.

## PR amont

**Non ouverte (hors de portée), 05/11/2026.** Raisons constatées : (1) `longbridge/gpui-kit` est en v0.9.0 alors que le fork est en 0.8.1, et la mise en page du dépôt a changé (`crates/gpui-base/src/input/…` n'existe plus à ce chemin) : le patch devrait être réécrit contre une base que nous ne pouvons pas compiler ici (CI seulement) ; (2) `CONTRIBUTING.md` exige « une PR, une chose » alors que le patch porte quatre mécanismes liés (masquage sans trou, hauteurs et blocs, police par ligne, atomes au clavier) ; (3) le patch n'a pas encore été éprouvé sur DirectWrite (T3). À rouvrir à la montée vers 0.9.x (PR de montée de gpui-kit, ADR-001), en une série de PR courtes. Texte prêt à l'emploi :

> **input: let the host dictate per-line row heights, hidden source ranges and per-line font sizes**
>
> Editors that render rich text over a plain buffer (live-preview Markdown, for instance) need three things the input does not offer today: a source range drawn with zero width instead of a gap, rows whose height (and font size) varies per logical line, and a way to step over a hidden range with the arrow keys.
>
> This adds `InputBaseState::set_row_model(hidden, line_heights, blocks, cx)` and `set_line_font_sizes(sizes, cx)`. Hidden ranges reuse the inline-token width mechanism with width 0; row heights are projected to y offsets with prefix sums (`row_top` / `row_at_y`, O(log n)); blocks are extra height below a line, not rows (the caret cannot enter them). Line numbers, the active-line background and `bounds_for_range` use the same per-row heights. Left/Right (and their selection variants) cross a hidden range in one step.
>
> With no model set the behavior is unchanged: every method falls back to `line_height * row`. Offsets stay UTF-8 byte offsets of the unmodified source. Tests: headless tests for each behavior (hidden range without gap, H1 height and font size, atomic block, IME bounds, undo/redo, 10,000 lines).

## Conséquences

- Le live preview (T3) s'écrit sur `EditorState` + `set_row_model`, avec le modèle `compute`/`curseurActif` et la vue de calques du spike S1.
- `gpui-base` n'est plus tirée de crates.io : le bruit de CI est nul (même arbre de dépendances), mais toute montée de version est maintenant une opération manuelle chiffrée (ci-dessus).
- Option C (mise en page maison sur `gpui::TextSystem`) reste le repli si une version future de `element.rs`/`state.rs` rend le patch intenable ; elle n'est plus nécessaire à court terme.
