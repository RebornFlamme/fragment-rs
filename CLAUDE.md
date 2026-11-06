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
- **Aucune copie de source de Fragment** (dépôt privé, celui-ci est public) : fixtures et commentaires se réécrivent. `cargo xtask audit-sources --rev origin/task/<id>` en code 0 avant toute fusion (ADR-007).
- Commentaires en français, `★` pour ce qui a été appris chèrement. Commits `feat : …` / `fix : …` / `port : …` / `spike : …` / `docs : …`.
- `target/` et la base Beads vivent HORS OneDrive (`.cargo/config.toml`, `C:\dev\…`).

## Conventions Rust (§9 de program.md) et vérification
- Rust 1.99.1 (`rust-toolchain.toml`), édition 2024, resolver 3. `gpui-kit = "=0.8.1"` (tire `gpui-pre =0.3.10`), déclaré UNE fois dans `[workspace.dependencies]` : une crate écrit `gpui-kit.workspace = true` (ADR-001).
- Lints : `[lints] workspace = true` dans chaque crate (`unwrap_used`/`expect_used` refusés hors tests, `clippy.toml`). Erreurs `thiserror` dans les crates, `anyhow` dans l'app.
- Vérifier avant de pousser : `cargo fmt --all`, `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo test -p <crate>` (crates pures seulement en local). Le reste : `git push` puis `powershell -File scripts/ci.ps1 <PR>`.
- Scripts : Windows PowerShell 5.1 (pas de `pwsh` sur le portable) ; fichiers `.ps1` en UTF-8 AVEC BOM, sinon 5.1 lit les accents en ANSI.
- Pixels : `pixels/reference/` = Fragment (régénéré par `scripts/pixels-reference.ps1`, Vite de Fragment déjà lancé de préférence) ; `fragment-pixels` (feature `pixels` de fragment-app) produit `pixels/candidate/` ; `cargo xtask pixels` compare. Les « 22 scènes » de visual-diff = 11 scènes × 2 thèmes.
- Beads : la base vit dans `C:/dev/fragment-rs-beads/.beads` (fichier local `.beads/redirect`) ; `.beads-export/issues.jsonl` est l'image commitée que lisent les sessions cloud (ADR-002). Chaque tâche remplit le `parite.toml` de sa crate.
- ★ Équipiers en worktrees parallèles : chacun exporte son propre `CARGO_TARGET_DIR` (`C:/dev/fragment-rs-target-<tâche>`) — un dossier cible partagé casse la résolution des dépendances par chemin.
- ★ Fragment est un dépôt GitHub PRIVÉ : une session cloud le clone en lecture dans `/tmp/Fragment` si l'app GitHub de Claude y a accès (program.md §2, « La référence TS depuis le cloud ») ; jamais de copie de ses sources dans ce dépôt public.

## Budget (résumé, détail §2 de program.md)
- Abonnement Max : chef + orchestrateur seulement. Pas de vague au-dessus de 80 % de la fenêtre 5 h ; pause à 90 % ; part fixe de 30 points d'hebdo par semaine (8 par jour au plus), arrêt global à 85 % (§2).
- Équipiers : sessions **cloud** (crédits cloud), modèle Sonnet 5.5, y compris le mécanique (un groupe de modules par session). **Aucune clé API, pas de `port.yml`** (décision utilisateur 01/11). GitHub Actions compile et teste, rien d'autre.
- Brief ≤ 400 mots, chemins plutôt que code, équipier neuf par tâche.


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:1105d646 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

> **`program.md` l'emporte sur ce bloc** : profil team-maintainer — une tâche = une branche `task/<id>` = une PR poussée (commit + push font partie du travail), bead fermé par le chef après fusion ; la mémoire de projet reste dans `program.md`, `docs/journal.md` et les ADR (pas de `bd remember` à la place).

