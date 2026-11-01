# ADR-004 — Unité des positions de texte : l'unité de code UTF-16, comme Fragment

- Statut : accepté (chef de projet, 01/11/2026). Soulevé par la PR #2 (fr-p1-text), qui avait choisi l'octet UTF-8.
- Contexte : en TS, toute position (`EditorPosition`, `SurfaceText.toPos/toIndex`, plages de `rangeHighlights`, ancres d'artefacts, offsets du PDF et de la page HTML) est un index en unités de code UTF-16 (`String.length`). En Rust, `&str` s'indexe en octets UTF-8. Les deux coïncident pour l'ASCII seulement.

## Décision

**Toute position qui traverse un CONTRAT est un index en unités de code UTF-16** : types publics de `fragment-text` (et des crates qui en héritent : `-layers`, `-chat`, `-pdf`, `-html`, `-artefacts`, `-annotation`), données persistées (journal d'artefacts `.fragment/artefacts/…`, `workspace.json`), protocole CLI et API JS des plugins.

- `fragment-text` fournit LA conversion (module `utf16` : `utf16_len(&str)`, `byte_to_utf16(&str, usize)`, `utf16_to_byte(&str, usize) -> Option<usize>` qui refuse le milieu d'une paire de substitution ou d'un caractère) et chaque module qui découpe un `&str` passe par elle. Un type `Utf16Pos(usize)` n'est PAS imposé (coût d'ergonomie) ; la règle est documentée sur chaque champ `pos`/`from`/`to`.
- Les cas de test portés gardent EXACTEMENT les données du TS (un test TS avec « é » ou un émoji garde ses nombres).

## Pourquoi

1. **Fichiers par-dessus l'application** : un coffre ouvert par Fragment puis par fragment-rs doit garder ses ancres justes. Le journal NDJSON stocke des offsets ; en octets, chaque note accentuée décalerait toutes les ancres (la cascade de citation rattraperait, au prix d'un ¼ d'erreurs consommé pour rien).
2. **L'API JS des plugins** (S3, QuickJS) parle en `String.length` : pas de conversion à chaque appel.
3. **Parité des tests** : mêmes données, mêmes nombres, aucune « adaptation » des cas.

## Conséquences

- PR #2 fusionnée telle quelle, puis corrigée par la tâche `fr-p1-text-utf16` (fragment-text seulement) AVANT que `fr-p1-layers` et `fr-p1-chat` démarrent (elles en dépendent dans Beads).
- Les tâches en cours (`fr-p1-artefacts`, `fr-p1-pdf`, `fr-p1-agent`) suivent cette règle ; une position en octets dans une PR est un motif de refus.
