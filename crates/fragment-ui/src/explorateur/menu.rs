// Port de app/src/ui/views/explorer/FileTree.tsx (ouvrirMenuFichier, creerNote, creerDossier,
// copierDe, cheminSysteme, montrerDansSysteme, ouvrirDansApp)
//! Les menus de l'explorateur (fichier, dossier, racine) et les gestes qu'ils déclenchent : créer
//! une note ou un dossier au nom libre, faire une copie, copier le chemin, supprimer.
//!
//! Les gestes sont des fonctions libres sur le [`Vault`] (testées sans fenêtre) ; le menu n'est
//! qu'un assemblage d'entrées qui les appellent. Ce que la vue seule sait faire (passer une ligne en
//! renommage, ouvrir la modale de déplacement, déplier un dossier, ouvrir un fichier) lui est
//! demandé par [`GesteVue`].
//!
//! ★ La RACINE n'a ni copie, ni déplacement, ni renommage, ni suppression : on n'opère pas sur le
//!   coffre lui-même. Seules les deux créations lui restent.

use std::path::PathBuf;
use std::rc::Rc;

use fragment_core::{DataWriteOptions, Keymap, TAbstractFile, TFile, TFolder, Vault, VaultError};

use fragment_core::vault::coffre::VaultResult;

use super::glisser::chemin_libre_pour;
use crate::menu::Menu;
use crate::notice::Notice;

/// Le message de la notice d'un chemin copié.
pub const MESSAGE_CHEMIN_COPIE: &str = "Chemin copié dans le presse-papiers";
/// Durée par défaut d'une notice (`Notice`, 5000 ms).
const DUREE_NOTICE_MS: u64 = 5000;

/// Les sections du menu, dans l'ordre de l'ouvreur (le TS les fige avec `addSections`).
pub const SECTIONS: [&str; 7] = [
    "create", "open", "action", "info", "system", "plugin", "edit",
];

/// Le hook `file-menu`.
pub type HookMenu = Rc<dyn Fn(&mut Menu, &TAbstractFile)>;

/// Ce que l'explorateur demande au reste de l'application : tout ce qui touche au Workspace, au
/// presse-papiers et au système. Chaque rappel a un défaut inoffensif ([`ActionsExplorateur::new`]).
#[derive(Clone)]
pub struct ActionsExplorateur {
    /// La pile de portées où le menu empile son Échap.
    pub keymap: Keymap,
    /// La racine du coffre sur le disque (« Depuis la racine du système »).
    pub racine_systeme: String,
    /// « Ouvrir dans un nouvel onglet » (`getLeaf('tab')`).
    pub ouvrir_onglet: Rc<dyn Fn(&TFile)>,
    /// « Ouvrir sur la droite » (`getLeaf('split')`).
    pub ouvrir_a_droite: Rc<dyn Fn(&TFile)>,
    /// Écrire un texte dans le presse-papiers.
    pub copier: Rc<dyn Fn(&str)>,
    /// « Afficher dans l'explorateur système » : le chemin absolu.
    pub montrer_dans_systeme: Rc<dyn Fn(&str)>,
    /// « Ouvrir dans l'app par défaut » : le chemin absolu.
    pub ouvrir_dans_app: Rc<dyn Fn(&str)>,
    /// Le réglage `trashOption` vaut-il `system` ?
    pub corbeille_systeme: Rc<dyn Fn() -> bool>,
    /// Le hook `file-menu` : les plugins mutent le menu (ils posent leur propre section).
    pub hook_menu: HookMenu,
    /// Une opération refusée par le coffre (le `console.warn` du TS).
    pub signaler: Rc<dyn Fn(&str)>,
}

impl ActionsExplorateur {
    /// Des actions sans effet : à compléter champ par champ par l'application.
    pub fn new(keymap: Keymap, racine_systeme: &str) -> Self {
        Self {
            keymap,
            racine_systeme: racine_systeme.to_owned(),
            ouvrir_onglet: Rc::new(|_| {}),
            ouvrir_a_droite: Rc::new(|_| {}),
            copier: Rc::new(|_| {}),
            montrer_dans_systeme: Rc::new(|_| {}),
            ouvrir_dans_app: Rc::new(|_| {}),
            corbeille_systeme: Rc::new(|| false),
            hook_menu: Rc::new(|_, _| {}),
            signaler: Rc::new(|_| {}),
        }
    }
}

/// Ce que seule la vue sait faire pour un menu.
pub trait GesteVue {
    /// Passe la ligne `chemin` en renommage.
    fn renommer(&self, chemin: &str);
    /// Ouvre la modale « Déplacer vers… » pour `chemin`.
    fn deplacer(&self, chemin: &str);
    /// Ouvre un dossier de l'arbre (idempotent) : rendre un enfant neuf visible.
    fn deplier(&self, chemin: &str);
    /// Ouvre une note dans la feuille courante (`getLeaf(false).openFile`).
    fn ouvrir(&self, fichier: &TFile);
}

/// « Sans titre.md », « Sans titre 1.md »… : le premier chemin libre du dossier.
pub fn chemin_libre(vault: &Vault, dossier: &str) -> String {
    vault.get_available_path(dossier, "Sans titre", "md")
}

/// « Nouveau dossier », « Nouveau dossier 1 »… : le premier chemin libre du dossier.
pub fn chemin_dossier_libre(vault: &Vault, parent: &str) -> String {
    vault.get_available_path(parent, "Nouveau dossier", "")
}

/// Crée une note vide au nom libre dans `dossier` (`creerNote`) ; l'appelant l'ouvre.
pub fn creer_note(vault: &Vault, dossier: &str) -> VaultResult<TFile> {
    vault.create(
        &chemin_libre(vault, dossier),
        "",
        DataWriteOptions::default(),
    )
}

/// Crée un dossier au nom libre dans `parent` (`creerDossier`) ; l'appelant le déplie et passe en
/// renommage.
pub fn creer_dossier(vault: &Vault, parent: &str) -> VaultResult<TFolder> {
    vault.create_folder(&chemin_dossier_libre(vault, parent))
}

/// « Faire une copie » : duplique un fichier OU un dossier à côté, au nom libre. L'appelant passe
/// en renommage sur la copie.
pub fn copier_de(vault: &Vault, chemin: &str) -> VaultResult<TAbstractFile> {
    let af = vault
        .get_abstract_file_by_path(chemin)
        .ok_or(VaultError::NoeudInconnu)?;
    let parent = chemin.rfind('/').map_or("", |i| &chemin[..i]);
    let cible = chemin_libre_pour(vault, parent, af.name());
    match &af {
        TAbstractFile::File(f) => vault.copy(f, &cible),
        TAbstractFile::Folder(d) => vault.copy(d, &cible),
    }
}

/// « Supprimer » : vers la corbeille système ou `.trash/` du coffre. Refusé pour la racine.
pub fn supprimer(vault: &Vault, chemin: &str, systeme: bool) -> VaultResult<()> {
    match vault
        .get_abstract_file_by_path(chemin)
        .ok_or(VaultError::NoeudInconnu)?
    {
        TAbstractFile::File(f) => vault.trash(&f, systeme),
        TAbstractFile::Folder(d) => vault.trash(&d, systeme),
    }
}

/// Le chemin ABSOLU sur le disque : la racine du coffre puis chaque segment (`cheminSysteme`).
pub fn chemin_systeme(racine: &str, chemin: &str) -> String {
    let mut p = PathBuf::from(racine);
    for segment in chemin.split('/').filter(|s| !s.is_empty()) {
        p.push(segment);
    }
    p.to_string_lossy().into_owned()
}

/// Construit le menu d'un élément (`ouvrirMenuFichier`) : trois formes, selon que la cible est la
/// racine (le « vide » de l'arbre), un dossier ou un fichier.
///
/// ★ L'ordre est constant : le cœur construit, les plugins mutent (`hook_menu`), la vue affiche.
pub fn construire_menu(
    vault: &Rc<Vault>,
    actions: &Rc<ActionsExplorateur>,
    vue: &Rc<dyn GesteVue>,
    af: &TAbstractFile,
) -> Menu {
    let mut menu = Menu::new();
    menu.add_sections(&SECTIONS);
    let chemin = af.path().to_owned();
    let racine = chemin.is_empty() && af.is_folder();

    match af {
        TAbstractFile::Folder(_) => {
            let (v, a, g, c) = (vault.clone(), actions.clone(), vue.clone(), chemin.clone());
            menu.add_item(|i| {
                i.set_title("Nouvelle note")
                    .set_icon("square-pen")
                    .set_section("create")
                    .on_click(move || match creer_note(&v, &c) {
                        Ok(f) => g.ouvrir(&f),
                        Err(e) => (a.signaler)(&format!("création refusée : {e}")),
                    });
            });
            let (v, a, g, c) = (vault.clone(), actions.clone(), vue.clone(), chemin.clone());
            menu.add_item(|i| {
                i.set_title("Nouveau dossier")
                    .set_icon("folder-plus")
                    .set_section("create")
                    .on_click(move || match creer_dossier(&v, &c) {
                        Ok(d) => {
                            if !c.is_empty() {
                                g.deplier(&c);
                            }
                            g.renommer(&d.path);
                        }
                        Err(e) => (a.signaler)(&format!("création refusée : {e}")),
                    });
            });
        }
        TAbstractFile::File(f) => {
            let (a, fichier) = (actions.clone(), f.clone());
            menu.add_item(|i| {
                i.set_title("Ouvrir dans un nouvel onglet")
                    .set_icon("file-plus-2")
                    .set_section("open")
                    .on_click(move || (a.ouvrir_onglet)(&fichier));
            });
            let (a, fichier) = (actions.clone(), f.clone());
            menu.add_item(|i| {
                i.set_title("Ouvrir sur la droite")
                    .set_icon("separator-vertical")
                    .set_section("open")
                    .on_click(move || (a.ouvrir_a_droite)(&fichier));
            });
        }
    }

    if !racine {
        let (v, a, g, c) = (vault.clone(), actions.clone(), vue.clone(), chemin.clone());
        menu.add_item(|i| {
            i.set_title("Faire une copie")
                .set_icon("copy")
                .set_section("action")
                .on_click(move || match copier_de(&v, &c) {
                    Ok(copie) => g.renommer(copie.path()),
                    Err(e) => (a.signaler)(&format!("copie refusée : {e}")),
                });
        });
        let (g, c) = (vue.clone(), chemin.clone());
        let titre = if af.is_folder() {
            "Déplacer le dossier vers…"
        } else {
            "Déplacer le fichier vers…"
        };
        menu.add_item(|i| {
            i.set_title(titre)
                .set_icon("folder-tree")
                .set_section("action")
                .on_click(move || g.deplacer(&c));
        });

        // « Copier le chemin » : un sous-menu, chemin de coffre puis chemin système.
        let (a, c) = (actions.clone(), chemin.clone());
        menu.add_item(|i| {
            i.set_title("Copier le chemin")
                .set_icon("clipboard-copy")
                .set_section("info");
            let sous = i.set_submenu();
            let (a1, c1) = (a.clone(), c.clone());
            sous.add_item(|s| {
                s.set_title("Depuis la racine du vault")
                    .set_icon("folder")
                    .on_click(move || copier_chemin(&a1, &c1));
            });
            sous.add_item(|s| {
                s.set_title("Depuis la racine du système")
                    .set_icon("hard-drive")
                    .on_click(move || {
                        copier_chemin(&a, &chemin_systeme(&a.racine_systeme, &c));
                    });
            });
        });

        // Système : « ouvrir dans l'app » n'a de sens que pour un fichier.
        if !af.is_folder() {
            let (a, c) = (actions.clone(), chemin.clone());
            menu.add_item(|i| {
                i.set_title("Ouvrir dans l'app par défaut")
                    .set_icon("arrow-up-right")
                    .set_section("system")
                    .on_click(move || (a.ouvrir_dans_app)(&chemin_systeme(&a.racine_systeme, &c)));
            });
        }
        let (a, c) = (actions.clone(), chemin.clone());
        menu.add_item(|i| {
            i.set_title("Afficher dans l'explorateur système")
                .set_icon("arrow-up-right")
                .set_section("system")
                .on_click(move || {
                    (a.montrer_dans_systeme)(&chemin_systeme(&a.racine_systeme, &c));
                });
        });
    }

    // Le maillon PLUGINS : le même menu part vers le hook, qui le mute.
    (actions.hook_menu)(&mut menu, af);

    if !racine {
        // Renommer ET Supprimer dans la MÊME section : aucun séparateur entre eux ; seul
        // Supprimer est en rouge.
        let (g, c) = (vue.clone(), chemin.clone());
        menu.add_item(|i| {
            i.set_title("Renommer…")
                .set_icon("pencil-line")
                .set_section("edit")
                .on_click(move || g.renommer(&c));
        });
        let (v, a) = (vault.clone(), actions.clone());
        menu.add_item(|i| {
            i.set_title("Supprimer")
                .set_icon("trash-2")
                .set_warning(true)
                .set_section("edit")
                .on_click(move || {
                    if let Err(e) = supprimer(&v, &chemin, (a.corbeille_systeme)()) {
                        (a.signaler)(&format!("suppression refusée : {e}"));
                    }
                });
        });
    }
    menu
}

/// Copie `texte` dans le presse-papiers et prévient (`copierChemin`).
fn copier_chemin(actions: &ActionsExplorateur, texte: &str) {
    (actions.copier)(texte);
    Notice::new(MESSAGE_CHEMIN_COPIE, DUREE_NOTICE_MS);
}

/// Le menu du « vide » de l'arbre : celui du dossier racine.
pub fn menu_racine(
    vault: &Rc<Vault>,
    actions: &Rc<ActionsExplorateur>,
    vue: &Rc<dyn GesteVue>,
) -> Menu {
    let racine: TAbstractFile = vault.get_root().into();
    construire_menu(vault, actions, vue, &racine)
}
