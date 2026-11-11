# Constantes de rendu de Fragment

**But** : reproduire au pixel le rendu de Fragment dans la réécriture Rust (GPUI). Cette table recense les constantes de mise en page lues dans les CSS de Fragment, avec leur fichier et leur ligne.

**Date** : 01/11/2026

**Règle** : valeur lue, jamais réestimée. Un écart entre une valeur de ce document et le rendu voulu se traite par une ADR, jamais par une retouche silencieuse.

**Source** : `C:\Users\julie\OneDrive\Documents\Projects\Fragment\app\src` (lecture seule). Les chemins ci-dessous sont relatifs à ce dossier. Les jetons sont dans `ui\index.css`.

## Valeurs clés

- **En-tête commune (onglets, bandeau du sidedock, toggle du ribbon) : `--header-height: 36px`**, ui\index.css:63. Utilisée à hauteur fixe en border-box (ui\workspace.css:268, 876), et pour la largeur de la case du toggle (ui\workspace.css:856). **INFIRMÉ** : ni 37 px ni 54 px n'apparaissent dans les CSS.
- Hauteur de 32 px pour le gestionnaire de coffres et l'en-tête des réglages : `height: 32px` (ui\vault-manager\vault-manager.css:23, 33 ; ui\core\settings\settings.css:29). Le rôle exact de chaque élément reste à confirmer dans le composant.
- Barre d'état : `height: 20px`, ui\workspace.css:1049 (`max-width: var(--status-bar-max-width, 400px)` ligne 1047).
- Écart standard : `--size-4-2: 8px`, ui\index.css:148 ; très utilisé en `gap` et `padding` (ui\core\menu.css:20, ui\core\settings\settings.css:70, ui\views\agent\policy\permission.css:7). **CONFIRMÉ** pour l'écart de 8 px. Écart interne de l'éditeur de chat : `--incrusted-toolbar-gap: 8px`, ui\views\agent\incrusted-toolbar.css:14.
- Padding de tête de la barre d'onglets : `padding: var(--size-4-1) 0 0` (4 px), ui\workspace.css:274.
- Icônes d'échelle : `--icon-xs: 14px`, `--icon-s: 16px`, `--icon-m: 18px`, `--icon-stroke: 1.5px`, ui\index.css:127-131. **INFIRMÉ** pour 22 px : la valeur 22 px n'existe que pour les pastilles d'avatar (ui\core\workspace\view-icons.css:16-17) et une icône du menu de calques (ui\core\workspace\layer-menu.css:63-64).
- Bandeau 38 px : `--size-2-19: 38px`, ui\index.css:146, peu utilisé. La seule occurrence de 38 px dans les CSS est la barre d'icônes du doc-widget, `--doc-widget-icons-height` de repli à 38 px (ui\views\doc-widget\doc-widget.css:147). **PARTIELLEMENT CONFIRMÉ** (un seul bandeau).
- Largeur du dock latéral : `width: var(--sidebar-width, 260px)`, ui\workspace.css:52 (variable `@property` ligne 37).
- Largeur d'onglet maximale : `--tab-max-width: 200px`, ui\workspace.css:257. Rayon du haut d'onglet : `--input-radius` 5 px (ui\workspace.css:246) ; rayon du bas : `--radius-s` 4 px (ui\workspace.css:247).
- Boutons de fenêtre (Windows) : `--frame-button-width: 46px`, ui\index.css:73 ; espace réservé à droite `--frame-right-space: calc(3 * 46px) = 138px`, ui\index.css:74.
- Bordure : `--border-width: 1px`, ui\index.css:83.
- Échelle des rayons : `--radius-s` 4 px, `--radius-sm` 6 px, `--radius-m` 8 px (menus, ui\core\menu.css:9), `--radius-ml` 10 px (modales, `--modal-radius`, ui\index.css:102), `--radius-l` 12 px (ui\core\modal.css:35), `--radius-xl` 16 px (ui\index.css:95).
- Tailles de police : 12 px pour les onglets et l'arbre (`--font-ui-small`, ui\index.css:67, 113 ; ui\workspace.css:403, 638), 13 px pour les menus (ui\core\menu.css:13), 14 px pour les modales (ui\core\modal.css:37), 10 px pour le badge d'avatar (ui\core\workspace\view-icons.css:20).
- Graisses : 400, 500, 600, 700 (ui\index.css:6, 13, 20, 27 ; `--font-normal` à `--font-bold`, lignes 120-123).
- Police : DM Sans auto-hébergée, graisses 400 à 700 (ui\index.css:3-30).
- Largeurs de modales : `min(560px, 100vw - 32px)` (ui\core\modal.css:44) et `min(700px, 100vw - 32px)` (ui\core\modal.css:89).
- Infobulle : `max-width: 280px` (ui\core\tooltip.css:8).
- Grille des réglages : `minmax(230px, 1fr)` (ui\core\settings\settings.css:190).
- Rond d'avatar : `border-radius: 999px` et `50%` (ui\core\workspace\view-icons.css:18, 108).

## Jetons

Déclarés sur `:root` (structure, identique dans les deux thèmes), sur `body` (identique dans les deux thèmes), puis sur `.theme-dark` et `.theme-light` (palette). Quand une valeur n'est déclarée que sur `body` ou `:root`, les deux colonnes portent la même valeur, suivie de « (commun) ».

| Variable | Valeur sombre | Valeur claire | Fichier:ligne |
|---|---|---|---|
| `--font-interface` | `'DM Sans', system-ui, -apple-system, 'Segoe UI', sans-serif` | (commun) | ui\index.css:52 |
| `--font-text` | `var(--font-interface)` | (commun) | ui\index.css:53 |
| `--header-height` | `36px` | (commun) | ui\index.css:63 |
| `--font-ui-small` | `12px` | (commun) | ui\index.css:67, 113 |
| `--frame-button-width` | `46px` | (commun) | ui\index.css:73 |
| `--frame-right-space` | `calc(3 * var(--frame-button-width))` | (commun) | ui\index.css:74 |
| `--border-width` | `1px` | (commun) | ui\index.css:83 |
| `--radius-hair` | `1px` | (commun) | ui\index.css:87 |
| `--radius-xxs` | `2px` | (commun) | ui\index.css:88 |
| `--radius-xs` | `3px` | (commun) | ui\index.css:89 |
| `--radius-s` | `4px` | (commun) | ui\index.css:90 |
| `--radius-sm` | `6px` | (commun) | ui\index.css:91 |
| `--radius-m` | `8px` | (commun) | ui\index.css:92 |
| `--radius-ml` | `10px` | (commun) | ui\index.css:93 |
| `--radius-l` | `12px` | (commun) | ui\index.css:94 |
| `--radius-xl` | `16px` | (commun) | ui\index.css:95 |
| `--input-radius` | `5px` | (commun) | ui\index.css:97 |
| `--button-radius` | `var(--input-radius)` | (commun) | ui\index.css:98 |
| `--checkbox-radius` | `var(--radius-xs)` | (commun) | ui\index.css:99 |
| `--clickable-icon-radius` | `var(--radius-sm)` | (commun) | ui\index.css:100 |
| `--menu-radius` | `var(--radius-m)` | (commun) | ui\index.css:101 |
| `--modal-radius` | `var(--radius-ml)` | (commun) | ui\index.css:102 |
| `--tooltip-radius` | `var(--radius-sm)` | (commun) | ui\index.css:103 |
| `--code-radius` | `var(--radius-xs)` | (commun) | ui\index.css:104 |
| `--font-monospace` | `'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace` | (commun) | ui\index.css:107 |
| `--font-serif` | `Georgia, 'Times New Roman', serif` | (commun) | ui\index.css:108 |
| `--font-ui-smallest` | `10px` | (commun) | ui\index.css:111 |
| `--font-ui-smaller` | `11px` | (commun) | ui\index.css:112 |
| `--font-ui-medium` | `13px` | (commun) | ui\index.css:114 |
| `--font-ui-normal` | `14px` | (commun) | ui\index.css:115 |
| `--font-ui-large` | `15px` | (commun) | ui\index.css:116 |
| `--font-ui-larger` | `18px` | (commun) | ui\index.css:117 |
| `--font-ui-largest` | `20px` | (commun) | ui\index.css:118 |
| `--font-normal` | `400` | (commun) | ui\index.css:120 |
| `--font-medium` | `500` | (commun) | ui\index.css:121 |
| `--font-semibold` | `600` | (commun) | ui\index.css:122 |
| `--font-bold` | `700` | (commun) | ui\index.css:123 |
| `--line-height-normal` | `1.5` | (commun) | ui\index.css:124 |
| `--icon-xs` | `14px` | (commun) | ui\index.css:127 |
| `--icon-s` | `16px` | (commun) | ui\index.css:128 |
| `--icon-m` | `18px` | (commun) | ui\index.css:129 |
| `--icon-l` | `18px` | (commun) | ui\index.css:130 |
| `--icon-stroke` | `1.5px` | (commun) | ui\index.css:131 |
| `--size-2-1` | `2px` | (commun) | ui\index.css:136 |
| `--size-2-2` | `4px` | (commun) | ui\index.css:137 |
| `--size-2-3` | `6px` | (commun) | ui\index.css:138 |
| `--size-2-5` | `10px` | (commun) | ui\index.css:139 |
| `--size-2-7` | `14px` | (commun) | ui\index.css:140 |
| `--size-2-9` | `18px` | (commun) | ui\index.css:141 |
| `--size-2-11` | `22px` | (commun) | ui\index.css:142 |
| `--size-2-13` | `26px` | (commun) | ui\index.css:143 |
| `--size-2-15` | `30px` | (commun) | ui\index.css:144 |
| `--size-2-17` | `34px` | (commun) | ui\index.css:145 |
| `--size-2-19` | `38px` | (commun) | ui\index.css:146 |
| `--size-4-1` | `4px` | (commun) | ui\index.css:147 |
| `--size-4-2` | `8px` | (commun) | ui\index.css:148 |
| `--size-4-3` | `12px` | (commun) | ui\index.css:149 |
| `--size-4-4` | `16px` | (commun) | ui\index.css:150 |
| `--size-4-5` | `20px` | (commun) | ui\index.css:151 |
| `--size-4-6` | `24px` | (commun) | ui\index.css:152 |
| `--size-4-7` | `28px` | (commun) | ui\index.css:153 |
| `--size-4-8` | `32px` | (commun) | ui\index.css:154 |
| `--size-4-9` | `36px` | (commun) | ui\index.css:155 |
| `--size-4-10` | `40px` | (commun) | ui\index.css:156 |
| `--size-4-11` | `44px` | (commun) | ui\index.css:157 |
| `--size-4-12` | `48px` | (commun) | ui\index.css:158 |
| `--size-4-14` | `56px` | (commun) | ui\index.css:159 |
| `--size-4-16` | `64px` | (commun) | ui\index.css:160 |
| `--size-4-18` | `72px` | (commun) | ui\index.css:161 |
| `--size-4-20` | `80px` | (commun) | ui\index.css:162 |
| `--anim-duration-none` | `0ms` | (commun) | ui\index.css:166 |
| `--anim-duration-superfast` | `80ms` | (commun) | ui\index.css:167 |
| `--anim-duration-quick` | `90ms` | (commun) | ui\index.css:168 |
| `--anim-duration-fast` | `120ms` | (commun) | ui\index.css:169 |
| `--anim-duration-moderate` | `150ms` | (commun) | ui\index.css:170 |
| `--anim-duration-slow` | `250ms` | (commun) | ui\index.css:171 |
| `--anim-motion-smooth` | `cubic-bezier(0.2, 0, 0, 1)` | (commun) | ui\index.css:172 |
| `--anim-motion-in-out` | `cubic-bezier(0.45, 0, 0.55, 1)` | (commun) | ui\index.css:173 |
| `--window-close-bg` | `#e81123` | (commun) | ui\index.css:184 |
| `--text-on-accent` | `#ffffff` | (commun) | ui\index.css:186 |
| `--toggle-thumb-color` | `#ffffff` | (commun) | ui\index.css:187 |
| `--background-modifier-cover` | `rgba(0, 0, 0, 0.5)` | (commun) | ui\index.css:189 |
| `--background-overlay` | `rgba(0, 0, 0, 0.75)` | (commun) | ui\index.css:190 |
| `--text-on-overlay` | `#ffffff` | (commun) | ui\index.css:191 |
| `--background-modifier-error` | `#a52828` | (commun) | ui\index.css:193 |
| `--background-modifier-warning` | `#d29922` | (commun) | ui\index.css:195 |
| `--text-on-warning` | `#1a1a1a` | (commun) | ui\index.css:196 |
| `--color-red` | `#e93147` | (commun) | ui\index.css:199 |
| `--color-orange` | `#ec7500` | (commun) | ui\index.css:200 |
| `--color-yellow` | `#e0ac00` | (commun) | ui\index.css:201 |
| `--color-green` | `#4caf82` | (commun) | ui\index.css:202 |
| `--color-cyan` | `#56b6c2` | (commun) | ui\index.css:203 |
| `--color-blue` | `#5b9dff` | (commun) | ui\index.css:204 |
| `--color-purple` | `#7852ee` | (commun) | ui\index.css:205 |
| `--color-pink` | `#d53984` | (commun) | ui\index.css:206 |
| `--link-color` | `var(--color-blue)` | (commun) | ui\index.css:209 |
| `--text-highlight-bg` | `rgba(255, 238, 0, 0.22)` | (commun) | ui\index.css:211 |
| `--code-normal` | `#abb2bf` | (commun) | ui\index.css:214 |
| `--code-comment` | `#7f848e` | (commun) | ui\index.css:215 |
| `--code-keyword` | `#c678dd` | (commun) | ui\index.css:216 |
| `--code-function` | `#61afef` | (commun) | ui\index.css:217 |
| `--code-property` | `#e06c75` | (commun) | ui\index.css:218 |
| `--code-value` | `#d19a66` | (commun) | ui\index.css:219 |
| `--code-tag` | `#e5c07b` | (commun) | ui\index.css:220 |
| `--code-operator` | `#56b6c2` | (commun) | ui\index.css:221 |
| `--code-string` | `#98c379` | (commun) | ui\index.css:222 |
| `--code-punctuation` | `#abb2bf` | (commun) | ui\index.css:223 |
| `--code-important` | `#ffffff` | (commun) | ui\index.css:224 |
| `--interactive-accent` | `var(--color-accent)` | (commun) | ui\index.css:228 |
| `--interactive-accent-hover` | `color-mix(in srgb, var(--color-accent) 85%, var(--text-normal))` | (commun) | ui\index.css:229 |
| `--color-accent-1` | `color-mix(in srgb, var(--color-accent) 70%, white)` | (commun) | ui\index.css:231 |
| `--color-accent-2` | `color-mix(in srgb, var(--color-accent) 75%, black)` | (commun) | ui\index.css:232 |
| `--text-accent` | `color-mix(in srgb, var(--color-accent), white 25%)` | `var(--color-accent)` | ui\index.css:235 (sombre), 302 (claire) |
| `--background-modifier-border` | `color-mix(in srgb, var(--text-normal) 9%, transparent)` | `color-mix(in srgb, var(--text-normal) 10%, transparent)` | ui\index.css:239 (sombre), 319 (claire) |
| `--background-modifier-border-hover` | `color-mix(in srgb, var(--text-normal) 20%, transparent)` | (commun) | ui\index.css:240 |
| `--background-modifier-hover` | `color-mix(in srgb, var(--text-normal) 6%, transparent)` | `color-mix(in srgb, var(--text-normal) 5%, transparent)` | ui\index.css:241 (sombre), 320 (claire) |
| `--background-modifier-active-hover` | `color-mix(in srgb, var(--text-normal) 10%, transparent)` | (commun) | ui\index.css:242 |
| `--scrollbar-thumb-bg` | `color-mix(in srgb, var(--text-normal) 20%, transparent)` | (commun) | ui\index.css:243 |
| `--scrollbar-active-thumb-bg` | `color-mix(in srgb, var(--text-normal) 30%, transparent)` | (commun) | ui\index.css:244 |
| `--workspace-bg` | `var(--background-primary)` | (commun) | ui\index.css:249 |
| `--color-accent` | `#646cff` | `#4b52d6` | ui\index.css:265 (sombre), 301 (claire) |
| `--tab-bar-bg` | `#0e0f11` | `#e8e7e1` | ui\index.css:267, 304 |
| `--background-secondary` | `#111214` | `#efeee9` | ui\index.css:268, 305 |
| `--background-primary` | `#141517` | `#f7f6f2` | ui\index.css:269, 306 |
| `--background-primary-alt` | `#191a1e` | `#ffffff` | ui\index.css:270, 307 |
| `--menu-bg` | `#1c1d21` | `#ffffff` | ui\index.css:271, 308 |
| `--background-secondary-alt` | `#222328` | `#ffffff` | ui\index.css:272, 309 |
| `--tab-outline-color` | `#34363c` | `#d3d2cb` | ui\index.css:273, 310 |
| `--text-normal` | `#dfded9` | `#1a1b1e` | ui\index.css:276, 312 |
| `--text-muted` | `#a3a29d` | `#5c5b57` | ui\index.css:277, 313 |
| `--text-faint` | `#6f6e6a` | `#8a8984` | ui\index.css:278, 314 |
| `--text-error` | `#e5534b` | `#c93c34` | ui\index.css:279, 315 |
| `--text-warning` | `#d29922` | `#9a6700` | ui\index.css:280, 316 |
| `--text-success` | `#4caf82` | `#2f8a5f` | ui\index.css:281, 317 |
| `--menu-shadow` | `0 8px 24px rgba(0, 0, 0, 0.55)` | `0 6px 20px rgba(17, 18, 20, 0.12)` | ui\index.css:283, 322 |
| `--shadow-xs` | `0 0.5px 1.5px rgba(0, 0, 0, 0.22)` | `0 0.5px 1.5px rgba(17, 18, 20, 0.15)` | ui\index.css:286, 323 |
| `--shadow-s` | `0 2px 8px rgba(0, 0, 0, 0.4)` | `0 2px 8px rgba(17, 18, 20, 0.08)` | ui\index.css:287, 324 |
| `--shadow-card` | `0 1px 2px rgba(0, 0, 0, 0.06), 0 6px 20px rgba(0, 0, 0, 0.08)` | `0 1px 2px rgba(17, 18, 20, 0.04), 0 6px 20px rgba(17, 18, 20, 0.06)` | ui\index.css:288, 325 |
| `--shadow-l` | `0 16px 48px rgba(0, 0, 0, 0.6)` | `0 16px 48px rgba(17, 18, 20, 0.16)` | ui\index.css:289, 326 |
| `--avatar-bg-l` | `0.37` | `0.91` | ui\index.css:292, 328 |
| `--avatar-bg-c` | `0.07` | `0.045` | ui\index.css:293, 329 |
| `--avatar-fg-l` | `0.9` | `0.41` | ui\index.css:294, 330 |
| `--avatar-fg-c` | `0.05` | `0.085` | ui\index.css:295, 331 |
| `--pdf-page-bg` | `#ffffff` | (commun) | ui\views\pdf\pdf-view.css:114 |

## Constantes de mise en page par fichier

Tableau généré par lecture directe des 50 fichiers `.css` : chaque ligne donne le sélecteur de la règle qui contient la déclaration (ou « (racine) » hors règle), la propriété, la valeur telle qu'écrite, et la ligne. Propriétés retenues : width, height, min/max-*, padding*, margin*, gap, row-gap, column-gap, top, left, right, bottom, inset, border-width, border-*-width, border-radius, border-*-radius, font-size, line-height, font-weight, grid-template*. Les commentaires sont ignorés.

### `ui/core/boot-progress.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.boot-progress` | inset | `0` | 8 |
| `.boot-progress` | gap | `var(--size-2-11)` | 15 |
| `.boot-progress-label` | font-size | `var(--font-ui-medium)` | 28 |
| `.boot-progress-label` | min-height | `1.5em` | 31 |
| `.boot-progress-track` | width | `calc(100% - 56px)` | 38 |
| `.boot-progress-track` | height | `6px` | 39 |
| `.boot-progress-track` | border-radius | `var(--radius-xs)` | 40 |
| `.boot-progress-bar` | top | `0` | 50 |
| `.boot-progress-bar` | bottom | `0` | 51 |
| `.boot-progress-bar` | left | `0` | 52 |
| `.boot-progress-bar` | width | `24%` | 53 |
| `.boot-progress-bar` | border-radius | `var(--radius-xs)` | 54 |
| `.boot-progress-bar` | width | `100%` | 71 |
| `.boot-progress-error` | max-width | `min(900px, calc(100% - 56px))` | 88 |
| `.boot-progress-error` | max-height | `60%` | 89 |
| `.boot-progress-error` | margin | `0` | 92 |
| `.boot-progress-error` | padding | `var(--size-2-7) var(--size-4-4)` | 93 |
| `.boot-progress-error` | border-radius | `var(--radius-sm)` | 94 |
| `.boot-progress-error` | font-size | `var(--font-ui-small)` | 98 |
| `.boot-progress-error` | line-height | `1.55` | 99 |

### `ui/core/editor/range-highlight.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.range-highlight` | border-radius | `var(--radius-xs)` | 7 |
| `.range-highlight.is-passage` | border-radius | `var(--radius-xxs)` | 20 |

### `ui/core/embed/embed-frame.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.embed-frame > .embed-frame-body` | max-height | `var(--embed-height, var(--embed-max-height))` | 13 |
| `.embed-frame[data-sizing='fixed']` | font-size | `var(--font-ui-medium)` | 32 |
| `.embed-frame[data-sizing='fixed']` | line-height | `normal` | 33 |
| `.embed-frame[data-sizing='fixed'] > .embed-frame-body` | height | `var(--embed-height, var(--embed-max-height))` | 39 |
| `.embed-frame.is-active::after` | inset | `0` | 49 |
| `.embed-frame.is-active::after` | border-radius | `inherit` | 50 |
| `.embed-frame-resize` | left | `50%` | 59 |
| `.embed-frame-resize` | bottom | `0` | 60 |
| `.embed-frame-resize` | width | `48px` | 61 |
| `.embed-frame-resize` | height | `10px` | 62 |
| `.embed-frame-resize::before` | left | `10px` | 71 |
| `.embed-frame-resize::before` | right | `10px` | 72 |
| `.embed-frame-resize::before` | bottom | `3px` | 73 |
| `.embed-frame-resize::before` | height | `3px` | 74 |
| `.embed-frame-resize::before` | border-radius | `var(--radius-xxs)` | 75 |

### `ui/core/menu.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.menu` | min-width | `180px` | 7 |
| `.menu` | padding | `var(--size-4-1)` | 8 |
| `.menu` | border-radius | `var(--radius-m)` | 9 |
| `.menu` | font-size | `var(--font-ui-medium)` | 13 |
| `.menu-item` | gap | `var(--size-4-2)` | 20 |
| `.menu-item` | padding | `5px var(--size-4-2)` | 21 |
| `.menu-item` | border-radius | `var(--radius-s)` | 22 |
| `.menu-item-icon` | width | `var(--icon-m)` | 45 |
| `.menu-item-icon svg` | width | `15px` | 51 |
| `.menu-item-icon svg` | height | `15px` | 52 |
| `.menu-item-icon:empty::before` | width | `var(--icon-m)` | 57 |
| `.menu-separator` | height | `1px` | 61 |
| `.menu-separator` | margin | `var(--size-4-1) var(--size-2-3)` | 62 |
| `.menu-item-chevron svg` | width | `var(--icon-xs)` | 78 |
| `.menu-item-chevron svg` | height | `var(--icon-xs)` | 79 |

### `ui/core/modal.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.modal-container` | inset | `0` | 9 |
| `.modal-container` | padding-top | `12vh` | 17 |
| `.modal-bg` | inset | `0` | 22 |
| `.modal, .prompt` | max-height | `70vh` | 31 |
| `.modal, .prompt` | border-radius | `var(--radius-l)` | 35 |
| `.modal, .prompt` | font-size | `var(--font-ui-normal)` | 37 |
| `.modal` | width | `min(560px, calc(100vw - 32px))` | 44 |
| `.modal` | padding | `var(--size-4-5) var(--size-4-6)` | 45 |
| `.modal-close-button` | top | `10px` | 50 |
| `.modal-close-button` | right | `10px` | 51 |
| `.modal-close-button` | width | `24px` | 53 |
| `.modal-close-button` | height | `24px` | 54 |
| `.modal-close-button` | border-radius | `var(--radius-sm)` | 57 |
| `.modal-close-button svg` | width | `var(--icon-s)` | 68 |
| `.modal-close-button svg` | height | `var(--icon-s)` | 69 |
| `.modal-title` | font-size | `var(--font-ui-larger)` | 73 |
| `.modal-title` | font-weight | `var(--font-semibold)` | 74 |
| `.modal-title` | margin-bottom | `var(--size-4-3)` | 75 |
| `.prompt` | width | `min(700px, calc(100vw - 32px))` | 89 |
| `.prompt-input-container` | padding | `var(--size-4-3) var(--size-2-7)` | 94 |
| `.prompt-input` | min-width | `0` | 100 |
| `.prompt-input` | padding | `0` | 101 |
| `.prompt-input` | font-size | `16px` | 107 |
| `.prompt-results` | min-height | `0` | 117 |
| `.prompt-results` | padding | `var(--size-2-3)` | 119 |
| `.suggestion-item` | gap | `var(--size-2-5)` | 125 |
| `.suggestion-item` | padding | `7px var(--size-2-5)` | 126 |
| `.suggestion-item` | border-radius | `var(--radius-sm)` | 127 |
| `.suggestion-content` | min-width | `0` | 142 |
| `.suggestion-highlight` | font-weight | `var(--font-semibold)` | 154 |
| `.suggestion-aux` | gap | `var(--size-2-3)` | 160 |
| `.suggestion-hotkey` | padding | `1px var(--size-2-3)` | 165 |
| `.suggestion-hotkey` | border-radius | `var(--radius-s)` | 166 |
| `.suggestion-hotkey` | font-size | `var(--font-ui-smaller)` | 171 |
| `.suggestion-flair svg` | width | `var(--icon-xs)` | 181 |
| `.suggestion-flair svg` | height | `var(--icon-xs)` | 182 |
| `.suggestion-empty` | padding | `var(--size-2-7) var(--size-2-5)` | 186 |
| `.prompt-instructions` | gap | `var(--size-4-1) var(--size-2-9)` | 194 |
| `.prompt-instructions` | padding | `var(--size-4-2) var(--size-2-7)` | 195 |
| `.prompt-instructions` | font-size | `var(--font-ui-small)` | 198 |
| `.prompt-instruction` | gap | `var(--size-2-3)` | 208 |
| `.prompt-instruction-command` | font-weight | `var(--font-semibold)` | 212 |

### `ui/core/notice.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.notice-container` | top | `0` | 6 |
| `.notice-container` | right | `0` | 7 |
| `.notice-container` | gap | `var(--size-4-2)` | 12 |
| `.notice-container` | padding | `var(--size-4-4)` | 13 |
| `.notice` | max-width | `340px` | 20 |
| `.notice` | padding | `var(--size-2-5) var(--size-2-7)` | 21 |
| `.notice` | border-radius | `var(--radius-m)` | 22 |
| `.notice` | font-size | `var(--font-ui-small)` | 27 |
| `.notice` | line-height | `1.4` | 28 |

### `ui/core/settings/settings.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.settings-window` | height | `100vh` | 17 |
| `.settings-titlebar` | height | `32px` | 29 |
| `.settings-titlebar` | padding | `0 var(--size-2-7)` | 33 |
| `.settings-titlebar-title` | font-size | `var(--font-ui-medium)` | 40 |
| `.settings-body` | min-height | `0` | 47 |
| `.vertical-tab-header` | width | `220px` | 51 |
| `.vertical-tab-header` | padding | `var(--size-4-3) var(--size-4-2)` | 53 |
| `.vertical-tab-header-group-title` | padding | `var(--size-2-7) var(--size-2-5) var(--size-4-1)` | 60 |
| `.vertical-tab-header-group-title` | font-size | `var(--font-ui-small)` | 62 |
| `.vertical-tab-nav-item` | gap | `var(--size-4-2)` | 70 |
| `.vertical-tab-nav-item` | padding | `var(--size-2-3) var(--size-2-5)` | 71 |
| `.vertical-tab-nav-item` | border-radius | `var(--radius-sm)` | 72 |
| `.vertical-tab-nav-item` | font-size | `var(--font-ui-medium)` | 74 |
| `.vertical-tab-nav-item-icon svg` | width | `var(--icon-s)` | 93 |
| `.vertical-tab-nav-item-icon svg` | height | `var(--icon-s)` | 94 |
| `.vertical-tab-content-container` | min-width | `0` | 99 |
| `.vertical-tab-content` | max-width | `760px` | 104 |
| `.vertical-tab-content` | padding | `var(--size-4-5) var(--size-4-6)` | 105 |
| `.vertical-tab-header .search-input-container` | margin | `var(--size-2-1) var(--size-2-3) var(--size-2-3)` | 110 |
| `.plugin-browser` | min-height | `0` | 119 |
| `.plugin-browser` | padding | `var(--size-4-4) var(--size-4-5)` | 120 |
| `.plugin-browser` | gap | `var(--size-2-5)` | 121 |
| `.plugin-browser-topbar` | gap | `var(--size-4-3)` | 127 |
| `.plugin-browser-sources` | gap | `var(--size-2-3)` | 136 |
| `.plugin-browser-source` | padding | `var(--size-2-3) var(--size-4-3)` | 141 |
| `.plugin-browser-source` | border-radius | `var(--radius-sm)` | 142 |
| `.plugin-browser-source` | font-size | `var(--font-ui-medium)` | 147 |
| `.plugin-browser-options` | gap | `var(--size-4-3)` | 161 |
| `.plugin-browser-installed-toggle` | gap | `var(--size-2-5)` | 167 |
| `.plugin-browser-installed-toggle` | font-size | `var(--font-ui-medium)` | 169 |
| `.plugin-browser-count` | font-size | `var(--font-ui-small)` | 174 |
| `.plugin-browser-status` | padding | `var(--size-4-6)` | 179 |
| `.plugin-browser-grid` | min-height | `0` | 187 |
| `.plugin-browser-grid` | grid-template-columns | `repeat(auto-fill, minmax(230px, 1fr))` | 190 |
| `.plugin-browser-grid` | gap | `var(--size-4-3)` | 191 |
| `.plugin-browser-grid` | padding-top | `var(--size-4-1)` | 193 |
| `.plugin-card` | gap | `var(--size-4-1)` | 199 |
| `.plugin-card` | padding | `var(--size-2-7) var(--size-4-4)` | 200 |
| `.plugin-card` | border-radius | `var(--radius-ml)` | 201 |
| `.plugin-card-head` | gap | `var(--size-4-2)` | 222 |
| `.plugin-card-name` | font-weight | `var(--font-semibold)` | 227 |
| `.plugin-card-badge` | padding | `1px var(--size-2-3)` | 231 |
| `.plugin-card-badge` | border-radius | `var(--radius-s)` | 232 |
| `.plugin-card-badge` | font-size | `var(--font-ui-smallest)` | 235 |
| `.plugin-card-badge` | font-weight | `var(--font-bold)` | 236 |
| `.plugin-card-author` | font-size | `var(--font-ui-small)` | 242 |
| `.plugin-card-meta` | gap | `var(--size-4-3)` | 247 |
| `.plugin-card-meta` | font-size | `var(--font-ui-small)` | 249 |
| `.plugin-card-desc` | font-size | `var(--font-ui-small)` | 254 |
| `.plugin-card-desc` | line-height | `1.4` | 255 |
| `.plugin-card-desc` | margin-top | `var(--size-4-1)` | 256 |
| `.plugin-browser-browse, .plugin-browser-detail` | min-height | `0` | 265 |
| `.plugin-browser-browse, .plugin-browser-detail` | gap | `var(--size-2-5)` | 266 |
| `.plugin-browser-sort` | gap | `var(--size-4-2)` | 276 |
| `.plugin-browser-sort` | font-size | `var(--font-ui-medium)` | 278 |
| `.plugin-browser-sort` | margin-left | `auto` | 279 |
| `.plugin-browser-sort` | margin-right | `var(--size-4-3)` | 280 |
| `.plugin-detail-back` | padding | `var(--size-2-3) var(--size-2-5)` | 287 |
| `.plugin-detail-back` | border-radius | `var(--radius-sm)` | 288 |
| `.plugin-detail-back` | font-size | `var(--font-ui-medium)` | 293 |
| `.plugin-detail-name` | font-size | `var(--font-ui-largest)` | 299 |
| `.plugin-detail-name` | font-weight | `var(--font-bold)` | 300 |
| `.plugin-detail-author` | font-size | `var(--font-ui-medium)` | 305 |
| `.plugin-detail-meta` | gap | `var(--size-2-7)` | 311 |
| `.plugin-detail-meta` | font-size | `var(--font-ui-small)` | 313 |
| `.plugin-detail-actions` | gap | `var(--size-4-3)` | 328 |
| `.plugin-detail-actions` | margin | `var(--size-4-1) 0` | 329 |
| `.plugin-detail-error` | font-size | `var(--font-ui-small)` | 334 |
| `.plugin-detail-desc` | font-size | `var(--font-ui-medium)` | 339 |
| `.plugin-detail-desc` | line-height | `var(--line-height-normal)` | 340 |
| `.plugin-detail-readme` | border-radius | `var(--radius-m)` | 345 |
| `.plugin-detail-readme` | padding | `var(--size-2-7) var(--size-4-4)` | 346 |
| `.plugin-detail-readme` | font-size | `var(--font-ui-medium)` | 348 |
| `.plugin-detail-readme` | line-height | `1.6` | 349 |
| `.plugin-detail-readme > :first-child` | margin-top | `0` | 355 |
| `.plugin-detail-readme > :last-child` | margin-bottom | `0` | 356 |
| `.plugin-detail-readme :is(h1, h2, h3, h4, h5, h6)` | margin | `1.2em 0 0.5em` | 358 |
| `.plugin-detail-readme :is(h1, h2, h3, h4, h5, h6)` | line-height | `1.3` | 359 |
| `.plugin-detail-readme :is(h1, h2, h3, h4, h5, h6)` | font-weight | `var(--font-semibold)` | 360 |
| `.plugin-detail-readme h1` | font-size | `1.6em` | 362 |
| `.plugin-detail-readme h2` | font-size | `1.35em` | 363 |
| `.plugin-detail-readme h3` | font-size | `1.15em` | 364 |
| `.plugin-detail-readme :is(p, ul, ol, blockquote, pre, table)` | margin | `0 0 0.8em` | 365 |
| `.plugin-detail-readme :is(ul, ol)` | padding-left | `1.6em` | 366 |
| `.plugin-detail-readme li.task-list-item > input` | margin | `0 var(--size-2-3) 0 -1.3em` | 368 |
| `.plugin-detail-readme code` | font-size | `0.9em` | 373 |
| `.plugin-detail-readme code` | padding | `1px var(--size-4-1)` | 374 |
| `.plugin-detail-readme code` | border-radius | `var(--radius-s)` | 375 |
| `.plugin-detail-readme pre` | padding | `var(--size-2-5) var(--size-4-3)` | 379 |
| `.plugin-detail-readme pre` | border-radius | `var(--radius-sm)` | 380 |
| `.plugin-detail-readme pre code` | padding | `0` | 384 |
| `.plugin-detail-readme blockquote` | padding-left | `var(--size-4-3)` | 386 |
| `.plugin-detail-readme hr` | margin | `1.2em 0` | 390 |
| `.plugin-detail-readme img` | max-width | `100%` | 391 |
| `.plugin-detail-readme :is(th, td)` | padding | `var(--size-4-1) var(--size-2-5)` | 393 |
| `.plugin-card-badge.mod-incompatible` | margin-left | `var(--size-4-2)` | 396 |
| `.plugin-detail-installed` | font-size | `var(--font-ui-small)` | 403 |
| `.plugin-update-badge` | margin-left | `var(--size-4-2)` | 408 |
| `.plugin-update-badge` | padding | `1px var(--size-2-3)` | 409 |
| `.plugin-update-badge` | border-radius | `var(--radius-s)` | 410 |
| `.plugin-update-badge` | font-size | `var(--font-ui-smallest)` | 413 |
| `.plugin-update-badge` | font-weight | `var(--font-bold)` | 414 |
| `.hotkeys-search` | margin-bottom | `var(--size-4-2)` | 421 |
| `.hotkey-chip` | gap | `var(--size-4-1)` | 427 |
| `.hotkey-kbd` | padding | `var(--size-2-1) 7px` | 431 |
| `.hotkey-kbd` | border-radius | `var(--input-radius)` | 432 |
| `.hotkey-kbd` | font-size | `var(--font-ui-small)` | 437 |
| `.hotkey-remove` | font-size | `var(--font-ui-smaller)` | 444 |
| `.hotkey-remove` | line-height | `1` | 445 |
| `.hotkey-remove` | padding | `var(--size-2-1)` | 446 |
| `.hotkey-capturing` | font-size | `var(--font-ui-small)` | 455 |
| `.hotkey-conflict` | margin-top | `var(--size-4-1)` | 461 |
| `.hotkey-conflict` | font-size | `var(--font-ui-small)` | 463 |
| `.setting-file-folder` | gap | `var(--size-4-2)` | 471 |
| `.setting-file-folder .setting-item-input` | width | `200px` | 475 |
| `.setting-page-title` | font-size | `var(--font-ui-larger)` | 487 |
| `.setting-page-title` | font-weight | `var(--font-bold)` | 488 |
| `.setting-page-title` | margin | `var(--size-4-2) var(--size-2-1) var(--size-2-3)` | 489 |
| `.setting-item` | gap | `var(--size-4-4)` | 498 |
| `.setting-item` | padding | `var(--size-2-7) var(--size-4-4)` | 499 |
| `.setting-item` | margin-bottom | `var(--size-2-5)` | 500 |
| `.setting-item` | border-radius | `var(--radius-ml)` | 502 |
| `.setting-item-info` | margin-right | `var(--size-4-5)` | 507 |
| `.setting-item-name` | font-size | `var(--font-ui-normal)` | 512 |
| `.setting-item-description` | font-size | `var(--font-ui-small)` | 517 |
| `.setting-item-description` | line-height | `1.4` | 518 |
| `.setting-item-description` | margin-top | `var(--size-2-1)` | 519 |
| `.setting-item-control` | gap | `var(--size-4-2)` | 531 |
| `.setting-item-heading` | padding | `var(--size-2-5) var(--size-2-1) var(--size-4-1)` | 536 |
| `.setting-item-heading` | margin-bottom | `var(--size-4-1)` | 537 |
| `.setting-item-heading` | margin-top | `var(--size-4-2)` | 538 |
| `.setting-item-heading` | border-radius | `0` | 540 |
| `.setting-item-heading:first-child` | margin-top | `0` | 544 |
| `.setting-item-heading .setting-item-name` | font-size | `16px` | 548 |
| `.setting-item-heading .setting-item-name` | font-weight | `var(--font-semibold)` | 549 |
| `.checkbox-container` | width | `42px` | 562 |
| `.checkbox-container` | height | `24px` | 563 |
| `.checkbox-container` | border-radius | `var(--radius-l)` | 564 |
| `.checkbox-container::after` | top | `3px` | 573 |
| `.checkbox-container::after` | left | `3px` | 574 |
| `.checkbox-container::after` | width | `18px` | 575 |
| `.checkbox-container::after` | height | `18px` | 576 |
| `.checkbox-container::after` | border-radius | `50%` | 577 |
| `.checkbox-container.is-enabled::after` | left | `21px` | 587 |
| `.setting-item-input` | border-radius | `var(--radius-sm)` | 601 |
| `.setting-item-input` | padding | `var(--size-2-3) var(--size-4-2)` | 602 |
| `.setting-item-input` | font-size | `var(--font-ui-medium)` | 604 |
| `textarea.setting-item-input` | min-height | `60px` | 613 |
| `textarea.setting-item-input` | width | `240px` | 615 |
| `.setting-secret .setting-item-input` | padding-right | `var(--size-4-8)` | 628 |
| `.setting-secret .setting-item-input` | width | `200px` | 629 |
| `.setting-secret-reveal` | right | `4px` | 634 |
| `.setting-secret-reveal` | width | `24px` | 638 |
| `.setting-secret-reveal` | height | `24px` | 639 |
| `.setting-secret-reveal` | border-radius | `var(--radius-s)` | 640 |
| `.setting-secret-reveal svg` | width | `15px` | 650 |
| `.setting-secret-reveal svg` | height | `15px` | 651 |
| `select.dropdown` | border-radius | `var(--radius-sm)` | 665 |
| `select.dropdown` | padding | `5px var(--size-4-7) 5px var(--size-4-2)` | 666 |
| `select.dropdown` | font-size | `var(--font-ui-medium)` | 668 |
| `input.slider` | width | `150px` | 681 |
| `input.slider` | height | `4px` | 682 |
| `input.slider` | border-radius | `var(--radius-xxs)` | 683 |
| `.slider-value` | min-width | `4.5em` | 692 |
| `.slider-value` | font-size | `var(--font-ui-medium)` | 694 |
| `input.slider::-webkit-slider-thumb` | width | `16px` | 701 |
| `input.slider::-webkit-slider-thumb` | height | `16px` | 702 |
| `input.slider::-webkit-slider-thumb` | border-radius | `50%` | 703 |
| `input.slider::-moz-range-thumb` | width | `16px` | 708 |
| `input.slider::-moz-range-thumb` | height | `16px` | 709 |
| `input.slider::-moz-range-thumb` | border-radius | `50%` | 711 |
| `input.color-input` | width | `28px` | 718 |
| `input.color-input` | height | `28px` | 719 |
| `input.color-input` | padding | `0` | 720 |
| `input.color-input` | border-radius | `50%` | 722 |
| `input.color-input::-webkit-color-swatch-wrapper` | padding | `0` | 729 |
| `input.color-input::-webkit-color-swatch` | border-radius | `50%` | 734 |
| `button.setting-button` | gap | `var(--size-2-3)` | 742 |
| `button.setting-button` | border-radius | `var(--radius-sm)` | 746 |
| `button.setting-button` | padding | `var(--size-2-3) var(--size-4-3)` | 747 |
| `button.setting-button` | font-size | `var(--font-ui-medium)` | 749 |
| `button.setting-button svg` | width | `var(--icon-s)` | 775 |
| `button.setting-button svg` | height | `var(--icon-s)` | 776 |
| `.extra-setting-button` | width | `28px` | 784 |
| `.extra-setting-button` | height | `28px` | 785 |
| `.extra-setting-button` | border-radius | `var(--radius-sm)` | 786 |
| `.extra-setting-button svg` | width | `var(--icon-s)` | 797 |
| `.extra-setting-button svg` | height | `var(--icon-s)` | 798 |
| `.search-input` | width | `100%` | 816 |
| `.search-input` | border-radius | `var(--radius-sm)` | 820 |
| `.search-input` | padding | `var(--size-2-3) var(--size-4-7) var(--size-2-3) var(--size-2-15)` | 821 |
| `.search-input` | font-size | `var(--font-ui-medium)` | 823 |
| `.search-input-container::before` | left | `9px` | 842 |
| `.search-input-container::before` | width | `var(--icon-xs)` | 843 |
| `.search-input-container::before` | height | `var(--icon-xs)` | 844 |
| `.search-input-clear-button` | right | `6px` | 853 |
| `.search-input-clear-button` | width | `18px` | 855 |
| `.search-input-clear-button` | height | `18px` | 856 |
| `.search-input-clear-button` | border-radius | `50%` | 857 |
| `.search-input-clear-button::before, .search-input-clear-button::after` | top | `50%` | 869 |
| `.search-input-clear-button::before, .search-input-clear-button::after` | left | `50%` | 870 |
| `.search-input-clear-button::before, .search-input-clear-button::after` | width | `10px` | 871 |
| `.search-input-clear-button::before, .search-input-clear-button::after` | height | `2px` | 872 |
| `.setting-tree` | padding-bottom | `var(--size-4-2)` | 885 |
| `.setting-tree-row.setting-item` | padding-top | `var(--size-2-2)` | 889 |
| `.setting-tree-row.setting-item` | padding-bottom | `var(--size-2-2)` | 890 |
| `.setting-tree-row.setting-item` | padding-left | `calc(var(--setting-tree-depth, 0) * var(--setting-tree-indent))` | 891 |
| `.setting-tree-row .setting-item-name` | gap | `var(--size-2-2)` | 898 |
| `.setting-tree-row .collapse-icon` | width | `var(--icon-s)` | 907 |
| `.setting-tree-row .collapse-icon svg, .setting-tree-icon svg` | width | `var(--icon-s)` | 918 |
| `.setting-tree-row .collapse-icon svg, .setting-tree-icon svg` | height | `var(--icon-s)` | 919 |

### `ui/core/toolbar.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.toolbar` | gap | `var(--size-2-1)` | 20 |
| `.toolbar` | padding | `var(--size-4-1)` | 21 |
| `.toolbar` | border-radius | `var(--radius-ml)` | 22 |
| `.toolbar` | font-size | `var(--font-ui-small)` | 27 |
| `.toolbar-handle` | min-width | `16px` | 48 |
| `.toolbar-handle` | margin-left | `var(--size-2-1)` | 49 |
| `.toolbar-handle` | padding | `0 1px` | 50 |
| `.toolbar-handle` | border-radius | `var(--radius-sm)` | 51 |
| `.toolbar-handle svg` | width | `var(--icon-xs)` | 69 |
| `.toolbar-handle svg` | height | `var(--icon-xs)` | 70 |
| `.toolbar.mod-vertical .toolbar-separator` | width | `auto` | 83 |
| `.toolbar.mod-vertical .toolbar-separator` | height | `1px` | 84 |
| `.toolbar.mod-vertical .toolbar-separator` | margin | `var(--size-4-1) var(--size-2-1)` | 86 |
| `.toolbar.mod-vertical .toolbar-section` | grid-template-columns | `repeat(var(--toolbar-columns, 2), auto)` | 98 |
| `.toolbar.mod-vertical .toolbar-options` | grid-template-columns | `repeat(var(--toolbar-columns, 2), auto)` | 110 |
| `.toolbar.mod-vertical .toolbar-handle` | margin-left | `0` | 115 |
| `.toolbar.mod-vertical .toolbar-handle` | margin-top | `var(--size-2-1)` | 116 |
| `.toolbar.mod-vertical .toolbar-handle` | min-width | `0` | 117 |
| `.toolbar.mod-vertical .toolbar-handle` | min-height | `16px` | 118 |
| `.toolbar.mod-vertical .toolbar-handle` | padding | `1px 0` | 119 |
| `.toolbar-section` | gap | `var(--size-2-1)` | 129 |
| `.toolbar-item` | gap | `var(--size-2-3)` | 143 |
| `.toolbar-item` | padding | `5px 7px` | 144 |
| `.toolbar-item` | border-radius | `var(--radius-sm)` | 145 |
| `.toolbar-item-icon` | width | `var(--icon-m)` | 179 |
| `.toolbar-item-icon` | height | `var(--icon-m)` | 180 |
| `.toolbar-item-icon svg` | width | `var(--icon-s)` | 185 |
| `.toolbar-item-icon svg` | height | `var(--icon-s)` | 186 |
| `.toolbar-separator` | width | `1px` | 199 |
| `.toolbar-separator` | margin | `var(--size-2-1) var(--size-4-1)` | 201 |
| `.toolbar-item.mod-options` | padding | `3px var(--size-4-1)` | 208 |
| `.toolbar-options` | gap | `var(--size-2-1)` | 220 |
| `.toolbar-option` | width | `24px` | 227 |
| `.toolbar-option` | height | `24px` | 228 |
| `.toolbar-option` | padding | `0` | 229 |
| `.toolbar-option` | border-radius | `var(--radius-sm)` | 231 |
| `.toolbar-option svg` | width | `15px` | 247 |
| `.toolbar-option svg` | height | `15px` | 248 |
| `.toolbar-option-dot` | width | `12px` | 255 |
| `.toolbar-option-dot` | height | `12px` | 256 |
| `.toolbar-option-dot` | border-radius | `50%` | 257 |

### `ui/core/tooltip.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.tooltip` | max-width | `280px` | 8 |
| `.tooltip` | padding | `var(--size-4-1) var(--size-4-2)` | 9 |
| `.tooltip` | border-radius | `var(--radius-sm)` | 10 |
| `.tooltip` | font-size | `var(--font-ui-small)` | 15 |
| `.tooltip` | line-height | `1.35` | 16 |

### `ui/core/vault-switcher.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.workspace-drawer-vault-switcher` | margin-top | `auto` | 19 |
| `.workspace-drawer-vault-switcher` | gap | `var(--size-4-1)` | 22 |
| `.workspace-drawer-vault-switcher` | padding | `var(--size-2-3) var(--size-4-2)` | 23 |
| `.vault-switcher-action` | gap | `var(--size-2-3)` | 35 |
| `.vault-switcher-action` | min-width | `0` | 37 |
| `.vault-switcher-action` | padding | `var(--size-4-1) var(--size-2-3)` | 38 |
| `.vault-switcher-action` | border-radius | `var(--radius-s)` | 39 |
| `.vault-switcher-chevron svg` | width | `var(--icon-xs)` | 58 |
| `.vault-switcher-chevron svg` | height | `var(--icon-xs)` | 59 |
| `.vault-switcher-name` | font-size | `var(--font-ui-small)` | 63 |
| `.vault-switcher-name` | font-weight | `var(--font-medium)` | 64 |
| `.vault-switcher-buttons` | gap | `var(--size-2-1)` | 76 |
| `.vault-switcher-buttons .clickable-icon svg` | width | `15px` | 80 |
| `.vault-switcher-buttons .clickable-icon svg` | height | `15px` | 81 |

### `ui/core/window-controls.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.titlebar-button-container` | top | `0` | 11 |
| `.titlebar-button-container` | right | `0` | 12 |
| `.titlebar-button` | width | `var(--frame-button-width)` | 30 |
| `.titlebar-button` | height | `34px` | 31 |
| `.titlebar-button svg` | width | `var(--icon-xs)` | 51 |
| `.titlebar-button svg` | height | `var(--icon-xs)` | 52 |

### `ui/core/workspace/layer-menu.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.menu.layer-menu` | width | `280px` | 6 |
| `.menu.layer-menu` | max-width | `calc(100vw - 16px)` | 7 |
| `.layer-menu .menu-item` | gap | `var(--size-2-3)` | 12 |
| `.layer-menu .menu-item-title` | min-width | `0` | 16 |
| `.layer-menu-chevron` | width | `12px` | 33 |
| `.layer-menu-chevron` | margin-right | `calc(var(--size-2-1) * -1)` | 34 |
| `.layer-menu-chevron svg` | width | `10px` | 45 |
| `.layer-menu-chevron svg` | height | `10px` | 46 |
| `.layer-menu-action` | width | `22px` | 63 |
| `.layer-menu-action` | height | `22px` | 64 |
| `.layer-menu-action` | margin | `-3px calc(var(--size-4-1) * -1) -3px 0` | 65 |
| `.layer-menu-action` | border-radius | `var(--radius-s)` | 66 |
| `.layer-menu-action svg` | width | `var(--icon-xs)` | 71 |
| `.layer-menu-action svg` | height | `var(--icon-xs)` | 72 |
| `.layer-menu .layer-menu-child` | padding-left | `var(--size-4-12)` | 98 |

### `ui/core/workspace/view-icons.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.view-icon` | width | `22px` | 16 |
| `.view-icon` | height | `22px` | 17 |
| `.view-icon` | border-radius | `999px` | 18 |
| `.view-icon` | font-size | `var(--font-ui-smallest)` | 20 |
| `.view-icon` | font-weight | `var(--font-semibold)` | 21 |
| `.view-icon` | line-height | `1` | 23 |
| `.view-icon svg` | width | `13px` | 40 |
| `.view-icon svg` | height | `13px` | 41 |
| `.view-icons > .view-icon + .view-icon` | margin-left | `-7px` | 45 |
| `.view-icons.is-bubbles` | width | `41px` | 65 |
| `.view-icons.is-bubbles` | height | `41px` | 66 |
| `.view-icons.is-bubbles > .view-icon` | margin | `0` | 71 |
| `.view-icons.is-bubbles > [data-slot='a']` | left | `0` | 74 |
| `.view-icons.is-bubbles > [data-slot='a']` | top | `0` | 74 |
| `.view-icons.is-bubbles > [data-slot='b']` | left | `19px` | 75 |
| `.view-icons.is-bubbles > [data-slot='b']` | top | `0` | 75 |
| `.view-icons.is-bubbles > [data-slot='c']` | left | `19px` | 76 |
| `.view-icons.is-bubbles > [data-slot='c']` | top | `19px` | 76 |
| `.view-icons.is-bubbles > [data-slot='d']` | left | `0` | 77 |
| `.view-icons.is-bubbles > [data-slot='d']` | top | `19px` | 77 |
| `.view-icons.is-bubbles[data-count='2'] > [data-slot='b']` | left | `19px` | 79 |
| `.view-icons.is-bubbles[data-count='2'] > [data-slot='b']` | top | `19px` | 79 |
| `.view-icons.is-bubbles[data-count='more']` | width | `calc(522 * var(--k))` | 97 |
| `.view-icons.is-bubbles[data-count='more']` | height | `calc(522 * var(--k))` | 98 |
| `.view-icons.is-bubbles[data-count='more']::before` | left | `0` | 104 |
| `.view-icons.is-bubbles[data-count='more']::before` | top | `0` | 105 |
| `.view-icons.is-bubbles[data-count='more']::before` | width | `100%` | 106 |
| `.view-icons.is-bubbles[data-count='more']::before` | height | `100%` | 107 |
| `.view-icons.is-bubbles[data-count='more']::before` | border-radius | `50%` | 108 |
| `.view-icons.is-bubbles[data-count='more'] > .view-icon` | width | `calc(var(--d) * var(--k))` | 116 |
| `.view-icons.is-bubbles[data-count='more'] > .view-icon` | height | `calc(var(--d) * var(--k))` | 117 |
| `.view-icons.is-bubbles[data-count='more'] > .view-icon` | font-size | `calc(var(--d) * var(--k) * 0.45)` | 119 |
| `.view-icons.is-bubbles[data-count='more'] > .view-icon svg` | width | `60%` | 123 |
| `.view-icons.is-bubbles[data-count='more'] > .view-icon svg` | height | `60%` | 124 |
| `.view-icons.is-bubbles[data-count='more'] > .view-icon-more` | font-size | `calc(var(--d) * var(--k) * 0.48)` | 131 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='a']` | left | `calc(237.5 * var(--k))` | 135 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='a']` | top | `calc(228 * var(--k))` | 135 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='b']` | left | `calc(14.5 * var(--k))` | 136 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='b']` | top | `calc(203.5 * var(--k))` | 136 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='c']` | left | `calc(149.5 * var(--k))` | 137 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='c']` | top | `calc(44.5 * var(--k))` | 137 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='d']` | left | `calc(354 * var(--k))` | 138 |
| `.view-icons.is-bubbles[data-count='more'] > [data-slot='d']` | top | `calc(105 * var(--k))` | 138 |

### `ui/coreCommands/marginMode.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.workspace-leaf.is-margin-mode :is(.doc-scroll, .pdf-scroll):not(.workspace-leaf .workspace-leaf *)` | padding-right | `25%` | 21 |
| `.workspace-leaf.is-margin-mode .pdf-scroll:not(.workspace-leaf .workspace-leaf *)` | padding-right | `40%` | 27 |

### `ui/index.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `@font-face` | font-weight | `400` | 6 |
| `@font-face` | font-weight | `500` | 13 |
| `@font-face` | font-weight | `600` | 20 |
| `@font-face` | font-weight | `700` | 27 |
| `:root` | line-height | `1.5` | 55 |
| `:root` | font-weight | `400` | 56 |
| `body` | margin | `0` | 251 |
| `body` | min-width | `320px` | 252 |
| `body` | height | `100%` | 253 |
| `a` | font-weight | `500` | 341 |
| `html` | height | `100%` | 366 |
| `h1` | font-size | `3.2em` | 370 |
| `h1` | line-height | `1.1` | 371 |

### `ui/snap/snap.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.is-snapped` | min-width | `0` | 8 |
| `.is-snapped` | min-height | `0` | 9 |
| `.mod-vertical > .workspace-resize-handle:has(+ .is-snapped)::after, .mod-vertical > .is-snapped + .workspace-resize-handle::after` | top | `var(--header-height)` | 28 |
| `.workspace-snap-spines` | width | `100%` | 36 |
| `.workspace-snap-spines` | height | `100%` | 37 |
| `.mod-snap-row > .workspace-snap-spines > .workspace-snap-header` | margin | `0 calc(-1 * var(--border-width))` | 59 |
| `.workspace-snap-spine-list` | min-height | `0` | 68 |
| `.workspace-snap-spine` | gap | `var(--size-4-1)` | 83 |
| `.workspace-snap-spine` | font-size | `var(--font-ui-small)` | 87 |
| `.workspace-snap-spine-icon svg` | width | `var(--icon-m)` | 106 |
| `.workspace-snap-spine-icon svg` | height | `var(--icon-m)` | 107 |
| `.mod-snap-row .workspace-snap-spine` | width | `var(--snap-spine-size)` | 124 |
| `.mod-snap-row .workspace-snap-spine` | height | `100%` | 125 |
| `.mod-snap-row .workspace-snap-spine` | padding | `var(--size-4-2) 0` | 126 |
| `.mod-snap-row .workspace-snap-spine-title` | min-height | `0` | 135 |
| `.mod-snap-column .workspace-snap-spine` | height | `var(--snap-spine-size)` | 141 |
| `.mod-snap-column .workspace-snap-spine` | padding | `0 var(--size-4-3)` | 143 |
| `.mod-snap-column .workspace-snap-spine-title` | min-width | `0` | 152 |
| `.mod-snap-column > .workspace-snap-spines.mod-under-frame .workspace-snap-spine:first-child` | padding-right | `var(--frame-right-space)` | 162 |

### `ui/vault-manager/vault-manager.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.vault-manager` | inset | `0` | 11 |
| `.vault-manager` | font-size | `var(--font-ui-small)` | 16 |
| `.vault-manager-titlebar` | height | `32px` | 23 |
| `.vault-manager-close` | width | `var(--frame-button-width)` | 32 |
| `.vault-manager-close` | height | `32px` | 33 |
| `.vault-manager-body` | min-height | `0` | 50 |
| `.vault-manager-list` | gap | `var(--size-2-1)` | 59 |
| `.vault-manager-list` | padding | `var(--size-4-3) var(--size-4-2)` | 60 |
| `.vault-manager-empty` | padding | `var(--size-4-3)` | 67 |
| `.vault-manager-item` | gap | `var(--size-4-1)` | 74 |
| `.vault-manager-item` | border-radius | `var(--radius-sm)` | 75 |
| `.vault-manager-item` | padding | `var(--size-4-2) var(--size-2-5)` | 76 |
| `.vault-manager-item-infos` | min-width | `0` | 85 |
| `.vault-manager-item-name` | font-weight | `var(--font-semibold)` | 90 |
| `.vault-manager-item-path` | font-size | `var(--font-ui-smaller)` | 101 |
| `.vault-manager-item-forget` | padding | `0 var(--size-2-3)` | 122 |
| `.vault-manager-panel` | gap | `var(--size-4-7)` | 139 |
| `.vault-manager-panel` | padding | `var(--size-4-6) var(--size-4-10)` | 140 |
| `.vault-manager-title` | margin | `0` | 144 |
| `.vault-manager-title` | line-height | `0` | 147 |
| `.vault-manager-logo` | height | `34px` | 153 |
| `.vault-manager-logo` | width | `auto` | 154 |
| `.vault-manager-title-text` | width | `1px` | 160 |
| `.vault-manager-title-text` | height | `1px` | 161 |
| `.vault-manager-title-text` | margin | `-1px` | 162 |
| `.vault-manager-title-text` | padding | `0` | 163 |
| `.vault-manager-actions` | width | `100%` | 171 |
| `.vault-manager-actions` | max-width | `420px` | 172 |
| `.vault-manager-actions` | gap | `var(--size-2-7)` | 175 |
| `.vault-manager-actions` | padding | `var(--size-4-5)` | 176 |
| `.vault-manager-actions` | border-radius | `var(--radius-ml)` | 178 |
| `.vault-manager-action` | gap | `var(--size-4-4)` | 186 |
| `.vault-manager-action-title` | font-weight | `var(--font-semibold)` | 189 |
| `.vault-manager-action-desc` | margin-top | `var(--size-2-1)` | 193 |
| `.vault-manager-button` | padding | `7px var(--size-2-9)` | 198 |
| `.vault-manager-button` | border-radius | `var(--radius-sm)` | 200 |
| `.vault-manager-button` | font-size | `var(--font-ui-small)` | 203 |
| `.vault-manager-button` | font-weight | `var(--font-semibold)` | 204 |

### `ui/views/agent/agent-chat.css`

Aucune.

### `ui/views/agent/bar/bar.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.agent-bar-host` | right | `0` | 11 |
| `.agent-bar` | width | `min(var(--agent-bar-width), calc(100% - 32px))` | 21 |
| `.agent-bar` | padding-bottom | `var(--size-4-3)` | 22 |
| `.agent-bar-pill` | gap | `var(--size-2-3)` | 32 |
| `.agent-bar-pill` | padding | `var(--size-2-3) var(--size-4-3)` | 33 |
| `.agent-bar-pill` | border-radius | `999px` | 35 |
| `.agent-bar-pill` | font-size | `var(--font-ui-small)` | 40 |
| `.agent-bar-pill-icon svg` | width | `var(--icon-s)` | 58 |
| `.agent-bar-pill-icon svg` | height | `var(--icon-s)` | 59 |
| `.agent-bar-field` | width | `100%` | 65 |
| `.agent-bar-field > .workspace-leaf` | inset | `auto` | 76 |
| `.agent-bar-field > .workspace-leaf` | height | `auto` | 77 |
| `.agent-bar-field > .workspace-leaf` | min-height | `0` | 78 |

### `ui/views/agent/controls/controls.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.agent-control` | gap | `var(--size-2-1)` | 9 |
| `.agent-control` | max-width | `160px` | 10 |
| `.agent-control` | padding | `var(--size-2-1) var(--size-2-3)` | 11 |
| `.agent-control` | border-radius | `var(--chat-radius-sm)` | 12 |
| `.agent-control` | font-size | `var(--font-ui-smaller)` | 15 |
| `.agent-control-chevron svg` | width | `var(--icon-xs)` | 53 |
| `.agent-control-chevron svg` | height | `var(--icon-xs)` | 54 |
| `.agent-usage` | width | `var(--icon-m)` | 63 |
| `.agent-usage` | height | `var(--icon-m)` | 64 |
| `.agent-context-zone` | gap | `var(--size-2-2)` | 95 |
| `.agent-context-chip` | gap | `var(--size-2-2)` | 105 |
| `.agent-context-chip` | max-width | `100%` | 106 |
| `.agent-context-chip` | padding | `var(--size-2-1) var(--size-2-1) var(--size-2-1) var(--size-2-3)` | 107 |
| `.agent-context-chip` | border-radius | `999px` | 109 |
| `.agent-context-chip` | font-size | `var(--font-ui-smaller)` | 113 |
| `.agent-context-icon svg, .agent-context-remove svg` | width | `var(--icon-xs)` | 132 |
| `.agent-context-icon svg, .agent-context-remove svg` | height | `var(--icon-xs)` | 133 |
| `.agent-context-remove` | padding | `var(--size-2-1)` | 139 |
| `.agent-context-remove` | border-radius | `50%` | 140 |

### `ui/views/agent/incrusted-toolbar.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.toolbar.mod-incrusted` | left | `0` | 23 |
| `.toolbar.mod-incrusted` | top | `0` | 24 |

### `ui/views/agent/lasso/lasso.css`

Aucune.

### `ui/views/agent/policy/permission.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.agent-permission` | gap | `var(--size-4-2)` | 7 |
| `.agent-permission` | margin | `var(--size-4-1) 0` | 8 |
| `.agent-permission` | padding | `var(--size-4-3)` | 9 |
| `.agent-permission` | border-radius | `var(--chat-radius)` | 11 |
| `.agent-permission` | font-size | `var(--font-ui-small)` | 14 |
| `.agent-permission-header` | gap | `var(--size-4-2)` | 25 |
| `.agent-permission-header` | font-weight | `var(--font-semibold)` | 26 |
| `.agent-permission-icon svg, .agent-permission-mark svg` | width | `var(--icon-s)` | 37 |
| `.agent-permission-icon svg, .agent-permission-mark svg` | height | `var(--icon-s)` | 38 |
| `.agent-permission-detail` | margin | `0` | 42 |
| `.agent-permission-detail` | padding | `var(--size-4-2) var(--size-4-3)` | 43 |
| `.agent-permission-detail` | border-radius | `var(--code-radius)` | 44 |
| `.agent-permission-detail` | font-size | `var(--font-ui-smaller)` | 47 |
| `.agent-permission-detail` | max-height | `200px` | 50 |
| `.agent-permission-plan` | max-height | `320px` | 55 |
| `.agent-permission-plan` | padding | `var(--size-4-2) var(--size-4-3)` | 57 |
| `.agent-permission-plan` | border-radius | `var(--code-radius)` | 58 |
| `.agent-permission-actions` | gap | `var(--size-4-2)` | 70 |
| `.agent-permission-button` | padding | `var(--size-2-2) var(--size-4-3)` | 75 |
| `.agent-permission-button` | border-radius | `var(--button-radius)` | 77 |
| `.agent-permission-why` | padding | `var(--size-2-2) var(--size-4-2)` | 103 |
| `.agent-permission-why` | border-radius | `var(--input-radius)` | 105 |
| `.agent-permission.is-answered` | gap | `var(--size-2-3)` | 114 |
| `.agent-permission.is-answered` | padding | `var(--size-2-2) var(--size-4-2)` | 115 |

### `ui/views/agent/quick/quick.css`

Aucune.

### `ui/views/agent/side-chat/side-chat.css`

Aucune.

### `ui/views/agent/trace/trace.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-part.agent-trace-host` | margin | `0 0 var(--size-2-5)` | 16 |
| `.agent-trace` | font-size | `var(--font-ui-medium)` | 35 |
| `.agent-trace` | line-height | `var(--line-height-normal)` | 36 |
| `.chat-view .agent-trace .agent-trace-header` | gap | `var(--size-2-3)` | 48 |
| `.chat-view .agent-trace .agent-trace-header` | max-width | `100%` | 49 |
| `.chat-view .agent-trace .agent-trace-header` | min-height | `26px` | 50 |
| `.chat-view .agent-trace .agent-trace-header` | padding | `0 var(--size-2-3) 0 0` | 51 |
| `.chat-view .agent-trace .agent-trace-header` | border-radius | `var(--radius-sm)` | 52 |
| `.agent-trace-status` | width | `20px` | 72 |
| `.agent-trace-status` | height | `20px` | 73 |
| `.agent-trace-title` | min-width | `0` | 86 |
| `.agent-trace-chevron svg, .agent-trace-step-chevron svg` | width | `13px` | 108 |
| `.agent-trace-chevron svg, .agent-trace-step-chevron svg` | height | `13px` | 109 |
| `.agent-trace[data-single='true'] .agent-trace-detail` | margin-left | `0` | 132 |
| `.agent-fade` | min-width | `0` | 143 |
| `.agent-fade-layer` | min-width | `0` | 148 |
| `.agent-odometer-cell` | height | `1.5em` | 192 |
| `.agent-collapsible` | grid-template-rows | `0fr` | 217 |
| `.agent-collapsible[data-open='true']` | grid-template-rows | `1fr` | 222 |
| `.agent-collapsible-inner` | min-height | `0` | 227 |
| `.agent-trace-steps` | margin | `var(--size-2-1) 0 0` | 257 |
| `.agent-trace-steps` | padding | `0` | 258 |
| `.agent-trace-step:not(:last-child)::before` | left | `6.5px` | 276 |
| `.agent-trace-step:not(:last-child)::before` | top | `19px` | 277 |
| `.agent-trace-step:not(:last-child)::before` | bottom | `-7px` | 278 |
| `.agent-trace-step:not(:last-child)::before` | width | `1px` | 279 |
| `.agent-trace-row` | gap | `var(--size-2-3)` | 286 |
| `.agent-trace-row` | min-height | `26px` | 287 |
| `.agent-trace-row` | min-width | `0` | 288 |
| `.agent-trace-marker` | width | `14px` | 312 |
| `.agent-trace-marker` | height | `14px` | 313 |
| `.agent-dot` | width | `5px` | 317 |
| `.agent-dot` | height | `5px` | 318 |
| `.agent-dot` | border-radius | `50%` | 319 |
| `.agent-spinner` | width | `11px` | 329 |
| `.agent-spinner` | height | `11px` | 330 |
| `.agent-spinner` | border-radius | `50%` | 331 |
| `.agent-trace-verb` | max-width | `100%` | 343 |
| `.agent-trace-target` | min-width | `0` | 360 |
| `.agent-trace-chip` | gap | `var(--size-4-1)` | 366 |
| `.agent-trace-chip` | min-width | `0` | 367 |
| `.agent-trace-chip` | padding | `0 var(--size-2-3)` | 368 |
| `.agent-trace-chip` | border-radius | `var(--radius-sm)` | 369 |
| `.agent-trace-chip` | font-weight | `var(--font-medium)` | 372 |
| `.agent-trace-chip` | line-height | `1.6` | 373 |
| `.agent-trace-chip.is-mono` | font-size | `var(--font-ui-small)` | 386 |
| `.agent-trace-chip.is-mono` | font-weight | `var(--font-normal)` | 387 |
| `.agent-trace-chip-icon svg` | width | `12px` | 397 |
| `.agent-trace-chip-icon svg` | height | `12px` | 398 |
| `.agent-trace-detail` | margin-left | `var(--size-4-5)` | 414 |
| `.agent-trace-result` | margin | `var(--size-2-1) 0 var(--size-2-3)` | 418 |
| `.agent-trace-result` | padding | `7px var(--size-2-5)` | 419 |
| `.agent-trace-result` | border-radius | `var(--radius-m)` | 421 |
| `.agent-trace-result` | font-size | `11.5px` | 424 |
| `.agent-trace-result` | line-height | `1.55` | 425 |
| `.agent-trace-result` | max-height | `10.5em` | 428 |
| `.agent-trace-thinking` | max-height | `calc(1.55em * 4)` | 437 |
| `.agent-trace-thinking` | margin | `0 0 var(--size-2-3)` | 438 |
| `.agent-trace-thinking-text` | line-height | `1.55` | 445 |
| `.agent-trace-step[data-state='done'] .agent-trace-thinking` | max-height | `18em` | 453 |

### `ui/views/annotation/annotation.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.annotation-surface-trace` | left | `0` | 37 |
| `.annotation-surface-trace` | top | `0` | 38 |
| `.toolbar.annotation-toolbar:not(.mod-vertical)` | max-width | `calc(100% - 16px)` | 79 |

### `ui/views/artefacts/artefacts-menu.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.artefact-menu-row .menu-item-title` | min-width | `0` | 6 |
| `.artefact-menu-actions` | gap | `var(--size-2-1)` | 21 |
| `.artefact-menu-actions` | margin | `-3px calc(var(--size-4-1) * -1) -3px var(--size-4-2)` | 22 |
| `.artefact-menu-actions .layer-menu-action` | margin | `0` | 39 |

### `ui/views/chat/chat-view.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-view` | min-height | `0` | 16 |
| `.chat-view` | padding | `0 !important` | 17 |
| `.chat-pane` | min-width | `0` | 29 |
| `.chat-pane` | min-height | `0` | 30 |
| `.chat-view[data-mode='panel'], .chat-view[data-mode='page']` | height | `100%` | 36 |
| `.chat-view[data-mode='panel'] > .chat-pane > .chat-messages, .chat-view[data-mode='page'] > .chat-pane > .chat-messages` | min-height | `0` | 42 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-messages` | padding-right | `calc((100% + var(--chat-sidebar-w)) * var(--chat-reserve))` | 108 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-messages > .chat-doc > .chat-column` | width | `var(--chat-d)` | 131 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-messages > .chat-doc > .chat-column` | margin-left | `max(0px, calc((100% + var(--chat-sidebar-w) - var(--chat-d)) / 2 - var(--chat-sidebar-w)))` | 132 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-messages > .chat-doc > .chat-column` | margin-right | `0` | 133 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-messages > .chat-doc > .chat-column` | padding-left | `var(--doc-text-start)` | 134 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-messages > .chat-doc > .chat-column` | padding-right | `var(--doc-text-end)` | 135 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-composer` | width | `var(--chat-texte-w)` | 141 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-composer` | margin-left | `var(--chat-texte-x)` | 142 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-composer` | margin-right | `0` | 143 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-header` | padding-left | `0` | 152 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-header` | padding-right | `0` | 153 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-header > .chat-header-actions` | margin-right | `var(--size-4-2)` | 157 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-header > .chat-title` | margin-left | `var(--chat-texte-x)` | 161 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-header > .chat-header-toggle` | left | `8px` | 169 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-header > .chat-header-toggle` | top | `50%` | 170 |
| `.chat-view[data-layout='cowork']` | grid-template-columns | `var(--chat-sidebar-w) minmax(0, 1fr)` | 183 |
| `.chat-sidebar-zone` | min-width | `0` | 193 |
| `.chat-sidebar-zone > .chat-sidebar` | width | `260px` | 198 |
| `.chat-sidebar-zone > .chat-sidebar` | height | `100%` | 199 |
| `.chat-welcome` | gap | `var(--size-4-3)` | 212 |
| `.chat-welcome` | padding | `0 var(--size-4-4) var(--size-4-5)` | 213 |
| `.chat-welcome` | font-size | `22px` | 216 |
| `.chat-welcome` | line-height | `1.2` | 217 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-welcome` | font-size | `34px` | 227 |
| `.chat-view[data-mode='page'] > .chat-pane > .chat-welcome` | padding-bottom | `var(--size-4-6)` | 228 |
| `.chat-welcome-icon svg` | width | `1em` | 238 |
| `.chat-welcome-icon svg` | height | `1em` | 239 |
| `.chat-view[data-empty='true'] > .chat-pane > .chat-empty.chat-welcome:not([hidden])` | margin-top | `auto` | 250 |
| `.chat-view[data-empty='true'] > .chat-pane > .chat-composer` | margin-bottom | `auto` | 254 |
| `.chat-view[data-mode='page'][data-empty='true'] > .chat-pane > .chat-composer .chat-composer-input` | min-height | `48px` | 262 |
| `.chat-view[data-mode='page'][data-empty='true'] > .chat-pane > .chat-composer .chat-composer-input` | font-size | `14.5px` | 263 |
| `.chat-header` | gap | `var(--size-4-2)` | 269 |
| `.chat-header` | min-height | `40px` | 270 |
| `.chat-header` | padding | `var(--size-2-3) var(--size-4-2) var(--size-2-3) var(--size-2-7)` | 271 |
| `.chat-title` | min-width | `0` | 277 |
| `.chat-title` | font-size | `var(--font-ui-medium)` | 281 |
| `.chat-title` | font-weight | `var(--font-semibold)` | 282 |
| `.chat-header-actions` | gap | `var(--size-2-1)` | 288 |
| `.chat-empty` | padding | `var(--size-4-6) var(--size-4-4)` | 297 |
| `.chat-empty` | font-size | `var(--font-ui-medium)` | 299 |
| `.chat-view ::-webkit-scrollbar` | width | `6px` | 310 |
| `.chat-view ::-webkit-scrollbar` | height | `6px` | 310 |
| `.chat-view ::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 315 |
| `.chat-history` | min-height | `0` | 323 |
| `.chat-history` | padding | `var(--size-2-3)` | 325 |
| `.chat-history-item` | width | `100%` | 332 |
| `.chat-history-item` | padding | `7px var(--size-2-5)` | 333 |
| `.chat-history-item` | border-radius | `var(--chat-radius-sm)` | 334 |
| `.chat-history-item` | font-size | `var(--font-ui-medium)` | 338 |
| `.chat-view[data-mode='anchored'] .chat-header-actions` | gap | `var(--size-2-1)` | 360 |
| `.chat-view[data-mode='anchored'] > .chat-pane > .chat-messages` | max-height | `min(60vh, calc(var(--gutter-room, 60vh) - 130px))` | 366 |
| `.chat-view[data-mode='anchored'] > .chat-pane > .chat-messages` | min-height | `40px` | 367 |
| `.chat-view[data-mode='anchored'] > .chat-pane > .chat-composer` | margin | `var(--size-2-3) var(--size-4-2) var(--size-4-2)` | 371 |
| `.chat-view[data-mode='bar']` | height | `auto` | 380 |
| `.chat-view[data-mode='bar'] > .chat-pane > .chat-composer` | margin | `0` | 394 |

### `ui/views/chat/components/attachments.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-composer-files` | gap | `var(--size-2-3)` | 6 |
| `.chat-composer-files` | padding | `var(--size-2-1) var(--size-2-1) var(--size-2-3)` | 7 |
| `.chat-file` | gap | `var(--size-2-3)` | 18 |
| `.chat-file` | max-width | `220px` | 19 |
| `.chat-file` | border-radius | `var(--radius-m)` | 21 |
| `.chat-file` | font-size | `var(--font-ui-small)` | 24 |
| `.chat-file:not(:has(.chat-file-thumb))` | padding | `var(--size-2-3) var(--size-4-2)` | 29 |
| `.chat-file-thumb` | min-width | `32px` | 34 |
| `.chat-file-thumb` | min-height | `32px` | 35 |
| `.chat-file-thumb` | max-width | `220px` | 36 |
| `.chat-file-thumb` | max-height | `160px` | 37 |
| `.chat-composer-files .chat-file-chip` | min-height | `56px` | 42 |
| `.chat-composer-files .chat-file-thumb` | width | `56px` | 47 |
| `.chat-composer-files .chat-file-thumb` | height | `56px` | 48 |
| `.chat-file-icon svg` | width | `var(--icon-s)` | 59 |
| `.chat-file-icon svg` | height | `var(--icon-s)` | 60 |
| `.chat-file-chip .chat-file-remove` | top | `2px` | 72 |
| `.chat-file-chip .chat-file-remove` | right | `2px` | 73 |
| `.chat-file-chip .chat-file-remove` | width | `20px` | 74 |
| `.chat-file-chip .chat-file-remove` | height | `20px` | 75 |
| `.chat-part[data-part='attachment']` | margin | `var(--size-2-3) var(--size-2-3) 0 0` | 88 |

### `ui/views/chat/components/post.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-post` | padding | `var(--size-4-3) var(--size-4-4) var(--size-4-1)` | 12 |
| `.chat-post` | font-size | `var(--font-ui-large)` | 15 |
| `.chat-post` | line-height | `20px` | 16 |
| `.chat-post-row` | grid-template-columns | `var(--chat-post-gutter) minmax(0, 1fr)` | 22 |
| `.chat-post-row` | column-gap | `var(--chat-post-gap)` | 23 |
| `.chat-post-row.has-reply > .chat-post-gutter::after` | width | `2px` | 37 |
| `.chat-post-row.has-reply > .chat-post-gutter::after` | margin-top | `var(--size-4-1)` | 38 |
| `.chat-post-row.has-reply > .chat-post-gutter::after` | margin-bottom | `calc(var(--size-4-1) * -1)` | 39 |
| `.chat-post-row.has-reply > .chat-post-gutter::after` | border-radius | `var(--radius-hair)` | 40 |
| `.chat-post .chat-avatar[data-size='lg']` | width | `40px` | 45 |
| `.chat-post .chat-avatar[data-size='lg']` | height | `40px` | 46 |
| `.chat-post .chat-avatar[data-size='lg']` | font-size | `var(--font-ui-large)` | 47 |
| `.chat-post .chat-avatar[data-size='md']` | width | `40px` | 52 |
| `.chat-post .chat-avatar[data-size='md']` | height | `40px` | 53 |
| `.chat-post .chat-avatar[data-size='md']` | font-size | `var(--font-ui-large)` | 54 |
| `.chat-post .chat-avatar svg` | width | `20px` | 59 |
| `.chat-post .chat-avatar svg` | height | `20px` | 60 |
| `.chat-post-main` | min-width | `0` | 64 |
| `.chat-post-main` | padding-bottom | `var(--size-4-3)` | 65 |
| `.chat-post-head` | gap | `var(--size-4-1)` | 71 |
| `.chat-post-head` | min-width | `0` | 72 |
| `.chat-post-head` | padding-right | `var(--chat-post-head-reserve, 28px)` | 73 |
| `.chat-post-name` | font-weight | `var(--font-bold)` | 80 |
| `.chat-post-text` | margin-top | `var(--size-2-1)` | 90 |
| `.chat-post-text` | max-height | `calc(var(--chat-post-lines) * 20px)` | 94 |
| `.chat-post-text.chat-markdown .cm-content` | font-size | `var(--font-ui-large)` | 108 |
| `.chat-post-text.chat-markdown .cm-content` | line-height | `20px` | 109 |
| `.chat-post-chip` | gap | `var(--size-2-3)` | 115 |
| `.chat-post-chip` | margin-top | `var(--size-4-2)` | 116 |
| `.chat-post-chip` | padding | `var(--size-4-1) var(--size-2-5)` | 117 |
| `.chat-post-chip` | border-radius | `999px` | 119 |
| `.chat-post-chip` | font-size | `var(--font-ui-medium)` | 121 |
| `.chat-post-chip svg` | width | `var(--icon-xs)` | 125 |
| `.chat-post-chip svg` | height | `var(--icon-xs)` | 126 |
| `.chat-post-quote` | gap | `var(--size-2-3)` | 133 |
| `.chat-post-quote` | min-width | `0` | 134 |
| `.chat-post-quote` | margin-top | `var(--size-2-5)` | 135 |
| `.chat-post-quote` | padding | `var(--size-4-2) var(--size-4-3)` | 136 |
| `.chat-post-quote` | border-radius | `var(--radius-xl)` | 138 |
| `.chat-post-quote` | font-size | `var(--font-ui-medium)` | 139 |
| `.chat-post-quote` | line-height | `16px` | 140 |
| `.chat-post-quote svg` | width | `var(--icon-xs)` | 151 |
| `.chat-post-quote svg` | height | `var(--icon-xs)` | 152 |
| `.chat-post-quote-doc` | max-width | `45%` | 157 |
| `.chat-post-quote-doc` | font-weight | `var(--font-semibold)` | 160 |
| `.chat-post-quote-text` | min-width | `0` | 164 |
| `.chat-post-actions` | gap | `var(--size-4-7)` | 173 |
| `.chat-post-actions` | margin-top | `var(--size-2-5)` | 174 |
| `.chat-post-actions` | font-size | `var(--font-ui-medium)` | 176 |
| `.chat-post-action` | gap | `var(--size-2-3)` | 182 |
| `.chat-post-action svg` | width | `17px` | 187 |
| `.chat-post-action svg` | height | `17px` | 188 |
| `.chat-post-more` | margin-top | `var(--size-2-3)` | 197 |
| `.chat-post-more` | font-size | `var(--font-ui-normal)` | 199 |
| `.chat-post-empty` | padding | `var(--size-4-2) 0 var(--size-4-3)` | 203 |
| `.chat-post-row` | font-size | `var(--font-ui-normal)` | 212 |
| `.chat-post-row` | line-height | `18px` | 213 |
| `.chat-post .chat-avatar[data-size]` | width | `28px` | 216 |
| `.chat-post .chat-avatar[data-size]` | height | `28px` | 217 |
| `.chat-post .chat-avatar[data-size]` | font-size | `var(--font-ui-small)` | 218 |
| `.chat-post-text.chat-markdown .cm-content` | font-size | `var(--font-ui-normal)` | 221 |
| `.chat-post-text.chat-markdown .cm-content` | line-height | `18px` | 222 |
| `.chat-post-text` | max-height | `calc(var(--chat-post-lines) * 18px)` | 225 |
| `.chat-post-actions` | gap | `var(--size-2-9)` | 228 |
| `.chat-post-quote` | padding | `var(--size-2-3) var(--size-2-5)` | 235 |
| `.chat-post-quote` | border-radius | `var(--radius-l)` | 236 |

### `ui/views/chat/components/primitives.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-button` | gap | `var(--size-2-3)` | 29 |
| `.chat-button` | border-radius | `var(--chat-radius-sm)` | 30 |
| `.chat-button[data-size='sm']` | width | `26px` | 36 |
| `.chat-button[data-size='sm']` | height | `26px` | 36 |
| `.chat-button[data-size='md']` | width | `32px` | 37 |
| `.chat-button[data-size='md']` | height | `32px` | 37 |
| `.chat-button svg` | width | `15px` | 38 |
| `.chat-button svg` | height | `15px` | 38 |
| `.chat-button[data-variant='primary']` | border-radius | `999px` | 43 |
| `.chat-avatar[data-size='md']` | width | `28px` | 52 |
| `.chat-avatar[data-size='md']` | height | `28px` | 52 |
| `.chat-avatar[data-size='md']` | font-size | `11.5px` | 52 |

### `ui/views/chat/components/quote.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-composer-quote` | padding | `var(--size-2-2) var(--size-2-3) 0` | 7 |
| `.chat-quote` | gap | `var(--size-2-3)` | 17 |
| `.chat-quote` | min-width | `0` | 18 |
| `.chat-quote` | padding | `var(--size-2-2) var(--size-2-4)` | 19 |
| `.chat-quote` | border-radius | `var(--radius-xs)` | 21 |
| `.chat-quote` | font-size | `var(--font-ui-small)` | 24 |
| `.chat-quote-text` | min-width | `0` | 39 |
| `.chat-quote-thumb` | max-height | `40px` | 47 |
| `.chat-quote-thumb` | max-width | `80px` | 48 |
| `.chat-quote-thumb` | border-radius | `var(--radius-xs)` | 49 |
| `.chat-part[data-part='quote']` | margin-bottom | `var(--size-2-3)` | 61 |

### `ui/views/chat/components/sidebar.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-sidebar` | gap | `var(--size-2-3)` | 7 |
| `.chat-sidebar` | min-width | `0` | 8 |
| `.chat-sidebar` | min-height | `0` | 9 |
| `.chat-sidebar` | padding | `var(--size-4-2)` | 10 |
| `.chat-sidebar-top` | min-height | `28px` | 20 |
| `.chat-sidebar-search` | gap | `var(--size-4-2)` | 26 |
| `.chat-sidebar-search` | height | `30px` | 27 |
| `.chat-sidebar-search` | padding | `0 var(--size-2-5)` | 28 |
| `.chat-sidebar-search` | border-radius | `var(--chat-radius-sm)` | 30 |
| `.chat-sidebar-search svg` | width | `var(--icon-xs)` | 41 |
| `.chat-sidebar-search svg` | height | `var(--icon-xs)` | 42 |
| `.chat-sidebar-search input` | min-width | `0` | 48 |
| `.chat-sidebar-search input` | font-size | `var(--font-ui-medium)` | 49 |
| `.chat-sidebar-new` | gap | `var(--size-4-2)` | 62 |
| `.chat-sidebar-new` | height | `30px` | 63 |
| `.chat-sidebar-new` | padding | `0 var(--size-2-5)` | 64 |
| `.chat-sidebar-new` | border-radius | `var(--chat-radius-sm)` | 65 |
| `.chat-sidebar-new` | font-size | `var(--font-ui-medium)` | 66 |
| `.chat-sidebar-new svg` | width | `15px` | 85 |
| `.chat-sidebar-new svg` | height | `15px` | 86 |
| `.chat-sidebar-list` | min-height | `0` | 91 |
| `.chat-sidebar-list` | margin | `var(--size-4-1) calc(var(--size-4-1) * -1) 0` | 93 |
| `.chat-sidebar-list` | padding | `0 var(--size-4-1) var(--size-4-2)` | 94 |
| `.chat-sidebar-group-label` | padding | `var(--size-2-7) var(--size-2-5) var(--size-4-1)` | 98 |
| `.chat-sidebar-group-label` | font-size | `11.5px` | 99 |
| `.chat-sidebar-group-label:first-child` | padding-top | `var(--size-4-1)` | 104 |
| `.chat-sidebar-empty` | padding | `var(--size-2-5)` | 121 |
| `.chat-sidebar-empty` | font-size | `12.5px` | 122 |

### `ui/views/chat/components/thread.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-doc` | min-height | `100%` | 22 |
| `.chat-column` | gap | `var(--size-2-7)` | 30 |
| `.chat-column` | padding | `var(--size-4-3) var(--size-2-7)` | 31 |
| `.chat-message` | gap | `var(--size-4-1)` | 38 |
| `.chat-message` | font-size | `var(--font-text-size, 16px)` | 45 |
| `.chat-message` | line-height | `1.55` | 46 |
| `.chat-message-header` | gap | `var(--size-4-2)` | 58 |
| `.chat-message-header` | font-size | `var(--font-ui-medium)` | 59 |
| `.chat-message-header` | font-weight | `var(--font-semibold)` | 60 |
| `.chat-message-header` | line-height | `1.2` | 61 |
| `.chat-message-body` | gap | `var(--size-4-1)` | 68 |
| `.chat-message-body` | min-width | `0` | 69 |
| `.chat-message-body` | max-width | `100%` | 70 |
| `.chat-message[data-role='user'] > .chat-message-body` | max-width | `85%` | 74 |
| `.chat-message[data-role='user'] > .chat-message-body > .chat-message-content` | padding | `7px var(--size-4-3)` | 78 |
| `.chat-message[data-role='user'] > .chat-message-body > .chat-message-content` | border-radius | `var(--chat-radius)` | 79 |
| `.chat-message-content` | min-width | `0` | 84 |
| `.chat-text p` | margin | `0 0 0.6em` | 89 |
| `.chat-text p:last-child` | margin-bottom | `0` | 93 |
| `.chat-text code` | padding | `0.1em 0.35em` | 97 |
| `.chat-text code` | border-radius | `var(--radius-s)` | 98 |
| `.chat-text code` | font-size | `0.9em` | 101 |
| `.chat-code` | margin | `0.4em 0 0.6em` | 105 |
| `.chat-code` | padding | `var(--size-2-5) var(--size-4-3)` | 106 |
| `.chat-code` | border-radius | `var(--chat-radius-sm)` | 108 |
| `.chat-code` | font-size | `var(--font-ui-small)` | 111 |
| `.chat-code code` | padding | `0` | 115 |
| `.chat-message[data-state='streaming'] .chat-text > p:last-child::after` | width | `6px` | 123 |
| `.chat-message[data-state='streaming'] .chat-text > p:last-child::after` | height | `1em` | 124 |
| `.chat-message[data-state='streaming'] .chat-text > p:last-child::after` | margin-left | `var(--size-2-1)` | 125 |
| `.chat-message[data-state='streaming'] .chat-text > p:last-child::after` | border-radius | `var(--radius-hair)` | 127 |
| `.chat-markdown .cm-scroller` | line-height | `inherit` | 146 |
| `.chat-markdown .cm-editor .cm-content` | min-width | `0` | 154 |
| `.chat-markdown .cm-editor .cm-content` | padding | `0` | 155 |
| `.chat-markdown .cm-editor .cm-content` | font-size | `inherit` | 157 |
| `.chat-markdown .cm-editor .cm-content` | line-height | `inherit` | 158 |
| `.chat-markdown .cm-line` | padding | `0` | 164 |
| `.chat-message[data-state='streaming'] .chat-markdown .cm-content > .cm-line:last-child::after` | width | `6px` | 174 |
| `.chat-message[data-state='streaming'] .chat-markdown .cm-content > .cm-line:last-child::after` | height | `1em` | 175 |
| `.chat-message[data-state='streaming'] .chat-markdown .cm-content > .cm-line:last-child::after` | margin-left | `var(--size-2-1)` | 176 |
| `.chat-message[data-state='streaming'] .chat-markdown .cm-content > .cm-line:last-child::after` | border-radius | `var(--radius-hair)` | 178 |
| `.chat-typing` | gap | `3px` | 189 |
| `.chat-typing` | padding | `var(--size-2-3) 0` | 190 |
| `.chat-typing span` | width | `5px` | 194 |
| `.chat-typing span` | height | `5px` | 195 |
| `.chat-typing span` | border-radius | `999px` | 196 |
| `.chat-message-error` | margin-top | `var(--size-4-1)` | 210 |
| `.chat-message-error` | font-size | `12.5px` | 212 |
| `.chat-message-actions` | gap | `var(--size-2-1)` | 218 |
| `.chat-message-actions` | margin-left | `calc(var(--size-2-3) * -1)` | 219 |
| `.chat-composer` | gap | `var(--size-2-3)` | 238 |
| `.chat-composer` | margin | `var(--size-4-2) var(--size-4-3) var(--size-4-3)` | 239 |
| `.chat-composer` | padding | `var(--size-4-2) var(--size-4-2) var(--size-2-3) var(--size-4-3)` | 240 |
| `.chat-composer` | border-radius | `calc(var(--chat-radius) + 2px)` | 242 |
| `.chat-composer-input` | width | `100%` | 257 |
| `.chat-composer-input` | min-height | `22px` | 258 |
| `.chat-composer-input` | max-height | `200px` | 259 |
| `.chat-composer-input` | font-size | `13.5px` | 263 |
| `.chat-composer-input` | line-height | `var(--line-height-normal)` | 264 |
| `.chat-composer-bar` | gap | `var(--size-2-3)` | 277 |
| `.chat-composer-tools` | gap | `var(--size-4-1)` | 283 |
| `.chat-composer-tools` | min-width | `0` | 284 |
| `.chat-composer-tools-end` | gap | `var(--size-4-1)` | 291 |
| `.chat-composer-tools-end` | margin-left | `auto` | 292 |
| `.chat-message.has-attachment > .chat-message-body` | max-width | `100%` | 310 |
| `.chat-message.has-attachment > .chat-message-body` | gap | `0` | 311 |
| `.chat-message.has-attachment > .chat-message-body` | border-radius | `var(--chat-radius)` | 313 |
| `.chat-message.has-attachment > .chat-message-body > .chat-message-content` | padding | `9px var(--size-2-7) var(--size-2-5)` | 319 |
| `.chat-message.has-attachment > .chat-message-body > .chat-message-content` | border-radius | `0` | 321 |
| `.chat-message > .chat-message-body > .chat-attachment` | min-width | `0` | 328 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | gap | `var(--size-2-3)` | 336 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | width | `100%` | 337 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | min-width | `0` | 338 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | margin | `0` | 339 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | padding | `7px var(--size-4-3)` | 340 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | border-radius | `0` | 342 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | font-size | `var(--font-ui-small)` | 348 |
| `.chat-message > .chat-message-body > .chat-attachment > .chat-attachment-header` | line-height | `1.2` | 349 |
| `.chat-attachment-icon svg, .chat-attachment-open svg` | width | `var(--icon-xs)` | 371 |
| `.chat-attachment-icon svg, .chat-attachment-open svg` | height | `var(--icon-xs)` | 372 |
| `.chat-attachment-title` | min-width | `0` | 376 |
| `.chat-attachment-body .embed-content .cm-content` | min-width | `0` | 402 |
| `.chat-attachment-body .embed-content .cm-content` | padding | `0 var(--size-2-7) var(--size-2-5)` | 403 |
| `.chat-attachment-body .embed-content .cm-scroller` | line-height | `inherit` | 409 |
| `.chat-attachment-body .embed-content .cm-line` | padding | `0` | 413 |

### `ui/views/chat/scroll/scroll.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.chat-pane > .chat-jump-anchor` | height | `0` | 32 |
| `.chat-jump` | bottom | `var(--size-4-2)` | 40 |
| `.chat-jump` | left | `50%` | 41 |
| `.chat-jump` | gap | `var(--size-2-3)` | 45 |
| `.chat-jump` | padding | `var(--size-2-2) var(--size-4-3)` | 46 |
| `.chat-jump` | font-size | `var(--font-ui-small)` | 49 |
| `.chat-jump` | border-radius | `999px` | 53 |
| `.chat-jump-icon` | width | `var(--icon-s)` | 73 |
| `.chat-jump-icon` | height | `var(--icon-s)` | 74 |
| `.chat-jump-icon svg` | width | `100%` | 78 |
| `.chat-jump-icon svg` | height | `100%` | 79 |
| `.chat-pane > .chat-rail` | left | `var(--size-4-2)` | 97 |
| `.chat-pane > .chat-rail` | top | `50%` | 98 |
| `.chat-pane > .chat-rail` | gap | `var(--size-2-3)` | 104 |
| `.chat-pane > .chat-rail` | max-height | `70%` | 105 |
| `.chat-pane > .chat-rail` | padding | `var(--size-2-2) 0` | 107 |
| `.chat-rail-tick` | width | `calc(var(--chat-rail-tick-w) + var(--rail-p, 0) * var(--chat-rail-wave))` | 120 |
| `.chat-rail-tick` | height | `var(--size-2-1)` | 121 |
| `.chat-rail-tick` | border-radius | `999px` | 122 |
| `.chat-rail-tick::before` | inset | `-4px -6px` | 132 |
| `.chat-rail-tick.is-hovered, .chat-rail-tick:focus-visible` | width | `var(--chat-rail-tick-w-hover)` | 137 |
| `.chat-pane > .chat-rail-preview` | width | `var(--chat-rail-preview-w)` | 145 |
| `.chat-pane > .chat-rail-preview` | max-width | `calc(100% - 48px)` | 146 |
| `.chat-pane > .chat-rail-preview` | gap | `var(--size-2-3)` | 150 |
| `.chat-pane > .chat-rail-preview` | padding | `var(--size-4-2) var(--size-4-3)` | 151 |
| `.chat-pane > .chat-rail-preview` | border-radius | `var(--menu-radius)` | 154 |
| `.chat-pane > .chat-rail-preview` | font-size | `var(--font-ui-small)` | 157 |
| `.chat-rail-preview-question` | font-weight | `var(--font-semibold)` | 166 |
| `.chat-rail-preview-file` | gap | `var(--size-2-2)` | 185 |
| `.chat-rail-preview-file svg` | width | `var(--icon-xs)` | 190 |
| `.chat-rail-preview-file svg` | height | `var(--icon-xs)` | 191 |
| `.chat-pane > .chat-search` | gap | `var(--size-2-2)` | 198 |
| `.chat-pane > .chat-search` | margin | `var(--size-2-2) var(--size-4-3)` | 199 |
| `.chat-pane > .chat-search` | padding | `var(--size-2-1) var(--size-4-2)` | 200 |
| `.chat-pane > .chat-search` | border-radius | `var(--input-radius)` | 203 |
| `.chat-pane > .chat-search` | font-size | `var(--font-ui-small)` | 205 |
| `.chat-search-icon svg, .chat-search-button svg` | width | `var(--icon-xs)` | 219 |
| `.chat-search-icon svg, .chat-search-button svg` | height | `var(--icon-xs)` | 220 |
| `.chat-search-input` | min-width | `0` | 226 |
| `.chat-search-button` | padding | `var(--size-2-1)` | 242 |
| `.chat-search-button` | border-radius | `var(--clickable-icon-radius)` | 243 |
| `.chat-column > .chat-message.is-first-unread::before` | gap | `var(--size-4-2)` | 267 |
| `.chat-column > .chat-message.is-first-unread::before` | margin-bottom | `var(--size-4-2)` | 268 |
| `.chat-column > .chat-message.is-first-unread::before` | font-size | `var(--font-ui-smaller)` | 270 |
| `.chat-column > .chat-message.is-first-unread::before` | font-weight | `var(--font-semibold)` | 271 |
| `.chat-column > .chat-message.is-first-unread::before` | padding-bottom | `var(--size-2-1)` | 274 |
| `.chat-announcer` | width | `1px` | 293 |
| `.chat-announcer` | height | `1px` | 294 |

### `ui/views/doc-widget/anchor-link.css`

Aucune.

### `ui/views/doc-widget/doc-widget.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.doc-widget:not(.is-bare)::after` | inset | `0` | 11 |
| `.doc-widget:not(.is-bare)::after` | border-radius | `inherit` | 15 |
| `.doc-widget` | line-height | `var(--line-height-normal)` | 30 |
| `.doc-widget.is-bare` | border-radius | `var(--radius-ml)` | 45 |
| `.doc-widget.is-bare > .workspace-leaf` | inset | `auto` | 50 |
| `.doc-widget.is-bare > .workspace-leaf` | border-radius | `var(--radius-ml)` | 55 |
| `.doc-widget .view-header` | gap | `var(--size-4-1)` | 73 |
| `.doc-widget .view-header` | height | `calc(var(--doc-widget-icons-height, 38px) - 1px)` | 75 |
| `.doc-widget .view-header` | padding | `0 var(--size-2-3) 0 var(--doc-widget-icons-width, 10px)` | 76 |
| `.doc-widget .view-header-title-container` | margin-top | `0` | 83 |
| `.doc-widget .view-header .view-header-title` | font-size | `var(--font-ui-medium)` | 90 |
| `.doc-widget .view-header .view-header-title` | font-weight | `var(--font-semibold)` | 91 |
| `.doc-widget .view-actions` | gap | `var(--size-2-1)` | 97 |
| `.doc-widget.is-bare::after` | inset | `0` | 112 |
| `.doc-widget.is-bare::after` | border-radius | `inherit` | 115 |
| `.doc-widget.is-bare .view-content` | padding | `0` | 125 |
| `.doc-widget > .doc-widget-icons[type='button']` | top | `0` | 141 |
| `.doc-widget > .doc-widget-icons[type='button']` | left | `0` | 142 |
| `.doc-widget > .doc-widget-icons[type='button']` | height | `var(--doc-widget-icons-height, 38px)` | 147 |
| `.doc-widget > .doc-widget-icons[type='button']` | padding | `0 7px 0 11px` | 148 |
| `.doc-widget > .doc-widget-icons[type='button']` | border-radius | `999px` | 149 |
| `.doc-widget.is-collapsed` | width | `var(--doc-widget-icons-width, 40px) !important` | 164 |
| `.doc-widget.is-collapsed` | height | `var(--doc-widget-icons-height, 38px) !important` | 165 |
| `.doc-widget.is-collapsed` | border-radius | `999px !important` | 169 |
| `.doc-widget-cluster` | font-size | `var(--font-ui-medium)` | 200 |
| `.doc-widget-cluster-list` | max-height | `calc(var(--gutter-room, 60vh) - 48px)` | 206 |
| `.doc-widget-cluster-head` | gap | `var(--size-4-2)` | 229 |
| `.doc-widget-cluster-head` | padding | `var(--size-4-3) var(--size-4-10) var(--size-4-3) var(--size-4-4)` | 230 |
| `.doc-widget-cluster-head.is-visually-hidden` | width | `1px` | 235 |
| `.doc-widget-cluster-head.is-visually-hidden` | height | `1px` | 236 |
| `.doc-widget-cluster-head.is-visually-hidden` | padding | `0` | 237 |
| `.doc-widget-cluster-head > .doc-widget-cluster-open[type='button']` | min-width | `0` | 252 |
| `.doc-widget-cluster-head > .doc-widget-cluster-open[type='button']` | font-weight | `var(--font-semibold)` | 254 |
| `.doc-widget-cluster-corner` | top | `8px` | 268 |
| `.doc-widget-cluster-corner` | right | `8px` | 269 |
| `.doc-widget-cluster-corner` | gap | `var(--size-2-1)` | 273 |
| `.doc-widget-cluster-corner > .doc-widget-cluster-goto[type='button'], .doc-widget-cluster-corner > .doc-widget-cluster-more[type='button']` | gap | `var(--size-4-1)` | 281 |
| `.doc-widget-cluster-corner > .doc-widget-cluster-goto[type='button'], .doc-widget-cluster-corner > .doc-widget-cluster-more[type='button']` | height | `26px` | 282 |
| `.doc-widget-cluster-corner > .doc-widget-cluster-goto[type='button'], .doc-widget-cluster-corner > .doc-widget-cluster-more[type='button']` | border-radius | `999px` | 284 |
| `.doc-widget-cluster-corner > .doc-widget-cluster-goto[type='button'], .doc-widget-cluster-corner > .doc-widget-cluster-more[type='button']` | width | `26px` | 295 |
| `.doc-widget-cluster-corner > .doc-widget-cluster-goto[type='button'], .doc-widget-cluster-corner > .doc-widget-cluster-more[type='button']` | padding | `0` | 296 |
| `.doc-widget-cluster-corner svg` | width | `15px` | 314 |
| `.doc-widget-cluster-corner svg` | height | `15px` | 315 |
| `.doc-widget-cluster-corner > .doc-widget-cluster-more svg` | width | `17px` | 319 |
| `.doc-widget-cluster-corner > .doc-widget-cluster-more svg` | height | `17px` | 320 |
| `.doc-widget-cluster-preview` | max-height | `420px` | 337 |
| `.doc-widget-cluster-preview-inner > .workspace-leaf` | inset | `auto` | 354 |
| `.doc-widget-cluster-preview-inner > .workspace-leaf` | height | `auto` | 355 |
| `.doc-widget-cluster:not(.doc-widget *)` | height | `100%` | 366 |
| `.doc-widget-cluster:not(.doc-widget *) .doc-widget-cluster-list` | max-height | `none` | 371 |
| `.doc-widget-cluster:not(.doc-widget *) .doc-widget-cluster-list` | height | `100%` | 372 |
| `.doc-widget-cluster-card` | width | `calc(100% - 24px)` | 382 |
| `.doc-widget-cluster-card` | max-width | `600px` | 383 |
| `.doc-widget-cluster-empty` | padding | `var(--size-4-6) var(--size-4-4)` | 387 |
| `.doc-widget-cluster-bar` | top | `0` | 402 |
| `.doc-widget-cluster-bar` | gap | `var(--size-2-3)` | 406 |
| `.doc-widget-cluster-bar` | padding | `var(--size-4-2) var(--size-4-3)` | 407 |
| `.doc-widget-cluster-bar .search-input` | border-radius | `999px` | 412 |
| `.doc-widget-cluster-bar > .doc-widget-cluster-filter[type='button']` | border-radius | `999px` | 419 |
| `.doc-widget-cluster-bar > .doc-widget-cluster-filter svg` | width | `var(--icon-s)` | 436 |
| `.doc-widget-cluster-bar > .doc-widget-cluster-filter svg` | height | `var(--icon-s)` | 437 |
| `.doc-widget-cluster-card` | margin | `var(--size-4-1) var(--size-4-3) var(--size-4-3)` | 449 |
| `.doc-widget-cluster-card` | border-radius | `var(--radius-ml)` | 451 |
| `.doc-widget-cluster-passage-header` | gap | `var(--size-2-3)` | 464 |
| `.doc-widget-cluster-passage-header` | padding | `7px var(--size-4-3)` | 465 |
| `.doc-widget-cluster-passage-header` | font-size | `var(--font-ui-small)` | 467 |
| `.doc-widget-cluster-passage-header` | line-height | `1.2` | 468 |
| `.doc-widget-cluster-passage-header svg` | width | `var(--icon-xs)` | 472 |
| `.doc-widget-cluster-passage-header svg` | height | `var(--icon-xs)` | 473 |
| `.doc-widget-cluster-passage-body` | padding | `0 var(--size-4-3) var(--size-4-2)` | 477 |
| `.doc-widget-cluster:not(.doc-widget *) .doc-widget-cluster-bar` | width | `100%` | 485 |
| `.doc-widget-cluster:not(.doc-widget *) .doc-widget-cluster-bar` | max-width | `624px` | 486 |
| `.doc-widget-cluster-list::-webkit-scrollbar` | width | `6px` | 492 |
| `.doc-widget-cluster-list::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 496 |

### `ui/views/explorer/explorer.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.file-explorer` | font-size | `var(--font-ui-small)` | 6 |
| `.nav-header` | padding | `var(--size-4-2) var(--size-4-2) var(--size-4-1)` | 12 |
| `.nav-buttons-container` | gap | `var(--size-4-1)` | 18 |
| `.nav-action-button` | padding | `var(--size-2-3)` | 25 |
| `.nav-action-button svg` | width | `var(--icon-m)` | 29 |
| `.nav-action-button svg` | height | `var(--icon-m)` | 30 |
| `.nav-files-container` | min-height | `0` | 38 |
| `.nav-files-container` | padding | `0 var(--size-4-1) var(--size-2-3)` | 40 |
| `.nav-files-container::-webkit-scrollbar` | width | `6px` | 43 |
| `.nav-files-container::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 46 |
| `.file-explorer .tree-item-self.is-drop-target` | border-radius | `var(--radius-s)` | 53 |
| `.file-tree` | min-height | `100%` | 57 |
| `.nav-rename-input` | width | `100%` | 64 |
| `.nav-rename-input` | padding | `0` | 69 |
| `.nav-rename-input` | margin | `0` | 70 |

### `ui/views/html/html-view.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.html-view` | padding | `0` | 8 |
| `.html-sizer` | width | `100%` | 13 |
| `.html-plane` | left | `0` | 23 |
| `.html-plane` | top | `0` | 24 |
| `.html-plane` | width | `0` | 25 |
| `.html-plane` | height | `0` | 26 |
| `.html-page` | width | `100%` | 33 |

### `ui/views/image/image-view.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.image-scroll` | min-height | `0` | 16 |
| `.image-scroll` | height | `100%` | 17 |
| `.image-scroll` | width | `100%` | 18 |
| `.image-scroll` | padding | `1rem` | 34 |
| `linear-gradient(45deg, var(--background-modifier-border) 25%, transparent 25%), linear-gradient(-45deg, var(--background-modifier-border) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, var(--background-modifier-border) 75%), .image-scroll::-webkit-scrollbar` | width | `6px` | 39 |
| `linear-gradient(45deg, var(--background-modifier-border) 25%, transparent 25%), linear-gradient(-45deg, var(--background-modifier-border) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, var(--background-modifier-border) 75%), .image-scroll::-webkit-scrollbar` | height | `6px` | 39 |
| `.image-scroll::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 43 |
| `.image-frame` | margin | `auto` | 56 |
| `.image-frame` | line-height | `0` | 57 |
| `.image-plane` | left | `0` | 73 |
| `.image-plane` | top | `0` | 74 |
| `.image-plane` | width | `0` | 75 |
| `.image-plane` | height | `0` | 76 |
| `.image-message` | padding | `3rem 1rem` | 83 |
| `.image-message` | font-size | `var(--font-ui-small)` | 86 |

### `ui/views/markdown/editor.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.cm-editor` | height | `auto` | 3 |
| `.doc-editor .cm-content` | min-width | `0` | 28 |
| `.cm-editor .cm-content` | font-size | `var(--font-text-size, 16px)` | 36 |
| `.cm-editor .cm-content` | max-width | `none` | 37 |
| `.cm-editor .cm-content` | margin | `0` | 38 |
| `.cm-editor .cm-content` | padding | `2rem 0` | 39 |
| `.cm-hl` | border-radius | `var(--radius-xs)` | 60 |
| `.cm-hl` | padding | `0.05em 0.15em` | 61 |
| `.cm-swatch` | width | `0.8em` | 70 |
| `.cm-swatch` | height | `0.8em` | 71 |
| `.cm-swatch` | border-radius | `var(--radius-xs)` | 72 |
| `.cm-swatch` | margin | `0 0.3em` | 73 |
| `.cm-ob-strong` | font-weight | `var(--font-bold)` | 87 |
| `.cm-ob-code` | font-size | `0.9em` | 93 |
| `.cm-ob-code` | border-radius | `var(--radius-s)` | 95 |
| `.cm-ob-code` | padding | `0.1em 0.35em` | 96 |
| `.cm-ob-codeblock` | font-size | `0.9em` | 105 |
| `.cm-ob-codeblock-first` | border-top-left-radius | `var(--radius-sm)` | 113 |
| `.cm-ob-codeblock-first` | border-top-right-radius | `var(--radius-sm)` | 114 |
| `.cm-ob-codeblock-first` | padding-top | `0.7em` | 115 |
| `.cm-ob-codeblock-first` | min-height | `1.15em` | 116 |
| `.cm-ob-codeblock-last` | border-bottom-left-radius | `var(--radius-sm)` | 119 |
| `.cm-ob-codeblock-last` | border-bottom-right-radius | `var(--radius-sm)` | 120 |
| `.cm-ob-codeblock-last` | padding-bottom | `0.7em` | 121 |
| `.cm-ob-codeblock-last` | min-height | `1.15em` | 122 |
| `.cm-ob-code-lang` | font-size | `0.7em` | 128 |
| `.cm-ob-code-lang` | padding | `0.15em 0.2em 0.15em 0.6em` | 132 |
| `.cm-ob-quote` | padding-left | `0.8em` | 140 |
| `.cm-ob-h1, .cm-ob-h2, .cm-ob-h3, .cm-ob-h4, .cm-ob-h5, .cm-ob-h6` | font-weight | `var(--font-bold)` | 148 |
| `.cm-ob-h1, .cm-ob-h2, .cm-ob-h3, .cm-ob-h4, .cm-ob-h5, .cm-ob-h6` | line-height | `1.25` | 149 |
| `.cm-ob-h1` | font-size | `1.9em` | 153 |
| `.cm-ob-h2` | font-size | `1.6em` | 154 |
| `.cm-ob-h3` | font-size | `1.35em` | 155 |
| `.cm-ob-h4` | font-size | `1.18em` | 156 |
| `.cm-ob-h5` | font-size | `1.05em` | 157 |
| `.cm-ob-h6` | font-size | `1em` | 158 |
| `.cm-hr` | width | `100%` | 174 |
| `.cm-hr` | height | `0` | 175 |
| `.cm-hr` | margin | `0.4em 0` | 176 |
| `.cm-ob-check` | margin-right | `0.15em` | 185 |
| `.cm-swatch` | width | `0.8em` | 194 |
| `.cm-swatch` | height | `0.8em` | 195 |
| `.cm-swatch` | border-radius | `var(--radius-xs)` | 196 |
| `.cm-swatch` | margin | `0 0.3em` | 197 |
| `.cm-ob-bullet, .cm-task-checkbox-wrap` | width | `var(--indent-width)` | 218 |
| `.cm-ob-bullet::before` | width | `0.42em` | 235 |
| `.cm-ob-bullet::before` | height | `0.42em` | 236 |
| `.cm-ob-bullet::before` | border-radius | `50%` | 237 |
| `.cm-task-checkbox` | width | `1.05em` | 262 |
| `.cm-task-checkbox` | height | `1.05em` | 263 |
| `.cm-task-checkbox` | margin | `0` | 264 |
| `.cm-indent` | width | `var(--indent-width)` | 273 |
| `.cm-foldGutter` | width | `var(--doc-fold-width)` | 313 |
| `.cm-fold-chevron svg` | width | `var(--icon-s)` | 337 |
| `.cm-fold-chevron svg` | height | `var(--icon-s)` | 338 |
| `.cm-editor .cm-foldPlaceholder` | padding | `0 0.3em` | 364 |
| `.cm-math-inline mjx-container[jax="SVG"]` | margin | `0` | 377 |
| `.cm-math-inline.is-display` | max-width | `100%` | 385 |
| `.cm-math-inline.is-display mjx-container[jax="SVG"][display="true"]` | margin | `0.2em 0` | 391 |
| `.cm-math-block` | margin | `0` | 401 |
| `.cm-math-block` | border-radius | `var(--radius-sm)` | 406 |
| `.cm-math-block` | padding | `1.3em 0.4em` | 407 |
| `.cm-math-block mjx-container[jax="SVG"][display="true"]` | margin | `0 auto` | 416 |
| `.cm-math-tooltip` | border-radius | `var(--radius-sm)` | 432 |
| `.cm-math-tooltip` | padding | `0.35em 0.6em` | 433 |
| `.cm-math-tooltip` | font-size | `1.15em` | 436 |
| `.cm-math-tooltip` | max-width | `60ch` | 437 |

### `ui/views/markdown/embeds.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.cm-wikilink` | border-radius | `var(--radius-xs)` | 15 |
| `.cm-embed` | max-width | `100%` | 30 |
| `.cm-embed.is-block` | max-width | `100%` | 42 |
| `.cm-embed.is-block:has(.cm-embed-image)` | width | `fit-content` | 48 |
| `.cm-embed-resize` | right | `3px` | 60 |
| `.cm-embed-resize` | bottom | `3px` | 61 |
| `.cm-embed-resize` | width | `24px` | 62 |
| `.cm-embed-resize` | height | `24px` | 63 |
| `.cm-embed.is-unresolved, .cm-embed-fallback` | padding | `0 var(--size-4-1)` | 83 |
| `.cm-embed.is-unresolved, .cm-embed-fallback` | border-radius | `var(--radius-xs)` | 84 |
| `.cm-embed-image` | width | `100%` | 97 |
| `.cm-embed-image` | height | `auto` | 98 |
| `.cm-embed-image` | border-radius | `var(--radius-s)` | 99 |
| `.cm-embed-image` | margin | `var(--size-4-1) 0` | 100 |
| `.cm-embed .cm-embed-content:not(:has(.cm-embed-image))` | border-radius | `var(--radius-s)` | 108 |
| `.cm-embed .cm-embed-content:not(:has(.cm-embed-image))` | margin | `var(--size-2-3) 0` | 110 |
| `.cm-embed-title` | font-size | `0.8em` | 113 |
| `.cm-embed-title` | padding | `var(--size-4-1) var(--size-2-5) 0` | 115 |
| `.cm-embed-notice` | padding | `var(--size-4-2) var(--size-2-5)` | 135 |
| `.cm-embed-notice` | font-size | `0.8em` | 136 |

### `ui/views/markdown/html-markdown.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.cm-html-block` | padding | `var(--size-2-1) var(--size-2-1) var(--size-2-1) var(--size-2-3)` | 13 |
| `.cm-html-block > .edit-block-button` | top | `4px` | 21 |
| `.cm-html-block > .edit-block-button` | right | `4px` | 22 |
| `.cm-html-block > .edit-block-button` | padding | `3px` | 24 |
| `.cm-html-block > .edit-block-button` | border-radius | `var(--radius-s)` | 25 |
| `.cm-html-block > .edit-block-button svg` | width | `var(--icon-s)` | 39 |
| `.cm-html-block > .edit-block-button svg` | height | `var(--icon-s)` | 40 |

### `ui/views/markdown/markdown-view.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.doc-scroll` | min-height | `0` | 31 |
| `.doc-scroll` | height | `100%` | 32 |
| `.doc-scroll` | width | `100%` | 40 |
| `.doc-scroll::-webkit-scrollbar` | width | `6px` | 46 |
| `.doc-scroll::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 50 |
| `.doc-sizer` | max-width | `var(--doc-width)` | 59 |
| `.doc-sizer` | margin | `0 auto` | 60 |
| `.doc-sizer` | padding | `0 var(--doc-gutter)` | 61 |
| `.inline-title` | padding | `2rem 0 0.5rem var(--size-4-6)` | 65 |
| `.inline-title` | font-size | `2.2em` | 67 |
| `.inline-title` | font-weight | `var(--font-bold)` | 68 |
| `.inline-title` | line-height | `1.2` | 69 |
| `.doc-editor` | width | `100%` | 81 |

### `ui/views/markdown/properties/properties.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.metadata-container` | padding | `0.4em var(--size-2-1) 1.2em var(--size-2-3)` | 25 |
| `.metadata-properties-heading` | padding | `0.2em 0 0.5em` | 34 |
| `.metadata-properties-title` | font-size | `var(--font-text-size, 16px)` | 37 |
| `.metadata-properties-title` | font-weight | `var(--font-semibold)` | 38 |
| `.metadata-content` | font-size | `0.9375em` | 46 |
| `.metadata-properties` | margin-left | `calc(-1 * var(--metadata-icon-inset))` | 51 |
| `.metadata-properties` | gap | `var(--size-2-1)` | 54 |
| `.metadata-property` | min-height | `30px` | 63 |
| `.metadata-property` | min-width | `0` | 64 |
| `.metadata-property` | border-radius | `var(--radius-sm)` | 66 |
| `.metadata-property-key` | min-width | `4em` | 85 |
| `.metadata-property-key` | border-radius | `var(--input-radius) 0 0 var(--input-radius)` | 86 |
| `.metadata-property-icon` | width | `calc(16px + 2 * var(--metadata-icon-inset))` | 97 |
| `.metadata-property-icon` | height | `24px` | 98 |
| `.metadata-property-icon` | border-radius | `var(--radius-s)` | 99 |
| `.metadata-property-icon svg` | width | `var(--icon-s)` | 108 |
| `.metadata-property-icon svg` | height | `var(--icon-s)` | 109 |
| `.metadata-property-key-input, .metadata-input` | width | `0` | 118 |
| `.metadata-property-key-input, .metadata-input` | min-width | `0` | 119 |
| `.metadata-property-key-input, .metadata-input` | height | `100%` | 120 |
| `.metadata-property-key-input, .metadata-input` | padding | `0 var(--metadata-value-pad)` | 121 |
| `.metadata-property-value` | min-width | `0` | 147 |
| `.metadata-input-checkbox` | margin | `0 0 0 var(--metadata-value-pad)` | 152 |
| `.metadata-input-checkbox` | width | `15px` | 153 |
| `.metadata-input-checkbox` | height | `15px` | 154 |
| `.multi-select-container` | gap | `var(--size-4-1)` | 164 |
| `.multi-select-container` | padding-block | `3px` | 165 |
| `.multi-select-container` | min-width | `0` | 167 |
| `.multi-select-container` | padding | `0 var(--metadata-value-pad)` | 168 |
| `.multi-select-pill` | gap | `var(--size-2-1)` | 174 |
| `.multi-select-pill` | max-width | `100%` | 175 |
| `.multi-select-pill` | padding | `1px var(--size-4-1) 1px var(--size-4-2)` | 176 |
| `.multi-select-pill` | border-radius | `999px` | 177 |
| `.multi-select-pill` | line-height | `var(--line-height-normal)` | 180 |
| `.multi-select-pill-remove-button` | padding | `1px` | 194 |
| `.multi-select-pill-remove-button` | border-radius | `50%` | 195 |
| `.multi-select-pill-remove-button svg` | width | `12px` | 203 |
| `.multi-select-pill-remove-button svg` | height | `12px` | 204 |
| `.multi-select-input` | width | `0` | 208 |
| `.multi-select-input` | min-width | `2em` | 209 |
| `.metadata-add-button` | height | `28px` | 221 |
| `.metadata-add-button` | margin-top | `var(--size-4-2)` | 222 |
| `.metadata-add-button` | margin-left | `calc(-1 * var(--metadata-icon-inset))` | 223 |
| `.metadata-add-button` | padding-right | `var(--size-4-2)` | 224 |
| `.metadata-add-button` | border-radius | `var(--input-radius)` | 225 |
| `.metadata-add-button` | line-height | `1` | 226 |
| `.metadata-add-button > span:first-child` | width | `calc(16px + 2 * var(--metadata-icon-inset))` | 238 |
| `.metadata-add-button > span:first-child` | margin-right | `var(--metadata-value-pad)` | 240 |
| `.metadata-add-button svg` | width | `var(--icon-s)` | 243 |
| `.metadata-add-button svg` | height | `var(--icon-s)` | 244 |
| `.metadata-drop-bar` | height | `2px` | 251 |
| `.metadata-drop-bar` | border-radius | `var(--radius-hair)` | 252 |
| `.cm-foldGutter .cm-gutterElement.cm-fold-properties` | padding-top | `calc(0.4em + 0.2em + 2px)` | 264 |

### `ui/views/markdown/table/table.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.cm-table-widget` | padding | `var(--size-2-7) var(--size-2-11) var(--size-4-5) var(--size-4-3)` | 14 |
| `.cm-table-widget::-webkit-scrollbar` | height | `6px` | 27 |
| `.cm-table-widget::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 31 |
| `.cm-table-cell` | min-width | `4em` | 59 |
| `.cm-table-cell` | padding | `0` | 60 |
| `.cm-table-cell` | font-weight | `normal` | 62 |
| `th.cm-table-cell` | font-weight | `var(--font-semibold)` | 65 |
| `.cm-table-cell .cm-editor` | min-width | `6em` | 75 |
| `.cm-table-cell .cm-scroller` | line-height | `var(--line-height-normal)` | 83 |
| `.cm-table-cell .cm-editor .cm-content` | padding | `var(--size-4-1) var(--size-4-2)` | 87 |
| `.cm-table-cell .cm-editor .cm-content` | min-height | `1.5em` | 88 |
| `.cm-table-cell .cm-line` | padding | `0` | 91 |
| `.cm-table-band` | border-radius | `var(--radius-s)` | 103 |
| `.cm-table-add-col` | left | `100%` | 111 |
| `.cm-table-add-col` | top | `0` | 112 |
| `.cm-table-add-col` | bottom | `0` | 113 |
| `.cm-table-add-col` | width | `18px` | 114 |
| `.cm-table-add-col` | margin-left | `var(--size-2-1)` | 115 |
| `.cm-table-add-row` | top | `100%` | 118 |
| `.cm-table-add-row` | left | `0` | 119 |
| `.cm-table-add-row` | right | `0` | 120 |
| `.cm-table-add-row` | height | `18px` | 121 |
| `.cm-table-add-row` | margin-top | `var(--size-2-1)` | 122 |
| `.cm-table-band-icon` | line-height | `0` | 126 |
| `.cm-table-band-icon svg` | width | `var(--icon-xs)` | 130 |
| `.cm-table-band-icon svg` | height | `var(--icon-xs)` | 131 |
| `.cm-table-tooltip` | padding | `var(--size-4-1) var(--size-4-2)` | 141 |
| `.cm-table-tooltip` | border-radius | `var(--radius-s)` | 142 |
| `.cm-table-tooltip` | font-size | `var(--font-ui-small)` | 143 |
| `.cm-table-grip-col` | height | `12px` | 175 |
| `.cm-table-grip-row` | width | `12px` | 179 |
| `.cm-table-grip-handle` | line-height | `0` | 184 |
| `.cm-table-grip-handle svg` | width | `var(--icon-xs)` | 189 |
| `.cm-table-grip-handle svg` | height | `var(--icon-xs)` | 190 |
| `.cm-table-grip-col.is-active` | border-radius | `var(--radius-sm) var(--radius-sm) 0 0` | 199 |
| `.cm-table-grip-col.is-active` | height | `14px` | 200 |
| `.cm-table-grip-row.is-active` | border-radius | `var(--radius-sm) 0 0 var(--radius-sm)` | 203 |
| `.cm-table-selection` | border-radius | `var(--radius-sm)` | 212 |
| `.cm-table-selection[data-kind='col']` | border-top-left-radius | `0` | 221 |
| `.cm-table-selection[data-kind='col']` | border-top-right-radius | `0` | 222 |
| `.cm-table-selection[data-kind='row']` | border-top-left-radius | `0` | 225 |
| `.cm-table-selection[data-kind='row']` | border-bottom-left-radius | `0` | 226 |
| `.cm-table-drop-indicator` | border-radius | `var(--radius-xxs)` | 235 |

### `ui/views/pdf/pdf-view.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.pdf-view` | min-width | `0` | 32 |
| `.pdf-toolbar` | height | `36px` | 39 |
| `.pdf-toolbar` | padding | `0 var(--size-4-2)` | 40 |
| `.pdf-toolbar` | font-size | `var(--font-ui-small)` | 45 |
| `.pdf-toolbar-left` | gap | `var(--size-2-1)` | 53 |
| `.pdf-toolbar-left` | min-width | `0` | 54 |
| `.pdf-toolbar .clickable-icon` | width | `26px` | 58 |
| `.pdf-toolbar .clickable-icon` | height | `26px` | 59 |
| `.pdf-toolbar .clickable-icon.pdf-toolbar-dropdown` | width | `20px` | 66 |
| `.pdf-toolbar .clickable-icon.pdf-toolbar-dropdown svg` | width | `var(--icon-xs)` | 69 |
| `.pdf-toolbar .clickable-icon.pdf-toolbar-dropdown svg` | height | `var(--icon-xs)` | 70 |
| `.pdf-toolbar-separator` | width | `1px` | 81 |
| `.pdf-toolbar-separator` | height | `16px` | 82 |
| `.pdf-toolbar-separator` | margin | `0 var(--size-2-3)` | 83 |
| `.pdf-page-numbers` | gap | `var(--size-2-3)` | 91 |
| `.pdf-page-input` | width | `44px` | 96 |
| `.pdf-page-input` | height | `24px` | 97 |
| `.pdf-page-input` | padding | `0 var(--size-2-3)` | 99 |
| `.pdf-page-input` | border-radius | `var(--radius-s)` | 101 |
| `.pdf-container` | min-height | `0` | 118 |
| `.pdf-container` | min-width | `0` | 119 |
| `.pdf-viewer-container` | min-width | `0` | 125 |
| `.pdf-viewer-container` | min-height | `0` | 126 |
| `.pdf-sidebar-container` | width | `var(--pdf-sidebar-width, 200px)` | 139 |
| `.pdf-sidebar-container` | max-width | `50%` | 141 |
| `.pdf-sidebar-container` | font-size | `var(--font-ui-small)` | 145 |
| `.pdf-sidebar-content` | min-width | `0` | 153 |
| `.pdf-sidebar-content::-webkit-scrollbar` | width | `6px` | 157 |
| `.pdf-sidebar-content::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 161 |
| `.pdf-sidebar-resizer` | top | `0` | 172 |
| `.pdf-sidebar-resizer` | bottom | `0` | 173 |
| `.pdf-sidebar-resizer` | right | `-3px` | 174 |
| `.pdf-sidebar-resizer` | width | `6px` | 175 |
| `.pdf-thumbnail-view` | gap | `var(--size-4-2)` | 187 |
| `.pdf-thumbnail-view` | padding | `var(--size-2-5) var(--size-2-7)` | 188 |
| `.pdf-thumbnail` | padding | `var(--size-4-1)` | 193 |
| `.pdf-thumbnail` | border-radius | `var(--radius-s)` | 194 |
| `.pdf-thumbnail-image` | width | `100%` | 205 |
| `.pdf-thumbnail-image canvas` | width | `100%` | 211 |
| `.pdf-thumbnail-image canvas` | height | `100%` | 212 |
| `.pdf-thumbnail-number` | right | `8px` | 217 |
| `.pdf-thumbnail-number` | bottom | `8px` | 218 |
| `.pdf-thumbnail-number` | min-width | `14px` | 219 |
| `.pdf-thumbnail-number` | padding | `1px var(--size-4-1)` | 220 |
| `.pdf-thumbnail-number` | border-radius | `var(--radius-xs)` | 221 |
| `.pdf-thumbnail-number` | font-size | `var(--font-ui-smallest)` | 224 |
| `.pdf-thumbnail-number` | line-height | `14px` | 225 |
| `.pdf-outline-view` | padding | `var(--size-4-2) var(--size-2-3) var(--size-4-2) var(--size-4-2)` | 231 |
| `.pdf-outline-view .tree-item-inner` | min-width | `0` | 243 |
| `.pdf-outline-view .tree-item-flair-outer` | margin-inline-start | `var(--size-4-2)` | 248 |
| `.pdf-outline-view .tree-item-flair` | font-size | `var(--font-ui-smaller)` | 251 |
| `.pdf-outline-empty` | padding | `2rem 1rem` | 256 |
| `.pdf-scroll` | min-height | `0` | 263 |
| `.pdf-scroll` | height | `100%` | 264 |
| `.pdf-scroll` | width | `100%` | 265 |
| `.pdf-scroll::-webkit-scrollbar` | width | `6px` | 274 |
| `.pdf-scroll::-webkit-scrollbar` | height | `6px` | 274 |
| `.pdf-scroll::-webkit-scrollbar-thumb` | border-radius | `var(--input-radius)` | 278 |
| `.pdf-sizer` | min-width | `min-content` | 297 |
| `.pdf-pages` | gap | `1rem` | 308 |
| `.pdf-pages` | padding | `1rem 0` | 309 |
| `.pdf-pages` | min-width | `min-content` | 310 |
| `.pdf-overlay-space` | left | `0` | 328 |
| `.pdf-overlay-space` | top | `0` | 329 |
| `.pdf-overlay-space` | width | `0` | 330 |
| `.pdf-overlay-space` | height | `0` | 331 |
| `.pdf-layer` | left | `0` | 349 |
| `.pdf-layer` | top | `0` | 350 |
| `.pdf-page canvas` | width | `100%` | 372 |
| `.pdf-page canvas` | height | `100%` | 373 |
| `.pdf-page canvas.is-stale` | inset | `0` | 380 |
| `.pdf-text-layer` | inset | `0` | 403 |
| `.pdf-text-layer` | line-height | `1` | 405 |
| `.pdf-text-layer > :not(.markedContent), .pdf-text-layer .markedContent span:not(.markedContent)` | font-size | `calc(var(--text-scale-factor) * var(--font-height))` | 442 |
| `.pdf-message` | padding | `3rem 1rem` | 458 |
| `.pdf-message` | font-size | `var(--font-ui-small)` | 461 |
| `.pdf-find-bar` | gap | `var(--size-2-1)` | 479 |
| `.pdf-find-bar` | height | `36px` | 480 |
| `.pdf-find-bar` | padding | `0 var(--size-4-2)` | 481 |
| `.pdf-find-bar` | font-size | `var(--font-ui-small)` | 486 |
| `.pdf-find-bar .clickable-icon` | width | `26px` | 496 |
| `.pdf-find-bar .clickable-icon` | height | `26px` | 497 |
| `.pdf-find-input-container` | gap | `var(--size-2-3)` | 511 |
| `.pdf-find-input-container` | min-width | `0` | 513 |
| `.pdf-find-input-container` | height | `24px` | 514 |
| `.pdf-find-input-container` | margin-right | `var(--size-2-3)` | 515 |
| `.pdf-find-input-container` | padding | `0 var(--size-4-1) 0 var(--size-2-5)` | 516 |
| `.pdf-find-input-container` | border-radius | `var(--radius-sm)` | 519 |
| `.pdf-find-icon svg` | width | `var(--icon-xs)` | 533 |
| `.pdf-find-icon svg` | height | `var(--icon-xs)` | 534 |
| `.pdf-find-input` | min-width | `0` | 539 |
| `.pdf-find-input` | height | `100%` | 540 |
| `.pdf-find-input` | padding | `0` | 541 |
| `.pdf-find-input` | font-size | `var(--font-ui-medium)` | 547 |
| `.pdf-find-count` | font-size | `var(--font-ui-small)` | 553 |
| `.pdf-find-bar .clickable-icon.pdf-find-case` | width | `20px` | 562 |
| `.pdf-find-bar .clickable-icon.pdf-find-case` | height | `20px` | 563 |
| `.pdf-find-highlight` | border-radius | `var(--radius-xxs)` | 569 |
| `.pdf-annotation-layer` | inset | `0` | 592 |
| `.pdf-annotation-layer .linkAnnotation > a` | font-size | `1em` | 613 |
| `.pdf-annotation-layer .linkAnnotation > a` | top | `0` | 614 |
| `.pdf-annotation-layer .linkAnnotation > a` | left | `0` | 615 |
| `.pdf-annotation-layer .linkAnnotation > a` | width | `100%` | 616 |
| `.pdf-annotation-layer .linkAnnotation > a` | height | `100%` | 617 |
| `.pdf-annotation-layer .linkAnnotation > a` | border-radius | `var(--radius-xxs)` | 618 |

### `ui/views/treeItem.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `.tree-item-self` | padding-top | `var(--size-4-1)` | 38 |
| `.tree-item-self` | padding-bottom | `var(--size-4-1)` | 39 |
| `.tree-item-self` | padding-inline-end | `var(--size-2-3)` | 40 |
| `.tree-item-self` | border-radius | `var(--radius-s)` | 41 |
| `.tree-item-children` | margin-inline-start | `var(--nav-item-children-margin-start)` | 71 |
| `.tree-item-children` | padding-inline-start | `var(--nav-item-children-padding-start)` | 73 |
| `.collapse-icon` | width | `var(--pm-tree-self-padding-start)` | 91 |
| `.collapse-icon` | margin-inline-start | `calc(-1 * var(--pm-tree-self-padding-start) + var(--nav-indentation-guide-width) / 2)` | 96 |
| `.collapse-icon` | margin-inline-end | `calc(-1 * var(--nav-indentation-guide-width) / 2)` | 97 |
| `.collapse-icon svg.right-triangle` | width | `10px` | 102 |
| `.collapse-icon svg.right-triangle` | height | `10px` | 103 |

### `ui/workspace.css`

| Sélecteur | Propriété | Valeur | Ligne |
|---|---|---|---|
| `#root` | width | `100%` | 14 |
| `#root` | height | `100%` | 18 |
| `.workspace` | min-height | `0` | 28 |
| `.workspace` | width | `100%` | 31 |
| `.workspace-sidedock` | width | `var(--sidebar-width, 260px)` | 52 |
| `.workspace-sidedock.is-collapsed` | width | `0` | 76 |
| `.workspace-sidedock:not(:has(.workspace-tabs))` | width | `0` | 87 |
| `.workspace-sidebar-resize-handle` | width | `0` | 125 |
| `.workspace-sidebar-resize-handle::before` | top | `0` | 133 |
| `.workspace-sidebar-resize-handle::before` | bottom | `0` | 133 |
| `.workspace-sidebar-resize-handle::before` | left | `-3px` | 133 |
| `.workspace-sidebar-resize-handle::before` | right | `-3px` | 133 |
| `.workspace-sidebar-resize-handle::after` | top | `0` | 138 |
| `.workspace-sidebar-resize-handle::after` | bottom | `0` | 138 |
| `.workspace-sidebar-resize-handle::after` | left | `50%` | 138 |
| `.workspace-sidebar-resize-handle::after` | width | `3px` | 139 |
| `.workspace-root` | min-width | `0` | 169 |
| `.workspace-split` | min-width | `0` | 174 |
| `.workspace-split` | min-height | `0` | 174 |
| `.mod-vertical   > .workspace-resize-handle` | width | `1px` | 191 |
| `.mod-horizontal > .workspace-resize-handle` | height | `1px` | 192 |
| `.mod-vertical   > .workspace-resize-handle::before` | top | `0` | 196 |
| `.mod-vertical   > .workspace-resize-handle::before` | bottom | `0` | 196 |
| `.mod-vertical   > .workspace-resize-handle::before` | left | `-3px` | 196 |
| `.mod-vertical   > .workspace-resize-handle::before` | right | `-3px` | 196 |
| `.mod-horizontal > .workspace-resize-handle::before` | left | `0` | 197 |
| `.mod-horizontal > .workspace-resize-handle::before` | right | `0` | 197 |
| `.mod-horizontal > .workspace-resize-handle::before` | top | `-3px` | 197 |
| `.mod-horizontal > .workspace-resize-handle::before` | bottom | `-3px` | 197 |
| `.mod-vertical   > .workspace-resize-handle::after` | top | `0` | 207 |
| `.mod-vertical   > .workspace-resize-handle::after` | bottom | `0` | 207 |
| `.mod-vertical   > .workspace-resize-handle::after` | left | `50%` | 207 |
| `.mod-vertical   > .workspace-resize-handle::after` | width | `1px` | 207 |
| `.mod-horizontal > .workspace-resize-handle::after` | left | `0` | 210 |
| `.mod-horizontal > .workspace-resize-handle::after` | right | `0` | 210 |
| `.mod-horizontal > .workspace-resize-handle::after` | top | `50%` | 210 |
| `.mod-horizontal > .workspace-resize-handle::after` | height | `1px` | 210 |
| `.mod-vertical   > .workspace-resize-handle:hover::after, .mod-vertical   > .workspace-resize-handle:active::after` | width | `3px` | 214 |
| `.mod-horizontal > .workspace-resize-handle:hover::after, .mod-horizontal > .workspace-resize-handle:active::after` | height | `3px` | 218 |
| `.workspace-tabs` | min-width | `0` | 224 |
| `.workspace-tabs` | min-height | `0` | 232 |
| `.workspace-tab-header-container` | height | `var(--header-height)` | 268 |
| `.workspace-tab-header-container` | gap | `0` | 272 |
| `.workspace-tab-header-container` | padding | `var(--size-4-1) 0 0` | 274 |
| `.workspace-tab-header-container-inner` | gap | `0` | 294 |
| `.workspace-tab-header-container-inner` | padding | `0 var(--size-4-1)` | 295 |
| `.workspace-tab-header-container-inner` | min-width | `0` | 297 |
| `.workspace-tab-header-new-tab` | padding | `0 var(--size-2-1) 0 0` | 304 |
| `.workspace-tab-header-new-tab` | margin-left | `-1px` | 312 |
| `.workspace-tab-header-tab-list` | padding | `0 var(--size-4-1) 0 var(--size-2-1)` | 335 |
| `.workspace-tabs.mod-top-right-space .workspace-tab-header-container` | padding-right | `var(--frame-right-inset, var(--frame-right-space))` | 347 |
| `.workspace-tab-header-right-toggle` | padding | `0 var(--size-4-1) 0 var(--size-2-1)` | 361 |
| `.clickable-icon` | padding | `var(--size-4-1)` | 372 |
| `.clickable-icon` | border-radius | `var(--radius-s)` | 373 |
| `.clickable-icon svg` | width | `var(--icon-s)` | 384 |
| `.clickable-icon svg` | height | `var(--icon-s)` | 385 |
| `.workspace-tab-header` | padding | `3px` | 402 |
| `.workspace-tab-header` | font-size | `var(--font-ui-small)` | 403 |
| `.workspace-tab-header` | border-radius | `var(--tab-radius-top) var(--tab-radius-top) 0 0` | 404 |
| `.workspace-tab-header` | width | `var(--tab-max-width)` | 423 |
| `.workspace-tab-header` | min-width | `0` | 424 |
| `.workspace-tab-header-inner` | gap | `var(--size-4-1)` | 433 |
| `.workspace-tab-header-inner` | padding | `3px 3px 3px 9px` | 436 |
| `.workspace-tab-header-inner` | border-radius | `var(--radius-s)` | 437 |
| `.workspace-tab-header-inner` | min-width | `0` | 440 |
| `.workspace-tab-header-inner-title` | min-width | `0` | 454 |
| `.workspace-tab-header-inner-close-button` | padding | `var(--size-2-1)` | 474 |
| `.workspace-tab-header-inner-close-button` | border-radius | `var(--radius-s)` | 475 |
| `.workspace-tab-header-inner-close-button svg` | width | `var(--icon-xs)` | 496 |
| `.workspace-tab-header-inner-close-button svg` | height | `var(--icon-xs)` | 497 |
| `.workspace-tab-header:not(.is-active)::after` | top | `8px` | 517 |
| `.workspace-tab-header:not(.is-active)::after` | bottom | `8px` | 518 |
| `.workspace-tab-header:not(.is-active)::after` | right | `0` | 519 |
| `.workspace-tab-header:not(.is-active)::after` | width | `1px` | 520 |
| `.workspace-tab-header.is-active::before, .workspace-tab-header.is-active::after` | bottom | `0` | 559 |
| `.workspace-tab-header.is-active::before, .workspace-tab-header.is-active::after` | width | `var(--tab-radius-bottom)` | 560 |
| `.workspace-tab-header.is-active::before, .workspace-tab-header.is-active::after` | height | `var(--tab-radius-bottom)` | 561 |
| `.workspace-tab-header.is-active::before` | left | `calc(-1 * var(--tab-radius-bottom))` | 565 |
| `.workspace-tab-header.is-active::before` | border-bottom-right-radius | `100%` | 566 |
| `.workspace-tab-header.is-active::after` | right | `calc(-1 * var(--tab-radius-bottom))` | 571 |
| `.workspace-tab-header.is-active::after` | border-bottom-left-radius | `100%` | 572 |
| `.workspace-tab-outline` | inset | `0` | 582 |
| `.workspace-tab-outline` | width | `100%` | 583 |
| `.workspace-tab-outline` | height | `100%` | 584 |
| `.workspace-tab-header-container::after` | top | `8px` | 605 |
| `.workspace-tab-header-container::after` | bottom | `8px` | 606 |
| `.workspace-tab-header-container::after` | left | `var(--drop-x, 0)` | 607 |
| `.workspace-tab-header-container::after` | width | `3px` | 610 |
| `.workspace-tab-header-container::after` | border-radius | `var(--radius-xxs)` | 611 |
| `.drag-ghost` | top | `0` | 630 |
| `.drag-ghost` | left | `0` | 631 |
| `.drag-ghost` | padding | `3px var(--size-2-5)` | 634 |
| `.drag-ghost` | border-radius | `var(--radius-sm)` | 635 |
| `.drag-ghost` | font-size | `var(--font-ui-small)` | 638 |
| `.workspace-tab-container` | min-height | `0` | 645 |
| `.workspace-leaf` | inset | `0` | 652 |
| `.workspace-tabs.is-split-preview-right  .workspace-leaf.is-active` | inset | `0 50% 0 0` | 671 |
| `.workspace-tabs.is-split-preview-left   .workspace-leaf.is-active` | inset | `0 0 0 50%` | 672 |
| `.workspace-tabs.is-split-preview-top    .workspace-leaf.is-active` | inset | `50% 0 0 0` | 673 |
| `.workspace-tabs.is-split-preview-bottom .workspace-leaf.is-active` | inset | `0 0 50% 0` | 674 |
| `.workspace-tab-container::after` | inset | `0` | 680 |
| `.workspace-tab-container::after` | border-radius | `var(--radius-m)` | 684 |
| `.workspace-tabs.is-split-preview-right  .workspace-tab-container::after` | inset | `0 0 0 50%` | 689 |
| `.workspace-tabs.is-split-preview-left   .workspace-tab-container::after` | inset | `0 50% 0 0` | 690 |
| `.workspace-tabs.is-split-preview-top    .workspace-tab-container::after` | inset | `0 0 50% 0` | 691 |
| `.workspace-tabs.is-split-preview-bottom .workspace-tab-container::after` | inset | `50% 0 0 0` | 692 |
| `.view` | min-width | `0` | 697 |
| `.view-header` | grid-template-columns | `1fr minmax(0, auto) 1fr` | 709 |
| `.view-header` | padding | `var(--size-2-1) var(--size-2-3)` | 711 |
| `.view-header-nav-buttons` | gap | `var(--size-2-1)` | 716 |
| `.view-header-title-container` | min-width | `0` | 724 |
| `.view-header-title-container` | margin-top | `1px` | 727 |
| `.view-header-title-container` | font-size | `var(--font-ui-small)` | 733 |
| `.view-header-title-parent` | min-width | `0` | 741 |
| `.view-header-title-parent` | max-width | `100%` | 743 |
| `.view-header-breadcrumb, .view-header-title` | min-width | `0` | 754 |
| `.view-header-breadcrumb` | max-width | `12em` | 758 |
| `.view-header-breadcrumb` | padding | `1px var(--size-4-1)` | 759 |
| `.view-header-breadcrumb` | border-radius | `var(--radius-s)` | 760 |
| `.view-header-breadcrumb-separator` | padding | `0 var(--size-2-1)` | 770 |
| `.view-header-title` | padding | `1px var(--size-4-1)` | 774 |
| `.view-header-title` | border-radius | `var(--radius-s)` | 775 |
| `.view-header-title-container.is-renaming .view-header-title-parent` | max-width | `0` | 783 |
| `.view-actions` | gap | `var(--size-2-1)` | 802 |
| `.view-action` | padding | `0` | 813 |
| `.view-action` | width | `26px` | 814 |
| `.view-action` | height | `26px` | 815 |
| `.view-action` | border-radius | `var(--radius-s)` | 816 |
| `.view-action svg` | margin | `auto` | 836 |
| `.view-action svg` | width | `var(--icon-m)` | 837 |
| `.view-action svg` | height | `var(--icon-m)` | 838 |
| `.view-content` | min-height | `0` | 843 |
| `.workspace-ribbon` | width | `var(--header-height)` | 856 |
| `.workspace-ribbon` | gap | `var(--size-4-2)` | 862 |
| `.workspace-ribbon` | padding | `0 0 var(--size-4-2)` | 863 |
| `.workspace-ribbon .sidebar-toggle-button` | height | `var(--header-height)` | 876 |
| `.workspace-ribbon .sidebar-toggle-button` | width | `100%` | 878 |
| `.workspace-ribbon.mod-left::after` | top | `var(--header-height)` | 892 |
| `.workspace-ribbon.mod-left::after` | right | `0` | 893 |
| `.workspace-ribbon.mod-left::after` | bottom | `0` | 894 |
| `.workspace-ribbon.mod-left::after` | width | `1px` | 895 |
| `.workspace-ribbon .clickable-icon svg` | width | `var(--icon-m)` | 905 |
| `.workspace-ribbon .clickable-icon svg` | height | `var(--icon-m)` | 906 |
| `.side-dock-actions` | gap | `var(--size-4-2)` | 916 |
| `.workspace-sidedock .workspace-tab-header-container` | padding | `0` | 934 |
| `.workspace-sidedock .workspace-tab-header-container-inner` | padding | `0 var(--size-2-1)` | 942 |
| `.workspace-sidedock .workspace-tab-header-inner-icon svg` | width | `var(--icon-m)` | 952 |
| `.workspace-sidedock .workspace-tab-header-inner-icon svg` | height | `var(--icon-m)` | 953 |
| `.workspace-sidedock .workspace-tab-header` | width | `auto` | 977 |
| `.workspace-sidedock .workspace-tab-header` | border-radius | `0` | 978 |
| `.workspace-sidedock .workspace-tab-header-inner` | gap | `0` | 992 |
| `.workspace-sidedock .workspace-tab-header-inner` | padding | `var(--size-4-1)` | 993 |
| `.workspace-sidedock .workspace-tab-header-inner` | border-radius | `var(--radius-s)` | 994 |
| `.status-bar` | right | `0` | 1038 |
| `.status-bar` | bottom | `0` | 1039 |
| `.status-bar` | max-width | `var(--status-bar-max-width, 400px)` | 1047 |
| `.status-bar` | height | `20px` | 1049 |
| `.status-bar` | padding | `0 var(--size-2-3)` | 1050 |
| `.status-bar` | border-top-left-radius | `var(--radius-sm)` | 1055 |
| `.status-bar` | font-size | `var(--font-ui-smaller)` | 1057 |
| `.status-bar-item` | height | `100%` | 1074 |
| `.status-bar-item` | padding | `0 var(--size-2-3)` | 1075 |
| `.status-bar-item svg` | width | `13px` | 1081 |
| `.status-bar-item svg` | height | `13px` | 1082 |

## Contrôle

- Fichiers CSS trouvés (Glob `**/*.css` sous `app\src`) : **50**.
- Fichiers CSS traités (une sous-section chacun) : **50**. Égalité : oui.
- Fichiers sans constante géométrique (« Aucune. ») : **5**.
- Lignes de la table des constantes de mise en page : **1561**.
- Lignes de la table des jetons : **143**.
- Le fichier de jetons est `ui\index.css` (et non `app\src\index.css`, qui n'existe pas).

## ★ Positions fractionnaires

**Question** : Fragment a des boîtes de 29,5 px (lignes de menu : 5 + 19,5 + 5). GPUI ou taffy arrondit-il à l'entier, et où ?

**Réponse** (sources de `gpui-pre 0.3.10`, `src/taffy.rs` et `src/util.rs`) : taffy lui-même N'ARRONDIT PAS (`disable_rounding()`). C'est GPUI qui arrondit, en deux temps :
1. **Avant la mise en page**, chaque longueur ABSOLUE écrite (`h`, `w`, `p`, `gap`, `top`, bordures…) passe par `round_to_device_pixel` : `round(longueur × dpr)`, la moitié vers zéro (29,5 devient 29, 19,5 devient 19). Les feuilles mesurées (texte) sont arrondies vers le HAUT. Les fractions (`relative(f)`) et les parts de flex ne sont PAS arrondies à cet étage.
2. **Après la mise en page**, chaque boîte est calée par ses arêtes ABSOLUES (`round_half_toward_zero(origine)` et de la même façon pour l'arête lointaine) : une seule réponse par coordonnée de fenêtre, sans dérive.

Chromium fait l'inverse du premier temps : il cumule en fractions exactes (1/64 px) et n'arrondit qu'à la peinture, arête par arête, à l'entier le plus proche, l'égalité VERS LE HAUT. Écrire `h(29.5)` donne donc 29 sur toute la pile là où Chromium alterne 30, 29, 30, 29… (arêtes 34,5 → 35, 64, 93,5 → 94…).

**Règle** : ne jamais écrire une longueur fractionnaire en comptant sur GPUI. Pour une pile de boîtes de hauteur fractionnaire, calculer les arêtes EXACTES (cumul en `f32` depuis l'origine exacte de la pile), les arrondir comme Chromium, et donner à GPUI les hauteurs ENTIÈRES qui en résultent : `echelle::calees(cx, origine, &hauteurs)`. Le cumul reste exact, seul l'affichage est arrondi. Exemple : le menu (`menu.rs`). Pour une pile dont l'origine est elle-même fractionnaire (sous-menu), transmettre l'origine exacte du parent.
- Limites : un pixel logique = un pixel écran (dpr 1, celui des scènes) ; le texte reste centré dans la boîte entière, alors que Chromium le pose à l'arête exacte puis arrondit la ligne de base : un décalage d'un pixel du texte est possible une ligne sur deux (à vérifier dans les scènes).
- Les largeurs fractionnaires (rare) se traitent de même, par le cumul horizontal.

## Largeur du texte : crénage et arrondi (fr-t1.3, #33)
★ Deux causes cumulées faisaient le texte de GPUI environ 1 % plus large que celui de Chromium :
- **Crénage absent.** GPUI passe à DirectWrite une typographie (`liga`, `clig`, `calt`) qui n'inclut pas `kern`. Les polices sont pourtant identiques des deux côtés.
- **Arrondi supérieur.** GPUI arrondit au pixel supérieur (`.ceil()`) la largeur d'une feuille de texte, alors que Chromium garde la fraction et n'arrondit que les arêtes.

Règle pour TOUT texte de `fragment-ui` :
- `polices::famille_interface(e)`, ou bien `.font_family(FAMILLE_INTERFACE).font_features(polices::caracteristiques())` ;
- quand une boîte se dimensionne sur son texte : `polices::largeur_texte` puis `largeur_ajustee`, jamais la largeur intrinsèque de GPUI.
