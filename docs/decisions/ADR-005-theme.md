# ADR-005 — Thème : jetons d'Obsidian, valeurs de Fragment, `struct Theme` (S4)

- Statut : accepté (chef de projet, 01/11/2026). Clôt le spike S4 (`fr-p2-s4`), mené sans équipier.
- Sources : `docs/constantes-rendu.md` § Jetons (relevé de `Fragment/app/src/ui/index.css` : `:root`, `body`, `.theme-dark`, `.theme-light`), `Fragment/app/src/ui/theme/themeModel.ts`, polices `Fragment/app/src/ui/fonts/*.woff2`.

## Décision

1. **Une seule source de valeurs : `fragment-theme`.** `struct Theme { mode: Mode, palette: Palette, forme: Forme, typo: Typo }` où chaque champ porte le NOM du jeton CSS en snake_case (`--background-primary` → `background_primary`, `--radius-m` → `radius_m`, `--header-height` → `header_height`). Les valeurs sont celles d'`index.css` AU BIT PRÈS : `Theme::sombre()` et `Theme::clair()` sont écrits à la main depuis la table de `docs/constantes-rendu.md`, une ligne par jeton, avec le renvoi `index.css:<ligne>` en commentaire.
2. **Résolution à la construction, pas à l'exécution.** Les alias (`var(--x)`), `calc()` et `color-mix(in srgb, …)` d'`index.css` (85 couleurs, dont des `color-mix`) sont résolus en valeurs finales (`Rgba` 8 bits, px) dans `Theme::sombre()/clair()`. Un `color-mix` est calculé en sRGB non linéaire, arrondi comme Chromium (au plus proche, 0,5 vers le haut) — c'est ce qui rendra les pixels stricts. Un test par `color-mix` vérifie la valeur contre la référence pixel quand une scène la montre.
3. **Un composant lit un jeton, jamais un littéral.** Garde-fou : test `fragment-theme/tests/pas_de_litteral.rs` qui parcourt `crates/*/src/**/*.rs` hors `fragment-theme` et échoue sur `rgb(0x`, `rgba(`, `hsla(`, `Rgba {` ; seule exception listée : la mire de `fragment-pixels` (test du rasterizeur).
4. **Thèmes de coffre** : `.fragment/themes/<Nom>/theme.json` = un objet plat `{ "<jeton CSS>": "<valeur CSS>" }` (mêmes noms qu'Obsidian/Fragment), appliqué PAR-DESSUS `Theme::sombre()/clair()`, rechargé à chaud par le watcher (T2/T11). Le `theme.css` de Fragment n'est PAS interprété (non-but, `program.md` §5 S4) ; un `theme.css` présent est signalé une fois dans une Notice.
5. **Polices** : DM Sans (400/500/600, latin) et JetBrains Mono sont embarquées depuis les woff2 de Fragment convertis en TTF (licence OFL, compatible), chargées par `gpui::TextSystem::add_fonts` au démarrage ; `--font-interface` résout vers « DM Sans » sans repli système pour que les pixels ne dépendent pas de la machine.
6. **Mode** : `Mode::Sombre | Clair`, choisi par le réglage `theme` (fragment-core, `settings/theme.ts`) ; le basculement re-rend sans recharger (un `Global` GPUI lu par les vues).

## Conséquences

- `fr-p1-divers` porte `themeModel.ts` (modèle) ; la `struct Theme` complète et le garde-fou sont une tâche de T11 (ou de T1 si le shell en a besoin avant), et les scènes `*-light` de `pixels/` deviennent rejouables.
- Une valeur de thème qui diverge d'`index.css` est un bug de pixels, pas une préférence.
