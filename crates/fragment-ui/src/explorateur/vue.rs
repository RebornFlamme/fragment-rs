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
use std::rc::Rc;

use fragment_core::workspace::vue_fichier::{REVEAL_IN_FOLDER, RevealInFolder};
use fragment_core::{
    ContexteVue, DocumentKey, EventRef, Events, FILE_OPEN, ItemView, ItemViewBase, Scope, TFile,
    Vault, VaultEvent, VaultEventType, View, ViewCreator,
};
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement as _, ParentElement as _, Pixels,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, div, point, px,
};
use serde_json::{Map, Value};

use super::arbre::{LigneVisible, lignes_visibles};
use super::etat::{EtatArbre, defilement_de, ouverts_de, vers_ephemere};
use crate::echelle::u;
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

impl Drop for FileExplorerView {
    fn drop(&mut self) {
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
        let mut ligne = Ligne::new(SharedString::from(l.chemin.clone()), l.titre.clone())
            .profondeur(profondeur)
            .actif(actif == Some(l.chemin.as_str()))
            .hauteur(h);
        if l.dossier {
            ligne = ligne.repliable(!l.ouvert);
        }
        if interne.sondes.get() {
            ligne = ligne.sonde(l.chemin.clone());
        }
        let (i, chemin, dossier) = (interne.clone(), l.chemin.clone(), l.dossier);
        ligne = ligne.sur_clic(move |_, fenetre, _| {
            if dossier {
                FileExplorerView::basculer(&i, &chemin);
            } else {
                FileExplorerView::ouvrir_chemin(&i, &chemin);
            }
            fenetre.refresh();
        });
        sortie.push(rendre_item(ligne, enfants, cx));
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
    if interne.sondes.get() {
        liste = liste.debug_selector(|| "explorateur/liste".to_owned());
    }
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(liste)
        .into_any_element()
}
