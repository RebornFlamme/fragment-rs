// Port de app/src/ui/core/editor/viewport.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LE VIEWPORT DE CODEMIRROR 6, PORTÉ — pour ce que les CALQUES posent sur une vue.
//
//  Une vue virtualise SON contenu (la note ses lignes, la vue PDF ses pages) ; ce
//  que le cœur pose par-dessus — les widgets des calques, les objets du journal —
//  est virtualisé ICI, de la même façon pour toute vue : n'existe à l'écran que
//  ce qui est dans le viewport.
//
//  Transcription de `@codemirror/view` 6.43.13 (dist/index.js) :
//    - constantes `VP` (Margin, MinCoverMargin, MaxCoverMargin)
//    - le biais de `ViewState.measure` (l. ~6432)
//    - `ViewState.getViewport` (l. 6470), sans `scrollTarget`
//    - `ViewState.viewportIsAppropriate` (l. 6501)
//    - la boucle de `EditorView.measure` (l. 8171) : on remesure tant que ça bouge
//  ★ Un seul écart, voulu : CM traduit sa bande de pixels en LIGNES (sa height
//    map) ; nos objets ont déjà une position en pixels (`coords_at_pos`), donc on
//    garde la BANDE telle quelle — même formule, sans l'arrondi aux lignes (et
//    sans les cas `from == 0` / `to == doc.length`, qui n'existent que parce que
//    l'arrondi bute aux bords du document).
//
//  @codemirror/view — MIT License, Copyright (C) 2018-2021 by Marijn Haverbeke
//  <marijn@haverbeke.berlin> and others.
// ═══════════════════════════════════════════════════════════════════════════

/// Les constantes de CM6 (`const enum VP`), en px.
pub struct Vp;

impl Vp {
    /// Ce qui existe au-delà du visible, réparti haut / bas selon le biais.
    pub const MARGIN: f64 = 1000.0;
    /// Le visible doit rester au moins à cette distance d'un bord de la bande…
    pub const MIN_COVER_MARGIN: f64 = 10.0;
    /// …et le biais peut porter cette exigence jusque-là.
    pub const MAX_COVER_MARGIN: f64 = 250.0;
}

/// Une bande verticale, en px (CM : `pixelViewport` / le viewport converti en hauteurs).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Bande {
    pub top: f64,
    pub bottom: f64,
}

/// Le biais : le sens (et l'ampleur) du défilement depuis la dernière mesure (CM : `measure`).
pub fn biais(d_top: f64, d_bottom: f64) -> f64 {
    if d_top > 0.0 && d_bottom > 0.0 {
        return d_top.max(d_bottom);
    }
    if d_top < 0.0 && d_bottom < 0.0 {
        return d_top.min(d_bottom);
    }
    0.0
}

/// CM : `getViewport(bias, null)` — le visible ± Margin, la marge penchée vers où l'on va.
pub fn bande(visible: Bande, bias: f64) -> Bande {
    let margin_top = 0.5 - (bias / Vp::MARGIN / 2.0).clamp(-0.5, 0.5);
    Bande {
        top: visible.top - margin_top * Vp::MARGIN,
        bottom: visible.bottom + (1.0 - margin_top) * Vp::MARGIN,
    }
}

/// CM : `viewportIsAppropriate` — la bande couvre-t-elle le visible, sans aller trop au-delà ?
pub fn bande_appropriee(b: Bande, visible: Bande, bias: f64) -> bool {
    b.top <= visible.top - Vp::MIN_COVER_MARGIN.max((-bias).min(Vp::MAX_COVER_MARGIN))
        && b.bottom >= visible.bottom + Vp::MIN_COVER_MARGIN.max(bias.min(Vp::MAX_COVER_MARGIN))
        && b.top > visible.top - 2.0 * Vp::MARGIN
        && b.bottom < visible.bottom + 2.0 * Vp::MARGIN
}

/// Le viewport d'une vue (CM : la part viewport de `ViewState`). On lui donne le
/// VISIBLE à chaque occasion où CM mesure (défilement, redimensionnement) ; il dit
/// si la bande a CHANGÉ — le seul cas où l'on prévient les calques.
#[derive(Debug)]
pub struct ViewportBand {
    /// La bande courante (`None` : jamais mesurée, ou hors vue).
    pub courante: Option<Bande>,
    visible: Bande,
    /// CM : `mustMeasureContent` — vrai après un changement : la mesure suivante calcule le biais.
    must_measure_content: bool,
}

impl Default for ViewportBand {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportBand {
    pub fn new() -> Self {
        Self {
            courante: None,
            visible: Bande::default(),
            must_measure_content: true,
        }
    }

    /// Le contenu a bougé (zoom, réagencement) : la prochaine mesure relit le biais (CM : l. 3013).
    pub fn content_changed(&mut self) {
        self.must_measure_content = true;
    }

    /// Une mesure : `visible` = le visible (`None` : hors vue). Rend vrai si la bande a changé.
    pub fn measure(&mut self, visible: Option<Bande>) -> bool {
        let mut change = false;
        // La boucle de `EditorView.measure` : tant que ça bouge, au plus 5 fois.
        let mut i = 0;
        while i <= 5 && self.mesure_unique(visible) {
            change = true;
            i += 1;
        }
        change
    }

    fn mesure_unique(&mut self, visible: Option<Bande>) -> bool {
        // CM : `inView` faux → le viewport est réputé approprié, rien ne change.
        let Some(visible) = visible else {
            return false;
        };
        if visible.bottom <= visible.top {
            return false;
        }
        let d_top = visible.top - self.visible.top;
        let d_bottom = visible.bottom - self.visible.bottom;
        self.visible = visible;
        // CM ne calcule le biais que quand il MESURE le contenu (l. 6432).
        let bias = if self.must_measure_content {
            biais(d_top, d_bottom)
        } else {
            0.0
        };
        self.must_measure_content = false;
        if let Some(courante) = self.courante
            && bande_appropriee(courante, visible, bias)
        {
            return false;
        }
        self.courante = Some(bande(visible, bias));
        // Le contenu de la nouvelle bande va être dessiné : la mesure suivante en tient compte.
        self.must_measure_content = true;
        true
    }
}
