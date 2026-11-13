// Port de app/src/ui/views/explorer/FileExplorerView.tsx, FileTree.tsx (le rendu, l'ouverture, la
// révélation), explorer.css (le conteneur défilant) et de useVaultVersion.ts (le pont du coffre)
//! La vue `file-explorer` : l'arbre du coffre dans un panneau.
//!
//! ★ Deux moitiés, comme les autres vues de ce dépôt (docs/contrats.md, cas douteux 7) :
//!   - [`FileExplorerView`] est la vue CÔTÉ MODÈLE (`View` + `ItemView` du cœur) : elle garde les
//!     dossiers ouverts, l'élément actif et la position de défilement, s'abonne au coffre et au
//!     Workspace, et se laisse rouvrir par l'état éphémère ;
//!   - [`rendre_explorateur`] la dessine, à chaque image, depuis le modèle ET le coffre : rien
//!     n'est copié (le TS relit de même l'arbre mutable au rendu, `useVaultVersion` n'est qu'un
//!     compteur qui déclenche l'image). Le montage au dock appartient à `fr-t2-montage`.
//!
//! ★ Mise à jour incrémentale : un événement du coffre ne reconstruit rien, il ne fait que mettre
//!   à jour ce que la vue retient par CHEMIN (dossiers ouverts, élément actif) ; la liste des
//!   lignes, elle, est relue dans le coffre à chaque image.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use fragment_core::workspace::vue_fichier::{REVEAL_IN_FOLDER, RevealInFolder};
use fragment_core::{
    ContexteVue, DocumentKey, EventRef, Events, FILE_OPEN, ItemView, ItemViewBase, Scope,
    TAbstractFile, TFile, Vault, VaultEvent, VaultEventType, View, ViewCreator,
};
use gpui_kit::{
    AnyElement, App, DragMoveEvent, ExternalPaths, InteractiveElement as _, IntoElement as _,
    MouseButton, ParentElement as _, Pixels, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, point, px,
};
use serde_json::{Map, Value};

use super::arbre::{LigneVisible, lignes_visibles};
use super::deplacer;
use super::etat::{EtatArbre, defilement_de, ouverts_de, vers_ephemere};
use super::glisser::{self as depot, Ressort};
use super::menu::{
    ActionsExplorateur, GesteVue, construire_menu, creer_dossier, creer_note, menu_racine,
};
use super::renommer::{self, Issue, Renommage, Suite};
use crate::echelle::{Echelle, u};
use crate::glisser::{CrochetsFichier, noter_appui_fichier, poser_crochets, retirer_crochets};
use crate::theme_global::theme_ou_defaut;
use crate::tree_item::{Ligne, hauteurs_calees, rendre_item};

/// Le type de la vue (`FILE_EXPLORER_VIEW_TYPE`).
pub const FILE_EXPLORER_VIEW_TYPE: &str = "file-explorer";

/// Ce que l'explorateur reçoit de l'application : le coffre, le flux d'événements du Workspace, et
/// deux gestes que la vue ne sait pas faire elle-même (elle ne connaît pas le Workspace).
#[derive(Clone)]
pub struct ContexteExplorateur {
    /// Le coffre dont on montre l'arbre.
    pub vault: Rc<Vault>,
    /// Le flux d'événements du Workspace (`file-open`, `reveal-in-folder`).
    pub workspace_events: Events,
    /// Un clic de fichier : l'ouvrir dans la feuille courante (`getLeaf(false).openFile`).
    pub ouvrir: Rc<dyn Fn(&TFile)>,
    /// Un dossier s'est ouvert ou fermé : demander la sauvegarde du layout (`requestSaveLayout`).
    pub sauvegarder: Rc<dyn Fn()>,
}

/// L'état partagé entre la vue et les rappels (clics, événements) qu'elle a posés.
struct Interne {
    vault: Rc<Vault>,
    etat: RefCell<EtatArbre>,
    defilement: ScrollHandle,
    /// Un défilement demandé (état restauré), rejoué quand la mise en page est connue.
    defilement_attente: Cell<Option<f32>>,
    /// Un chemin à faire apparaître (révélation), rejoué de même.
    reveler_attente: RefCell<Option<String>>,
    ouvrir: Rc<dyn Fn(&TFile)>,
    sauvegarder: Rc<dyn Fn()>,
    /// Le nombre d'événements du coffre qui ont changé la FORME de l'arbre (`useVaultVersion`).
    version: Cell<u64>,
    /// Nommer chaque ligne pour les mesures de test (`<chemin>/ligne`).
    sondes: Cell<bool>,
    /// Ce que l'application fournit aux actions (menus, presse-papiers, système).
    actions: RefCell<Option<Rc<ActionsExplorateur>>>,
    /// Le champ de renommage ouvert sur une ligne.
    renommage: RefCell<Option<Renommage>>,
    /// Le ressort des dépôts (dossier fermé survolé qui s'ouvre).
    ressort: RefCell<Ressort>,
    /// Le dossier coloré comme cible d'un dépôt (glisser interne ou fichiers du système).
    cible_depot: RefCell<Option<String>>,
    /// Un glisser de fichier a eu lieu : le clic qui suit ne doit pas ouvrir le fichier.
    a_glisse: Cell<bool>,
    /// Le chemin et la hauteur (px de fenêtre) de chaque ligne visible, relevés au dernier rendu.
    rangees: RefCell<Vec<(String, f32)>>,
    /// Le zoom du dernier rendu (px de fenêtre par px logique).
    zoom: Cell<f32>,
    /// Les crochets du glisser de fichier ont été posés par cette vue.
    branche: Cell<bool>,
    /// L'horloge du ressort.
    debut: Instant,
}

/// La vue `file-explorer`, côté modèle.
pub struct FileExplorerView {
    interne: Rc<Interne>,
    base: ItemViewBase,
    abonnements: Vec<EventRef>,
}

impl FileExplorerView {
    /// Monte l'explorateur : s'abonne au coffre (créer, supprimer, renommer) et au Workspace.
    pub fn new(ctx: ContexteExplorateur, app_scope: &Scope) -> Self {
        let interne = Rc::new(Interne {
            vault: ctx.vault.clone(),
            etat: RefCell::new(EtatArbre::new()),
            defilement: ScrollHandle::new(),
            defilement_attente: Cell::new(None),
            reveler_attente: RefCell::new(None),
            ouvrir: ctx.ouvrir,
            sauvegarder: ctx.sauvegarder,
            version: Cell::new(0),
            sondes: Cell::new(false),
            actions: RefCell::new(None),
            renommage: RefCell::new(None),
            ressort: RefCell::new(Ressort::new()),
            cible_depot: RefCell::new(None),
            a_glisse: Cell::new(false),
            rangees: RefCell::new(Vec::new()),
            zoom: Cell::new(1.0),
            branche: Cell::new(false),
            debut: Instant::now(),
        });
        let mut abonnements = Vec::new();

        // Le coffre : seule la forme de l'arbre compte (`modify` n'est volontairement pas écouté).
        let i = interne.clone();
        abonnements.push(ctx.vault.on(VaultEventType::Create, move |_| {
            i.version.set(i.version.get() + 1);
        }));
        let i = interne.clone();
        abonnements.push(ctx.vault.on(VaultEventType::Delete, move |e| {
            if let VaultEvent::Delete { file, .. } = e {
                i.etat.borrow_mut().supprimer(file.path());
                i.version.set(i.version.get() + 1);
            }
        }));
        let i = interne.clone();
        abonnements.push(ctx.vault.on(VaultEventType::Rename, move |e| {
            if let VaultEvent::Rename { file, old_path, .. } = e {
                i.etat.borrow_mut().renommer(old_path, file.path());
                i.version.set(i.version.get() + 1);
            }
        }));

        // Le fichier actif : celui de la feuille active.
        // ★ `file-open` à `None` (la feuille active est l'explorateur lui-même, ou une vue sans
        //   document) NE retire PAS l'actif : cliquer un dossier rend l'explorateur actif, et le
        //   fichier ouvert doit rester surligné.
        let i = interne.clone();
        abonnements.push(ctx.workspace_events.on(FILE_OPEN, move |d| {
            let Some(Some(cle)) = d.downcast_ref::<Option<DocumentKey>>() else {
                return;
            };
            if i.vault.get_file_by_path(cle.as_str()).is_some() {
                i.etat.borrow_mut().definir_actif(Some(cle.as_str()));
            }
        }));
        // Le fil d'Ariane des vues de fichier demande à voir un dossier.
        let i = interne.clone();
        abonnements.push(ctx.workspace_events.on(REVEAL_IN_FOLDER, move |d| {
            if let Some(r) = d.downcast_ref::<RevealInFolder>() {
                Self::reveler_dans(&i, &r.folder.path);
            }
        }));

        Self {
            interne,
            base: ItemViewBase::new(app_scope),
            abonnements,
        }
    }

    fn reveler_dans(interne: &Interne, chemin: &str) {
        let a_ouvert = interne.etat.borrow_mut().reveler(chemin);
        *interne.reveler_attente.borrow_mut() = Some(chemin.to_owned());
        if a_ouvert {
            (interne.sauvegarder)();
        }
    }

    /// Révèle un fichier ou un dossier (`revealInFolder`) : ouvre ses ANCÊTRES (pas lui-même), le
    /// surligne et le fait défiler à l'écran. Appelable dès la création de la vue : le défilement
    /// attend la première mise en page.
    pub fn reveal_in_folder(&self, chemin: &str) {
        Self::reveler_dans(&self.interne, chemin);
    }

    /// Les lignes visibles, dans l'ordre d'affichage.
    pub fn lignes(&self) -> Vec<LigneVisible> {
        lignes_visibles(&self.interne.vault, &self.interne.etat.borrow())
    }

    /// Les dossiers ouverts (`getExpanded`), la racine `""` comprise.
    pub fn ouverts(&self) -> Vec<String> {
        self.interne.etat.borrow().ouverts()
    }

    /// Rouvre exactement ces dossiers (`setExpanded`).
    pub fn definir_ouverts(&self, chemins: &[String]) {
        self.interne.etat.borrow_mut().restaurer(chemins);
    }

    /// L'élément actif, s'il y en a un.
    pub fn actif(&self) -> Option<String> {
        self.interne.etat.borrow().actif().map(str::to_owned)
    }

    /// Le clic d'un dossier : l'ouvre ou le ferme, et demande la sauvegarde du layout.
    pub fn basculer_dossier(&self, chemin: &str) {
        Self::basculer(&self.interne, chemin);
    }

    fn basculer(interne: &Interne, chemin: &str) {
        if interne.etat.borrow_mut().basculer(chemin) {
            (interne.sauvegarder)();
        }
    }

    /// Le clic d'un fichier : l'élément actif, puis l'ouvrir.
    pub fn ouvrir_fichier(&self, chemin: &str) {
        Self::ouvrir_chemin(&self.interne, chemin);
    }

    fn ouvrir_chemin(interne: &Interne, chemin: &str) {
        interne.etat.borrow_mut().definir_actif(Some(chemin));
        if let Some(f) = interne.vault.get_file_by_path(chemin) {
            (interne.ouvrir)(&f);
        }
    }

    /// « Tout replier » : seule la racine reste ouverte.
    pub fn tout_replier(&self) {
        self.interne.etat.borrow_mut().tout_replier();
        (self.interne.sauvegarder)();
    }

    /// « Trier » : inverse le sens du tri des noms.
    pub fn basculer_tri(&self) {
        self.interne.etat.borrow_mut().basculer_tri();
    }

    /// Le nombre d'événements du coffre qui ont changé la forme de l'arbre.
    pub fn version(&self) -> u64 {
        self.interne.version.get()
    }

    /// Nomme chaque ligne (`<chemin>/ligne`) et le conteneur (`explorateur/liste`) pour les mesures.
    pub fn activer_sondes(&self) {
        self.interne.sondes.set(true);
    }

    /// La poignée de défilement de la liste.
    pub fn defilement(&self) -> &ScrollHandle {
        &self.interne.defilement
    }
}

// ── Les actions (fr-t2-explorateur-actions) ───────────────────────────────────────────────────

/// Ce que la vue répond aux menus (`GesteVue`) : un pont vers l'état partagé.
struct GesteInterne(Rc<Interne>);

impl GesteVue for GesteInterne {
    fn renommer(&self, chemin: &str) {
        Interne::demarrer_renommage(&self.0, chemin);
    }

    fn deplacer(&self, chemin: &str) {
        self.0.demander_deplacement(chemin);
    }

    fn deplier(&self, chemin: &str) {
        self.0.ouvrir_dossier(chemin);
    }

    fn ouvrir(&self, fichier: &TFile) {
        FileExplorerView::ouvrir_chemin(&self.0, &fichier.path);
    }
}

impl Interne {
    fn actions(&self) -> Option<Rc<ActionsExplorateur>> {
        self.actions.borrow().clone()
    }

    fn signaler(&self, message: &str) {
        if let Some(a) = self.actions() {
            (a.signaler)(message);
        }
    }

    /// Ouvre un dossier (idempotent) et demande la sauvegarde du layout s'il était fermé.
    fn ouvrir_dossier(&self, chemin: &str) {
        if self.etat.borrow_mut().ouvrir(chemin) {
            (self.sauvegarder)();
        }
    }

    /// Entre en renommage sur `chemin` (`startRename`) : la ligne est révélée pour rester visible.
    ///
    /// ★ Sans devenir l'élément ACTIF : `startRename` ne pose que le chemin du champ. La ligne
    /// renommée gardait le fond de l'actif (scène t2-explorateur-renommer, run 43351537908).
    fn demarrer_renommage(interne: &Rc<Self>, chemin: &str) {
        if let Some(r) = Renommage::ouvrir(&interne.vault, chemin) {
            *interne.renommage.borrow_mut() = Some(r);
            let actif = interne.etat.borrow().actif().map(str::to_owned);
            FileExplorerView::reveler_dans(interne, chemin);
            interne.etat.borrow_mut().definir_actif(actif.as_deref());
        }
    }

    fn demander_deplacement(&self, chemin: &str) {
        let signaler = self.actions().map_or_else(
            || Rc::new(|_: &str| {}) as Rc<dyn Fn(&str)>,
            |a| a.signaler.clone(),
        );
        deplacer::ouvrir_modale(self.vault.clone(), chemin, signaler);
    }

    fn geste(interne: &Rc<Self>) -> Rc<dyn GesteVue> {
        Rc::new(GesteInterne(interne.clone()))
    }

    fn maintenant_ms(&self) -> u64 {
        u64::try_from(self.debut.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Referme des dossiers (ceux que le ressort avait ouverts).
    fn fermer_dossiers(&self, chemins: &[String]) {
        if chemins.is_empty() {
            return;
        }
        {
            let mut etat = self.etat.borrow_mut();
            for c in chemins {
                if etat.est_ouvert(c) {
                    etat.basculer(c);
                }
            }
        }
        (self.sauvegarder)();
    }

    /// L'élément de l'arbre sous un point de la fenêtre (px de fenêtre) : `Some("")` dans le vide de
    /// la liste (la racine), `None` hors de la liste (`explorerPathAt`).
    fn chemin_sous_le_point(&self, x: f32, y: f32) -> Option<String> {
        let cadre = self.defilement.bounds();
        let (gauche, haut): (f32, f32) = (cadre.origin.x.into(), cadre.origin.y.into());
        let (largeur, hauteur): (f32, f32) = (cadre.size.width.into(), cadre.size.height.into());
        if hauteur <= 0.0 || x < gauche || x > gauche + largeur || y < haut || y > haut + hauteur {
            return None;
        }
        let decalage = -f32::from(self.defilement.offset().y);
        let rangees = self.rangees.borrow();
        let hauteurs: Vec<f32> = rangees.iter().map(|(_, h)| *h).collect();
        Some(
            depot::rang_a(&hauteurs, y - haut + decalage)
                .map_or_else(String::new, |i| rangees[i].0.clone()),
        )
    }

    /// Le survol d'un dépôt de fichiers du système au point `(x, y)` (px de fenêtre).
    fn survoler_depot_au_point(&self, x: f32, y: f32) {
        let Some(chemin) = self.chemin_sous_le_point(x, y) else {
            self.finir_depot(true);
            return;
        };
        let dossier = depot::dossier_de(&self.vault, &chemin);
        self.survoler_dossier(&dossier, self.maintenant_ms());
        self.avancer_ressort(self.maintenant_ms());
    }

    fn survoler_dossier(&self, dossier: &str, maintenant: u64) {
        let ordres = {
            let etat = self.etat.borrow();
            self.ressort
                .borrow_mut()
                .survoler(dossier, |d| etat.est_ouvert(d), maintenant)
        };
        self.fermer_dossiers(&ordres.fermer);
        *self.cible_depot.borrow_mut() = Some(dossier.to_owned());
    }

    fn avancer_ressort(&self, maintenant: u64) {
        let ouvrir = self.ressort.borrow_mut().avancer(maintenant);
        if let Some(dossier) = ouvrir {
            self.ouvrir_dossier(&dossier);
        }
    }

    /// Le glisser de fichiers du système est fini (`endDrag`).
    fn finir_depot(&self, restaurer: bool) {
        let a_fermer = self.ressort.borrow_mut().finir(restaurer);
        self.fermer_dossiers(&a_fermer);
        *self.cible_depot.borrow_mut() = None;
    }

    /// Importe `fichiers` dans le dossier visé par `chemin` ; les échecs sont signalés.
    fn importer(&self, chemin: &str, fichiers: &[depot::Entrant]) {
        let dossier = depot::dossier_de(&self.vault, chemin);
        self.finir_depot(false);
        for (nom, issue) in depot::importer(&self.vault, &dossier, fichiers) {
            if let Err(e) = issue {
                self.signaler(&format!("import impossible : {nom} ({e})"));
            }
        }
    }

    /// Le dépôt de fichiers du système au point `(x, y)` : importe au nom libre.
    fn deposer_au_point(&self, x: f32, y: f32, chemins: &[PathBuf]) {
        let dossier = self
            .chemin_sous_le_point(x, y)
            .map_or_else(String::new, |c| depot::dossier_de(&self.vault, &c));
        self.finir_depot(false);
        for (nom, issue) in depot::importer_chemins(&self.vault, &dossier, chemins) {
            if let Err(e) = issue {
                self.signaler(&format!("import impossible : {nom} ({e})"));
            }
        }
    }
}

impl FileExplorerView {
    /// Donne à la vue ce qu'elle ne sait pas faire seule : workspace, presse-papiers, système,
    /// corbeille. Sans actions, les menus ne s'ouvrent pas.
    pub fn definir_actions(&self, actions: ActionsExplorateur) {
        *self.interne.actions.borrow_mut() = Some(Rc::new(actions));
    }

    /// Le menu d'un élément (`ouvrirMenuFichier`) : `""` = la racine du coffre. `None` sans actions
    /// ou si le chemin n'existe pas.
    pub fn menu_de(&self, chemin: &str) -> Option<crate::menu::Menu> {
        let actions = self.interne.actions()?;
        let geste = Interne::geste(&self.interne);
        if chemin.is_empty() {
            return Some(menu_racine(&self.interne.vault, &actions, &geste));
        }
        let af: TAbstractFile = self.interne.vault.get_abstract_file_by_path(chemin)?;
        Some(construire_menu(&self.interne.vault, &actions, &geste, &af))
    }

    /// Ouvre le menu de `chemin` à `(x, y)` (px logiques de la fenêtre).
    pub fn ouvrir_menu(&self, chemin: &str, x: f32, y: f32) {
        Self::montrer_menu(&self.interne, chemin, x, y);
    }

    fn montrer_menu(interne: &Rc<Interne>, chemin: &str, x: f32, y: f32) {
        let Some(actions) = interne.actions() else {
            return;
        };
        let geste = Interne::geste(interne);
        let menu = if chemin.is_empty() {
            menu_racine(&interne.vault, &actions, &geste)
        } else if let Some(af) = interne.vault.get_abstract_file_by_path(chemin) {
            construire_menu(&interne.vault, &actions, &geste, &af)
        } else {
            return;
        };
        menu.show_at_position(x, y, &actions.keymap);
    }

    /// Le bouton « Nouvelle note » de l'en-tête : une note au nom libre à la racine, ouverte.
    pub fn nouvelle_note(&self) {
        match creer_note(&self.interne.vault, "") {
            Ok(f) => Self::ouvrir_chemin(&self.interne, &f.path),
            Err(e) => self.interne.signaler(&format!("création refusée : {e}")),
        }
    }

    /// Le bouton « Nouveau dossier » de l'en-tête : un dossier au nom libre à la racine, en
    /// renommage.
    pub fn nouveau_dossier(&self) {
        match creer_dossier(&self.interne.vault, "") {
            Ok(d) => Interne::demarrer_renommage(&self.interne, &d.path),
            Err(e) => self.interne.signaler(&format!("création refusée : {e}")),
        }
    }

    /// Passe la ligne `chemin` en renommage (`startRename`).
    pub fn demarrer_renommage(&self, chemin: &str) {
        Interne::demarrer_renommage(&self.interne, chemin);
    }

    /// Le champ de renommage ouvert, s'il y en a un.
    pub fn renommage(&self) -> Option<Renommage> {
        self.interne.renommage.borrow().clone()
    }

    /// Une frappe de texte dans le champ de renommage.
    pub fn saisir(&self, texte: &str) {
        if let Some(r) = self.interne.renommage.borrow_mut().as_mut() {
            r.inserer(texte);
        }
    }

    /// Une touche pour le champ de renommage (`"enter"`, `"escape"`, `"backspace"`). Entrée valide,
    /// Échap annule. Rend vrai si la touche a été prise par le champ.
    pub fn touche_renommage(&self, touche: &str) -> bool {
        let suite = {
            let mut r = self.interne.renommage.borrow_mut();
            let Some(r) = r.as_mut() else {
                return false;
            };
            r.touche(touche)
        };
        match suite {
            Suite::Valider => self.valider_renommage(),
            Suite::Annuler => self.annuler_renommage(),
            Suite::Continuer => {}
        }
        true
    }

    /// Termine le renommage avec la saisie (Entrée, ou la perte du focus : `onBlur`).
    pub fn valider_renommage(&self) {
        let Some(r) = self.interne.renommage.borrow_mut().take() else {
            return;
        };
        if let Issue::Refuse(e) =
            renommer::valider(&self.interne.vault, r.chemin(), Some(r.saisie()))
        {
            self.interne.signaler(&format!("renommage refusé : {e}"));
        }
    }

    /// Abandonne le renommage sans rien changer (Échap).
    pub fn annuler_renommage(&self) {
        self.interne.renommage.borrow_mut().take();
    }

    /// Ouvre la modale « Déplacer vers… » pour `chemin`.
    pub fn demander_deplacement(&self, chemin: &str) {
        self.interne.demander_deplacement(chemin);
    }

    /// Le dossier coloré comme cible d'un dépôt, s'il y en a un.
    pub fn cible_depot(&self) -> Option<String> {
        self.interne.cible_depot.borrow().clone()
    }

    /// Un dépôt de fichiers du système survole `dossier` à l'instant `maintenant_ms` (le ressort
    /// ouvre un dossier fermé après [`depot::DELAI_RESSORT_MS`]).
    pub fn survoler_depot(&self, dossier: &str, maintenant_ms: u64) {
        self.interne.survoler_dossier(dossier, maintenant_ms);
    }

    /// L'horloge du ressort avance : ouvre le dossier dont le délai est échu.
    pub fn avancer_ressort(&self, maintenant_ms: u64) {
        self.interne.avancer_ressort(maintenant_ms);
    }

    /// Le dépôt est fini (`endDrag`) : `restaurer` referme ce que le survol a ouvert.
    pub fn finir_depot(&self, restaurer: bool) {
        self.interne.finir_depot(restaurer);
    }

    /// Les fichiers déposés depuis le système, importés au nom libre dans le dossier visé par
    /// `chemin` (le dossier lui-même, le parent d'un fichier, ou la racine).
    pub fn deposer_fichiers(&self, chemin: &str, fichiers: &[depot::Entrant]) {
        self.interne.importer(chemin, fichiers);
    }

    /// Branche le glisser de FICHIER du workspace sur l'arbre : le dossier survolé se colore, un
    /// lâcher sur l'arbre déplace, un lâcher sur une pile ouvre le fichier dans la feuille créée
    /// (`ouvrir` reçoit cette feuille et le chemin). Un seul explorateur par fil.
    pub fn brancher_glisser(&self, ouvrir: crate::glisser::RappelOuvrir) {
        let i = self.interne.clone();
        let survol = self.interne.clone();
        let deplace = self.interne.clone();
        let etat = self.interne.clone();
        poser_crochets(CrochetsFichier {
            chemin_sous_le_point: Some(Rc::new(move |x, y| {
                let zoom = i.zoom.get();
                i.chemin_sous_le_point(x as f32 * zoom, y as f32 * zoom)
            })),
            ouvrir: Some(ouvrir),
            survol: Some(Rc::new(move |chemin| {
                let dossier = chemin.map(|c| depot::dossier_de(&survol.vault, c));
                *survol.cible_depot.borrow_mut() = dossier;
            })),
            deplacer: Some(Rc::new(move |fichier, cible| {
                *deplace.cible_depot.borrow_mut() = None;
                if let Err(e) = depot::deplacer_fichier(&deplace.vault, fichier, cible) {
                    deplace.signaler(&format!("déplacement refusé : {e}"));
                }
            })),
            etat: Some(Rc::new(move |en_cours| {
                if en_cours {
                    etat.a_glisse.set(true);
                } else {
                    *etat.cible_depot.borrow_mut() = None;
                }
            })),
        });
        self.interne.branche.set(true);
    }

    /// Un glisser de fichier a eu lieu depuis le dernier clic.
    pub fn a_glisse(&self) -> bool {
        self.interne.a_glisse.get()
    }
}

impl Drop for FileExplorerView {
    fn drop(&mut self) {
        if self.interne.branche.get() {
            retirer_crochets();
        }
        for a in &self.abonnements {
            a.off();
        }
    }
}

impl View for FileExplorerView {
    fn get_view_type(&self) -> String {
        FILE_EXPLORER_VIEW_TYPE.to_owned()
    }

    /// La racine a un nom vide par décision de modèle : afficher le nom du coffre est une décision
    /// d'AFFICHAGE, prise ici.
    fn get_display_text(&self) -> String {
        self.interne.vault.get_name()
    }

    fn get_icon(&self) -> String {
        "folder-closed".to_owned()
    }

    fn scope(&self) -> Option<Scope> {
        Some(self.base.scope.clone())
    }

    /// `{ scroll, expanded }` : le défilement de l'arbre et ses dossiers OUVERTS.
    fn get_ephemeral_state(&self) -> Map<String, Value> {
        let i = &self.interne;
        let defilement = i
            .defilement_attente
            .get()
            .unwrap_or_else(|| -f32::from(i.defilement.offset().y));
        vers_ephemere(&i.etat.borrow(), defilement)
    }

    /// ★ Les dossiers D'ABORD : le défilement, rejoué à la première mise en page, se mesure sur
    ///   l'arbre déplié.
    fn set_ephemeral_state(&mut self, state: &Value) {
        if let Some(ouverts) = ouverts_de(state) {
            self.interne.etat.borrow_mut().restaurer(&ouverts);
        }
        if let Some(d) = defilement_de(state) {
            self.interne.defilement_attente.set(Some(d.max(0.0)));
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ItemView for FileExplorerView {
    fn item_scope(&self) -> &Scope {
        &self.base.scope
    }
}

/// La fabrique du type `file-explorer`, à ranger dans le registre des vues (`register_view` d'un
/// plugin cœur).
pub fn fabrique(ctx: ContexteExplorateur) -> ViewCreator {
    Rc::new(move |c: &ContexteVue| Box::new(FileExplorerView::new(ctx.clone(), &c.app_scope)))
}

/// Le contenu d'une vue pour [`crate::onglets::ContenuDeVue`] : l'arbre, si la vue est
/// l'explorateur.
pub fn contenu(vue: &dyn View, cx: &App) -> Option<AnyElement> {
    vue.as_any()
        .downcast_ref::<FileExplorerView>()
        .map(|v| rendre_explorateur(v, cx))
}

/// Rejoue les demandes de défilement en attente, dès que la liste a une taille mesurée.
fn defiler_si_pret(interne: &Interne, hauteurs: &[Pixels], lignes: &[LigneVisible], bas: Pixels) {
    let visible: f32 = interne.defilement.bounds().size.height.into();
    if visible <= 0.0 {
        return;
    }
    let total: f32 = hauteurs.iter().map(|h| f32::from(*h)).sum::<f32>() + f32::from(bas);
    let maximum = (total - visible).max(0.0);
    let mut position = -f32::from(interne.defilement.offset().y);
    if let Some(d) = interne.defilement_attente.take() {
        position = d.min(maximum);
    }
    if let Some(chemin) = interne.reveler_attente.borrow_mut().take()
        && let Some(rang) = lignes.iter().position(|l| l.chemin == chemin)
    {
        // `scrollIntoView({ block: 'nearest' })` : le moins de défilement possible.
        let haut: f32 = hauteurs[..rang].iter().map(|h| f32::from(*h)).sum();
        let bas_ligne = haut + f32::from(hauteurs[rang]);
        if haut < position {
            position = haut;
        } else if bas_ligne > position + visible {
            position = bas_ligne - visible;
        }
        position = position.clamp(0.0, maximum);
    }
    interne.defilement.set_offset(point(px(0.0), px(-position)));
}

/// Les lignes de profondeur `profondeur` à partir du rang `*rang`, chacune suivie de ses enfants.
fn construire(
    interne: &Rc<Interne>,
    lignes: &[LigneVisible],
    hauteurs: &[Pixels],
    rang: &mut usize,
    profondeur: usize,
    actif: Option<&str>,
    cx: &App,
) -> Vec<AnyElement> {
    let mut sortie = Vec::new();
    while *rang < lignes.len() && lignes[*rang].profondeur == profondeur {
        let l = &lignes[*rang];
        let h = hauteurs[*rang];
        *rang += 1;
        let enfants = if l.dossier && l.ouvert {
            construire(interne, lignes, hauteurs, rang, profondeur + 1, actif, cx)
        } else {
            Vec::new()
        };
        // Le champ de renommage remplace le nom de la ligne : tout sélectionné à l'ouverture (sans
        // curseur, comme une sélection non vide de Chromium), puis la saisie et un curseur.
        let (titre, selectionne): (SharedString, bool) = match interne.renommage.borrow().as_ref() {
            Some(r) if r.chemin() == l.chemin && r.selectionne() => {
                (r.saisie().to_owned().into(), true)
            }
            Some(r) if r.chemin() == l.chemin => (format!("{}\u{258f}", r.saisie()).into(), false),
            _ => (l.titre.clone().into(), false),
        };
        let cible = interne.cible_depot.borrow().as_deref() == Some(l.chemin.as_str());
        let mut ligne = Ligne::new(SharedString::from(l.chemin.clone()), titre)
            .profondeur(profondeur)
            .actif(actif == Some(l.chemin.as_str()))
            .cible_depot(l.dossier && cible)
            .titre_selectionne(selectionne)
            .hauteur(h);
        if l.dossier {
            ligne = ligne.repliable(!l.ouvert);
        }
        if interne.sondes.get() {
            ligne = ligne.sonde(l.chemin.clone());
        }
        let (i, chemin, dossier) = (interne.clone(), l.chemin.clone(), l.dossier);
        ligne = ligne.sur_clic(move |_, fenetre, _| {
            if i.renommage
                .borrow()
                .as_ref()
                .is_some_and(|r| r.chemin() == chemin)
            {
                return;
            }
            if dossier {
                FileExplorerView::basculer(&i, &chemin);
            } else if !i.a_glisse.replace(false) {
                // (Le clic qui suit un glisser n'ouvre pas le fichier : le drapeau vient de tomber.)
                FileExplorerView::ouvrir_chemin(&i, &chemin);
            }
            fenetre.refresh();
        });
        let (i, chemin) = (interne.clone(), l.chemin.clone());
        ligne = ligne.sur_menu(move |evt, fenetre, app| {
            let zoom = Echelle::courante(app).zoom();
            FileExplorerView::montrer_menu(
                &i,
                &chemin,
                f32::from(evt.position.x) / zoom,
                f32::from(evt.position.y) / zoom,
            );
            // La ligne a pris le clic droit : le « vide » de l'arbre n'ouvre pas le menu racine.
            app.stop_propagation();
            fenetre.refresh();
        });
        let mut item = rendre_item(ligne, enfants, cx);
        if !l.dossier {
            // L'appui sur un fichier note le début d'un glisser (le seuil est jugé par le workspace).
            let (chemin, libelle) = (l.chemin.clone(), l.titre.to_string());
            item = div()
                .on_mouse_down(MouseButton::Left, move |evt, _, app| {
                    noter_appui_fichier(&chemin, &libelle, evt.position, app);
                })
                .child(item)
                .into_any_element();
        }
        sortie.push(item);
    }
    sortie
}

/// Dessine l'arbre : les lignes visibles du coffre, dans le conteneur défilant (`.nav-files-container`).
pub fn rendre_explorateur(vue: &FileExplorerView, cx: &App) -> AnyElement {
    let interne = &vue.interne;
    let t = theme_ou_defaut(cx);
    let lignes = vue.lignes();
    let hauteurs = hauteurs_calees(cx, 0.0, lignes.len());
    let bas = u(cx, t.forme.size_2_3);
    defiler_si_pret(interne, &hauteurs, &lignes, bas);
    // Ce que le glisser et le dépôt ont besoin de savoir de l'image courante.
    interne.zoom.set(Echelle::courante(cx).zoom());
    *interne.rangees.borrow_mut() = lignes
        .iter()
        .zip(&hauteurs)
        .map(|(l, h)| (l.chemin.clone(), f32::from(*h)))
        .collect();
    interne.avancer_ressort(interne.maintenant_ms());

    let actif = interne.etat.borrow().actif().map(str::to_owned);
    let mut rang = 0;
    let rangees = construire(
        interne,
        &lignes,
        &hauteurs,
        &mut rang,
        0,
        actif.as_deref(),
        cx,
    );

    let mut liste = div()
        .id("nav-files-container")
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .track_scroll(&interne.defilement)
        .px(u(cx, t.forme.size_4_1))
        .pb(bas)
        .child(div().flex().flex_col().children(rangees));
    // Clic droit dans le vide : le menu du dossier RACINE (les lignes ont arrêté le leur).
    let i = interne.clone();
    liste = liste.on_mouse_down(MouseButton::Right, move |evt, fenetre, app| {
        let zoom = Echelle::courante(app).zoom();
        FileExplorerView::montrer_menu(
            &i,
            "",
            f32::from(evt.position.x) / zoom,
            f32::from(evt.position.y) / zoom,
        );
        fenetre.refresh();
    });
    // Fichiers déposés depuis le système : le dossier visé se colore (et s'ouvre après un délai),
    // le lâcher importe au nom libre. Quitter la liste sans lâcher referme ce que le survol a ouvert.
    let i = interne.clone();
    liste =
        liste.on_drag_move::<ExternalPaths>(move |e: &DragMoveEvent<ExternalPaths>, fenetre, _| {
            if e.bounds.contains(&e.event.position) {
                i.survoler_depot_au_point(
                    f32::from(e.event.position.x),
                    f32::from(e.event.position.y),
                );
            } else if i.cible_depot.borrow().is_some() {
                i.finir_depot(true);
            }
            fenetre.refresh();
        });
    let i = interne.clone();
    liste = liste.on_drop::<ExternalPaths>(move |chemins, fenetre, _| {
        let p = fenetre.mouse_position();
        i.deposer_au_point(f32::from(p.x), f32::from(p.y), chemins.paths());
        fenetre.refresh();
    });
    if interne.sondes.get() {
        liste = liste.debug_selector(|| "explorateur/liste".to_owned());
    }
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(rendre_en_tete(interne, cx))
        .child(liste)
        .into_any_element()
}

/// Un geste d'un bouton de l'en-tête.
type GesteEnTete = fn(&Rc<Interne>);

/// Les boutons de l'en-tête (`addNavButton`), dans l'ordre : icône (au trait fin), libellé, geste.
const BOUTONS_EN_TETE: [(&str, &str, GesteEnTete); 4] = [
    ("square-pen-fin", "Nouvelle note", |i| {
        match creer_note(&i.vault, "") {
            Ok(f) => FileExplorerView::ouvrir_chemin(i, &f.path),
            Err(e) => i.signaler(&format!("création refusée : {e}")),
        }
    }),
    (
        "folder-plus-fin",
        "Nouveau dossier",
        |i| match creer_dossier(&i.vault, "") {
            Ok(d) => Interne::demarrer_renommage(i, &d.path),
            Err(e) => i.signaler(&format!("création refusée : {e}")),
        },
    ),
    ("arrow-up-narrow-wide-fin", "Trier", |i| {
        i.etat.borrow_mut().basculer_tri();
    }),
    ("chevrons-down-up-fin", "Tout replier", |i| {
        i.etat.borrow_mut().tout_replier();
        (i.sauvegarder)();
    }),
];

/// L'en-tête fixe de l'explorateur (`.nav-header`) : dans un dock, les actions de la vue vivent
/// dans son contenu (l'en-tête de vue y est masqué). Quatre `.clickable-icon.nav-action-button`
/// centrés, au-dessus de la liste qui défile.
fn rendre_en_tete(interne: &Rc<Interne>, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let survol = crate::theme_global::couleur(c.background_modifier_hover);
    let teinte = crate::theme_global::couleur(c.text_normal);
    let boutons = BOUTONS_EN_TETE
        .iter()
        .enumerate()
        .map(|(rang, &(icone, _libelle, geste))| {
            let i = interne.clone();
            div()
                .id(("nav-action-button", rang))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .p(u(cx, f.size_2_3))
                .rounded(u(cx, f.radius_s))
                .opacity(crate::onglets::mesures::OPACITE_REPOS)
                .hover(move |s| s.opacity(1.0).bg(survol))
                .on_click(move |_, fenetre, _| {
                    geste(&i);
                    fenetre.refresh();
                })
                .child(
                    gpui_kit::svg()
                        .path(crate::icones::chemin(icone))
                        .size(u(cx, f.icon_m))
                        .flex_none()
                        .text_color(teinte),
                )
        });
    div()
        .flex_none()
        .pt(u(cx, f.size_4_2))
        .px(u(cx, f.size_4_2))
        .pb(u(cx, f.size_4_1))
        .child(
            div()
                .flex()
                .justify_center()
                .gap(u(cx, f.size_4_1))
                .children(boutons),
        )
        .into_any_element()
}
