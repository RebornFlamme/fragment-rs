# ADR-018 — Boîte de couleur native (premier `unsafe` de fragment-ui)

- Statut : proposé (équipier `fr-t11.4`, 13/11/2026). À accepter par le chef.
- Contexte : Fragment règle l'accent par `<input type="color">`, soit une pastille ronde qui ouvre la boîte de couleur du système. GPUI n'a rien d'équivalent, et dessiner un sélecteur (carré saturation/luminosité, barre de teinte) inventerait une interface absente de Fragment.

## Décision

Au clic sur la pastille, sous Windows, appeler `ChooseColorW` (comdlg32) par la crate `windows` 0.62.2, celle que `gpui-pre` 0.3.10 tire déjà (aucune crate nouvelle au lock). Features ajoutées à `fragment-ui`, cible Windows seulement : `Win32_Foundation`, `Win32_UI_Controls_Dialogs`, `Win32_UI_WindowsAndMessaging`.

- **`unsafe` isolé** dans une seule fonction, `boite_systeme` (`reglages/selecteur_couleur.rs`), commentée `SAFETY` : l'appel FFI, sur une structure `CHOOSECOLORW` initialisée sur place et qui ne survit pas à l'appel, plus `GetForegroundWindow` pour la fenêtre propriétaire.
- Le reste passe par le trait `BoiteCouleur` : la fenêtre ne connaît ni Windows ni la FFI, et les tests injectent une fausse boîte.
- Hors Windows : `boite_systeme` rend `None`, la pastille reste inerte (écart dans `parite.toml`).

## Alternatives écartées

- Sélecteur dessiné en GPUI : redessine l'interface de Fragment.
- `gpui-component::ColorPicker` : feature `component`, thème propre (ADR-001).
- `rfd` ou une crate de boîtes natives : `rfd` n'a pas de sélecteur de couleur ; une crate de plus pour un appel.
