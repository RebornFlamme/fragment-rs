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

use fragment_cli::coffres::RegistreCoffres;
use fragment_core::vault::fs::FileSystemAdapter;
use fragment_core::vault::surveillance::OptionsSurveillance;
use fragment_core::workspace::vue_fichier::{REVEAL_IN_FOLDER, RevealInFolder};
use fragment_core::{
    App as AppTrait, AppCoeur, Command, DataAdapter, Disposer, EMPTY_VIEW_TYPE, EventRef, Hotkey,
    MemoryAdapter, Modifier, NouvelleFeuille, OWNER_CORE, OpenViewState, TFile, Vault,
    VaultDocumentProvider, ViewCreator, ViewState, Workspace, relay_vault_events,
};
use fragment_ui::coffres::selecteur::{self, ActionsSelecteur};
use fragment_ui::coffres::{CommandeGestionnaire, ControleurCoffres, Effet, VueGestionnaire};
use fragment_ui::explorateur::{
    self, ActionsExplorateur, ContexteExplorateur, FILE_EXPLORER_VIEW_TYPE, FileExplorerView,
};
use fragment_ui::onglets::ContenuDeVue;
use fragment_ui::vue_markdown;
use fragment_ui::vue_pdf;

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
    // La vue `pdf` (fr-t4-vue-pdf) : vue, extension `pdf`.
    retraits.extend(vue_pdf::enregistrer(
        app,
        vue_pdf::ContextePdf {
            vault: coffre.vault.clone(),
            workspace_events: app.workspace().borrow().events().clone(),
            keymap: app.keymap().clone(),
        },
    ));
    retraits
}

/// Le contenu des vues que l'application sait dessiner (`ContexteOnglets::contenu_de_vue`).
pub fn contenu_de_vue() -> ContenuDeVue {
    Rc::new(|v, cx| {
        explorateur::contenu(v, cx)
            .or_else(|| vue_markdown::contenu(v, cx))
            .or_else(|| vue_pdf::contenu(v, cx))
    })
}

// ── Le choix du coffre (fr-t2-gestionnaire) ─────────────────────────────
// Port de app/src/electron/vaults.ts (la fenêtre de gestion, la boîte de choix de dossier, la
// relance sur un autre coffre) et du démarrage de app/src/electron/main.ts.

/// Le dossier des données de l'application (`config.json` : le coffre courant et la liste).
/// `FRAGMENT_USER_DATA` le remplace (tests, scènes). ★ Distinct de celui de Fragment (Electron) :
/// les deux applications écriraient le même `config.json` à des moments différents.
pub fn dossier_user_data() -> PathBuf {
    if let Some(d) = std::env::var_os("FRAGMENT_USER_DATA") {
        return PathBuf::from(d);
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.unwrap_or_else(std::env::temp_dir).join("fragment-rs")
}

/// La boîte native de choix de dossier ; `None` si elle est annulée.
#[cfg(any(windows, target_os = "macos"))]
pub fn choisir_dossier(titre: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_title(titre)
        .pick_folder()
        .map(|p| p.to_string_lossy().into_owned())
}

/// La boîte native de choix de dossier ; `None` si elle est annulée. ★ Sous Linux, `zenity` plutôt
/// qu'une dépendance GTK : la CI Linux n'a pas les en-têtes de GTK.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn choisir_dossier(titre: &str) -> Option<String> {
    let sortie = std::process::Command::new("zenity")
        .args(["--file-selection", "--directory", "--title", titre])
        .output()
        .ok()?;
    let chemin = String::from_utf8_lossy(&sortie.stdout).trim().to_owned();
    (sortie.status.success() && !chemin.is_empty()).then_some(chemin)
}

/// Ce qu'un item du menu du sélecteur demande (il n'a pas de `cx` : la demande attend le tic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Demande {
    Basculer(String),
    Gerer,
}

/// File des demandes du menu du sélecteur.
pub type Demandes = Rc<RefCell<Vec<Demande>>>;

/// Installe l'ouvreur du menu du sélecteur : la liste relue à chaque ouverture, les items poussent
/// leur demande dans `demandes`.
pub fn brancher_selecteur(
    controleur: Rc<RefCell<ControleurCoffres>>,
    keymap: fragment_core::Keymap,
    demandes: Demandes,
) -> Disposer {
    selecteur::brancher(Rc::new(move |x, y| {
        let coffres = controleur.try_borrow().ok().map(|c| c.lister());
        let (d1, d2) = (demandes.clone(), demandes.clone());
        let actions = ActionsSelecteur {
            basculer: Rc::new(move |c| d1.borrow_mut().push(Demande::Basculer(c))),
            gerer: Rc::new(move || d2.borrow_mut().push(Demande::Gerer)),
        };
        selecteur::menu(coffres.as_deref(), &actions).show_at_position(x, y, &keymap);
    }))
}

/// Démarre l'application sur un coffre ; rend son Keymap (le menu du sélecteur y empile Échap).
pub type Lanceur = Rc<dyn Fn(&mut gpui_kit::App, &str) -> Option<fragment_core::Keymap>>;

/// Le choix du coffre côté application : le contrôleur de `fragment-ui`, la fenêtre du
/// gestionnaire, et l'application démarrée par `lanceur`.
pub struct ChoixCoffre {
    controleur: Rc<RefCell<ControleurCoffres>>,
    gestionnaire: RefCell<Option<gpui_kit::AnyWindowHandle>>,
    demandes: Demandes,
    lanceur: Lanceur,
    retraits: RefCell<Vec<Disposer>>,
}

impl ChoixCoffre {
    pub fn new(registre: RegistreCoffres, lanceur: Lanceur) -> Rc<Self> {
        Rc::new(Self {
            controleur: Rc::new(RefCell::new(ControleurCoffres::new(registre))),
            gestionnaire: RefCell::new(None),
            demandes: Rc::new(RefCell::new(Vec::new())),
            lanceur,
            retraits: RefCell::new(Vec::new()),
        })
    }

    /// Le démarrage : `impose` (argument de la ligne de commande) ou le coffre mémorisé, sinon le
    /// gestionnaire seul. Lance aussi le tic qui sert les demandes du menu du sélecteur.
    pub fn demarrer(self: &Rc<Self>, cx: &mut gpui_kit::App, impose: Option<&str>) {
        let effets = {
            let mut c = self.controleur.borrow_mut();
            match impose {
                Some(d) => c.demarrer_sur(d),
                None => c.demarrer(),
            }
        };
        self.appliquer_resultat(cx, effets);
        let faible = Rc::downgrade(self);
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let Some(choix) = faible.upgrade() else {
                    break;
                };
                let demandes: Vec<Demande> = choix.demandes.borrow_mut().drain(..).collect();
                if !demandes.is_empty() {
                    cx.update(|cx| choix.servir(cx, demandes));
                }
            }
        })
        .detach();
    }

    fn servir(self: &Rc<Self>, cx: &mut gpui_kit::App, demandes: Vec<Demande>) {
        for d in demandes {
            let effets = match d {
                Demande::Basculer(c) => self.controleur.borrow_mut().choisir(&c),
                Demande::Gerer => Ok(self.controleur.borrow_mut().gerer()),
            };
            self.appliquer_resultat(cx, effets);
        }
    }

    /// Une commande de la fenêtre du gestionnaire. Différée : elle peut fermer la fenêtre dont
    /// l'événement est en cours de traitement.
    fn commande(self: &Rc<Self>, commande: CommandeGestionnaire, cx: &mut gpui_kit::App) {
        let choix = self.clone();
        cx.defer(move |cx| {
            let effets = {
                let mut c = choix.controleur.borrow_mut();
                match commande {
                    CommandeGestionnaire::Fermer => Ok(c.fermer_gestionnaire()),
                    CommandeGestionnaire::Ouvrir => {
                        let d = choisir_dossier("Ouvrir un dossier comme coffre");
                        c.dossier_ouvert(d.as_deref())
                    }
                    CommandeGestionnaire::Creer => {
                        let d = choisir_dossier("Créer un nouveau coffre");
                        c.dossier_cree(d.as_deref())
                    }
                    CommandeGestionnaire::Choisir(p) => c.choisir(&p),
                    CommandeGestionnaire::Oublier(p) => c.oublier(&p).map(|()| Vec::new()),
                }
            };
            choix.appliquer_resultat(cx, effets);
        });
    }

    fn appliquer_resultat(
        self: &Rc<Self>,
        cx: &mut gpui_kit::App,
        effets: Result<Vec<Effet>, fragment_cli::coffres::ErreurCoffres>,
    ) {
        match effets {
            Ok(e) => self.appliquer(cx, e),
            Err(e) => eprintln!("fragment : coffres : {e}"),
        }
    }

    /// Applique les effets du contrôleur aux fenêtres.
    pub fn appliquer(self: &Rc<Self>, cx: &mut gpui_kit::App, effets: Vec<Effet>) {
        for effet in effets {
            match effet {
                Effet::OuvrirGestionnaire => self.ouvrir_gestionnaire(cx),
                Effet::FermerGestionnaire => {
                    let fenetre = self.gestionnaire.borrow_mut().take();
                    if let Some(h) = fenetre {
                        let _ = h.update(cx, |_, window, _| window.remove_window());
                    }
                }
                Effet::OuvrirApp(c) => {
                    if let Some(keymap) = (self.lanceur)(cx, &c) {
                        let r = brancher_selecteur(
                            self.controleur.clone(),
                            keymap,
                            self.demandes.clone(),
                        );
                        self.retraits.borrow_mut().push(r);
                    }
                }
                Effet::Relancer(c) => relancer(cx, &c),
                Effet::Quitter => cx.quit(),
            }
        }
    }

    fn ouvrir_gestionnaire(self: &Rc<Self>, cx: &mut gpui_kit::App) {
        let existante = *self.gestionnaire.borrow();
        if let Some(h) = existante
            && h.update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            return;
        }
        let (c, choix) = (self.controleur.clone(), Rc::downgrade(self));
        let lister: Rc<dyn Fn() -> Vec<fragment_cli::coffres::VaultInfo>> =
            Rc::new(move || c.try_borrow().map(|c| c.lister()).unwrap_or_default());
        let relais: fragment_ui::coffres::gestionnaire::Relais = Rc::new(move |commande, cx| {
            if let Some(choix) = choix.upgrade() {
                choix.commande(commande, cx);
            }
        });
        match ouvrir_fenetre_gestionnaire(cx, lister, relais) {
            Ok(h) => *self.gestionnaire.borrow_mut() = Some(h),
            Err(e) => eprintln!("fragment : gestionnaire de coffres : {e:#}"),
        }
    }
}

impl Drop for ChoixCoffre {
    fn drop(&mut self) {
        for r in self.retraits.borrow_mut().drain(..) {
            r();
        }
    }
}

/// Ouvre la fenêtre du gestionnaire (840 × 560, « Fragment »).
pub fn ouvrir_fenetre_gestionnaire(
    cx: &mut gpui_kit::App,
    lister: Rc<dyn Fn() -> Vec<fragment_cli::coffres::VaultInfo>>,
    relais: fragment_ui::coffres::gestionnaire::Relais,
) -> anyhow::Result<gpui_kit::AnyWindowHandle> {
    use fragment_ui::coffres::gestionnaire::{HAUTEUR, LARGEUR};
    use gpui_kit::AppContext as _;
    let opts = crate::options_fenetre(cx, crate::TITRE, LARGEUR, HAUTEUR);
    let (h, _) = gpui_kit::open_window(opts, cx, move |_, cx| {
        cx.new(|_| VueGestionnaire::new(lister, relais))
    })?;
    Ok(h)
}

/// L'application tourne déjà : elle se relance sur `chemin` (aucun état de l'ancien coffre ne
/// survit), puis s'arrête.
fn relancer(cx: &mut gpui_kit::App, chemin: &str) {
    match std::env::current_exe() {
        Ok(exe) => match std::process::Command::new(exe).arg(chemin).spawn() {
            Ok(_) => cx.quit(),
            Err(e) => eprintln!("fragment : relance sur {chemin} : {e}"),
        },
        Err(e) => eprintln!("fragment : relance : {e}"),
    }
}
