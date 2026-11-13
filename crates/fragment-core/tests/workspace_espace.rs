// Port de app/e2e/activeleaf-tracking.spec.ts, app/e2e/boot-scope.spec.ts,
// app/e2e/workspace-layout.spec.ts et app/src/ui/coreCommands/WorkspaceCommandsPlugin.test.ts,
// au niveau du MODÈLE (Workspace, feuilles, portées, restauration), avec des vues factices.
//
// Les specs e2e lancent l'application sur un coffre temporaire, la ferment et la relancent. Ici,
// un « banc » assemble l'App du cœur, un fournisseur de documents en mémoire (le coffre) et un
// stockage de layout en mémoire partagé entre deux bancs successifs (le redémarrage). Les vues
// « markdown », « pdf », « chat » et « explorateur » sont des vues de test : elles retiennent le
// fichier de leur état, une lecture opaque, et posent Mod+F dans leur portée (le Ctrl+F du PDF
// et de la note).

// Fichier de test : les assistants hors `#[test]` (banc, vues factices) déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::{Cell, RefMut};
use std::collections::HashSet;
use std::rc::Rc;

use fragment_core::documents::{DocumentProvider, OWNER_CORE};
use fragment_core::keymap::types::Modifier;
use fragment_core::{
    App, AppCoeur, Command, CommandHost, ContexteVue, DocumentKey, EditorContext, EmptyView,
    ItemId, ItemViewBase, KeyEvent, LayoutMemoire, LayoutStore, NouvelleFeuille, OpenViewState,
    Scope, View, ViewCreator, ViewState, ViewStateResult, Workspace, enregistrer_commandes_app,
    enregistrer_commandes_workspace,
};
use serde_json::{Map, Value, json};

// ─── Le coffre : un fournisseur de documents du schéma vide ───

struct Coffre {
    fichiers: HashSet<String>,
}

fn type_de(chemin: &str) -> Option<&'static str> {
    if chemin.ends_with(".md") {
        Some("markdown")
    } else if chemin.ends_with(".pdf") {
        Some("pdf")
    } else {
        None
    }
}

impl DocumentProvider for Coffre {
    fn scheme(&self) -> &str {
        ""
    }

    fn open(&self, key: &str) -> Option<ViewState> {
        if !self.fichiers.contains(key) {
            return None;
        }
        Some(ViewState::with_state(type_de(key)?, json!({ "file": key })))
    }

    fn key_of(&self, state: &ViewState) -> Option<DocumentKey> {
        type_de_vue_de_fichier(&state.type_)?;
        let f = state.state.as_ref()?.get("file")?.as_str()?;
        Some(f.into())
    }

    fn title(&self, key: &str) -> String {
        let nom = key.rsplit('/').next().unwrap_or(key);
        nom.rsplit_once('.').map_or(nom, |(b, _)| b).to_owned()
    }

    fn exists(&self, key: &str) -> bool {
        self.fichiers.contains(key)
    }
}

fn type_de_vue_de_fichier(t: &str) -> Option<()> {
    matches!(t, "markdown" | "pdf").then_some(())
}

// ─── Une vue factice ───

struct VueTest {
    type_: String,
    base: ItemViewBase,
    fichier: Option<String>,
    lecture: Map<String, Value>,
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
            fichier: None,
            lecture: Map::new(),
            recherches,
        }
    }
}

impl View for VueTest {
    fn get_view_type(&self) -> String {
        self.type_.clone()
    }

    fn get_display_text(&self) -> String {
        match &self.fichier {
            Some(f) => Coffre {
                fichiers: HashSet::new(),
            }
            .title(f),
            None => self.type_.clone(),
        }
    }

    fn get_document(&self) -> Option<DocumentKey> {
        self.fichier.as_deref().map(DocumentKey::from)
    }

    fn navigation(&self) -> bool {
        type_de_vue_de_fichier(&self.type_).is_some()
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
        if let Some(f) = state.get("file").and_then(Value::as_str) {
            self.fichier = Some(f.to_owned());
        }
    }

    fn get_ephemeral_state(&self) -> Map<String, Value> {
        self.lecture.clone()
    }

    fn set_ephemeral_state(&mut self, state: &Value) {
        if let Some(m) = state.as_object() {
            self.lecture = m.clone();
        }
    }

    /// Seules les notes exposent un éditeur (les PDF, chats, explorateurs n'en ont pas).
    fn editor_context(&self) -> Option<EditorContext> {
        (self.type_ == "markdown").then(|| EditorContext {
            editor: Rc::new(self.type_.clone()),
            view: Rc::new(()),
        })
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

// ─── Le banc : une « session » de l'application ───

struct Banc {
    app: Rc<AppCoeur>,
    store: LayoutMemoire,
    _retraits: Vec<Box<dyn FnOnce()>>,
}

/// Lance une session sur un coffre et un stockage de layout donnés, et attend le layout prêt.
fn lancer(fichiers: &[&str], store: &LayoutMemoire) -> Banc {
    let app = AppCoeur::new();
    let mut retraits: Vec<Box<dyn FnOnce()>> = Vec::new();
    for t in ["markdown", "pdf", "chat", "explorateur"] {
        retraits.push(app.view_registry().register(t, fabrique(t), "test"));
    }
    let coffre = Coffre {
        fichiers: fichiers.iter().map(|s| (*s).to_owned()).collect(),
    };
    retraits.push(
        app.documents()
            .register(Rc::new(coffre), OWNER_CORE)
            .unwrap(),
    );
    retraits.extend(enregistrer_commandes_workspace(
        &app.commands(),
        app.workspace(),
    ));
    retraits.extend(enregistrer_commandes_app(&app.commands(), app.workspace()));
    // La commande « sur la vue active » des specs (`annotation:toggle`) : elle bascule un
    // calque dans la lecture de la vue active, et n'est disponible que s'il y en a une.
    let ws = Rc::downgrade(app.workspace());
    retraits.push(app.commands().add_command(
        Command::new("test:calque", "Basculer le calque").with_check_callback(move |checking| {
            let ws = ws.upgrade()?;
            let mut w = ws.try_borrow_mut().ok()?;
            let Some(a) = w.active_leaf() else {
                return Some(false);
            };
            if !checking {
                let v = w.view_mut(a)?;
                let mut lecture = v.get_ephemeral_state();
                let calques = lecture.entry("activeLayers").or_insert(json!([]));
                let liste = calques.as_array_mut()?;
                if let Some(i) = liste.iter().position(|c| c == "annotation") {
                    liste.remove(i);
                } else {
                    liste.push(json!("annotation"));
                }
                v.set_ephemeral_state(&Value::Object(lecture));
            }
            Some(true)
        }),
        "test",
    ));
    {
        let mut w = app.workspace().borrow_mut();
        w.set_layout_store(Rc::new(store.clone()));
        w.default_side_view_type = "explorateur".to_owned();
        w.document_initial = fichiers
            .iter()
            .find(|f| f.ends_with(".md"))
            .map(|f| DocumentKey::from(*f));
        w.load_layout();
    }
    Banc {
        app,
        store: store.clone(),
        _retraits: retraits,
    }
}

impl Banc {
    fn ws(&self) -> RefMut<'_, Workspace> {
        self.app.workspace().borrow_mut()
    }

    /// Ouvre ces notes, chacune dans un nouvel onglet, actif.
    fn ouvrir(&self, noms: &[&str]) {
        for n in noms {
            let leaf = self.ws().open_document(
                &format!("{n}.md"),
                NouvelleFeuille::Onglet,
                Some(actif_vrai()),
            );
            assert!(leaf.is_some(), "{n}.md ne s'ouvre pas");
        }
    }

    /// (type de la vue montée, nom du document) de la feuille active.
    fn actif(&self) -> Option<(String, Option<String>)> {
        let w = self.ws();
        let a = w.active_leaf()?;
        let t = w.leaf(a)?.view().get_view_type();
        let base = w.document_de(a).map(|d| {
            let s: &str = &d;
            s.rsplit_once('.').map_or(s, |(b, _)| b).to_owned()
        });
        Some((t, base))
    }

    fn executer(&self, id: &str) -> bool {
        self.app.commands().execute_command_by_id(id, None)
    }

    fn ctrl_f(&self) -> Option<bool> {
        let mut evt = KeyEvent::new("f");
        evt.mods.ctrl = true;
        evt.mods.meta = cfg!(target_os = "macos");
        evt.mods.ctrl = !cfg!(target_os = "macos");
        self.app.keymap().handle_key_event(&evt)
    }

    /// Combien de Mod+F la vue de cette feuille a reçus.
    fn recherches(&self, leaf: ItemId) -> u32 {
        let w = self.ws();
        w.leaf(leaf)
            .and_then(|l| l.view().as_any().downcast_ref::<VueTest>())
            .map_or(0, |v| v.recherches.get())
    }

    /// L'état « boot-scope » : type demandé de l'active, visible dans sa pile, sa portée au
    /// sommet, ids uniques.
    fn etat(&self) -> (Option<String>, bool, bool, bool) {
        let w = self.ws();
        let a = w.active_leaf();
        let type_ = a
            .and_then(|a| w.leaf(a))
            .map(|l| l.get_view_type().to_owned());
        let visible = a.is_some_and(|a| {
            w.arbre()
                .parent(a)
                .is_some_and(|p| w.arbre().active_leaf(p) == Some(a))
        });
        let sommet = a
            .and_then(|a| w.leaf(a))
            .and_then(|l| l.view().scope())
            .is_some_and(|s| self.app.keymap().scope() == s);
        let ids: Vec<String> = w
            .iterate_all_leaves()
            .into_iter()
            .map(|l| w.leaf(l).unwrap().id.clone())
            .collect();
        let uniques = ids.iter().collect::<HashSet<_>>().len() == ids.len();
        (type_, visible, sommet, uniques)
    }

    /// La pile du centre qui contient l'active (ou la première).
    fn pile_centrale(&self) -> ItemId {
        let w = self.ws();
        w.arbre().piles_sous(w.root_split())[0]
    }
}

fn actif_vrai() -> OpenViewState {
    OpenViewState {
        active: Some(true),
        ..OpenViewState::default()
    }
}

fn attendu(type_: &str) -> (Option<String>, bool, bool, bool) {
    (Some(type_.to_owned()), true, true, true)
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  activeleaf-tracking.spec.ts
// ═══════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fermer_longlet_actif_reassigne_activeleaf_a_longlet_visible() {
    let b = lancer(&["a.md", "b.md", "c.md"], &LayoutMemoire::new());
    b.ouvrir(&["a", "b", "c"]);
    assert_eq!(b.actif(), Some(("markdown".into(), Some("c".into()))));

    // Fermer l'onglet ACTIF (c).
    let a = b.ws().active_leaf().unwrap();
    b.ws().detach(a);

    let apres = b.actif();
    assert!(apres.is_some()); // ★ jamais vide tant qu'un onglet reste
    assert_eq!(apres.unwrap().0, "markdown"); // l'onglet redevenu visible est l'actif

    // Une commande « sur la vue active » marche de nouveau.
    assert!(b.executer("test:calque"));
    let w = b.ws();
    let lecture = w
        .leaf(w.active_leaf().unwrap())
        .unwrap()
        .view()
        .get_ephemeral_state();
    assert!(
        lecture["activeLayers"]
            .as_array()
            .unwrap()
            .contains(&json!("annotation"))
    );
}

#[test]
fn presser_un_onglet_le_rend_actif_activation_sur_pointerdown() {
    let b = lancer(&["a.md", "b.md"], &LayoutMemoire::new());
    b.ouvrir(&["a", "b"]); // b est actif (ouvert en dernier)
    assert_eq!(b.actif().unwrap().1.as_deref(), Some("b"));

    // L'appui sur l'en-tête dont le libellé est « a » (le premier trouvé, comme la spec).
    let pile = b.pile_centrale();
    let i = {
        let w = b.ws();
        w.arbre()
            .children(pile)
            .iter()
            .position(|&l| w.leaf(l).unwrap().get_display_text() == "a")
            .unwrap()
    };
    b.ws().activer_onglet(pile, i);
    assert_eq!(b.actif(), Some(("markdown".into(), Some("a".into()))));
}

#[test]
fn au_demarrage_activeleaf_tombe_sur_la_vue_principale_jamais_null_repli_de_restauration() {
    let store = LayoutMemoire::new();
    {
        let b = lancer(&["a.md"], &store);
        b.ouvrir(&["a"]);
        // L'état HÉRITÉ du défaut : aucune active au moment de la sauvegarde.
        b.ws().set_active_leaf(None);
        assert!(b.ws().save_layout().unwrap());
        let brut: Value = serde_json::from_str(&b.store.contenu().unwrap()).unwrap();
        assert_eq!(brut["active"], Value::Null);
    }
    // Redémarrage : la restauration relit le fichier.
    let b = lancer(&["a.md"], &store);
    assert_eq!(b.actif().unwrap().0, "markdown"); // ★ le repli a activé la vue principale
    assert!(b.executer("test:calque")); // et une commande « vue active » marche d'emblée
}

#[test]
fn un_clic_ou_un_focus_dans_une_feuille_la_rend_active_pas_seulement_son_onglet_les_raccourcis_de_document_suivent()
 {
    let b = lancer(&["a.md"], &LayoutMemoire::new());
    b.ouvrir(&["a"]);
    let note = b.ws().active_leaf().unwrap();
    // Le chat (dock droit) prend la main.
    let chat = b.ws().get_right_leaf().unwrap();
    b.ws().set_view_state(
        chat,
        ViewState {
            active: Some(true),
            ..ViewState::new("chat")
        },
        None,
    );
    assert_eq!(b.actif().unwrap().0, "chat");

    // Un appui DANS le texte de la note (pas son onglet) : la note redevient active.
    b.ws().activer_par_contact(note);
    assert_eq!(b.actif().unwrap().0, "markdown");
    // Ses raccourcis de document suivent : Mod+F part à SA portée.
    assert_eq!(b.ctrl_f(), Some(false));
    assert_eq!((b.recherches(note), b.recherches(chat)), (1, 0));

    // Le focus dans le champ du chat : le chat redevient actif, et reçoit la frappe.
    b.ws().activer_par_contact(chat);
    assert_eq!(b.actif().unwrap().0, "chat");
    b.ctrl_f();
    assert_eq!((b.recherches(note), b.recherches(chat)), (1, 1));

    // Un appui dans l'explorateur (dock gauche) : l'explorateur est actif, comme Obsidian.
    let explorateur = {
        let w = b.ws();
        w.arbre().feuilles_sous(w.left_split())[0]
    };
    b.ws().activer_par_contact(explorateur);
    assert_eq!(b.actif().unwrap().0, "explorateur");
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  boot-scope.spec.ts
// ═══════════════════════════════════════════════════════════════════════════════════════

fn feuille(id: &str, type_: &str, fichier: Option<&str>) -> Value {
    match fichier {
        Some(f) => {
            json!({"type": "leaf", "id": id, "state": {"type": type_, "state": {"file": f}}})
        }
        None => json!({"type": "leaf", "id": id, "state": {"type": type_}}),
    }
}

fn racine(direction: &str, enfants: Vec<Value>) -> Value {
    json!({"type": "split", "direction": direction, "children": enfants})
}

/// Le layout réel de la régression, réécrit : l'active désigne un onglet CACHÉ derrière le PDF
/// montré, et « leaf-7 » y figure trois fois (dont une vue inconnue dans un dock replié).
fn layout_reel() -> String {
    let mut droite = racine(
        "horizontal",
        vec![json!({"type": "tabs", "currentTab": 0,
                    "children": [feuille("leaf-7", "vue-inconnue", None)]})],
    );
    droite["collapsed"] = json!(true);
    let pile = json!({"type": "tabs", "currentTab": 4, "children": [
        feuille("leaf-5", "markdown", Some("a.md")),
        feuille("leaf-7", "markdown", Some("a.md")),
        feuille("leaf-3", "markdown", Some("b.md")),
        feuille("leaf-6", "markdown", Some("b.md")),
        feuille("leaf-7", "pdf", Some("doc.pdf")),
    ]});
    json!({
        "version": 1,
        "active": "leaf-6",
        "left": racine("horizontal", vec![]),
        "right": droite,
        "main": racine("vertical", vec![pile]),
    })
    .to_string()
}

const COFFRE_PDF: &[&str] = &["doc.pdf", "a.md", "b.md"];

#[test]
fn demarrage_sans_clic_longlet_visible_est_actif_ctrlf_marche_tout_de_suite() {
    let b = lancer(COFFRE_PDF, &LayoutMemoire::avec(&layout_reel()));
    assert_eq!(b.etat(), attendu("pdf"));
    let pdf = b.ws().active_leaf().unwrap();
    assert_eq!(b.ctrl_f(), Some(false));
    assert_eq!(b.recherches(pdf), 1);
}

#[test]
fn en_session_montrer_ou_fermer_un_onglet_ne_laisse_jamais_lactive_cachee() {
    let layout = json!({
        "version": 1, "active": "leaf-a",
        "left": racine("horizontal", vec![]),
        "right": racine("horizontal", vec![]),
        "main": racine("vertical", vec![json!({"type": "tabs", "currentTab": 0,
            "children": [feuille("leaf-a", "markdown", Some("a.md"))]})]),
    });
    let b = lancer(COFFRE_PDF, &LayoutMemoire::avec(&layout.to_string()));

    // Un nouvel onglet ouvert SANS `active` (un Mod+clic de lien) est montré… donc actif.
    b.ws()
        .open_document("doc.pdf", NouvelleFeuille::Onglet, None)
        .unwrap();
    assert_eq!(b.etat(), attendu("pdf"));

    // Un onglet vide à la fin, puis on remontre le PDF par la pile seule : l'actif suit.
    let pile = b.pile_centrale();
    b.ws().create_leaf_in_parent(pile, None).unwrap(); // [a, pdf, vide]
    b.ws().select_tab(pile, 1);
    assert_eq!(b.etat(), attendu("pdf"));

    // Fermer un onglet SITUÉ AVANT l'actif ne montre pas son voisin.
    let markdown = b.ws().get_leaves_of_type("markdown")[0];
    b.ws().detach(markdown);
    assert_eq!(b.etat(), attendu("pdf"));
    let pdf = b.ws().active_leaf().unwrap();
    b.ctrl_f();
    assert_eq!(b.recherches(pdf), 1);
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  workspace-layout.spec.ts — sérialisation (§8.2) et vues différées (§8.4)
// ═══════════════════════════════════════════════════════════════════════════════════════

/// Ce qui doit survivre au redémarrage.
#[derive(Debug)]
struct Photo {
    piles: Vec<(Vec<String>, Option<usize>)>,
    differees: usize,
    materialisees: usize,
    actif: Option<String>,
    gauche_repliee: bool,
    titres: Vec<String>,
}

fn photographier(b: &Banc) -> Photo {
    let w = b.ws();
    let root = w.root_split();
    let piles = w
        .arbre()
        .piles_sous(root)
        .into_iter()
        .map(|p| {
            let onglets = w
                .arbre()
                .children(p)
                .iter()
                .map(|&l| w.leaf(l).unwrap().get_view_type().to_owned())
                .collect();
            (onglets, w.arbre().active_index(p))
        })
        .collect();
    let feuilles = w.arbre().feuilles_sous(root);
    let differees = feuilles
        .iter()
        .filter(|&&l| w.leaf(l).unwrap().is_deferred())
        .count();
    Photo {
        piles,
        differees,
        materialisees: feuilles.len() - differees,
        actif: w
            .active_leaf()
            .and_then(|a| w.document_de(a))
            .map(|d| d.to_string()),
        gauche_repliee: w.arbre().sidedock(w.left_split()).unwrap().collapsed,
        titres: feuilles
            .iter()
            .map(|&l| w.leaf(l).unwrap().get_display_text())
            .collect(),
    }
}

#[test]
fn un_coffre_neuf_demarre_sur_le_layout_par_defaut_et_lecrit() {
    let b = lancer(&["un.md"], &LayoutMemoire::new());
    let photo = photographier(&b);
    assert_eq!(photo.piles.len(), 1);
    assert_eq!(photo.actif.as_deref(), Some("un.md"));
    assert!(!photo.gauche_repliee);

    // Le layout par défaut doit être PERSISTÉ, sinon rien ne se restaure jamais.
    assert!(b.ws().save_layout().unwrap());
    let layout: Value = serde_json::from_str(&b.store.contenu().unwrap()).unwrap();
    assert_eq!(layout["version"], json!(1));
    assert_eq!(layout["main"]["type"], json!("split"));
}

#[test]
fn larbre_lepinglage_et_le_repli_reviennent_apres_un_redemarrage() {
    let store = LayoutMemoire::new();
    let fichiers = ["un.md", "deux.md", "trois.md"];
    // ── Session 1 : un layout non trivial ──
    {
        let b = lancer(&fichiers, &store);
        {
            let mut w = b.ws();
            // Une deuxième pile, à côté de la première.
            w.open_document("deux.md", NouvelleFeuille::Split, Some(actif_vrai()))
                .unwrap();
            // Un troisième onglet, épinglé, dans la pile courante.
            let leaf = w.get_leaf(NouvelleFeuille::Onglet).unwrap();
            w.open_document_in(leaf, "trois.md", Some(actif_vrai()));
            w.set_pinned(leaf, true);
            let left = w.left_split();
            w.collapse(left);
            w.save_layout().unwrap();
        }
        let avant = photographier(&b);
        assert_eq!(avant.piles.len(), 2);
        assert!(avant.gauche_repliee);
    }
    // ── Session 2 : même coffre, même écran ──
    let b = lancer(&fichiers, &store);
    let apres = photographier(&b);
    assert_eq!(apres.piles.len(), 2);
    assert!(apres.gauche_repliee);
    assert_eq!(apres.actif.as_deref(), Some("trois.md"));
    let epingles: Vec<String> = {
        let w = b.ws();
        w.iterate_all_leaves()
            .into_iter()
            .filter(|&l| w.leaf(l).unwrap().pinned())
            .map(|l| w.document_de(l).map(|d| d.to_string()).unwrap_or_default())
            .collect()
    };
    assert_eq!(epingles, vec!["trois.md".to_owned()]);

    // ★ Un dock restauré REPLIÉ n'a pas matérialisé ses feuilles : le déplier les charge.
    let (left, explorateur) = {
        let w = b.ws();
        let left = w.left_split();
        let pile = w.arbre().children(left)[0];
        (left, w.arbre().active_leaf(pile).unwrap())
    };
    assert!(b.ws().leaf(explorateur).unwrap().is_deferred());
    b.ws().expand(left);
    let w = b.ws();
    let l = w.leaf(explorateur).unwrap();
    // ★ La VUE montée distingue la coquille de la vraie chose ; la feuille, elle, répondait
    //   « explorateur » même différée.
    assert_eq!(
        (l.is_deferred(), l.view().get_view_type()),
        (false, "explorateur".into())
    );
}

#[test]
fn seules_les_feuilles_visibles_sont_materialisees_un_clic_charge_la_suivante() {
    let store = LayoutMemoire::new();
    let fichiers = ["un.md", "deux.md", "trois.md"];
    {
        let b = lancer(&fichiers, &store);
        let mut w = b.ws();
        // Trois onglets dans UNE pile, dans un ordre choisi.
        let leaf = w.get_leaf(NouvelleFeuille::Non).unwrap();
        w.open_document_in(leaf, "un.md", Some(actif_vrai()));
        for n in ["deux.md", "trois.md"] {
            w.open_document(n, NouvelleFeuille::Onglet, Some(actif_vrai()))
                .unwrap();
        }
        w.save_layout().unwrap();
    }
    let b = lancer(&fichiers, &store);
    // La preuve du différé : une seule vue construite sur les trois.
    let photo = photographier(&b);
    assert_eq!(photo.piles[0].0.len(), 3);
    assert_eq!(photo.materialisees, 1);
    assert_eq!(photo.differees, 2);
    // ★ Les onglets non matérialisés portent quand même le BON titre (la coquille se nomme).
    let mut titres = photo.titres.clone();
    titres.sort();
    assert_eq!(titres, vec!["deux", "trois", "un"]);

    // Montrer le premier onglet le matérialise — et rien d'autre.
    let pile = b.pile_centrale();
    b.ws().select_tab(pile, 0);
    let photo = photographier(&b);
    assert_eq!((photo.materialisees, photo.differees), (2, 1));
}

#[test]
fn un_onglet_vide_restaure_reste_un_onglet_vide_pas_une_vue_introuvable() {
    let store = LayoutMemoire::new();
    {
        let b = lancer(&["un.md"], &store);
        let pile = b.pile_centrale();
        let mut w = b.ws();
        // Ce que produit le bouton « + » de la barre d'onglets.
        let vide = w.create_leaf_in_parent(pile, None).unwrap();
        w.set_active_leaf(Some(vide));
        w.save_layout().unwrap();
    }
    let b = lancer(&["un.md"], &store);
    let pile = b.pile_centrale();
    let w = b.ws();
    let enfants = w.arbre().children(pile);
    let vide = w.leaf(*enfants.last().unwrap()).unwrap();
    let message = vide
        .view()
        .as_any()
        .downcast_ref::<EmptyView>()
        .map(EmptyView::message)
        .unwrap();
    // Le contrat : un onglet vide RESTAURÉ est indiscernable d'un onglet vide NEUF.
    assert!(!message.contains("Aucune vue enregistrée"));
    assert_eq!(message, "");
    assert_eq!(vide.get_display_text(), "New Tab");
    assert!(!vide.is_deferred()); // rien à différer
    // ★ Et le voisin, lui, est resté une note (différée) : le cas de l'onglet vide ne
    //   désactive pas le §8.4.
    assert_eq!(w.leaf(enfants[0]).unwrap().get_view_type(), "markdown");
}

#[test]
fn un_workspacejson_corrompu_ne_bloque_pas_le_demarrage() {
    let store = LayoutMemoire::avec("{\"version\":1,\"main\":{\"type\":\"spl"); // écriture coupée
    let b = lancer(&["un.md"], &store);
    let photo = photographier(&b);
    // Layout par défaut, exactement comme un coffre neuf.
    assert_eq!(photo.piles.len(), 1);
    assert_eq!(photo.actif.as_deref(), Some("un.md"));
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  WorkspaceCommandsPlugin.test.ts — next-tab / previous-tab après correctif, sur le vrai
//  Workspace (les deux cas « LE BUG » reproduisent l'ancienne logique : voir parite.toml).
// ═══════════════════════════════════════════════════════════════════════════════════════

/// Une pile [a, b, c] dont `a` est l'onglet visible et actif.
fn pile_abc() -> (Banc, ItemId, Vec<ItemId>) {
    let b = lancer(&["a.md", "b.md", "c.md"], &LayoutMemoire::new());
    b.ouvrir(&["b", "c"]);
    let pile = b.pile_centrale();
    b.ws().activer_onglet(pile, 0);
    let enfants = b.ws().arbre().children(pile).to_vec();
    (b, pile, enfants)
}

#[test]
fn defile_a_b_c_a_depuis_longlet_visible_et_realigne_activeleaf() {
    let (b, pile, abc) = pile_abc();
    let mut visite = Vec::new();
    for _ in 0..4 {
        assert!(b.executer("workspace:next-tab"));
        visite.push(b.ws().arbre().active_index(pile).unwrap());
    }
    assert_eq!(visite, vec![1, 2, 0, 1]); // vrai défilement cyclique
    assert_eq!(b.ws().active_leaf(), Some(abc[1])); // l'actif suit l'onglet visible
}

#[test]
fn previous_tab_defile_en_sens_inverse_a_c_b_a() {
    let (b, pile, _) = pile_abc();
    let mut visite = Vec::new();
    for _ in 0..3 {
        assert!(b.executer("workspace:previous-tab"));
        visite.push(b.ws().arbre().active_index(pile).unwrap());
    }
    assert_eq!(visite, vec![2, 1, 0]);
}

#[test]
fn repart_de_longlet_visible_meme_si_activeleaf_a_derive_le_coeur_du_fix() {
    let (b, pile, abc) = pile_abc();
    // L'utilisateur montre c. (★ Ici l'actif ne PEUT plus dériver : montrer un onglet de la
    // pile de l'active l'active. Le cas reste vrai : le départ est l'onglet VISIBLE.)
    b.ws().select_tab(pile, 2);
    assert!(b.executer("workspace:next-tab"));
    assert_eq!(b.ws().arbre().active_index(pile), Some(0)); // depuis c visible → a
    assert_eq!(b.ws().active_leaf(), Some(abc[0]));
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  Ajoutés : ce que les specs supposent sans le tester
// ═══════════════════════════════════════════════════════════════════════════════════════

#[test]
fn deux_etages_dactif_la_feuille_hebergee_et_son_hote() {
    let b = lancer(&["a.md", "b.md"], &LayoutMemoire::new());
    b.ouvrir(&["a"]);
    let hote = b.ws().active_leaf().unwrap();
    let hebergee = b.ws().create_hosted_leaf(hote);
    b.ws()
        .set_view_state(hebergee, ViewState::new("chat"), None);

    // Seule une feuille hébergée PAR l'active est acceptée.
    b.ws().set_active_hosted_leaf(Some(hebergee));
    assert_eq!(b.ws().active_hosted_leaf(), Some(hebergee));
    assert_eq!(b.ws().active_leaf(), Some(hote)); // l'actif reste l'HÔTE
    // Sa portée est au sommet, au-dessus de celle de l'hôte : Mod+F va au chat.
    b.ctrl_f();
    assert_eq!((b.recherches(hebergee), b.recherches(hote)), (1, 0));

    // Un appui dans la feuille hébergée active l'hôte (son contenu vit dans l'hôte).
    b.ouvrir(&["b"]);
    assert_eq!(b.ws().active_hosted_leaf(), None); // l'hôte a changé : le second étage tombe
    b.ws().activer_par_contact(hebergee);
    assert_eq!(b.ws().active_leaf(), Some(hote));
}

#[test]
fn une_modale_ouverte_reste_au_dessus_quand_lactif_change() {
    let b = lancer(&["a.md", "b.md"], &LayoutMemoire::new());
    b.ouvrir(&["a"]);
    let modale = Scope::echap(|| {});
    b.app.keymap().push_scope(&modale);
    b.ouvrir(&["b"]);
    assert_eq!(b.app.keymap().scope(), modale);
    b.app.keymap().pop_scope(&modale);
    let actif = b.ws().active_leaf().unwrap();
    let s = b.ws().leaf(actif).unwrap().view().scope().unwrap();
    assert_eq!(b.app.keymap().scope(), s);
}

#[test]
fn fermer_le_dernier_onglet_dune_pile_du_centre_ferme_la_pile_et_garde_un_actif() {
    let b = lancer(&["a.md", "b.md"], &LayoutMemoire::new());
    b.ws()
        .open_document("b.md", NouvelleFeuille::Split, Some(actif_vrai()))
        .unwrap();
    let n_piles = |b: &Banc| {
        let w = b.ws();
        w.arbre().piles_sous(w.root_split()).len()
    };
    assert_eq!(n_piles(&b), 2);
    let a = b.ws().active_leaf().unwrap();
    b.ws().detach(a);
    assert_eq!(n_piles(&b), 1);
    assert_eq!(b.actif().unwrap().1.as_deref(), Some("a")); // ★ jamais vide
    // Le dernier onglet du dernier pane : un onglet vide le remplace, actif.
    let a = b.ws().active_leaf().unwrap();
    b.ws().detach(a);
    assert_eq!(n_piles(&b), 1);
    assert_eq!(b.actif().unwrap().0, "empty");
}

#[test]
fn lhistorique_revient_et_repart_la_restauration_ne_lecrit_pas() {
    let store = LayoutMemoire::new();
    let b = lancer(&["a.md", "b.md"], &store);
    assert!(!b.executer("app:go-back")); // rien derrière : indisponible
    let leaf = b.ws().active_leaf().unwrap();
    b.ws().open_document_in(leaf, "b.md", None);
    assert_eq!(b.actif().unwrap().1.as_deref(), Some("b"));
    assert!(b.executer("app:go-back"));
    assert_eq!(b.actif().unwrap().1.as_deref(), Some("a"));
    assert!(b.executer("app:go-forward"));
    assert_eq!(b.actif().unwrap().1.as_deref(), Some("b"));
    b.ws().save_layout().unwrap();
    drop(b);
    let b = lancer(&["a.md", "b.md"], &store);
    assert!(!b.executer("app:go-back")); // rouvrir l'app n'est pas « naviguer »
}

#[test]
fn un_type_inconnu_attend_son_plugin_et_garde_sa_demande_au_layout() {
    let b = lancer(&["a.md"], &LayoutMemoire::new());
    let leaf = b.ws().get_leaf(NouvelleFeuille::Onglet).unwrap();
    b.ws().set_view_state(leaf, ViewState::new("kanban"), None);
    {
        let w = b.ws();
        let l = w.leaf(leaf).unwrap();
        assert!(l.is_placeholder());
        assert_eq!(l.get_view_type(), "kanban");
        assert_eq!(l.view().get_view_type(), "empty");
        assert_eq!(l.get_display_text(), "Vue indisponible");
        // ★ La sérialisation garde ce que l'onglet voulait.
        assert_eq!(l.serialize().state.type_, "kanban");
    }
    // Le plugin arrive : l'onglet devient la vraie vue, sans rechargement.
    let _r = b
        .app
        .view_registry()
        .register("kanban", fabrique("kanban"), "plugin");
    let w = b.ws();
    assert_eq!(w.leaf(leaf).unwrap().view().get_view_type(), "kanban");
}

#[test]
fn diviser_ouvre_la_meme_vue_dans_le_nouveau_pane_actif() {
    let b = lancer(&["a.md"], &LayoutMemoire::new());
    let avant = b.ws().active_leaf().unwrap();
    b.ws().set_pinned(avant, true);
    assert!(b.executer("workspace:split-vertical"));
    let apres = b.ws().active_leaf().unwrap();
    assert_ne!(apres, avant);
    assert_eq!(b.actif(), Some(("markdown".into(), Some("a".into()))));
    assert!(!b.ws().leaf(apres).unwrap().pinned()); // la vue, pas l'épinglage
    let w = b.ws();
    assert_eq!(w.arbre().piles_sous(w.root_split()).len(), 2);
}

#[test]
fn les_commandes_de_docks_et_dongles_agissent_sur_le_workspace() {
    let b = lancer(&["a.md"], &LayoutMemoire::new());
    let replie = |b: &Banc| {
        let w = b.ws();
        w.arbre().sidedock(w.left_split()).unwrap().collapsed
    };
    assert!(!replie(&b));
    assert!(b.executer("app:toggle-left-sidebar"));
    assert!(replie(&b));
    assert!(b.executer("workspace:new-tab"));
    assert_eq!(b.actif().unwrap().0, "empty");
    assert!(b.executer("workspace:toggle-pin"));
    let a = b.ws().active_leaf().unwrap();
    assert!(b.ws().leaf(a).unwrap().pinned());
    assert!(b.executer("workspace:close-others"));
    let w = b.ws();
    assert_eq!(w.arbre().children(w.arbre().parent(a).unwrap()), &[a]);
}

#[test]
fn layout_change_une_fois_par_operation_et_rien_avant_le_layout_pret() {
    let b = lancer(&["a.md", "b.md"], &LayoutMemoire::new());
    let n = Rc::new(Cell::new(0));
    let c = n.clone();
    let _r = b
        .ws()
        .events()
        .on(fragment_core::LAYOUT_CHANGE, move |_| c.set(c.get() + 1));
    // Déplacer une pile : la structure change, une émission.
    b.ws()
        .open_document("b.md", NouvelleFeuille::Split, None)
        .unwrap();
    n.set(0);
    let (root, deuxieme) = {
        let w = b.ws();
        let root = w.root_split();
        (root, w.arbre().children(root)[1])
    };
    b.ws().move_item(root, deuxieme, Some(0)).unwrap();
    assert_eq!(n.get(), 1);
    // Avant le layout prêt, une sauvegarde n'écrit rien.
    let app = AppCoeur::new();
    let store = LayoutMemoire::new();
    let mut w = app.workspace().borrow_mut();
    w.set_layout_store(Rc::new(store.clone()));
    assert!(!w.save_layout().unwrap());
    assert!(store.lire().unwrap().is_none());
}

#[test]
fn active_editor_rend_l_editeur_de_la_feuille_active() {
    let b = lancer(&["a.md"], &LayoutMemoire::new());
    let ctx = b
        .app
        .active_editor()
        .expect("la note active expose son éditeur");
    assert_eq!(
        ctx.editor.downcast_ref::<String>().map(String::as_str),
        Some("markdown")
    );
    // Une feuille sans éditeur (le PDF) : rien.
    let feuille = b.ws().active_leaf().unwrap();
    b.app.workspace().borrow_mut().set_view_state(
        feuille,
        ViewState::with_state("pdf", json!({ "file": "a.pdf" })),
        None,
    );
    assert!(b.app.active_editor().is_none());
}
