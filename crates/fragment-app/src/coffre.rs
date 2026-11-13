// Port de app/src/ui/bootApp.tsx (l'ouverture du coffre au démarrage), de app/src/ui/core/app.ts
// (le coffre et les documents de l'App) et du plugin cœur de l'explorateur de app/src/ui/seedCore.ts
// (vue `file-explorer`, commande `file-explorer:new-file`, réponse à `reveal-in-folder`).
//
//! Le coffre de l'application : le dossier de travail devient un [`Vault`] (adaptateur disque,
//! surveillance, fournisseur de documents), et le plugin cœur `file-explorer` le montre au dock
//! gauche.
//!
//! ★ `.fragment/` est le dossier de configuration du coffre : il n'entre jamais dans l'arbre (le
//!   coffre ignore les chemins cachés), et les stockages (`stockage.rs`) y écrivent leurs JSON.
//! ★ La surveillance tourne dans son fil ; ses changements n'atteignent le coffre qu'au
//!   [`Coffre::pomper`] du tic de l'application (le coffre et ses abonnés ne sont pas `Send`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use fragment_core::vault::fs::FileSystemAdapter;
use fragment_core::vault::surveillance::OptionsSurveillance;
use fragment_core::workspace::vue_fichier::{REVEAL_IN_FOLDER, RevealInFolder};
use fragment_core::{
    App as AppTrait, AppCoeur, Command, DataAdapter, Disposer, EMPTY_VIEW_TYPE, EventRef, Hotkey,
    MemoryAdapter, Modifier, NouvelleFeuille, OWNER_CORE, OpenViewState, TFile, Vault,
    VaultDocumentProvider, ViewCreator, ViewState, Workspace, relay_vault_events,
};
use fragment_ui::explorateur::{
    self, ActionsExplorateur, ContexteExplorateur, FILE_EXPLORER_VIEW_TYPE, FileExplorerView,
};
use fragment_ui::onglets::ContenuDeVue;
use fragment_ui::vue_markdown;

/// La commande « Nouvelle note » (l'id d'Obsidian, une API : raccourcis, CLI, plugins).
pub const COMMANDE_NOUVELLE_NOTE: &str = "file-explorer:new-file";

/// D'où vient le coffre.
#[derive(Clone)]
pub enum SourceCoffre {
    /// Un dossier du disque, surveillé.
    Dossier(PathBuf),
    /// Un disque en mémoire (tests).
    Memoire(Rc<MemoryAdapter>),
}

/// Le coffre ouvert, et ce qui le relie à l'App.
pub struct Coffre {
    pub vault: Rc<Vault>,
    /// L'adaptateur disque, pour pomper la surveillance (`None` en mémoire).
    disque: Option<Rc<FileSystemAdapter>>,
    /// La racine absolue sur le disque (`""` en mémoire).
    pub racine: String,
    retraits: RefCell<Vec<Disposer>>,
    abonnements: RefCell<Vec<EventRef>>,
}

impl Coffre {
    /// Ouvre et lit le coffre. Seule une racine illisible est une erreur.
    pub fn ouvrir(source: &SourceCoffre) -> anyhow::Result<Rc<Self>> {
        let (adaptateur, disque, racine): (Rc<dyn DataAdapter>, _, _) = match source {
            SourceCoffre::Dossier(chemin) => {
                let fs = Rc::new(FileSystemAdapter::new(chemin.clone()));
                (fs.clone(), Some(fs), chemin.to_string_lossy().into_owned())
            }
            SourceCoffre::Memoire(m) => (m.clone(), None, String::new()),
        };
        let vault = Vault::new(adaptateur);
        vault.load()?;
        Ok(Rc::new(Self {
            vault,
            disque,
            racine,
            retraits: RefCell::new(Vec::new()),
            abonnements: RefCell::new(Vec::new()),
        }))
    }

    /// Branche le coffre sur l'App : il fournit les documents du schéma vide, ses événements
    /// suivent jusqu'aux documents (renommage dans les historiques ← →), et le layout par défaut
    /// ouvre l'explorateur à gauche et la première note au centre.
    pub fn brancher(&self, app: &AppCoeur) {
        let extensions = app.extension_registry().clone();
        let fournisseur = VaultDocumentProvider::new(
            self.vault.clone(),
            Rc::new(move |ext: &str| extensions.get(ext)),
            EMPTY_VIEW_TYPE,
        );
        match app.documents().register(Rc::new(fournisseur), OWNER_CORE) {
            Ok(r) => self.retraits.borrow_mut().push(r),
            Err(e) => eprintln!("fragment : coffre : {e}"),
        }
        self.abonnements
            .borrow_mut()
            .extend(relay_vault_events(app.documents(), &self.vault));
        let mut w = app.workspace().borrow_mut();
        w.default_side_view_type = FILE_EXPLORER_VIEW_TYPE.to_owned();
        w.document_initial = self
            .vault
            .get_markdown_files()
            .first()
            .map(|f| fragment_core::DocumentKey::new(f.path.clone()));
    }

    /// Lance la surveillance du disque (sans effet en mémoire).
    pub fn surveiller(&self) {
        if let Some(d) = &self.disque
            && let Err(e) = d.surveiller(OptionsSurveillance::default())
        {
            eprintln!("fragment : surveillance du coffre : {e}");
        }
    }

    /// Livre au coffre les changements du disque en attente ; rend leur nombre.
    pub fn pomper(&self) -> usize {
        self.disque.as_ref().map_or(0, |d| d.pomper())
    }
}

impl Drop for Coffre {
    fn drop(&mut self) {
        for a in self.abonnements.borrow_mut().drain(..) {
            a.off();
        }
        for r in self.retraits.borrow_mut().drain(..) {
            r();
        }
    }
}

/// Ouvre `chemin` dans une feuille du centre (`getLeaf(nouvelle).openFile`) et l'active.
fn ouvrir_dans(ws: &Weak<RefCell<Workspace>>, nouvelle: NouvelleFeuille, fichier: &TFile) {
    let Some(ws) = ws.upgrade() else { return };
    let Ok(mut w) = ws.try_borrow_mut() else {
        return;
    };
    if let Ok(leaf) = w.get_leaf(nouvelle) {
        w.open_document_in(
            leaf,
            &fichier.path,
            Some(OpenViewState {
                active: Some(true),
                ..OpenViewState::default()
            }),
        );
    }
}

/// Lance un programme du système sans l'attendre ; un échec est journalisé.
fn lancer(programme: &str, arguments: &[&str]) {
    if let Err(e) = std::process::Command::new(programme)
        .args(arguments)
        .spawn()
    {
        eprintln!("fragment : {programme} : {e}");
    }
}

/// Ouvre un dossier DU COFFRE (chemin relatif à sa racine) dans l'explorateur du système : c'est
/// le « Open themes folder » des réglages. ★ Le programme n'est pas attendu (`explorer` rend un
/// code non nul même quand il a réussi) ; seul l'échec du lancement est une erreur.
pub fn ouvrir_dossier_du_coffre(racine: &str, relatif: &str) -> std::io::Result<()> {
    let chemin = std::path::Path::new(racine).join(relatif);
    let chemin = chemin.to_string_lossy();
    let programme = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(programme)
        .arg(&*chemin)
        .spawn()
        .map(|_| ())
}

/// Ce que l'explorateur demande à l'application (onglets, système).
fn actions(app: &AppCoeur, coffre: &Coffre) -> ActionsExplorateur {
    let mut a = ActionsExplorateur::new(app.keymap().clone(), &coffre.racine);
    let ws = Rc::downgrade(app.workspace());
    a.ouvrir_onglet = Rc::new(move |f| ouvrir_dans(&ws, NouvelleFeuille::Onglet, f));
    let ws = Rc::downgrade(app.workspace());
    a.ouvrir_a_droite = Rc::new(move |f| ouvrir_dans(&ws, NouvelleFeuille::Split, f));
    a.montrer_dans_systeme = Rc::new(|chemin| {
        if cfg!(windows) {
            lancer("explorer", &[&format!("/select,{chemin}")]);
        } else if cfg!(target_os = "macos") {
            lancer("open", &["-R", chemin]);
        } else {
            let dossier = std::path::Path::new(chemin)
                .parent()
                .map_or_else(|| chemin.to_owned(), |p| p.to_string_lossy().into_owned());
            lancer("xdg-open", &[&dossier]);
        }
    });
    a.ouvrir_dans_app = Rc::new(|chemin| {
        if cfg!(windows) {
            lancer("cmd", &["/C", "start", "", chemin]);
        } else if cfg!(target_os = "macos") {
            lancer("open", &[chemin]);
        } else {
            lancer("xdg-open", &[chemin]);
        }
    });
    a.signaler = Rc::new(|m| eprintln!("fragment : explorateur : {m}"));
    a
}

/// La fabrique du type `file-explorer` : la vue de `fragment-ui`, avec ses actions.
fn fabrique_explorateur(app: &AppCoeur, coffre: &Coffre) -> ViewCreator {
    let ws = Rc::downgrade(app.workspace());
    let ws2 = ws.clone();
    let contexte = ContexteExplorateur {
        vault: coffre.vault.clone(),
        workspace_events: app.workspace().borrow().events().clone(),
        ouvrir: Rc::new(move |f| ouvrir_dans(&ws, NouvelleFeuille::Non, f)),
        sauvegarder: Rc::new(move || {
            if let Some(ws) = ws2.upgrade()
                && let Ok(mut w) = ws.try_borrow_mut()
            {
                w.request_save_layout();
            }
        }),
    };
    let base = explorateur::fabrique(contexte);
    let actions = actions(app, coffre);
    Rc::new(move |c| {
        let vue = base(c);
        if let Some(e) = vue.as_any().downcast_ref::<FileExplorerView>() {
            e.definir_actions(actions.clone());
        }
        vue
    })
}

/// « Sans titre.md », « Sans titre 1.md »… à la racine (`newFileLocation` vaut `root` par défaut),
/// créée puis ouverte dans la feuille courante.
fn nouvelle_note(vault: &Vault, ws: &Weak<RefCell<Workspace>>) {
    let chemin = explorateur::menu::chemin_libre(vault, "");
    match vault.create(&chemin, "", fragment_core::DataWriteOptions::default()) {
        Ok(f) => ouvrir_dans(ws, NouvelleFeuille::Non, &f),
        Err(e) => eprintln!("fragment : nouvelle note : {e}"),
    }
}

/// Montre l'explorateur (créé au besoin dans le dock gauche, dock déplié) et y révèle `chemin`.
fn reveler_dans_l_explorateur(ws: &Weak<RefCell<Workspace>>, chemin: &str) {
    let Some(ws) = ws.upgrade() else { return };
    let Ok(mut w) = ws.try_borrow_mut() else {
        return;
    };
    let leaf = match w.get_leaves_of_type(FILE_EXPLORER_VIEW_TYPE).first() {
        Some(&l) => l,
        None => {
            let Ok(l) = w.get_left_leaf() else { return };
            w.set_view_state(l, ViewState::new(FILE_EXPLORER_VIEW_TYPE), None);
            l
        }
    };
    w.reveal_leaf(leaf);
    w.load_if_deferred(leaf);
    if let Some(v) = w.view_mut(leaf)
        && let Some(e) = v.as_any().downcast_ref::<FileExplorerView>()
    {
        e.reveal_in_folder(chemin);
    }
}

/// Le plugin cœur `file-explorer` : sa vue, sa commande, sa réponse au fil d'Ariane. Rend les
/// retraits (le déchargement du plugin).
pub fn activer_explorateur(app: &AppCoeur, coffre: &Coffre) -> Vec<Disposer> {
    let mut retraits = vec![app.view_registry().register(
        FILE_EXPLORER_VIEW_TYPE,
        fabrique_explorateur(app, coffre),
        "file-explorer",
    )];
    let (vault, ws) = (coffre.vault.clone(), Rc::downgrade(app.workspace()));
    let commande = Command::new(COMMANDE_NOUVELLE_NOTE, "Nouvelle note")
        .with_hotkeys(vec![Hotkey::new(&[Modifier::Mod], "n")])
        .with_callback(move || nouvelle_note(&vault, &ws));
    retraits.push(app.commands().add_command(commande, "file-explorer"));

    // Le fil d'Ariane des vues de fichier ÉMET `reveal-in-folder` ; le plugin y répond.
    let ws = Rc::downgrade(app.workspace());
    let abonnement = app
        .workspace()
        .borrow()
        .events()
        .on(REVEAL_IN_FOLDER, move |d| {
            if let Some(r) = d.downcast_ref::<RevealInFolder>() {
                reveler_dans_l_explorateur(&ws, &r.folder.path);
            }
        });
    retraits.push(Box::new(move || abonnement.off()));
    // La vue `markdown` (fr-t3-vue) : vue, extension `md`, commandes `editor:*`.
    retraits.extend(vue_markdown::enregistrer(
        app,
        vue_markdown::ContexteMarkdown {
            vault: coffre.vault.clone(),
            workspace_events: app.workspace().borrow().events().clone(),
            horloge: vue_markdown::horloge_systeme(),
        },
    ));
    retraits
}

/// Le contenu des vues que l'application sait dessiner (`ContexteOnglets::contenu_de_vue`).
pub fn contenu_de_vue() -> ContenuDeVue {
    Rc::new(|v, cx| explorateur::contenu(v, cx).or_else(|| vue_markdown::contenu(v, cx)))
}
