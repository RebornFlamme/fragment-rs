# S4 — Thème (verdict)

Verdict : **GO**. Décision complète : `docs/decisions/ADR-005-theme.md`.

- Jetons : noms d'Obsidian, valeurs d'`index.css` résolues à la construction (alias, `calc`, `color-mix` sRGB arrondi comme Chromium).
- Thème de coffre : `theme.json` plat rechargé à chaud ; `theme.css` non supporté (non-but).
- Polices DM Sans / JetBrains Mono embarquées (OFL).
- Garde-fou : test contre les littéraux de couleur hors `fragment-theme`.
