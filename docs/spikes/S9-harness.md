# S9 (harness) — Le binaire `claude` piloté en `stream-json` depuis Rust

Tâche `fr-t7-harness-spike`. Code : `crates/fragment-agent/examples/spike_harness.rs`. Décision : `docs/decisions/ADR-016-harness-claude.md`.

Verdict : **GO**. Tout ce que Fragment obtient du Claude Agent SDK passe aussi par le binaire seul, piloté par un processus enfant, JSON par lignes et protocole de contrôle. Rien ne manque, sur 2.1.295 comme sur 2.1.316.

## Question

Fragment lance un processus Node (`sdkProcess.mjs`) qui appelle le SDK, et le SDK lance `claude`. Peut-on supprimer l'étage Node et parler au binaire directement ? Le SDK ne fait que du transport : il écrit des options en ligne de commande, envoie `initialize` (hooks, prompt système), relaie `can_use_tool` et `hook_callback`, et envoie `interrupt`. La lecture de `sdk.mjs` (0.3.295, lecture seule) donne la liste des options qu'il passe. Le spike les rejoue depuis Rust.

## Montage

- `std::process::Command` lance `claude` avec `stdin`, `stdout` et `stderr` en tuyaux. Un fil lit `stdout` ligne à ligne vers un canal (`serde_json::Value`). Un fil garde `stderr`. Le fil principal écrit sur `stdin` et sert les demandes de contrôle. Il n'y a ni dépendance nouvelle ni `async`.
- Options communes, celles du SDK :
  - `-p --input-format stream-json --output-format stream-json --verbose --include-partial-messages` ;
  - `--model haiku` ;
  - `--setting-sources=` et `--strict-mcp-config` (l'isolement) ;
  - `--permission-prompt-tool stdio --permission-mode default` ;
  - `--tools Read[,Write,Bash]` et `--allowedTools Read`.
  - Selon le tour : `--session-id`, `--resume`, `--fork-session --resume-session-at`, `--thinking=adaptive --thinking-display summarized`, `--effort`.
- Le prompt système passe par `initialize` (`systemPrompt: [texte]`), comme le SDK, et non par la ligne de commande : pas de limite de 32 k caractères sous Windows. Le prompt est **factice**, écrit pour le spike (décision de l'orchestrateur du 13/11), avec un code de test pour savoir lequel le modèle a reçu.
- ★ **Environnement** : toutes les variables `CLAUDE*` et `ANTHROPIC*` héritées sont retirées. Le spike tourne lui-même dans une session Claude Code, qui exporte entre autres `CLAUDECODE`, `CLAUDE_CODE_SIMPLE` (le mode `--bare`, qui ignore le login OAuth) et une URL de proxy. Le binaire lit alors le login local de l'utilisateur. Le programme n'a aucune clé API.
- Le dossier de travail est jetable (`C:/dev/s8-bac`). Les seuls outils sont Read, Write (dans le bac) et Bash, toujours refusé. Le modèle est `haiku`. Une passe complète fait 30 tours, pour environ 0,005 $ en équivalent API sur l'abonnement.

## Mesures (Windows 11, 2.1.316 ; 2.1.295 = le binaire livré avec le SDK de Fragment)

| Point | Résultat | Chiffres |
|---|---|---|
| Lancement → réponse à `initialize` | OK | 1,2 à 2,5 s (2,4 s sur 2.1.295) |
| Tour de texte | OK, `text-delta` en flux | 1er évènement 0,9 à 2,1 s ; `result` 1,3 à 1,5 s |
| 2e tour dans le même processus | OK (entrée stream-json multi-tours) | 1er texte 1,1 à 1,6 s ; aucun coût de lancement |
| Pensée résumée | OK : `thinking_delta`, puis texte | 1re pensée 1,2 s ; 19 à 22 deltas, ~280 car. |
| Outils (Read) | OK : `assistant` (tool_use) puis `user` (tool_result) | tour à 2 appels : 2,5 à 3,6 s |
| Garde `PreToolUse` (`hook_callback`) | appelée avant CHAQUE outil, Read compris | `{}` = règles du binaire |
| Garde : `deny` | l'outil ne s'exécute pas, aucune carte | le modèle lit « PreToolUse:Read hook error: <raison> » (`is_error`) |
| Garde : `ask` sur `ls` (lecture seule) | ★ carte forcée (`can_use_tool`) | réponse `deny` : rien n'est exécuté |
| Carte : refus | `{"behavior":"deny","message"}` → tool_result en erreur | fichier non écrit |
| Carte : autorisation | `{"behavior":"allow","updatedInput"}` | fichier écrit |
| Carte : « toujours » | `updatedPermissions: [addRules Write, session]` → plus aucune carte pour Write | ★ la suggestion du binaire est `setMode: acceptEdits`, trop large |
| Session neuve à id choisi | `--session-id <uuid>` → `result.session_id` identique | |
| Reprise (`--resume`), autre processus | OK, la mémoire est là (« ALBATROS, BALEINE ») | lancement → `result` 2,9 à 7,8 s |
| Fork (`--resume A --fork-session --session-id B --resume-session-at <uuid>`) | OK : B a l'historique de A jusqu'au point, pas après | B ne connaît que ALBATROS ; A reste intact |
| Relecture (`history`) | `~/.claude/projects/<cwd encodé>/<id>.jsonl` lu en Rust → `to_chat_entries` | 4 bulles. B garde les mêmes `uuid` que A jusqu'au point |
| Interruption (`control_request` `interrupt`) | OK | réponse de contrôle en 5 à 9 ms ; `result` en 20 à 37 ms (171 ms une fois) |
| Après une interruption | le processus vit, le tour suivant répond | |
| Image PNG + PDF en blocs `image` / `document` (base64) | OK : « rouge », « PERROQUET » | 1,4 s |
| Isolement | `mcp_servers: []`, aucun réglage ni CLAUDE.md, outils = `--tools` | restent les plugins et skills INTÉGRÉS au binaire, inaccessibles sans l'outil Skill |
| Prompt par `initialize` | appliqué (le modèle donne le code du prompt) | |
| ★ Prompt à la reprise | **instantané** par défaut : une reprise garde le prompt du 1er tour | `--system-prompt-snapshot off` → le nouveau prompt est vu (2.1.295 et 2.1.316) |
| Fermeture de stdin → sortie | code 0 | 0,5 à 1,4 s |

### Codes d'erreur

| Cas | Comportement |
|---|---|
| Option inconnue | code 1 en 0,3 s, avant `initialize` ; stderr `error: unknown option '…'` |
| `--session-id` qui n'est pas un UUID | code 1 en 0,5 s ; stderr `Error: Invalid session ID. Must be a valid UUID.` |
| `--resume` d'une session absente | code 1 en ~2 s, avant `initialize` ; stderr `No conversation found with session ID: …` |
| Modèle inconnu | `initialize` OK, puis un `result` en **`subtype: success` + `is_error: true`**, le texte dans `result` ; puis code 1, stderr `[claude-code:unrecognized_model]` |
| Tour interrompu | `result` en `subtype: error_during_execution`, `is_error: true`, `errors: ["[ede_diagnostic] …"]` ; le processus continue |

## Constats pour `fr-t7-harness-claude`

1. **`events.rs` tient le flux réel** sans retouche : texte, pensée (`start` puis deltas), outils, résultats, usage, `done`. Sur le transcript disque (lignes `user` et `assistant`), `to_chat_entries` rend les bulles et les repères `last`. `fork_point` s'applique tel quel.
2. **Un tour interrompu finit en `is_error`**. Le harness sait qu'il a envoyé `interrupt` : il ne doit pas montrer `[ede_diagnostic]` comme une erreur. Il faut aligner sur ce que Fragment affiche après « Arrêter » (la spec `chat`).
3. **`--system-prompt-snapshot off` est obligatoire** si le prompt est relu à chaque tour. ★ Fragment ne le passe pas (ni `systemPromptSnapshot` dans `initialize`). Une conversation reprise y garde donc le prompt de son premier tour, avec 2.1.295 aussi. C'est à signaler à l'orchestrateur : un écart possible dans Fragment, qui est en lecture seule ici.
4. **« Toujours »** se règle chez Fragment, par `PermanentRule` et la garde. Le harness n'a pas besoin des `updatedPermissions` du binaire. S'il s'en sert, ce doit être par `addRules`, jamais par la suggestion `setMode`.
5. **Existence d'une session** : Fragment appelle `getSessionInfo`, le harness Rust teste le fichier `~/.claude/projects/<cwd>/<id>.jsonl`. Le chemin est encodé ainsi : tout caractère non alphanumérique devient `-`, d'où `C:\dev\s8-bac` → `C--dev-s8-bac`. Trois cas restent à vérifier : les chemins longs, les chemins non ASCII et `CLAUDE_CONFIG_DIR`. Sinon `--resume` sur une session absente, c'est code 1 et pas de tour.
6. **Options cachées** : `--thinking`, `--thinking-display` et `--resume-session-at` n'apparaissent pas dans `--help`. Le SDK les passe pourtant, et elles marchent sur les deux versions. C'est le principal risque de dérive, couvert par la version minimale et les transcripts.
7. **Sécurité des transcripts** : la réponse à `initialize` porte le COMPTE (courriel, organisation, abonnement). Le spike le masque avant d'écrire. Le harness ne doit jamais le journaliser.

## Transcripts réels

Seize fichiers `<bac>/transcripts/<scénario>.jsonl`, hors dépôt, sur le portable (`C:/dev/s8-bac/transcripts/`). Ce sont NOS prompts seulement. Les lignes `>` vont vers le binaire, les lignes `<` en viennent. Le compte est masqué et le dossier de l'utilisateur remplacé par `~`. Ils sont régénérés par `cargo run -p fragment-agent --example spike_harness`. `fr-t7-harness-claude` y choisit ses fixtures et les relit avant tout commit : chaque ligne `system/init` liste les skills et plugins intégrés, inutiles dans une fixture.

## Reproduire

```text
cargo run -p fragment-agent --example spike_harness                    # tous les scénarios, claude du PATH
cargo run -p fragment-agent --example spike_harness -- permissions     # un seul
cargo run -p fragment-agent --example spike_harness -- texte --claude <chemin\claude.exe> --bac C:/dev/s8-bac-295
```

Scénarios : `texte pensee permissions sessions instantane interruption pieces erreurs`. La CI compile l'exemple (clippy `--all-targets`) et ne le lance jamais.
