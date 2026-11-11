//! Tests du workspace rendu : splits, poignées, docks, boutons de fenêtre et glisser d'onglet
//! (tâche `fr-t1-splits`).
//!
//! Chaque test porte le nom d'un `test(...)` e2e de Fragment, en snake_case et sans accents :
//! `workspace-structure.spec.ts`, `workspace-layout.spec.ts`, `tab-drag-split.spec.ts`, les cas
//! traduisibles de `resize-gesture.spec.ts` et ceux de `tab-drop-frame-space.spec.ts`. Ils rejouent le
//! scénario sur le VRAI rendu : une fenêtre de test GPUI de 1280 × 800 (sans GPU), des gestes de souris
//! simulés (appui, mouvements bouton enfoncé, relâché : le geste continu de `Window::dispatch_event`),
//! le Workspace du cœur derrière. Les vues sont factices (« markdown », « chat », « pdf ») : T1 n'a ni
//! coffre ni vraie vue.
//!
//! Les positions viennent des CSS de Fragment (`docs/constantes-rendu.md`) : le ruban fait 36 px, une
//! barre d'onglets 36, une poignée 1 (attrapable sur 7), un dock 260 + 1 de bordure, les boutons de
//! fenêtre 3 × 46.
//!
//! Non traduit : la « contre-épreuve » de `resize-gesture` (elle retire le CSS du gel ; il n'y a pas de
//! CSS ici).

// Fichier de test : les assistants hors `#[test]` (banc, vues factices) déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::{Cell, RefMut};
use std::rc::Rc;

use fragment_core::{
    App, AppCoeur, ContexteVue, DeferredView, EmptyView, ItemId, ItemViewBase, LAYOUT_CHANGE,
    LayoutMemoire, Scope, View, ViewCreator, Workspace, enregistrer_commandes_app,
    enregistrer_commandes_workspace,
};
use fragment_ui::menu;
use fragment_ui::onglets::ContexteOnglets;
use fragment_ui::splits::{ContexteSplits, rendre_workspace};
use gpui_kit::{
    Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, point, px, size,
};
use serde_json::{Value, json};

// ─── Une vue factice ───

struct VueTest {
    type_: String,
    base: ItemViewBase,
}

impl View for VueTest {
    fn get_view_type(&self) -> String {
        self.type_.clone()
    }

    fn get_display_text(&self) -> String {
        self.type_.clone()
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

fn fabrique(type_: &'static str) -> ViewCreator {
    Rc::new(move |ctx: &ContexteVue| {
        Box::new(VueTest {
            type_: type_.to_owned(),
            base: ItemViewBase::new(&ctx.app_scope),
        }) as Box<dyn View>
    })
}

// ─── Les layouts ───

fn feuille(id: &str, type_: &str) -> Value {
    json!({"type": "leaf", "id": id, "state": {"type": type_, "state": {}}})
}

fn pile(courant: usize, feuilles: Vec<Value>) -> Value {
    json!({"type": "tabs", "currentTab": courant, "children": feuilles})
}

fn avec_dimension(mut n: Value, d: f64) -> Value {
    n["dimension"] = json!(d);
    n
}

/// Un layout : les enfants du centre (split vertical), et éventuellement une pile dans le dock droit
/// (déplié).
fn layout(active: Value, centre: Vec<Value>, droite: Option<Value>) -> LayoutMemoire {
    let deplie = droite.is_some();
    LayoutMemoire::avec(
        &json!({
            "version": 1,
            "active": active,
            "left": {"type": "split", "direction": "horizontal", "children": [], "collapsed": true},
            "right": {"type": "split", "direction": "horizontal",
                      "children": droite.map(|p| vec![p]).unwrap_or_default(), "collapsed": !deplie},
            "main": {"type": "split", "direction": "vertical", "children": centre},
        })
        .to_string(),
    )
}

// ─── Le banc : une session de l'application, branchée sur le rendu du workspace ───

struct Banc {
    app: Rc<AppCoeur>,
    ctx: ContexteSplits,
    /// L'horloge des images (ms), avancée à la main.
    temps: Rc<Cell<f64>>,
    _retraits: Vec<Box<dyn FnOnce()>>,
}

fn lancer(store: &LayoutMemoire) -> Banc {
    let app = AppCoeur::new();
    let mut retraits: Vec<Box<dyn FnOnce()>> = Vec::new();
    for t in ["markdown", "chat", "pdf"] {
        retraits.push(app.view_registry().register(t, fabrique(t), "test"));
    }
    retraits.extend(enregistrer_commandes_workspace(
        &app.commands(),
        app.workspace(),
    ));
    retraits.extend(enregistrer_commandes_app(&app.commands(), app.workspace()));
    {
        let mut w = app.workspace().borrow_mut();
        w.set_layout_store(Rc::new(store.clone()));
        w.load_layout();
    }
    let onglets = ContexteOnglets {
        workspace: app.workspace().clone(),
        commandes: app.commands(),
        keymap: app.keymap().clone(),
        contenu_de_vue: None,
    };
    let temps = Rc::new(Cell::new(0.0));
    let horloge = temps.clone();
    let ctx = ContexteSplits::new(onglets).avec_horloge(move || horloge.get());
    Banc {
        app,
        ctx,
        temps,
        _retraits: retraits,
    }
}

impl Banc {
    fn ws(&self) -> RefMut<'_, Workspace> {
        self.app.workspace().borrow_mut()
    }

    fn piles_du_centre(&self) -> Vec<ItemId> {
        let w = self.ws();
        w.arbre().piles_sous(w.root_split())
    }

    fn feuille_par_id(&self, id: &str) -> ItemId {
        let w = self.ws();
        w.iterate_all_leaves()
            .into_iter()
            .find(|&l| w.leaf(l).is_some_and(|f| f.id == id))
            .unwrap()
    }

    fn largeur_figee(&self, leaf: ItemId) -> f64 {
        self.ctx.boite_figee(leaf).unwrap().largeur
    }

    fn largeur_vue(&self, leaf: ItemId) -> f64 {
        self.ctx.boite_de_la_feuille(leaf).unwrap().largeur
    }

    fn avancer(&self, ms: f64) {
        self.temps.set(self.temps.get() + ms);
    }
}

// ─── La fenêtre de test : le workspace et la couche des menus ───

struct Racine {
    ctx: ContexteSplits,
}

impl Render for Racine {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(rendre_workspace(&self.ctx, window.viewport_size(), cx))
            .child(menu::couche(cx))
    }
}

/// Une image.
fn image(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
}

fn peindre(vcx: &mut VisualTestContext) {
    // Deux images : la première pose les cibles de la souris, la seconde les rend atteignables.
    image(vcx);
    image(vcx);
}

fn fenetre<'a>(
    cx: &'a mut TestAppContext,
    banc: &Banc,
) -> (Entity<Racine>, &'a mut VisualTestContext) {
    let ctx = banc.ctx.clone();
    let (racine, vcx) = cx.add_window_view(|_, _| Racine { ctx });
    vcx.simulate_resize(size(px(1280.0), px(800.0)));
    peindre(vcx);
    (racine, vcx)
}

fn appuyer(vcx: &mut VisualTestContext, x: f64, y: f64) {
    vcx.simulate_mouse_move(
        point(px(x as f32), px(y as f32)),
        None,
        Modifiers::default(),
    );
    vcx.simulate_mouse_down(
        point(px(x as f32), px(y as f32)),
        MouseButton::Left,
        Modifiers::default(),
    );
    peindre(vcx);
}

/// Un mouvement bouton enfoncé, suivi d'UNE image (un pas du geste par image, comme la souris).
fn tirer(vcx: &mut VisualTestContext, x: f64, y: f64) {
    vcx.simulate_mouse_move(
        point(px(x as f32), px(y as f32)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    image(vcx);
}

/// `n` pas de `dx` depuis `(x, y)`.
fn pas(vcx: &mut VisualTestContext, x: f64, y: f64, n: usize, dx: f64) {
    for i in 1..=n {
        tirer(vcx, x + i as f64 * dx, y);
    }
}

fn relacher(vcx: &mut VisualTestContext, x: f64, y: f64) {
    vcx.simulate_mouse_up(
        point(px(x as f32), px(y as f32)),
        MouseButton::Left,
        Modifiers::default(),
    );
    peindre(vcx);
}

fn clic(vcx: &mut VisualTestContext, x: f64, y: f64) {
    vcx.simulate_click(point(px(x as f32), px(y as f32)), Modifiers::default());
    peindre(vcx);
}

/// Le corps de la pile `pile` dans la mise en page courante.
fn corps(banc: &Banc, pile: ItemId) -> fragment_core::Cadre {
    banc.ctx.disposition().corps(pile, 36.0).unwrap()
}

/// Glisse l'onglet `rang` de `pile` vers `(dx, dy)` : appui au centre de son en-tête, franchissement
/// du seuil de 4 px en 3 pas, puis 10 pas jusqu'à la cible, relâché (le geste de
/// `tab-drag-split.spec.ts`).
fn glisser_onglet(
    vcx: &mut VisualTestContext,
    banc: &Banc,
    pile: ItemId,
    rang: usize,
    dx: f64,
    dy: f64,
) {
    let barre = banc.ctx.disposition().barre(pile, 36.0).unwrap();
    let (sx, sy) = (barre.x + 4.0 + 206.0 * rang as f64 + 103.0, barre.y + 20.0);
    appuyer(vcx, sx, sy);
    for i in 1..=3 {
        let t = i as f64 / 3.0;
        tirer(vcx, sx + 8.0 * t, sy + 8.0 * t);
    }
    let (x0, y0) = (sx + 8.0, sy + 8.0);
    for i in 1..=10 {
        let t = i as f64 / 10.0;
        tirer(vcx, x0 + (dx - x0) * t, y0 + (dy - y0) * t);
    }
    relacher(vcx, dx, dy);
}

// ─── tab-drag-split.spec.ts ───

#[test]
fn glisser_un_onglet_vers_le_bord_droit_cree_un_split() {
    // Une seule pile, DEUX onglets : « un » puis « deux », et c'est « deux » qui est actif.
    let banc = lancer(&layout(
        json!("deux"),
        vec![pile(
            1,
            vec![feuille("un", "markdown"), feuille("deux", "markdown")],
        )],
        None,
    ));
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    assert_eq!(banc.piles_du_centre().len(), 1);
    let source = banc.piles_du_centre()[0];
    let un = banc.feuille_par_id("un");

    // On glisse le PREMIER onglet (INACTIF) vers le bord droit : 8 % du bord, mi-hauteur du CORPS.
    let b = corps(&banc, source);
    let (dx, dy) = (b.droite() - b.largeur * 0.08, b.y + b.hauteur / 2.0);
    let barre = banc.ctx.disposition().barre(source, 36.0).unwrap();
    let (sx, sy) = (barre.x + 4.0 + 103.0, barre.y + 20.0);
    appuyer(vcx, sx, sy);
    tirer(vcx, sx + 2.0, sy + 2.0);
    assert!(
        !banc.ctx.glisser_en_cours(),
        "sous le seuil de 4 px : pas encore un glisser"
    );
    tirer(vcx, sx + 8.0, sy + 8.0);
    assert!(
        banc.ctx.glisser_en_cours(),
        "le seuil franchi, le glisser démarre"
    );
    for i in 1..=10 {
        let t = i as f64 / 10.0;
        tirer(
            vcx,
            sx + 8.0 + (dx - sx - 8.0) * t,
            sy + 8.0 + (dy - sy - 8.0) * t,
        );
    }
    // L'aperçu : la moitié droite de la pile.
    let a = banc.ctx.apercu().unwrap();
    assert_eq!(
        (a.pile, a.zone),
        (source, fragment_core::DropZone::SplitRight)
    );
    relacher(vcx, dx, dy);

    assert!(!banc.ctx.glisser_en_cours());
    let piles = banc.piles_du_centre();
    assert_eq!(piles.len(), 2, "le split a eu lieu");
    // L'onglet glissé est seul dans la nouvelle pile, à droite, et il est actif.
    let w = banc.ws();
    assert_eq!(w.arbre().children(piles[1]), &[un]);
    assert_eq!(w.active_leaf(), Some(un));
    drop(w);
    // Et le rendu suit : deux piles côte à côte.
    let d = banc.ctx.disposition();
    assert!(d.cadre(piles[0]).unwrap().droite() < d.cadre(piles[1]).unwrap().x);
}

// ─── workspace-structure.spec.ts ───

fn deux_panes() -> Banc {
    lancer(&layout(
        json!("a"),
        vec![
            pile(0, vec![feuille("a", "markdown")]),
            pile(0, vec![feuille("b", "markdown")]),
        ],
        None,
    ))
}

#[test]
fn layout_change_quand_la_structure_change_une_pile_deplacee_une_seule_emission() {
    let banc = deux_panes();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let [a, b] = banc.piles_du_centre()[..] else {
        panic!("deux piles attendues");
    };
    let n = Rc::new(Cell::new(0));
    let compte = n.clone();
    let _ref = banc
        .ws()
        .events()
        .on(LAYOUT_CHANGE, move |_| compte.set(compte.get() + 1));
    // Déplacer une pile (aucune vue ne change) : la 2e passe en tête.
    let racine = banc.ws().root_split();
    banc.ws().move_item(racine, b, Some(0)).unwrap();
    peindre(vcx);
    assert_eq!(n.get(), 1);
    // Le rendu suit l'arbre : b est maintenant à gauche.
    let d = banc.ctx.disposition();
    assert_eq!(d.cadre(b).unwrap().x, 36.0);
    assert!(d.cadre(a).unwrap().x > 600.0);
}

#[test]
fn la_sauvegarde_garde_la_proportion_d_un_pane_presente_autrement_en_css_le_modele_flex_grow_inline()
 {
    let banc = deux_panes();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let [a, b] = banc.piles_du_centre()[..] else {
        panic!("deux piles attendues");
    };
    banc.ws().poser_grow(a, 0.6);
    banc.ws().poser_grow(b, 1.4);
    peindre(vcx);
    // Le rendu est la projection du modèle : 1243 px partagés en 0,6 / 1,4 (arêtes calées).
    let d = banc.ctx.disposition();
    assert_eq!(
        d.cadre(a).unwrap().largeur,
        (36.0_f64 + 1243.0 * 0.3).round() - 36.0
    );
    // Et la sauvegarde lit le modèle, pas l'écran.
    let dims: Vec<f64> = match banc.ws().get_layout().main.children.as_slice() {
        [
            fragment_core::LayoutNode::Tabs(x),
            fragment_core::LayoutNode::Tabs(y),
        ] => {
            vec![x.dimension.unwrap(), y.dimension.unwrap()]
        }
        autre => panic!("deux piles attendues : {autre:?}"),
    };
    assert!((dims[0] - 0.6).abs() < 1e-5);
    assert!((dims[1] - 1.4).abs() < 1e-5);
}

#[test]
fn l_espace_des_boutons_de_fenetre_va_a_la_pile_qui_est_dessous_un_pane_etroit_au_coin_le_cede() {
    let banc = deux_panes();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let [a, b] = banc.piles_du_centre()[..] else {
        panic!("deux piles attendues");
    };
    let porteur = |banc: &Banc| {
        let d = banc.ctx.disposition();
        let w = banc.ws();
        let m = fragment_core::MiseEnPage {
            workspace: d.fenetre,
            rect: &|id| d.cadre(id),
        };
        w.espace_des_boutons(Some(&m), 138.0).unwrap()
    };
    assert_eq!(porteur(&banc).pile, b, "avant : le pane du coin");

    // b réduit à une tranche de 36 px au coin haut-droit : les boutons (3 × 46 px) couvrent la fin
    // de la barre d'onglets de a.
    banc.ws().poser_grow(a, 2.0 * 1207.0 / 1243.0);
    banc.ws().poser_grow(b, 2.0 * 36.0 / 1243.0);
    peindre(vcx);
    let e = porteur(&banc);
    assert_eq!(e.pile, a, "pendant : la pile qui est DESSOUS");
    // La réserve est le RECOUVREMENT (138 − 36 − 1 de poignée), pas 138 : le bouton du dock droit
    // de a se colle aux boutons de fenêtre.
    let r = e.recouvrement.unwrap();
    let droite_a = banc.ctx.disposition().cadre(a).unwrap().droite();
    let gauche_boutons = 1280.0 - 138.0;
    assert!(((droite_a - r) - gauche_boutons).abs() < 0.5);

    // Le bouton du dock droit est rendu dans la barre de a : un clic dessus ouvre le dock droit.
    let droit = banc.ws().right_split();
    assert!(!banc.ws().arbre().dock_ouvert(droit));
    clic(vcx, gauche_boutons - 4.0 - 12.0, 20.0);
    assert!(banc.ws().arbre().dock_ouvert(droit));
    banc.ws().collapse(droit);

    banc.ws().poser_grow(a, 1.0);
    banc.ws().poser_grow(b, 1.0);
    peindre(vcx);
    assert_eq!(porteur(&banc).pile, b, "après : de retour au pane du coin");
}

// ─── workspace-layout.spec.ts ───

#[test]
fn un_coffre_neuf_demarre_sur_le_layout_par_defaut_et_l_ecrit() {
    let store = LayoutMemoire::new();
    let banc = lancer(&store);
    let mut cx = TestAppContext::single();
    let (_racine, _vcx) = fenetre(&mut cx, &banc);
    let d = banc.ctx.disposition();
    assert_eq!(banc.piles_du_centre().len(), 1);
    assert!(
        d.cadre(banc.piles_du_centre()[0]).is_some(),
        "la pile est à l'écran"
    );
    assert!(banc.ws().save_layout().unwrap());
    let ecrit: Value = serde_json::from_str(&store.contenu().unwrap()).unwrap();
    assert_eq!(ecrit["version"], 1);
    assert_eq!(ecrit["main"]["type"], "split");
}

#[test]
fn l_arbre_l_epinglage_et_le_repli_reviennent_apres_un_redemarrage() {
    let store = layout(
        json!("un"),
        vec![pile(0, vec![feuille("un", "markdown")])],
        None,
    );
    {
        let banc = lancer(&store);
        let mut cx = TestAppContext::single();
        let (_racine, vcx) = fenetre(&mut cx, &banc);
        // Une deuxième pile à côté de la première (par le glisser du rendu), un onglet épinglé, et
        // une feuille dans le dock gauche (déplié).
        let un = banc.feuille_par_id("un");
        let pile = banc.piles_du_centre()[0];
        let deux = banc.ws().create_leaf_in_parent(pile, None).unwrap();
        peindre(vcx);
        let b = corps(&banc, pile);
        glisser_onglet(
            vcx,
            &banc,
            pile,
            1,
            b.droite() - 40.0,
            b.y + b.hauteur / 2.0,
        );
        assert_eq!(banc.piles_du_centre().len(), 2);
        let seconde = banc.piles_du_centre()[1];
        let trois = banc.ws().create_leaf_in_parent(seconde, None).unwrap();
        banc.ws().set_pinned(trois, true);
        banc.ws().set_active_leaf(Some(trois));
        let gauche = banc.ws().get_left_leaf().unwrap();
        banc.ws()
            .set_view_state(gauche, fragment_core::ViewState::new("chat"), None);
        let dock = banc.ws().left_split();
        banc.ws().expand(dock);
        peindre(vcx);
        assert!(
            banc.ctx
                .disposition()
                .dock(fragment_core::Cote::Gauche)
                .is_some()
        );
        // Le repli, par la case du ruban.
        clic(vcx, 18.0, 18.0);
        assert!(
            banc.ctx
                .disposition()
                .dock(fragment_core::Cote::Gauche)
                .is_none()
        );
        assert!(banc.ws().save_layout().unwrap());
        let _ = (un, deux);
    }

    let banc = lancer(&store);
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    assert_eq!(banc.piles_du_centre().len(), 2);
    let dock = banc.ws().left_split();
    assert!(!banc.ws().arbre().dock_ouvert(dock), "le repli est revenu");
    assert!(
        banc.ctx
            .disposition()
            .dock(fragment_core::Cote::Gauche)
            .is_none()
    );
    let epinglees: Vec<ItemId> = {
        let w = banc.ws();
        w.iterate_all_leaves()
            .into_iter()
            .filter(|&l| w.leaf(l).is_some_and(|f| f.pinned()))
            .collect()
    };
    assert_eq!(epinglees.len(), 1);
    assert_eq!(banc.ws().active_leaf(), Some(epinglees[0]));

    // ★ Un dock restauré REPLIÉ n'a pas matérialisé ses feuilles : le déplier (la case du ruban) doit
    //   les charger.
    let gauche = banc.ws().arbre().feuilles_sous(dock)[0];
    assert!(banc.ws().leaf(gauche).unwrap().is_deferred());
    clic(vcx, 18.0, 18.0);
    assert!(
        banc.ctx
            .disposition()
            .dock(fragment_core::Cote::Gauche)
            .is_some()
    );
    let w = banc.ws();
    assert!(!w.leaf(gauche).unwrap().is_deferred());
    assert_eq!(w.leaf(gauche).unwrap().view().get_view_type(), "chat");
}

#[test]
fn seules_les_feuilles_visibles_sont_materialisees_un_clic_charge_la_suivante() {
    let banc = lancer(&layout(
        json!("trois"),
        vec![pile(
            2,
            vec![
                feuille("un", "markdown"),
                feuille("deux", "markdown"),
                feuille("trois", "markdown"),
            ],
        )],
        None,
    ));
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let differees = |banc: &Banc| {
        let w = banc.ws();
        w.iterate_all_leaves()
            .into_iter()
            .filter(|&l| w.leaf(l).unwrap().view().as_any().is::<DeferredView>())
            .count()
    };
    assert_eq!(
        differees(&banc),
        2,
        "une seule vue construite sur les trois"
    );

    // Un appui sur le PREMIER en-tête (dans le rendu) le matérialise, et rien d'autre.
    appuyer(vcx, 36.0 + 4.0 + 103.0, 20.0);
    relacher(vcx, 36.0 + 4.0 + 103.0, 20.0);
    assert_eq!(differees(&banc), 1);
    let un = banc.feuille_par_id("un");
    assert_eq!(banc.ws().active_leaf(), Some(un));
}

#[test]
fn un_onglet_vide_restaure_reste_un_onglet_vide_pas_une_vue_introuvable() {
    let store = layout(
        json!("un"),
        vec![pile(0, vec![feuille("un", "markdown")])],
        None,
    );
    {
        let banc = lancer(&store);
        let mut cx = TestAppContext::single();
        let (_racine, vcx) = fenetre(&mut cx, &banc);
        // Le « + » de la barre : après l'en-tête (36 + 8 + 206), moins 1 de marge, 12 de demi-case.
        clic(vcx, 36.0 + 8.0 + 206.0 - 1.0 + 12.0, 20.0);
        let pile = banc.piles_du_centre()[0];
        assert_eq!(banc.ws().arbre().children(pile).len(), 2);
        assert!(banc.ws().save_layout().unwrap());
    }
    let banc = lancer(&store);
    let mut cx = TestAppContext::single();
    let (_racine, _vcx) = fenetre(&mut cx, &banc);
    let pile = banc.piles_du_centre()[0];
    let enfants = banc.ws().arbre().children(pile).to_vec();
    let w = banc.ws();
    let vide = w.leaf(enfants[1]).unwrap();
    // Un onglet vide RESTAURÉ est indiscernable d'un onglet vide NEUF : un panneau blanc.
    let message = vide
        .view()
        .as_any()
        .downcast_ref::<EmptyView>()
        .map(|v| v.message().to_owned());
    assert_eq!(message.as_deref(), Some(""));
    assert_eq!(vide.get_display_text(), "New Tab");
    assert!(!vide.is_deferred());
    // ★ Et le voisin markdown, lui, est bien resté différé.
    assert_eq!(w.leaf(enfants[0]).unwrap().get_view_type(), "markdown");
}

#[test]
fn un_workspace_json_corrompu_ne_bloque_pas_le_demarrage() {
    let banc = lancer(&LayoutMemoire::avec(r#"{"version":1,"main":{"type":"spl"#));
    let mut cx = TestAppContext::single();
    let (_racine, _vcx) = fenetre(&mut cx, &banc);
    // Le layout par défaut, exactement comme un coffre neuf : une pile, à l'écran.
    assert_eq!(banc.piles_du_centre().len(), 1);
    let centre = banc.piles_du_centre()[0];
    assert!(banc.ctx.disposition().cadre(centre).is_some());
}

// ─── resize-gesture.spec.ts : le gel du geste, règle du maximum ───

/// Une note au centre, une conversation dans le dock droit déplié.
fn note_et_chat() -> Banc {
    lancer(&layout(
        json!("note"),
        vec![pile(0, vec![feuille("note", "markdown")])],
        Some(pile(0, vec![feuille("chat", "chat")])),
    ))
}

/// La poignée du dock droit : sa frontière (1280 − 261 de dock).
const POIGNEE_DOCK_DROIT: f64 = 1019.0;

#[test]
fn dock_vers_l_exterieur_le_chat_qui_grandit_suit_en_direct_le_centre_qui_retrecit_est_rogne() {
    let banc = note_et_chat();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let (note, chat) = (banc.feuille_par_id("note"), banc.feuille_par_id("chat"));
    let note_avant = banc.largeur_vue(note);

    appuyer(vcx, POIGNEE_DOCK_DROIT, 400.0);
    assert!(banc.ctx.geste().is_some());
    pas(vcx, POIGNEE_DOCK_DROIT, 400.0, 4, -10.0);
    // Le dock a grandi de 40 px vers l'extérieur.
    assert_eq!(banc.largeur_vue(chat), 300.0);
    // Le chat (qui grandit) suit en direct ; le centre (qui rétrécit) garde sa boîte : rogné.
    assert_eq!(banc.largeur_figee(chat), banc.largeur_vue(chat));
    assert_eq!(banc.largeur_figee(note), note_avant);
    assert!(banc.largeur_figee(note) > banc.largeur_vue(note));
    relacher(vcx, POIGNEE_DOCK_DROIT - 40.0, 400.0);
    // Au relâché tout est libéré, et la largeur est dans le modèle.
    assert!(banc.ctx.boite_figee(note).is_none());
    let droit = banc.ws().right_split();
    assert_eq!(
        banc.ws().arbre().sidedock(droit).unwrap().width,
        Some(300.0)
    );
}

#[test]
fn aller_retour_en_un_geste_au_retour_les_deux_cotes_sont_rognes_aucun_trou_aucun_reflux() {
    let banc = note_et_chat();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let (note, chat) = (banc.feuille_par_id("note"), banc.feuille_par_id("chat"));

    appuyer(vcx, POIGNEE_DOCK_DROIT, 400.0);
    pas(vcx, POIGNEE_DOCK_DROIT, 400.0, 4, -10.0);
    let aller = (banc.largeur_figee(note), banc.largeur_figee(chat));
    pas(vcx, POIGNEE_DOCK_DROIT - 40.0, 400.0, 2, 10.0);
    // Au retour : les deux côtés reviennent dans une zone que leur contenu couvre déjà. Rien ne
    // reflue (les boîtes figées n'ont pas bougé), et chaque contenu couvre sa feuille (aucun trou).
    assert_eq!((banc.largeur_figee(note), banc.largeur_figee(chat)), aller);
    for leaf in [note, chat] {
        assert!(banc.largeur_figee(leaf) >= banc.largeur_vue(leaf));
        assert!(!banc.ctx.est_trou(leaf));
    }
    assert!(
        banc.largeur_figee(chat) > banc.largeur_vue(chat),
        "le chat est rogné"
    );
    relacher(vcx, POIGNEE_DOCK_DROIT - 20.0, 400.0);
}

/// Un pas du geste qui fait refluer une vue CHÈRE : l'image qui suit le reflux arrive 45 ms plus tard
/// (au-delà du budget de 20 ms).
fn pas_cher(vcx: &mut VisualTestContext, banc: &Banc, x: f64, y: f64) {
    vcx.simulate_mouse_move(
        point(px(x as f32), px(y as f32)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    banc.avancer(45.0);
    image(vcx);
    banc.avancer(5.0);
    image(vcx);
}

#[test]
fn une_pause_comble_les_trous_la_feuille_marquee_rattrape_sa_place_le_reste_ne_bouge_pas() {
    let banc = note_et_chat();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let (note, chat) = (banc.feuille_par_id("note"), banc.feuille_par_id("chat"));

    appuyer(vcx, POIGNEE_DOCK_DROIT, 400.0);
    pas_cher(vcx, &banc, POIGNEE_DOCK_DROIT - 10.0, 400.0);
    assert!(
        banc.ctx.est_trou(chat),
        "le reflux du chat a dépassé le budget"
    );
    banc.avancer(5.0);
    tirer(vcx, POIGNEE_DOCK_DROIT - 30.0, 400.0);
    assert!(
        banc.largeur_figee(chat) < banc.largeur_vue(chat),
        "trou : il ne suit plus"
    );
    let note_figee = banc.largeur_figee(note);

    // La pause : 100 ms sans mouvement, constatées à une image qui suit une image courte.
    banc.avancer(130.0);
    image(vcx);
    banc.avancer(10.0);
    image(vcx);
    assert_eq!(
        banc.largeur_figee(chat),
        banc.largeur_vue(chat),
        "le trou est comblé"
    );
    assert_eq!(
        banc.largeur_figee(note),
        note_figee,
        "le reste ne bouge pas"
    );
    relacher(vcx, POIGNEE_DOCK_DROIT - 30.0, 400.0);
}

#[test]
fn split_note_pdf_le_pdf_qui_grandit_reflue_en_direct_la_note_est_rognee_au_retour_plus_rien() {
    let banc = lancer(&layout(
        json!("note"),
        vec![
            pile(0, vec![feuille("note", "markdown")]),
            pile(0, vec![feuille("pdf", "pdf")]),
        ],
        None,
    ));
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let (note, pdf) = (banc.feuille_par_id("note"), banc.feuille_par_id("pdf"));
    let poignee = banc.ctx.disposition().poignees[0].trait_;
    let x = poignee.x + 0.5;
    let note_avant = banc.largeur_vue(note);

    appuyer(vcx, x, 400.0);
    assert!(matches!(
        banc.ctx.geste(),
        Some(fragment_ui::splits::Geste::Split { .. })
    ));
    pas(vcx, x, 400.0, 4, -10.0);
    assert_eq!(
        banc.largeur_figee(pdf),
        banc.largeur_vue(pdf),
        "le PDF suit"
    );
    assert_eq!(banc.largeur_figee(note), note_avant, "la note est rognée");
    let aller = (banc.largeur_figee(note), banc.largeur_figee(pdf));
    pas(vcx, x - 40.0, 400.0, 4, 10.0);
    assert_eq!(
        (banc.largeur_figee(note), banc.largeur_figee(pdf)),
        aller,
        "au retour plus rien"
    );
    relacher(vcx, x, 400.0);
    assert!(banc.ctx.boite_figee(pdf).is_none());
}

#[test]
fn garde_fou_une_feuille_dont_le_reflux_depasse_le_budget_ne_suit_plus_trou_jusqu_au_relache() {
    let banc = note_et_chat();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let chat = banc.feuille_par_id("chat");

    appuyer(vcx, POIGNEE_DOCK_DROIT, 400.0);
    pas_cher(vcx, &banc, POIGNEE_DOCK_DROIT - 10.0, 400.0);
    assert!(banc.ctx.est_trou(chat));
    let figee = banc.largeur_figee(chat);
    // Des pas rapides (sans pause) : le trou ne suit plus.
    for i in 2..=5 {
        banc.avancer(8.0);
        tirer(vcx, POIGNEE_DOCK_DROIT - 10.0 * i as f64, 400.0);
    }
    assert!(banc.ctx.est_trou(chat));
    assert_eq!(banc.largeur_figee(chat), figee);
    assert!(banc.largeur_vue(chat) > figee);
    // Au relâché, tout est libéré.
    relacher(vcx, POIGNEE_DOCK_DROIT - 50.0, 400.0);
    assert!(!banc.ctx.est_trou(chat));
    assert!(banc.ctx.boite_figee(chat).is_none());
}

// ─── tab-drop-frame-space.spec.ts ───

/// Le trou entre la dernière pile du centre et le bord droit de la fenêtre.
fn trou_du_centre(banc: &Banc) -> f64 {
    let d = banc.ctx.disposition();
    let fin = banc
        .piles_du_centre()
        .iter()
        .filter_map(|&p| d.cadre(p))
        .map(|c| c.droite())
        .fold(0.0, f64::max);
    1280.0 - fin
}

#[test]
fn restaurer_un_layout_a_dimension_inferieure_a_1_ne_laisse_pas_de_trou() {
    let banc = lancer(&layout(
        json!(null),
        vec![avec_dimension(
            pile(0, vec![feuille("leaf-0", "markdown")]),
            0.95,
        )],
        None,
    ));
    let mut cx = TestAppContext::single();
    let (_racine, _vcx) = fenetre(&mut cx, &banc);
    assert!(trou_du_centre(&banc) <= 1.0);
}

#[test]
fn un_merge_dont_le_survivant_a_un_flex_grow_inferieur_a_1_renormalise_pas_de_trou() {
    let banc = deux_panes();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let [gauche, droite] = banc.piles_du_centre()[..] else {
        panic!("deux piles attendues");
    };
    banc.ws().poser_grow(gauche, 0.5);
    peindre(vcx);
    // On vide la pile DROITE dans la GAUCHE (au centre de son corps) : gauche survit, seule.
    let b = corps(&banc, gauche);
    glisser_onglet(
        vcx,
        &banc,
        droite,
        0,
        b.x + b.largeur / 2.0,
        b.y + b.hauteur / 2.0,
    );
    assert_eq!(banc.piles_du_centre().len(), 1);
    assert!(trou_du_centre(&banc) <= 1.0);
}

// ─── Les poignées et les docks au rendu ───

#[test]
fn tirer_la_poignee_d_un_split_transfere_la_proportion_et_la_sauvegarde_la_garde() {
    let store = layout(
        json!("a"),
        vec![
            pile(0, vec![feuille("a", "markdown")]),
            pile(0, vec![feuille("b", "markdown")]),
        ],
        None,
    );
    let banc = lancer(&store);
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let [a, b] = banc.piles_du_centre()[..] else {
        panic!("deux piles attendues");
    };
    appuyer(vcx, 658.5, 400.0);
    pas(vcx, 658.5, 400.0, 5, -20.0);
    relacher(vcx, 558.5, 400.0);
    let d = banc.ctx.disposition();
    assert_eq!(d.cadre(a).unwrap().largeur, 522.0, "100 px de moins");
    let somme = {
        let w = banc.ws();
        w.arbre().grow(a) + w.arbre().grow(b)
    };
    assert!((somme - 2.0).abs() < 1e-9);
    // La proportion est sauvée au relâché et revient au redémarrage.
    assert!(banc.ws().save_layout().unwrap());
    let grow = banc.ws().arbre().grow(a);
    drop(d);
    let banc2 = lancer(&store);
    let a2 = banc2.piles_du_centre()[0];
    assert!((banc2.ws().arbre().grow(a2) - grow).abs() < 1e-9);
}

#[test]
fn la_case_du_ruban_replie_et_deplie_le_dock_gauche() {
    let banc = lancer(&layout(
        json!("a"),
        vec![pile(0, vec![feuille("a", "markdown")])],
        None,
    ));
    let gauche = banc.ws().get_left_leaf().unwrap();
    banc.ws()
        .set_view_state(gauche, fragment_core::ViewState::new("chat"), None);
    let dock = banc.ws().left_split();
    banc.ws().expand(dock);
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let d = banc.ctx.disposition();
    let g = d.dock(fragment_core::Cote::Gauche).unwrap();
    // 36 de ruban, 260 de dock, 1 de bordure : le centre commence à 297.
    assert_eq!(g.boite, fragment_core::Cadre::new(36.0, 0.0, 261.0, 800.0));
    assert_eq!(d.cadre(banc.piles_du_centre()[0]).unwrap().x, 297.0);
    clic(vcx, 18.0, 18.0);
    assert!(!banc.ws().arbre().dock_ouvert(dock));
    assert_eq!(
        banc.ctx
            .disposition()
            .cadre(banc.piles_du_centre()[0])
            .unwrap()
            .x,
        36.0
    );
    clic(vcx, 18.0, 18.0);
    assert!(banc.ws().arbre().dock_ouvert(dock));
}

#[test]
fn un_appui_dans_un_dock_rend_sa_feuille_active() {
    let banc = note_et_chat();
    let mut cx = TestAppContext::single();
    let (_racine, vcx) = fenetre(&mut cx, &banc);
    let chat = banc.feuille_par_id("chat");
    appuyer(vcx, 1150.0, 400.0);
    relacher(vcx, 1150.0, 400.0);
    assert_eq!(banc.ws().active_leaf(), Some(chat));
}

#[test]
fn l_action_layout_des_scenes_charge_l_arbre_avec_ses_docks() {
    let banc = lancer(&LayoutMemoire::new());
    let commandes = banc.app.commands();
    let _abonnement = fragment_ui::splits::brancher_scene(&commandes, banc.app.workspace());
    commandes.events().trigger(
        "pixels:layout",
        &json!({
            "main": {"type": "split", "direction": "vertical", "children": [
                {"type": "tabs", "tabs": 2, "current": 1}, {"type": "tabs", "tabs": 1}
            ]},
            "right": {"collapsed": false, "tabs": 2}
        }),
    );
    let mut cx = TestAppContext::single();
    let (_racine, _vcx) = fenetre(&mut cx, &banc);
    assert_eq!(banc.piles_du_centre().len(), 2);
    let d = banc.ctx.disposition();
    assert_eq!(d.piles.len(), 3, "deux au centre, une au dock droit");
    assert!(d.dock(fragment_core::Cote::Droite).is_some());
}
