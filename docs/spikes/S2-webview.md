# S2 — Pages HTML vivantes (gpui-webview 0.8.1 / WebView2)

Tâche `fr-p2-s2`, PR #20. Verdict global : **GO-AVEC**. Une page du coffre vit dans une webview (JS, pont, thème,
sandbox : tout est mesuré et passe), MAIS `gpui-webview` tel quel ne montre RIEN sous Windows (la page est
invisible sous le rendu GPUI) et le « calque GPUI par-dessus » n'existe pas. Il faut héberger la webview dans un
**popup possédé** et dessiner les menus/calques comme **fenêtres GPUI distinctes** (prouvé au pixel, voir §6).

Code : `crates/fragment-html/tests/spike_s2.rs`, `tests/spike_s2/scenario.rs` (scénario partagé), `examples/spike_s2.rs`,
fixtures `tests/fixtures/` (dont `htmlBridge.js`, `HTML_BRIDGE_SCRIPT` recopié tel quel de `app/src/shared/htmlBridge.ts`).
Captures RÉELLES du bureau du runner (CopyFromScreen) : artifact `spike-s2-captures` du workflow `spike-s2`
(run 42038930016) — `1/2` calque GPUI et popup layered, `3` fenêtre nue, `4` popup possédé, `5` menu GPUI au-dessus.
`unsafe` : FFI Win32 minimale (user32/gdi32/kernel32) dans `scenario.rs::win` + un `HasWindowHandle` nu ; spike seulement.

## 0. Dépendances : gpui-fast ?

- **Hypothèse** : `gpui-webview` exige `gpui-fast` (autre GPUI) — condition 5 de S3.
- **Essai** : lecture de `gpui-webview-0.8.1.crate` (`Cargo.toml`, `lib.rs`, `composition.rs`) ; compilation en CI.
- **Mesure** : dépend de `gpui-kit 0.8.1` (`default-features = false`), `lb-wry 0.53.3`, `anyhow`, `log`. `gpui-fast` est une
  feature **facultative** (`gpui-fast = ["dep:objc2", "dep:objc2-foundation"]`), et son code (`composition.rs`, `linux.rs`)
  ne couvre que **macOS et Linux**. Une assertion de compilation impose seulement que `gpui-kit` et `gpui-webview` soient
  d'accord sur `gpui-fast`.
- **Verdict : GO.** `gpui-webview = "=0.8.1"` sans feature, un seul GPUI (celui de gpui-kit), compile et tourne sur
  `windows-latest` (aucune dépendance `gpui-fast`). Mais ★ **sous Windows, `gpui-webview` n'a aucun code de composition** :
  c'est un HWND enfant brut (voir §6). `gpui-fast` ne règle donc pas le problème sans un adaptateur Windows à écrire.
  À tracer dans T9 : `lb-wry` (wry de Longbridge) ne propose pas non plus `CreateCoreWebView2CompositionController`.

## 1. Une page du coffre dans une feuille, JS qui tourne

- **Essai** : fenêtre GPUI réelle (900×600), `WebView::build` (WebView2), protocole `fragment-local` (racine
  `tests/fixtures/vault`, ressources web seulement via `page_servable` de `fragment-text`, pont injecté par
  `inject_html_bridge`), page 500×300 posée dans une `div` GPUI.
- **Mesure** : le script de la page calcule `6 * 7`, l'hôte relit `"42"` par `evaluate_script_with_callback`. Premier
  message `height` du pont reçu **142 ms** après l'ouverture de la fenêtre (runner `windows-latest`, WebView2 à chaud).
  Tout le scénario (2 fenêtres, 5 captures PowerShell) : ~30 s, dont ~12 s d'attentes voulues.
- **Verdict : GO** (la webview fonctionne ; sa VISIBILITÉ est le point §6).

## 2. Pont `postMessage` (hauteur, lien, touche)

- **Hypothèse** : le script de Fragment marche tel quel. **Faux, en une adaptation de deux lignes** : le pont est écrit
  pour une IFRAME (`if (window.parent === window) return;` puis `window.parent.postMessage`). Une webview de premier
  niveau (wry) est son propre parent : le script s'arrêterait à la 1re ligne. `adapt_bridge` retire la garde et remplace
  `envoyer` par `window.ipc.postMessage(JSON.stringify(m))` (test `le_pont_de_fragment_est_recopie_et_adaptable`).
- **Mesure** : reçus côté Rust, JSON `{"fragmentHtml":1,…}` : `hello`, `height` (300), `text`, `layout`, `link`
  (`href = http://fragment-local.localhost/note.md`, clic JS sur `<a>`), `key` (`Ctrl+K` rejoué dans la page).
  Non vus : `selection`, `rects`, `anchor`, `wheel` (même mécanique, non exercés) et le sens app → page (`theme`,
  `rects`) : il passerait par un `MessageEvent` fabriqué avec `source: window` (la page compare à `parent`).
- **Verdict : GO.** Le pont se garde, adapté. Le choix « iframe sandboxée dans une page hôte » (parité stricte avec
  Fragment) n'a pas été essayé ; le HTML hôte relaierait `message` vers `window.ipc`.

## 3. Variables de thème avant la 1re image

- **Essai** : le thème part dans l'URL (`?fragment-theme=<JSON encodé>`), que le pont lit à sa 1re exécution (injecté en
  tête de `<head>`). Le premier `<script>` de la page lit `--background-primary`.
- **Mesure** : `"#1e90ff"` lu avant tout autre script, donc avant la 1re image ; le fond bleu de la page (capture 3/4/5)
  vient de la règle `:where(html){background:var(--background-primary)}` du pont. Pas de clignotement mesuré
  (non quantifié : pas de capture image par image).
- **Verdict : GO.**

## 4. Sandbox (pas d'accès fichier hors coffre)

- **Essai** : la page sonde elle-même ses accès ; le protocole `fragment-local` résout via `resolve_in_vault`
  (`..`, `\`, `:`, `%2e`/`%2f` décodés, liens symboliques par `canonicalize`) et `page_servable` (pas de `.md`).
- **Mesure** : `fetch('file:///C:/Windows/win.ini')` bloqué par WebView2 ; `/note.md` 403 ; `/..%2f..%2fCargo.toml` 403 ;
  fichier absent 404 ; `/ok.txt` 200. Test unitaire : 6 chemins d'évasion refusés.
- **Verdict : GO.** ★ Écart avec Fragment : la page est ici un document de premier niveau sur l'origine
  `fragment-local.localhost`, pas une iframe sans `allow-same-origin` : elle atteint `window.ipc` (l'hôte doit n'accepter
  que les messages `fragmentHtml`, comme l'app TS) et lit tout le coffre servable de SA racine. Non testé : réseau
  sortant (`fetch('https://…')`) — à couper par politique WebView2 (CSP injectée ou filtre de requêtes).

## 5. Capture réelle : méthode

`CopyFromScreen` (PowerShell) sur le bureau interactif du runner + `GetPixel` sur le DC de l'écran (pixels du bureau
composé par DWM, popups layered compris). Le témoin (carré GPUI rouge hors webview) vaut (255,0,0) : le bureau est
lisible. Le runner : Windows Server 2025, 1024×768, 96 dpi (échelle 1,0), « Test Mode ». `render_to_image` ne voit pas un
HWND enfant (confirmé par construction : tout ce qui suit est mesuré sur l'écran).

## 6. LE POINT DUR : un calque PAR-DESSUS la page

### 6.0 Constat de base : la webview d'un `gpui-webview` est INVISIBLE sous Windows
- **Essai A** (fenêtre opaque, `gpui_webview::WebView` dans une `div` à (40,60) 500×300, carré GPUI rouge chevauchant, 
  déclaré APRÈS la webview).
- **Mesure A** : `innerWidth×innerHeight` côté page = 500×300 (bornes posées, JS vivant) MAIS le pixel de la zone
  « webview seule » vaut (32,32,32) = le fond GPUI ; le carré GPUI est rouge ; capture 1 : aucune page.
- **Essai B** (même scène, `WindowBackgroundAppearance::Transparent`, sans fond GPUI) : la zone vaut (255,255,255) (blanc :
  le fond de la fenêtre, PAS le bureau — `Transparent` ne donne pas de transparence par pixel ici) ; toujours aucune page.
- **Contrôle** : la MÊME webview (même protocole, même page) dans une fenêtre Win32 NUE de premier niveau (E1) vaut
  (30,144,255) = le bleu de la page. WebView2 se peint donc bien sur ce runner ; c'est GPUI qui la masque.
- **Lecture** : le rendu GPUI (DirectComposition sur la fenêtre) recouvre les HWND enfants, quel que soit l'ordre de
  déclaration — le calque « par-dessus » ne se pose même pas : c'est la page entière qui est dessous.
  ★ Pas observé : ce même cas sur un GPU réel / un autre pilote (le runner est une VM, rendu logiciel probable).
- **Verdict : NO-GO sur gpui-webview Windows tel quel** (la page n'apparaît pas).

### (a) Popup au-dessus
- **Popup layered** (`WS_EX_LAYERED|NOACTIVATE|TOOLWINDOW`, possédé par la fenêtre GPUI, alpha 160) : mesuré
  (172,12,12) sur le fond GPUI, (255,95,95) sur du blanc = le mélange attendu de rouge à 63 % : il passe dessus.
  Utile pour un calque natif simple (surbrillance, curseur), pas pour de l'UI GPUI.
- **★ Webview dans un POPUP possédé (E2)** : webview enfant d'un `WS_POPUP` posé sur la feuille (position écran =
  origine client + (40,60)) : pixel (30,144,255) : **la page est visible**, au-dessus du rendu GPUI (capture 4).
- **★ Menu GPUI au-dessus du popup-webview (E2b)** : une 2e fenêtre GPUI (jaune, sans titre) ouverte au milieu :
  (255,255,0) au centre, (30,144,255) hors du menu (capture 5) : **un calque dessiné par GPUI passe par-dessus la
  page, à condition d'être sa propre fenêtre.**
- **Verdict : GO-AVEC.** Architecture réaliste : la feuille HTML = un popup possédé qui suit le rectangle de la vue ;
  les menus, popovers, curseurs d'annotation, surbrillances qui doivent recouvrir la page = fenêtres GPUI (ou layered)
  distinctes. Coûts/risques NON mesurés : suivi de position/taille/DPI/minimisation/ordre Z avec la fenêtre principale ;
  clip partiel (une page défilant sous une barre GPUI : réduire le popup ou `SetWindowRgn`) ; entrée souris/clavier et
  focus entre fenêtres (la touche du pont a été simulée par JS, pas une vraie frappe) ; pas de transparence par pixel
  pour une fenêtre GPUI (`Transparent` = blanc ici) donc pas d'ombres/coins arrondis/fonds translucides sur ces menus ;
  `render_to_image` et la parité au pixel ne voient pas le popup (la feuille HTML est hors du harnais `pixels`).

### (b) WebView2 en composition visuelle dans l'arbre DComp de GPUI
- **Hypothèse** : possible avec `gpui-fast`. **Constat par lecture, non essayé** : `gpui-fast-windows 0.1.12`
  (`src/fast/composition.rs`) crée bel et bien des « surfaces natives » = un `IDCompositionVisual` avec clip et
  décalage, et sa doc cite « a WebView2 controller's » visual ; `platform_handle()` rend cet `IUnknown`. Mais :
  `lb-wry` n'a aucun contrôleur de composition ; `gpui-webview` n'a pas d'adaptateur Windows (macOS/Linux seulement) ;
  il faudrait donc écrire `webview2-com` (`CreateCoreWebView2CompositionController`, `put_RootVisualTarget`), le relais
  souris (`SendMouseInput`), clavier/focus (`MoveFocus`, curseur) et le DPI, ET basculer TOUT sur `gpui-fast`
  (`gpui-kit/gpui-fast`, `gpui-shell/gpui-fast`, un autre GPUI : ADR-001 et le fork `gpui-base` à rejouer, pixels à revalider).
- **Chiffrage** : 8 à 15 jours, risque élevé (API `experimental`, deux GPUI à qualifier). À n'engager que si la
  parité du calque (ombres, transparence, ordre Z fin) devient exigible.
- **Verdict : NON ÉVALUÉ À L'ESSAI** ; faisable en principe, hors budget du spike.

### (c) Replis
- **Page sans calques** (webview en popup, rien par-dessus sauf fenêtres GPUI) : c'est le GO-AVEC ci-dessus ; la perte
  est la superposition fine (annotations dessinées sur la page, surbrillances de recherche). Celles-là peuvent venir du
  pont lui-même : la page peint ses surbrillances en DOM (`rects` du pont, déjà dans le script) au lieu de GPUI.
- **Rendu HTML natif pour pages statiques** : non évalué (aucun moteur HTML dans gpui-kit 0.8.1 n'a été examiné) ; ne
  couvre de toute façon pas les pages à JS, qui sont l'objet de la vue.

## Repli chiffré si la superposition fine est exigée

| Option | Coût | Perte |
|---|---|---|
| Popup possédé + menus en fenêtres GPUI (recommandé) | 4–6 j (hôte HWND : géométrie, clip, DPI, Z, focus, tests) | ombres/transparence des menus sur la page ; feuille HTML hors `pixels` |
| Popup + surbrillances/annotations peintes en DOM par le pont | +1–2 j | l'annotation vit dans la page, pas dans GPUI |
| Composition DComp via `gpui-fast` (b) | 8–15 j, risque élevé | basculer tout le socle sur un autre GPUI |

## Conditions pour T9 (vue HTML)

1. `gpui-webview = "=0.8.1"` NE sert PAS tel quel sous Windows (page invisible) : ne l'utiliser que pour ses accesseurs ;
   héberger `lb-wry` (`wry = "=0.53.3"`, paquet `lb-wry`) dans un popup possédé, `build_as_child` sur ce HWND.
2. Pont : `HTML_BRIDGE_SCRIPT` + adaptation `window.ipc` (ou iframe hôte), l'hôte n'accepte que `fragmentHtml`.
3. Thème par `?fragment-theme=` + `inject_html_bridge` dans le protocole ; sandbox = `resolve_in_vault` + `page_servable`,
   et une politique pour le réseau sortant (à écrire).
4. Pile : le scénario tourne sur un fil à 16 Mo (comme S3).
5. Ne pas activer `gpui-fast` (aucun besoin pour (a)).

## Ce qui n'a pas pu être observé

GPU réel, plusieurs écrans, échelle > 100 %, clic/saisie/focus réels dans le popup (tout est simulé par JS),
déplacement/redimensionnement/minimisation de la fenêtre, clip partiel au défilement, performances au-delà du premier
message (142 ms), consommation mémoire d'un popup WebView2 par page, `selection`/`rects`/`wheel`/`anchor` du pont,
réseau sortant, macOS/Linux, et l'option (b) à l'essai.
