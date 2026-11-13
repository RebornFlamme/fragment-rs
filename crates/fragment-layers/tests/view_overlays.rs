// Port de app/src/ui/core/layers/ViewOverlays.ts (géométrie) — pas de fichier de tests TS :
// cas écrits d'après le contrat d'`OverlayHost` et les specs e2e `viewport`, `gutter-room`,
// `margin-mode` et `reader-pdf` (« zoom »), sur des rectangles inventés ici.

use std::cell::Cell;
use std::rc::Rc;

use fragment_layers::gutter_arbiter::{GutterArbiter, SlotElements};
use fragment_layers::gutter_stack::gutter_gap;
use fragment_layers::types::{ElementId, GutterBand, GutterSide, Point};
use fragment_layers::view_overlays::{
    GUTTER_PAD, GeometrieVue, MesuresPlans, Racine, bande_de_marge, cle_de_geometrie,
    client_vers_document, client_vers_viewport, coin_viewport, document_vers_client,
    echelle_bornee, niveau_vers_marge, origine_du_plan, position_dans_le_pane,
};
use fragment_text::Rect;

fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Rect {
    Rect {
        left,
        top,
        right,
        bottom,
    }
}

/// Un pane de 1000 × 600 à (100, 50), une colonne de 600 centrée, le plan à l'origine
/// du contenu défilé de `defile` px.
fn mesures(defile: f64, echelle: f64) -> MesuresPlans {
    let pane = rect(100.0, 50.0, 1100.0, 650.0);
    MesuresPlans {
        pane,
        origine: Some(origine_du_plan(
            rect(300.0, 50.0, 900.0, 650.0),
            0.0,
            defile,
        )),
        colonne: Some(rect(300.0, 50.0 - defile, 900.0, 4000.0)),
        echelle,
    }
}

#[test]
fn client_to_viewport_rend_le_point_du_pane_et_rien_dehors() {
    let pane = rect(100.0, 50.0, 1100.0, 650.0);
    assert_eq!(
        client_vers_viewport(pane, 150.0, 70.0),
        Some(Point { x: 50.0, y: 20.0 })
    );
    // Les bords sont dedans.
    assert_eq!(
        client_vers_viewport(pane, 1100.0, 650.0),
        Some(Point {
            x: 1000.0,
            y: 600.0
        })
    );
    assert_eq!(client_vers_viewport(pane, 99.0, 70.0), None);
    assert_eq!(client_vers_viewport(pane, 150.0, 651.0), None);
}

#[test]
fn document_to_client_est_l_inverse_exact_de_client_to_document_a_toute_echelle() {
    let o = Point {
        x: 300.0,
        y: -950.0,
    };
    for e in [1.0, 0.5, 1.5, 2.0, 3.25] {
        for (x, y) in [(0.0, 0.0), (120.5, 3000.25), (-40.0, 7.0)] {
            let d = client_vers_document(o, e, x, y);
            let c = document_vers_client(o, e, d.x, d.y);
            assert!((c.x - x).abs() < 1e-9 && (c.y - y).abs() < 1e-9, "e={e}");
        }
    }
    // Échelle 1 : une translation pure.
    assert_eq!(
        client_vers_document(o, 1.0, 310.0, 0.0),
        Point { x: 10.0, y: 950.0 }
    );
    // Échelle 2 : la distance à l'origine est divisée.
    assert_eq!(
        client_vers_document(o, 2.0, 310.0, -950.0),
        Point { x: 5.0, y: 0.0 }
    );
}

#[test]
fn une_echelle_absurde_vaut_un() {
    assert_eq!(echelle_bornee(0.0), 1.0);
    assert_eq!(echelle_bornee(-2.0), 1.0);
    assert_eq!(echelle_bornee(f64::NAN), 1.0);
    assert_eq!(echelle_bornee(f64::INFINITY), 1.0);
    assert_eq!(echelle_bornee(0.75), 0.75);
}

#[test]
fn l_origine_du_plan_retire_son_propre_defilement() {
    assert_eq!(
        origine_du_plan(rect(300.0, 50.0, 900.0, 650.0), 4.0, 200.0),
        Point {
            x: 296.0,
            y: -150.0
        }
    );
}

#[test]
fn la_bande_de_marge_va_du_souffle_du_pane_a_l_ecart_de_la_colonne() {
    let pane = rect(100.0, 50.0, 1100.0, 650.0);
    let col = rect(300.0, 0.0, 900.0, 2000.0);
    let o = Point { x: 300.0, y: 0.0 };
    // Marge brute 200 − 14 = 186 → écart 18,6 (10 %).
    let g = bande_de_marge(GutterSide::Left, pane, col, o, &gutter_gap);
    let gap = gutter_gap(200.0 - GUTTER_PAD);
    assert!((g.left - (100.0 + GUTTER_PAD - 300.0)).abs() < 1e-9);
    assert!((g.width - (300.0 - gap - 114.0)).abs() < 1e-9);
    let d = bande_de_marge(GutterSide::Right, pane, col, o, &gutter_gap);
    assert!((d.left - (900.0 + gap - 300.0)).abs() < 1e-9);
    assert!((d.width - (1100.0 - GUTTER_PAD - 900.0 - gap)).abs() < 1e-9);
}

#[test]
fn l_ecart_colonne_widget_est_celui_de_la_vue() {
    // Une vue PDF : 12 px fixes (la page porte déjà son blanc).
    let pane = rect(0.0, 0.0, 1000.0, 600.0);
    let col = rect(200.0, 0.0, 800.0, 600.0);
    let o = Point { x: 0.0, y: 0.0 };
    let d = bande_de_marge(GutterSide::Right, pane, col, o, &|_| 12.0);
    assert_eq!(
        d,
        GutterBand {
            left: 812.0,
            width: 1000.0 - GUTTER_PAD - 812.0
        }
    );
}

#[test]
fn marge_trop_etroite_la_bande_a_une_largeur_negative() {
    let pane = rect(0.0, 0.0, 600.0, 600.0);
    let col = rect(0.0, 0.0, 600.0, 600.0);
    let o = Point { x: 0.0, y: 0.0 };
    for side in GutterSide::TOUS {
        assert!(bande_de_marge(side, pane, col, o, &gutter_gap).width <= 0.0);
    }
}

#[test]
fn la_cle_de_geometrie_ignore_la_hauteur_de_la_colonne_mais_pas_sa_place() {
    let pane = rect(0.0, 0.0, 1000.0, 600.0);
    let a = cle_de_geometrie(pane, Some(rect(200.0, 0.0, 800.0, 3000.0)));
    // Une frappe : la colonne grandit.
    assert_eq!(
        a,
        cle_de_geometrie(pane, Some(rect(200.0, 0.0, 800.0, 3020.0)))
    );
    // Une réserve de marge déplace la colonne sans la redimensionner.
    assert_ne!(
        a,
        cle_de_geometrie(pane, Some(rect(100.0, 0.0, 700.0, 3000.0)))
    );
    // Un sous-pixel ne compte pas.
    assert_eq!(
        a,
        cle_de_geometrie(pane, Some(rect(200.2, 0.0, 800.2, 3000.0)))
    );
}

#[test]
fn la_marge_n_est_pas_zoomee_le_niveau_voulu_est_converti() {
    // Le niveau voulu arrive en unités du plan ; la marge parle en px écran.
    assert_eq!(niveau_vers_marge(Some(100.0), 2.0), Some(200.0));
    assert_eq!(niveau_vers_marge(None, 2.0), None);
    // La bande de marge, elle, ne dépend pas de l'échelle : px écran.
    let mut g1 = GeometrieVue::default();
    let mut g2 = GeometrieVue::default();
    g1.poser_mesures(mesures(0.0, 1.0));
    g2.poser_mesures(mesures(0.0, 2.0));
    assert_eq!(
        g1.gutter_band(GutterSide::Right),
        g2.gutter_band(GutterSide::Right)
    );
}

#[test]
fn chaque_racine_a_son_unite_dans_le_pane() {
    let pane = rect(100.0, 50.0, 1100.0, 650.0);
    let o = Some(Point {
        x: 300.0,
        y: -150.0,
    });
    // Viewport : coin du pane, tel quel.
    assert_eq!(
        position_dans_le_pane(Racine::Viewport, pane, o, 2.0, 10.0, 20.0),
        Some(Point { x: 10.0, y: 20.0 })
    );
    // Document : unités du plan × échelle, depuis l'origine.
    assert_eq!(
        position_dans_le_pane(Racine::Document, pane, o, 2.0, 10.0, 100.0),
        Some(Point { x: 220.0, y: 0.0 })
    );
    // Marge : px écran depuis l'origine (zoom annulé).
    assert_eq!(
        position_dans_le_pane(Racine::Marge, pane, o, 2.0, 10.0, 100.0),
        Some(Point {
            x: 210.0,
            y: -100.0
        })
    );
    // Sans plan document, seule la racine viewport a un sens.
    assert_eq!(
        position_dans_le_pane(Racine::Document, pane, None, 1.0, 0.0, 0.0),
        None
    );
}

#[test]
fn ancre_viewport_par_coin_les_quatre_coins() {
    let pane = rect(100.0, 50.0, 1100.0, 650.0);
    let t = (200.0, 40.0);
    assert_eq!(
        coin_viewport(pane, t, Some(8.0), None, Some(8.0), None),
        Point { x: 8.0, y: 8.0 }
    );
    assert_eq!(
        coin_viewport(pane, t, None, Some(8.0), Some(8.0), None),
        Point { x: 792.0, y: 8.0 }
    );
    assert_eq!(
        coin_viewport(pane, t, Some(0.0), None, None, Some(0.0)),
        Point { x: 0.0, y: 560.0 }
    );
    assert_eq!(
        coin_viewport(pane, t, None, Some(0.0), None, Some(0.0)),
        Point { x: 800.0, y: 560.0 }
    );
}

#[test]
fn sans_plan_document_seul_le_viewport_repond() {
    let mut g = GeometrieVue::default();
    let mut m = mesures(0.0, 1.0);
    m.origine = None;
    g.poser_mesures(m);
    assert!(g.client_to_viewport(150.0, 60.0).is_some());
    assert!(g.client_to_document(150.0, 60.0).is_none());
    assert!(g.document_to_client(0.0, 0.0).is_none());
    assert!(g.gutter_band(GutterSide::Left).is_none());
    assert!(g.viewport().is_none());
}

#[test]
fn la_bande_ne_change_pas_a_chaque_image_seulement_en_approchant_d_un_bord() {
    let mut g = GeometrieVue::default();
    // Première mesure : la bande existe, sans annonce.
    let ch = g.poser_mesures(mesures(0.0, 1.0));
    assert!(!ch.viewport);
    let b0 = g.viewport().expect("bande mesurée");
    assert!(b0.top < 0.0 && b0.bottom > 600.0);
    // Des petits pas (une image de défilement chacun) : aucune annonce.
    let mut annonces = 0;
    for i in 1..=10 {
        if g.poser_mesures(mesures(f64::from(i) * 20.0, 1.0)).viewport {
            annonces += 1;
        }
    }
    assert_eq!(annonces, 0);
    assert_eq!(g.viewport(), Some(b0));
    // Défiler loin : le visible approche du bord, la bande change, UNE annonce.
    let ch = g.poser_mesures(mesures(1200.0, 1.0));
    assert!(ch.viewport);
    let b1 = g.viewport().expect("bande");
    assert!(b1.top > b0.top);
}

#[test]
fn zoom_la_bande_suit_en_unites_du_plan() {
    let mut g = GeometrieVue::default();
    g.poser_mesures(mesures(0.0, 1.0));
    let b1 = g.viewport().expect("bande");
    let ch = g.poser_mesures(mesures(0.0, 2.0));
    assert!(ch.echelle_marge);
    let b2 = g.viewport().expect("bande");
    // La même bande écran, deux fois plus courte en unités du plan.
    assert!((b2.top - b1.top / 2.0).abs() < 1e-9);
    assert!((b2.bottom - b1.bottom / 2.0).abs() < 1e-9);
}

#[test]
fn la_geometrie_n_est_annoncee_que_si_les_marges_peuvent_changer() {
    let mut g = GeometrieVue::default();
    assert!(!g.poser_mesures(mesures(0.0, 1.0)).geometrie);
    // Défiler (vertical) : la clé ne bouge pas.
    assert!(!g.poser_mesures(mesures(300.0, 1.0)).geometrie);
    // Le pane rétrécit (la colonne se recentre) : annoncé.
    let mut m = mesures(300.0, 1.0);
    m.pane.right = 1000.0;
    m.colonne = Some(rect(250.0, -250.0, 850.0, 4000.0));
    assert!(g.poser_mesures(m).geometrie);
    assert!(!g.poser_mesures(m).geometrie);
}

#[test]
fn les_bandes_sont_memorisees_pour_la_mesure_et_oubliees_a_la_suivante() {
    let mut g = GeometrieVue::new(Box::new(|_| 10.0));
    g.poser_mesures(mesures(0.0, 1.0));
    let a = g.gutter_band(GutterSide::Left).expect("bande");
    assert_eq!(g.gutter_band(GutterSide::Left), Some(a));
    // La colonne se déplace (défilement horizontal) : recalculée.
    let mut m = mesures(0.0, 1.0);
    m.colonne = Some(rect(350.0, 50.0, 950.0, 4000.0));
    g.poser_mesures(m);
    let b = g.gutter_band(GutterSide::Left).expect("bande");
    assert_ne!(a, b);
}

#[test]
fn la_conversion_client_document_suit_le_defilement() {
    let mut g = GeometrieVue::default();
    g.poser_mesures(mesures(0.0, 1.0));
    let avant = g.client_to_document(400.0, 300.0).expect("plan");
    g.poser_mesures(mesures(250.0, 1.0));
    let apres = g.client_to_document(400.0, 300.0).expect("plan");
    // Le même point de l'écran tombe 250 unités plus bas dans le document.
    assert_eq!(apres.y - avant.y, 250.0);
    let c = g.document_to_client(apres.x, apres.y).expect("plan");
    assert_eq!(c, Point { x: 400.0, y: 300.0 });
}

/// Des éléments de slot minimaux (hauteur fixe, tout affiché) : on lit le haut écrit.
#[derive(Default)]
struct Slots {
    haut: Cell<Option<f64>>,
}

impl SlotElements for Slots {
    fn is_displayed(&self, _el: ElementId) -> bool {
        true
    }
    fn height(&self, _el: ElementId) -> f64 {
        40.0
    }
    fn set_top(&self, _el: ElementId, top: f64) {
        self.haut.set(Some(top));
    }
    fn set_room(&self, _el: ElementId, _room: Option<f64>) {}
    fn set_clustered(&self, _el: ElementId, _clustered: bool) {}
}

#[test]
fn zoom_le_widget_de_marge_est_place_en_px_ecran() {
    // Le niveau voulu arrive en unités du plan ; l'arbitre le multiplie par l'échelle.
    let slots = Rc::new(Slots::default());
    let echelle = Rc::new(Cell::new(2.0));
    let rendus = Rc::new(Cell::new(0));
    let (e, r) = (Rc::clone(&echelle), Rc::clone(&rendus));
    let arb = GutterArbiter::new(slots.clone())
        .with_wanted_scale(move || e.get())
        .with_on_slot_removed(move |_| r.set(r.get() + 1));
    let slot = arb.slot(ElementId(1), GutterSide::Right);
    slot.set_wanted(Some(100.0));
    arb.run_microtasks();
    assert_eq!(slots.haut.get(), Some(200.0));
    slot.remove();
    assert_eq!(rendus.get(), 1);
    // Un second retrait ne prévient plus.
    slot.remove();
    assert_eq!(rendus.get(), 1);
}
