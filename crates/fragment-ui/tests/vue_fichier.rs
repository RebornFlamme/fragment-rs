//! Tests de la vue de fichier côté UI (fr-t2-montage). Port de app/e2e/view-header-nav.spec.ts
//! (les flèches naviguent dans l'historique de l'onglet, le titre reste centré), découpé en
//! plusieurs cas, plus le fil d'Ariane et `reveal-in-folder`.
//!
//! NON TRADUIT : « le titre reste centré dans le header (grille 1fr auto 1fr, ±2 px) » — c'est
//! du rendu, il relève des scènes pixels.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::documents::OWNER_CORE;
use fragment_core::workspace::vue_fichier::{FileView, REVEAL_IN_FOLDER, RevealInFolder};
use fragment_core::{
    App, AppCoeur, BreadcrumbSegment, DataAdapter, ItemId, MemoryAdapter, NouvelleFeuille, Vault,
    VaultDocumentProvider, relay_vault_events,
};
use fragment_ui::vue_fichier::{VUE_FICHIER_SIMPLE, VueFichierSimple, fabrique};

struct Banc {
    app: Rc<AppCoeur>,
    vault: Rc<Vault>,
    leaf: ItemId,
    /// Les enregistrements et abonnements, tenus en vie le temps du test.
    _garde: Vec<Box<dyn std::any::Any>>,
}

fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("dossier/a.md", "# A\n");
    disque.put("b.md", "# B\n");
    let adaptateur: Rc<dyn DataAdapter> = disque;
    let vault = Vault::new(adaptateur);
    vault.load().unwrap();
    let app = AppCoeur::new();
    let events = app.workspace().borrow().events().clone();
    let mut garde: Vec<Box<dyn std::any::Any>> = vec![
        Box::new(app.view_registry().register(
            VUE_FICHIER_SIMPLE,
            fabrique(vault.clone(), events),
            "test",
        )),
        Box::new(
            app.extension_registry()
                .register("md", VUE_FICHIER_SIMPLE.to_owned(), "test"),
        ),
    ];
    let reg = app.extension_registry().clone();
    let ext: fragment_core::vault::ExtensionLookup = Rc::new(move |e| reg.get(e));
    let fournisseur = VaultDocumentProvider::new(vault.clone(), ext, "empty");
    garde.push(Box::new(
        app.documents()
            .register(Rc::new(fournisseur), OWNER_CORE)
            .unwrap(),
    ));
    // Le coffre relaie ses renommages vers les documents : l'historique suit la clé.
    garde.push(Box::new(relay_vault_events(app.documents(), &vault)));
    let leaf = app
        .workspace()
        .borrow_mut()
        .get_leaf(NouvelleFeuille::Onglet)
        .unwrap();
    Banc {
        app,
        vault,
        leaf,
        _garde: garde,
    }
}

impl Banc {
    fn ouvrir(&self, chemin: &str) {
        assert!(
            self.app
                .workspace()
                .borrow_mut()
                .open_document_in(self.leaf, chemin, None)
        );
    }
    fn actif(&self) -> Option<String> {
        self.app
            .workspace()
            .borrow()
            .document_de(self.leaf)
            .map(|d| d.to_string())
    }
    fn fleches(&self) -> (bool, bool) {
        let w = self.app.workspace().borrow();
        let h = &w.leaf(self.leaf).unwrap().history;
        (h.can_go_back(), h.can_go_forward())
    }
    fn reculer(&self) {
        assert!(self.app.workspace().borrow_mut().go_back(self.leaf));
    }
    fn avancer(&self) {
        assert!(self.app.workspace().borrow_mut().go_forward(self.leaf));
    }
    fn avec_vue<R>(&self, f: impl FnOnce(&mut VueFichierSimple) -> R) -> R {
        let mut w = self.app.workspace().borrow_mut();
        let vue = w
            .view_mut(self.leaf)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<VueFichierSimple>()
            .unwrap();
        f(vue)
    }
}

#[test]
fn fleches_apres_deux_ouvertures_arriere_actif_avant_inactif() {
    let b = banc();
    b.ouvrir("dossier/a.md");
    b.ouvrir("b.md");
    assert_eq!(b.actif().as_deref(), Some("b.md"));
    assert_eq!(b.fleches(), (true, false));
}

#[test]
fn fleches_retour_donne_a_md_et_avant_devient_actif() {
    let b = banc();
    b.ouvrir("dossier/a.md");
    b.ouvrir("b.md");
    b.reculer();
    assert_eq!(b.actif().as_deref(), Some("dossier/a.md"));
    assert!(b.fleches().1);
}

#[test]
fn fleches_avance_redonne_b_md() {
    let b = banc();
    b.ouvrir("dossier/a.md");
    b.ouvrir("b.md");
    b.reculer();
    b.avancer();
    assert_eq!(b.actif().as_deref(), Some("b.md"));
    assert_eq!(b.fleches(), (true, false));
}

#[test]
fn fleches_un_fichier_renomme_entre_temps_l_historique_suit() {
    let b = banc();
    b.ouvrir("dossier/a.md");
    b.ouvrir("b.md");
    b.reculer();
    b.avancer();
    let a = b.vault.get_file_by_path("dossier/a.md").unwrap();
    b.vault.rename(&a, "dossier/a2.md").unwrap();
    b.reculer();
    assert_eq!(b.actif().as_deref(), Some("dossier/a2.md"));
}

#[test]
fn fil_d_ariane_segments_de_dossier_a_md() {
    let b = banc();
    b.ouvrir("dossier/a.md");
    b.avec_vue(|v| {
        let fil = v.breadcrumb().unwrap().clone();
        assert_eq!(fil.title, "a");
        assert_eq!(fil.path, "dossier/a.md");
        assert_eq!(fil.segments.len(), 1);
        match &fil.segments[0] {
            BreadcrumbSegment::Folder(d) => {
                assert_eq!(d.name, "dossier");
                assert_eq!(d.path, "dossier");
            }
            autre => panic!("segment inattendu : {autre:?}"),
        }
    });
}

#[test]
fn reveal_folder_emet_reveal_in_folder() {
    let b = banc();
    b.ouvrir("dossier/a.md");
    let recu: Rc<RefCell<Vec<String>>> = Rc::default();
    let r = recu.clone();
    let events = b.app.workspace().borrow().events().clone();
    let _abo = events.on(REVEAL_IN_FOLDER, move |d| {
        r.borrow_mut().push(
            d.downcast_ref::<RevealInFolder>()
                .unwrap()
                .folder
                .path
                .clone(),
        );
    });
    b.avec_vue(|v| {
        v.reveal_folder("dossier");
        v.reveal_folder("inexistant");
    });
    assert_eq!(*recu.borrow(), ["dossier"]);
}
