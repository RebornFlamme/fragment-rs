# Rastérisation des glyphes : GPUI/DirectWrite contre Chromium/Skia (fr-p4.1.3, 13/11/2026)

Question de l'orchestrateur : en T2, 174 débuts de ligne sur 186 sont à une abscisse entière chez
Chromium, et pourtant l'encre du candidat est plus étroite des deux côtés. Quel jeu de paramètres de
rastérisation reproduit la référence (mode DirectWrite, paramètres du système, gamma et contraste de
Skia, poste) ?

**Réponse : ce n'est ni le mode DirectWrite ni le gamma. C'est l'anticrénelage.** Chromium peint en
**ClearType** (masque LCD, franges R/B) le texte posé sur une surface opaque non composée. Il ne peint
en gris que dans les calques composés : modale, notices, infobulle, palette, menus. `fragment-pixels`,
lui, force le gris partout (`crates/fragment-app/src/pixels/mod.rs`, `set_text_rendering_mode(Grayscale)`,
réglage de fr-t1.1 pris sur les surfaces flottantes). Sur les mots que la référence peint en ClearType,
le candidat a **22 % d'encre en moins et 0,77 px de largeur en moins**. Sur les mots qu'elle peint en
gris, l'encre est la même à 1 % près. Le reste (−0,44 px) vient de l'origine des boîtes (fr-p4.1.1).

La sonde COM, qui devait rastériser chaque jeu de paramètres hors de l'app, n'a pas été compilée : la
permission de compiler hors dépôt a été refusée. La mesure ci-dessous n'en a pas besoin. Elle lit les
références et le candidat, et les sources de Skia, de Chromium et de GPUI.

## 1. ClearType ou gris : par scène, puis par mot

Mesure `chroma_ref.py` sur les 204 PNG de `pixels/reference/` (main, fab1e25). La chroma d'un pixel
vaut max(R,G,B) − min(R,G,B). Un pixel a la **signature LCD** quand |R − B| > 30 et qu'un de ses
voisins horizontaux a l'écart de signe opposé (frange rouge d'un côté, bleue de l'autre). Un aplat
d'accent n'a pas cette signature. Les colonnes « texte » ne portent que sur les boîtes des glyphes du
relevé (T1, T2 et T11, mêmes MD5 que main).

| scène (sombre ; le clair est à ±5 %) | chroma max | px > 30 | px LCD | texte : max | texte : px > 30 |
|---|---|---|---|---|---|
| t1-modale, t1-notice, t1-notice-empilees, t1-infobulle, t1-palette-aucun-resultat | 6 à 8 | 0 | 0 | 4 à 6 | 0 |
| t1-palette, t1-palette-selection | 155 | 558 | **0** | 155 | 558 (aplat d'accent) |
| t1-menu | 154 | 368 | **0** | 148 | 260 (accent) |
| t1-onglets, t1-onglet-survol, t1-onglet-epingle, t1-fenetre-vide, t1-dock-* | 161 | 204 | 126 | 161 | 204 |
| t1-entete-vue | 161 | 882 | 485 | 161 | 882 |
| t1-reglages-appearance, t1-reglages-zoom | 163 | 12 704 | 6 427 | 163 | 11 076 |
| t2-explorateur | 161 | 1 795 | 1 081 | 161 | 1 795 |
| t2-layout-defaut | 161 | 4 573 | 1 988 | 161 | 4 573 |
| t3-* (27 scènes) | 161 à 255 | 3 085 à 17 102 | 1 016 à 7 512 | – | – |
| t4-* (15 scènes) | 184 à 234 | 6 137 à 17 371 | 1 779 à 10 097 | – | – |

Au mot (`mots_lcd.py`, DM Sans seulement, thème sombre) :

| tranche | mots ClearType | mots gris | autres |
|---|---|---|---|
| T1 | 430 | 422 | 18 |
| T2 | 492 | 210 | 4 |
| T11 | 280 | 86 | 58 |

★ **Le mode se décide par surface, pas par scène.** Sur `t1-onglets`, seul l'onglet actif (« New Tab »
en x = 258) est en ClearType (chroma 161, 117 px > 30). Les trois onglets inactifs, la barre d'état
(« Fragment 0.0.0 ») et le nom du coffre sont gris (chroma ≤ 6). Cela tranche entre les deux mesures
qui se contredisaient. fr-t2-montage lisait 3 à 5 de chroma sur un « New Tab » : c'était un onglet
inactif. Toute la fenêtre de réglages est en ClearType (78 mots sur 80). L'explorateur, le fil
d'Ariane, le gestionnaire de coffres et l'éditeur (T3) le sont aussi. Les menus, la palette, la
modale, les notices et l'infobulle sont gris (calques composés, voir S8).

Candidat (artefact `pixels` du run 43356976195) : chroma ≤ 12 sur tout le texte, aucun pixel LCD.

## 2. Ce que coûte le gris, mot par mot

`encre.py` compare le candidat (run 43356976195) à la référence. La fenêtre est le mot élargi de
2 px, sur un fond uni (on écarte les survols et les sélections). L'encre d'un pixel vaut
max(|px − fond|) sur les canaux. La largeur d'encre est l'étendue des colonnes au-dessus de 15 % du
maximum.

| mots (T1 + T2) | n | encre cand/réf | é.-t. | largeur cand − réf | bord gauche | bord droit | plus étroits | plus larges |
|---|---|---|---|---|---|---|---|---|
| ClearType dans la référence | 589 | **0,779** | 0,042 | **−0,77 px** | +0,44 | −0,33 | 56 % | 4 % |
| gris dans la référence | 506 | **1,010** | 0,111 | −0,44 px | +0,18 | −0,25 | 39 % | 12 % |
| T2 seul, ClearType | 164 | 0,784 | 0,040 | −0,98 px | +0,63 | −0,34 | 70 % | 1 % |
| T2 seul, gris | 132 | 0,982 | 0,107 | −0,21 px | −0,05 | −0,26 | 27 % | 13 % |

- « Encre plus étroite des deux côtés » (fr-p4.1.2) : c'est la signature du ClearType. Le masque 3×1
  étale chaque fût sur les sous-pixels voisins, et le contraste de Skia (1,0, § 3) le renforce. Le gris
  de GPUI n'a ni l'un ni l'autre. L'écart d'encre est régulier (é.-t. 0,04) : c'est un mode, pas une
  position.
- Sur les mots gris, l'encre totale est juste (1,01). La correction gamma et contraste de GPUI
  (alpha_correction.hlsl, reprise de Windows Terminal) et la table de Skia donnent donc la même masse
  à 1 % près. Le reste en largeur (−0,2 à −0,4 px, surtout le bord droit) est l'origine calée de la
  boîte de texte, déjà chiffrée par fr-p4.1.1.

## 3. Les paramètres, source par source

| | Chromium/Skia (Electron de la référence) | GPUI 0.3.12 (candidat) |
|---|---|---|
| Anticrénelage | ClearType (LCD16) si `SPI_GETFONTSMOOTHINGTYPE` = ClearType ET surface opaque non composée ; sinon gris (A8) | `TextRenderingMode` global de l'App : `Grayscale` forcé par fragment-pixels |
| Mode DirectWrite | `SkScalerContext_DW` : DM Sans (gasp ou pas d'instructions) → `NATURAL_SYMMETRIC` ; grille `ENABLED` (hinting jamais `None` sous Chromium Windows : `HINTING_MEDIUM`) ; texture `ALIASED_1x1` en gris, `CLEARTYPE_3x1` en LCD | `GetRecommendedRenderingMode` (`NATURAL_SYMMETRIC`, grille `ENABLED` pour DM Sans 12 px, sonde de fr-p4.1.1) ; textures identiques |
| Gamma et contraste | Build Windows de Chromium : `SK_GAMMA_SRGB` et `SK_GAMMA_CONTRAST=1.0` (`skia/BUILD.gn`). Ils ne sont remplacés par `IDWriteRenderingParams` (gamma, contraste renforcé) **que si la clé `HKCU\SOFTWARE\Microsoft\Avalon.Graphics\DISPLAYn` existe** (`font_util_win.cc`, fonction `kUseGammaContrastRegistrySettings` active par défaut). Table de pré-mélange `SkMaskGamma` à 3 bits de luminance, appliquée au masque avant le mélange | `IDWriteFactory::CreateRenderingParams()` du système, lu une fois (`OnceLock`) : gamma → `get_gamma_correction_ratios`, `GetGrayscaleEnhancedContrast` (gris), `GetEnhancedContrast` (ClearType), atténués pour le clair sur sombre. **Non exposé** |
| Positionnement | quart de pixel horizontal (`kSubpixelRound` 1/8) | quart de pixel horizontal (fr-p4.1.1) |

Conséquence du point gamma : sur le portable, la clé `Avalon.Graphics` existe **mais elle est vide**,
sans sous-clé `DISPLAY1`. Chromium y prend donc ses valeurs de build (sRGB, contraste 1,0), pas celles
du système. Un réglage du tuner ClearType sur le poste changerait les références. Il faudrait alors
les recapturer.

## 4. Le poste

| | portable (références) | runner `windows-latest` (candidat) |
|---|---|---|
| Lissage des polices (`FontSmoothing`) | 2 (actif) | voir le journal du job (étape « Réglages de texte du runner ») |
| Type (`FontSmoothingType`) | **2 = ClearType** | idem |
| `FontSmoothingGamma` | 0 (défaut) | idem |
| Orientation | 1 = RGB | idem |
| `HKCU\…\Avalon.Graphics` | présente, vide (pas de `DISPLAY1`) | idem |
| Écran | 1920 × 1080 physique, 150 % (un processus non sensible au DPI voit 1280 × 720), `DpiValue` 0 = recommandé | 1920 × 1080 posé par `Set-DisplayResolution`, 96 dpi |
| Rendu | Electron 43, Chromium GPU (AMD Radeon) ; captures en DPR 1 | GPUI DirectX 11 sur WARP |

- DPI et mise à l'échelle **ne jouent pas** sur la rastérisation des glyphes des références : Fragment
  y est capturé en DPR 1 (PROVENANCE.txt, et le zoom de 150 % ne passe pas dans la page).
- Le ClearType du système **joue** : c'est lui qui fait choisir le LCD à Chromium. Une référence
  capturée sur un poste en lissage standard, ou avec `--disable-lcd-text` (#102 pour T2), serait
  grise partout.
- GPUI ne lit le système qu'en mode `PlatformDefault`. `fragment-pixels` impose le mode : le runner
  ne change donc rien au candidat, à ses `CreateRenderingParams` près (gamma et contraste).

## 5. Le paramètre exposé : `set_text_rendering_mode`

GPUI expose `App::set_text_rendering_mode(TextRenderingMode)`. Sur cette branche, `fragment-pixels` lit
`FRAGMENT_PIXELS_TEXTE=subpixel`, qui donne le ClearType de GPUI. La valeur par défaut reste le gris.
Le job `pixels` publie une seconde capture, `pixels/candidate-subpixel/`, sans en faire une porte.
Résultats de la CI : § 6.

Limites, d'après les sources :
- Le mode est un `Rc<Cell<_>>` **global à l'App**, lu par `Window::paint_glyph` au moment de peindre,
  et le ClearType exige une fenêtre `WindowBackgroundAppearance::Opaque`. Tout passer en ClearType
  serait aussi faux que tout passer en gris : 422 mots de T1 sur 870 sont gris chez Chromium.
- Pour reproduire Chromium, il faudrait choisir le mode **par surface**, comme lui. C'est possible
  sans fork : un élément de `fragment-ui` qui passe le mode à `Subpixel` avant de peindre ses
  enfants et le remet ensuite (on a `&mut App` dans `Element::paint`), posé sur les surfaces opaques
  non composées (onglet actif, explorateur, réglages, éditeur). Coût estimé : un élément et sa pose
  sur 6 à 8 vues, plus la vérification que l'atlas sépare bien les sprites gris des sprites
  sous-pixel (`subpixel_rendering` fait partie de `RenderGlyphParams`, donc de la clé). Ce n'est pas
  fait ici : le choix revient à l'orchestrateur.
- Gamma et contraste ne sont pas exposés (lus du système dans un `OnceLock`). Pour suivre Skia au
  niveau près (sRGB et contraste 1,0 au lieu des valeurs DirectWrite), il faudrait patcher
  `gpui-pre-windows` (`directx_renderer.rs::get_font_info`, `alpha_correction.hlsl`), donc un fork.
  Le § 2 montre que ce n'est pas la cause : en gris, l'encre est juste à 1 %.

## 6. Mesure CI des deux modes

À compléter avec le run de la PR : chroma, encre et largeur de `candidate-subpixel` contre les
références ClearType.

## Reproduire

Scripts hors dépôt, dans `C:/dev/raster-sonde/py/` (Python lancé par `uv`) :

```
cd <worktree>
uv run --no-project --with pillow --with numpy python C:/dev/raster-sonde/py/chroma_ref.py pixels/reference C:/dev/fragment-rs-mesure-glyphes/pixels/reference
uv run --no-project --with pillow --with numpy python C:/dev/raster-sonde/py/mots_lcd.py   pixels/reference C:/dev/fragment-rs-mesure-glyphes/pixels/reference
gh run download <run> -n pixels -D <art>     # candidat gris et candidate-subpixel
uv run --no-project --with pillow --with numpy python C:/dev/raster-sonde/py/encre.py pixels/reference <art>/pixels/candidate C:/dev/fragment-rs-mesure-glyphes/pixels/reference t1- t2-
uv run --no-project --with pillow --with numpy python C:/dev/raster-sonde/py/encre.py pixels/reference <art>/pixels/candidate-subpixel C:/dev/fragment-rs-mesure-glyphes/pixels/reference t1- t2-
```

Les relevés de glyphes (`*.glyphes.json`) sont ceux de fr-p4.1.1 (`docs/mesures/placement-glyphes.md`).
Sources lues : Skia `src/ports/SkScalerContext_win_dw.cpp`, `SkTypeface_win_dw.cpp`, `src/core/SkMaskGamma.*`,
`SkScalerContext.*` ; Chromium `skia/BUILD.gn`, `skia/ext/legacy_display_globals.cc`,
`ui/gfx/font_render_params_win.cc`, `ui/gfx/font_util_win.cc`, `ui/base/ui_base_features.cc` (branche
main, novembre 2026) ; `gpui-pre-windows-0.3.12/src/direct_write.rs`, `directx_renderer.rs`,
`alpha_correction.hlsl`, `gpui-pre-0.3.12/src/window.rs` (`should_use_subpixel_rendering`).
