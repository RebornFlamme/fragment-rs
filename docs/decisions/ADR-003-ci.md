# ADR-003 — CI : GitHub Actions, Windows + Linux, preuve pixels sur runner hébergé

- Statut : accepté (chef de projet, 01/11/2026) ; volet « pixels » CONFIRMÉ par le run 41306245205.
- Contexte : `program.md` §4 — pas de compilation lourde sur le portable (OneDrive, 10 agents). Dépôt public → minutes Actions gratuites.

## Décision

**`ci.yml`** (PR, push sur `main`, manuel ; `concurrency` par ref avec annulation) :

| Job | Runner | Commande |
|---|---|---|
| `fmt` | ubuntu | `cargo fmt --all -- --check` |
| `clippy` | ubuntu (+ paquets X11/Wayland/fontconfig/Vulkan de GPUI) | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| `test-linux` | ubuntu | `cargo test --workspace --exclude fragment-ui --exclude fragment-app` |
| `test-windows` | windows | `cargo test --workspace` |
| `check-app` | windows | `cargo check -p fragment-app` |
| `pixels` | windows | `fragment-pixels` ×2 puis `cargo xtask pixels` (preuve 0 pixel) ; artifact `pixels` |

- `Swatinem/rust-cache@v2` partout (clés séparées linux / windows / pixels / release). `sccache` seulement si le cache dépasse 10 Go.
- `CARGO_TARGET_DIR` est ramené dans l'espace de travail du runner (le `.cargo/config.toml` vise `C:/dev/…`, propre au portable).
- Toolchain lue dans `rust-toolchain.toml` (rustup est présent sur les runners). `--locked` partout.
- Lints d'atelier : `clippy::unwrap_used` et `expect_used` en `deny` pour tout le workspace ; `clippy.toml` les autorise dans les tests (`allow-unwrap-in-tests`, `allow-expect-in-tests`).

**`build.yml`** (push sur `main`) : `cargo build --release -p fragment-app --bin fragment` sous Windows, artifact `fragment-rs-win-x64` (zip : `fragment.exe` + LICENSE).

**Pas de workflow agent.** Décision utilisateur (01/11) : aucune clé API dans ce programme ; GitHub Actions ne fait que compiler et tester. Le portage se fait en sessions cloud (program.md §2).

**Pixels.** La capture passe par `Window::render_to_image` (feature `test-support` de GPUI) dans une vraie fenêtre : DirectX rend dans une texture hors écran puis la relit ; sur un runner sans GPU, l'adaptateur est WARP. Phase 0 : la scène `fenetre-vide` est capturée deux fois et les deux images sont comparées (attendu : 0 pixel), et le nombre de couleurs distinctes est journalisé (une image unie trahirait un rendu absent).

## Vérification

Le premier run vert de `pixels` sur `main` vaut preuve ; il est cité dans `docs/journal.md`. Si WARP ne rend pas (image unie, erreur DirectX) : ADR-004 « runner auto-hébergé » (`scripts/pixels-runner.ps1` sur le portable) avant la phase 3, sans bloquer les phases 1-2.

**Résultat (01/11, run 41306245205, commit 201fa0b).** Runner `windows-latest`, adaptateur « Microsoft Hyper-V Video » (pas de GPU : rendu logiciel). Écran passé en 1920×1080 par `Set-DisplayResolution` (sinon 1024×768 rognait la fenêtre en 1024×720 — constaté au run précédent). `fenetre-vide` : 1280×800, 1 couleur (dock vide, attendu) ; `mire` : 1280×800, 511 couleurs (aplat, rayon, trait de 1 px, glyphes anticrénelés) ; chacune comparée à sa seconde capture : **Strict, 0 pixel**. Pas besoin d'ADR « runner auto-hébergé ». ★ Piège corrigé : fermer la dernière fenêtre quitte l'application (code 0) — `fragment-pixels` garde ses fenêtres ouvertes, et la CI exige la présence de chaque capture.

