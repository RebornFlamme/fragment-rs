// Port de app/e2e/scroll-persistence.spec.ts (la position de lecture aux trois étages).
//! Tests de la position de lecture (tâche `fr-t2-lecture`).
//!
//! Le TS juge la lecture dans un vrai navigateur ; ici une VUE DE TEST porte un défileur
//! simulé (contenu, fenêtre, boîte) et se branche sur le vrai Workspace du cœur : historique
//! ← →, `rebuild_view`, layout sauvé puis relu, feuilles différées. La plateforme est
//! simulée par la vue : un onglet caché perd sa boîte ET son défilement, une vue réinsérée
//! garde sa boîte mais revient à 0. Les cas note, PDF et conversation du TS sont hérités par
//! fr-t3-vue, fr-t4-vue-pdf et T7 (ils surchargent la lecture, la rétention reste la même) ;
//! le cas explorateur est porté par `tests/explorateur.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashSet;
use std::rc::Rc;

use fragment_core::documents::{DocumentProvider, OWNER_CORE};
use fragment_core::{
    App, AppCoeur, ContexteVue, DocumentKey, ItemId, ItemView, ItemViewBase, LayoutMemoire,
    NouvelleFeuille, OpenViewState, Scope, View, ViewCreator, ViewState, ViewStateResult,
    Workspace,
};
use fragment_ui::lecture::{
    CLE_SCROLL, DELAI_REPRISE_MS, Defileur, IssueReprise, Lecture, Position, Reprise,
};
use serde_json::{Map, Value, json};

const TYPE: &str = "lecture-test";

// ─── Le défileur simulé ───

/// Une fenêtre de 400 px sur un contenu dont la hauteur peut arriver tard. Sans boîte, la
/// mise en page est à 0 × 0 : on ne peut pas défiler.
struct Faux {
    haut: f32,
    gauche: f32,
    contenu: f32,
    boite: bool,
}

impl Faux {
    fn maximum(&self) -> f32 {
        if self.boite {
            (self.contenu - 400.0).max(0.0)
        } else {
            0.0
        }
    }
}

impl Defileur for Faux {
    fn position(&self) -> (f32, f32) {
        (self.haut, self.gauche)
    }
    fn poser(&mut self, haut: f32, gauche: f32) {
        self.haut = haut.clamp(0.0, self.maximum());
        self.gauche = gauche.max(0.0);
    }
    fn a_une_boite(&self) -> bool {
        self.boite
    }
}

// ─── Le coffre : des clés quelconques ouvrent la vue de test ───

struct Coffre {
    fichiers: HashSet<String>,
}

impl DocumentProvider for Coffre {
    fn scheme(&self) -> &str {
        ""
    }
    fn open(&self, key: &str) -> Option<ViewState> {
        self.fichiers
            .contains(key)
            .then(|| ViewState::with_state(TYPE, json!({ "file": key })))
    }
    fn key_of(&self, state: &ViewState) -> Option<DocumentKey> {
        (state.type_ == TYPE)
            .then(|| state.state.as_ref()?.get("file")?.as_str().map(Into::into))
            .flatten()
    }
    fn title(&self, key: &str) -> String {
        key.to_owned()
    }
    fn exists(&self, key: &str) -> bool {
        self.fichiers.contains(key)
    }
}

// ─── La vue de test : le défaut générique, tel que le fera toute vue sans surcharge ───

struct VueLecture {
    base: ItemViewBase,
    fichier: Option<String>,
    lecture: Lecture,
    faux: Rc<RefCell<Faux>>,
    horloge: Rc<Cell<u64>>,
}

impl VueLecture {
    fn aller(&mut self, p: &Value) {
        let d: Rc<RefCell<dyn Defileur>> = self.faux.clone();
        self.lecture.aller_a_position(self.horloge.get(), p, d);
    }

    fn lue(&self) -> Option<Value> {
        Some(Position::lire(&*self.faux.borrow()).en_json())
    }

    /// L'utilisateur défile.
    fn defiler_a(&mut self, y: f32) {
        self.faux.borrow_mut().poser(y, 0.0);
        self.lecture.signaler_defilement();
    }

    /// Une image : vrai quand une position a été retenue.
    fn image(&mut self) -> bool {
        let boite = self.faux.borrow().boite;
        let lue = self.lue();
        self.lecture.image(self.horloge.get(), boite, || lue)
    }

    /// Autre onglet / dock replié : la boîte disparaît et le défilement est jeté ; l'événement
    /// de défilement part quand même.
    fn cacher(&mut self) {
        {
            let mut f = self.faux.borrow_mut();
            f.boite = false;
            f.haut = 0.0;
            f.gauche = 0.0;
        }
        self.lecture.signaler_defilement();
        self.lecture.boite_changee(false, true);
    }

    /// La boîte revient : l'observateur de taille se déclenche.
    fn montrer(&mut self) {
        self.faux.borrow_mut().boite = true;
        self.boite_changee();
    }

    /// Onglet déplacé dans une autre pile : jamais à 0 × 0, mais le défileur repart à zéro.
    fn reinserer(&mut self) {
        {
            let mut f = self.faux.borrow_mut();
            f.haut = 0.0;
            f.gauche = 0.0;
        }
        self.boite_changee();
    }

    fn boite_changee(&mut self) {
        let (boite, zero) = {
            let f = self.faux.borrow();
            (f.boite, f.haut == 0.0 && f.gauche == 0.0)
        };
        if let Some(p) = self.lecture.boite_changee(boite, zero) {
            self.aller(&p);
        }
    }

    fn haut(&self) -> f32 {
        self.faux.borrow().haut
    }
}

impl View for VueLecture {
    fn get_view_type(&self) -> String {
        TYPE.to_owned()
    }
    fn get_display_text(&self) -> String {
        self.fichier.clone().unwrap_or_else(|| TYPE.to_owned())
    }
    fn get_document(&self) -> Option<DocumentKey> {
        self.fichier.as_deref().map(DocumentKey::from)
    }
    fn navigation(&self) -> bool {
        true
    }
    fn scope(&self) -> Option<Scope> {
        Some(self.base.scope.clone())
    }
    fn get_state(&self) -> Map<String, Value> {
        let mut m = Map::new();
        if let Some(f) = &self.fichier {
            m.insert("file".into(), json!(f));
        }
        m
    }
    fn set_state(&mut self, state: &Map<String, Value>, _: ViewStateResult) {
        if let Some(f) = state.get("file").and_then(Value::as_str)
            && self.fichier.as_deref() != Some(f)
        {
            // `view-document-change` : la position retenue était celle de l'ancien document.
            self.lecture.document_change();
            // Un autre contenu : le défileur repart en haut.
            self.faux.borrow_mut().poser(0.0, 0.0);
            self.fichier = Some(f.to_owned());
        }
    }
    fn get_ephemeral_state(&self) -> Map<String, Value> {
        let boite = self.faux.borrow().boite;
        self.lecture.etat_ephemere(Map::new(), boite, || self.lue())
    }
    fn set_ephemeral_state(&mut self, state: &Value) {
        if let Some(p) = self.lecture.appliquer_etat(state) {
            self.aller(&p);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ItemView for VueLecture {
    fn item_scope(&self) -> &Scope {
        &self.base.scope
    }
}

// ─── Le banc : une « session » de l'application ───

struct Banc {
    app: Rc<AppCoeur>,
    store: LayoutMemoire,
    horloge: Rc<Cell<u64>>,
    _retraits: Vec<Box<dyn FnOnce()>>,
}

const DOCS: [&str; 3] = ["a.md", "b.md", "c.md"];

fn lancer(store: &LayoutMemoire, contenu: f32) -> Banc {
    let app = AppCoeur::new();
    let horloge = Rc::new(Cell::new(0_u64));
    let hauteur = Rc::new(Cell::new(contenu));
    let (h, c) = (horloge.clone(), hauteur.clone());
    let fabrique: ViewCreator = Rc::new(move |ctx: &ContexteVue| {
        Box::new(VueLecture {
            base: ItemViewBase::new(&ctx.app_scope),
            fichier: None,
            lecture: Lecture::new(),
            faux: Rc::new(RefCell::new(Faux {
                haut: 0.0,
                gauche: 0.0,
                contenu: c.get(),
                boite: true,
            })),
            horloge: h.clone(),
        }) as Box<dyn View>
    });
    let coffre = Coffre {
        fichiers: DOCS.iter().map(|s| (*s).to_owned()).collect(),
    };
    let retraits: Vec<Box<dyn FnOnce()>> = vec![
        Box::new(app.view_registry().register(TYPE, fabrique, "test")),
        Box::new(
            app.documents()
                .register(Rc::new(coffre), OWNER_CORE)
                .unwrap(),
        ),
    ];
    {
        let mut w = app.workspace().borrow_mut();
        w.set_layout_store(Rc::new(store.clone()));
        w.load_layout();
    }
    Banc {
        app,
        store: store.clone(),
        horloge,
        _retraits: retraits,
    }
}

fn banc() -> Banc {
    lancer(&LayoutMemoire::new(), 2000.0)
}

fn actif() -> OpenViewState {
    OpenViewState {
        active: Some(true),
        ..OpenViewState::default()
    }
}

impl Banc {
    fn ws(&self) -> RefMut<'_, Workspace> {
        self.app.workspace().borrow_mut()
    }

    /// Ouvre un document dans un onglet neuf, actif.
    fn ouvrir(&self, doc: &str) -> ItemId {
        self.ws()
            .open_document(doc, NouvelleFeuille::Onglet, Some(actif()))
            .expect("ouvrir")
    }

    fn vue<R>(&self, leaf: ItemId, f: impl FnOnce(&mut VueLecture) -> R) -> R {
        let mut w = self.ws();
        let v = w.view_mut(leaf).expect("vue");
        f(v.as_any_mut().downcast_mut::<VueLecture>().expect("type"))
    }

    fn haut(&self, leaf: ItemId) -> f32 {
        self.vue(leaf, |v| v.haut())
    }

    /// L'utilisateur défile, puis une image passe : l'hôte demande la sauvegarde si la vue
    /// a retenu une position.
    fn defiler(&self, leaf: ItemId, y: f32) {
        self.vue(leaf, |v| v.defiler_a(y));
        self.image(leaf);
    }

    fn image(&self, leaf: ItemId) -> bool {
        self.horloge.set(self.horloge.get() + 16);
        let retenue = self.vue(leaf, |v| v.image());
        if retenue {
            self.ws().request_save_layout();
        }
        retenue
    }

    /// La feuille de ce type, la seule de la session relancée.
    fn feuille(&self) -> ItemId {
        let f = self.ws().get_leaves_of_type(TYPE);
        assert_eq!(f.len(), 1, "une seule feuille attendue : {f:?}");
        f[0]
    }

    fn layout(&self) -> Value {
        serde_json::from_str(&self.store.contenu().expect("layout écrit")).unwrap()
    }
}

/// Toutes les feuilles du layout JSON.
fn feuilles(v: &Value, sortie: &mut Vec<Value>) {
    match v {
        Value::Object(o) => {
            if o.get("type").and_then(Value::as_str) == Some("leaf") {
                sortie.push(v.clone());
            }
            o.values().for_each(|x| feuilles(x, sortie));
        }
        Value::Array(a) => a.iter().for_each(|x| feuilles(x, sortie)),
        _ => {}
    }
}

fn pres(a: f32, b: f32) -> bool {
    (a - b).abs() < 2.0
}

// ─── Étages 1 et 2 : « autre onglet puis retour, ←, rebuildView, recharger » ───

#[test]
fn vue_de_test_autre_onglet_puis_retour() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 800.0);
    assert!(pres(b.haut(leaf), 800.0));
    // Un autre onglet de la même pile : la vue est cachée, Chromium jette le défilement.
    b.vue(leaf, |v| v.cacher());
    assert!(pres(b.haut(leaf), 0.0));
    b.vue(leaf, |v| v.montrer());
    assert!(pres(b.haut(leaf), 800.0), "revenue en {}", b.haut(leaf));
}

#[test]
fn vue_de_test_cachee_l_etat_ephemere_rend_la_position_retenue() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 640.0);
    b.vue(leaf, |v| v.cacher());
    let e = b.vue(leaf, |v| v.get_ephemeral_state());
    assert_eq!(e[CLE_SCROLL], json!({ "top": 640, "left": 0 }));
}

#[test]
fn vue_de_test_fleche_arriere() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 900.0);
    // Naviguer ailleurs DANS l'onglet, puis ←.
    assert!(b.ws().open_document_in(leaf, "b.md", None));
    assert!(pres(b.haut(leaf), 0.0));
    assert!(b.ws().go_back(leaf));
    assert!(pres(b.haut(leaf), 900.0), "← en {}", b.haut(leaf));
}

#[test]
fn vue_de_test_rebuild_view() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 700.0);
    b.ws().rebuild_view(leaf);
    assert!(
        pres(b.haut(leaf), 700.0),
        "reconstruite en {}",
        b.haut(leaf)
    );
}

#[test]
fn vue_de_test_recharger() {
    let store = LayoutMemoire::new();
    {
        let b = lancer(&store, 2000.0);
        let leaf = b.ouvrir("a.md");
        b.defiler(leaf, 1100.0);
        // La sauvegarde débouncée du layout (2 s) : l'hôte l'écrit quand elle est due.
        assert!(b.ws().prendre_sauvegarde_due());
        assert!(b.ws().save_layout().unwrap());
    }
    let b = lancer(&store, 2000.0);
    let leaf = b.feuille();
    b.ws().load_if_deferred(leaf);
    assert!(pres(b.haut(leaf), 1100.0), "rechargée en {}", b.haut(leaf));
}

// ─── Étage 2 : réinsertion ───

#[test]
fn note_deplacer_l_onglet_dans_une_autre_pile_elle_ne_revient_pas_en_haut() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 1000.0);
    // L'onglet est réinséré ailleurs : la boîte ne passe pas par 0 × 0, le défileur repart à 0.
    b.vue(leaf, |v| v.reinserer());
    assert!(pres(b.haut(leaf), 1000.0), "réinsérée en {}", b.haut(leaf));
}

#[test]
fn une_vue_cachee_ne_retient_pas_la_remise_a_zero() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 500.0);
    b.vue(leaf, |v| v.cacher()); // l'événement de défilement part, la boîte est à 0 × 0
    assert!(!b.image(leaf), "rien à retenir sans boîte");
    let retenue = b.vue(leaf, |v| v.lecture.retenue().cloned());
    assert_eq!(retenue, Some(json!({ "top": 500, "left": 0 })));
}

#[test]
fn une_vue_qui_revient_en_haut_retenue_en_haut_ne_change_rien() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 0.0);
    b.vue(leaf, |v| v.reinserer());
    assert!(pres(b.haut(leaf), 0.0));
}

#[test]
fn le_document_qui_change_oublie_la_position_retenue() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.defiler(leaf, 800.0);
    assert!(b.ws().open_document_in(leaf, "b.md", None));
    b.vue(leaf, |v| v.cacher());
    b.vue(leaf, |v| v.montrer());
    assert!(
        pres(b.haut(leaf), 0.0),
        "la position de a.md ne doit pas passer sur b.md"
    );
}

// ─── Étage 1 : la reprise réessayée ───

#[test]
fn la_reprise_est_reessayee_a_chaque_image_jusqu_a_ce_que_le_contenu_soit_assez_haut() {
    let store = LayoutMemoire::new();
    {
        let b = lancer(&store, 2000.0);
        let leaf = b.ouvrir("a.md");
        b.defiler(leaf, 1200.0);
        b.ws().save_layout().unwrap();
    }
    // Le contenu n'est pas encore arrivé (conversation relue, page qui se charge).
    let b = lancer(&store, 0.0);
    let leaf = b.feuille();
    b.ws().load_if_deferred(leaf);
    assert!(pres(b.haut(leaf), 0.0));
    assert!(b.vue(leaf, |v| v.lecture.reprise_en_cours()));
    b.image(leaf);
    assert!(pres(b.haut(leaf), 0.0));
    b.vue(leaf, |v| v.faux.borrow_mut().contenu = 2000.0);
    b.image(leaf);
    assert!(pres(b.haut(leaf), 1200.0), "reprise en {}", b.haut(leaf));
    assert!(!b.vue(leaf, |v| v.lecture.reprise_en_cours()));
}

#[test]
fn la_reprise_est_abandonnee_au_premier_geste() {
    let store = LayoutMemoire::new();
    {
        let b = lancer(&store, 2000.0);
        let leaf = b.ouvrir("a.md");
        b.defiler(leaf, 1200.0);
        b.ws().save_layout().unwrap();
    }
    let b = lancer(&store, 0.0);
    let leaf = b.feuille();
    b.ws().load_if_deferred(leaf);
    // La molette, le pointeur ou le clavier : la lecture de l'utilisateur prime.
    b.vue(leaf, |v| v.lecture.geste());
    b.vue(leaf, |v| v.faux.borrow_mut().contenu = 2000.0);
    b.image(leaf);
    assert!(
        pres(b.haut(leaf), 0.0),
        "abandonnée mais posée en {}",
        b.haut(leaf)
    );
}

#[test]
fn reprise_reussie_au_premier_essai_expiree_apres_10_s() {
    let tentatives = Rc::new(Cell::new(0));
    let t = tentatives.clone();
    let r = Reprise::demarrer(
        0,
        Box::new(move || {
            t.set(t.get() + 1);
            true
        }),
    );
    assert_eq!(r.issue(), IssueReprise::Reussie);
    assert_eq!(tentatives.get(), 1);

    let mut r = Reprise::demarrer(1_000, Box::new(|| false));
    assert_eq!(r.image(1_016), IssueReprise::EnCours);
    assert_eq!(r.image(1_000 + DELAI_REPRISE_MS), IssueReprise::EnCours);
    assert_eq!(r.image(1_001 + DELAI_REPRISE_MS), IssueReprise::Expiree);
    r.geste();
    assert_eq!(
        r.issue(),
        IssueReprise::Expiree,
        "un geste n'efface pas l'issue"
    );
}

#[test]
fn une_nouvelle_reprise_annule_la_precedente() {
    let mut l = Lecture::new();
    let a = Rc::new(Cell::new(0));
    let a2 = a.clone();
    l.reprendre(
        0,
        Box::new(move || {
            a2.set(a2.get() + 1);
            false
        }),
    );
    l.reprendre(16, Box::new(|| false));
    l.image(32, false, || None);
    assert_eq!(a.get(), 1, "l'ancien essai ne rejoue plus");
}

#[test]
fn une_position_d_une_autre_forme_est_ignoree_par_le_defaut() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    // Une note y met `{line, dy}`, un PDF `{page, fy, fx}` : pas un défilement en pixels.
    b.vue(leaf, |v| {
        v.set_ephemeral_state(&json!({ "scroll": { "line": 40, "dy": 3 } }))
    });
    assert!(pres(b.haut(leaf), 0.0));
    assert!(!b.vue(leaf, |v| v.lecture.reprise_en_cours()));
    // ...mais elle est retenue telle quelle, pour être rendue à l'identique.
    let e = b.vue(leaf, |v| v.lecture.retenue().cloned());
    assert_eq!(e, Some(json!({ "line": 40, "dy": 3 })));
}

#[test]
fn la_position_se_relit_arrondie_et_sans_gauche() {
    assert_eq!(
        Position::depuis_json(&json!({ "top": 12.6 })),
        Some(Position {
            haut: 13,
            gauche: 0
        })
    );
    assert_eq!(Position::depuis_json(&json!({ "left": 3 })), None);
    assert_eq!(Position::depuis_json(&json!(7)), None);
}

// ─── Étage 3 : workspace.json ───

#[test]
fn chaque_position_retenue_demande_la_sauvegarde_du_layout() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    b.ws().prendre_sauvegarde_due(); // celle de l'ouverture
    // Une image sans défilement ne demande rien.
    assert!(!b.image(leaf));
    assert!(!b.ws().prendre_sauvegarde_due());
    // Trois événements dans la même image : UNE position retenue, UNE demande.
    b.vue(leaf, |v| {
        v.defiler_a(100.0);
        v.defiler_a(200.0);
        v.defiler_a(300.0);
    });
    assert!(b.image(leaf));
    assert!(b.ws().prendre_sauvegarde_due());
    assert!(!b.image(leaf));
}

#[test]
fn l_etat_ephemere_entier_est_dans_workspace_json() {
    let b = banc();
    let leaf = b.ouvrir("a.md");
    // Un état éphémère qui n'est pas qu'une position (zoom, calques) voyage avec elle.
    b.vue(leaf, |v| {
        v.set_ephemeral_state(&json!({ "activeLayers": ["annotation"] }));
        v.defiler_a(450.0);
    });
    b.image(leaf);
    assert!(b.ws().save_layout().unwrap());
    let mut f = Vec::new();
    feuilles(&b.layout(), &mut f);
    let e = f
        .iter()
        .find_map(|l| l.get("eState"))
        .expect("eState dans le layout");
    assert_eq!(e[CLE_SCROLL], json!({ "top": 450, "left": 0 }));
}

#[test]
fn pas_d_ecriture_forcee_a_la_fermeture() {
    let store = LayoutMemoire::new();
    {
        let b = lancer(&store, 2000.0);
        let leaf = b.ouvrir("a.md");
        b.defiler(leaf, 300.0);
        b.ws().save_layout().unwrap();
        // Les dernières secondes de défilement, jamais écrites : la fermeture n'écrit rien.
        b.defiler(leaf, 1500.0);
        assert!(b.ws().prendre_sauvegarde_due());
    }
    let b = lancer(&store, 2000.0);
    let leaf = b.feuille();
    b.ws().load_if_deferred(leaf);
    assert!(pres(b.haut(leaf), 300.0), "relu en {}", b.haut(leaf));
}

#[test]
fn une_feuille_differee_jamais_vue_garde_son_etat_a_la_sauvegarde() {
    let store = LayoutMemoire::new();
    {
        let b = lancer(&store, 2000.0);
        let a = b.ouvrir("a.md");
        b.defiler(a, 600.0);
        let _c = b.ouvrir("c.md"); // c.md passe devant : a.md reste en fond
        b.ws().save_layout().unwrap();
    }
    // Redémarrage : a.md est en fond, donc différée. Elle n'est jamais regardée.
    let b = lancer(&store, 2000.0);
    assert!(b.ws().save_layout().unwrap());
    let mut f = Vec::new();
    feuilles(&b.layout(), &mut f);
    let garde = f
        .iter()
        .any(|l| l["eState"][CLE_SCROLL] == json!({ "top": 600, "left": 0 }));
    assert!(garde, "l'état de la feuille différée est perdu : {f:?}");
}
