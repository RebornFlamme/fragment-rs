# ADR-012 — Le HTML du Markdown : traducteur natif écrit pour ce dépôt, SVG par le rastériseur de GPUI, pas de WebView par bloc

Statut : proposé (fr-t3-html, 11/11/2026). Mesures : `docs/spikes/S6-html-markdown.md`. Des pertes de fidélité y sont chiffrées : divergences à trancher par l'orchestrateur (program.md §8).

## Contexte

Fragment rend le HTML d'une note ou d'une réponse de l'agent dans le DOM de Chromium, après DOMPurify : `div` stylée, SVG (animé en SMIL, à la couleur du thème), `details` ouvrable, balises en ligne, `iframe`. program.md §5 (T3) prévoyait « rendu natif gpui-kit, politique de sanitize ». Le spike S6 a mesuré le rendu natif de gpui-kit 0.8.1 sur 12 fixtures, le SVG et le SMIL par le rastériseur de GPUI contre Chromium, et relu S2 pour une WebView par bloc.

## Décision

1. **Le HTML natif de gpui-kit tel quel est REFUSÉ.** `gpui-base::text::format::html` ne tient que 17 traits sur 34. Il ignore tout le CSS en ligne (14 déclarations sur 14), ne dessine pas le SVG et en laisse fuiter les `<text>` dans la note, n'a pas de `details` repliable, perd `sub`/`sup`/`kbd`, supprime les `<iframe>`, et garde les `href="javascript:…"`.
2. **Le rendu est un traducteur écrit pour ce dépôt** (`fragment-ui/src/md_html`, tâche `fr-t3-html-rendu`) : HTML assaini par la politique de `fragment-markdown::sanitize_html` AVANT toute analyse, analysé par html5ever, traduit en éléments GPUI (`div`, texte balisé, tableaux, `details` avec son état). Un lecteur de déclarations CSS couvre le sous-ensemble : couleur, fond, bordure, padding, rayon, police, alignement, `display: flex` avec `gap` et `flex`. Tout autre CSS est ignoré, comme l'est aujourd'hui tout le CSS dans le natif.
3. **Le SVG est peint par `SvgRenderer` de GPUI** (celui des `ImageFormat::Svg`) après substitution des variables du thème (`var(--…)`, `currentColor`) par les jetons de `fragment-theme`. Mesuré : 4,3 % des pixels au-delà de 16 (contours), taille exacte.
4. **Les animations SMIL sont jouées par un interpréteur minimal** : le SVG est figé à l'instant `t` puis rastérisé. Sous-ensemble de départ : `animate` numérique, `animateTransform` `translate`, `rotate`, `scale`, `set` ; interpolation linéaire des `values`, `repeatCount="indefinite"`. Mesuré : valeurs identiques à Chromium à 0,01 près, 1,35 ms par image (debug). Garde-fous pour la tâche : `ParsedSvg` conservé, 30 images/s au plus, seulement ce qui est visible.
5. **Pas de WebView par bloc.** Un bloc dans le flux d'une note défile, se rogne et passe sous des calques GPUI ; un popup WebView2 (S2) n'en est pas capable, est invisible au harnais pixels et se paie en fenêtre et en contrôleur par bloc. La WebView reste réservée à la vue HTML pleine page (T9).
6. **`<iframe>` : carte de repli** (lien et titre) dans la note. Un lecteur vivant serait une WebView popup activée au clic, dans le cadre commun des embeds (T9), pas ici.
7. **Le bouton « </> », la source au curseur et le commentaire caché** restent ceux de la spec (`markdown-html.spec.ts`) ; ils relèvent des décorations de l'éditeur, pas de ce traducteur.

## Conséquences

- Dépendance directe nouvelle probable : `html5ever` et `markup5ever_rcdom` (déjà au lock via `gpui-base`), à noter dans la PR de `fr-t3-html-rendu`. Aucun fork de plus.
- **Dettes de fidélité à remonter** (voir S6) : `<iframe>` rendue en carte ; animations CSS et `<style>` non jouées ; SMIL hors sous-ensemble ; anticrénelage SVG ; CSS hors sous-ensemble (`grid`, `transform`, filtres). Projection : 32 traits sur 34 contre 17 pour le natif ; seuls les traits SVG/SMIL sont mesurés, le reste est à confirmer par la tâche d'implémentation.
- `fr-t3-html-rendu` : le traducteur écrit ses propres tests (un `#[test]` par cas de `markdown-html.spec.ts`), reprend `tests/spike_s6/smil.rs` en module de `fragment-ui` et les fixtures de S6, puis supprime le spike.
- Repli, si le traducteur dépasse son budget : ne rendre nativement que `div`/`span`/tableau/SVG statique et mettre les blocs restants (flex, animés) en carte-lien « ouvrir dans une page HTML » vers la vue T9. La perte est alors chiffrée par S6 (§5).
