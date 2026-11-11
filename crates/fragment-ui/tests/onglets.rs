//! Tests de la barre d'onglets, des en-têtes et de l'activation au pointeur (tâche `fr-t1-onglets`).
//!
//! Les quatre premiers portent le nom des quatre `test(...)` de `app/e2e/activeleaf-tracking.spec.ts`
//! (en snake_case, sans accents) et rejouent leur scénario sur le vrai rendu : une fenêtre de test
//! GPUI (sans GPU), des appuis de souris simulés aux coordonnées des en-têtes, le Workspace du cœur
//! derrière. Les vues sont des vues factices : elles retiennent une lecture et posent Mod+F dans leur
//! portée (le Ctrl+F de la note), ce qui prouve que la portée du CLAVIER suit la feuille active.
//!
//! Les positions viennent des CSS de Fragment (`docs/constantes-rendu.md`) : la barre fait 36 px, le
//! premier en-tête commence à x = 4 et mesure 206 × 32 (centre : 107, 20), les suivants viennent bout
//! à bout.

// Fichier de test : les assistants hors `#[test]` (banc, vues factices) déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::{Cell, RefMut};
use std::rc::Rc;
use std::sync::Arc;

use fragment_core::keymap::types::Modifier;
use fragment_core::{
    App, AppCoeur, ContexteVue, EmptyView, ItemId, ItemViewBase, KeyEvent, LayoutMemoire, Scope,
    View, ViewCreator, ViewState, Workspace, enregistrer_commandes_app,
    enregistrer_commandes_workspace,
};
use fragment_ui::entete::a_un_entete;
use fragment_ui::menu;
use fragment_ui::onglets::actions::{self, lignes_de_la_pile};
use fragment_ui::onglets::{ContexteOnglets, OptionsPile, brancher_scene, rendre_pile};
use fragment_ui::polices;
use gpui_kit::{
    AnyElement, Context, Entity, Hsla, IntoElement, Modifiers, MouseButton, ParentElement as _,
    Render, Styled as _, TestAppContext, TextRun, TextSystem, VisualTestContext, Window,
    WindowTextSystem, div, point, px,
};
use serde_json::{Value, json};

// ─── Une vue factice ───

struct VueTest {
    type_: String,
    base: ItemViewBase,
    /// Combien de fois Mod+F a été reçu par la portée de cette vue.
    recherches: Rc<Cell<u32>>,
}

impl VueTest {
    fn new(type_: &str, app_scope: &Scope) -> Self {
        let base = ItemViewBase::new(app_scope);
        let recherches = Rc::new(Cell::new(0));
        let r = recherches.clone();
        base.scope
            .register(Some(&[Modifier::Mod]), Some("f"), move |_, _| {
                r.set(r.get() + 1);
                Some(false)
            });
        Self {
            type_: type_.to_owned(),
            base,
            recherches,
        }
    }
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
    Rc::new(move |ctx: &ContexteVue| Box::new(VueTest::new(type_, &ctx.app_scope)) as Box<dyn View>)
}

// ─── Le banc : une session de l'application, branchée sur le rendu ───

struct Banc {
    app: Rc<AppCoeur>,
    ctx: ContexteOnglets,
    _retraits: Vec<Box<dyn FnOnce()>>,
}

fn feuille(id: &str, type_: &str) -> Value {
    json!({"type": "leaf", "id": id, "state": {"type": type_, "state": {}}})
}

fn pile(courant: usize, feuilles: Vec<Value>) -> Value {
    json!({"type": "tabs", "currentTab": courant, "children": feuilles})
}

fn split(direction: &str, enfants: Vec<Value>) -> Value {
    json!({"type": "split", "direction": direction, "children": enfants})
}

/// Un layout : la pile centrale, et éventuellement une pile dans le dock droit (déplié).
fn layout(active: Value, centre: Value, droite: Option<Value>) -> LayoutMemoire {
    let mut dock = split("horizontal", droite.map(|p| vec![p]).unwrap_or_default());
    dock["collapsed"] = json!(false);
    LayoutMemoire::avec(
        &json!({
            "version": 1,
            "active": active,
            "left": split("horizontal", vec![]),
            "right": dock,
            "main": split("vertical", vec![centre]),
        })
        .to_string(),
    )
}

fn lancer(store: &LayoutMemoire) -> Banc {
    let app = AppCoeur::new();
    let mut retraits: Vec<Box<dyn FnOnce()>> = Vec::new();
    for t in ["markdown", "chat"] {
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
    let ctx = ContexteOnglets {
        workspace: app.workspace().clone(),
        commandes: app.commands(),
        keymap: app.keymap().clone(),
        contenu_de_vue: None,
    };
    Banc {
        app,
        ctx,
        _retraits: retraits,
    }
}

impl Banc {
    fn ws(&self) -> RefMut<'_, Workspace> {
        self.app.workspace().borrow_mut()
    }

    /// Le type de la vue de la feuille active (celle qu'elle VEUT montrer).
    fn type_actif(&self) -> Option<String> {
        let w = self.ws();
        let a = w.active_leaf()?;
        Some(w.leaf(a)?.get_view_type().to_owned())
    }

    fn id_actif(&self) -> Option<String> {
        let w = self.ws();
        let a = w.active_leaf()?;
        Some(w.leaf(a)?.id.clone())
    }

    fn pile_centrale(&self) -> ItemId {
        let w = self.ws();
        w.arbre().piles_sous(w.root_split())[0]
    }

    fn pile_droite(&self) -> ItemId {
        let w = self.ws();
        w.arbre().piles_sous(w.right_split())[0]
    }

    fn ctrl_f(&self) -> Option<bool> {
        let mut evt = KeyEvent::new("f");
        evt.mods.ctrl = !cfg!(target_os = "macos");
        evt.mods.meta = cfg!(target_os = "macos");
        self.app.keymap().handle_key_event(&evt)
    }

    /// Combien de Mod+F la vue de cette feuille a reçus.
    fn recherches(&self, leaf: ItemId) -> u32 {
        let w = self.ws();
        w.leaf(leaf)
            .and_then(|l| l.view().as_any().downcast_ref::<VueTest>())
            .map_or(0, |v| v.recherches.get())
    }
}

// ─── La fenêtre de test : des piles côte à côte et la couche des menus ───

struct Racine {
    ctx: ContexteOnglets,
    piles: Vec<(ItemId, OptionsPile)>,
}

impl Render for Racine {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colonnes: Vec<AnyElement> = self
            .piles
            .iter()
            .map(|(p, o)| rendre_pile(&self.ctx, *p, *o, cx))
            .collect();
        div()
            .size_full()
            .flex()
            .flex_row()
            .children(colonnes)
            .child(menu::couche(cx))
    }
}

fn peindre(vcx: &mut VisualTestContext) {
    // Deux images : la première pose les cibles de la souris, la seconde les rend atteignables.
    for _ in 0..2 {
        vcx.update(|w, cx| {
            w.draw(cx).clear(cx);
        });
    }
}

fn fenetre<'a>(
    cx: &'a mut TestAppContext,
    banc: &Banc,
    piles: Vec<ItemId>,
) -> (Entity<Racine>, &'a mut VisualTestContext) {
    let ctx = banc.ctx.clone();
    let (racine, vcx) = cx.add_window_view(|_, _| Racine {
        ctx,
        piles: piles
            .into_iter()
            .map(|p| (p, OptionsPile::default()))
            .collect(),
    });
    peindre(vcx);
    (racine, vcx)
}

/// Un appui de la souris (gauche) à `(x, y)`, puis un nouveau dessin.
fn appuyer(vcx: &mut VisualTestContext, x: f32, y: f32) {
    vcx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::default());
    peindre(vcx);
}

/// Le centre de l'en-tête du `rang`-ième onglet (le premier commence à x = 4, chacun mesure 206).
fn centre_entete(rang: usize) -> (f32, f32) {
    (4.0 + 206.0 * rang as f32 + 103.0, 20.0)
}

// ─── Les quatre cas de `activeleaf-tracking.spec.ts` ───

#[test]
fn fermer_l_onglet_actif_reassigne_active_leaf_a_l_onglet_visible() {
    let banc = lancer(&layout(
        json!("c"),
        pile(
            2,
            vec![
                feuille("a", "markdown"),
                feuille("b", "markdown"),
                feuille("c", "markdown"),
            ],
        ),
        None,
    ));
    assert_eq!(banc.id_actif().as_deref(), Some("c"));

    // Fermer l'onglet ACTIF.
    let c = banc.ws().active_leaf().unwrap();
    actions::fermer_onglet(&mut banc.ws(), c);

    // ★ plus jamais null tant qu'un onglet reste, et c'est l'onglet redevenu visible.
    let apres = banc.ws().active_leaf();
    assert!(apres.is_some());
    assert_eq!(banc.type_actif().as_deref(), Some("markdown"));
    let pile = banc.pile_centrale();
    assert_eq!(banc.ws().arbre().active_leaf(pile), apres);

    // Le payoff : une commande « sur la vue active » (ici, son raccourci) marche de nouveau.
    assert_eq!(banc.ctrl_f(), Some(false));
    assert_eq!(banc.recherches(apres.unwrap()), 1);
}

#[test]
fn presser_un_onglet_le_rend_actif_activation_sur_pointerdown() {
    let banc = lancer(&layout(
        json!("b"),
        pile(1, vec![feuille("a", "markdown"), feuille("b", "markdown")]),
        None,
    ));
    assert_eq!(banc.id_actif().as_deref(), Some("b"));

    // L'activation vit sur l'APPUI de l'en-tête : un appui de souris simulé, sans relâchement.
    let mut cx = TestAppContext::single();
    let pile = banc.pile_centrale();
    let (_racine, vcx) = fenetre(&mut cx, &banc, vec![pile]);
    let (x, y) = centre_entete(0);
    appuyer(vcx, x, y);

    assert_eq!(banc.id_actif().as_deref(), Some("a"));
    assert_eq!(banc.ws().arbre().active_index(pile), Some(0));
}

#[test]
fn au_demarrage_active_leaf_tombe_sur_la_vue_principale_jamais_null_repli_de_restauration() {
    let store = layout(json!("a"), pile(0, vec![feuille("a", "markdown")]), None);
    {
        // Reproduit l'état HÉRITÉ du bug : l'active était nulle au moment de sauver, donc
        // workspace.json est écrit avec `active: null`.
        let banc = lancer(&store);
        banc.ws().set_active_leaf(None);
        assert!(banc.ws().save_layout().unwrap());
    }
    let ecrit: Value = serde_json::from_str(&store.contenu().unwrap()).unwrap();
    assert_eq!(ecrit["active"], Value::Null);

    // Redémarrage : la restauration relit workspace.json.
    let banc = lancer(&store);
    // ★ le repli a activé la vue principale…
    assert_eq!(banc.type_actif().as_deref(), Some("markdown"));
    // … et une commande « vue active » marche direct.
    assert_eq!(banc.ctrl_f(), Some(false));
}

#[test]
fn un_clic_ou_un_focus_dans_une_feuille_la_rend_active_pas_seulement_son_onglet_les_raccourcis_de_document_suivent()
 {
    let banc = lancer(&layout(
        json!("note"),
        pile(0, vec![feuille("note", "markdown")]),
        Some(pile(0, vec![feuille("fil", "chat")])),
    ));
    let mut cx = TestAppContext::single();
    let (centre, droite) = (banc.pile_centrale(), banc.pile_droite());
    let (_racine, vcx) = fenetre(&mut cx, &banc, vec![centre, droite]);
    let largeur = f32::from(vcx.update(|w, _| w.viewport_size().width));

    // Le chat (dock droit) prend la main.
    let fil = banc.ws().arbre().active_leaf(droite).unwrap();
    banc.ws().set_active_leaf(Some(fil));
    assert_eq!(banc.type_actif().as_deref(), Some("chat"));

    // Clic dans le CORPS de la note (pas son onglet) : la note redevient active.
    appuyer(vcx, 100.0, 300.0);
    assert_eq!(banc.type_actif().as_deref(), Some("markdown"));

    // Ses raccourcis de document marchent.
    let note = banc.ws().active_leaf().unwrap();
    assert_eq!(banc.ctrl_f(), Some(false));
    assert_eq!(banc.recherches(note), 1);

    // Un appui dans le corps du chat : le chat redevient actif, et Mod+F n'arrive plus à la note.
    appuyer(vcx, largeur * 0.75, 300.0);
    assert_eq!(banc.type_actif().as_deref(), Some("chat"));
    banc.ctrl_f();
    assert_eq!(banc.recherches(note), 1);
    assert_eq!(banc.recherches(fil), 1);
}

// ─── Le reste de la barre ───

#[test]
fn la_croix_de_l_onglet_actif_le_ferme_sans_activer_son_voisin_avant_de_partir() {
    let banc = lancer(&layout(
        json!("b"),
        pile(
            1,
            vec![
                feuille("a", "markdown"),
                feuille("b", "markdown"),
                feuille("c", "markdown"),
            ],
        ),
        None,
    ));
    let mut cx = TestAppContext::single();
    let pile = banc.pile_centrale();
    let (_racine, vcx) = fenetre(&mut cx, &banc, vec![pile]);

    // La croix de l'actif (2e onglet) : à 6 px du bord droit de l'en-tête (3 + 3), carrée de 18.
    let (x, y) = centre_entete(1);
    vcx.simulate_click(
        point(px(x + 103.0 - 6.0 - 9.0), px(y)),
        Modifiers::default(),
    );
    peindre(vcx);

    let lignes = lignes_de_la_pile(&banc.ws(), pile);
    assert_eq!(lignes.len(), 2, "l'onglet est fermé");
    // L'ancien 3e prend la place du 2e et devient l'actif (le relais de la fermeture).
    assert_eq!(banc.id_actif().as_deref(), Some("c"));
    assert!(lignes[1].visible);
}

#[test]
fn le_bouton_plus_ajoute_un_onglet_vide_a_la_fin_et_l_active() {
    let banc = lancer(&layout(
        json!("a"),
        pile(0, vec![feuille("a", "markdown"), feuille("b", "markdown")]),
        None,
    ));
    let mut cx = TestAppContext::single();
    let pile = banc.pile_centrale();
    let (_racine, vcx) = fenetre(&mut cx, &banc, vec![pile]);

    // Le « + » : après les 2 en-têtes (8 + 2 × 206 de zone intérieure), moins 1 de marge, 12 de
    // demi-case.
    let x = 8.0 + 2.0 * 206.0 - 1.0 + 12.0;
    vcx.simulate_click(point(px(x), px(20.0)), Modifiers::default());
    peindre(vcx);

    let lignes = lignes_de_la_pile(&banc.ws(), pile);
    assert_eq!(lignes.len(), 3);
    assert!(lignes[2].visible, "le nouvel onglet est visible");
    assert_eq!(banc.ws().active_leaf(), Some(lignes[2].leaf), "et actif");
    assert_eq!(lignes[2].titre, "New Tab");
}

#[test]
fn le_clic_droit_sur_un_onglet_ouvre_le_menu_fermer() {
    let banc = lancer(&layout(
        json!("a"),
        pile(0, vec![feuille("a", "markdown"), feuille("b", "markdown")]),
        None,
    ));
    let mut cx = TestAppContext::single();
    let pile = banc.pile_centrale();
    let (_racine, vcx) = fenetre(&mut cx, &banc, vec![pile]);

    let (x, y) = centre_entete(1);
    vcx.simulate_mouse_down(
        point(px(x), px(y)),
        MouseButton::Right,
        Modifiers::default(),
    );
    peindre(vcx);
    assert!(menu::menu_ouvert());

    // « Fermer » est la seule entrée : elle ferme l'onglet cliqué (le 2e), pas l'actif.
    assert!(menu::cliquer(0, 0));
    assert!(!menu::menu_ouvert());
    let lignes = lignes_de_la_pile(&banc.ws(), pile);
    assert_eq!(lignes.len(), 1);
    assert_eq!(banc.id_actif().as_deref(), Some("a"));
}

#[test]
fn la_pile_liste_les_onglets_dans_l_ordre_avec_titre_et_visibilite() {
    let banc = lancer(&layout(
        json!("b"),
        pile(
            1,
            vec![
                feuille("a", "markdown"),
                feuille("b", "chat"),
                feuille("c", "markdown"),
            ],
        ),
        None,
    ));
    let pile = banc.pile_centrale();
    let lignes = lignes_de_la_pile(&banc.ws(), pile);
    let titres: Vec<&str> = lignes.iter().map(|l| l.titre.as_str()).collect();
    // Les coquilles différées se nomment d'après leur type demandé ; la visible est montée.
    assert_eq!(titres, ["markdown", "chat", "markdown"]);
    assert_eq!(
        lignes.iter().map(|l| l.visible).collect::<Vec<_>>(),
        [false, true, false]
    );
}

#[test]
fn le_bouton_du_dock_droit_donne_une_pile_au_dock_vide_puis_le_replie() {
    let banc = lancer(&layout(
        json!("a"),
        pile(0, vec![feuille("a", "markdown")]),
        None,
    ));
    let droit = banc.ws().right_split();
    // Déplié mais vide : traité comme fermé.
    assert!(banc.ws().arbre().children(droit).is_empty());
    actions::basculer_dock_droit(&mut banc.ws());
    assert!(banc.ws().arbre().dock_ouvert(droit));
    assert!(!banc.ws().arbre().children(droit).is_empty());
    // Ouvert et non vide : le second appui le replie.
    actions::basculer_dock_droit(&mut banc.ws());
    assert!(!banc.ws().arbre().dock_ouvert(droit));
}

// ─── L'en-tête de vue ───

#[test]
fn l_entete_n_existe_ni_pour_la_vue_vide_neuve_ni_pour_une_coquille_differee() {
    let banc = lancer(&layout(
        json!("b"),
        pile(1, vec![feuille("a", "markdown"), feuille("b", "markdown")]),
        None,
    ));
    let pile = banc.pile_centrale();
    // La feuille cachée est une coquille différée : pas d'en-tête.
    let cachee = banc.ws().arbre().children(pile)[0];
    assert!(!a_un_entete(banc.ws().leaf(cachee).unwrap().view()));
    // La visible est montée : un en-tête.
    let visible = banc.ws().arbre().children(pile)[1];
    assert!(a_un_entete(banc.ws().leaf(visible).unwrap().view()));

    // Une feuille neuve porte une vue vide jamais montée : un panneau blanc, sans en-tête.
    let neuve = actions::nouvel_onglet(&mut banc.ws(), pile).unwrap();
    assert!(!a_un_entete(banc.ws().leaf(neuve).unwrap().view()));
    // `set_view_state` la monte : en-tête et message.
    banc.ws()
        .set_view_state(neuve, ViewState::new("empty"), None);
    let w = banc.ws();
    let vue = w.leaf(neuve).unwrap().view();
    assert!(a_un_entete(vue));
    assert_eq!(
        vue.as_any().downcast_ref::<EmptyView>().unwrap().message(),
        "Aucun fichier ouvert"
    );
}

// ─── Les scènes de pixels ───

#[test]
fn les_evenements_de_scene_epinglent_et_montent_la_vue_vide() {
    let banc = lancer(&layout(
        json!("a"),
        pile(0, vec![feuille("a", "markdown"), feuille("b", "markdown")]),
        None,
    ));
    let commandes = banc.app.commands();
    fragment_ui::onglets::commandes_de_scene(&commandes);
    let _abonnements = brancher_scene(&commandes, banc.app.workspace());

    // `pixels:args` précède la commande, comme le fait le lecteur de scènes.
    commandes
        .events()
        .trigger("pixels:args", &json!({"onglet": 1}));
    assert!(commandes.execute_command_by_id("pixels:epingler", None));
    let pile = banc.pile_centrale();
    let b = banc.ws().arbre().children(pile)[1];
    assert!(banc.ws().leaf(b).unwrap().pinned());
    let a = banc.ws().arbre().children(pile)[0];
    assert!(!banc.ws().leaf(a).unwrap().pinned());

    commandes
        .events()
        .trigger("pixels:args", &json!({"onglet": 1}));
    assert!(commandes.execute_command_by_id("pixels:monter-vue", None));
    let w = banc.ws();
    let vue = w.leaf(b).unwrap().view();
    assert!(vue.as_any().downcast_ref::<EmptyView>().is_some());
    assert!(a_un_entete(vue));
}

#[test]
fn les_commandes_de_scene_sont_enregistrees() {
    let banc = lancer(&LayoutMemoire::new());
    let commandes = banc.app.commands();
    fragment_ui::onglets::commandes_de_scene(&commandes);
    for id in ["pixels:epingler", "pixels:monter-vue"] {
        assert!(commandes.find_command(id).is_some(), "{id} absente");
    }
}

// ─── Risque 3 de T1 : la chasse de DM Sans sur un titre d'onglet ───

/// ★ Première mesure (risque 3 de `docs/tranches/T1.md`) : « New Tab » en DM Sans 400 à 12 px.
/// Sur Fragment (Chromium, DM Sans variable à axe optique), l'ENCRE du titre de l'onglet actif va de x = 258
/// à x = 304 (47 px, mesurés sur `t1-onglets-dark.png`) ; la chasse (avance) attendue est donc d'environ
/// 47 à 48 px. Ici, l'instance statique 400 (ADR-005) sur le système de texte de la plateforme : la mesure
/// est imprimée (`cargo test -p fragment-ui --test onglets chasse -- --nocapture`) et bornée à 46-48,5 px (crénage de polices::interface).
#[test]
fn la_chasse_de_dm_sans_sur_un_titre_d_onglet_reste_a_deux_pixels_de_chromium() {
    use gpui_kit::platform::current_platform;
    let systeme = Arc::new(TextSystem::new(current_platform(false).text_system()));
    polices::enregistrer(&systeme).expect("polices enregistrées");
    let fenetre_texte = WindowTextSystem::new(systeme);
    let titre = "New Tab";
    let run = TextRun {
        len: titre.len(),
        font: polices::interface(gpui_kit::FontWeight::NORMAL),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let ligne = fenetre_texte.shape_line(titre.into(), px(12.0), &[run], None);
    let largeur = f32::from(ligne.width());
    eprintln!(
        "chasse DM Sans 400, 12 px, « {titre} » : {largeur:.2} px (Chromium : encre de 47 px)"
    );
    assert!(
        (46.0..=48.5).contains(&largeur),
        "chasse de « {titre} » = {largeur} px, attendu 47 à 48"
    );
}
