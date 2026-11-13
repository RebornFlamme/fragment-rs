# ADR-016 — Harness Claude : le binaire `claude` en processus enfant, protocole `stream-json`

Statut : **proposé**, verdict **GO** (fr-t7-harness-spike, 13/11/2026). Mesures : `docs/spikes/S9-harness.md`. Débloque `fr-t7-harness-claude`.

## Contexte

Fragment pilote l'agent par un processus Node (`sdkProcess.mjs`), qui appelle le Claude Agent SDK, qui lance à son tour le binaire `claude`. program.md §4 demande que fragment-rs parle au binaire directement, sans Node. S9 a rejoué depuis Rust ce que fait le SDK, contre le vrai binaire en 2.1.295 (celui du SDK de Fragment) et en 2.1.316 (installé). Tout passe : tour en flux, pensée résumée, outils, garde `PreToolUse`, carte, « toujours », commande en lecture seule forcée à la carte, reprise, fork à un point donné, interruption, image et PDF, isolement des réglages, prompt par `initialize`.

## Décision

### 1. Le processus

Un **processus `claude` par tour**, comme le SDK : on le lance, on envoie `initialize`, un message utilisateur, on sert le contrôle jusqu'au `result`, puis on ferme `stdin`.
- La suite d'une conversation part avec `--resume <id>`, un fork avec `--resume <A> --fork-session --session-id <B> --resume-session-at <last>`, une conversation neuve avec `--session-id <id>`. Fragment choisit l'id.
- C'est le plus simple, et il colle à Fragment : le prompt et les options (modèle, effort, politique, dossiers) sont relus à chaque tour.
- Optimisation mesurée, non retenue pour la v1 : garder le processus entre deux tours épargne 1,2 à 2,5 s de lancement par tour, puisque l'entrée stream-json accepte plusieurs tours. Elle exige de relancer à chaque changement d'option ou de prompt.

### 2. La ligne de commande

Elle est construite par une fonction pure et testée, à partir de `HarnessTurn` et de la politique compilée (`compile_claude`) :
- `-p --input-format stream-json --output-format stream-json --verbose --include-partial-messages` ;
- `--setting-sources= --strict-mcp-config` : ni réglages, ni CLAUDE.md, ni MCP ou connecteurs de l'utilisateur ;
- `--permission-prompt-tool stdio` et `--permission-mode <politique>` ;
- `--allowedTools` / `--disallowedTools` de la politique, `--add-dir` pour chaque dossier en plus ;
- `--model`, `--effort`, `--thinking=adaptive --thinking-display summarized` ;
- `--system-prompt-snapshot off` : ★ sans lui, une reprise garde le prompt du premier tour ;
- `--plugin-dir <dossier de l'agent>` et `Skill(<nom>)` dans les outils permis, pour les skills lus au lancement dans le dossier de l'agent de l'utilisateur (décision de l'orchestrateur : rien n'est publié).

Le prompt système passe par `initialize` (`systemPrompt`), jamais par la ligne de commande (32 k caractères au plus sous Windows).

### 3. L'environnement

Il hérite de celui de l'app, **moins tout `CLAUDE*` et `ANTHROPIC*`**. Le shim `fragment` passe en tête du `PATH`, sous la clé déjà présente (`Path` sous Windows). Aucune clé API : le binaire lit le login local. Le spawn utilise `CREATE_NO_WINDOW` sous Windows.

### 4. Le lecteur de flux

Trois fils par tour, sans dépendance nouvelle :
- **le lecteur** : `stdout` ligne à ligne, `serde_json::Value`, vers un canal. Une ligne non JSON est journalisée, jamais fatale ;
- **la fin de `stderr`** : 2 000 caractères au plus, c'est le message d'une mort ;
- **le contrôleur**, qui possède `stdin` et sert le canal :
  - message ordinaire → `events::to_agent_events` → `HarnessIO::event`. Le module est déjà porté et vérifié sur le flux réel, sans retouche ;
  - `control_request` `hook_callback` → `HarnessIO::guard` → `{}` pour Allow, `permissionDecision: "ask"` pour Ask, `"deny"` + raison pour Deny ;
  - `control_request` `can_use_tool` → `HarnessIO::request_permission` (`PermissionRequest` : outil, famille, entrée, titre, raison, plan pour `ExitPlanMode`) → `{"behavior":"allow","updatedInput"}` ou `{"behavior":"deny","message"}`. « Toujours » reste chez Fragment (`PermanentRule` + garde) ; on n'envoie jamais `updatedPermissions` de type `setMode` ;
  - une demande encore ouverte quand le tour finit ou est interrompu reçoit un **refus**, jamais rien ;
  - `result` → `Done`, ferme `stdin`, puis attend la sortie (tuée au-delà d'un délai).
- `HarnessRun::interrupt` écrit `{"type":"control_request","request":{"subtype":"interrupt"}}`. Le `result` suit en moins de 40 ms, en `error_during_execution` : le harness le sait et rend un `Done` d'arrêt, sans erreur montrée.

### 5. Les erreurs (→ `AgentEvent::Error` / `ErreurHarness`)

- Le processus sort avant `initialize` (option inconnue, UUID invalide, session absente) : code 1, et le message est la fin de `stderr`.
- `result` en `subtype: success` + `is_error: true` (modèle inconnu, non connecté, réseau) : le texte est dans `result`, comme le relève déjà `sdkProcess.mjs`.
- Une session absente ne se reprend pas. Le harness teste le transcript `~/.claude/projects/<cwd encodé>/<id>.jsonl` avant de choisir entre `--resume` et `--session-id`.

### 6. L'historique

`history()` lit ce même JSONL en Rust (lignes `user` et `assistant`) et le passe à `events::to_chat_entries`. Le SDK n'est pas nécessaire. Un fork garde les `uuid` de sa source jusqu'au point.

### 7. La version minimale

**2.1.295**, épinglée dans `engine_model` (`at_least`). Ce sont les deux versions mesurées. Le harness vérifie `claude_code_version` dans `system/init` et le journalise.

## Conséquences

- Plus de Node ni de SDK dans l'app. Le moteur reste celui de `fr-t7-moteur` : Claude Code installé, ou téléchargé et vérifié.
- Le protocole de contrôle n'est pas une API publique. `--thinking`, `--thinking-display` et `--resume-session-at` sont des options cachées. Trois garde-fous :
  - la version minimale ;
  - les tests du lecteur sur des transcripts réels (S9, réécrits en fixtures par `fr-t7-harness-claude`) ;
  - l'exemple `spike_harness`, à relancer à chaque montée de version du binaire.
- La réponse à `initialize` porte le compte de l'utilisateur (courriel, organisation) : jamais journalisée.
- ★ Pour l'orchestrateur : Fragment, qui relit `prompt.md` à chaque tour, ne passe pas `systemPromptSnapshot`. Une conversation reprise y garde donc le prompt de son premier tour (vérifié sur 2.1.295). C'est un écart probable dans Fragment, en lecture seule ici.

## Repli (si une version future casse le protocole)

On revient à l'architecture de Fragment : un petit processus Node lancé par l'app, qui appelle le SDK. Le protocole JSON sur stdio entre l'app et ce processus reprend les `op` de `sdkProcess.mjs`, réécrit dans nos mots. Le reste du harness Rust ne change pas : il garde `AgentEvent`, la garde et la carte, et ne remplace que le transport.

## Alternatives écartées

- **Le SDK Node (le repli)** : un runtime Node à livrer ou à trouver, et un étage de plus, pour un transport que Rust fait seul.
- **`-p` en texte, sans stream-json** : ni carte, ni garde, ni interruption, ni flux.
- **Un processus gardé par conversation** : mesuré et viable, mais il demande de relancer à chaque changement de prompt ou d'option. C'est une optimisation possible après la v1.
