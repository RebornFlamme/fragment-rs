//! Tests de l'hôte des calques (tâche `fr-t5-hote`), dans une fenêtre de test GPUI (sans GPU), sur
//! une FAUSSE surface : un pane qui capte les clics, un contenu « défilé » d'un décalage réglable
//! et un zoom réglable. Pas de fichier de tests TS pour `ViewOverlays.ts` : les cas suivent le
//! contrat d'`OverlayHost` et les specs e2e `viewport` (bande stable aux petits pas, zoom) et
//! `reader-pdf` (« zoom : le widget de marge garde SA taille »). Les boîtes vérifiées sont celles
//! que GPUI a VRAIMENT posées (relevées par un `canvas` dans le contenu), pas celles du modèle.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_layers::overlay_host::{OverlayHost, WidgetElements};
use fragment_layers::types::{ElementId, GutterSide, OverlayPlane, Point};
use fragment_text::Rect;
use fragment_ui::calques::{HoteCalques, SourcePlans, plans_calques};
use gpui_kit::{
    Bounds, Context, InteractiveElement as _, IntoElement, Modifiers, MouseButton,
    ParentElement as _, Pixels, Render, Styled as _, TestAppContext, VisualTestContext, Window,
    canvas, div, point, px, size,
};

// ─── La fausse surface ───

/// Le contenu défile de `defile` px ; la colonne de lecture va de 200 à 800 ; zoom `echelle`.
struct FausseSource {
    defile: Cell<f64>,
    echelle: Cell<f64>,
    decalage_colonne: Cell<f64>,
}

impl SourcePlans for FausseSource {
    fn origine(&self) -> Option<Point> {
        Some(Point {
            x: 200.0 + self.decalage_colonne.get(),
            y: -self.defile.get(),
        })
    }
    fn colonne(&self) -> Option<Rect> {
        let d = self.decalage_colonne.get();
        Some(Rect {
            left: 200.0 + d,
            top: -self.defile.get(),
            right: 800.0 + d,
            bottom: 3000.0 - self.defile.get(),
        })
    }
    fn echelle(&self) -> f64 {
        self.echelle.get()
    }
}

struct FausseVue {
    hote: Rc<HoteCalques>,
    source: Rc<FausseSource>,
    clics: Rc<Cell<u32>>,
}

impl Render for FausseVue {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let clics = Rc::clone(&self.clics);
        let source: Rc<dyn SourcePlans> = self.source.clone();
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| clics.set(clics.get() + 1)),
            )
            .child(plans_calques(Rc::clone(&self.hote), source))
    }
}

struct Banc {
    hote: Rc<HoteCalques>,
    source: Rc<FausseSource>,
    clics: Rc<Cell<u32>>,
}

fn banc(cx: &mut TestAppContext, plan_document: bool) -> (Banc, &mut VisualTestContext) {
    let hote = if plan_document {
        HoteCalques::pour_editeur()
    } else {
        HoteCalques::new(false, Box::new(|_| 12.0))
    };
    let source = Rc::new(FausseSource {
        defile: Cell::new(0.0),
        echelle: Cell::new(1.0),
        decalage_colonne: Cell::new(0.0),
    });
    let clics = Rc::new(Cell::new(0));
    let (h, s, c) = (Rc::clone(&hote), Rc::clone(&source), Rc::clone(&clics));
    let (_vue, vcx) = cx.add_window_view(move |_, _| FausseVue {
        hote: h,
        source: s,
        clics: c,
    });
    vcx.simulate_resize(size(px(1000.0), px(600.0)));
    image(vcx);
    (
        Banc {
            hote,
            source,
            clics,
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

/// Un élément de 100 × 30 qui relève la boîte où GPUI l'a posé.
fn element(hote: &HoteCalques) -> (ElementId, Rc<RefCell<Option<Bounds<Pixels>>>>) {
    let el = hote.nouvel_element();
    let releve: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));
    let r = Rc::clone(&releve);
    hote.poser_contenu(
        el,
        Rc::new(move |_, _| {
            let r = Rc::clone(&r);
            div()
                .relative()
                .w(px(100.0))
                .h(px(30.0))
                .child(
                    canvas(move |b, _, _| *r.borrow_mut() = Some(b), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .into_any_element()
        }),
    );
    (el, releve)
}

fn coin(releve: &Rc<RefCell<Option<Bounds<Pixels>>>>) -> (f32, f32) {
    let b = releve.borrow().expect("posé");
    (f32::from(b.origin.x), f32::from(b.origin.y))
}

// ─── Les cas ───

#[test]
fn les_conversions_client_document_font_l_aller_retour_a_toute_echelle() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    for (e, defile) in [(1.0, 0.0), (2.0, 300.0), (0.5, 40.0)] {
        b.source.echelle.set(e);
        b.source.defile.set(defile);
        image(vcx);
        let d = b.hote.client_to_document(500.0, 250.0).unwrap();
        let c = b.hote.document_to_client(d.x, d.y).unwrap();
        assert!((c.x - 500.0).abs() < 1e-9 && (c.y - 250.0).abs() < 1e-9);
        assert_eq!(d.y, (250.0 + defile) / e);
    }
    // Le plan viewport : le coin du pane, rien dehors.
    assert_eq!(
        b.hote.client_to_viewport(10.0, 20.0),
        Some(Point { x: 10.0, y: 20.0 })
    );
    assert_eq!(b.hote.client_to_viewport(1001.0, 20.0), None);
}

#[test]
fn un_element_du_plan_document_suit_le_texte_au_defilement_et_au_zoom() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let (el, releve) = element(&b.hote);
    let _off = b.hote.mount(el, OverlayPlane::Document);
    b.hote.attach(el, false);
    b.hote.set_left(el, 10.0);
    b.hote.set_top(el, 500.0);
    image(vcx);
    assert_eq!(coin(&releve), (210.0, 500.0));
    // Le contenu défile : l'élément suit, sans qu'on réécrive rien.
    b.source.defile.set(300.0);
    image(vcx);
    assert_eq!(coin(&releve), (210.0, 200.0));
    // Le zoom : unités du plan × 2.
    b.source.echelle.set(2.0);
    image(vcx);
    assert_eq!(coin(&releve), (220.0, 700.0));
    // La boîte rendue au modèle est celle que GPUI a posée.
    let r = b.hote.client_rect(el);
    assert_eq!(
        (r.left, r.top, r.right, r.bottom),
        (220.0, 700.0, 320.0, 730.0)
    );
    assert_eq!(b.hote.offset_box(el).height, 30.0);
}

#[test]
fn zoom_le_widget_de_marge_garde_sa_taille_et_reste_a_l_ecart_de_la_colonne() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let (el, releve) = element(&b.hote);
    let _off = b.hote.mount(el, OverlayPlane::Document);
    b.source.echelle.set(2.0);
    image(vcx);
    let slot = b.hote.gutter_slot(el, GutterSide::Right);
    let bande = b.hote.gutter_band(GutterSide::Right).unwrap();
    b.hote.set_left(el, bande.left);
    // Le niveau voulu arrive en unités du plan : 100 × 2 = 200 px écran sous l'origine.
    slot.set_wanted(Some(100.0));
    image(vcx);
    let (x, y) = coin(&releve);
    // La bande commence à l'écart de la colonne (bord droit 800), en px écran (GPUI arrondit le
    // coin au pixel de l'appareil).
    assert!(
        (f64::from(x) - (200.0 + bande.left)).abs() <= 0.5,
        "x={x} bande={bande:?} fiche={:?}",
        b.hote.fiche(el).map(|f| (f.racine, f.left, f.top))
    );
    assert!(f64::from(x) > 800.0);
    assert_eq!(y, 200.0);
    // Et sa taille est celle d'une note, pas zoomée.
    let r = releve.borrow().unwrap();
    assert_eq!(
        (f32::from(r.size.width), f32::from(r.size.height)),
        (100.0, 30.0)
    );
    // Le slot rendu : l'élément revient au plan document (unités zoomées).
    slot.remove();
    image(vcx);
    assert_eq!(
        b.hote.fiche(el).unwrap().racine,
        Some(fragment_layers::view_overlays::Racine::Document)
    );
}

#[test]
fn ancre_viewport_par_coin_le_plan_viewport_couvre_le_pane() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let (el, releve) = element(&b.hote);
    let _off = b.hote.mount(el, OverlayPlane::Viewport);
    b.hote
        .set_viewport_edges(el, None, Some(8.0), None, Some(8.0));
    image(vcx);
    assert_eq!(coin(&releve), (892.0, 562.0));
    b.hote
        .set_viewport_edges(el, Some(8.0), None, None, Some(0.0));
    image(vcx);
    assert_eq!(coin(&releve), (8.0, 570.0));
    // Le défilement ne le déplace pas.
    b.source.defile.set(400.0);
    image(vcx);
    assert_eq!(coin(&releve), (8.0, 570.0));
}

#[test]
fn pass_through_le_clic_traverse_l_element_jusqu_a_la_surface() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let (el, _releve) = element(&b.hote);
    let _off = b.hote.mount(el, OverlayPlane::Viewport);
    b.hote
        .set_viewport_edges(el, Some(0.0), None, Some(0.0), None);
    b.hote.attach(el, false);
    image(vcx);
    vcx.simulate_click(point(px(50.0), px(15.0)), Modifiers::default());
    assert_eq!(
        b.clics.get(),
        0,
        "un widget capte : le texte ne reçoit rien"
    );
    b.hote.attach(el, true);
    image(vcx);
    vcx.simulate_click(point(px(50.0), px(15.0)), Modifiers::default());
    assert_eq!(b.clics.get(), 1, "transparent : le clic traverse");
    // Hors de tout widget, la surface reçoit toujours.
    vcx.simulate_click(point(px(500.0), px(300.0)), Modifiers::default());
    assert_eq!(b.clics.get(), 2);
}

#[test]
fn la_bande_ne_change_pas_a_chaque_image_seulement_en_approchant_d_un_bord() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let annonces = Rc::new(Cell::new(0));
    let a = Rc::clone(&annonces);
    let _off = b
        .hote
        .on_viewport_change(Box::new(move || a.set(a.get() + 1)));
    let b0 = b.hote.viewport().unwrap();
    for i in 1..=10 {
        b.source.defile.set(f64::from(i) * 20.0);
        image(vcx);
    }
    assert_eq!(annonces.get(), 0);
    assert_eq!(b.hote.viewport(), Some(b0));
    b.source.defile.set(1500.0);
    image(vcx);
    assert_eq!(annonces.get(), 1);
    assert!(b.hote.viewport().unwrap().top > b0.top);
}

#[test]
fn la_geometrie_est_annoncee_quand_la_colonne_se_deplace_pas_au_defilement() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let annonces = Rc::new(Cell::new(0));
    let a = Rc::clone(&annonces);
    let _off = b
        .hote
        .on_geometry_change(Box::new(move || a.set(a.get() + 1)));
    b.source.defile.set(250.0);
    image(vcx);
    assert_eq!(annonces.get(), 0);
    // Une réserve de marge déplace la colonne sans la redimensionner.
    b.source.decalage_colonne.set(-100.0);
    image(vcx);
    assert_eq!(annonces.get(), 1);
    assert_eq!(b.hote.content_client_rect().unwrap().left, 100.0);
}

#[test]
fn sans_plan_document_seul_le_viewport_repond() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, false);
    image(vcx);
    assert!(!b.hote.a_un_plan_document());
    assert!(b.hote.client_to_document(10.0, 10.0).is_none());
    assert!(b.hote.document_to_client(0.0, 0.0).is_none());
    assert!(b.hote.gutter_band(GutterSide::Left).is_none());
    assert!(b.hote.viewport().is_none());
    assert!(b.hote.client_to_viewport(10.0, 10.0).is_some());
}

#[test]
fn un_element_masque_n_est_ni_dessine_ni_mesure_affiche_il_revient() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let (el, releve) = element(&b.hote);
    let _off = b.hote.mount(el, OverlayPlane::Viewport);
    b.hote.set_displayed(el, false);
    image(vcx);
    assert!(releve.borrow().is_none());
    assert_eq!(b.hote.fiche(el).unwrap().taille, (0.0, 0.0));
    b.hote.set_displayed(el, true);
    image(vcx);
    assert!(releve.borrow().is_some());
    // Mesuré dans la même image que posé : l'arbitre lit cette hauteur.
    assert_eq!(b.hote.fiche(el).unwrap().taille, (100.0, 30.0));
}

#[test]
fn demonte_l_element_n_est_plus_dessine() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx, true);
    let (el, releve) = element(&b.hote);
    let off = b.hote.mount(el, OverlayPlane::Document);
    image(vcx);
    assert!(releve.borrow().is_some());
    off();
    *releve.borrow_mut() = None;
    image(vcx);
    assert!(releve.borrow().is_none());
    assert_eq!(b.hote.fiche(el).unwrap().racine, None);
}
