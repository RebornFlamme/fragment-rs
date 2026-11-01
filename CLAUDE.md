# fragment-rs — notes pour Claude

Réécriture de Fragment (clone d'Obsidian pour humains et agents) en Rust sur GPUI / gpui-kit.

**Lis d'abord `program.md`** : rôles, budget (trois voies), registre Beads, phases, portes, commandes, conventions. C'est la source de vérité de l'organisation. Le chef de projet le relit au début de chaque vague.

**La spec, c'est Fragment** : `C:\Users\julie\OneDrive\Documents\Projects\Fragment` — son `CLAUDE.md` (invariants et décisions, ~600 lignes), son code TS, ses 96 specs e2e dans `app/e2e/`. Ce dépôt est en **LECTURE SEULE** : aucune session de ce programme n'y écrit.

## Règles courtes
- Une tâche Beads = une branche `task/<id>` = une PR. `bd ready` → `bd update <id> --claim` → travail → PR → CI verte → fusion par le chef → `bd close`.
- Tests d'abord : un `#[test]` par cas TS, même nom en snake_case, mêmes données. En tête de chaque fichier porté : `// Port de app/src/…/x.ts`.
- `cargo test -p <crate>` en local pour les crates pures ; **jamais** `cargo build -p fragment-app` en local : c'est la CI (`gh run view <id> --log-failed`).
- `clippy -D warnings`, pas d'`unwrap` hors tests, pas d'`unsafe` sans ADR, pas de dépendance nouvelle sans la noter dans la PR, pas de fork de crate sans ADR.
- Le cœur (`fragment-core`, `-text`, `-layers`, `-artefacts`) ne dépend pas de `gpui-kit` et ne connaît aucune vue par son nom.
- Un jeton de thème, jamais un littéral de couleur hors `fragment-theme`.
- **Pixel perfect** : toute tâche de rendu livre ses scènes (`pixels/scenes/`), la référence capturée sur Fragment et un job `pixels` vert. Constantes de mise en page lues dans les CSS de Fragment (`docs/constantes-rendu.md`), jamais réestimées. Géométrie et couleurs en strict (0 pixel) ; seul l'anticrénelage des glyphes a une tolérance, suivie comme une dette.
- Commentaires en français, `★` pour ce qui a été appris chèrement. Commits `feat : …` / `fix : …` / `port : …` / `spike : …` / `docs : …`.
- `target/` et la base Beads vivent HORS OneDrive (`.cargo/config.toml`, `C:\dev\…`).

## Conventions Rust (§9 de program.md) et vérification
- Rust 1.99.1 (`rust-toolchain.toml`), édition 2024, resolver 3. `gpui-kit = "=0.8.1"` (tire `gpui-pre =0.3.10`), déclaré UNE fois dans `[workspace.dependencies]` : une crate écrit `gpui-kit.workspace = true` (ADR-001).
- Lints : `[lints] workspace = true` dans chaque crate (`unwrap_used`/`expect_used` refusés hors tests, `clippy.toml`). Erreurs `thiserror` dans les crates, `anyhow` dans l'app.
- Vérifier avant de pousser : `cargo fmt --all`, `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo test -p <crate>` (crates pures seulement en local). Le reste : `git push` puis `powershell -File scripts/ci.ps1 <PR>`.
- Scripts : Windows PowerShell 5.1 (pas de `pwsh` sur le portable) ; fichiers `.ps1` en UTF-8 AVEC BOM, sinon 5.1 lit les accents en ANSI.
- Pixels : `pixels/reference/` = Fragment (régénéré par `scripts/pixels-reference.ps1`, Vite de Fragment déjà lancé de préférence) ; `fragment-pixels` (feature `pixels` de fragment-app) produit `pixels/candidate/` ; `cargo xtask pixels` compare. Les « 22 scènes » de visual-diff = 11 scènes × 2 thèmes.
- Beads : la base vit dans `C:\devragment-rs-beads\.beads` (fichier local `.beads/redirect`) ; `.beads-export/issues.jsonl` est l'image commitée que lisent `port.yml` et les sessions cloud (ADR-002). Chaque tâche remplit le `parite.toml` de sa crate.
- ★ Fragment est un dépôt GitHub PRIVÉ : la voie C le monte via le secret `FRAGMENT_READ_TOKEN` (sinon pas de référence TS en CI).

## Budget (résumé, détail §2 de program.md)
- Abonnement Max : chef + orchestrateur seulement. Pas de vague au-dessus de 80 % de la fenêtre 5 h ; pause à 90 % ; semaine ≤ 10 + 90 × (fraction de semaine écoulée) %.
- Équipiers : sessions **cloud** (crédits cloud), modèle Sonnet 5.5. Mécanique : workflow `port.yml` (crédits API), un job par tâche.
- Brief ≤ 400 mots, chemins plutôt que code, équipier neuf par tâche.
