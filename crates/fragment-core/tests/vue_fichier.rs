//! Tests des vues de fichier (fr-t2-fileview) : `FileView`, `EditableFileView`, `TextFileView`.
//! Port de app/src/ui/core/workspace/{FileView,EditableFileView,TextFileView}.ts. Ces trois
//! fichiers n'ont AUCUN test unitaire côté TS : ce sont tous des cas AJOUTÉS (voir
//! `parite.d/fr-t2-fileview.toml`), écrits d'après le comportement du code source et la spec
//! e2e `view-header-nav` (le fil et l'historique). Données et fixtures réécrites.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::documents::{DocumentProvider, OWNER_CORE};
use fragment_core::keymap::Keymap;
use fragment_core::workspace::vue_fichier::{
    ContexteFichier, EditableFileView, FileView, FileViewBase, REVEAL_IN_FOLDER, RevealInFolder,
    SAVE_DELAY_MS, TextFileBase, TextFileView, VIEW_DOCUMENT_CHANGE, VIEW_FILE_OPEN,
    ViewDocumentChange, ViewFileOpen, sanitize_pasted_title,
};
use fragment_core::{
    App, AppCoeur, BreadcrumbSegment, ChangeType, ContexteVue, DataAdapter, DocumentKey, Events,
    ItemId, ItemView, ItemViewBase, MemoryAdapter, NouvelleFeuille, Scope, TFile, Vault, View,
    ViewCreator, ViewState, ViewStateResult,
};
use serde_json::{Map, Value, json};

type Journal = Rc<RefCell<Vec<String>>>;

// ─── Les vues de test ───

/// Une vue de fichier minimale : elle journalise ses hooks.
struct VueFichier {
    base: ItemViewBase,
    fichier: FileViewBase,
    journal: Journal,
    refuse: Option<&'static str>,
}

/// Une vue de texte : l'éditeur est une simple chaîne.
struct VueTexte {
    base: ItemViewBase,
    fichier: FileViewBase,
    texte: TextFileBase,
    journal: Journal,
    editeur: String,
}

macro_rules! impl_view_fichier {
    ($t:ty, $type_:expr) => {
        impl View for $t {
            fn get_view_type(&self) -> String {
                $type_.to_owned()
            }
            fn get_display_text(&self) -> String {
                self.file_display_text()
            }
            fn get_document(&self) -> Option<DocumentKey> {
                self.file_document()
            }
            fn navigation(&self) -> bool {
                true
            }
            fn scope(&self) -> Option<Scope> {
                Some(self.base.scope.clone())
            }
            fn on_open(&mut self) {
                self.abonner_coffre();
            }
            fn on_close(&mut self) {
                self.desabonner_coffre();
            }
            fn get_state(&self) -> Map<String, Value> {
                self.file_state()
            }
            fn set_state(&mut self, state: &Map<String, Value>, _: ViewStateResult) {
                self.set_file_state(state);
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
        }
        impl ItemView for $t {
            fn item_scope(&self) -> &Scope {
                &self.base.scope
            }
        }
    };
}

impl_view_fichier!(VueFichier, "fichier");
impl_view_fichier!(VueTexte, "texte");

impl FileView for VueFichier {
    fn file_base(&self) -> &FileViewBase {
        &self.fichier
    }
    fn file_base_mut(&mut self) -> &mut FileViewBase {
        &mut self.fichier
    }
    fn on_load_file(&mut self, f: &TFile) {
        self.journal.borrow_mut().push(format!("load {}", f.path));
    }
    fn on_unload_file(&mut self, f: &TFile) {
        self.journal.borrow_mut().push(format!("unload {}", f.path));
    }
    fn on_rename(&mut self, f: &TFile) {
        self.journal.borrow_mut().push(format!("rename {}", f.path));
    }
    fn can_accept_extension(&self, ext: &str) -> bool {
        self.refuse != Some(ext)
    }
}

impl FileView for VueTexte {
    fn file_base(&self) -> &FileViewBase {
        &self.fichier
    }
    fn file_base_mut(&mut self) -> &mut FileViewBase {
        &mut self.fichier
    }
    fn on_load_file(&mut self, f: &TFile) {
        self.text_on_load_file(f);
    }
    fn on_unload_file(&mut self, f: &TFile) {
        self.text_on_unload_file(f);
    }
    fn on_vault_modify(&mut self, f: &TFile) {
        self.text_on_vault_modify(f);
    }
    fn est_modifiable(&self) -> bool {
        true
    }
}

impl EditableFileView for VueTexte {}

impl TextFileView for VueTexte {
    fn text_base(&self) -> &TextFileBase {
        &self.texte
    }
    fn text_base_mut(&mut self) -> &mut TextFileBase {
        &mut self.texte
    }
    fn get_view_data(&self) -> String {
        self.editeur.clone()
    }
    fn set_view_data(&mut self, data: &str, clear: bool) {
        self.journal
            .borrow_mut()
            .push(format!("set_view_data clear={clear}"));
        self.editeur = data.to_owned();
    }
    fn clear(&mut self) {
        self.journal.borrow_mut().push("clear".to_owned());
        self.editeur.clear();
    }
}

// ─── Le banc ───

struct Banc {
    disque: Rc<MemoryAdapter>,
    vault: Rc<Vault>,
    events: Events,
    /// Les événements du Workspace, une ligne chacun.
    annonces: Journal,
    journal: Journal,
    /// Une vraie feuille (un `ItemId` ne se fabrique pas), tenue en vie par son App.
    leaf: ItemId,
    _app: Rc<AppCoeur>,
}

fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("racine.md", "# Racine");
    disque.put("notes/idees.md", "des idées");
    disque.put("notes/autre.md", "autre chose");
    disque.put("a/b/c/d/e/profond.md", "tout au fond");
    disque.put("images/logo.png", "png");
    disque.put_dir("vide");
    let adaptateur: Rc<dyn DataAdapter> = disque.clone();
    let vault = Vault::new(adaptateur);
    vault.load().unwrap();
    let events = Events::new();
    let annonces: Journal = Rc::default();
    let a = annonces.clone();
    events.on(VIEW_FILE_OPEN, move |d| {
        let e = d.downcast_ref::<ViewFileOpen>().unwrap();
        a.borrow_mut().push(format!("file-open {}", e.file.path));
    });
    let a = annonces.clone();
    events.on(VIEW_DOCUMENT_CHANGE, move |d| {
        let e = d.downcast_ref::<ViewDocumentChange>().unwrap();
        a.borrow_mut()
            .push(format!("document-change {}", e.document.as_str()));
    });
    let a = annonces.clone();
    events.on(REVEAL_IN_FOLDER, move |d| {
        let e = d.downcast_ref::<RevealInFolder>().unwrap();
        a.borrow_mut().push(format!("reveal {}", e.folder.path));
    });
    let app = AppCoeur::new();
    let leaf = app
        .workspace()
        .borrow_mut()
        .get_leaf(NouvelleFeuille::Onglet)
        .unwrap();
    Banc {
        disque,
        vault,
        events,
        annonces,
        journal: Rc::default(),
        leaf,
        _app: app,
    }
}

impl Banc {
    fn ctx(&self) -> ContexteFichier {
        ContexteFichier {
            leaf: self.leaf,
            vault: self.vault.clone(),
            workspace_events: self.events.clone(),
        }
    }

    fn vue(&self) -> VueFichier {
        let k = Keymap::new();
        let mut v = VueFichier {
            base: ItemViewBase::new(k.root_scope()),
            fichier: FileViewBase::new(self.ctx()),
            journal: self.journal.clone(),
            refuse: None,
        };
        v.on_open();
        v
    }

    fn texte(&self) -> VueTexte {
        let k = Keymap::new();
        let mut v = VueTexte {
            base: ItemViewBase::new(k.root_scope()),
            fichier: FileViewBase::new(self.ctx()),
            texte: TextFileBase::new(),
            journal: self.journal.clone(),
            editeur: String::new(),
        };
        v.on_open();
        v
    }

    fn fichier(&self, path: &str) -> TFile {
        self.vault.get_file_by_path(path).unwrap()
    }

    fn lignes(journal: &Journal) -> Vec<String> {
        journal.borrow().clone()
    }

    fn vider(&self) {
        self.annonces.borrow_mut().clear();
        self.journal.borrow_mut().clear();
    }
}

fn opts() -> fragment_core::DataWriteOptions {
    fragment_core::DataWriteOptions::default()
}

// ─── FileView : le fichier courant ───

#[test]
fn charger_un_fichier_pose_le_fichier_et_le_document() {
    let b = banc();
    let mut v = b.vue();
    assert!(v.file().is_none());
    assert_eq!(v.file_document(), None);
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert_eq!(v.file().unwrap().path, "notes/idees.md");
    // La clé du document : le chemin du fichier.
    assert_eq!(v.get_document().unwrap().as_str(), "notes/idees.md");
    assert!(v.navigation());
}

#[test]
fn charger_le_meme_fichier_ne_relance_rien() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("racine.md")));
    b.vider();
    v.load_file(Some(b.fichier("racine.md")));
    assert!(Banc::lignes(&b.journal).is_empty());
    assert!(Banc::lignes(&b.annonces).is_empty());
}

#[test]
fn changer_de_fichier_decharge_l_ancien_puis_charge_le_nouveau() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("racine.md")));
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert_eq!(
        Banc::lignes(&b.journal),
        ["load racine.md", "unload racine.md", "load notes/idees.md"]
    );
}

#[test]
fn charger_annonce_view_file_open_puis_view_document_change() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert_eq!(
        Banc::lignes(&b.annonces),
        ["file-open notes/idees.md", "document-change notes/idees.md"]
    );
}

#[test]
fn charger_aucun_fichier_decharge_sans_rien_annoncer() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("racine.md")));
    b.vider();
    v.load_file(None);
    assert_eq!(Banc::lignes(&b.journal), ["unload racine.md"]);
    assert!(Banc::lignes(&b.annonces).is_empty());
    assert!(v.file().is_none());
    assert!(v.breadcrumb().is_none());
}

#[test]
fn le_titre_est_le_nom_sans_extension_ou_aucun_fichier() {
    let b = banc();
    let mut v = b.vue();
    assert_eq!(v.get_display_text(), "Aucun fichier");
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert_eq!(v.get_display_text(), "idees");
}

// ─── FileView : l'état sérialisable ───

#[test]
fn get_state_porte_le_chemin_et_null_sans_fichier() {
    let b = banc();
    let mut v = b.vue();
    assert_eq!(Value::Object(v.get_state()), json!({"file": null}));
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert_eq!(
        Value::Object(v.get_state()),
        json!({"file": "notes/idees.md"})
    );
}

#[test]
fn set_state_resout_le_chemin_et_charge_le_fichier() {
    let b = banc();
    let mut v = b.vue();
    let etat = json!({"file": "notes/autre.md"});
    v.set_state(etat.as_object().unwrap(), ViewStateResult::default());
    assert_eq!(v.file().unwrap().path, "notes/autre.md");
    assert_eq!(Banc::lignes(&b.journal), ["load notes/autre.md"]);
}

#[test]
fn set_state_fichier_absent_laisse_la_vue_comme_elle_etait() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("racine.md")));
    b.vider();
    let etat = json!({"file": "disparu.md"});
    v.set_state(etat.as_object().unwrap(), ViewStateResult::default());
    assert_eq!(v.file().unwrap().path, "racine.md");
    assert!(Banc::lignes(&b.journal).is_empty());
}

#[test]
fn set_state_extension_refusee_ne_charge_rien() {
    let b = banc();
    let mut v = b.vue();
    v.refuse = Some("png");
    let etat = json!({"file": "images/logo.png"});
    v.set_state(etat.as_object().unwrap(), ViewStateResult::default());
    assert!(v.file().is_none());
    assert!(Banc::lignes(&b.journal).is_empty());
}

#[test]
fn set_state_sans_chemin_texte_est_ignore() {
    let b = banc();
    let mut v = b.vue();
    for etat in [json!({}), json!({"file": null}), json!({"file": 3})] {
        v.set_state(etat.as_object().unwrap(), ViewStateResult::default());
        assert!(v.file().is_none());
    }
}

#[test]
fn un_dossier_n_est_pas_un_fichier_au_set_state() {
    let b = banc();
    let mut v = b.vue();
    let etat = json!({"file": "notes"});
    v.set_state(etat.as_object().unwrap(), ViewStateResult::default());
    assert!(v.file().is_none());
}

// ─── FileView : le fil d'Ariane ───

fn noms(v: &VueFichier) -> Vec<String> {
    v.breadcrumb()
        .unwrap()
        .segments
        .iter()
        .map(|s| match s {
            BreadcrumbSegment::Folder(f) => f.name.clone(),
            BreadcrumbSegment::Ellipsis { .. } => "…".to_owned(),
        })
        .collect()
}

#[test]
fn le_fil_donne_les_dossiers_le_titre_et_le_chemin() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    let fil = v.breadcrumb().unwrap();
    assert_eq!(noms(&v), ["notes"]);
    assert_eq!(fil.title, "idees");
    assert_eq!(fil.path, "notes/idees.md");
    assert!(v.hidden_folders_menu().is_empty());
}

#[test]
fn un_fichier_a_la_racine_n_a_aucun_segment() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("racine.md")));
    assert!(noms(&v).is_empty());
}

#[test]
fn le_fil_replie_le_milieu_en_ellipse_et_garde_les_dossiers_masques() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("a/b/c/d/e/profond.md")));
    // Cinq dossiers : le premier, « … », les deux derniers.
    assert_eq!(noms(&v), ["a", "…", "d", "e"]);
    let menu = v.hidden_folders_menu();
    assert_eq!(
        menu.iter()
            .map(|e| (e.title.as_str(), e.path.as_str(), e.icon))
            .collect::<Vec<_>>(),
        [("b", "a/b", "folder"), ("c", "a/b/c", "folder")]
    );
}

#[test]
fn cliquer_un_dossier_du_fil_annonce_reveal_in_folder() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vider();
    v.reveal_folder("notes");
    assert_eq!(Banc::lignes(&b.annonces), ["reveal notes"]);
}

#[test]
fn reveal_in_folder_ignore_un_chemin_inconnu_ou_un_fichier() {
    let b = banc();
    let v = b.vue();
    v.reveal_folder("nulle/part");
    v.reveal_folder("racine.md");
    assert!(Banc::lignes(&b.annonces).is_empty());
}

// ─── FileView : les événements du coffre ───

#[test]
fn renommer_le_fichier_previent_la_vue_met_a_jour_le_fil_et_le_document() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vider();
    b.vault
        .rename(&b.fichier("notes/idees.md"), "notes/projets.md")
        .unwrap();
    v.traiter_evenements_coffre();
    assert_eq!(v.file().unwrap().path, "notes/projets.md");
    assert_eq!(Banc::lignes(&b.journal), ["rename notes/projets.md"]);
    // La clé du document a changé : les calques la relisent.
    assert_eq!(
        Banc::lignes(&b.annonces),
        ["document-change notes/projets.md"]
    );
    assert_eq!(v.breadcrumb().unwrap().title, "projets");
    assert_eq!(v.breadcrumb().unwrap().path, "notes/projets.md");
}

#[test]
fn renommer_un_dossier_ancetre_change_le_chemin_sans_on_rename() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vider();
    let notes = b.vault.get_folder_by_path("notes").unwrap();
    b.vault.rename(&notes, "carnet").unwrap();
    v.traiter_evenements_coffre();
    assert_eq!(v.file().unwrap().path, "carnet/idees.md");
    assert_eq!(noms(&v), ["carnet"]);
    // L'événement ne porte pas NOTRE fichier : ni hook, ni changement de document.
    assert!(Banc::lignes(&b.journal).is_empty());
    assert!(Banc::lignes(&b.annonces).is_empty());
}

#[test]
fn renommer_un_autre_fichier_ne_touche_pas_la_vue() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vider();
    b.vault
        .rename(&b.fichier("notes/autre.md"), "notes/ailleurs.md")
        .unwrap();
    v.traiter_evenements_coffre();
    assert_eq!(v.file().unwrap().path, "notes/idees.md");
    assert!(Banc::lignes(&b.journal).is_empty());
}

#[test]
fn supprimer_le_fichier_vide_la_vue_et_le_fil() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vault.delete(&b.fichier("notes/idees.md"), true).unwrap();
    v.traiter_evenements_coffre();
    assert!(v.file().is_none());
    assert!(v.breadcrumb().is_none());
    assert_eq!(v.get_display_text(), "Aucun fichier");
}

#[test]
fn supprimer_un_autre_fichier_ne_touche_pas_la_vue() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vault.delete(&b.fichier("notes/autre.md"), true).unwrap();
    v.traiter_evenements_coffre();
    assert_eq!(v.file().unwrap().path, "notes/idees.md");
}

#[test]
fn une_vue_demontee_n_ecoute_plus_le_coffre() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.on_close();
    b.vault.delete(&b.fichier("notes/idees.md"), true).unwrap();
    v.traiter_evenements_coffre();
    assert!(v.file().is_some());
    // Et remonter la vue se réabonne (une seule fois).
    v.on_open();
    v.on_open();
    // Le fichier a déjà disparu du coffre : le nouvel événement vient d'un autre.
    b.vault.delete(&b.fichier("notes/autre.md"), true).unwrap();
    v.traiter_evenements_coffre();
    assert!(v.file().is_some());
}

// ─── FileView : le renommage par le titre ───

#[test]
fn renommer_garde_le_dossier_et_l_extension() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert!(v.rename_basename("  projets  "));
    v.traiter_evenements_coffre();
    assert_eq!(v.file().unwrap().path, "notes/projets.md");
    assert!(b.vault.get_file_by_path("notes/idees.md").is_none());
}

#[test]
fn renommer_un_fichier_a_la_racine() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("racine.md")));
    assert!(v.rename_basename("base"));
    v.traiter_evenements_coffre();
    assert_eq!(v.file().unwrap().path, "base.md");
}

#[test]
fn renommer_avec_un_nom_vide_ou_inchange_ne_fait_rien() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert!(!v.rename_basename(""));
    assert!(!v.rename_basename("   "));
    assert!(!v.rename_basename("idees"));
    assert!(b.vault.get_file_by_path("notes/idees.md").is_some());
}

#[test]
fn renommer_sans_fichier_ne_fait_rien() {
    let b = banc();
    let mut v = b.vue();
    assert!(!v.rename_basename("x"));
    assert_eq!(v.start_header_rename(), None);
}

#[test]
fn un_refus_du_coffre_est_journalise_et_le_fichier_reste() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    // « autre » existe déjà dans le même dossier : collision.
    assert!(!v.rename_basename("autre"));
    assert_eq!(v.file().unwrap().path, "notes/idees.md");
    assert!(b.vault.get_file_by_path("notes/autre.md").is_some());
}

#[test]
fn editer_le_titre_propose_le_nom_sans_extension_et_valide() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert!(!v.is_renaming());
    assert_eq!(v.start_header_rename().as_deref(), Some("idees"));
    assert!(v.is_renaming());
    // Un second clic pendant l'édition ne la relance pas.
    assert_eq!(v.start_header_rename(), None);
    v.finish_header_rename("projets");
    assert!(!v.is_renaming());
    assert_eq!(v.file().unwrap().path, "notes/projets.md");
    assert_eq!(v.breadcrumb().unwrap().title, "projets");
}

#[test]
fn echap_pendant_l_edition_annule_le_renommage() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.start_header_rename();
    v.cancel_header_rename();
    v.finish_header_rename("projets");
    assert!(!v.is_renaming());
    assert_eq!(v.file().unwrap().path, "notes/idees.md");
    // Une édition suivante n'hérite pas de l'annulation.
    v.start_header_rename();
    v.finish_header_rename("projets");
    assert_eq!(v.file().unwrap().path, "notes/projets.md");
}

#[test]
fn un_rename_venu_d_ailleurs_ne_redessine_pas_le_fil_pendant_la_saisie() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("a/b/c/d/e/profond.md")));
    v.start_header_rename();
    let notes = b.vault.get_folder_by_path("notes").unwrap();
    b.vault.rename(&notes, "carnet").unwrap();
    v.traiter_evenements_coffre();
    // Le fil gardé tel quel : le champ n'est pas détruit.
    assert_eq!(noms(&v), ["a", "…", "d", "e"]);
    v.finish_header_rename("profond");
    assert_eq!(noms(&v), ["a", "…", "d", "e"]);
}

#[test]
fn finir_une_edition_refusee_affiche_la_verite_du_coffre() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.start_header_rename();
    v.finish_header_rename("autre");
    assert_eq!(v.breadcrumb().unwrap().title, "idees");
}

#[test]
fn finir_sans_edition_en_cours_ne_fait_rien() {
    let b = banc();
    let mut v = b.vue();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.finish_header_rename("projets");
    assert_eq!(v.file().unwrap().path, "notes/idees.md");
}

#[test]
fn un_collage_dans_le_titre_perd_ses_retours_a_la_ligne() {
    assert_eq!(
        sanitize_pasted_title("un\ndeux\r\ntrois\rquatre"),
        "un deux trois quatre"
    );
    assert_eq!(sanitize_pasted_title("simple"), "simple");
}

// ─── EditableFileView ───

#[test]
fn seule_une_vue_modifiable_repond_oui() {
    let b = banc();
    assert!(!b.vue().est_modifiable());
    assert!(b.texte().est_modifiable());
}

// ─── TextFileView ───

#[test]
fn charger_lit_le_disque_et_pose_le_texte_comme_neuf() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert_eq!(v.editeur, "des idées");
    assert_eq!(v.texte.data, "des idées");
    assert_eq!(Banc::lignes(&b.journal), ["set_view_data clear=true"]);
}

#[test]
fn sauvegarder_ecrit_par_le_coffre_et_garde_le_texte() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.editeur = "des idées nouvelles".to_owned();
    v.save(false).unwrap();
    assert_eq!(
        b.vault.read(&b.fichier("notes/idees.md")).unwrap(),
        "des idées nouvelles"
    );
    assert_eq!(v.texte.data, "des idées nouvelles");
    assert_eq!(v.editeur, "des idées nouvelles");
}

#[test]
fn sauvegarder_avec_clear_vide_l_editeur() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.editeur = "final".to_owned();
    v.save(true).unwrap();
    assert_eq!(b.vault.read(&b.fichier("notes/idees.md")).unwrap(), "final");
    assert_eq!(v.editeur, "");
}

#[test]
fn sauvegarder_sans_fichier_ne_fait_rien() {
    let b = banc();
    let mut v = b.texte();
    v.editeur = "orphelin".to_owned();
    v.save(true).unwrap();
    assert_eq!(v.editeur, "orphelin");
}

#[test]
fn la_sauvegarde_differee_attend_500_ms_apres_la_derniere_demande() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    assert_eq!(SAVE_DELAY_MS, 500);
    v.editeur = "un".to_owned();
    v.request_save(1_000);
    assert_eq!(v.next_save_deadline(), Some(1_500));
    // Une frappe de plus repart du délai complet.
    v.editeur = "deux".to_owned();
    v.request_save(1_400);
    assert_eq!(v.next_save_deadline(), Some(1_900));
    assert!(!v.poll_save(1_899));
    assert_eq!(
        b.vault.read(&b.fichier("notes/idees.md")).unwrap(),
        "des idées"
    );
    assert!(v.poll_save(1_900));
    assert_eq!(b.vault.read(&b.fichier("notes/idees.md")).unwrap(), "deux");
    // Une seule écriture : rien d'autre n'est dû.
    assert!(!v.poll_save(5_000));
    assert_eq!(v.next_save_deadline(), None);
}

#[test]
fn poll_sans_demande_ne_sauvegarde_pas() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.editeur = "modifié sans demande".to_owned();
    assert!(!v.poll_save(10_000));
    assert_eq!(
        b.vault.read(&b.fichier("notes/idees.md")).unwrap(),
        "des idées"
    );
}

#[test]
fn quitter_le_fichier_sauvegarde_ce_qui_ne_l_est_pas_puis_vide() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.editeur = "pas encore écrit".to_owned();
    v.request_save(0);
    b.vider();
    v.load_file(Some(b.fichier("notes/autre.md")));
    // L'ancien fichier a reçu le texte en attente.
    assert_eq!(
        b.vault.read(&b.fichier("notes/idees.md")).unwrap(),
        "pas encore écrit"
    );
    // Vidé, puis le nouveau texte est chargé comme neuf.
    assert_eq!(
        Banc::lignes(&b.journal),
        ["clear", "set_view_data clear=true"]
    );
    assert_eq!(v.editeur, "autre chose");
    // ★ La demande différée de l'ancien fichier ne vise pas le nouveau.
    assert!(!v.poll_save(10_000));
    assert_eq!(
        b.vault.read(&b.fichier("notes/autre.md")).unwrap(),
        "autre chose"
    );
}

#[test]
fn une_modification_du_disque_remplace_le_texte_sans_le_remettre_a_zero() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vider();
    b.disque.put("notes/idees.md", "écrit ailleurs");
    b.vault.on_change(ChangeType::Modify, "notes/idees.md");
    v.traiter_evenements_coffre();
    assert_eq!(v.editeur, "écrit ailleurs");
    assert_eq!(v.texte.data, "écrit ailleurs");
    assert_eq!(Banc::lignes(&b.journal), ["set_view_data clear=false"]);
}

#[test]
fn l_echo_de_notre_propre_sauvegarde_ne_change_rien() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    v.editeur = "ma frappe".to_owned();
    v.save(false).unwrap();
    b.vider();
    // Le coffre a annoncé `modify` : le disque égale l'écran, rien à refaire.
    v.traiter_evenements_coffre();
    assert!(Banc::lignes(&b.journal).is_empty());
    assert_eq!(v.editeur, "ma frappe");
}

#[test]
fn la_modification_d_un_autre_fichier_est_ignoree() {
    let b = banc();
    let mut v = b.texte();
    v.load_file(Some(b.fichier("notes/idees.md")));
    b.vider();
    b.vault
        .modify(&b.fichier("notes/autre.md"), "changé", opts())
        .unwrap();
    v.traiter_evenements_coffre();
    assert!(Banc::lignes(&b.journal).is_empty());
    assert_eq!(v.editeur, "des idées");
}

// ─── Le Workspace : l'historique n'est écrit que par l'ouverture ───

/// Un fournisseur de documents du schéma vide : tout chemin du coffre s'ouvre dans la vue
/// « fichier ».
struct Coffre(Rc<Vault>);

impl DocumentProvider for Coffre {
    fn scheme(&self) -> &str {
        ""
    }
    fn open(&self, key: &str) -> Option<ViewState> {
        self.0.get_file_by_path(key)?;
        Some(ViewState::with_state("fichier", json!({ "file": key })))
    }
    fn key_of(&self, state: &ViewState) -> Option<DocumentKey> {
        if state.type_ != "fichier" {
            return None;
        }
        state.state.as_ref()?.get("file")?.as_str().map(Into::into)
    }
    fn title(&self, key: &str) -> String {
        key.to_owned()
    }
    fn exists(&self, key: &str) -> bool {
        self.0.get_file_by_path(key).is_some()
    }
}

fn historique_peut_reculer(app: &AppCoeur, leaf: ItemId) -> bool {
    app.workspace()
        .borrow()
        .leaf(leaf)
        .unwrap()
        .history
        .can_go_back()
}

#[test]
fn l_historique_n_est_ecrit_que_par_open_document_pas_par_la_restauration() {
    let b = banc();
    let app = AppCoeur::new();
    let events = app.workspace().borrow().events().clone();
    let (vault, journal) = (b.vault.clone(), b.journal.clone());
    let fabrique: ViewCreator = Rc::new(move |ctx: &ContexteVue| {
        Box::new(VueFichier {
            base: ItemViewBase::new(&ctx.app_scope),
            fichier: FileViewBase::new(ContexteFichier {
                leaf: ctx.leaf,
                vault: vault.clone(),
                workspace_events: events.clone(),
            }),
            journal: journal.clone(),
            refuse: None,
        }) as Box<dyn View>
    });
    let _r1 = app.view_registry().register("fichier", fabrique, "test");
    let _r2 = app
        .documents()
        .register(Rc::new(Coffre(b.vault.clone())), OWNER_CORE)
        .unwrap();

    let leaf = app
        .workspace()
        .borrow_mut()
        .get_leaf(NouvelleFeuille::Onglet)
        .unwrap();

    // 1. Restauration d'un layout : set_view_state avec un fichier. Pas d'historique.
    app.workspace().borrow_mut().set_view_state(
        leaf,
        ViewState::with_state("fichier", json!({"file": "racine.md"})),
        None,
    );
    assert_eq!(
        app.workspace().borrow().document_de(leaf).as_deref(),
        Some("racine.md")
    );
    assert!(!historique_peut_reculer(&app, leaf));

    // 2. Une coquille différée puis sa matérialisation (le clic sur l'onglet restauré) : non plus.
    app.workspace().borrow_mut().set_deferred_state(
        leaf,
        ViewState::with_state("fichier", json!({"file": "notes/idees.md"})),
        None,
    );
    app.workspace().borrow_mut().load_if_deferred(leaf);
    assert!(!historique_peut_reculer(&app, leaf));
    assert_eq!(
        app.workspace().borrow().document_de(leaf).as_deref(),
        Some("notes/idees.md")
    );

    // 3. Reconstruire la vue (un registre a changé) : toujours pas.
    app.workspace().borrow_mut().rebuild_view(leaf);
    assert!(!historique_peut_reculer(&app, leaf));

    // 4. Ouvrir un document (le geste de navigation) : l'onglet se souvient d'où il vient.
    assert!(
        app.workspace()
            .borrow_mut()
            .open_document_in(leaf, "notes/autre.md", None)
    );
    assert!(historique_peut_reculer(&app, leaf));
    assert_eq!(
        app.workspace().borrow().document_de(leaf).as_deref(),
        Some("notes/autre.md")
    );

    // 5. ← revient à idees, SANS écrire une entrée de plus : → est possible, ← ne l'est plus.
    assert!(app.workspace().borrow_mut().go_back(leaf));
    assert_eq!(
        app.workspace().borrow().document_de(leaf).as_deref(),
        Some("notes/idees.md")
    );
    assert!(!historique_peut_reculer(&app, leaf));
}
