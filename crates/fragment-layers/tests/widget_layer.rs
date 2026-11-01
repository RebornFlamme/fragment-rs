// Port de app/src/ui/core/layers/WidgetLayer.test.ts (+ mapAnchor.test.ts, déjà
// porté dans fragment-text : ici la règle est vérifiée sur la VRAIE union
// `WidgetAnchor`)
//
// ★ Le TS monte un hôte d'overlays minimal sous jsdom (un div par plan) et lit
//   `el.style`. Ici `Hote` implémente `OverlayHost` + `WidgetElements` et
//   ENREGISTRE les écritures de style (`Style`) ; l'arbitre des marges et l'actif
//   sont les vrais (`GutterArbiter`, `WidgetActivation`). `FauxEditeur` est une
//   `DocumentSurface` à géométrie simple : la ligne de la position `p` est à
//   `top = 2·p`, colonne `left = 100`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use fragment_layers::gutter_arbiter::{GutterArbiter, GutterSlot, SlotElements};
use fragment_layers::overlay_host::{Disposer, OverlayHost, WidgetElements};
use fragment_layers::types::{
    CompactInfo, ElementId, GutterBand, GutterSide, OverlayPlane, Point, ViewportBand,
};
use fragment_layers::widget_activation::{ActivationEntry, ElementTree, WidgetActivation};
use fragment_layers::widget_layer::{
    BuildTicket, DomBuild, LazyWidget, MIN_COMPACT_W, MIN_GUTTER_W, ViewportCorner, WidgetAnchor,
    WidgetBox, WidgetHandle, WidgetLayer, WidgetOptions, WidgetSource, map_anchor,
};
use fragment_text::{
    Assoc, DocumentSurface, EditorChange, EditorRange, MappedAnchor, MappedPos, Rect,
};

// ── l'hôte simulé ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Default)]
struct Style {
    left: Option<f64>,
    right: Option<f64>,
    top: Option<f64>,
    bottom: Option<f64>,
    largeur_imposee: Option<f64>,
    masque: bool,
    pass_through: Option<bool>,
}

#[derive(Default)]
struct Tables {
    styles: HashMap<ElementId, Style>,
    hauteurs: HashMap<ElementId, f64>,
    rects: HashMap<ElementId, Rect>,
    focus: Option<ElementId>,
    places: HashMap<ElementId, f64>,
    caches: HashSet<ElementId>,
    montes: Vec<(ElementId, OverlayPlane)>,
    bandes: HashMap<usize, GutterBand>,
    viewport: Option<ViewportBand>,
    contenu: Option<Rect>,
    geometrie: Vec<Rc<dyn Fn()>>,
    viewport_cbs: Vec<Rc<dyn Fn()>>,
    restaurations: Vec<ElementId>,
}

type Partage = Rc<RefCell<Tables>>;

struct Elements(Partage);

impl SlotElements for Elements {
    fn is_displayed(&self, el: ElementId) -> bool {
        !self.0.borrow().styles.get(&el).is_some_and(|s| s.masque)
    }

    fn height(&self, el: ElementId) -> f64 {
        self.0.borrow().hauteurs.get(&el).copied().unwrap_or(20.0)
    }

    fn set_top(&self, el: ElementId, top: f64) {
        self.0.borrow_mut().styles.entry(el).or_default().top = Some(top);
    }

    fn set_room(&self, el: ElementId, room: Option<f64>) {
        let mut t = self.0.borrow_mut();
        match room {
            Some(r) => t.places.insert(el, r),
            None => t.places.remove(&el),
        };
    }

    fn set_clustered(&self, el: ElementId, clustered: bool) {
        let mut t = self.0.borrow_mut();
        if clustered {
            t.caches.insert(el);
        } else {
            t.caches.remove(&el);
        }
    }
}

/// Pas d'imbrication entre les éléments de ces tests.
struct Plat;

impl ElementTree for Plat {
    fn contains(&self, ancestor: ElementId, node: ElementId) -> bool {
        ancestor == node
    }
}

struct Hote {
    t: Partage,
    arbitre: GutterArbiter,
    actif: WidgetActivation,
}

impl Hote {
    fn new() -> Rc<Self> {
        let t: Partage = Rc::new(RefCell::new(Tables::default()));
        let actif = WidgetActivation::new(Rc::new(Plat));
        let a = actif.clone();
        let arbitre = GutterArbiter::new(Rc::new(Elements(Rc::clone(&t))))
            .with_visible_height(|| 1000.0)
            .with_active_el(move || a.active_el());
        Rc::new(Self { t, arbitre, actif })
    }

    fn style(&self, el: ElementId) -> Style {
        self.t.borrow().styles.get(&el).cloned().unwrap_or_default()
    }

    fn regler_bande(&self, side: GutterSide, band: Option<GutterBand>) {
        let i = usize::from(side == GutterSide::Right);
        let mut t = self.t.borrow_mut();
        match band {
            Some(b) => t.bandes.insert(i, b),
            None => t.bandes.remove(&i),
        };
    }

    fn geometrie_change(&self) {
        let cbs: Vec<_> = self.t.borrow().geometrie.clone();
        for cb in cbs {
            cb();
        }
    }

    fn viewport_change(&self) {
        let cbs: Vec<_> = self.t.borrow().viewport_cbs.clone();
        for cb in cbs {
            cb();
        }
    }
}

impl OverlayHost for Hote {
    fn mount(&self, el: ElementId, plane: OverlayPlane) -> Disposer {
        self.t.borrow_mut().montes.push((el, plane));
        let t = Rc::clone(&self.t);
        Box::new(move || {
            t.borrow_mut()
                .montes
                .retain(|(e, p)| !(*e == el && *p == plane))
        })
    }

    fn client_to_viewport(&self, x: f64, y: f64) -> Option<Point> {
        // Le pane commence à (10, 5) ; 800 × 600.
        (x >= 10.0 && y >= 5.0 && x <= 810.0 && y <= 605.0).then_some(Point {
            x: x - 10.0,
            y: y - 5.0,
        })
    }

    fn client_to_document(&self, x: f64, y: f64) -> Option<Point> {
        Some(Point { x: x - 10.0, y })
    }

    fn document_to_client(&self, x: f64, y: f64) -> Option<Point> {
        Some(Point { x: x + 10.0, y })
    }

    fn gutter_band(&self, side: GutterSide) -> Option<GutterBand> {
        let i = usize::from(side == GutterSide::Right);
        self.t.borrow().bandes.get(&i).copied()
    }

    fn gutter_slot(&self, el: ElementId, side: GutterSide) -> GutterSlot {
        self.arbitre.slot(el, side)
    }

    fn track_activation(&self, el: ElementId) -> ActivationEntry {
        self.actif.track(el)
    }

    fn on_geometry_change(&self, cb: Box<dyn Fn()>) -> Disposer {
        self.t.borrow_mut().geometrie.push(Rc::from(cb));
        Box::new(|| {})
    }

    fn viewport(&self) -> Option<ViewportBand> {
        self.t.borrow().viewport
    }

    fn on_viewport_change(&self, cb: Box<dyn Fn()>) -> Disposer {
        self.t.borrow_mut().viewport_cbs.push(Rc::from(cb));
        Box::new(|| {})
    }

    fn content_client_rect(&self) -> Option<Rect> {
        self.t.borrow().contenu
    }
}

impl WidgetElements for Hote {
    fn attach(&self, el: ElementId, pass_through: bool) {
        self.t
            .borrow_mut()
            .styles
            .entry(el)
            .or_default()
            .pass_through = Some(pass_through);
    }

    fn set_viewport_edges(
        &self,
        el: ElementId,
        left: Option<f64>,
        right: Option<f64>,
        top: Option<f64>,
        bottom: Option<f64>,
    ) {
        let mut t = self.t.borrow_mut();
        let s = t.styles.entry(el).or_default();
        s.left = left;
        s.right = right;
        s.top = top;
        s.bottom = bottom;
    }

    fn clear_right_bottom(&self, el: ElementId) {
        let mut t = self.t.borrow_mut();
        let s = t.styles.entry(el).or_default();
        s.right = None;
        s.bottom = None;
    }

    fn set_left(&self, el: ElementId, left: f64) {
        self.t.borrow_mut().styles.entry(el).or_default().left = Some(left);
    }

    fn set_top(&self, el: ElementId, top: f64) {
        self.t.borrow_mut().styles.entry(el).or_default().top = Some(top);
    }

    fn impose_width(&self, el: ElementId, width: f64) {
        self.t
            .borrow_mut()
            .styles
            .entry(el)
            .or_default()
            .largeur_imposee = Some(width);
    }

    fn restore_width(&self, el: ElementId) {
        let mut t = self.t.borrow_mut();
        t.styles.entry(el).or_default().largeur_imposee = None;
        t.restaurations.push(el);
    }

    fn set_displayed(&self, el: ElementId, displayed: bool) {
        self.t.borrow_mut().styles.entry(el).or_default().masque = !displayed;
    }

    fn offset_box(&self, el: ElementId) -> WidgetBox {
        let t = self.t.borrow();
        let s = t.styles.get(&el).cloned().unwrap_or_default();
        WidgetBox {
            // La position RENDUE : pour une ancre depuis la droite / le bas, 700 / 500.
            left: s.left.unwrap_or(700.0),
            top: s.top.unwrap_or(500.0),
            width: s.largeur_imposee.unwrap_or(50.0),
            height: t.hauteurs.get(&el).copied().unwrap_or(20.0),
        }
    }

    fn written_origin(&self, el: ElementId) -> (f64, f64) {
        let s = self.style(el);
        (s.left.unwrap_or(0.0), s.top.unwrap_or(0.0))
    }

    fn client_rect(&self, el: ElementId) -> Rect {
        self.t.borrow().rects.get(&el).copied().unwrap_or_default()
    }

    fn contains_focus(&self, el: ElementId) -> bool {
        self.t.borrow().focus == Some(el)
    }
}

// ── l'éditeur simulé ────────────────────────────────────────────────────────

struct FauxEditeur {
    viewport: Cell<(usize, usize)>,
    rendues: RefCell<Vec<(usize, usize)>>,
    sans_coords: RefCell<HashSet<usize>>,
}

impl FauxEditeur {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            viewport: Cell::new((0, 10_000)),
            rendues: RefCell::new(vec![(0, 10_000)]),
            sans_coords: RefCell::new(HashSet::new()),
        })
    }
}

impl DocumentSurface for FauxEditeur {
    fn coords_at_pos(&self, offset: usize) -> Option<Rect> {
        if self.sans_coords.borrow().contains(&offset) {
            return None;
        }
        let top = offset as f64 * 2.0;
        Some(Rect {
            left: 100.0,
            top,
            right: 110.0,
            bottom: top + 20.0,
        })
    }

    fn pos_at_coords(&self, _x: f64, y: f64) -> Option<usize> {
        (y >= 0.0).then(|| (y / 2.0).round() as usize)
    }

    fn viewport_range(&self) -> EditorRange {
        let (from, to) = self.viewport.get();
        EditorRange { from, to }
    }

    fn rendered_ranges(&self) -> Vec<EditorRange> {
        self.rendues
            .borrow()
            .iter()
            .map(|&(from, to)| EditorRange { from, to })
            .collect()
    }

    fn request_update(&self) {}
}

// ── la mise en place ────────────────────────────────────────────────────────

fn calque(avec_editeur: bool) -> (Rc<Hote>, Rc<FauxEditeur>, WidgetLayer) {
    let hote = Hote::new();
    let editeur = FauxEditeur::new();
    let ed: Option<Rc<dyn DocumentSurface>> = avec_editeur.then(|| Rc::clone(&editeur) as _);
    let couche = WidgetLayer::new(ed, Rc::clone(&hote) as _, Rc::clone(&hote) as _);
    (hote, editeur, couche)
}

fn element(n: u64) -> ElementId {
    ElementId(n)
}

fn pose(couche: &WidgetLayer, el: ElementId, a: WidgetAnchor) -> Rc<dyn WidgetHandle> {
    couche.add_widget(WidgetSource::Element(el), a, WidgetOptions::default())
}

fn doc(pos: usize) -> WidgetAnchor {
    WidgetAnchor::Document {
        pos,
        to: None,
        dx: 5.0,
        dy: 3.0,
    }
}

fn marge(pos: usize, dy: f64) -> WidgetAnchor {
    WidgetAnchor::Gutter {
        side: GutterSide::Right,
        pos,
        to: None,
        dy,
    }
}

/// Un `mapPos` de CM réduit : `n` caractères insérés en `at` (n < 0 : supprimés).
fn edition(at: usize, n: i64) -> EditorChange {
    EditorChange::doc(move |pos, assoc| {
        if n < 0 {
            let fin = at + n.unsigned_abs() as usize;
            if pos > at && pos < fin {
                return MappedPos {
                    pos: at,
                    deleted: true,
                };
            }
            if pos >= fin {
                return MappedPos {
                    pos: pos - n.unsigned_abs() as usize,
                    deleted: false,
                };
            }
            return MappedPos {
                pos,
                deleted: false,
            };
        }
        let n = n as usize;
        let pousse = pos > at || (pos == at && assoc == Assoc::After);
        MappedPos {
            pos: if pousse { pos + n } else { pos },
            deleted: false,
        }
    })
}

// ── WidgetLayer — ancre viewport par COIN, widget transparent aux clics ─────

#[test]
fn defaut_mesuree_depuis_le_haut_gauche_et_le_widget_capte_les_clics_inchange() {
    let (hote, _, couche) = calque(false);
    let el = element(1);
    pose(&couche, el, WidgetAnchor::viewport(10.0, 20.0));
    let s = hote.style(el);
    assert_eq!(
        (s.left, s.right, s.top, s.bottom),
        (Some(10.0), None, Some(20.0), None)
    );
    // `pointer-events: auto` : il capte.
    assert_eq!(s.pass_through, Some(false));
}

#[test]
fn depuis_un_autre_coin_right_bottom_ecrits_les_cotes_opposes_vides_et_relu_au_changement_d_ancre()
{
    let (hote, _, couche) = calque(false);
    let el = element(1);
    let h = pose(
        &couche,
        el,
        WidgetAnchor::Viewport {
            x: 0.0,
            y: 12.0,
            from: Some(ViewportCorner::BottomLeft),
        },
    );
    let s = hote.style(el);
    assert_eq!(
        (s.left, s.right, s.top, s.bottom),
        (Some(0.0), None, None, Some(12.0))
    );
    h.set_anchor(WidgetAnchor::Viewport {
        x: 5.0,
        y: 6.0,
        from: Some(ViewportCorner::TopRight),
    });
    let s = hote.style(el);
    assert_eq!(
        (s.left, s.right, s.top, s.bottom),
        (None, Some(5.0), Some(6.0), None)
    );
    h.set_anchor(WidgetAnchor::viewport(1.0, 2.0));
    let s = hote.style(el);
    assert_eq!(
        (s.left, s.right, s.top, s.bottom),
        (Some(1.0), None, Some(2.0), None)
    );
}

#[test]
fn pass_through_l_element_ne_capte_pas_ses_enfants_decident() {
    let (hote, _, couche) = calque(false);
    let el = element(1);
    couche.add_widget(
        WidgetSource::Element(el),
        WidgetAnchor::Viewport {
            x: 0.0,
            y: 0.0,
            from: Some(ViewportCorner::BottomLeft),
        },
        WidgetOptions { pass_through: true },
    );
    assert_eq!(hote.style(el).pass_through, Some(true));
}

// ── mapAnchor (mapAnchor.test.ts, sur la vraie union) ───────────────────────

fn applique(a: WidgetAnchor, c: &EditorChange) -> MappedAnchor<WidgetAnchor> {
    map_anchor(a, &*c.map_pos)
}

#[test]
fn map_anchor_un_point_colle_au_texte_qui_suit_une_plage_suit_map_range_une_ancre_a_l_ecran_passe_inchangee()
 {
    // Un point : insertion pile dessus → poussé (assoc 1).
    let m = applique(doc(10), &edition(10, 3));
    assert_eq!((m.anchor.pos(), m.deleted), (Some(13), false));
    // Une plage : taper dedans l'allonge, supprimée seulement si tout son texte part.
    let plage = WidgetAnchor::Gutter {
        side: GutterSide::Left,
        pos: 10,
        to: Some(20),
        dy: 1.0,
    };
    let m = applique(plage, &edition(15, 4));
    assert_eq!(
        m.anchor,
        WidgetAnchor::Gutter {
            side: GutterSide::Left,
            pos: 10,
            to: Some(24),
            dy: 1.0
        }
    );
    assert!(!m.deleted);
    let m = applique(plage, &edition(5, -20));
    assert!(m.deleted);
    // À l'écran : inchangée.
    let vp = WidgetAnchor::viewport(1.0, 2.0);
    let m = applique(vp, &edition(0, 9));
    assert_eq!((m.anchor, m.deleted), (vp, false));
}

// ── cas ajoutés : ancres document, remappage, masquage, marge, déclarés ─────

#[test]
fn ancre_document_posee_aux_coords_du_glyphe_plus_dx_dy_et_remappee_a_l_edition() {
    let (hote, _, couche) = calque(true);
    let el = element(1);
    let h = pose(&couche, el, doc(10));
    let s = hote.style(el);
    assert_eq!((s.left, s.top), (Some(105.0), Some(23.0))); // 100+5, 2·10+3
    assert_eq!((s.right, s.bottom), (None, None));
    // Cinq caractères insérés au-dessus : l'ancre suit son texte, le widget se recale.
    couche.on_editor_change(&edition(2, 5));
    assert_eq!(h.get_anchor().pos(), Some(15));
    let s = hote.style(el);
    assert_eq!((s.left, s.top), (Some(105.0), Some(33.0)));
}

#[test]
fn texte_ancre_supprime_le_widget_se_detache_en_viewport_a_sa_place_a_l_ecran() {
    let (hote, _, couche) = calque(true);
    let el = element(1);
    hote.t.borrow_mut().rects.insert(
        el,
        Rect {
            left: 210.0,
            top: 105.0,
            right: 260.0,
            bottom: 125.0,
        },
    );
    let h = pose(
        &couche,
        el,
        WidgetAnchor::Document {
            pos: 10,
            to: Some(20),
            dx: 0.0,
            dy: 0.0,
        },
    );
    assert!(matches!(h.get_anchor(), WidgetAnchor::Document { .. }));
    couche.on_editor_change(&edition(5, -20));
    // Le pane commence à (10, 5) : (210, 105) client → (200, 100) viewport.
    assert_eq!(h.get_anchor(), WidgetAnchor::viewport(200.0, 100.0));
    let s = hote.style(el);
    assert_eq!((s.left, s.top), (Some(200.0), Some(100.0)));
    // Il a changé de plan : monté dans le plan viewport, plus dans le document.
    assert_eq!(hote.t.borrow().montes, vec![(el, OverlayPlane::Viewport)]);
}

#[test]
fn texte_ancre_supprime_widget_hors_du_pane_il_garde_une_ancre_document_au_point_d_effondrement() {
    let (hote, _, couche) = calque(true);
    let el = element(1);
    // Hors du pane (x < 10) : pas de conversion viewport possible.
    hote.t.borrow_mut().rects.insert(
        el,
        Rect {
            left: -50.0,
            top: 0.0,
            right: 0.0,
            bottom: 20.0,
        },
    );
    let h = pose(&couche, el, doc(10));
    couche.on_editor_change(&edition(5, -20));
    assert_eq!(h.get_anchor().pos(), Some(5));
    assert!(matches!(h.get_anchor(), WidgetAnchor::Document { .. }));
}

#[test]
fn texte_replie_le_widget_est_masque_contenu_cache_donc_widget_cache_puis_rendu() {
    let (hote, editeur, couche) = calque(true);
    let el = element(1);
    pose(&couche, el, doc(50));
    assert!(!hote.style(el).masque);
    // La ligne 50 est repliée : un trou dans les portions dessinées.
    *editeur.rendues.borrow_mut() = vec![(0, 40), (60, 10_000)];
    couche.on_editor_change(&edition(100, 1));
    assert!(hote.style(el).masque);
    // Déplié : rendu à la vue.
    *editeur.rendues.borrow_mut() = vec![(0, 10_000)];
    couche.on_editor_change(&edition(100, 1));
    assert!(!hote.style(el).masque);
}

#[test]
fn hors_du_viewport_rendu_pas_de_coords_on_garde_la_derniere_position() {
    let (hote, editeur, couche) = calque(true);
    let el = element(1);
    pose(&couche, el, doc(10));
    let avant = hote.style(el);
    editeur.sans_coords.borrow_mut().insert(10);
    couche.on_editor_change(&edition(100, 1));
    assert_eq!(hote.style(el).left, avant.left);
    assert_eq!(hote.style(el).top, avant.top);
}

#[test]
fn marge_la_bande_impose_left_et_largeur_le_haut_vient_de_l_arbitre() {
    let (hote, _, couche) = calque(true);
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    let el = element(1);
    let h = pose(&couche, el, marge(10, 4.0));
    hote.arbitre.flush();
    let s = hote.style(el);
    assert_eq!(s.left, Some(600.0));
    assert_eq!(s.largeur_imposee, Some(180.0));
    // Voulu = 2·10 + 4 = 24, seul : pile sur son ancre.
    assert_eq!(s.top, Some(24.0));
    assert!(h.fits_gutter());
    // Sa boîte est lue APRÈS la passe de l'arbitre (get_box force le flush).
    let b = h.get_box();
    assert_eq!(
        b.map(|b| (b.left, b.top, b.width)),
        Some((600.0, 24.0, 180.0))
    );
}

#[test]
fn marge_trop_etroite_le_widget_se_masque_et_l_annonce_on_fit_change_reduit_il_tient_jusqu_a_28() {
    let (hote, _, couche) = calque(true);
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    let el = element(1);
    let fits = Rc::new(RefCell::new(Vec::new()));
    let f = Rc::clone(&fits);
    let h = pose(&couche, el, marge(10, 0.0));
    let _ = h.on_fit_change(Box::new(move |v| f.borrow_mut().push(v)));
    assert!(!hote.style(el).masque);
    // La fenêtre rétrécit : la bande n'a plus la largeur d'un widget entier.
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: MIN_GUTTER_W - 1.0,
        }),
    );
    hote.geometrie_change();
    assert!(hote.style(el).masque);
    assert!(!h.fits_gutter());
    assert_eq!(*fits.borrow(), vec![false]);
    assert!(h.get_box().is_none());
    // Réduit à ses icônes : il tient dans une bande de MIN_COMPACT_W.
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: MIN_COMPACT_W,
        }),
    );
    let info = Rc::new(CompactInfo::new("Fil", vec!["chat".into()], || {}));
    h.set_compact(Some(Rc::clone(&info)));
    assert!(!hote.style(el).masque);
    assert_eq!(hote.style(el).largeur_imposee, None); // sa largeur à lui
    // Le même `info` répété ne fait rien (identité) ; `None` rend le widget entier.
    h.set_compact(Some(info));
    h.set_compact(None);
    assert!(hote.style(el).masque);
}

#[test]
fn quitter_la_marge_rend_la_largeur_et_la_place_a_l_arbitre() {
    let (hote, _, couche) = calque(true);
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    let el = element(1);
    let h = pose(&couche, el, marge(10, 0.0));
    hote.arbitre.flush();
    let places = Rc::new(RefCell::new(Vec::new()));
    let p = Rc::clone(&places);
    let _ = h.on_room_change(Box::new(move |r| p.borrow_mut().push(r)));
    assert_eq!(h.get_room(), Some(f64::INFINITY));
    h.set_anchor(doc(10));
    assert_eq!(hote.t.borrow().restaurations, vec![el]);
    assert_eq!(hote.style(el).largeur_imposee, None);
    // Plus en marge : plus de place, annoncé.
    assert_eq!(h.get_room(), None);
    assert_eq!(*places.borrow(), vec![None]);
    assert!(h.fits_gutter());
}

#[test]
fn deux_widgets_de_marge_de_deux_calques_s_ecartent_sans_se_recouvrir() {
    let (hote, _, couche) = calque(true);
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    // Deux calques sur la MÊME vue : un seul arbitre, au niveau de l'hôte.
    let autre = WidgetLayer::new(
        Some(Rc::clone(&FauxEditeur::new()) as Rc<dyn DocumentSurface>),
        Rc::clone(&hote) as _,
        Rc::clone(&hote) as _,
    );
    let (a, b) = (element(1), element(2));
    pose(&couche, a, marge(10, 0.0)); // voulu 20, hauteur 20
    pose(&autre, b, marge(11, 0.0)); // voulu 22
    hote.arbitre.flush();
    let (ta, tb) = (hote.style(a).top, hote.style(b).top);
    // Voulus 20 et 22, hauteurs 20, gap 8 : il manque 26 → 13 chacun.
    assert_eq!((ta, tb), (Some(7.0), Some(35.0)));
}

#[test]
fn le_deplacement_par_l_arbitre_previent_on_box_change() {
    let (hote, _, couche) = calque(true);
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    let (a, b) = (element(1), element(2));
    let ha = pose(&couche, a, marge(10, 0.0));
    hote.arbitre.flush();
    let n = Rc::new(Cell::new(0));
    let c = Rc::clone(&n);
    let _ = ha.on_box_change(Box::new(move || c.set(c.get() + 1)));
    // Un widget qui arrive pousse le premier.
    pose(&couche, b, marge(11, 0.0));
    hote.arbitre.flush();
    assert_eq!(n.get(), 1);
}

#[test]
fn la_geometrie_qui_change_replace_les_marges() {
    let (hote, _, couche) = calque(true);
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    let el = element(1);
    pose(&couche, el, marge(10, 0.0));
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 500.0,
            width: 120.0,
        }),
    );
    hote.geometrie_change();
    assert_eq!(hote.style(el).left, Some(500.0));
    assert_eq!(hote.style(el).largeur_imposee, Some(120.0));
}

#[test]
fn un_widget_de_marge_neuf_sans_coords_reessaie_a_l_image_suivante_dix_fois_au_plus() {
    let (hote, editeur, couche) = calque(true);
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    editeur.sans_coords.borrow_mut().insert(10);
    let el = element(1);
    pose(&couche, el, marge(10, 0.0));
    assert!(couche.needs_frame());
    // Le texte devient mesurable : l'image suivante le place.
    editeur.sans_coords.borrow_mut().clear();
    couche.run_frame();
    hote.arbitre.flush();
    assert_eq!(hote.style(el).top, Some(20.0));
    // Toujours pas de coords : 10 essais, pas un de plus.
    let (hote2, editeur2, couche2) = calque(true);
    hote2.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 600.0,
            width: 180.0,
        }),
    );
    editeur2.sans_coords.borrow_mut().insert(10);
    pose(&couche2, element(1), marge(10, 0.0));
    let mut images = 0;
    while couche2.needs_frame() {
        couche2.run_frame();
        images += 1;
        assert!(images <= 20, "boucle");
    }
    assert_eq!(images, 10);
}

#[test]
fn ancres_a_partir_d_un_point_client_document_viewport_marge() {
    let (hote, _, couche) = calque(true);
    // Le glyphe de la position 50 : top 100. On vise (150, 107) client.
    let a = couche.document_anchor_at(150.0, 107.0);
    // posAtCoords(y=107) → round(53.5) = 54 → glyphe top 108 ; dy = 107 − 108.
    assert_eq!(
        a,
        Some(WidgetAnchor::Document {
            pos: 54,
            to: None,
            dx: 140.0 - 100.0,
            dy: 107.0 - 108.0
        })
    );
    assert_eq!(
        couche.viewport_anchor_at(60.0, 45.0),
        Some(WidgetAnchor::viewport(50.0, 40.0))
    );
    assert_eq!(couche.viewport_anchor_at(0.0, 0.0), None); // hors du pane
    // La marge sonde au MILIEU de la colonne.
    hote.t.borrow_mut().contenu = Some(Rect {
        left: 100.0,
        top: 0.0,
        right: 500.0,
        bottom: 900.0,
    });
    let g = couche.gutter_anchor_at(200.0, GutterSide::Left);
    assert_eq!(
        g,
        Some(WidgetAnchor::Gutter {
            side: GutterSide::Left,
            pos: 100,
            to: None,
            dy: 0.0
        })
    );
    assert!(couche.can_anchor_to_document());
}

#[test]
fn sans_editeur_seules_les_ancres_viewport_existent() {
    let (_, _, couche) = calque(false);
    assert!(!couche.can_anchor_to_document());
    assert_eq!(couche.document_anchor_at(150.0, 107.0), None);
    assert_eq!(couche.gutter_anchor_at(200.0, GutterSide::Left), None);
}

#[test]
fn gutter_fits_dit_si_la_bande_a_la_largeur_d_un_widget() {
    let (hote, _, couche) = calque(true);
    assert!(!couche.gutter_fits(GutterSide::Right)); // pas de bande
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 0.0,
            width: MIN_GUTTER_W,
        }),
    );
    assert!(couche.gutter_fits(GutterSide::Right));
    hote.regler_bande(
        GutterSide::Right,
        Some(GutterBand {
            left: 0.0,
            width: MIN_GUTTER_W - 1.0,
        }),
    );
    assert!(!couche.gutter_fits(GutterSide::Right));
}

#[test]
fn get_box_ancre_depuis_la_droite_ou_le_bas_lit_la_position_rendue() {
    let (_, _, couche) = calque(false);
    let el = element(1);
    let h = pose(&couche, el, WidgetAnchor::viewport(10.0, 20.0));
    assert_eq!(
        h.get_box(),
        Some(WidgetBox {
            left: 10.0,
            top: 20.0,
            width: 50.0,
            height: 20.0
        })
    );
    h.set_anchor(WidgetAnchor::Viewport {
        x: 0.0,
        y: 0.0,
        from: Some(ViewportCorner::BottomRight),
    });
    // Les côtés écrits (right/bottom) ne disent pas où il est : l'hôte le mesure.
    assert_eq!(h.get_box().map(|b| (b.left, b.top)), Some((700.0, 500.0)));
}

// ── widgets DÉCLARÉS (LazyWidget) : construits / détruits par le viewport ───

#[derive(Default)]
struct Journal {
    construits: Vec<ElementId>,
    montes: Vec<ElementId>,
    detruits: Vec<ElementId>,
}

struct Paresseux {
    suivant: Cell<u64>,
    journal: Rc<RefCell<Journal>>,
    /// `Pending` : l'hôte livre plus tard ; le ticket reçu est gardé ici.
    asynchrone: bool,
    ticket: Cell<Option<BuildTicket>>,
    echec: Cell<bool>,
    handle: RefCell<Option<Rc<dyn WidgetHandle>>>,
    retire_a_la_destruction: Cell<bool>,
}

impl Paresseux {
    fn new(asynchrone: bool) -> (Rc<Self>, Rc<RefCell<Journal>>) {
        let journal = Rc::new(RefCell::new(Journal::default()));
        (
            Rc::new(Self {
                suivant: Cell::new(100),
                journal: Rc::clone(&journal),
                asynchrone,
                ticket: Cell::new(None),
                echec: Cell::new(false),
                handle: RefCell::new(None),
                retire_a_la_destruction: Cell::new(false),
            }),
            journal,
        )
    }
}

impl LazyWidget for Paresseux {
    fn to_dom(&self, _handle: Rc<dyn WidgetHandle>, ticket: BuildTicket) -> DomBuild {
        if self.echec.get() {
            return DomBuild::Failed;
        }
        if self.asynchrone {
            self.ticket.set(Some(ticket));
            return DomBuild::Pending;
        }
        self.suivant.set(self.suivant.get() + 1);
        let el = ElementId(self.suivant.get());
        self.journal.borrow_mut().construits.push(el);
        DomBuild::Ready(el)
    }

    fn mounted(&self, el: ElementId, _handle: Rc<dyn WidgetHandle>) {
        self.journal.borrow_mut().montes.push(el);
    }

    fn destroy(&self, el: ElementId) {
        self.journal.borrow_mut().detruits.push(el);
        // Un objet qui se défait retire souvent son handle : ignoré pendant la destruction.
        if self.retire_a_la_destruction.get()
            && let Some(h) = self.handle.borrow().as_ref()
        {
            h.remove();
        }
    }
}

#[test]
fn widget_declare_construit_dans_le_viewport_detruit_en_sortant_le_handle_vit_toujours() {
    let (hote, _, couche) = calque(true);
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 0.0,
        bottom: 300.0,
    });
    let (lazy, journal) = Paresseux::new(false);
    lazy.retire_a_la_destruction.set(true);
    // Hors du viewport (top = 2·500 + 3) : rien n'est construit.
    let h = couche.add_widget(
        WidgetSource::Lazy(Rc::clone(&lazy) as _),
        doc(500),
        WidgetOptions::default(),
    );
    *lazy.handle.borrow_mut() = Some(Rc::clone(&h));
    assert!(journal.borrow().construits.is_empty());
    assert!(hote.t.borrow().montes.is_empty());
    // Une édition qui ramène l'ancre dans le viewport : construit, monté, inscrit.
    couche.on_editor_change(&edition(0, -380));
    assert_eq!(h.get_anchor().pos(), Some(120));
    assert_eq!(journal.borrow().construits.len(), 1);
    assert_eq!(journal.borrow().montes, journal.borrow().construits);
    let el = journal.borrow().construits[0];
    assert_eq!(hote.t.borrow().montes, vec![(el, OverlayPlane::Document)]);
    assert_eq!(hote.style(el).left, Some(105.0));
    // Elle ressort (viewport qui défile) : détruit, démonté — mais le handle vit.
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 1000.0,
        bottom: 1300.0,
    });
    hote.viewport_change();
    assert_eq!(journal.borrow().detruits, vec![el]);
    assert!(hote.t.borrow().montes.is_empty());
    // ★ Le `remove` venu du `destroy` du système n'a PAS retiré la déclaration.
    assert_eq!(h.get_anchor().pos(), Some(120));
    // Elle revient : reconstruite (un autre élément).
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 0.0,
        bottom: 300.0,
    });
    hote.viewport_change();
    assert_eq!(journal.borrow().construits.len(), 2);
    // Un retrait EXPLICITE détruit et oublie.
    lazy.retire_a_la_destruction.set(false);
    h.remove();
    assert_eq!(journal.borrow().detruits.len(), 2);
    hote.viewport_change();
    assert_eq!(journal.borrow().construits.len(), 2);
}

#[test]
fn widget_declare_asynchrone_pending_puis_deliver_une_construction_depassee_est_refusee() {
    let (hote, _, couche) = calque(true);
    let dedans = ViewportBand {
        top: 0.0,
        bottom: 300.0,
    };
    let dehors = ViewportBand {
        top: 1000.0,
        bottom: 1300.0,
    };
    hote.t.borrow_mut().viewport = Some(dedans);
    let (lazy, journal) = Paresseux::new(true);
    let h = couche.add_widget(
        WidgetSource::Lazy(Rc::clone(&lazy) as _),
        doc(10),
        WidgetOptions::default(),
    );
    // `to_dom` a rendu `Pending` : rien n'est monté, le ticket est gardé.
    let t1 = lazy.ticket.get().unwrap_or(BuildTicket(0));
    assert!(hote.t.borrow().montes.is_empty());
    // Pendant l'attente, `position` ne reconstruit pas (construction en cours).
    lazy.ticket.set(None);
    hote.viewport_change();
    assert_eq!(lazy.ticket.get(), None);
    // L'élément arrive : posé, monté, placé.
    let el1 = ElementId(500);
    assert!(couche.deliver(&h, t1, el1));
    assert_eq!(journal.borrow().montes, vec![el1]);
    assert_eq!(hote.t.borrow().montes, vec![(el1, OverlayPlane::Document)]);
    assert_eq!(hote.style(el1).top, Some(23.0));
    // Il sort du viewport : détruit. Il revient : nouvelle construction, NOUVEAU jeton.
    hote.t.borrow_mut().viewport = Some(dehors);
    hote.viewport_change();
    assert_eq!(journal.borrow().detruits, vec![el1]);
    hote.t.borrow_mut().viewport = Some(dedans);
    hote.viewport_change();
    let t2 = lazy.ticket.get().unwrap_or(BuildTicket(0));
    assert_ne!(t1, t2);
    // L'ancienne livraison arrive en retard : refusée, l'élément est défait.
    let en_retard = ElementId(501);
    assert!(!couche.deliver(&h, t1, en_retard));
    assert_eq!(journal.borrow().detruits, vec![el1, en_retard]);
    assert!(hote.t.borrow().montes.is_empty());
    // La bonne : posée.
    let el2 = ElementId(502);
    assert!(couche.deliver(&h, t2, el2));
    assert_eq!(hote.t.borrow().montes, vec![(el2, OverlayPlane::Document)]);
}

#[test]
fn widget_declare_dont_to_dom_leve_n_existe_pas_et_sera_retente() {
    let (hote, _, couche) = calque(true);
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 0.0,
        bottom: 300.0,
    });
    let (lazy, journal) = Paresseux::new(false);
    lazy.echec.set(true);
    couche.add_widget(
        WidgetSource::Lazy(Rc::clone(&lazy) as _),
        doc(10),
        WidgetOptions::default(),
    );
    assert!(journal.borrow().construits.is_empty());
    lazy.echec.set(false);
    couche.on_editor_change(&edition(1000, 1));
    assert_eq!(journal.borrow().construits.len(), 1);
}

#[test]
fn l_actif_et_celui_qui_a_le_focus_restent_meme_hors_du_viewport() {
    let (hote, _, couche) = calque(true);
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 0.0,
        bottom: 300.0,
    });
    let (lazy, journal) = Paresseux::new(false);
    let h = couche.add_widget(
        WidgetSource::Lazy(Rc::clone(&lazy) as _),
        doc(10),
        WidgetOptions::default(),
    );
    let el = journal.borrow().construits[0];
    // Le focus entre dans le widget, puis le viewport s'en éloigne : il reste.
    hote.t.borrow_mut().focus = Some(el);
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 1000.0,
        bottom: 1300.0,
    });
    hote.viewport_change();
    assert!(journal.borrow().detruits.is_empty());
    // Le focus sort : revu à la microtâche suivante → détruit.
    hote.t.borrow_mut().focus = None;
    couche.focus_out(el);
    assert!(couche.needs_microtasks());
    couche.run_microtasks();
    assert_eq!(journal.borrow().detruits, vec![el]);
    // Actif (cliqué) : il reste de même ; relâché, il est revu.
    hote.viewport_change(); // reconstruit ? non : dehors
    assert_eq!(journal.borrow().construits.len(), 1);
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 0.0,
        bottom: 300.0,
    });
    hote.viewport_change();
    let el2 = journal.borrow().construits[1];
    h.set_active(true);
    assert!(h.is_active());
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 1000.0,
        bottom: 1300.0,
    });
    hote.viewport_change();
    assert_eq!(journal.borrow().detruits.len(), 1);
    h.set_active(false);
    couche.run_microtasks();
    assert_eq!(journal.borrow().detruits, vec![el, el2]);
}

#[test]
fn l_activation_est_rejouee_a_la_naissance_de_l_element_manuel_et_feuille() {
    let (hote, _, couche) = calque(true);
    hote.t.borrow_mut().viewport = Some(ViewportBand {
        top: 0.0,
        bottom: 300.0,
    });
    // Hors viewport : pas d'élément, donc pas d'inscription à l'actif…
    let (lazy, _) = Paresseux::new(false);
    let h = couche.add_widget(
        WidgetSource::Lazy(Rc::clone(&lazy) as _),
        doc(900),
        WidgetOptions::default(),
    );
    // …mais ce qu'on lui demande est mémorisé et rejoué à sa naissance.
    h.set_activation_manual(true);
    h.set_activation_leaf(Some(fragment_layers::types::LeafId(9)));
    assert!(!h.is_active());
    couche.on_editor_change(&edition(0, -800));
    h.set_active(true);
    assert!(h.is_active());
    assert_eq!(
        hote.actif.active_leaf(),
        Some(fragment_layers::types::LeafId(9))
    );
    let vus = Rc::new(RefCell::new(Vec::new()));
    let v = Rc::clone(&vus);
    let _ = h.on_active_change(Box::new(move |a| v.borrow_mut().push(a)));
    h.set_active(false);
    assert_eq!(*vus.borrow(), vec![false]);
}

#[test]
fn detruire_le_calque_demonte_tout_et_se_desabonne() {
    let (hote, _, couche) = calque(true);
    let el = element(1);
    pose(&couche, el, doc(10));
    assert_eq!(hote.t.borrow().montes.len(), 1);
    couche.destroy();
    assert!(hote.t.borrow().montes.is_empty());
}
