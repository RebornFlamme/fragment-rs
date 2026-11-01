# ADR-001 — Socle UI : gpui-kit 0.8.1 et gpui-pre 0.3.10 épinglés

- Statut : accepté (chef de projet, 01/11/2026), dans le cadre des décisions figées de `program.md` §0.
- Contexte : il faut une base GPUI avec dock, onglets, listes virtuelles, éditeur, Markdown/HTML, webview, runtime JS et tests headless. Le code de Zed (éditeur compris) est sous GPL-3, incompatible avec la licence MIT du dépôt.

## Décision

| Crate | Version | Licence | Rôle |
|---|---|---|---|
| `gpui-kit` | `=0.8.1` (crates.io, publiée le 26/10/2026) | Apache-2.0 | façade unique : `gpui_kit::*` = GPUI, `gpui_kit::component` = gpui-component, `gpui_kit::base` |
| `gpui-pre` + `gpui-pre-*` (macros, platform, windows, collections, util…) | `=0.3.10` (épinglé PAR gpui-kit 0.8.1 lui-même) | Apache-2.0 | GPUI publié par tranches, tiré transitivement |
| `gpui-webview`, `gpui-shell` | `0.8.1` disponibles | Apache-2.0 | S2 / S3, ajoutées par leurs spikes |

- Le workspace déclare `gpui-kit = "=0.8.1"` dans `[workspace.dependencies]` ; aucune crate ne dépend de `gpui-pre` directement (la façade le fournit, voir le README de gpui-kit). `Cargo.lock` est commité et la CI tourne en `--locked`.
- Toolchain : Rust stable `1.99.1` (`rust-toolchain.toml`), dernière stable au 01/11/2026. Édition 2024, resolver 3.
- L'éditeur de Zed n'est PAS vendorisé. L'éditeur live preview se construit sur les primitives de gpui-kit/GPUI (S1).
- Monter gpui-kit (et donc la famille gpui-pre) = une PR dédiée qui ne change que les versions et `Cargo.lock`, avec CI verte et scènes pixels inchangées.

## Constats faits en lisant les sources (01/11)

- `gpui_kit::open_window` pose lui-même la `Root` de gpui-base ; l'application fournit sa vue, pas une autre Root.
- `DockArea` vit dans `gpui-base` (modèle, persistance, drag) ; `gpui_kit::component::dock::DockSkin` est l'habillage. Un dock sans skin fonctionne mais ne dessine rien.
- ★ Capture d'image : `Window::render_to_image` n'existe que sous la feature `test-support` (gpui-kit → gpui-pre → gpui-pre-windows). Sous Windows, elle rend la scène dans une texture DirectX hors écran puis la relit. En revanche `current_headless_renderer()` (fenêtre sans plateforme) renvoie `None` sous Windows : la capture exige une VRAIE fenêtre (plateforme Windows) ; d'où le binaire `fragment-pixels` (feature `pixels`) qui ouvre une fenêtre normale et capture sa scène.

## Conséquences

- Licences : tout l'arbre UI est Apache-2.0, compatible MIT. Toute dépendance à licence non compatible remonte à l'orchestrateur (§8).
- Un fork de gpui-kit/GPUI ne passe que par `[patch.crates-io]` + PR amont + ADR (§10).
