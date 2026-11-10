# Parité pixels : produire les références sans écrire dans Fragment

Les références (`pixels/reference/*.png`) sont des captures de Fragment pilotée par son harnais e2e.
Fragment est en lecture seule : on exécute ses outils, on n'y écrit rien (décision du 06/11).

## Commande
`powershell -File scripts/pixels-reference.ps1 [scène]` — sans argument, les 11 scènes dans les deux thèmes (22 PNG).

## Ce que fait le script
1. **Serveur.** Le port 5123 est codé en dur dans Fragment (harnais e2e et processus principal) : il ne se
   surcharge pas. S'il répond déjà, c'est le serveur de l'utilisateur : on le réutilise et on ne l'arrête jamais.
   Sinon on lance Vite (binaire de `Fragment/app/node_modules`) avec la config enveloppe
   `pixels/vite.reference.config.ts`, qui reprend celle de Fragment et ne change que `root` et `cacheDir`
   (`C:/dev/fragment-rs-vite-cache`). Le serveur lancé par le script est arrêté en fin de passe (arbre de
   processus), et le script vérifie que le port est libéré.
2. **Pas de `.vite-temp`.** La config est chargée avec `--configLoader runner` (en mémoire) ; le chargeur par
   défaut écrirait un fichier temporaire dans `node_modules` de Fragment.
3. **Playwright** (celui de Fragment) joue `pixels/scenes/*.spec.ts` ; `userData` d'Electron et coffre vont dans
   `%TEMP%` (harnais) ; `FRAGMENT_NO_ONBOARDING=1`.
4. **Garde.** `git status --porcelain --ignored` de Fragment, plus l'empreinte (chemin, taille, date) de tout
   `app/` hors `node_modules` et de `node_modules/.vite`, `.vite-temp`, `.cache`, relevés avant et après ; un écart
   fait échouer le script. Une modification concurrente de Fragment par l'utilisateur pendant la passe
   déclenche donc aussi l'échec : relancer.
5. **Provenance.** Une passe complète réussie écrit `pixels/reference/PROVENANCE.txt` (commit, branche, état de
   l'arbre, mode du serveur).

## Si un PNG change
Comparer avec `git status` / `git diff --stat pixels/reference`. Un écart est à expliquer avant de commiter
(fixture ou décor réécrits, mise à jour de Fragment : le commit de PROVENANCE.txt le montre).

## Dettes de parité : surfaces flottantes T1 (fr-t1-menus)
Scènes `t1-{notice,notice-empilees,menu,infobulle,toolbar-horizontale,toolbar-sections}` (T1, décor
`_t1-flottantes` : seule la surface flottante sur le fond du workspace ; les scènes en fenêtre entière viendront
avec `fr-t1-binaire`). Les 12 captures du candidat existent et la mise en page est la bonne ; le job `pixels` reste
ROUGE (écarts de 0,5 à 4,3 % des pixels), pour des causes identifiées :
- **Largeur de texte / rembourrage (notice)** : la carte candidate fait 2 px de plus que celle de Fragment, le texte
  démarre 1 px plus à gauche et 1 px plus bas (interligne et approche des glyphes de GPUI contre Chromium).
- **Hauteur des lignes du menu** : Fragment compte 29,5 px par ligne (5 + 19,5 + 5), le candidat 29 ; GPUI arrondit
  les boîtes à l'entier malgré `h(29.5)`. Le menu candidat est 3 à 4 px plus court. Piste : positionner les lignes
  en absolu sur une grille fractionnaire, ou accepter l'arrondi par un ADR.
- **Ombres** : `--menu-shadow` / `--shadow-s` se rendent un cran plus claires côté GPUI (écart de 2 niveaux, zones
  de plus de 100 px : hors du sas des glyphes).
- **Icônes** (traits Lucide de 1,5 px) : anticrénelage différent, zones de 16 × 16 px.
- **Survol du menu** : Chromium ne peint pas `:hover` sur l'entrée dans le harnais de référence (cause non
  élucidée) ; le JSON candidat garde donc le pointeur garé.
- ★ La croix de l'onglet actif (437, 19) apparaissait sur les 12 premières références : `.workspace-tab-header-
  inner-close-button` force `visibility: visible` et survit à `#root{visibility:hidden}`. Le décor masque désormais
  `#root *`.
- **Décision du chef (10/11)** : la PR est fusionnée avec ces 6 scènes en `"depuis": "T2"`, pour garder la CI verte. Les écarts ci-dessus forment le bead `fr-t1.1`, qui bloque `fr-t1-binaire` : la tranche T1 n'est pas finie sans eux (program.md §6). `fr-t1-binaire` repasse les scènes à T1.
