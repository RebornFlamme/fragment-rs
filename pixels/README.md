# pixels/ — parité au pixel avec Fragment

Voir `program.md`, « Parité au PIXEL ». Une scène = un coffre + des actions + une capture
(1280 × 800, DPR 1, sombre ET clair).

| Dossier | Rôle |
|---|---|
| `scenes/<scene>.spec.ts` | Playwright : pilote **Fragment** par son harnais e2e (`readerHarness`), écrit `reference/<scene>-<thème>.png` |
| `scenes/<scene>.json` | La même scène pour le candidat Rust (`fragment-pixels`) |
| `scenes/_commun.ts` | Décor partagé (note + PDF en split + chat + conversation ancrée), gel caret/animations |
| `vault/` | Coffre de fixture, copié de `visual-diff.spec.ts` (le harnais l'écrit dans `%TEMP%`, jamais ici) |
| `reference/` | PNG capturés sur Fragment (commités) ; `candidate/` et `diff/` : générés, ignorés |

Régénérer : `powershell -File scripts/pixels-reference.ps1 [scène]` (hors CI, Fragment doit être présent).
Prérequis dans `Fragment/app` : `node_modules` et `dist-electron/`.

**Mode de lancement : Vite enveloppe, sur 5123.** Le serveur Vite est lancé par une config enveloppe
posée dans `pixels/` (pas par la config de Fragment) : il écoute le port **5123**, son cache va HORS de
Fragment (`C:/dev/fragment-rs-vite-cache`) et rien ne s'écrit dans l'arbre de Fragment. Si un serveur
tourne déjà sur 5123 (celui de l'utilisateur), il est RÉUTILISÉ tel quel, sans être arrêté. Voir
`docs/tranches/T1.md` §6.

## Schéma JSON
Racine : `scene`, `window {width,height,dpr}`, `settings_window` (1000 × 700), `themes` (`dark`, `light` ;
défaut `dark`), `vault`, `vue` (`app` par défaut, `mire`, `jouet`), `actions`, et :
- `depuis` : `"T1"`, `"T7"`… la tranche à partir de laquelle la scène existe. `fragment-pixels` ne la
  rejoue pas et `cargo xtask pixels` ne la compare pas avant (`TRANCHE_COURANTE`, dans
  `crates/fragment-app/src/pixels/actions.rs` ET `xtask/src/pixels.rs` : la même valeur). Absent = toujours.
  Les scènes à décor note/PDF/chat sont marquées T7 (le chat), `annotation` T8, `reglages-editor` T3,
  `reglages-community-plugins` T10, `reglages-hotkeys` T11. `reglages-appearance` reste en attente (T7)
  tant que Fragment n'a pas prouvé qu'elle ne dépend pas du décor.
- `sans_reference` : scène de PREUVE de la CI Rust (`fenetre-vide`, `mire`, `jouet-*`), sans équivalent
  Fragment : `xtask` n'en attend aucune référence, même sous `--exiger-references`.
- `pixels_attendus` : `[{x,y,rgb:[r,g,b]}]`, pixels que chaque capture doit porter (la scène s'auto-contrôle).

Une action inconnue, un champ manquant ou en trop, une touche invalide = ERREUR (scène, rang, cause).

| Action | Champs | Sens |
|---|---|---|
| `command` | `id`, `args?` | exécute la commande du `CommandManager` (celles de `fragment_ui::scene`). `args` est publié avant sur l'événement `pixels:args` (une `serde_json::Value`) : GPUI n'a pas d'arguments de commande |
| `open` | `path`, `split?` | événement `pixels:open` (décor T7) ; erreur si personne n'écoute |
| `layout` | `tree` | événement `pixels:layout` ; charge un arbre d'onglets de vues VIDES (forme ci-dessous) ; erreur si personne n'écoute |
| `click` | `x`,`y`, `button?` (`left`/`right`/`middle`), `count?`, `target?` (`main`/`settings`) | déplacement, puis `count` appuis/relâchements |
| `hover` | `x`,`y`, `target?` | déplacement du pointeur |
| `key` | `keys`, `target?` | une combinaison à la Playwright : `Enter`, `Control+Shift+P` |
| `type` | `text`, `target?` | caractère par caractère |
| `wait` | `ms` | pause |
| `wait-stable` | — | polices prêtes, 2 images, 150 ms, SANS bouger le pointeur (le garer : `hover 2,2`) |
| `drag` | `from{x,y}`, `to{x,y}`, `release?` (vrai), `steps?` (10), `button?`, `target?` | geste continu : appui, `steps` déplacements bouton enfoncé, relâchement si `release`. `release:false` laisse le pointeur maintenu pour la capture |

`layout.tree` = `{main: Noeud, left?: {collapsed?, width?}, right?: {…}}` (docks repliés par défaut) ;
`Noeud` = `{type:"tabs", tabs:N, current?:i, dimension?}` ou `{type:"split", direction:"vertical"|"horizontal",
children:[Noeud…], dimension?}`. Le même objet alimente `act.layout(arbre)` côté Fragment (`_t1.ts`).

La capture est implicite en fin d'actions, par thème (la fenêtre de réglages si `settings_window`).
`pixels:notice` et `doc-widget:open-anchored` sont à fournir côté Rust (Fragment les fait par JS).
Les scènes de réglages sont en 1000 × 700, pas 1280 × 800.

## Ce que fait `fragment-pixels`
`fragment-pixels <scènes> [sortie]` lit les JSON, saute les scènes d'une tranche postérieure, puis pour
chaque thème : installe le thème, ouvre la fenêtre (et celle des réglages), rejoue les actions par
`Window::dispatch_event` sur la fenêtre RÉELLE (un redessin entre deux événements : GPUI teste les cibles
sur la dernière image peinte), capture par `render_to_image` et contrôle taille et `pixels_attendus`.
Le lecteur vit dans `crates/fragment-app/src/pixels/` (`actions` : schéma pur ; `entree` : gestes ;
`capture` ; `fenetres` : mire, vue jouet, fenêtre de réglages) et est AUSSI une cible de test (`cargo test
-p fragment-app`) : le schéma et la vue jouet sont exercés sans GPU.

## Côté Fragment : `scenes/_t1.ts`
`sceneT1(nom, corps, arbre?)` : coffre `pixels/vault`, fenêtre 1280 × 800, layout remplacé par des onglets
de vues vides (docks repliés — le layout par défaut de Fragment contient l'explorateur, T2), rendu figé.
`corps` reçoit `act` (`command`, `layout`, `click`, `hover`, `key`, `type`, `wait`, `waitStable`, `drag`,
`relacher`) : les mêmes noms que le JSON. `_commun.ts` reste le décor des scènes T7+. ★ `_t1.ts` n'a pas
encore été exécuté (le lanceur Vite enveloppe vient de `fr-t1-references`).

## État
22 captures (11 scènes × 2 thèmes) écrites, déterministes (2 passes, MD5 identiques sur base et selection).
Le plugin `e2e-chat` du coffre est un double de test JS : le candidat n'a pas à l'exécuter, il lui faut
seulement l'état visuel (conversation « Bonjour », conversation ancrée).
Note : `note.md` exige des fins de ligne LF (ajouter `pixels/vault/** -text` en `.gitattributes` racine).
Dates : non figées ; aucun écart observé entre deux passes.
