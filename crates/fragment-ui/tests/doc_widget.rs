//! Tests du cadre DocWidget et du service `doc-widget:host@1` en mémoire (tâche `fr-t5-doc-widget`).
//!
//! Les cas portent le nom des tests de `doc-widget-views.spec.ts` (e2e) en snake_case, sur une
//! VUE de test écrite ici (jamais traduite du `.cjs` de Fragment : ADR-007) : deux icônes, un titre,
//! un mode « full » ou « pill » choisi à SES seuils sur la place reçue. Le handle de widget est un
//! faux qui garde ce que le cadre lui demande. Aucun test ne touche à GPUI : le modèle est pur.
//! Les trois cas de T6 (record au rechargement, tombe au journal, migration v1) : n.a. ici.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{App, AppCoeur, DocumentKey, ContexteVue, ItemId, NouvelleFeuille, View, ViewState};
use fragment_layers::overlay_host::Disposer;
use fragment_layers::types::{CompactInfo, GutterSide, LeafId};
use fragment_layers::widget_layer::{WidgetAnchor, WidgetBox, WidgetHandle};
use fragment_text::RangeHighlights;
use fragment_ui::doc_widget::cadre::{Ailleurs, DocWidget, OptionsCadre, OptionsOuverture};
use fragment_ui::doc_widget::icones_vue::{self, Motif, Pastille};
use fragment_ui::doc_widget::service::{Attente, FOCUS_VALABLE_MS};
use fragment_ui::doc_widget::{
    Crochets, DocWidgetAction, DocWidgetHost, DocWidgetOpen, DocWidgetPlacement, EvenementService,
    PlacementWidget, ServiceDocWidget, etat_d_ouverture, etat_de_place, place_de_l_etat,
};
use serde_json::{Map, Value, json};

const NOTE: &str = "article.md";
const TYPE_VUE: &str = "e2e-room-view";

// ─── La vue de test ───

/// Ce que la vue de test a reçu, partagé avec le test.
#[derive(Default)]
struct VueEtat {
    icones: RefCell<Vec<String>>,
    mode: RefCell<String>,
    placement: RefCell<Option<Value>>,
    focus: Cell<u32>,
    etats: RefCell<Vec<Value>>,
}

struct VueDeTest {
    etat: Rc<VueEtat>,
}

impl View for VueDeTest {
    fn get_view_type(&self) -> String {
        TYPE_VUE.to_owned()
    }
    fn get_display_text(&self) -> String {
        "Vue de test".to_owned()
    }
    fn get_icons(&self) -> Vec<String> {
        self.etat.icones.borrow().clone()
    }
    fn get_document(&self) -> Option<DocumentKey> {
        Some(DocumentKey::new("chat:test"))
    }
    /// Ses SEUILS à elle : au moins 200 px de place, la vue entière ; moins, une pastille.
    fn set_ephemeral_state(&mut self, state: &Value) {
        self.etat.etats.borrow_mut().push(state.clone());
        if let Some(room) = place_de_l_etat(state) {
            let mode = match room {
                Some(r) if r >= 200.0 => "full",
                Some(_) => "pill",
                None => "hidden",
            };
            *self.etat.mode.borrow_mut() = mode.to_owned();
        }
        if let Some(p) = state.get("docWidget") {
            *self.etat.placement.borrow_mut() = Some(p.clone());
        }
        if state.get("focus").and_then(Value::as_bool) == Some(true) {
            self.etat.focus.set(self.etat.focus.get() + 1);
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ─── Le faux handle de widget ───

#[derive(Default)]
struct FauxHandle {
    ancre: Cell<Option<WidgetAnchor>>,
    actif: Cell<bool>,
    manuel: Cell<Option<bool>>,
    feuille: Cell<Option<LeafId>>,
    compact: RefCell<Option<Rc<CompactInfo>>>,
    fits: Cell<bool>,
    retire: Cell<bool>,
    nb_set_compact: Cell<u32>,
    cb_actif: RefCell<Vec<Box<dyn Fn(bool)>>>,
    cb_room: RefCell<Vec<Box<dyn Fn(Option<f64>)>>>,
    cb_fit: RefCell<Vec<Box<dyn Fn(bool)>>>,
}

impl FauxHandle {
    fn new(ancre: WidgetAnchor) -> Rc<Self> {
        let h = Self::default();
        h.ancre.set(Some(ancre));
        h.fits.set(true);
        Rc::new(h)
    }
    fn emettre_room(&self, r: Option<f64>) {
        for cb in self.cb_room.borrow().iter() {
            cb(r);
        }
    }
    fn emettre_fit(&self, f: bool) {
        self.fits.set(f);
        for cb in self.cb_fit.borrow().iter() {
            cb(f);
        }
    }
    fn icones_compactes(&self) -> Option<Vec<String>> {
        self.compact.borrow().as_ref().map(|c| c.icons.clone())
    }
}

impl WidgetHandle for FauxHandle {
    fn get_anchor(&self) -> WidgetAnchor {
        self.ancre.get().unwrap()
    }
    fn set_anchor(&self, anchor: WidgetAnchor) {
        self.ancre.set(Some(anchor));
    }
    fn get_box(&self) -> Option<WidgetBox> {
        None
    }
    fn on_box_change(&self, _cb: Box<dyn Fn()>) -> Disposer {
        Box::new(|| {})
    }
    fn get_room(&self) -> Option<f64> {
        None
    }
    fn on_room_change(&self, cb: Box<dyn Fn(Option<f64>)>) -> Disposer {
        self.cb_room.borrow_mut().push(cb);
        Box::new(|| {})
    }
    fn is_active(&self) -> bool {
        self.actif.get()
    }
    fn on_active_change(&self, cb: Box<dyn Fn(bool)>) -> Disposer {
        self.cb_actif.borrow_mut().push(cb);
        Box::new(|| {})
    }
    fn set_active(&self, active: bool) {
        if self.actif.get() == active {
            return;
        }
        self.actif.set(active);
        for cb in self.cb_actif.borrow().iter() {
            cb(active);
        }
    }
    fn set_activation_manual(&self, manual: bool) {
        self.manuel.set(Some(manual));
    }
    fn set_activation_leaf(&self, leaf: Option<LeafId>) {
        self.feuille.set(leaf);
    }
    fn set_compact(&self, info: Option<Rc<CompactInfo>>) {
        if let (Some(a), Some(b)) = (self.compact.borrow().as_ref(), info.as_ref())
            && Rc::ptr_eq(a, b)
        {
            return; // l'identité compte : la même, répétée, ne fait rien
        }
        self.nb_set_compact.set(self.nb_set_compact.get() + 1);
        *self.compact.borrow_mut() = info;
    }
    fn fits_gutter(&self) -> bool {
        self.fits.get()
    }
    fn on_fit_change(&self, cb: Box<dyn Fn(bool)>) -> Disposer {
        self.cb_fit.borrow_mut().push(cb);
        Box::new(|| {})
    }
    fn remove(&self) {
        self.retire.set(true);
    }
}

// ─── Le décor ───

struct Decor {
    app: Rc<AppCoeur>,
    note: ItemId,
    vue: Rc<VueEtat>,
    surlignages: Rc<RefCell<RangeHighlights>>,
}

fn decor() -> Decor {
    let app = AppCoeur::new();
    let vue = Rc::new(VueEtat::default());
    *vue.icones.borrow_mut() = vec!["🙂".to_owned(), "message-circle".to_owned()];
    let etat = vue.clone();
    app.view_registry().register(
        TYPE_VUE,
        Rc::new(move |_: &ContexteVue| Box::new(VueDeTest { etat: etat.clone() }) as Box<dyn View>),
        "test",
    );
    let note = {
        let mut w = app.workspace().borrow_mut();
        let note = w.get_leaf(NouvelleFeuille::Non).unwrap();
        w.set_active_leaf(Some(note));
        note
    };
    Decor {
        app,
        note,
        vue,
        surlignages: Rc::new(RefCell::new(RangeHighlights::new())),
    }
}

/// Un widget NU ancré sur `[from, to)` de la marge droite, monté sur un faux handle.
fn widget_nu(d: &Decor, from: usize, to: usize) -> (Rc<DocWidget>, Rc<FauxHandle>) {
    widget(d, from, to, None)
}

fn widget(d: &Decor, from: usize, to: usize, taille: Option<(f64, f64)>) -> (Rc<DocWidget>, Rc<FauxHandle>) {
    let dw = DocWidget::new(
        d.app.workspace().clone(),
        OptionsCadre {
            non_groupable: false,
            hote: Some(d.note),
            surlignages: Some(d.surlignages.clone()),
        },
    );
    dw.poser_id("dw-1");
    let placement = DocWidgetPlacement { id: "dw-1".to_owned(), file: NOTE.to_owned(), from, to };
    let leaf = dw.ouvrir_vue(
        ViewState::new(TYPE_VUE),
        OptionsOuverture {
            w: taille.map(|t| t.0),
            h: taille.map(|t| t.1),
            open: Some(true),
            e_state: Some(placement.en_etat()),
        },
    );
    assert!(leaf.is_some());
    let h = FauxHandle::new(WidgetAnchor::Gutter {
        side: GutterSide::Right,
        pos: from,
        to: Some(to),
        dy: 0.0,
    });
    dw.bind(h.clone());
    (dw, h)
}

fn titres(dw: &DocWidget) -> Vec<String> {
    dw.actions_entete().into_iter().map(|a| a.titre).collect()
}

fn plages(d: &Decor) -> Vec<(usize, usize, String)> {
    d.surlignages
        .borrow()
        .get("doc-widget:dw-1")
        .iter()
        .map(|r| (r.from, r.to, r.cls.clone().unwrap_or_default()))
        .collect()
}

// ─── Le service : doc-widget:host@1 ───

fn service(ouvert: bool, t: Rc<Cell<f64>>) -> Rc<ServiceDocWidget> {
    ServiceDocWidget::new(move |_| ouvert, move || t.get())
}

#[test]
fn ouverte_par_le_service_ancree_au_passage_dans_la_marge_droite() {
    let s = service(true, Rc::new(Cell::new(0.0)));
    let id = s
        .open(DocWidgetOpen::new(ViewState::new(TYPE_VUE), NOTE, 12, 18))
        .unwrap();
    let liste = s.list(NOTE);
    assert_eq!(liste.len(), 1);
    assert_eq!((liste[0].id.as_str(), liste[0].from, liste[0].to), (id.as_str(), Some(12), Some(18)));
    let r = s.enregistrement(&id).unwrap();
    assert!(r.ouvert, "une vue qu'on vient de demander se montre");
    assert!(matches!(r.anchor, WidgetAnchor::Gutter { side: GutterSide::Right, pos: 12, to: Some(18), .. }));
    assert!(s.list("autre.md").is_empty());
}

#[test]
fn ouvrir_sans_document_hote_ouvert_rend_rien() {
    let s = service(false, Rc::new(Cell::new(0.0)));
    assert_eq!(s.open(DocWidgetOpen::new(ViewState::new(TYPE_VUE), NOTE, 0, 3)), None);
    assert!(s.list(NOTE).is_empty());
}

#[test]
fn le_passage_suit_les_editions_dans_list() {
    let s = service(true, Rc::new(Cell::new(0.0)));
    let id = s.open(DocWidgetOpen::new(ViewState::new(TYPE_VUE), NOTE, 12, 18)).unwrap();
    // Le widget monté tient l'ancre remappée : « Une ligne de plus.\n\n » (19 car.) au-dessus,
    // puis une frappe de 3 car. dans le mot.
    s.suivre_ancre(&id, || WidgetAnchor::Gutter { side: GutterSide::Right, pos: 31, to: Some(40), dy: 0.0 });
    let e = &s.list(NOTE)[0];
    assert_eq!((e.from, e.to), (Some(31), Some(40)));
    // Démonté (hors de la bande) : le record garde sa dernière ancre.
    s.oublier_ancre(&id);
    assert_eq!(s.list(NOTE)[0].from, Some(31));
}

#[test]
fn fermer_par_le_service_une_tombe_le_widget_part() {
    let s = service(true, Rc::new(Cell::new(0.0)));
    let vus = Rc::new(RefCell::new(Vec::new()));
    let v = vus.clone();
    s.on(move |e| v.borrow_mut().push(e.clone()));
    let id = s.open(DocWidgetOpen::new(ViewState::new(TYPE_VUE), NOTE, 1, 4)).unwrap();
    s.close(&id);
    assert!(s.enregistrement(&id).is_none());
    assert!(s.list(NOTE).is_empty());
    assert_eq!(
        *vus.borrow(),
        vec![EvenementService::Ouvert(id.clone()), EvenementService::Ferme(id)]
    );
}

#[test]
fn set_open_bascule_et_n_annonce_que_le_changement() {
    let s = service(true, Rc::new(Cell::new(0.0)));
    let vus = Rc::new(RefCell::new(Vec::new()));
    let v = vus.clone();
    let id = s.open(DocWidgetOpen::new(ViewState::new(TYPE_VUE), NOTE, 1, 4)).unwrap();
    s.on(move |e| v.borrow_mut().push(e.clone()));
    s.set_open(&id, true); // déjà ouvert : rien
    s.set_open(&id, false);
    assert!(!s.enregistrement(&id).unwrap().ouvert);
    s.reveal(&id);
    assert!(s.enregistrement(&id).unwrap().ouvert);
    assert_eq!(
        *vus.borrow(),
        vec![
            EvenementService::Bascule { id: id.clone(), ouvert: false },
            EvenementService::Revele(id),
        ]
    );
}

#[test]
fn le_focus_d_une_creation_n_est_valable_que_deux_secondes() {
    let mut e = Map::new();
    e.insert("focus".to_owned(), json!(true));
    e.insert("draft".to_owned(), json!("Bonjour"));
    let a = Attente { e_state: e, ne_ms: 1000.0 };
    // À l'écran (Ask, Entourer) : la construction suit la création de quelques ms.
    let tot = etat_d_ouverture(Some(&a), 1000.0 + 50.0).unwrap();
    assert_eq!(tot.get("focus"), Some(&json!(true)));
    // Construite des minutes plus tard (hors de la bande) : le focus est tombé, le brouillon reste.
    let tard = etat_d_ouverture(Some(&a), 1000.0 + FOCUS_VALABLE_MS + 1.0).unwrap();
    assert!(!tard.contains_key("focus"));
    assert_eq!(tard.get("draft"), Some(&json!("Bonjour")));
    assert_eq!(etat_d_ouverture(None, 5.0), None);
}

#[test]
fn l_etat_d_ouverture_est_livre_une_fois_au_montage() {
    let t = Rc::new(Cell::new(0.0));
    let s = service(true, t.clone());
    let mut o = DocWidgetOpen::new(ViewState::new(TYPE_VUE), NOTE, 1, 4);
    let mut e = Map::new();
    e.insert("send".to_owned(), json!("premier tour"));
    e.insert("focus".to_owned(), json!(true));
    o.e_state = Some(e);
    let id = s.open(o).unwrap();
    t.set(60_000.0); // construit une minute après
    let livre = s.prendre_etat_ouverture(&id).unwrap();
    assert_eq!(livre.get("send"), Some(&json!("premier tour")));
    assert!(!livre.contains_key("focus"));
    assert_eq!(s.prendre_etat_ouverture(&id), None);
}

// ─── Le cadre ───

#[test]
fn ouverte_par_le_service_vue_nue_en_marge_ouverte_son_passage_surligne_au_survol() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    assert!(dw.est_nu() && dw.est_ouvert() && !dw.est_replie());
    // L'en-tête : titre de la vue + actions de doc-widget, pas de croix.
    assert_eq!(dw.titre(), "Vue de test");
    assert_eq!(
        titres(&dw),
        ["Ouvrir à droite", "Ouvrir dans un nouvel onglet", "Ouvrir dans le panneau", "Réduire"]
    );
    assert!(dw.actions_entete().iter().all(|a| a.icone != "x"));
    // Aucun lien dessiné, aucun passage surligné tant que rien ne survole.
    assert!(plages(&d).is_empty());
    // Survol : le passage (et lui seul) se surligne, à la plage de l'ancre.
    dw.poser_survol(true);
    assert_eq!(plages(&d), vec![(12, 18, "doc-widget-passage is-passage is-hover".to_owned())]);
    dw.poser_survol(false);
    assert!(plages(&d).is_empty());
    // La vue a reçu son placement (état éphémère d'ouverture).
    let p = d.vue.placement.borrow().clone().unwrap();
    assert_eq!(p["file"], NOTE);
    assert_eq!((p["from"].as_u64(), p["to"].as_u64()), (Some(12), Some(18)));
    // Nu : activation par la vue hôte (non manuelle) ; sa feuille est confiée à l'actif.
    assert_eq!(h.manuel.get(), Some(false));
    assert_eq!(h.feuille.get(), dw.leaf().map(fragment_ui::doc_widget::cadre::leaf_id));
}

#[test]
fn la_vue_garde_ses_actions_de_widget_et_ouvrir_dans_le_panneau_la_met_dans_le_dock() {
    let d = decor();
    let (dw, _h) = widget_nu(&d, 12, 18);
    dw.poser_crochets(Crochets {
        actions: Some(Rc::new(|| vec![DocWidgetAction::Dock])),
        ..Crochets::default()
    });
    assert_eq!(titres(&dw), ["Ouvrir dans le panneau"]);
    assert!(dw.actions_entete().iter().all(|a| a.cle != "collapse"));
    dw.poser_crochets(Crochets::default());
    let dock = dw.actions_entete().into_iter().find(|a| a.cle == "dock").unwrap();
    (dock.action)();
    let w = d.app.workspace().borrow();
    let droite = w.right_split();
    let dans_le_dock: Vec<_> = w
        .get_leaves_of_type(TYPE_VUE)
        .into_iter()
        .filter(|&l| w.arbre().get_root(l) == droite)
        .collect();
    assert_eq!(dans_le_dock.len(), 1);
    assert!(!w.arbre().est_pile(dw.leaf().unwrap()), "le widget reste");
}

#[test]
fn reduire_reste_le_dernier_bouton_meme_si_la_vue_ajoute_une_action_apres() {
    let d = decor();
    let (dw, _h) = widget_nu(&d, 12, 18);
    dw.ajouter_action_vue("star", "Action tardive", || {});
    let t = titres(&dw);
    assert_eq!(t.last().map(String::as_str), Some("Réduire"));
    assert_eq!(t[t.len() - 2], "Action tardive");
    assert_eq!(t.len(), 5);
}

#[test]
fn supprimer_est_clic_droit_fermer_le_plugin_supprime_le_record() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    let ferme = Rc::new(Cell::new(0));
    let f = ferme.clone();
    dw.on_closed(move || f.set(f.get() + 1));
    let menu = dw.menu_contextuel();
    let fermer = menu.iter().find(|e| e.titre == "Fermer").unwrap();
    (fermer.action)();
    assert_eq!(ferme.get(), 1);
    assert!(dw.est_ferme() && h.retire.get());
    assert!(d.app.workspace().borrow().get_leaves_of_type(TYPE_VUE).is_empty(), "la feuille hébergée est détachée");
    // Fermer deux fois ne ferme qu'une fois.
    dw.fermer();
    assert_eq!(ferme.get(), 1);
}

#[test]
fn le_menu_clic_droit_ouvert_reduire_puis_fermer_replie_n_offre_plus_reduire() {
    let d = decor();
    let (dw, _h) = widget_nu(&d, 12, 18);
    let t: Vec<_> = dw.menu_contextuel().into_iter().map(|e| e.titre).collect();
    assert_eq!(t, ["Réduire", "Fermer"]);
    dw.replier();
    let t: Vec<_> = dw.menu_contextuel().into_iter().map(|e| e.titre).collect();
    assert_eq!(t, ["Fermer"]);
}

#[test]
fn ouvrir_a_droite_une_nouvelle_pile_du_centre_ouvrir_dans_un_nouvel_onglet_la_pile_de_la_note() {
    let d = decor();
    let (dw, _h) = widget_nu(&d, 12, 18);
    let (racine, piles_avant) = {
        let w = d.app.workspace().borrow();
        (w.root_split(), w.arbre().children(w.root_split()).len())
    };
    dw.ouvrir_ailleurs(Ailleurs::Droite);
    {
        let w = d.app.workspace().borrow();
        assert_eq!(w.arbre().children(racine).len(), piles_avant + 1);
        let l = w.get_leaves_of_type(TYPE_VUE).into_iter().find(|&l| w.arbre().parent(l).is_some()).unwrap();
        assert_eq!(w.active_leaf(), Some(l), "la nouvelle vue est active");
        assert_eq!(w.arbre().get_root(l), racine);
    }
    // Retour à la note, puis « nouvel onglet » : dans SA pile.
    {
        let mut w = d.app.workspace().borrow_mut();
        let l = w.get_leaves_of_type(TYPE_VUE).into_iter().find(|&l| w.arbre().parent(l).is_some()).unwrap();
        w.detach(l);
        w.set_active_leaf(Some(d.note));
    }
    dw.ouvrir_ailleurs(Ailleurs::Onglet);
    let w = d.app.workspace().borrow();
    let l = w.get_leaves_of_type(TYPE_VUE).into_iter().find(|&l| w.arbre().parent(l).is_some()).unwrap();
    assert_eq!(w.arbre().parent(l), w.arbre().parent(d.note));
    assert_eq!(w.active_leaf(), Some(l));
}

#[test]
fn ouvrir_ailleurs_emporte_l_etat_que_la_vue_veut_voir_suivre() {
    let d = decor();
    let (dw, _h) = widget_nu(&d, 12, 18);
    dw.poser_crochets(Crochets {
        transfert: Some(Rc::new(|| Some(json!({ "cluster": "g1" })))),
        ..Crochets::default()
    });
    dw.ouvrir_ailleurs(Ailleurs::Panneau);
    assert!(d.vue.etats.borrow().iter().any(|e| e.get("cluster") == Some(&json!("g1"))));
}

#[test]
fn actif_le_passage_reste_surligne_plus_marque_relache_il_part() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    h.set_active(true);
    assert!(dw.est_actif());
    // Le pointeur est parti : l'actif garde le passage, plus marqué.
    dw.poser_survol(false);
    assert_eq!(plages(&d), vec![(12, 18, "doc-widget-passage is-passage is-active".to_owned())]);
    // Un clic dans le texte : l'actif est relâché, le surlignage part.
    h.set_active(false);
    assert!(plages(&d).is_empty());
}

#[test]
fn flottant_le_bouclier_ne_part_qu_a_l_activation_manuelle() {
    let d = decor();
    let (dw, h) = widget(&d, 12, 18, Some((420.0, 320.0)));
    assert!(!dw.est_nu());
    assert_eq!(h.manuel.get(), Some(true), "un glisser du bouclier n'active pas");
    assert!(dw.a_un_bouclier());
    // Un clic sans bouger active.
    dw.bouclier_appui();
    dw.bouclier_relache();
    assert!(h.is_active() && !dw.a_un_bouclier());
    // Relâché (clic dehors), le bouclier revient.
    h.set_active(false);
    assert!(dw.a_un_bouclier());
}

#[test]
fn flottant_un_glisser_deplace_et_ne_signale_qu_a_la_fin() {
    let d = decor();
    let (dw, h) = widget(&d, 12, 18, Some((420.0, 320.0)));
    h.set_anchor(WidgetAnchor::Viewport { x: 200.0, y: 200.0, from: None });
    let vu = Rc::new(RefCell::new(Vec::new()));
    let v = vu.clone();
    dw.on_moved(move |a| v.borrow_mut().push(a));
    dw.bouclier_appui();
    dw.bouclier_glisse(2.0, 1.0); // sous le seuil : rien
    assert_eq!(h.get_anchor(), WidgetAnchor::Viewport { x: 200.0, y: 200.0, from: None });
    dw.bouclier_glisse(30.0, 10.0);
    dw.bouclier_glisse(40.0, 20.0);
    assert_eq!(h.get_anchor(), WidgetAnchor::Viewport { x: 240.0, y: 220.0, from: None });
    assert!(vu.borrow().is_empty(), "une écriture par geste, pas par mouvement");
    dw.bouclier_relache();
    assert_eq!(vu.borrow().len(), 1);
    assert!(!h.is_active(), "un glisser n'active pas");
}

#[test]
fn en_marge_le_x_appartient_a_la_bande_un_glisser_ne_change_que_dy() {
    let a = WidgetAnchor::Gutter { side: GutterSide::Right, pos: 3, to: Some(9), dy: 5.0 };
    assert_eq!(
        DocWidget::decaler(a, 40.0, 12.0),
        WidgetAnchor::Gutter { side: GutterSide::Right, pos: 3, to: Some(9), dy: 17.0 }
    );
}

#[test]
fn replie_le_widget_montre_les_icones_de_sa_vue_et_suit_update_header() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    // « Réduire » par le menu : replié, la marge reçoit la forme réduite (icônes de la vue).
    let reduire = dw.menu_contextuel().into_iter().find(|e| e.titre == "Réduire").unwrap();
    (reduire.action)();
    dw.tour();
    assert!(dw.est_replie());
    assert_eq!(h.icones_compactes(), Some(vec!["🙂".to_owned(), "message-circle".to_owned()]));
    let textes: Vec<_> = dw.pastilles().iter().map(Pastille::texte).collect();
    assert_eq!(textes, ["🙂", ""]);
    assert!(matches!(dw.pastilles()[1], Pastille::Icone(ref n) if n == "message-circle"));
    // La vue change ses icônes : updateHeader, le widget replié suit.
    *d.vue.icones.borrow_mut() = ["A", "B", "C", "D", "E"].map(String::from).to_vec();
    d.app.workspace().borrow_mut().update_header(dw.leaf().unwrap());
    dw.tour();
    assert_eq!(h.icones_compactes().map(|v| v.len()), Some(5));
    let textes: Vec<_> = dw.pastilles().iter().map(Pastille::texte).collect();
    assert_eq!(textes, ["A", "B", "C", "+2"]);
}

#[test]
fn la_forme_reduite_garde_son_identite_tant_que_rien_ne_change() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    dw.replier();
    let avant = h.nb_set_compact.get();
    d.app.workspace().borrow_mut().update_header(dw.leaf().unwrap());
    dw.tour(); // titre et icônes identiques : le même objet, rien de refait
    assert_eq!(h.nb_set_compact.get(), avant);
}

#[test]
fn replie_la_forme_reduite_porte_son_menu_son_survol_et_son_document() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    dw.replier();
    let info = h.compact.borrow().clone().unwrap();
    assert_eq!(info.label, "Vue de test");
    assert!(info.groupable);
    assert_eq!((info.doc.as_ref().unwrap())(), Some("chat:test".to_owned()));
    // Survolée dans la grappe : son passage se surligne.
    (info.hover.as_ref().unwrap())(true);
    assert_eq!(plages(&d).len(), 1);
    (info.hover.as_ref().unwrap())(false);
    assert!(plages(&d).is_empty());
    // Le menu de sa ligne (⋮) : les ouvertures, puis Fermer, en dernier et en danger.
    let menu = (info.menu.as_ref().unwrap())();
    let t: Vec<_> = menu.iter().map(|e| (e.titre.as_str(), e.section.as_str())).collect();
    assert_eq!(
        t,
        [
            ("Ouvrir dans le panneau", "open"),
            ("Ouvrir à droite", "open"),
            ("Ouvrir dans un nouvel onglet", "open"),
            ("Fermer", "danger"),
        ]
    );
    assert!(menu.last().unwrap().avertissement);
}

#[test]
fn reveal_de_la_forme_reduite_deplie_active_et_previent_le_plugin() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    dw.replier();
    dw.tour();
    let bascules = Rc::new(RefCell::new(Vec::new()));
    let b = bascules.clone();
    dw.on_toggled(move |o| b.borrow_mut().push(o));
    let revele = Rc::new(Cell::new(false));
    let r = revele.clone();
    dw.on_revealed(move || r.set(true));
    let info = h.compact.borrow().clone().unwrap();
    (info.reveal)();
    assert!(revele.get() && dw.est_ouvert() && !dw.est_replie());
    assert_eq!(*bascules.borrow(), vec![true]);
    assert!(h.is_active());
    assert!(h.compact.borrow().is_none(), "ouvert : plus de forme réduite");
    // Le focus de la vue vient au tour (après l'ouverture).
    assert_eq!(d.vue.focus.get(), 0);
    dw.tour();
    assert_eq!(d.vue.focus.get(), 1);
}

#[test]
fn reduire_relache_l_actif_apres_le_geste_pas_pendant() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    h.set_active(true);
    dw.replier();
    assert!(h.is_active(), "le clic sur Réduire ne doit pas être ré-activé : le relâché attend le tour");
    dw.tour();
    assert!(!h.is_active());
}

#[test]
fn la_place_deux_vues_proches_la_premiere_passe_en_pastille_un_clic_la_rend_complete() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    // L'arbitre annonce la place AVANT la mesure : la vue choisit son rendu à ses seuils.
    h.emettre_room(Some(f64::INFINITY));
    assert_eq!(*d.vue.mode.borrow(), "full");
    h.emettre_room(Some(40.0));
    assert_eq!(*d.vue.mode.borrow(), "pill");
    // Un clic sur la pastille : la vue regagne sa place (l'arbitre repousse le suivant).
    h.emettre_room(Some(f64::INFINITY));
    assert_eq!(*d.vue.mode.borrow(), "full");
    h.emettre_room(None);
    assert_eq!(*d.vue.mode.borrow(), "hidden");
    drop(dw);
}

#[test]
fn la_place_infinie_et_absente_se_codent_sans_perte() {
    assert_eq!(place_de_l_etat(&etat_de_place(Some(f64::INFINITY))), Some(Some(f64::INFINITY)));
    assert_eq!(place_de_l_etat(&etat_de_place(None)), Some(None));
    assert_eq!(place_de_l_etat(&etat_de_place(Some(120.0))), Some(Some(120.0)));
    assert_eq!(place_de_l_etat(&json!({ "focus": true })), None);
}

#[test]
fn sans_marge_pour_la_vue_entiere_le_widget_se_montre_replie_sans_toucher_a_la_bascule() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    let bascules = Rc::new(Cell::new(0));
    let b = bascules.clone();
    dw.on_toggled(move |_| b.set(b.get() + 1));
    h.emettre_fit(false);
    assert!(dw.est_replie());
    assert!(dw.est_ouvert(), "la bascule de l'utilisateur est intacte");
    assert_eq!(bascules.get(), 0);
    assert!(h.compact.borrow().is_some(), "la forme réduite tient dans la marge étroite");
    assert_eq!(dw.infobulle_icones(), "Vue de test");
    // La fenêtre s'élargit : le widget redevient entier.
    h.emettre_fit(true);
    assert!(!dw.est_replie() && h.compact.borrow().is_none());
    assert_eq!(dw.infobulle_icones(), "Réduire");
}

#[test]
fn masque_par_son_calque_le_passage_n_est_plus_surligne() {
    let d = decor();
    let (dw, _h) = widget_nu(&d, 12, 18);
    dw.poser_survol(true);
    assert_eq!(plages(&d).len(), 1);
    dw.poser_masque(true);
    assert!(plages(&d).is_empty());
}

#[test]
fn le_service_et_le_widget_ensemble_ouvrir_replier_fermer() {
    let d = decor();
    let s = service(true, Rc::new(Cell::new(0.0)));
    let id = s.open(DocWidgetOpen::new(ViewState::new(TYPE_VUE), NOTE, 12, 18)).unwrap();
    let (dw, _h) = widget_nu(&d, 12, 18);
    let s2 = s.clone();
    let i2 = id.clone();
    dw.on_toggled(move |o| s2.poser_ouvert(&i2, o));
    let s3 = s.clone();
    let i3 = id.clone();
    dw.on_closed(move || s3.close(&i3));
    dw.replier();
    assert!(!s.enregistrement(&id).unwrap().ouvert, "la bascule est persistée");
    dw.fermer();
    assert!(s.enregistrement(&id).is_none(), "fermer = le record part");
}

// ─── Le menu clic droit : les places ───

struct Places;
impl PlacementWidget for Places {
    fn can_anchor_to_document(&self) -> bool {
        true
    }
    fn gutter_fits(&self, side: GutterSide) -> bool {
        side == GutterSide::Left
    }
    fn document_anchor_at(&self) -> Option<WidgetAnchor> {
        Some(WidgetAnchor::Document { pos: 12, to: None, dx: 4.0, dy: 2.0 })
    }
    fn viewport_anchor_at(&self) -> Option<WidgetAnchor> {
        Some(WidgetAnchor::viewport(10.0, 20.0))
    }
    fn gutter_anchor_at(&self, side: GutterSide) -> Option<WidgetAnchor> {
        Some(WidgetAnchor::Gutter { side, pos: 12, to: None, dy: 0.0 })
    }
}

#[test]
fn le_menu_clic_droit_propose_les_places_qu_on_n_occupe_pas_et_qui_tiennent() {
    let d = decor();
    let (dw, h) = widget_nu(&d, 12, 18);
    dw.poser_placement(Rc::new(Places));
    // En marge droite : ancrer, détacher, marge gauche (la droite est occupée, et ne tient pas).
    let t: Vec<_> = dw.menu_contextuel().into_iter().map(|e| e.titre).collect();
    assert_eq!(
        t,
        ["Ancrer dans le document", "Détacher du document", "Placer dans la marge gauche", "Réduire", "Fermer"]
    );
    // « Ancrer dans le document » : le passage garde sa LONGUEUR depuis le nouveau début.
    let vu = Rc::new(RefCell::new(Vec::new()));
    let v = vu.clone();
    dw.on_moved(move |a| v.borrow_mut().push(a));
    let ancrer = dw.menu_contextuel().into_iter().find(|e| e.titre == "Ancrer dans le document").unwrap();
    (ancrer.action)();
    assert_eq!(h.get_anchor(), WidgetAnchor::Document { pos: 12, to: Some(18), dx: 4.0, dy: 2.0 });
    assert_eq!(vu.borrow().len(), 1);
}

// ─── Les icônes d'une vue ───

#[test]
fn largeur_des_icones_replie_comme_ouvert() {
    // 11 px à gauche + 7 à droite, des pastilles de 22 px qui se chevauchent de 7 (pas de 15).
    assert_eq!(icones_vue::largeur_pile(1), 40.0);
    assert_eq!(icones_vue::largeur_pile(2), 55.0);
    assert_eq!(icones_vue::largeur_pile(3), 70.0);
    assert_eq!(icones_vue::largeur_pile(5), 85.0); // 3 + « +N »
    assert_eq!(icones_vue::largeur_boite(3, Motif::Bulles), 18.0 + 41.0);
    assert_eq!(icones_vue::largeur_boite(7, Motif::Bulles), 18.0 + 48.0);
    assert_eq!(icones_vue::hauteur_bandeau(Motif::Pile), 38.0);
    assert_eq!(icones_vue::hauteur_bandeau(Motif::Bulles), 55.0);
}

#[test]
fn l_entete_d_un_widget_fait_37_px_et_celui_d_une_grappe_54() {
    let d = decor();
    let (dw, _h) = widget_nu(&d, 12, 18);
    assert_eq!(dw.hauteur_entete(), 37.0);
    assert_eq!(dw.hauteur_icones(), 38.0);
    assert_eq!(icones_vue::HAUTEUR_BULLES - 1.0, 54.0);
}

#[test]
fn la_teinte_d_une_pastille_est_stable_par_le_texte() {
    assert_eq!(icones_vue::teinte("A"), icones_vue::teinte("A"));
    assert!(icones_vue::TEINTES.contains(&icones_vue::teinte("🙂")));
    // « A » : 65 % 9973 = 65 ; 65 % 8 = 1 → ambre.
    assert_eq!(icones_vue::teinte("A"), 60.0);
}

#[test]
fn au_dela_de_trois_icones_une_pastille_plus_n_en_pile_et_en_bulles() {
    let cinq: Vec<String> = ["A", "B", "C", "D", "E"].map(String::from).to_vec();
    let pile = icones_vue::pastilles(&cinq, Motif::Pile);
    assert_eq!(pile.len(), 4);
    assert_eq!(pile[3], Pastille::Plus(2));
    // Bulles : jusqu'à 4, toutes ; au-delà (le disque), 3 plus « +N » dans la grande bas-droite.
    assert_eq!(icones_vue::pastilles(&cinq[..4], Motif::Bulles).len(), 4);
    let disque = icones_vue::pastilles(&cinq, Motif::Bulles);
    assert_eq!((disque.len(), &disque[3]), (4, &Pastille::Plus(2)));
}

#[test]
fn oklch_donne_le_noir_le_blanc_et_un_gris() {
    assert_eq!(icones_vue::oklch_vers_rgb(0.0, 0.0, 0.0), [0, 0, 0]);
    assert_eq!(icones_vue::oklch_vers_rgb(1.0, 0.0, 0.0), [255, 255, 255]);
    let [r, g, b] = icones_vue::oklch_vers_rgb(0.5, 0.0, 0.0);
    assert!(r == g && g == b && (r > 90 && r < 130));
}
