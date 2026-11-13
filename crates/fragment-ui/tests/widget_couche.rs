//! Tests de la couche des widgets (tâche `fr-t5-widgets`), dans une fenêtre de test GPUI (sans GPU),
//! sur une FAUSSE surface de texte (des lignes de 20 px, des caractères de 8 px, un repli, des
//! éditions) et les widgets de TEST de `widget_couche::essai`. Les cas sont ceux des specs e2e de
//! Fragment `viewport` (hors PDF et hors records), `gutter-arbiter` (le globe = le RIGIDE, les
//! cartes = des VUES), `gutter-room` et `widget-anchor` (hors « déborde sans être rogné », scène),
//! nommés d'après elles. Les données sont réécrites pour cette surface.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_layers::overlay_host::{OverlayHost, WidgetElements};
use fragment_layers::types::{GutterSide, Point};
use fragment_layers::widget_layer::WidgetAnchor;
use fragment_text::{Assoc, DocumentSurface, EditorChange, EditorRange, MappedPos, Rect};
use fragment_ui::calques::{HoteCalques, SourcePlans};
use fragment_ui::widget_couche::{
    CalqueWidgets, CoucheWidgets, SorteEssai, SpecEssai, WidgetEssai, poser_essai,
};
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, Modifiers, MouseButton, ParentElement as _,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, point, px, size,
};

// ─── La fausse surface ───

const LIGNE: f64 = 20.0;
const CAR: f64 = 8.0;
const LONGUEUR: usize = 51;
const NB_LIGNES: usize = 800;
const HAUT_PANE: f64 = 600.0;

/// La colonne de lecture (bords gauche et droit, px client) et le défilement.
struct Source {
    defile: Cell<f64>,
    colonne: Cell<(f64, f64)>,
}

impl SourcePlans for Source {
    fn origine(&self) -> Option<Point> {
        Some(Point {
            x: self.colonne.get().0,
            y: -self.defile.get(),
        })
    }
    fn colonne(&self) -> Option<Rect> {
        let (l, r) = self.colonne.get();
        Some(Rect {
            left: l,
            top: -self.defile.get(),
            right: r,
            bottom: NB_LIGNES as f64 * LIGNE - self.defile.get(),
        })
    }
}

/// Des lignes (longueur en caractères, saut compris) ; un repli cache les lignes `a..=b`.
struct Surface {
    source: Rc<Source>,
    lignes: RefCell<Vec<usize>>,
    repli: Cell<Option<(usize, usize)>>,
}

impl Surface {
    fn debut(&self, ligne: usize) -> usize {
        self.lignes.borrow()[..ligne].iter().sum()
    }
    fn total(&self) -> usize {
        self.lignes.borrow().iter().sum()
    }
    fn ligne_de(&self, pos: usize) -> (usize, usize) {
        let mut d = 0;
        for (i, n) in self.lignes.borrow().iter().enumerate() {
            if pos < d + n {
                return (i, pos - d);
            }
            d += n;
        }
        (self.lignes.borrow().len() - 1, 0)
    }
    fn y(&self, ligne: usize) -> f64 {
        let v = match self.repli.get() {
            Some((a, b)) if ligne > b => ligne - (b - a + 1),
            Some((a, _)) if ligne >= a => a,
            _ => ligne,
        };
        v as f64 * LIGNE
    }
    fn pos(&self, ligne: usize, col: usize) -> usize {
        self.debut(ligne) + col
    }
    /// Une ligne de `n` caractères insérée en tête.
    fn inserer_en_tete(&self, n: usize) -> EditorChange {
        self.lignes.borrow_mut().insert(0, n);
        EditorChange::doc(move |p, a| MappedPos {
            pos: if p == 0 && a == Assoc::Before {
                0
            } else {
                p + n
            },
            deleted: false,
        })
    }
    /// Supprime `de..a` (sur une même ligne).
    fn supprimer(&self, de: usize, a: usize) -> EditorChange {
        let (l, _) = self.ligne_de(de);
        self.lignes.borrow_mut()[l] -= a - de;
        EditorChange::doc(move |p, _| {
            if p <= de {
                MappedPos {
                    pos: p,
                    deleted: false,
                }
            } else if p >= a {
                MappedPos {
                    pos: p - (a - de),
                    deleted: false,
                }
            } else {
                MappedPos {
                    pos: de,
                    deleted: true,
                }
            }
        })
    }
}

fn vue_changee() -> EditorChange {
    EditorChange {
        doc_changed: false,
        selection_changed: false,
        viewport_changed: true,
        map_pos: Box::new(|p, _| MappedPos {
            pos: p,
            deleted: false,
        }),
        origin: None,
    }
}

impl DocumentSurface for Surface {
    fn coords_at_pos(&self, offset: usize) -> Option<Rect> {
        let (l, c) = self.ligne_de(offset);
        let (x, y) = (c as f64 * CAR, self.y(l));
        Some(Rect {
            left: x,
            top: y,
            right: x + CAR,
            bottom: y + LIGNE,
        })
    }
    fn pos_at_coords(&self, x: f64, y: f64) -> Option<usize> {
        let o = self.source.origine()?;
        let mut l = ((y - o.y) / LIGNE).floor().max(0.0) as usize;
        if let Some((a, b)) = self.repli.get()
            && l >= a
        {
            l += b - a + 1;
        }
        let l = l.min(self.lignes.borrow().len() - 1);
        let n = self.lignes.borrow()[l];
        let c = ((x - o.x) / CAR).round().clamp(0.0, (n - 1) as f64) as usize;
        Some(self.debut(l) + c)
    }
    fn viewport_range(&self) -> EditorRange {
        EditorRange {
            from: 0,
            to: self.total(),
        }
    }
    fn rendered_ranges(&self) -> Vec<EditorRange> {
        match self.repli.get() {
            Some((a, b)) => vec![
                EditorRange {
                    from: 0,
                    to: self.debut(a) - 1,
                },
                EditorRange {
                    from: self.debut(b + 1),
                    to: self.total(),
                },
            ],
            None => vec![self.viewport_range()],
        }
    }
    fn request_update(&self) {}
}

// ─── Le banc ───

struct Vue {
    couche: Rc<CoucheWidgets>,
    source: Rc<Source>,
    texte: Rc<Cell<u32>>,
}

impl Render for Vue {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let texte = Rc::clone(&self.texte);
        let source: Rc<dyn SourcePlans> = self.source.clone();
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| texte.set(texte.get() + 1)),
            )
            .child(self.couche.element(source))
    }
}

struct Banc {
    couche: Rc<CoucheWidgets>,
    source: Rc<Source>,
    surface: Rc<Surface>,
}

impl Banc {
    fn hote(&self) -> &Rc<HoteCalques> {
        self.couche.hote()
    }
    fn calque(&self) -> CalqueWidgets {
        self.couche.nouveau_calque()
    }
    fn marge(&self, c: &CalqueWidgets, sorte: SorteEssai, pos: usize, h: f64) -> WidgetEssai {
        poser_essai(
            &self.couche,
            c,
            SpecEssai::marge(sorte, pos, None, GutterSide::Right, h),
        )
    }
    /// Le début du « paragraphe » `n` (1 = le premier) : une ligne sur trois (60 px).
    fn p(&self, n: usize) -> usize {
        self.surface.pos(3 * (n - 1), 0)
    }
    fn voulu(&self, w: &WidgetEssai) -> f64 {
        let a = w.handle.get_anchor();
        let dy = match a {
            WidgetAnchor::Gutter { dy, .. } | WidgetAnchor::Document { dy, .. } => dy,
            WidgetAnchor::Viewport { .. } => 0.0,
        };
        self.surface.coords_at_pos(a.pos().unwrap()).unwrap().top + dy
    }
    fn top(&self, w: &WidgetEssai) -> f64 {
        self.hote()
            .fiche(w.element().unwrap())
            .unwrap()
            .top
            .unwrap()
    }
    fn sur(&self, w: &WidgetEssai) -> bool {
        w.element().is_some() && (self.top(w) - self.voulu(w)).abs() < 1.5
    }
    fn visible(&self, w: &WidgetEssai) -> bool {
        w.element()
            .and_then(|el| self.hote().fiche(el))
            .is_some_and(|f| f.dessinee())
    }
    fn rect(&self, w: &WidgetEssai) -> Rect {
        self.hote().client_rect(w.element().unwrap())
    }
    fn hauteur(&self, w: &WidgetEssai) -> f64 {
        self.hote().fiche(w.element().unwrap()).unwrap().taille.1
    }
    fn sans_recouvrement(&self, ws: &[&WidgetEssai]) {
        let vis: Vec<(usize, Rect)> = ws
            .iter()
            .enumerate()
            .filter(|(_, w)| self.visible(w))
            .map(|(i, w)| (i, self.rect(w)))
            .collect();
        for (i, a) in &vis {
            for (j, b) in &vis {
                if i < j {
                    let croise = a.left < b.right
                        && b.left < a.right
                        && a.top < b.bottom
                        && b.top < a.bottom;
                    assert!(!croise, "{i} recouvre {j} : {a:?} {b:?}");
                }
            }
        }
    }
    fn montes(&self, ws: &[&WidgetEssai]) -> usize {
        ws.iter().filter(|w| w.element().is_some()).count()
    }
}

fn banc(cx: &mut TestAppContext) -> (Banc, &mut VisualTestContext) {
    let source = Rc::new(Source {
        defile: Cell::new(0.0),
        colonne: Cell::new((200.0, 800.0)),
    });
    let surface = Rc::new(Surface {
        source: Rc::clone(&source),
        lignes: RefCell::new(vec![LONGUEUR; NB_LIGNES]),
        repli: Cell::new(None),
    });
    let ed: Rc<dyn DocumentSurface> = surface.clone();
    let couche = CoucheWidgets::new(HoteCalques::pour_editeur(), Some(ed));
    let (c, s) = (Rc::clone(&couche), Rc::clone(&source));
    let (_vue, vcx) = cx.add_window_view(move |_, _| Vue {
        couche: c,
        source: s,
        texte: Rc::new(Cell::new(0)),
    });
    vcx.simulate_resize(size(px(1000.0), px(HAUT_PANE as f32)));
    images(vcx);
    (
        Banc {
            couche,
            source,
            surface,
        },
        vcx,
    )
}

fn image(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vcx.run_until_parked();
}

/// « Une image complète » : quelques images, le temps que tout converge.
fn images(vcx: &mut VisualTestContext) {
    for _ in 0..6 {
        image(vcx);
    }
}

fn cliquer(vcx: &mut VisualTestContext, x: f64, y: f64) {
    vcx.simulate_click(point(px(x as f32), px(y as f32)), Modifiers::default());
    images(vcx);
}

fn cliquer_widget(b: &Banc, vcx: &mut VisualTestContext, w: &WidgetEssai) {
    let r = b.rect(w);
    cliquer(vcx, (r.left + r.right) / 2.0, (r.top + r.bottom) / 2.0);
}

/// Un clic dans le TEXTE, focus rendu : l'utilisateur « reprend la main ».
fn reprendre_la_main(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| w.blur(cx));
    cliquer(vcx, 500.0, 30.0);
}

fn focaliser(vcx: &mut VisualTestContext, w: &WidgetEssai) {
    let h = w.poignee_focus().expect("dessiné une fois");
    vcx.update(|win, cx| win.focus(&h, cx));
    images(vcx);
}

fn defiler(b: &Banc, vcx: &mut VisualTestContext, y: f64) {
    b.source.defile.set(y);
    images(vcx);
}

const BAS: f64 = NB_LIGNES as f64 * LIGNE - HAUT_PANE;

// ─── viewport.spec ───

#[test]
fn seule_la_conversation_du_viewport_est_montee_defiler_echange() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let haut = b.marge(&c, SorteEssai::Vue, b.surface.pos(2, 0), 100.0);
    let bas = b.marge(&c, SorteEssai::Vue, b.surface.pos(790, 0), 100.0);
    images(vcx);
    assert!(haut.element().is_some() && bas.element().is_none());
    defiler(&b, vcx, BAS);
    assert!(haut.element().is_none() && bas.element().is_some());
    assert!(b.visible(&bas));
    // Revenue dans la bande, c'est une instance NEUVE (le handle, lui, a survécu).
    defiler(&b, vcx, 0.0);
    assert!(haut.element().is_some());
    assert_eq!(haut.declare.constructions(), 2);
}

#[test]
fn la_bande_ne_change_pas_a_chaque_image_seulement_en_approchant_d_un_bord_hysteresis_de_cm6() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let n = Rc::new(Cell::new(0));
    let a = Rc::clone(&n);
    let _off = b
        .hote()
        .on_viewport_change(Box::new(move || a.set(a.get() + 1)));
    for i in 1..=40 {
        b.source.defile.set(f64::from(i) * 10.0);
        image(vcx);
    }
    // 400 px : moins que la demi-marge → aucun changement.
    assert_eq!(n.get(), 0);
    for i in 1..=40 {
        b.source.defile.set(400.0 + f64::from(i) * 100.0);
        image(vcx);
    }
    assert!(n.get() > 0 && n.get() < 40, "{}", n.get());
}

#[test]
fn la_conversation_focalisee_reste_construite_hors_du_viewport_relachee_elle_est_detruite() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let haut = b.marge(&c, SorteEssai::Vue, b.surface.pos(2, 0), 100.0);
    let bas = b.marge(&c, SorteEssai::Vue, b.surface.pos(790, 0), 100.0);
    images(vcx);
    cliquer_widget(&b, vcx, &haut);
    assert_eq!(b.couche.focalise(), haut.element());
    defiler(&b, vcx, BAS);
    assert_eq!(b.montes(&[&haut, &bas]), 2);
    reprendre_la_main(vcx);
    assert_eq!(b.montes(&[&haut, &bas]), 1);
    assert!(haut.element().is_none());
}

#[test]
fn aller_a_d_une_conversation_non_montee_on_y_va_elle_se_monte_elle_est_active() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let haut = b.marge(&c, SorteEssai::Vue, b.surface.pos(2, 0), 100.0);
    defiler(&b, vcx, BAS);
    assert!(haut.element().is_none());
    // « Aller à » : on y défile, et l'activation attend la construction.
    haut.declare
        .au_prochain_montage(&haut.handle, |_, h| h.set_active(true));
    defiler(&b, vcx, 0.0);
    let el = haut.element().expect("monté");
    assert_eq!(b.hote().activation().active_el(), Some(el));
    assert!(haut.handle.is_active());
}

#[test]
fn creee_hors_de_l_ecran_construite_plus_tard_elle_ne_vole_ni_le_focus_ni_le_defilement() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let tardive = b.marge(&c, SorteEssai::Vue, b.surface.pos(400, 0), 100.0);
    images(vcx);
    assert!(tardive.element().is_none());
    reprendre_la_main(vcx);
    let cible = 400.0 * LIGNE - 100.0;
    defiler(&b, vcx, cible);
    assert!(tardive.element().is_some());
    assert_eq!(b.couche.focalise(), None);
    assert!(!tardive.handle.is_active());
    assert_eq!(b.source.defile.get(), cible);
}

#[test]
fn rouvrir_la_note_seules_celles_du_viewport_se_montent() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let _a = b.marge(&c, SorteEssai::Vue, b.surface.pos(2, 0), 100.0);
    images(vcx);
    c.fermer();
    images(vcx);
    assert!(b.hote().a_dessiner().is_empty());
    let c = b.calque();
    let haut = b.marge(&c, SorteEssai::Vue, b.surface.pos(2, 0), 100.0);
    let bas = b.marge(&c, SorteEssai::Vue, b.surface.pos(790, 0), 100.0);
    images(vcx);
    assert_eq!(b.montes(&[&haut, &bas]), 1);
    assert!(haut.element().is_some());
}

// ─── gutter-arbiter.spec ───

struct Arbitre {
    globes: CalqueWidgets,
    cartes: CalqueWidgets,
    globe: WidgetEssai,
}

fn arbitre(b: &Banc, vcx: &mut VisualTestContext) -> Arbitre {
    let globes = b.calque();
    let cartes = b.calque();
    let globe = b.marge(&globes, SorteEssai::Rigide, b.surface.pos(10, 5), 120.0);
    images(vcx);
    Arbitre {
        globes,
        cartes,
        globe,
    }
}

#[test]
fn seul_dans_sa_marge_le_globe_est_pile_sur_son_ancre() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    assert!(b.sur(&a.globe));
    assert!(b.visible(&a.globe));
}

#[test]
fn une_carte_active_on_y_tape_reste_sur_son_ancre_le_globe_d_un_autre_calque_s_ecarte() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    let carte = b.marge(&a.cartes, SorteEssai::Vue, b.surface.pos(10, 0), 100.0);
    images(vcx);
    focaliser(vcx, &carte);
    assert!(carte.handle.is_active());
    assert!(b.sur(&carte), "carte active sur son ancre");
    assert!(!b.sur(&a.globe));
    let (c, g) = (b.rect(&carte), b.rect(&a.globe));
    assert!(g.top - c.bottom < 12.0 && g.top >= c.bottom);
    b.sans_recouvrement(&[&carte, &a.globe]);
}

#[test]
fn cliquer_le_globe_le_rend_actif_il_revient_sur_son_ancre_la_carte_s_ecarte_sans_perdre_le_clic() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    let carte = b.marge(&a.cartes, SorteEssai::Vue, b.surface.pos(10, 0), 100.0);
    images(vcx);
    focaliser(vcx, &carte);
    assert!(!b.sur(&a.globe));
    cliquer_widget(&b, vcx, &a.globe);
    assert!(b.sur(&a.globe));
    assert_eq!(a.globe.clics(), 1, "le clic n'est pas perdu");
    let (c, g) = (b.rect(&carte), b.rect(&a.globe));
    assert!(c.bottom <= g.top && g.top - c.bottom < 12.0);
    cliquer_widget(&b, vcx, &carte);
    assert!(b.sur(&carte));
    assert_eq!(carte.clics(), 1);
    b.sans_recouvrement(&[&carte, &a.globe]);
}

#[test]
fn decocher_le_calque_du_globe_la_carte_revient_sur_son_ancre_le_recocher_aucun_recouvrement() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    let carte = b.marge(&a.cartes, SorteEssai::Vue, b.surface.pos(10, 0), 100.0);
    images(vcx);
    cliquer_widget(&b, vcx, &a.globe);
    assert!(!b.sur(&carte));
    a.globes.fermer();
    images(vcx);
    assert!(a.globe.element().is_none());
    assert!(b.sur(&carte));
    let globes = b.calque();
    let globe = b.marge(&globes, SorteEssai::Rigide, b.surface.pos(10, 5), 120.0);
    images(vcx);
    assert!(b.visible(&globe) && b.visible(&carte));
    b.sans_recouvrement(&[&carte, &globe]);
}

#[test]
fn decocher_le_calque_des_cartes_elles_disparaissent_le_globe_revient_sur_son_ancre_recocher_elles_reviennent()
 {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    let carte = b.marge(&a.cartes, SorteEssai::Vue, b.surface.pos(10, 0), 100.0);
    images(vcx);
    focaliser(vcx, &carte);
    assert!(!b.sur(&a.globe));
    a.cartes.fermer();
    images(vcx);
    assert!(carte.element().is_none());
    assert!(b.sur(&a.globe));
    let cartes = b.calque();
    let carte = b.marge(&cartes, SorteEssai::Vue, b.surface.pos(10, 0), 100.0);
    images(vcx);
    assert!(b.visible(&carte));
    b.sans_recouvrement(&[&carte, &a.globe]);
}

#[test]
fn une_carte_qui_grandit_pousse_ses_voisines_jamais_de_recouvrement_en_cours_de_route() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    let carte = b.marge(&a.cartes, SorteEssai::Vue, b.surface.pos(8, 0), 30.0);
    images(vcx);
    for i in 1..=15 {
        carte.poser_hauteur(30.0 + 20.0 * f64::from(i));
        // UNE image : ce qui est peint ne se recouvre jamais.
        image(vcx);
        assert_eq!(b.hauteur(&carte), carte.hauteur());
        b.sans_recouvrement(&[&carte, &a.globe]);
    }
    images(vcx);
    b.sans_recouvrement(&[&carte, &a.globe]);
}

#[test]
fn trois_cartes_et_le_globe_ordre_du_texte_garde_aucun_recouvrement() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    let cartes: Vec<WidgetEssai> = [12, 8, 10]
        .into_iter()
        .map(|l| b.marge(&a.cartes, SorteEssai::Vue, b.surface.pos(l, 0), 100.0))
        .collect();
    images(vcx);
    let tous: Vec<&WidgetEssai> = cartes.iter().chain([&a.globe]).collect();
    b.sans_recouvrement(&tous);
    let mut par_voulu: Vec<usize> = (0..tous.len()).collect();
    par_voulu.sort_by(|&i, &j| b.voulu(tous[i]).total_cmp(&b.voulu(tous[j])));
    let mut par_top: Vec<usize> = (0..tous.len()).collect();
    par_top.sort_by(|&i, &j| b.top(tous[i]).total_cmp(&b.top(tous[j])));
    assert_eq!(par_top, par_voulu);
}

#[test]
fn marge_trop_etroite_tout_se_masque_elargie_tout_revient_sans_recouvrement() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let a = arbitre(&b, vcx);
    let carte = b.marge(&a.cartes, SorteEssai::Vue, b.surface.pos(10, 0), 100.0);
    images(vcx);
    b.source.colonne.set((100.0, 950.0));
    images(vcx);
    assert!(!b.visible(&carte) && !b.visible(&a.globe));
    b.source.colonne.set((200.0, 800.0));
    images(vcx);
    assert!(b.visible(&carte) && b.visible(&a.globe));
    b.sans_recouvrement(&[&carte, &a.globe]);
}

// ─── gutter-room.spec ───

#[test]
fn ancres_eloignees_place_ecart_jusqu_a_l_ancre_suivante_moins_8_le_dernier_illimitee_pas_de_variable()
 {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let ws: Vec<WidgetEssai> = [1, 5, 9]
        .into_iter()
        .map(|n| b.marge(&c, SorteEssai::Flex, b.p(n), 30.0))
        .collect();
    images(vcx);
    let room = |i: usize| ws[i].handle.get_room().unwrap();
    assert!((room(0) - (b.voulu(&ws[1]) - b.voulu(&ws[0]) - 8.0)).abs() < 0.5);
    assert!((room(1) - (b.voulu(&ws[2]) - b.voulu(&ws[1]) - 8.0)).abs() < 0.5);
    assert_eq!(room(2), f64::INFINITY);
    for w in &ws {
        assert!(b.sur(w));
        assert_eq!(b.hauteur(w), 30.0);
    }
    // La place est posée sur l'élément, sauf illimitée.
    let place = |i: usize| b.hote().fiche(ws[i].element().unwrap()).unwrap().place;
    assert!(place(0).is_some());
    assert_eq!(place(2), None);
    b.sans_recouvrement(&ws.iter().collect::<Vec<_>>());
}

#[test]
fn ancres_proches_chaque_widget_seuil_se_compacte_selon_sa_place_tous_sur_leur_ancre_sans_recouvrement()
 {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let ws: Vec<WidgetEssai> = [1, 2, 3]
        .into_iter()
        .map(|n| b.marge(&c, SorteEssai::Seuil, b.p(n), 200.0))
        .collect();
    images(vcx);
    for w in &ws {
        let room = w.handle.get_room();
        assert_eq!(
            w.compact(),
            room.is_some_and(|r| r < 80.0),
            "place {room:?}"
        );
    }
    assert!(ws[0].compact());
    assert!(!ws[2].compact());
    b.sans_recouvrement(&ws.iter().collect::<Vec<_>>());
}

#[test]
fn l_actif_sa_place_la_hauteur_visible_il_est_complet_le_suivant_pousse_voit_sa_place_reduite() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let ws: Vec<WidgetEssai> = [1, 2, 3]
        .into_iter()
        .map(|n| b.marge(&c, SorteEssai::Seuil, b.p(n), 200.0))
        .collect();
    images(vcx);
    cliquer_widget(&b, vcx, &ws[0]);
    assert!(!ws[0].compact());
    assert!((ws[0].handle.get_room().unwrap() - (HAUT_PANE - 16.0)).abs() < 0.5);
    assert_eq!(b.hauteur(&ws[0]), 200.0);
    assert!(b.sur(&ws[0]));
    let attendu = (b.voulu(&ws[2]) - (b.voulu(&ws[0]) + 200.0 + 8.0) - 8.0).max(0.0);
    let r1 = ws[1].handle.get_room().unwrap();
    assert!((r1 - attendu).abs() < 0.5, "{r1} ≠ {attendu}");
    assert_eq!(ws[1].compact(), r1 < 80.0);
    b.sans_recouvrement(&ws.iter().collect::<Vec<_>>());
    // Le suivant devient actif : l'inverse.
    cliquer_widget(&b, vcx, &ws[1]);
    assert!(!ws[1].compact());
    assert!(b.sur(&ws[1]));
    assert!(ws[0].compact());
    b.sans_recouvrement(&ws.iter().collect::<Vec<_>>());
}

#[test]
fn un_rigide_d_un_autre_calque_reduit_la_place_du_suivant_decoche_elle_revient() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let seuil = b.marge(&c, SorteEssai::Seuil, b.surface.pos(2, 0), 200.0);
    let flex = b.marge(&c, SorteEssai::Flex, b.surface.pos(30, 0), 20.0);
    images(vcx);
    let sans = seuil.handle.get_room().unwrap();
    assert!(!seuil.compact());
    let globes = b.calque();
    let globe = b.marge(&globes, SorteEssai::Rigide, 0, 200.0);
    images(vcx);
    let avec = seuil.handle.get_room().unwrap();
    assert!(avec < sans);
    // La place se calcule en POUSSANT depuis les ancres (le globe à la sienne).
    let attendu = (b.voulu(&flex)
        - b.voulu(&seuil)
            .max(b.voulu(&globe) + b.hauteur(&globe) + 8.0)
        - 8.0)
        .max(0.0);
    assert!((avec - attendu).abs() < 0.5, "{avec} ≠ {attendu}");
    b.sans_recouvrement(&[&seuil, &flex, &globe]);
    globes.fermer();
    images(vcx);
    assert!((seuil.handle.get_room().unwrap() - sans).abs() < 0.5);
    assert!(!seuil.compact());
}

#[test]
fn stabilite_une_fois_places_plus_aucune_annonce_de_place_pas_d_oscillation() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let ws: Vec<WidgetEssai> = [1, 2, 3, 4]
        .into_iter()
        .map(|n| b.marge(&c, SorteEssai::Seuil, b.p(n), 200.0))
        .collect();
    images(vcx);
    let avant: u32 = ws.iter().map(WidgetEssai::annonces).sum();
    for _ in 0..10 {
        image(vcx);
    }
    assert_eq!(ws.iter().map(WidgetEssai::annonces).sum::<u32>(), avant);
}

#[test]
fn marge_trop_etroite_masques_place_null_elargie_la_place_revient() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let ws: Vec<WidgetEssai> = [1, 5]
        .into_iter()
        .map(|n| b.marge(&c, SorteEssai::Flex, b.p(n), 30.0))
        .collect();
    images(vcx);
    b.source.colonne.set((100.0, 950.0));
    images(vcx);
    assert_eq!(
        ws.iter().map(|w| w.handle.get_room()).collect::<Vec<_>>(),
        [None, None]
    );
    b.source.colonne.set((200.0, 800.0));
    images(vcx);
    assert!(ws[0].handle.get_room().unwrap() > 0.0);
    assert_eq!(ws[1].handle.get_room(), Some(f64::INFINITY));
}

// ─── widget-anchor.spec ───

/// Un widget posé à l'écran (ancre viewport) au milieu du pane, sur la ligne 12.
fn widget_libre(b: &Banc, c: &CalqueWidgets, vcx: &mut VisualTestContext) -> WidgetEssai {
    let w = poser_essai(
        &b.couche,
        c,
        SpecEssai {
            sorte: SorteEssai::Vue,
            ancre: WidgetAnchor::viewport(500.0, 240.0),
            hauteur: 80.0,
        },
    );
    images(vcx);
    w
}

/// `anchorInDocument` : l'ancre document qui laisse le widget où il est.
fn ancrer(b: &Banc, c: &CalqueWidgets, w: &WidgetEssai, vcx: &mut VisualTestContext) {
    let r = b.rect(w);
    let a = c.layer().document_anchor_at(r.left, r.top).unwrap();
    w.handle.set_anchor(a);
    images(vcx);
}

/// Le coin client du glyphe d'une position.
fn glyphe(b: &Banc, pos: usize) -> (f64, f64) {
    let r = b.surface.coords_at_pos(pos).unwrap();
    let o = b.source.origine().unwrap();
    (o.x + r.left, o.y + r.top)
}

#[test]
fn ancrer_ne_deplace_pas_le_widget() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    let avant = b.rect(&w);
    ancrer(&b, &c, &w, vcx);
    assert!(matches!(
        w.handle.get_anchor(),
        WidgetAnchor::Document { .. }
    ));
    let apres = b.rect(&w);
    assert!((apres.left - avant.left).abs() < 2.0 && (apres.top - avant.top).abs() < 2.0);
    // Et GPUI l'a bien posé là.
    let p = w.pose().unwrap();
    assert!((f64::from(f32::from(p.origin.x)) - apres.left).abs() < 1.0);
}

#[test]
fn le_widget_suit_son_texte_quand_on_edite_au_dessus() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    ancrer(&b, &c, &w, vcx);
    let pos0 = w.handle.get_anchor().pos().unwrap();
    let (w0, g0) = (b.rect(&w), glyphe(&b, pos0));
    b.couche.relayer(&b.surface.inserer_en_tete(18));
    images(vcx);
    let pos1 = w.handle.get_anchor().pos().unwrap();
    assert_eq!(pos1, pos0 + 18);
    let (w1, g1) = (b.rect(&w), glyphe(&b, pos1));
    assert!((g1.1 - g0.1).abs() > 5.0);
    assert!((w1.left - w0.left - (g1.0 - g0.0)).abs() < 2.0);
    assert!((w1.top - w0.top - (g1.1 - g0.1)).abs() < 2.0);
}

#[test]
fn l_ancre_est_unique_le_widget_lit_celle_du_layer_il_n_en_garde_pas_de_copie() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    ancrer(&b, &c, &w, vcx);
    let pos0 = w.handle.get_anchor().pos().unwrap();
    b.couche.relayer(&b.surface.inserer_en_tete(18));
    images(vcx);
    let avant = b.rect(&w);
    // Un glisser de 30 px : on part de l'ancre COURANTE (remappée), lue au handle.
    if let WidgetAnchor::Document { pos, to, dx, dy } = w.handle.get_anchor() {
        w.handle.set_anchor(WidgetAnchor::Document {
            pos,
            to,
            dx: dx + 30.0,
            dy,
        });
    }
    images(vcx);
    assert_eq!(w.handle.get_anchor().pos(), Some(pos0 + 18));
    assert!((b.rect(&w).left - avant.left - 30.0).abs() < 2.0);
}

#[test]
fn une_edition_faite_hors_du_viewport_rendu_est_rattrapee_au_retour_a_l_ecran() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    ancrer(&b, &c, &w, vcx);
    let pos0 = w.handle.get_anchor().pos().unwrap();
    let (w0, g0) = (b.rect(&w), glyphe(&b, pos0));
    defiler(&b, vcx, BAS);
    assert!(w.element().is_none(), "hors de la bande : détruit");
    b.couche.relayer(&b.surface.inserer_en_tete(14));
    images(vcx);
    defiler(&b, vcx, 0.0);
    assert_eq!(w.declare.constructions(), 2, "remonté : une instance neuve");
    let pos1 = w.handle.get_anchor().pos().unwrap();
    assert_eq!(pos1, pos0 + 14);
    let (w1, g1) = (b.rect(&w), glyphe(&b, pos1));
    assert!((g1.1 - g0.1).abs() > 5.0);
    assert!((w1.left - w0.left - (g1.0 - g0.0)).abs() < 2.0);
    assert!((w1.top - w0.top - (g1.1 - g0.1)).abs() < 2.0);
}

#[test]
fn un_redimensionnement_de_la_fenetre_ne_decroche_pas_le_widget_de_son_texte() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    ancrer(&b, &c, &w, vcx);
    let pos = w.handle.get_anchor().pos().unwrap();
    let (w0, g0) = (b.rect(&w), glyphe(&b, pos));
    // La fenêtre s'élargit : la colonne (même largeur) se recentre.
    vcx.simulate_resize(size(px(1400.0), px(HAUT_PANE as f32)));
    b.source.colonne.set((400.0, 1000.0));
    images(vcx);
    let (w1, g1) = (b.rect(&w), glyphe(&b, pos));
    assert!((g1.0 - g0.0).abs() > 50.0);
    assert!((w1.left - w0.left - (g1.0 - g0.0)).abs() < 2.0);
    assert!((w1.top - w0.top - (g1.1 - g0.1)).abs() < 2.0);
}

fn placer_dans_la_marge(side: GutterSide) {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    ancrer(&b, &c, &w, vcx);
    let avant = b.rect(&w).top;
    let a = c.layer().gutter_anchor_at(avant, side).unwrap();
    w.handle.set_anchor(a);
    images(vcx);
    assert!(matches!(w.handle.get_anchor(), WidgetAnchor::Gutter { side: s, .. } if s == side));
    let r = b.rect(&w);
    assert!((r.top - avant).abs() < 2.0);
    let (col_g, col_d) = b.source.colonne.get();
    match side {
        GutterSide::Left => assert!(r.left >= -1.0 && r.right <= col_g + 1.0, "{r:?}"),
        GutterSide::Right => assert!(r.left >= col_d - 1.0 && r.right <= 1001.0, "{r:?}"),
    }
    assert!(r.right - r.left > 60.0);
}

#[test]
fn placer_dans_la_marge_gauche_garde_le_widget_au_meme_niveau() {
    placer_dans_la_marge(GutterSide::Left);
}

#[test]
fn placer_dans_la_marge_droite_garde_le_widget_au_meme_niveau() {
    placer_dans_la_marge(GutterSide::Right);
}

/// Un widget de marge droite à la ligne 12.
fn widget_de_marge(b: &Banc, c: &CalqueWidgets, vcx: &mut VisualTestContext) -> WidgetEssai {
    let w = b.marge(c, SorteEssai::Vue, b.surface.pos(12, 0), 80.0);
    images(vcx);
    w
}

#[test]
fn un_widget_de_marge_suit_le_texte_quand_on_edite_au_dessus() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_de_marge(&b, &c, vcx);
    let pos0 = w.handle.get_anchor().pos().unwrap();
    let (w0, g0) = (b.rect(&w), glyphe(&b, pos0));
    b.couche.relayer(&b.surface.inserer_en_tete(18));
    images(vcx);
    let (w1, g1) = (b.rect(&w), glyphe(&b, pos0 + 18));
    assert!((g1.1 - g0.1).abs() > 5.0);
    assert!((w1.top - w0.top - (g1.1 - g0.1)).abs() < 2.0);
    assert!((w1.left - w0.left).abs() < 1.0);
}

#[test]
fn un_widget_de_marge_se_redimensionne_avec_la_marge_et_n_en_sort_jamais() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_de_marge(&b, &c, vcx);
    let large = b.rect(&w);
    b.source.colonne.set((200.0, 860.0));
    images(vcx);
    let etroit = b.rect(&w);
    assert!((large.right - large.left) - (etroit.right - etroit.left) > 50.0);
    assert!(etroit.left >= 860.0 - 1.0 && etroit.right <= 1001.0);
    // Et GPUI l'a posé à cette largeur.
    let p = w.pose().unwrap();
    assert!((f64::from(f32::from(p.size.width)) - (etroit.right - etroit.left)).abs() < 1.0);
}

#[test]
fn marge_trop_etroite_le_widget_se_masque_plutot_que_de_mordre_le_texte() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_de_marge(&b, &c, vcx);
    assert!(b.visible(&w));
    b.source.colonne.set((200.0, 950.0));
    images(vcx);
    assert!(!b.visible(&w));
    w.oublier_pose();
    image(vcx);
    assert!(w.pose().is_none(), "masqué : pas peint");
    b.source.colonne.set((200.0, 800.0));
    images(vcx);
    assert!(b.visible(&w));
    let r = b.rect(&w);
    assert!(r.left >= 799.0 && r.right <= 1001.0);
}

#[test]
fn texte_ancre_supprime_le_widget_se_detache_sur_place_sans_se_teleporter() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    ancrer(&b, &c, &w, vcx);
    let pos = w.handle.get_anchor().pos().unwrap();
    let avant = b.rect(&w);
    b.couche.relayer(&b.surface.supprimer(pos - 5, pos + 5));
    images(vcx);
    assert!(matches!(
        w.handle.get_anchor(),
        WidgetAnchor::Viewport { .. }
    ));
    let apres = b.rect(&w);
    assert!((apres.left - avant.left).abs() < 3.0 && (apres.top - avant.top).abs() < 3.0);
}

#[test]
fn texte_ancre_replie_le_widget_se_masque_et_revient_a_sa_place_au_depli() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_libre(&b, &c, vcx);
    ancrer(&b, &c, &w, vcx);
    let avant = b.rect(&w);
    b.surface.repli.set(Some((5, 15)));
    b.couche.relayer(&vue_changee());
    images(vcx);
    assert!(w.element().is_none(), "replié : détruit");
    assert!(matches!(
        w.handle.get_anchor(),
        WidgetAnchor::Document { .. }
    ));
    b.surface.repli.set(None);
    b.couche.relayer(&vue_changee());
    images(vcx);
    assert!(b.visible(&w));
    let apres = b.rect(&w);
    assert!((apres.left - avant.left).abs() < 2.0 && (apres.top - avant.top).abs() < 2.0);
}

// ─── La présentation (sans spec) ───

#[test]
fn un_appui_dans_un_menu_n_est_pas_dehors_l_actif_reste() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_de_marge(&b, &c, vcx);
    cliquer_widget(&b, vcx, &w);
    assert!(w.handle.is_active());
    let menu = Rc::new(Cell::new(true));
    let m = Rc::clone(&menu);
    b.couche.poser_ephemere(move || m.get());
    cliquer(vcx, 500.0, 30.0);
    assert!(w.handle.is_active(), "dans un menu : rien ne change");
    menu.set(false);
    cliquer(vcx, 500.0, 30.0);
    assert!(!w.handle.is_active(), "dehors : relâché");
}

#[test]
fn le_clic_droit_dehors_ne_relache_pas_l_actif() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_de_marge(&b, &c, vcx);
    cliquer_widget(&b, vcx, &w);
    assert!(w.handle.is_active());
    // Un clic droit dans le texte (menu contextuel) : rien ne change.
    let p = point(px(500.0), px(30.0));
    vcx.simulate_mouse_down(p, MouseButton::Right, Modifiers::default());
    vcx.simulate_mouse_up(p, MouseButton::Right, Modifiers::default());
    images(vcx);
    assert!(w.handle.is_active());
}

#[test]
fn le_widget_actif_est_marque_et_l_actif_ne_s_active_qu_apres_le_geste() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let c = b.calque();
    let w = widget_de_marge(&b, &c, vcx);
    let r = b.rect(&w);
    let p = point(
        px(((r.left + r.right) / 2.0) as f32),
        px((r.top + 10.0) as f32),
    );
    vcx.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
    images(vcx);
    assert!(!w.handle.is_active(), "jamais à l'appui");
    vcx.simulate_mouse_up(p, MouseButton::Left, Modifiers::default());
    images(vcx);
    assert!(w.handle.is_active());
    assert_eq!(w.clics(), 1);
}
