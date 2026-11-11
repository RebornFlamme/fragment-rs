// Port de app/src/ui/core/workspace/FileView.ts, EditableFileView.ts et TextFileView.ts
//
// Les vues de FICHIER, côté cœur : la logique, sans rendu. Trois étages, comme dans Fragment :
//   `FileView` (un fichier courant, son chargement, le fil d'Ariane, le renommage par le titre),
//   `EditableFileView` (un simple marqueur : « cette vue sait écrire son fichier »),
//   `TextFileView` (le texte, la sauvegarde différée, la relecture quand le disque change).
//
// ★ ÉCARTS DE MODÈLE (la vue ne connaît ni le Workspace ni l'App, voir vue.rs) :
//   - pas d'héritage : chaque étage est un TRAIT dont les méthodes « abstraites » du TS sont les
//     hooks (`on_load_file`…) et dont l'état vit dans une structure à embarquer
//     (`FileViewBase`, `TextFileBase`). Pas de `super.` non plus : une vue de texte écrit
//     `fn on_load_file(&mut self, f) { self.text_on_load_file(f) }` ;
//   - les abonnements au coffre ne peuvent pas modifier la vue (elle appartient à sa feuille) :
//     ils déposent l'événement dans une boîte, que l'HÔTE vide en appelant
//     `traiter_evenements_coffre` après une opération du coffre (comme le TS, les gestionnaires
//     tournent dans l'ordre d'arrivée) ;
//   - les événements du Workspace (`view-file-open`…) partent par le flux d'`Events` du
//     Workspace, que la fabrique de la vue reçoit en clone ;
//   - API synchrone, temps injecté pour la sauvegarde différée (voir debounce.rs).
//
// ★ Jamais une vue de fichier ne nomme l'explorateur : elle ÉMET `reveal-in-folder`, et
//   quiconque sait révéler un dossier écoute.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use serde_json::{Map, Value};

use super::arbre::ItemId;
use super::vue::ItemView;
use crate::breadcrumb::{
    BREADCRUMB_MAX_DEPTH, BreadcrumbFolder, BreadcrumbSegment, breadcrumb_segments,
};
use crate::debounce::Debouncer;
use crate::document_key::DocumentKey;
use crate::events::{EventRef, Events};
use crate::vault::coffre::VaultResult;
use crate::vault::{
    DataWriteOptions, FileHandle, TAbstractFile, TFile, TFolder, Vault, VaultEvent, VaultEventType,
};

/// `view-file-open` : « cette vue vient de charger CE fichier ». Charge : [`ViewFileOpen`].
/// ★ Le signal PAR VUE, émis à la source unique du chargement (`load_file`), distinct de
///   `file-open` (centré sur la feuille active : il rate la réouverture dans l'onglet déjà actif).
pub const VIEW_FILE_OPEN: &str = "view-file-open";
/// `view-document-change` : la clé du document d'une vue a changé (chargement, renommage) ;
/// les calques la relisent. Charge : [`ViewDocumentChange`].
pub const VIEW_DOCUMENT_CHANGE: &str = "view-document-change";
/// `reveal-in-folder` : quelqu'un demande à voir ce dossier. Charge : [`RevealInFolder`].
pub const REVEAL_IN_FOLDER: &str = "reveal-in-folder";

/// Délai de la sauvegarde différée d'un texte, en millisecondes.
pub const SAVE_DELAY_MS: u64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewFileOpen {
    pub leaf: ItemId,
    pub file: TFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewDocumentChange {
    pub leaf: ItemId,
    pub document: DocumentKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealInFolder {
    pub folder: TFolder,
}

/// Ce que la fabrique d'une vue de fichier donne à son état de base.
#[derive(Clone)]
pub struct ContexteFichier {
    /// La feuille qui héberge la vue (`ContexteVue::leaf`).
    pub leaf: ItemId,
    pub vault: Rc<Vault>,
    /// Le flux d'événements du Workspace (un clone : les poignées sont partagées).
    pub workspace_events: Events,
}

/// Le fil d'Ariane tel que le rendu le dessine : `dossier / … / dossier / titre`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilAriane {
    pub segments: Vec<BreadcrumbSegment>,
    /// Le titre (`get_display_text`) et son infobulle (le chemin complet).
    pub title: String,
    pub path: String,
}

/// Une entrée du menu des dossiers masqués par le « … ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntreeDossier {
    pub title: String,
    pub icon: &'static str,
    pub path: String,
}

/// Le titre du header en cours d'édition, ou rien.
#[derive(Debug, Clone, Copy, Default)]
struct Renommage {
    actif: bool,
    /// Échap pressé : la fin de l'édition ANNULE au lieu de valider.
    annule: bool,
}

/// L'état commun des vues de fichier, à embarquer.
pub struct FileViewBase {
    ctx: ContexteFichier,
    file: Option<TFile>,
    /// `allowNoFile` : la vue survit-elle à la disparition de son fichier ?
    pub allow_no_file: bool,
    fil: Option<FilAriane>,
    dossiers_masques: Vec<BreadcrumbFolder>,
    renommage: Renommage,
    boite: Rc<RefCell<VecDeque<VaultEvent>>>,
    abonnements: Vec<EventRef>,
}

impl FileViewBase {
    pub fn new(ctx: ContexteFichier) -> Self {
        Self {
            ctx,
            file: None,
            allow_no_file: false,
            fil: None,
            dossiers_masques: Vec::new(),
            renommage: Renommage::default(),
            boite: Rc::default(),
            abonnements: Vec::new(),
        }
    }

    pub fn vault(&self) -> &Rc<Vault> {
        &self.ctx.vault
    }

    pub fn leaf(&self) -> ItemId {
        self.ctx.leaf
    }
}

/// Le chemin d'un fichier sans son dernier segment, avec son `/` final (`""` à la racine).
fn dossier_de(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..=i])
}

/// Un collage dans le titre : les retours à la ligne deviennent des espaces.
pub fn sanitize_pasted_title(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// Une vue de fichier.
pub trait FileView: ItemView {
    fn file_base(&self) -> &FileViewBase;
    fn file_base_mut(&mut self) -> &mut FileViewBase;

    // ─── Les hooks (méthodes « surchargeables » du TS) ───

    fn on_load_file(&mut self, file: &TFile) {
        let _ = file;
    }

    fn on_unload_file(&mut self, file: &TFile) {
        let _ = file;
    }

    /// Le fichier de la vue vient d'être renommé (`file` porte le nouveau chemin).
    fn on_rename(&mut self, file: &TFile) {
        let _ = file;
    }

    /// Le disque a modifié le fichier de la vue (`modify`).
    fn on_vault_modify(&mut self, file: &TFile) {
        let _ = file;
    }

    fn can_accept_extension(&self, extension: &str) -> bool {
        let _ = extension;
        true
    }

    /// `instanceof EditableFileView` : la vue sait-elle écrire son fichier ?
    fn est_modifiable(&self) -> bool {
        false
    }

    // ─── Le fichier courant ───

    fn file(&self) -> Option<&TFile> {
        self.file_base().file.as_ref()
    }

    /// La clé du document : le chemin du fichier (`getDocument`).
    fn file_document(&self) -> Option<DocumentKey> {
        self.file().map(|f| DocumentKey::new(f.path.clone()))
    }

    /// `getDisplayText` : le nom sans extension, ou « Aucun fichier ».
    fn file_display_text(&self) -> String {
        self.file()
            .map_or_else(|| "Aucun fichier".to_owned(), |f| f.basename().to_owned())
    }

    /// `loadFile` : décharge l'ancien fichier, pose le nouveau, annonce.
    ///
    /// Idempotent : recharger le fichier déjà chargé ne relance rien.
    fn load_file(&mut self, file: Option<TFile>) {
        let meme = match (&self.file_base().file, &file) {
            (None, None) => true,
            (Some(a), Some(b)) => a.id == b.id,
            _ => false,
        };
        if meme {
            return;
        }
        if let Some(ancien) = self.file_base().file.clone() {
            self.on_unload_file(&ancien);
        }
        self.file_base_mut().file = file.clone();
        self.refresh_breadcrumb();
        let Some(file) = file else {
            return;
        };
        self.on_load_file(&file);
        let leaf = self.file_base().ctx.leaf;
        let events = self.file_base().ctx.workspace_events.clone();
        events.trigger(
            VIEW_FILE_OPEN,
            &ViewFileOpen {
                leaf,
                file: file.clone(),
            },
        );
        events.trigger(
            VIEW_DOCUMENT_CHANGE,
            &ViewDocumentChange {
                leaf,
                document: DocumentKey::new(file.path),
            },
        );
    }

    // ─── L'état sérialisable : le CHEMIN, jamais l'objet ───

    fn file_state(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert(
            "file".to_owned(),
            self.file()
                .map_or(Value::Null, |f| Value::String(f.path.clone())),
        );
        m
    }

    /// `setState` : résout le chemin AU MOMENT DE L'USAGE (il vient d'un layout qui peut dater).
    /// Fichier absent ou extension refusée : un avertissement, la vue reste comme elle était.
    fn set_file_state(&mut self, state: &Map<String, Value>) {
        let Some(path) = state.get("file").and_then(Value::as_str) else {
            return;
        };
        let Some(file) = self.file_base().ctx.vault.get_file_by_path(path) else {
            log::warn!("[view] fichier introuvable au restore : {path}");
            return;
        };
        if !self.can_accept_extension(file.extension()) {
            log::warn!(
                "[view] {} n'accepte pas .{} : {path}",
                self.get_view_type(),
                file.extension()
            );
            return;
        }
        self.load_file(Some(file));
    }

    // ─── Les événements du coffre ───

    /// `onload` : s'abonne à `rename`, `delete` et `modify`. Idempotent.
    fn abonner_coffre(&mut self) {
        if !self.file_base().abonnements.is_empty() {
            return;
        }
        let base = self.file_base_mut();
        for genre in [
            VaultEventType::Rename,
            VaultEventType::Delete,
            VaultEventType::Modify,
        ] {
            let boite = base.boite.clone();
            let r = base
                .ctx
                .vault
                .on(genre, move |e| boite.borrow_mut().push_back(e.clone()));
            base.abonnements.push(r);
        }
    }

    /// Le démontage : plus d'abonnement, plus rien en attente.
    fn desabonner_coffre(&mut self) {
        let base = self.file_base_mut();
        for r in base.abonnements.drain(..) {
            r.off();
        }
        base.boite.borrow_mut().clear();
    }

    /// Traite, dans l'ordre d'arrivée, ce que le coffre a annoncé depuis le dernier appel.
    ///
    /// - `rename` : l'instantané du fichier est relu (un dossier ancêtre renommé change AUSSI
    ///   notre chemin, sans que l'événement porte notre fichier) ; si c'est le nôtre,
    ///   `on_rename` puis `view-document-change` ; dans tous les cas le fil est reconstruit ;
    /// - `delete` : si c'est notre fichier, la vue n'en a plus (le fil se vide) ;
    /// - `modify` : l'instantané est relu et `on_vault_modify` prévenu.
    fn traiter_evenements_coffre(&mut self) {
        loop {
            let Some(evt) = self.file_base().boite.borrow_mut().pop_front() else {
                break;
            };
            let notre_id = self.file_base().file.as_ref().map(|f| f.id);
            match evt {
                VaultEvent::Rename { file, .. } => {
                    self.relire_instantane();
                    if notre_id.is_some()
                        && Some(file.id()) == notre_id
                        && let Some(nouveau) = self.file_base().file.clone()
                    {
                        self.on_rename(&nouveau);
                        let leaf = self.file_base().ctx.leaf;
                        self.file_base().ctx.workspace_events.trigger(
                            VIEW_DOCUMENT_CHANGE,
                            &ViewDocumentChange {
                                leaf,
                                document: DocumentKey::new(nouveau.path),
                            },
                        );
                    }
                    self.refresh_breadcrumb();
                }
                VaultEvent::Delete { file, .. } => {
                    if notre_id.is_some() && Some(file.id()) == notre_id {
                        self.file_base_mut().file = None;
                        self.refresh_breadcrumb();
                        // ★ Si `!allow_no_file`, la feuille devrait se détacher : à l'hôte
                        //   (le TS avait ce TODO, la vue ne connaît pas sa feuille).
                    }
                }
                VaultEvent::Modify { file } => {
                    if notre_id == Some(file.id) {
                        self.relire_instantane();
                        self.on_vault_modify(&file);
                    }
                }
                VaultEvent::Create { .. } | VaultEvent::Raw { .. } => {}
            }
        }
    }

    /// L'instantané du fichier est une COPIE : le relire dans le coffre (chemin, stat).
    fn relire_instantane(&mut self) {
        let Some(id) = self.file_base().file.as_ref().map(|f| f.id) else {
            return;
        };
        let frais = match self.file_base().ctx.vault.get_by_id(id) {
            Some(TAbstractFile::File(f)) => Some(f),
            _ => None,
        };
        if frais.is_some() {
            self.file_base_mut().file = frais;
        }
    }

    // ─── Le fil d'Ariane ───

    /// Le fil courant, tel que le rendu doit le dessiner (`None` sans fichier).
    fn breadcrumb(&self) -> Option<&FilAriane> {
        self.file_base().fil.as_ref()
    }

    /// Reconstruit le fil depuis le chemin courant.
    ///
    /// ★ Pendant un renommage par le titre, on ne touche à rien : un `rename` venu d'ailleurs
    ///   détruirait le champ en cours de saisie. La fin de l'édition rafraîchit de toute façon.
    fn refresh_breadcrumb(&mut self) {
        if self.file_base().renommage.actif {
            return;
        }
        let Some(path) = self.file().map(|f| f.path.clone()) else {
            let b = self.file_base_mut();
            b.fil = None;
            b.dossiers_masques.clear();
            return;
        };
        let segments = breadcrumb_segments(&path, BREADCRUMB_MAX_DEPTH);
        let masques = segments
            .iter()
            .find_map(|s| match s {
                BreadcrumbSegment::Ellipsis { hidden } => Some(hidden.clone()),
                BreadcrumbSegment::Folder(_) => None,
            })
            .unwrap_or_default();
        let title = self.get_display_text();
        let b = self.file_base_mut();
        b.dossiers_masques = masques;
        b.fil = Some(FilAriane {
            segments,
            title,
            path,
        });
    }

    /// Le menu du « … » : un item par dossier masqué.
    fn hidden_folders_menu(&self) -> Vec<EntreeDossier> {
        self.file_base()
            .dossiers_masques
            .iter()
            .map(|f| EntreeDossier {
                title: f.name.clone(),
                icon: "folder",
                path: f.path.clone(),
            })
            .collect()
    }

    /// Clic sur un dossier du fil : le cœur ÉMET `reveal-in-folder` (rien si le dossier n'existe
    /// pas ou si ce chemin est un fichier).
    fn reveal_folder(&self, path: &str) {
        if let Some(folder) = self.file_base().ctx.vault.get_folder_by_path(path) {
            self.file_base()
                .ctx
                .workspace_events
                .trigger(REVEAL_IN_FOLDER, &RevealInFolder { folder });
        }
    }

    // ─── Le renommage par le titre ───

    fn is_renaming(&self) -> bool {
        self.file_base().renommage.actif
    }

    /// Clic sur le titre : les dossiers se replient, le titre passe en édition. Rend le texte à
    /// éditer (le nom SANS extension), ou `None` s'il n'y a pas de fichier ou si l'édition est
    /// déjà en cours.
    fn start_header_rename(&mut self) -> Option<String> {
        if self.file().is_none() || self.file_base().renommage.actif {
            return None;
        }
        self.file_base_mut().renommage = Renommage {
            actif: true,
            annule: false,
        };
        self.file().map(|f| f.basename().to_owned())
    }

    /// Échap pressé pendant l'édition : la fin annulera.
    fn cancel_header_rename(&mut self) {
        if self.file_base().renommage.actif {
            self.file_base_mut().renommage.annule = true;
        }
    }

    /// Fin de l'édition (Entrée, Échap, perte du focus) : valide ou annule, puis redéploie le
    /// fil. Dans tous les cas on affiche la VÉRITÉ du coffre, refus compris.
    fn finish_header_rename(&mut self, texte: &str) {
        if !self.file_base().renommage.actif {
            return;
        }
        let annule = self.file_base().renommage.annule;
        self.file_base_mut().renommage = Renommage::default();
        if !annule {
            self.rename_basename(texte);
        }
        self.traiter_evenements_coffre();
        self.refresh_breadcrumb();
    }

    /// Renomme le fichier en gardant son dossier et son extension : LE chemin de renommage des
    /// vues de fichier. Vide ou inchangé : rien ; un refus du coffre (collision, nom invalide)
    /// est journalisé. Rend `true` si le coffre a renommé.
    fn rename_basename(&mut self, nouveau: &str) -> bool {
        let Some(file) = self.file().cloned() else {
            return false;
        };
        let nom = nouveau.trim();
        if nom.is_empty() || nom == file.basename() {
            return false;
        }
        let ext = if file.extension().is_empty() {
            String::new()
        } else {
            format!(".{}", file.extension())
        };
        let cible = format!("{}{nom}{ext}", dossier_de(&file.path));
        match self.file_base().ctx.vault.rename(&file, &cible) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("[view] renommage refusé : {e}");
                false
            }
        }
    }
}

/// Le marqueur d'`EditableFileView` : une vue de fichier qui sait écrire son fichier.
/// Une vue qui l'implémente répond `true` à `est_modifiable` (le `instanceof` du TS).
pub trait EditableFileView: FileView {}

// ─── TextFileView ───

/// L'état d'une vue de texte, à embarquer à côté de `FileViewBase`.
pub struct TextFileBase {
    /// Le dernier texte lu ou écrit (`data`).
    pub data: String,
    echeance: Debouncer<()>,
    due: Rc<Cell<bool>>,
}

impl TextFileBase {
    pub fn new() -> Self {
        let due = Rc::new(Cell::new(false));
        let d = due.clone();
        Self {
            data: String::new(),
            // `true` : chaque frappe repart du délai complet (« attendre que ça se calme »).
            echeance: Debouncer::new(move |()| d.set(true), SAVE_DELAY_MS, true),
            due,
        }
    }
}

impl Default for TextFileBase {
    fn default() -> Self {
        Self::new()
    }
}

/// Une vue de texte : lit, édite, sauvegarde.
pub trait TextFileView: EditableFileView {
    fn text_base(&self) -> &TextFileBase;
    fn text_base_mut(&mut self) -> &mut TextFileBase;

    /// Le texte courant de l'éditeur.
    fn get_view_data(&self) -> String;
    /// Pose le texte dans l'éditeur ; `clear` : c'est un chargement neuf (pas de pile d'undo).
    fn set_view_data(&mut self, data: &str, clear: bool);
    /// Vide l'éditeur (le fichier le quitte).
    fn clear(&mut self);

    /// À appeler depuis `FileView::on_load_file`.
    ///
    /// ★ `read` et non `cached_read` : on s'apprête à ÉDITER, donc on veut la vérité du disque.
    fn text_on_load_file(&mut self, file: &TFile) {
        match self.file_base().vault().read(file) {
            Ok(data) => {
                self.text_base_mut().data.clone_from(&data);
                self.set_view_data(&data, true);
            }
            Err(e) => log::warn!("[view] lecture impossible de {} : {e}", file.path),
        }
    }

    /// À appeler depuis `FileView::on_unload_file` : ce qui n'est pas encore écrit l'est, puis
    /// l'éditeur est vidé.
    fn text_on_unload_file(&mut self, _file: &TFile) {
        if let Err(e) = self.save(true) {
            log::warn!("[view] sauvegarde avant déchargement impossible : {e}");
        }
    }

    /// À appeler depuis `FileView::on_vault_modify` : le disque a changé ; si l'écran diffère,
    /// il prend le texte du disque. L'écho de notre propre sauvegarde ne change donc rien.
    fn text_on_vault_modify(&mut self, file: &TFile) {
        let Ok(disque) = self.file_base().vault().read(file) else {
            return;
        };
        if disque == self.get_view_data() {
            return;
        }
        self.text_base_mut().data.clone_from(&disque);
        self.set_view_data(&disque, false);
    }

    /// `requestSave` : demande une sauvegarde, coalescée (500 ms après la dernière demande).
    /// `now_ms` : l'horloge de l'hôte, qui rappelle `poll_save`.
    fn request_save(&mut self, now_ms: u64) {
        self.text_base_mut().echeance.call((), now_ms);
    }

    /// La prochaine échéance de sauvegarde, pour que l'hôte arme SON minuteur.
    fn next_save_deadline(&self) -> Option<u64> {
        self.text_base().echeance.next_deadline()
    }

    /// Exécute la sauvegarde demandée si son échéance est passée. Rend `true` si elle a eu lieu.
    fn poll_save(&mut self, now_ms: u64) -> bool {
        self.text_base_mut().echeance.poll(now_ms);
        if !self.text_base().due.replace(false) {
            return false;
        }
        match self.save(false) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("[view] sauvegarde impossible : {e}");
                false
            }
        }
    }

    /// Écrit le texte courant dans le coffre. Sans fichier : rien.
    ///
    /// ★ Par `Vault::modify`, jamais par l'adaptateur : c'est le coffre qui gère l'écho.
    /// ★ Écart minime : une sauvegarde faite annule la demande différée en attente, qui n'aurait
    ///   plus rien à écrire (et viserait un autre fichier si la vue a changé de fichier).
    fn save(&mut self, clear: bool) -> VaultResult<()> {
        let Some(file) = self.file().cloned() else {
            return Ok(());
        };
        let data = self.get_view_data();
        self.text_base_mut().data.clone_from(&data);
        self.text_base_mut().echeance.cancel();
        self.text_base().due.set(false);
        self.file_base()
            .vault()
            .modify(&file, &data, DataWriteOptions::default())?;
        if clear {
            self.clear();
        }
        Ok(())
    }
}
