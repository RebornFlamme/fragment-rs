# ADR-007 — Aucune source du dépôt privé Fragment dans ce dépôt public

Statut : accepté (décision de l'orchestrateur, 05/11/2026). La purge de l'historique reste une décision de l'utilisateur.

## Contexte

`fragment-rs` est public ; Fragment est privé ; le §2 de `program.md` interdit toute copie. Deux copies de fixtures étaient connues. L'audit de tout le dépôt a montré que la fuite était plus large : 22 ports Rust reprenaient tels quels de longs blocs de commentaires d'en-tête du TS (invariants, décisions, diagrammes), parce que les briefs de la phase 1 demandaient de garder les commentaires en français sans interdire la reprise littérale.

## Décision

1. **Une fixture ou un commentaire se réécrit** : aucune copie de source de Fragment, ni code, ni commentaire, ni fixture.
2. **Audit obligatoire avant chaque fusion** : `cargo xtask audit-sources --rev origin/task/<id>` doit sortir en code 0. Il est local, car Fragment n'est pas sur les runners. Le chef le lance, et son code de sortie bloque la fusion.
3. **Critère** (abaissé de 8 à 6 lignes par l'orchestrateur le 06/11) : au moins 6 lignes consécutives identiques à un fichier de `Fragment/app/src`, `app/e2e` ou `packages`, indentation retirée, hors lignes vides et lignes d'accolades seules. Le rapport ne contient que des chemins et des numéros de ligne.
4. **Exceptions** : `xtask/audit-sources-exclus.txt`, avec une raison par ligne. Seule entrée : `LICENSE`, le texte standard MIT.
5. **Pas de réécriture d'historique sans décision de l'utilisateur.**

## Résultat de l'audit du 05/11/2026 (seuil de 8 lignes)

`cargo xtask audit-sources --historique` : 663 fichiers de Fragment indexés, 1 550 objets audités (toutes les références et les têtes de PR `refs/pull/*`).

Colonnes : fichier de ce dépôt ; source dans Fragment ; plus longue plage copiée, en lignes significatives ; premier commit qui la porte. Tout a été public dès sa poussée.

| Fichier | Source dans Fragment | Lignes | Premier commit |
|---|---|---:|---|
| `crates/fragment-agent/src/actions_model.rs` | `app/src/ui/views/agent/actions/actionsModel.ts` | 10 | a940806 (01/11) |
| `crates/fragment-agent/src/attachment_blocks.rs` | `app/src/ui/views/agent/attachmentBlocks.ts` | 9 | a48c0d6 (01/11) |
| `crates/fragment-agent/src/policy_model.rs` | `app/src/ui/views/agent/policy/policyModel.ts` | 8 | 22a0a21 (01/11) |
| `crates/fragment-agent/src/skills_model.rs` | `app/src/ui/views/agent/skillsModel.ts` | 8 | 3ea8ecb (01/11) |
| `crates/fragment-artefacts/src/anchor/mod.rs` | `app/src/ui/core/artefacts/anchor.ts` | 24 | dbb7de4 (01/11) |
| `crates/fragment-artefacts/src/artefact_manager.rs` | `app/src/ui/core/artefacts/ArtefactManager.ts` | 21 | 42539f9 (01/11) |
| `crates/fragment-artefacts/src/clock.rs` | `app/src/ui/core/artefacts/clock.ts` | 15 | 1e2c3af (01/11) |
| `crates/fragment-artefacts/src/device.rs` | `app/src/ui/core/artefacts/device.ts` | 11 | 6bb82e7 (01/11) |
| `crates/fragment-artefacts/src/fold.rs` | `app/src/ui/core/artefacts/fold.ts` | 15 | bff7987 (01/11) |
| `crates/fragment-artefacts/src/journal/compact.rs` | `app/src/ui/core/artefacts/journal/compact.ts` | 18 | 6d067d4 (01/11) |
| `crates/fragment-artefacts/src/journal/import.rs` | `app/src/ui/core/artefacts/journal/import.ts` | 19 | 6d067d4 (01/11) |
| `crates/fragment-artefacts/src/journal/reader.rs` | `app/src/ui/core/artefacts/journal/reader.ts` | 11 | 6d067d4 (01/11) |
| `crates/fragment-artefacts/src/journal/writer.rs` | `app/src/ui/core/artefacts/journal/writer.ts` | 13 | 6d067d4 (01/11) |
| `crates/fragment-artefacts/src/objects.rs` | `app/src/ui/core/artefacts/objects.ts` | 12 | 843c84b (01/11) |
| `crates/fragment-chat/src/chat_surface_model.rs` | `app/src/ui/views/chat/chatSurfaceModel.ts` | 11 | cbbb7a8 (01/11) |
| `crates/fragment-chat/src/follow_model.rs` | `app/src/ui/views/chat/scroll/followModel.ts` | 14 | cfaf672 (01/11) |
| `crates/fragment-cli/src/instance_index.rs` | `app/src/shared/cli/instanceIndex.ts` | 12 | 2c1db9b (01/11) |
| `crates/fragment-html/tests/fixtures/htmlBridge.js` | `app/src/shared/htmlBridge.ts` | 142 | 0cbe484 (05/11) |
| `crates/fragment-layers/src/gutter_stack.rs` | `app/src/ui/core/layers/gutterStack.ts` | 15 | 7ffbe66 (01/11) |
| `crates/fragment-pdf/src/pdf_editor.rs` | `app/src/ui/views/pdf/PdfEditor.ts` | 12 | c00b22a (01/11) |
| `crates/fragment-snap/src/snap_model.rs` | `app/src/ui/snap/snapModel.ts` | 15 | 5814b51 (01/11) |
| `crates/fragment-text/src/find_passage.rs` | `app/src/ui/core/editor/findPassage.ts` | 9 | a85147e (01/11) |
| `crates/fragment-text/src/viewport.rs` | `app/src/ui/core/editor/viewport.ts` | 8 | 103e824 (01/11) |
| `pixels/scenes/_commun.ts` | `app/e2e/visual-diff.spec.ts` | 19 | 199cb51 (01/11) |
| `pixels/vault/.fragment/plugins/e2e-chat/main.js` | `app/e2e/fixtures/chat-provider-plugin.cjs` | 76 | 199cb51 (01/11) |

Nature des copies :

- `pixels/vault/.fragment/plugins/e2e-chat/main.js` : le fichier entier (89 lignes). Sur `main` depuis le 01/11.
- `crates/fragment-html/tests/fixtures/htmlBridge.js` : `HTML_BRIDGE_SCRIPT` presque entier (270 lignes). Jamais sur `main` : seulement dans la PR #20 et sa branche, depuis le 05/11.
- `pixels/scenes/_commun.ts` : code de pilotage repris de `visual-diff.spec.ts`. Sur `main` depuis le 01/11.
- Les 22 autres : blocs de commentaires, aucune ligne de code. Sur `main` depuis le 01/11.

**Exposition** (API GitHub, 05/11) : 0 fork, 0 étoile, 2 vues sur 14 jours, 999 clones dont 191 uniques. Les clones comptent les runners (160 exécutions de CI) et les sessions de ce programme, sans qu'on puisse les séparer d'un tiers : on ne peut pas affirmer que personne d'autre n'a téléchargé ces fichiers.

## Corrections

Tous ces fichiers sont réécrits dans la PR de cette ADR, sauf `htmlBridge.js`, supprimé de la PR #20 au profit de `pont-minimal.js`. L'audit de l'arbre de `main` rend 0 copie à la fusion.

★ Non vérifié : la fixture `e2e-chat` et `_commun.ts` réécrits n'ont pas été rejoués sur Fragment, car Vite (port 5123) ne tournait pas et le script de référence refuse de le lancer. Les PNG de référence commités restent ceux de l'ancienne fixture. La prochaine exécution de `scripts/pixels-reference.ps1` doit donner des PNG identiques ; un écart viendrait de cette réécriture.

## Passage à 6 lignes (06/11/2026)

L'orchestrateur a demandé le seuil de 6 lignes et la correction des 5 plages de 7 lignes ou plus. Ce seuil en signalant 16, les 16 sont corrigées pour que l'audit rende le code 0 : 15 blocs de commentaires reformulés, et une liste de champs d'un littéral de structure réordonnée dans `artefact_manager.rs` (sans effet sur le comportement). Ces 16 plages restent elles aussi dans l'historique.

## Ce qui reste

- **Dans l'historique git** : toutes les versions d'origine de ces fichiers restent lisibles dans les commits ci-dessus et les têtes de PR (`refs/pull/*`, que seul le support GitHub peut supprimer). Rien n'a été réécrit ni poussé de force.
- **Sous le seuil** : sur `main`, après le passage à 6 lignes et la réécriture de 16 plages supplémentaires, `--seuil` trouve encore 12 plages à 5 lignes et 40 à 4 (chaque seuil incluant le précédent). Non corrigées.
- **Hors de portée de l'audit** : un port Rust est une traduction fidèle du TS par construction (structure, noms, données de test, chemins en tête de fichier), et les constantes de `docs/constantes-rendu.md` et les noms de `docs/contrats.md` viennent de Fragment. L'audit compare des lignes identiques et ne mesure pas cette dérivation, qui découle de la décision de rendre ce dépôt public.
