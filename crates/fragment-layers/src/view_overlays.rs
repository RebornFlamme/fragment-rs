// Port de app/src/ui/core/layers/ViewOverlays.ts (la GÉOMÉTRIE seule : ni DOM, ni observateur)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Ce que l'hôte des calques d'une vue CALCULE, séparé de ce qu'il MONTE.
//
//  L'hôte (fragment-ui, `calques/`) possède trois racines : le plan VIEWPORT
//  (couvre le pane, ne défile pas), le plan DOCUMENT (son origine est celle de
//  `coords_at_pos`, il suit le texte) et, posée dans ce dernier, la racine de
//  MARGE, dont le zoom est annulé. Tout le calcul qui en découle vit ici, pur et
//  testable sans fenêtre :
//    - les conversions client ↔ plan, l'une exactement l'inverse de l'autre ;
//    - la bande de marge d'un côté, tirée des rectangles du pane et de la colonne
//      (écart colonne → widget propre à la vue, souffle fixe côté pane) ;
//    - la clé de géométrie, qui ne change que si les marges peuvent changer ;
//    - le viewport des calques (`fragment_text::viewport`), annoncé seulement
//      quand sa bande change ;
//    - l'échelle que la racine de marge annule.
//
//  ★ Pourquoi l'échelle est relue à chaque mesure et jamais retenue d'une passe
//    à l'autre : le zoom d'une vue (PDF) change sans prévenir ; une copie serait
//    fausse dès le premier pincement. Même règle pour les rectangles CLIENT : on
//    ne garde que ceux de la mesure COURANTE, oubliés à la suivante.
//
//  ★ Transposition GPUI : le DOM met le plan document à l'échelle par un
//    `transform` et la marge annule ce transform. GPUI n'a pas de transform de
//    sous-arbre : l'hôte place donc LUI-MÊME chaque élément dans le pane avec
//    `position_dans_le_pane` — unités du plan × échelle pour le plan document,
//    px écran tels quels pour la marge. Le résultat est le même repère.
// ═══════════════════════════════════════════════════════════════════════════

use fragment_text::Rect;
use fragment_text::viewport::{Bande, ViewportBand as BandeDeVue};

use crate::gutter_stack::gutter_gap;
use crate::types::{GutterBand, GutterSide, OverlayPlane, Point, ViewportBand};

/// Le souffle FIXE entre un widget de marge et le bord du pane (px écran) : de
/// quoi laisser passer la barre de défilement (6 px) sans la recouvrir. Côté
/// colonne, l'écart est proportionnel à la marge (`gutter_gap`, ou celui de la vue).
pub const GUTTER_PAD: f64 = 14.0;

/// Le rang d'empilement du plan document : au-dessus des calques de la surface
/// (surlignages, curseurs distants), un widget passe toujours devant.
pub const DOCUMENT_PLANE_Z: u32 = 200;

/// L'échelle d'un plan, ramenée à 1 si elle est nulle, négative ou non finie :
/// on divise par elle, une valeur absurde ferait tout exploser.
pub fn echelle_bornee(s: f64) -> f64 {
    if s.is_finite() && s > 0.0 { s } else { 1.0 }
}

/// L'origine CLIENT du repère `coords_at_pos` : le coin du parent du plan, moins
/// ce qu'il a lui-même défilé (nul quand c'est un ancêtre qui défile, mais le
/// compter rend le calcul juste dans les deux cas).
pub fn origine_du_plan(parent: Rect, defile_x: f64, defile_y: f64) -> Point {
    Point {
        x: parent.left - defile_x,
        y: parent.top - defile_y,
    }
}

/// Un point CLIENT → le plan viewport (coin haut-gauche du pane), ou `None` s'il
/// tombe hors du pane (bords compris dedans). Sert aussi de hit-test.
pub fn client_vers_viewport(pane: Rect, x: f64, y: f64) -> Option<Point> {
    if x < pane.left || x > pane.right || y < pane.top || y > pane.bottom {
        return None;
    }
    Some(Point {
        x: x - pane.left,
        y: y - pane.top,
    })
}

/// Un point CLIENT (px écran) → le plan document (unités NON zoomées de la vue).
/// La division par l'échelle est ce qui fait atterrir juste un objet ancré sur une
/// vue zoomée ; à l'échelle 1, c'est une simple translation.
pub fn client_vers_document(origine: Point, echelle: f64, x: f64, y: f64) -> Point {
    let e = echelle_bornee(echelle);
    Point {
        x: (x - origine.x) / e,
        y: (y - origine.y) / e,
    }
}

/// L'inverse EXACT de `client_vers_document` : la promesse que le plan est
/// inversible (poser une barre sur une forme, rogner une capture, sonder).
pub fn document_vers_client(origine: Point, echelle: f64, x: f64, y: f64) -> Point {
    let e = echelle_bornee(echelle);
    Point {
        x: origine.x + x * e,
        y: origine.y + y * e,
    }
}

/// La largeur brute de la marge d'un côté (entre la colonne et le souffle du pane),
/// celle dont on tire l'écart colonne → widget.
pub fn marge_brute(side: GutterSide, pane: Rect, colonne: Rect) -> f64 {
    match side {
        GutterSide::Left => colonne.left - pane.left - GUTTER_PAD,
        GutterSide::Right => pane.right - GUTTER_PAD - colonne.right,
    }
}

/// La bande de MARGE d'un côté, dans le repère de la racine de marge : px ÉCRAN
/// mesurés depuis l'origine du plan (le zoom y est annulé). Elle ne dépend que du
/// pane et de la colonne, jamais du document ; `width` ≤ 0 quand la colonne
/// remplit le pane (l'appelant masque).
pub fn bande_de_marge(
    side: GutterSide,
    pane: Rect,
    colonne: Rect,
    origine: Point,
    ecart: &dyn Fn(f64) -> f64,
) -> GutterBand {
    let gap = ecart(marge_brute(side, pane, colonne));
    let (de, a) = match side {
        GutterSide::Left => (pane.left + GUTTER_PAD, colonne.left - gap),
        GutterSide::Right => (colonne.right + gap, pane.right - GUTTER_PAD),
    };
    GutterBand {
        left: de - origine.x,
        width: a - de,
    }
}

/// La CLÉ de géométrie des marges : largeur du pane, largeur de la colonne, et ses
/// deux écarts au pane, arrondis au pixel. La position de la colonne compte (une
/// réserve de marge la déplace sans la redimensionner) ; sa HAUTEUR non (elle
/// change à chaque frappe, pour des marges inchangées).
pub fn cle_de_geometrie(pane: Rect, colonne: Option<Rect>) -> [i64; 4] {
    let largeur = |r: Rect| r.right - r.left;
    let (col, gauche, droite) = match colonne {
        Some(c) => (largeur(c), c.left - pane.left, pane.right - c.right),
        None => (0.0, 0.0, 0.0),
    };
    [largeur(pane), col, gauche, droite].map(|v| v.round() as i64)
}

/// Le visible du pane en px écran relatifs à l'origine du plan, ou `None` si le
/// pane n'a pas de hauteur.
pub fn visible_du_pane(pane: Rect, origine: Point) -> Option<Bande> {
    if pane.bottom - pane.top <= 0.0 {
        return None;
    }
    Some(Bande {
        top: pane.top - origine.y,
        bottom: pane.bottom - origine.y,
    })
}

/// La bande du viewport (px écran) dans les unités du plan document.
pub fn bande_vers_plan(b: Bande, echelle: f64) -> ViewportBand {
    let e = echelle_bornee(echelle);
    ViewportBand {
        top: b.top / e,
        bottom: b.bottom / e,
    }
}

/// Un niveau voulu de marge : il arrive en unités du plan (`coords_at_pos`) et
/// l'arbitre parle en px de la marge (px écran).
pub fn niveau_vers_marge(top: Option<f64>, echelle: f64) -> Option<f64> {
    top.map(|t| t * echelle_bornee(echelle))
}

/// Où l'hôte doit poser un élément.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Racine {
    /// Le plan viewport : coin du pane.
    Viewport,
    /// Le plan document : unités de la vue, mises à l'échelle.
    Document,
    /// La racine de marge : dans le plan document, px écran (zoom annulé).
    Marge,
}

impl From<OverlayPlane> for Racine {
    fn from(p: OverlayPlane) -> Self {
        match p {
            OverlayPlane::Viewport => Racine::Viewport,
            OverlayPlane::Document => Racine::Document,
        }
    }
}

/// Le coin haut-gauche d'un élément, relatif au coin du PANE (px écran), depuis
/// son `left`/`top` écrits dans le repère de sa racine. `origine` est l'origine
/// CLIENT du plan document (`None` : pas de plan document, seule la racine
/// viewport a un sens).
pub fn position_dans_le_pane(
    racine: Racine,
    pane: Rect,
    origine: Option<Point>,
    echelle: f64,
    left: f64,
    top: f64,
) -> Option<Point> {
    match racine {
        Racine::Viewport => Some(Point { x: left, y: top }),
        Racine::Document => {
            let o = origine?;
            let c = document_vers_client(o, echelle, left, top);
            Some(Point {
                x: c.x - pane.left,
                y: c.y - pane.top,
            })
        }
        Racine::Marge => {
            let o = origine?;
            Some(Point {
                x: o.x + left - pane.left,
                y: o.y + top - pane.top,
            })
        }
    }
}

/// Le coin haut-gauche d'un élément du plan VIEWPORT depuis ses quatre côtés
/// écrits (ancre par coin) : `left` sinon `right`, `top` sinon `bottom`, rapportés
/// au pane que le plan couvre. `taille` est la boîte mesurée de l'élément.
pub fn coin_viewport(
    pane: Rect,
    taille: (f64, f64),
    left: Option<f64>,
    right: Option<f64>,
    top: Option<f64>,
    bottom: Option<f64>,
) -> Point {
    let (w, h) = (pane.right - pane.left, pane.bottom - pane.top);
    let x = match (left, right) {
        (Some(l), _) => l,
        (None, Some(r)) => w - r - taille.0,
        (None, None) => 0.0,
    };
    let y = match (top, bottom) {
        (Some(t), _) => t,
        (None, Some(b)) => h - b - taille.1,
        (None, None) => 0.0,
    };
    Point { x, y }
}

/// Une mesure des plans d'une vue à un instant, en coordonnées CLIENT.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MesuresPlans {
    /// Le rectangle du pane (le plan viewport le couvre).
    pub pane: Rect,
    /// L'origine du repère `coords_at_pos` (`origine_du_plan`) ; `None` : vue sans
    /// plan document.
    pub origine: Option<Point>,
    /// La colonne de lecture déclarée par la vue (défaut : le parent du plan).
    pub colonne: Option<Rect>,
    /// L'échelle de la vue sur son plan document (bornée à l'usage).
    pub echelle: f64,
}

/// Ce qu'une nouvelle mesure a changé : à qui l'hôte doit le dire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changements {
    /// La clé de géométrie a changé : prévenir `on_geometry_change`.
    pub geometrie: bool,
    /// La bande du viewport a changé : prévenir `on_viewport_change`.
    pub viewport: bool,
    /// L'échelle annulée par la marge a changé : l'arbitre repasse.
    pub echelle_marge: bool,
}

/// L'écart colonne → widget d'une vue (défaut : `gutter_gap` ; PDF : 12 px fixes).
pub type Ecart = Box<dyn Fn(f64) -> f64>;

/// L'état géométrique d'un hôte : la dernière mesure, les bandes de marge
/// mémorisées pour elle, le viewport et son hystérésis, la clé de géométrie.
pub struct GeometrieVue {
    mesures: Option<MesuresPlans>,
    ecart: Ecart,
    /// Bandes déjà calculées pour la mesure courante (indice : gauche, droite).
    bandes: [Option<Option<GutterBand>>; 2],
    bande_vue: BandeDeVue,
    cle: Option<[i64; 4]>,
    /// Tailles (pane, colonne) de la dernière mesure : un redimensionnement fait
    /// relire le biais du viewport, comme l'observateur de taille du TS.
    tailles: Option<[i64; 4]>,
    echelle_marge: f64,
}

fn indice(side: GutterSide) -> usize {
    match side {
        GutterSide::Left => 0,
        GutterSide::Right => 1,
    }
}

impl Default for GeometrieVue {
    fn default() -> Self {
        Self::new(Box::new(gutter_gap))
    }
}

impl GeometrieVue {
    /// Un état vide, avec l'écart colonne → widget de la vue.
    pub fn new(ecart: Ecart) -> Self {
        Self {
            mesures: None,
            ecart,
            bandes: [None, None],
            bande_vue: BandeDeVue::new(),
            cle: None,
            tailles: None,
            echelle_marge: 1.0,
        }
    }

    /// La dernière mesure posée.
    pub fn mesures(&self) -> Option<MesuresPlans> {
        self.mesures
    }

    /// L'échelle courante, bornée (1 sans mesure).
    pub fn echelle(&self) -> f64 {
        self.mesures.map_or(1.0, |m| echelle_bornee(m.echelle))
    }

    /// Pose une mesure (après la mise en page, avant de placer) et dit ce qui a
    /// changé. Les bandes de marge mémorisées sont oubliées dès que la mesure
    /// diffère ; le viewport n'annonce que si SA bande change.
    pub fn poser_mesures(&mut self, m: MesuresPlans) -> Changements {
        let mut ch = Changements::default();
        if self.mesures != Some(m) {
            self.bandes = [None, None];
        }
        let premiere = self.mesures.is_none();
        self.mesures = Some(m);

        let cle = cle_de_geometrie(m.pane, m.colonne);
        if self.cle != Some(cle) {
            // La toute première clé n'est pas un CHANGEMENT : personne n'avait
            // encore vu de géométrie.
            ch.geometrie = self.cle.is_some();
            self.cle = Some(cle);
        }

        let e = echelle_bornee(m.echelle);
        if e != self.echelle_marge {
            self.echelle_marge = e;
            ch.echelle_marge = true;
        }

        let hauteur = |r: Rect| r.bottom - r.top;
        let tailles = [
            cle[0],
            hauteur(m.pane).round() as i64,
            cle[1],
            m.colonne.map_or(0, |c| hauteur(c).round() as i64),
        ];
        if self.tailles.is_some_and(|t| t != tailles) {
            self.bande_vue.content_changed();
        }
        self.tailles = Some(tailles);

        let visible = m.origine.and_then(|o| visible_du_pane(m.pane, o));
        let change = self.bande_vue.measure(visible);
        // La première mesure n'annonce pas (le TS mesure sans prévenir au premier
        // appel de `viewport()`).
        ch.viewport = change && !premiere;
        ch
    }

    /// Point CLIENT → plan viewport (`None` hors du pane, ou sans mesure).
    pub fn client_to_viewport(&self, x: f64, y: f64) -> Option<Point> {
        client_vers_viewport(self.mesures?.pane, x, y)
    }

    /// Point CLIENT → plan document (`None` sans plan document).
    pub fn client_to_document(&self, x: f64, y: f64) -> Option<Point> {
        let m = self.mesures?;
        Some(client_vers_document(m.origine?, m.echelle, x, y))
    }

    /// Point du plan document → CLIENT (`None` sans plan document).
    pub fn document_to_client(&self, x: f64, y: f64) -> Option<Point> {
        let m = self.mesures?;
        Some(document_vers_client(m.origine?, m.echelle, x, y))
    }

    /// La bande de marge d'un côté (mémorisée pour la mesure courante).
    pub fn gutter_band(&mut self, side: GutterSide) -> Option<GutterBand> {
        let i = indice(side);
        if let Some(b) = self.bandes[i] {
            return b;
        }
        let b = self.mesures.and_then(|m| {
            Some(bande_de_marge(
                side,
                m.pane,
                m.colonne?,
                m.origine?,
                &*self.ecart,
            ))
        });
        self.bandes[i] = Some(b);
        b
    }

    /// Oublie les bandes mémorisées (défilement horizontal, fin de passe).
    pub fn oublier_bandes(&mut self) {
        self.bandes = [None, None];
    }

    /// Le viewport des calques en unités du plan (`None` sans plan document).
    pub fn viewport(&self) -> Option<ViewportBand> {
        let m = self.mesures?;
        m.origine?;
        let b = self.bande_vue.courante?;
        Some(bande_vers_plan(b, m.echelle))
    }

    /// Un niveau voulu (unités du plan) → px de la marge.
    pub fn niveau_vers_marge(&self, top: Option<f64>) -> Option<f64> {
        niveau_vers_marge(top, self.echelle())
    }

    /// Le coin d'un élément relatif au pane, selon sa racine (voir
    /// `position_dans_le_pane`).
    pub fn position_dans_le_pane(&self, racine: Racine, left: f64, top: f64) -> Option<Point> {
        let m = self.mesures?;
        position_dans_le_pane(racine, m.pane, m.origine, m.echelle, left, top)
    }
}
