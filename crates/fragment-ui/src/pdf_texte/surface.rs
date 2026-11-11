// Port de app/src/ui/views/pdf/PdfEditor.ts (la surface : coordsAtPos, posAtCoords, lignes,
// sélection, coordsForRange, viewportRange, renderedRanges)
//
// La `TextSurface` d'un PDF. Elle ne lit aucun DOM : elle répond à partir d'un INSTANTANÉ que la
// vue (`TextePdf`) tient à jour à chaque image — la géométrie des pages (celle du modèle de la
// vue, déjà vérifiée par pdf-geometry), l'échelle, le défilement, l'origine de la zone dans la
// fenêtre, les pages dessinées et le texte des pages peintes.
//
// Repères :
// - les positions : `(page − 1) × 1e6` désigne la page (son coin), `+ 1 + i` son caractère `i`
//   (unités UTF-16) ;
// - les rects SORTANTS sont en unités du PLAN non zoomées (les points de la page, posés au coin
//   que donne la géométrie) : invariants au zoom, comme le plan `scale(echelle)` de Fragment ;
// - `pos_at_coords` reçoit des coordonnées CLIENT, ici les px logiques de la fenêtre GPUI.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fragment_pdf::document::CibleLien;
use fragment_pdf::pdf_editor::PAGE_STRIDE;
use fragment_text::{DocumentSurface, EditorPosition, EditorRange, Rect, TextSurface};

use super::modele::TextePage;

/// Le pas des positions, en `usize` (le contrat `TextSurface` parle `usize`).
const PAS: usize = PAGE_STRIDE as usize;

/// La position qui désigne la page `page` (à partir de 1) elle-même.
pub fn pos_de_page(page: usize) -> usize {
    page.saturating_sub(1) * PAS
}

/// La page (à partir de 1) d'une position.
pub fn page_de_pos(pos: usize) -> usize {
    pos / PAS + 1
}

/// L'index de caractère d'une position, `None` pour l'ancre de page.
pub fn index_de_pos(pos: usize) -> Option<usize> {
    (pos % PAS).checked_sub(1)
}

/// La place d'une page dans le plan, lue dans la géométrie du modèle.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct PlacePage {
    /// Haut et hauteur en px écran depuis l'origine du plan (`top_px`, `hauteur_px`).
    pub top_px: f64,
    pub hauteur_px: f64,
    /// Le rectangle de la page en unités du plan (`coin`).
    pub coin: Rect,
}

/// L'instantané que la vue tient à jour.
#[derive(Debug, Default)]
pub(crate) struct EtatSurface {
    pub places: Vec<PlacePage>,
    pub echelle: f64,
    /// L'origine de la zone des pages dans la fenêtre (px logiques).
    pub origine: (f64, f64),
    /// Le zoom de l'app (px logiques par px CSS).
    pub zoom: f64,
    /// Le défilement, en px CSS : (haut, gauche).
    pub defilement: (f64, f64),
    /// La bande visible (pages à partir de 1, incluses).
    pub visibles: Option<(usize, usize)>,
    pub dessinees: Vec<EditorRange>,
    /// Le texte des pages PEINTES : il meurt avec l'image, comme la couche de texte du TS.
    pub textes: HashMap<usize, Rc<TextePage>>,
    /// Les glyphes déjà lus, qui SURVIVENT à la libération d'une page : un trait ancré sur un
    /// mot ne retombe pas au coin de la page le temps que le texte revienne.
    pub memoire: HashMap<usize, Rc<TextePage>>,
    /// La sélection : (ancre, tête), en positions.
    pub selection: Option<(usize, usize)>,
}

/// Un point de la fenêtre ramené à une page : la page (à partir de 1), le point en unités de
/// la page, et s'il tombe DANS la hauteur de la page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PointPage {
    pub page: usize,
    pub x: f64,
    pub y: f64,
    pub dedans: bool,
}

/// La surface de texte d'une vue PDF : une poignée sur l'instantané de sa vue.
#[derive(Clone, Default)]
pub struct SurfacePdf {
    pub(crate) etat: Rc<RefCell<EtatSurface>>,
    demandes: Rc<Cell<u64>>,
}

impl SurfacePdf {
    pub(crate) fn new() -> Self {
        Self {
            etat: Rc::new(RefCell::new(EtatSurface {
                echelle: 1.0,
                zoom: 1.0,
                ..EtatSurface::default()
            })),
            demandes: Rc::new(Cell::new(0)),
        }
    }

    /// Le nombre de `request_update` reçus (le « poke » des calques).
    pub fn demandes_de_mise_a_jour(&self) -> u64 {
        self.demandes.get()
    }

    /// Un point du PLAN (unités non zoomées, celles des rects sortants) → la fenêtre (px logiques).
    pub fn vers_fenetre(&self, x: f64, y: f64) -> (f64, f64) {
        let e = self.etat.borrow();
        (
            e.origine.0 + (x * e.echelle - e.defilement.1) * e.zoom,
            e.origine.1 + (y * e.echelle - e.defilement.0) * e.zoom,
        )
    }

    /// Le texte de la sélection (pages peintes seulement).
    pub fn texte_selectionne(&self) -> String {
        let r = self.get_selection();
        let e = self.etat.borrow();
        let mut sortie = String::new();
        if r.from == r.to {
            return sortie;
        }
        for page in page_de_pos(r.from)..=page_de_pos(r.to) {
            let Some(t) = e.textes.get(&page) else {
                continue;
            };
            let (d, f) = bornes(page, r.from, r.to, t.longueur());
            if f > d {
                sortie.push_str(&t.extrait(d, f));
            }
        }
        sortie
    }

    /// La page sous un point de la fenêtre, comme `posAtCoords` : dans sa hauteur, ou sinon la
    /// plus proche verticalement. `None` sans pages.
    pub(crate) fn point_page(&self, x: f64, y: f64) -> Option<PointPage> {
        let e = self.etat.borrow();
        let n = e.places.len();
        if n == 0 {
            return None;
        }
        let zoom = if e.zoom > 0.0 { e.zoom } else { 1.0 };
        let x_px = (x - e.origine.0) / zoom + e.defilement.1;
        let y_px = (y - e.origine.1) / zoom + e.defilement.0;
        // La première page dont le bas dépasse le point (la dernière au-delà).
        let i = e
            .places
            .partition_point(|p| p.top_px + p.hauteur_px <= y_px)
            .min(n - 1);
        let p = e.places[i];
        let dedans = y_px >= p.top_px && y_px <= p.top_px + p.hauteur_px;
        let i = if dedans {
            i
        } else if i > 0 {
            // Hors de toute page : la précédente finit au-dessus, celle-ci commence en dessous.
            let q = e.places[i - 1];
            if y_px - (q.top_px + q.hauteur_px) < p.top_px - y_px {
                i - 1
            } else {
                i
            }
        } else {
            i
        };
        let coin = e.places[i].coin;
        let echelle = if e.echelle > 0.0 { e.echelle } else { 1.0 };
        Some(PointPage {
            page: i + 1,
            x: x_px / echelle - coin.left,
            y: y_px / echelle - coin.top,
            dedans,
        })
    }

    /// La position d'insertion la plus proche d'un point de la fenêtre, pour le glisser d'une
    /// sélection ; `None` si la page visée n'a pas (encore) de texte.
    pub(crate) fn caret_proche(&self, x: f64, y: f64) -> Option<usize> {
        let p = self.point_page(x, y)?;
        let e = self.etat.borrow();
        let i = e.textes.get(&p.page)?.caret_proche(p.x, p.y)?;
        Some(pos_de_page(p.page) + 1 + i)
    }

    /// Le caractère sous un point de la fenêtre (pas l'ancre de repli de la page).
    pub(crate) fn caret_sous(&self, x: f64, y: f64) -> Option<usize> {
        let p = self.point_page(x, y).filter(|p| p.dedans)?;
        let e = self.etat.borrow();
        let i = e.textes.get(&p.page)?.caret_sous(p.x, p.y)?;
        Some(pos_de_page(p.page) + 1 + i)
    }

    /// Le lien sous un point de la fenêtre : (page, rang du lien dans la page, cible).
    pub(crate) fn lien_sous(&self, x: f64, y: f64) -> Option<(usize, usize, CibleLien)> {
        let p = self.point_page(x, y).filter(|p| p.dedans)?;
        let e = self.etat.borrow();
        let t = e.textes.get(&p.page)?;
        let lien = t.lien_sous(p.x, p.y)?;
        let rang = t.liens.iter().position(|l| std::ptr::eq(l, lien))?;
        Some((p.page, rang, lien.cible.clone()))
    }
}

/// `[from, to)` ramenée dans le texte de `page` : des index de caractères.
fn bornes(page: usize, from: usize, to: usize, longueur: usize) -> (usize, usize) {
    let base = pos_de_page(page) + 1;
    (
        from.saturating_sub(base).min(longueur),
        to.saturating_sub(base).min(longueur),
    )
}

fn decaler(r: Rect, coin: Rect) -> Rect {
    Rect {
        left: coin.left + r.left,
        top: coin.top + r.top,
        right: coin.left + r.right,
        bottom: coin.top + r.bottom,
    }
}

impl DocumentSurface for SurfacePdf {
    /// Le rect du GLYPHE si la position désigne un caractère déjà lu ; sinon le coin de sa page
    /// (l'ancre de page, un scan, une page dont le texte n'est pas encore là).
    fn coords_at_pos(&self, offset: usize) -> Option<Rect> {
        let e = self.etat.borrow();
        let page = page_de_pos(offset);
        let coin = e.places.get(page - 1)?.coin;
        let glyphe =
            index_de_pos(offset).and_then(|i| e.memoire.get(&page).and_then(|t| t.boite(i)));
        Some(match glyphe {
            Some(b) => decaler(b, coin),
            None => coin,
        })
    }

    /// Un point de la fenêtre → le caractère sous lui, sinon l'ancre de la page qui le contient.
    /// Hors de toute page (entre deux pages, la marge), la page la plus proche verticalement :
    /// dessiner dans la marge s'ancre à la page d'en face.
    fn pos_at_coords(&self, x: f64, y: f64) -> Option<usize> {
        let p = self.point_page(x, y)?;
        if p.dedans {
            let e = self.etat.borrow();
            if let Some(i) = e.textes.get(&p.page).and_then(|t| t.caret_sous(p.x, p.y)) {
                return Some(pos_de_page(p.page) + 1 + i);
            }
        }
        Some(pos_de_page(p.page))
    }

    /// La bande montée : ce qui y est ancré hors de l'écran garde sa dernière position.
    fn viewport_range(&self) -> EditorRange {
        match self.etat.borrow().visibles {
            Some((a, b)) => EditorRange {
                from: pos_de_page(a),
                to: pos_de_page(b) + PAS - 1,
            },
            None => EditorRange::default(),
        }
    }

    /// Les pages réellement peintes (image courante ou périmée), en plages contiguës.
    fn rendered_ranges(&self) -> Vec<EditorRange> {
        self.etat.borrow().dessinees.clone()
    }

    fn request_update(&self) {
        self.demandes.set(self.demandes.get() + 1);
    }

    fn as_text(&self) -> Option<&dyn TextSurface> {
        Some(self)
    }
}

impl TextSurface for SurfacePdf {
    /// Le texte de la page `n` (à partir de 0) ; vide si elle n'est pas peinte. Une « ligne »
    /// est une page : un PDF n'a pas d'autre unité d'adressage réelle au-dessus du caractère.
    fn get_line(&self, n: usize) -> String {
        self.etat
            .borrow()
            .textes
            .get(&(n + 1))
            .map(|t| t.texte.clone())
            .unwrap_or_default()
    }

    fn line_count(&self) -> usize {
        self.etat.borrow().places.len()
    }

    fn offset_to_pos(&self, offset: usize) -> EditorPosition {
        EditorPosition {
            line: page_de_pos(offset) - 1,
            ch: index_de_pos(offset).unwrap_or(0),
        }
    }

    fn pos_to_offset(&self, pos: EditorPosition) -> usize {
        pos_de_page(pos.line + 1) + 1 + pos.ch
    }

    /// La sélection faite à la souris dans la couche de texte, en positions (vide : `{0, 0}`).
    fn get_selection(&self) -> EditorRange {
        match self.etat.borrow().selection {
            Some((a, b)) if a != b => EditorRange {
                from: a.min(b),
                to: a.max(b),
            },
            _ => EditorRange::default(),
        }
    }

    /// Un rect par rangée visuelle de la plage, sur les pages peintes.
    fn coords_for_range(&self, from: usize, to: usize) -> Vec<Rect> {
        let (debut, fin) = (from.min(to), from.max(to));
        let e = self.etat.borrow();
        let mut rects = Vec::new();
        for page in page_de_pos(debut)..=page_de_pos(fin) {
            let (Some(t), Some(place)) = (e.textes.get(&page), e.places.get(page - 1)) else {
                continue;
            };
            let (d, f) = bornes(page, debut, fin, t.longueur());
            if f <= d {
                continue;
            }
            rects.extend(t.rangees(d, f).into_iter().map(|r| decaler(r, place.coin)));
        }
        rects
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn positions_page_majeures() {
        assert_eq!(pos_de_page(1), 0);
        assert_eq!(pos_de_page(3), 2_000_000);
        assert_eq!(page_de_pos(1_999_999), 2);
        assert_eq!(index_de_pos(2_000_000), None);
        assert_eq!(index_de_pos(2_000_001), Some(0));
    }

    #[test]
    fn offset_to_pos_et_retour() {
        let s = SurfacePdf::new();
        let p = s.offset_to_pos(1_000_006);
        assert_eq!(p, EditorPosition { line: 1, ch: 5 });
        assert_eq!(s.pos_to_offset(p), 1_000_006);
        // L'ancre de page se lit au caractère 0.
        assert_eq!(
            s.offset_to_pos(1_000_000),
            EditorPosition { line: 1, ch: 0 }
        );
    }

    #[test]
    fn sans_pages_aucune_position() {
        let s = SurfacePdf::new();
        assert_eq!(s.pos_at_coords(10.0, 10.0), None);
        assert_eq!(s.coords_at_pos(0), None);
        assert_eq!(s.get_selection(), EditorRange::default());
        assert_eq!(s.viewport_range(), EditorRange::default());
    }

    fn deux_pages() -> SurfacePdf {
        let s = SurfacePdf::new();
        {
            let mut e = s.etat.borrow_mut();
            e.echelle = 2.0;
            e.places = vec![
                PlacePage {
                    top_px: 16.0,
                    hauteur_px: 200.0,
                    coin: Rect {
                        left: 10.0,
                        top: 8.0,
                        right: 60.0,
                        bottom: 108.0,
                    },
                },
                PlacePage {
                    top_px: 232.0,
                    hauteur_px: 200.0,
                    coin: Rect {
                        left: 10.0,
                        top: 116.0,
                        right: 60.0,
                        bottom: 216.0,
                    },
                },
            ];
        }
        s
    }

    #[test]
    fn entre_deux_pages_la_plus_proche_verticalement() {
        let s = deux_pages();
        // Le blanc entre les pages va de 216 à 232 px.
        assert_eq!(s.pos_at_coords(50.0, 220.0), Some(0));
        assert_eq!(s.pos_at_coords(50.0, 230.0), Some(1_000_000));
        // Au-dessus de tout : la première ; sous tout : la dernière.
        assert_eq!(s.pos_at_coords(50.0, 0.0), Some(0));
        assert_eq!(s.pos_at_coords(50.0, 9_000.0), Some(1_000_000));
    }

    #[test]
    fn la_position_zero_est_le_coin_de_la_page() {
        let s = deux_pages();
        assert_eq!(s.coords_at_pos(0).unwrap().top, 8.0);
        assert_eq!(s.coords_at_pos(1_000_000).unwrap().top, 116.0);
        // Un caractère sans texte lu : le coin de sa page.
        assert_eq!(s.coords_at_pos(1_000_003).unwrap().left, 10.0);
    }
}
