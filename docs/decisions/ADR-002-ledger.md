# ADR-002 — Registre des tâches : Beads hors OneDrive, export commité

- Statut : accepté (chef de projet, 01/11/2026).
- Contexte : `program.md` §3 impose Beads (`bd`, préfixe `fr-`). bd 1.3.2 stocke tout dans une base Dolt embarquée sous `.beads/`. Or le dépôt vit sous OneDrive (verrous, `EPERM`), et les équipiers des voies B (sessions cloud) et C (GitHub Actions) n'ont pas accès au disque du portable.

## Décision

1. **Base hors OneDrive.** La base réelle est `C:\dev\fragment-rs-beads\.beads` (créée par `bd init --prefix fr` avec `BEADS_DIR`). Le dépôt n'a qu'un fichier `.beads/redirect` LOCAL qui y pointe (mécanisme natif de bd ; non commité car il porte un chemin absolu de la machine, `.gitignore`).
2. **Une seule source d'écriture : le chef, sur le portable.** Création, `--claim`, notes, fermeture : `bd` en local.
3. **Export commité pour les autres voies.** `bd export -o .beads-export/issues.jsonl` est régénéré par `scripts/bootstrap.ps1` et `scripts/wave.ps1`, puis commité sur `main`. `port.yml` (voie C) et les sessions cloud (voie B) LISENT la tâche dans ce fichier ; ils ne modifient pas le registre. L'avancement remonte par les PR (`gh pr list`), que le chef reporte dans Beads (`bd update <id> --notes "PR #n"`, `bd close`).
4. **Identifiants lisibles.** Épics `fr-p0`…`fr-p4`, tâches `fr-p1-<groupe>` (création avec `--id`), pour que les branches `task/<id>` et les briefs restent parlants.
5. **Dépendances.** `fr-p1` dépend de `fr-p0` : `bd ready` ne rend la phase 1 qu'une fois la porte 0 franchie et `fr-p0` fermée. Dans la phase 1, `core-b` et `markdown`/`divers` attendent `core-a`, `layers`/`chat` attendent `text`, pour qu'aucune paire de tâches ouvertes ne partage un fichier (`lib.rs` de fragment-core notamment).
6. **Tables de parité sans conflit.** Chaque tâche remplit le `parite.toml` de SA crate ; `docs/parite-tests.md` est régénéré par le chef (`cargo xtask parite`) — sinon les 11 tâches écriraient le même fichier.

## Conséquences

- Un export périmé fait lire à un équipier une description ancienne : le chef exporte AVANT de lancer une vague (`wave.ps1`).
- Perte du portable = perte du registre vivant ; l'export commité en garde l'image à la dernière vague (suffisant pour reconstruire par `bd init --from-jsonl`).
