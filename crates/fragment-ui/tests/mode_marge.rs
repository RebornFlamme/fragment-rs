//! Tests de la marge de lecture 1/4 – 3/4 (tâche `fr-t5-mode-marge`), nommés d'après les cas de
//! `app/e2e/margin-mode.spec.ts` (snake_case, sans accents). Le scénario est rejoué sur une FAUSSE
//! vue de note dans une fenêtre de test GPUI (sans GPU) : une zone qui défile dont le padding droit
//! est la réserve, une colonne de 720 px au plus centrée dedans (relevée par un `canvas`, donc posée
//! par GPUI), le vrai Workspace et le vrai Keymap du cœur derrière. Les balayages sont de vrais
//! événements de molette GPUI ; l'horloge (geste et transition) est réglée par le test, image par
//! image.

// Fichier de test : les assistants hors `#[test]` (banc, vue factice) déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::{Cell, RefCell, RefMut};
use std::rc::Rc;

use fragment_core::{
    App as _, AppCoeur, ContexteVue, DocumentKey, Hotkey, HotkeyStore, ItemId, ItemViewBase,
    LayoutMemoire, Scope, View, ViewCreator, Workspace,
};
use fragment_theme::Theme;
use fragment_ui::clavier::balayage::{self, Balayage, Defileur, Horloge};
use fragment_ui::mode_marge::{
    self, Genre, MargeDeLecture, enregistrer_commandes, geometrie, largeur_utile_pdf,
};
use gpui_kit::{
    Bounds, Context, InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Pixels,
    Render, ScrollDelta, ScrollEvent, Styled as _, TestAppContext, VisualTestContext, Window,
    canvas, div, point, px, size,
};
use serde_json::json;

// ─── Les vues factices ───

/// Une vue qui montre (ou non) un document, avec sa portée de vue.
struct VueTest {
    document: bool,
    base: ItemViewBase,
}

impl View for VueTest {
    fn get_view_type(&self) -> String {
        if self.document { "markdown" } else { "vide" }.to_owned()
    }
    fn get_display_text(&self) -> String {
        self.get_view_type()
    }
    fn get_document(&self) -> Option<DocumentKey> {
        self.document.then(|| DocumentKey::new("article.md"))
    }
    fn scope(&self) -> Option<Scope> {
        Some(self.base.scope.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn fabrique(document: bool) -> ViewCreator {
    Rc::new(move |ctx: &ContexteVue| {
        Box::new(VueTest {
            document,
            base: ItemViewBase::new(&ctx.app_scope),
        }) as Box<dyn View>
    })
}

struct StoreMuet;
impl HotkeyStore for StoreMuet {
    fn read(&self) -> std::io::Result<Option<String>> {
        Ok(None)
    }
    fn write(&self, _: &str) -> std::io::Result<()> {
        Ok(())
    }
}

/// Le défileur horizontal d'un tableau large : 300 px visibles sur 900.
struct Tableau {
    position: Cell<f64>,
}

/// La fausse note : la zone qui défile (réserve en padding droit), la colonne, un tableau large
/// en haut à gauche de la colonne (de 0 à 300 en x, de 0 à 100 en y).
struct FausseNote {
    marge: Rc<MargeDeLecture>,
    leaf: ItemId,
    balayage: Balayage,
    tableau: Rc<Tableau>,
    colonne: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

impl Render for FausseNote {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let r = self.marge.reserve(self.leaf, Genre::Note);
        if r.en_transition {
            window.request_animation_frame();
        }
        let largeur = f32::from(window.viewport_size().width);
        let pr = geometrie::reserve(largeur, r.fraction) + LARGEUR_BARRE;
        let releve = Rc::clone(&self.colonne);
        let tableau = Rc::clone(&self.tableau);
        let table = div()
            .id("tableau")
            .absolute()
            .left_0()
            .top_0()
            .w(px(300.0))
            .h(px(100.0))
            .on_scroll(move |e, window, _| {
                let d = Defileur {
                    position: tableau.position.get(),
                    visible: 300.0,
                    contenu: 900.0,
                };
                balayage::absorber(&d, e, window.line_height());
                // Le tableau défile (borné), comme le ferait le navigateur.
                let (dx, _) = balayage::delta_dom(&e.delta, window.line_height());
                tableau
                    .position
                    .set((tableau.position.get() + dx).clamp(0.0, 600.0));
            });
        balayage::ecouter(div().id("racine").size_full(), self.balayage.clone()).child(
            div().size_full().pr(px(pr)).child(
                div()
                    .relative()
                    .w_full()
                    .max_w(px(720.0))
                    .mx_auto()
                    .h(px(400.0))
                    .child(
                        canvas(
                            move |b, _, _| *releve.borrow_mut() = Some(b),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(table),
            ),
        )
    }
}

/// `.doc-scroll::-webkit-scrollbar { width: 6px }`, toujours réservée.
const LARGEUR_BARRE: f32 = 6.0;
const LARGEUR: f32 = 1000.0;

// ─── Le banc ───

struct Banc {
    app: Rc<AppCoeur>,
    marge: Rc<MargeDeLecture>,
    horloge: Rc<Cell<f64>>,
    tableau: Rc<Tableau>,
    colonne: Rc<RefCell<Option<Bounds<Pixels>>>>,
    _retraits: Vec<Box<dyn FnOnce()>>,
}

fn layout() -> LayoutMemoire {
    let feuille =
        |id: &str, t: &str| json!({"type": "leaf", "id": id, "state": {"type": t, "state": {}}});
    let vide = json!({"type": "split", "direction": "horizontal", "children": []});
    LayoutMemoire::avec(
        &json!({
            "version": 1,
            "active": "a",
            "left": vide,
            "right": vide,
            "main": {"type": "split", "direction": "vertical", "children": [
                {"type": "tabs", "currentTab": 0, "children": [
                    feuille("a", "markdown"), feuille("b", "markdown"), feuille("c", "vide"),
                ]},
            ]},
        })
        .to_string(),
    )
}

fn banc(cx: &mut TestAppContext) -> (Banc, &mut VisualTestContext) {
    let app = AppCoeur::new();
    // Les raccourcis AVANT les commandes : leurs liaisons par défaut sont lues à l'enregistrement.
    app.installer_raccourcis(Rc::new(StoreMuet));
    let mut retraits: Vec<Box<dyn FnOnce()>> = vec![
        app.view_registry()
            .register("markdown", fabrique(true), "test"),
        app.view_registry()
            .register("vide", fabrique(false), "test"),
    ];
    {
        let mut w = app.workspace().borrow_mut();
        w.set_layout_store(Rc::new(layout()));
        w.load_layout();
    }
    let horloge = Rc::new(Cell::new(1000.0));
    let h = Rc::clone(&horloge);
    let lire: Horloge = Rc::new(move || h.get());
    let marge = MargeDeLecture::avec_horloge(&Theme::sombre(), Rc::clone(&lire));
    retraits.extend(enregistrer_commandes(
        &app.commands(),
        app.workspace(),
        &marge,
    ));
    let balayage = Balayage::avec_horloge(app.keymap().clone(), lire);
    let tableau = Rc::new(Tableau {
        position: Cell::new(0.0),
    });
    let colonne = Rc::new(RefCell::new(None));
    let leaf = app.workspace().borrow().active_leaf().unwrap();
    let (m, t, c) = (Rc::clone(&marge), Rc::clone(&tableau), Rc::clone(&colonne));
    let (_vue, vcx) = cx.add_window_view(move |_, _| FausseNote {
        marge: m,
        leaf,
        balayage,
        tableau: t,
        colonne: c,
    });
    vcx.simulate_resize(size(px(LARGEUR), px(600.0)));
    image(vcx);
    (
        Banc {
            app,
            marge,
            horloge,
            tableau,
            colonne,
            _retraits: retraits,
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

impl Banc {
    fn ws(&self) -> RefMut<'_, Workspace> {
        self.app.workspace().borrow_mut()
    }

    fn active(&self) -> ItemId {
        self.ws().active_leaf().unwrap()
    }

    fn mode(&self) -> bool {
        self.marge.est_ouverte(self.active())
    }

    fn avancer(&self, ms: f64) {
        self.horloge.set(self.horloge.get() + ms);
    }

    /// (largeur de la colonne, marge droite : du bord droit de la colonne au bord du pane).
    fn geo(&self) -> (f32, f32) {
        let b = self.colonne.borrow().expect("colonne posée");
        (
            f32::from(b.size.width),
            LARGEUR - f32::from(b.origin.x + b.size.width),
        )
    }

    /// Une molette au point (x, y), delta DOM (dx, dy), Ctrl ou non : un événement GPUI (sens inverse).
    fn molette(
        &self,
        vcx: &mut VisualTestContext,
        (x, y): (f32, f32),
        dx: f32,
        dy: f32,
        ctrl: bool,
    ) {
        vcx.simulate_event(ScrollEvent {
            position: point(px(x), px(y)),
            delta: ScrollDelta::Pixels(point(px(-dx), px(-dy))),
            modifiers: Modifiers {
                control: ctrl,
                ..Modifiers::default()
            },
            ..ScrollEvent::default()
        });
    }

    /// Un balayage à deux doigts : 14 molettes de ±20 px espacées de 10 ms, puis la fin du geste.
    fn balayer(&self, vcx: &mut VisualTestContext, droite: bool, ou: (f32, f32)) {
        let dx = if droite { 20.0 } else { -20.0 };
        for _ in 0..14 {
            self.molette(vcx, ou, dx, 0.0, false);
            self.avancer(10.0);
        }
        self.avancer(400.0);
        image(vcx);
    }

    /// Laisse finir la transition (dix images de 16 ms).
    fn finir(&self, vcx: &mut VisualTestContext) {
        for _ in 0..10 {
            self.avancer(16.0);
            image(vcx);
        }
    }

    fn executer(&self, id: &str) -> bool {
        self.app.commands().execute_command_by_id(id, None)
    }
}

/// Au milieu de la note, hors du tableau.
const NOTE: (f32, f32) = (500.0, 300.0);

// ─── Les cas de margin-mode.spec.ts ───

#[test]
fn balayage_vers_la_droite_la_marge_droite_prend_1_4_la_colonne_de_texte_ne_s_elargit_pas_vers_la_gauche_retour()
 {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let (col_avant, droite_avant) = b.geo();
    assert!(!b.mode());
    assert_eq!(col_avant, 720.0);
    b.balayer(vcx, true, NOTE);
    assert!(b.mode());
    b.finir(vcx);
    let (col, droite) = b.geo();
    assert!(droite > LARGEUR * 0.25, "{droite}");
    // La colonne garde sa largeur (720 px au plus), ou rétrécit si les 3/4 n'y suffisent pas.
    assert!(col <= col_avant + 1.0);
    assert!(col >= col_avant.min(LARGEUR * 0.75) - 2.0);
    // La géométrie posée par GPUI est celle du modèle pur.
    let m = mode_marge::mise_en_page_note(LARGEUR, Genre::Note.fraction());
    assert_eq!((col, droite), (m.colonne_largeur, m.marge_droite));
    b.balayer(vcx, false, NOTE);
    assert!(!b.mode());
    b.finir(vcx);
    assert_eq!(b.geo(), (col_avant, droite_avant));
}

#[test]
fn elargir_puis_retrecir_aussitot_sans_pause_inertie_encore_en_cours_les_deux_passent() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    for _ in 0..14 {
        b.molette(vcx, NOTE, 20.0, 0.0, false);
        b.avancer(8.0);
    }
    for _ in 0..6 {
        b.molette(vcx, NOTE, 4.0, 0.0, false); // l'inertie
        b.avancer(8.0);
    }
    assert!(b.mode());
    for _ in 0..14 {
        b.molette(vcx, NOTE, -20.0, 0.0, false);
        b.avancer(8.0);
    }
    assert!(!b.mode());
}

#[test]
fn rapide_et_symetrique_la_marge_change_en_moins_de_150_ms_sans_saut_a_l_aller_comme_au_retour() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    // La marge droite échantillonnée à chaque image (60 Hz) pendant la transition.
    let echantillons = |on: bool, vcx: &mut VisualTestContext| {
        let t0 = b.horloge.get();
        let mut out = vec![(0.0, b.geo().1)];
        b.marge.poser(b.active(), on);
        for _ in 0..20 {
            b.avancer(1000.0 / 60.0);
            image(vcx);
            out.push((b.horloge.get() - t0, b.geo().1));
        }
        out
    };
    let analyse = |xs: &[(f64, f32)]| {
        let debut = xs[0].1;
        let fin = xs[xs.len() - 1].1;
        let total = (fin - debut).abs();
        let arrivee = xs.iter().find(|e| (e.1 - fin).abs() < 1.0).unwrap().0;
        let pas = xs
            .windows(2)
            .map(|w| (w[1].1 - w[0].1).abs())
            .fold(0.0_f32, f32::max);
        let intermediaires = xs
            .iter()
            .filter(|e| (e.1 - debut).abs() > 1.0 && (e.1 - fin).abs() > 1.0)
            .count();
        (total, arrivee, pas, intermediaires)
    };
    let aller = analyse(&echantillons(true, vcx));
    let retour = analyse(&echantillons(false, vcx));
    assert!(aller.0 > 100.0, "{aller:?}");
    assert!((retour.0 - aller.0).abs() < 2.0, "{aller:?} {retour:?}");
    for (total, arrivee, pas, intermediaires) in [aller, retour] {
        assert!(arrivee < 150.0, "nerveux : {arrivee}");
        assert!(intermediaires > 0, "animé, pas un saut");
        assert!(pas < total * 0.8, "ni un saut d'un bloc : {pas} / {total}");
    }
}

#[test]
fn un_seul_declenchement_par_geste_inertie_comprise_deux_balayages_droite_toujours_actif() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    b.balayer(vcx, true, NOTE);
    b.balayer(vcx, true, NOTE);
    assert!(b.mode());
}

#[test]
fn ctrl_molette_horizontale_pinch_et_defilement_vertical_rien() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    for _ in 0..14 {
        b.molette(vcx, NOTE, 20.0, 0.0, true);
        b.avancer(10.0);
    }
    b.avancer(300.0);
    for _ in 0..14 {
        b.molette(vcx, NOTE, 5.0, 40.0, false);
        b.avancer(10.0);
    }
    b.avancer(300.0);
    image(vcx);
    assert!(!b.mode());
}

#[test]
fn sur_un_tableau_large_qui_defile_encore_c_est_le_tableau_qui_defile_pas_la_marge() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let (col, _) = b.geo();
    let x0 = f32::from(b.colonne.borrow().unwrap().origin.x);
    let sur_le_tableau = (x0 + 150.0, 50.0);
    // Vers la droite : le tableau peut encore défiler, le geste est pour lui.
    b.balayer(vcx, true, sur_le_tableau);
    assert!(!b.mode());
    assert!(b.tableau.position.get() > 0.0);
    // Arrivé au bout (600 px), le geste suivant revient à la marge.
    b.tableau.position.set(600.0);
    b.balayer(vcx, true, sur_le_tableau);
    assert!(b.mode());
    assert_eq!(b.geo().0, col);
}

#[test]
fn la_commande_workspace_toggle_margin_bascule_pareil() {
    let mut cx = TestAppContext::single();
    let (b, _vcx) = banc(&mut cx);
    assert!(b.executer("workspace:toggle-margin"));
    assert!(b.mode());
    assert!(b.executer("workspace:toggle-margin"));
    assert!(!b.mode());
}

#[test]
fn le_geste_est_le_raccourci_de_commandes_swipe_right_workspace_open_margin_swipe_left_close_margin_une_vue_qui_le_lie_dans_sa_portee_l_emporte()
 {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let hm = fragment_core::CommandHost::hotkey_manager(&*b.app).unwrap();
    assert_eq!(
        hm.hotkeys_for_command("workspace:open-margin"),
        vec![Hotkey::new(&[], "SwipeRight")]
    );
    assert_eq!(
        hm.hotkeys_for_command("workspace:close-margin"),
        vec![Hotkey::new(&[], "SwipeLeft")]
    );
    // Une vue prend le geste chez elle : la commande n'est pas appelée.
    let portee = {
        let w = b.ws();
        w.leaf(w.active_leaf().unwrap())
            .unwrap()
            .view()
            .scope()
            .unwrap()
    };
    let recus = Rc::new(Cell::new(0));
    let r = Rc::clone(&recus);
    let liaison = portee.register(Some(&[]), Some("SwipeRight"), move |_, _| {
        r.set(r.get() + 1);
        Some(false)
    });
    b.balayer(vcx, true, NOTE);
    assert_eq!(recus.get(), 1);
    assert!(!b.mode());
    portee.unregister(&liaison);
    b.balayer(vcx, true, NOTE);
    assert!(b.mode());
}

#[test]
fn balayage_sur_un_pdf_une_marge_exageree_40_un_pdf_a_deja_ses_marges_les_pages_tiennent_dans_le_reste_retour()
 {
    // Le modèle du PDF (le montage PDF applique la réserve à sa zone) : 40 %, sans transition, et la
    // largeur utile sur laquelle s'ajuste le zoom « largeur de page ».
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let leaf = b.active();
    b.balayer(vcx, true, NOTE);
    let r = b.marge.reserve(leaf, Genre::Pdf);
    assert_eq!(r.fraction, 0.40);
    assert!(!r.en_transition);
    let utile = largeur_utile_pdf(LARGEUR, r.fraction);
    assert!(utile <= LARGEUR * 0.60 + 2.0);
    b.balayer(vcx, false, NOTE);
    let r = b.marge.reserve(leaf, Genre::Pdf);
    assert_eq!((r.fraction, r.en_transition), (0.0, false));
    assert_eq!(largeur_utile_pdf(LARGEUR, r.fraction), LARGEUR);
}

// ─── Ajoutés ───

#[test]
fn la_marge_est_celle_de_la_feuille_les_autres_feuilles_et_les_vues_hebergees_n_en_prennent_pas() {
    let mut cx = TestAppContext::single();
    let (b, _vcx) = banc(&mut cx);
    let a = b.active();
    let autre = {
        let w = b.ws();
        let pile = w.arbre().parent(a).unwrap();
        w.arbre().children(pile)[1]
    };
    assert!(b.executer("workspace:open-margin"));
    assert!(b.marge.est_ouverte(a));
    assert!(!b.marge.est_ouverte(autre));
    assert_eq!(b.marge.reserve(autre, Genre::Note).fraction, 0.0);
    // Ouvrir quand c'est ouvert, fermer quand c'est fermé : sans effet, toujours disponible.
    assert!(b.executer("workspace:open-margin"));
    assert!(b.marge.est_ouverte(a));
    assert!(b.executer("workspace:close-margin"));
    assert!(b.executer("workspace:close-margin"));
    assert!(!b.marge.est_ouverte(a));
}

#[test]
fn indisponible_sur_une_vue_qui_ne_montre_pas_de_document() {
    let mut cx = TestAppContext::single();
    let (b, _vcx) = banc(&mut cx);
    let vide = {
        let w = b.ws();
        let pile = w.arbre().parent(w.active_leaf().unwrap()).unwrap();
        w.arbre().children(pile)[2]
    };
    b.ws().set_active_leaf(Some(vide));
    for id in [
        "workspace:toggle-margin",
        "workspace:open-margin",
        "workspace:close-margin",
    ] {
        assert!(!b.executer(id), "{id}");
    }
    assert!(!b.marge.est_ouverte(vide));
}

#[test]
fn pixels_marge_fige_l_etat_final_sans_transition() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    mode_marge::commandes_de_scene(&b.app.commands());
    let _ecoute = mode_marge::brancher_scene(&b.app.commands(), b.app.workspace(), &b.marge);
    let poser = |on: bool| {
        b.app
            .commands()
            .events()
            .trigger("pixels:args", &json!({ "on": on }));
        assert!(b.executer("pixels:marge"));
    };
    poser(true);
    let r = b.marge.reserve(b.active(), Genre::Note);
    assert_eq!((r.fraction, r.en_transition), (0.25, false));
    image(vcx);
    let m = mode_marge::mise_en_page_note(LARGEUR, 0.25);
    assert_eq!(b.geo().1, m.marge_droite);
    poser(false);
    assert!(!b.mode());
}

#[test]
fn le_retrait_des_commandes_ferme_toutes_les_marges() {
    let mut cx = TestAppContext::single();
    let (mut b, _vcx) = banc(&mut cx);
    assert!(b.executer("workspace:open-margin"));
    let fermees = Rc::new(Cell::new(0));
    let f = Rc::clone(&fermees);
    let _abonne = b.marge.on_change(move |_| f.set(f.get() + 1));
    for retrait in b._retraits.drain(..) {
        retrait();
    }
    assert!(!b.mode());
    assert_eq!(fermees.get(), 1);
    assert!(
        b.app
            .commands()
            .find_command("workspace:toggle-margin")
            .is_none()
    );
}
