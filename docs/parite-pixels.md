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
