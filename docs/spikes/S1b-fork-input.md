# S1b — Fork minimal de `gpui-base::input` pour le live preview

- Tâche Beads `fr-p2-s1b`, PR « fr-p2-s1b : sous-spike fork de gpui-base::input ». Suite de S1 (`docs/spikes/S1-editeur.md`, option B).
- Code : `vendor/gpui-base/` (+ `NOTICE`), `crates/fragment-markdown/tests/spike_s1b.rs` (7 tests headless), job CI `fork-patch`. Décision : `docs/decisions/ADR-006-fork-input.md`.
- Méthode : `gpui-base` 0.8.1 téléchargé de crates.io (sha256 vérifié), copié dans `vendor/`, patch ajouté (module `row_model` + points d'accroche courts dans `display_map/`, `element.rs`, `state.rs`, `movement.rs`), branché par `[patch.crates-io]`. Aucune compilation locale (disque plein) : tout est itéré par la CI ; le patch a compilé et passé du premier coup.
- Mesure : même plateforme de test que S1 (`NoopTextSystem`, chasse fixe 7,8 px, ligne 20 px, aucun pixel). Les valeurs viennent d'un run CI jetable (`windows-latest`, branche supprimée depuis), débogage ET `--release`. `test-windows` de la PR exécute les mêmes tests (verts).

## Verdict global : **GO** — les six critères sont verts ; fork n°1 retenu (ADR-006)

Taille du patch : **+531 / −30 lignes** sur 7 fichiers, dont 158 lignes en module nouveau (`row_model.rs`), 137 en bloc ajouté à `display_map.rs`, ~110 pour la mise en forme des plages masquées (`layout_hidden_lines`, calquée sur `layout_token_lines`). Le reste (≈ 125 lignes) est fait de remplacements d'une à six lignes (`line_height * rang` → `row_top(rang)`), chacun marqué `// ★ fragment-rs`. Le stock fait 33 000 lignes pour `input/` : le patch pèse 1,6 %. Les fichiers monolithiques `element.rs` (5 460 lignes) et `state.rs` (12 400) n'ont **pas** résisté : 19 points d'accroche suffisent parce que le code savait déjà (a) mettre une rangée en fragments dont les offsets restent en octets source (jetons inline), (b) accumuler des hauteurs par ligne (`LineLayout::size`). L'estimation du spike S1 (3 à 4 semaines) était pessimiste d'un ordre de grandeur pour ce périmètre.

API ajoutée (une seule) : `EditorState::set_row_model(hidden: Vec<Range<usize>>, line_heights: Vec<(usize, Pixels)>, blocks: Vec<(usize, Pixels)>, cx)`. L'hôte la rappelle après chaque changement de texte ou de curseur (comme les décorations) ; sans changement, le wrapper ne refait rien.

## Par critère

### (a) `#` masqué SANS TROU — GO
- Hypothèse : une plage masquée est un élément de largeur 0 pour le wrapper (mécanisme des jetons) et un fragment sans glyphes à la mise en forme.
- Essai : `diese_masque_sans_trou` (`# Titre\n\ncorps\n`, plage `0..2`).
- Mesure : x du « T » **45,6 px révélé → 30 px masqué** = x du début de ligne (`30 px` = marge du texte) ; largeur de `# ` révélé 15,6 px (2 × 7,8) ; révélation sous le curseur (modèle vidé) : x revient exactement à 45,6 px ; y de la ligne « corps » inchangé ; `value()` intact (la source n'est jamais touchée). S1 mesurait 45,6 px dans les deux états (trou).
- Verdict : GO. ★ Le texte visible garde ses décorations (couleur, graisse, fond : `has_background` propagé).

### (b) H1 à la hauteur dictée — GO
- Essai : `h1_a_la_hauteur_dictee` (modèle : ligne 0 à 36 px, `# ` masqué).
- Mesure : `range_to_bounds` du titre : hauteur **36 px** ; ligne suivante décalée de **36 px** ; la suivante de 20 px (lh) ; clic à 30 px sous le haut du titre → curseur 5 (dans le titre), clic à 38 px → curseur 8 (début du corps) ; Bas depuis le titre atterrit dans le corps.
- Verdict : GO. Géométrie, souris, défilement (`scroll_to`), curseur (hauteur 0,85 × rangée) suivent. ★ Limite : la hauteur de rangée est dictée, pas la TAILLE de police : un H1 est plus haut, pas plus gros (une taille par plage est l'extension naturelle du même module, non faite ici, voir ADR-006).

### (c) Bloc de 120 px atomique au clavier — GO
- Essai : `bloc_de_120_px_atomique`, deux variantes.
- Bloc SOUS une ligne (`blocks = [(1, 120 px)]` sur `a\nb\nc\nd`) : « c » repoussé de **140 px** (20 + 120) de « b », « d » à 20 px de « c » ; Bas : a → b → c (le bloc est franchi **d'un coup**, aucun arrêt dedans) ; Haut : c → b → a ; un clic dans le bloc tombe sur la ligne suivante (jamais dans de la source cachée).
- Bloc À LA PLACE d'une ligne (ligne « b » masquée + hauteur 120 px) : « c » à **120 px** de « b » ; a → (bloc) → c en deux pas ; source intacte.
- Verdict : GO. Atomique parce que le bloc est de la hauteur et non une rangée ; S1 mesurait l'inverse (Bas entrait dans la source cachée, octet 7 dans `7..36`).

### (d) IME (protocole de composition) — GO
- Essai : `ime_toujours_vert_avec_le_fork` : `# ` tapé, puis masqué ET ligne H1 36 px pendant la composition (`に` → `日本` → commit `日本語`).
- Mesure : plage marquée `2..3` puis `2..4` (UTF-16), après commit sélection `5..5` côté système et `11..11` côté éditeur : **identique à S1**. La plage masquée `0..2` n'a pas bougé pendant la composition (« 日 » au x du début de ligne).
- Verdict : GO. ★ Comme S1 : l'IME du système (fenêtre de candidats, TSF) n'est pas testable en headless ; à vérifier à la main avec le japonais dès que T3 a un exemple lançable. `bounds_for_range` (position de la fenêtre de candidats) suit désormais les hauteurs dictées (dette soldée par `fr-p3-fork-input`, ADR-006).

### (e) Annuler / rétablir — GO
- Essai : `annuler_et_retablir_avec_le_fork` : `# ab⏎cd`, modèle actif (`0..2` masqué, H1, bloc 40 px), Ctrl+Z, Ctrl+Y, puis frappe en tête de la ligne masquée.
- Mesure : annulation → `# ab\n`, rétablissement → `# ab\ncd` (exact) ; frappe en `0..0` → `x# ab\ncd`, la plage masquée se décale avec l'insertion (`adjust_inline_metrics` du wrapper), pas de panique.
- Verdict : GO.

### (f) Soft wrap et 10 000 lignes — GO
- Soft wrap (`soft_wrap_avec_le_fork`, 480 caractères sur 800 px, « mot » du début masqué, ligne « fin » à 30 px) : **7 rangées** (identique à S1), y du premier et du dernier caractère 8 / 88 px (identique), Bas suit la rangée visuelle (offset 100), « fin » à 30 px de haut.
- 10 000 lignes (`dix_mille_lignes_avec_le_fork`, 623 890 octets, **1 000 titres masqués à 30 px + 1 000 blocs de 40 px**) :

| | débogage | `--release` | S1 (stock, décorations) |
|---|---|---|---|
| pose du modèle (1 000 plages, ré-enroulement) + 1re image | 135,7 ms | 11,6 ms | — |
| défilement : moyenne / max par pas de 3 lignes | 25,2 / 33,7 ms | 2,9 / 3,8 ms | 16,6 / 23,3 (déb.) ; 3,9 / 5,6 (rel.) |
| frappe au milieu du document (moyenne / caractère) | 23,3 ms | 2,4 ms | 12,4 ms (déb.) ; 2,9 ms (rel.) |

  Lignes visibles après défilement : `240..261` (rendu virtualisé : 21 lignes seulement) ; un titre visible est masqué sans trou à 10 000 lignes et mesure 30 px.
- Verdict : GO. ★ Le coût en débogage est plus élevé (~1,5×) à cause du modèle de 2 000 entrées reconstruit à chaque projection (`rebuild_row_metrics`, O(k) par changement) ; en `--release` il est équivalent au stock. Les glyphes ne sont toujours pas mesurés (`NoopTextSystem`) : à rejouer avec l'exemple et un vrai texte avant T3.

## Régression du stock
Les 10 tests de `spike_s1.rs` (frappe, IME, décorations, case à cocher, calque de bloc, wrap, undo, coordonnées, 10 000 lignes) restent verts SUR LE FORK (job `test-windows`) : à modèle vide, toutes les méthodes du patch retombent sur `ligne_de_base × rang`.

## Job CI `fork-patch`
Télécharge gpui-base à la version lue dans `Cargo.lock`, vérifie le `sha256` (consigné dans `NOTICE`), compare chaque fichier à l'original, échoue si un fichier non déclaré diverge ou si la version vendored n'est plus l'épinglée, produit le diff (artefact `gpui-base-fork.patch`) et le ré-applique à une copie neuve de l'original. ★ Il a déjà servi : à son premier run il a signalé `.cargo_vcs_info.json` (fichier généré de l'archive, non vendored) comme divergent — corrigé, vert ensuite.

## À retenir pour T3
- Dicter le modèle APRÈS chaque `Change` ET chaque déplacement du curseur (règle `curseurActif`), avec une garde de comparaison pour ne pas rappeler `set_row_model` à l'identique ; tous les offsets en octets UTF-8 (conversion UTF-16 à la frontière, ADR-004), lignes logiques.
- Pas d'atomicité Gauche/Droite sur une plage masquée : le curseur fait un pas invisible (à régler dans l'adaptateur si Fragment l'exige : sauter la plage).
- Un widget de bloc se pose en calque aux coordonnées de `range_to_bounds` de la ligne d'ancrage + une rangée ; le bloc réserve la place, le calque dessine.
- Numéros de ligne, fond de ligne active et position IME ne connaissent pas encore les hauteurs variables (désactivés ou approximatifs dans le live preview).
