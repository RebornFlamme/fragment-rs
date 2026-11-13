// Port de app/src/ui/views/markdown/MarkdownView.ts et de app/src/ui/core/workspace/TextFileView.ts
// (la vue `markdown` : un fichier .md lu et écrit par le coffre, sauvegarde différée, relecture
// du disque, lecture seule sérialisée, position de lecture `{line, dy}`).
//! [`VueMarkdown`] : la vue de note, sans GPUI pour tout ce qui se teste. Le rendu (l'éditeur du
//! live preview) est monté par [`super::montage`] à partir des [`Liens`] de la vue.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::command::EditorContext;
use fragment_core::workspace::vue_fichier::{
    ContexteFichier, EditableFileView, FileView, FileViewBase, TextFileBase, TextFileView,
};
use fragment_core::{
    ContexteVue, DataWriteOptions, DocumentKey, Events, ItemView, ItemViewBase, Scope, TFile,
    Vault, View, ViewCreator, ViewStateResult,
};
use fragment_markdown::commandes::actions::EditeurPartage;
use serde_json::{Map, Value};

use super::note::{EditeurNote, Note, NotePartagee};
use super::position::{CarteNote, PoseNote, PositionNote, lire_position};
use crate::lecture::{DELAI_REPRISE_MS, Lecture};

/// Le type de la vue (celui d'Obsidian, une API : layouts, `setViewState`).
pub const MARKDOWN_VIEW_TYPE: &str = "markdown";
/// La clé de l'état sérialisé qui porte la lecture seule (elle survit au rechargement).
pub const CLE_LECTURE_SEULE: &str = "readOnly";

/// L'horloge de l'hôte, en millisecondes (injectée par les tests).
pub type Horloge = Rc<dyn Fn() -> u64>;

/// La position de lecture vue par le montage : ce qu'il a lu, ce qu'il doit poser.
#[derive(Default)]
pub struct Lecteur {
    lecture: Lecture,
    /// Une position à rejoindre (état éphémère appliqué, boîte revenue).
    a_poser: Option<Value>,
    /// La reprise en cours et son échéance.
    pose: Option<(PoseNote, u64)>,
    a_une_boite: bool,
    /// La dernière position lue à l'écran.
    lue: Option<Value>,
}

/// Ce que le montage reçoit de la vue : des poignées partagées, jamais la vue elle-même.
#[derive(Clone)]
pub struct Liens {
    pub note: NotePartagee,
    pub lecteur: Rc<RefCell<Lecteur>>,
    pub horloge: Horloge,
    /// L'éditeur monté (rempli par le rendu).
    pub montage: super::montage::CelluleMontage,
    fichier: Rc<RefCell<Option<TFile>>>,
    vault: Rc<Vault>,
}

impl Liens {
    /// La sauvegarde différée : écrit si l'échéance est passée. Rend vrai si elle a écrit.
    ///
    /// ★ Par `Vault::modify` (le coffre gère l'écho), comme `TextFileView::save`.
    pub fn pomper(&self, maintenant_ms: u64) -> bool {
        if !self.note.borrow_mut().sauvegarde_due(maintenant_ms) {
            return false;
        }
        self.ecrire()
    }

    /// Écrit le texte courant (Ctrl+S, sauvegarde due).
    pub fn ecrire(&self) -> bool {
        let Some(f) = self.fichier.borrow().clone() else {
            return false;
        };
        let texte = self.note.borrow().texte().to_owned();
        self.note.borrow_mut().sauvegarde_faite();
        match self.vault.modify(&f, &texte, DataWriteOptions::default()) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("fragment : markdown : sauvegarde impossible : {e}");
                false
            }
        }
    }

    /// Une frappe de l'éditeur monté : le document la prend, la sauvegarde est demandée.
    pub fn frappe(&self, texte: &str, selection: (usize, usize)) {
        let maintenant = (self.horloge)();
        let mut n = self.note.borrow_mut();
        if n.frappe(texte, selection) {
            n.demander_sauvegarde(maintenant);
        }
    }

    /// Une image du montage : la boîte, la position lue, la reprise en cours. Rend vrai quand
    /// une position a été retenue (l'hôte demande alors la sauvegarde du layout).
    pub fn image(&self, maintenant_ms: u64, carte: Option<&mut dyn CarteNote>) -> bool {
        let mut l = self.lecteur.borrow_mut();
        let Some(carte) = carte else {
            l.a_une_boite = false;
            let _ = l.lecture.boite_changee(false, false);
            return false;
        };
        let (h, g) = carte.position();
        if let Some(p) = l
            .lecture
            .boite_changee(carte.a_une_boite(), h == 0.0 && g == 0.0)
        {
            l.a_poser = Some(p);
        }
        l.a_une_boite = carte.a_une_boite();
        if let Some(p) = l.a_poser.take() {
            l.pose = match PositionNote::depuis_json(&p) {
                Some(n) => Some((PoseNote::new(n), maintenant_ms + DELAI_REPRISE_MS)),
                None => {
                    // `{top, left}` : au-dessus du texte, des pixels.
                    if let Some(o) = p.as_object() {
                        let haut = o.get("top").and_then(Value::as_f64).unwrap_or(0.0);
                        let gauche = o.get("left").and_then(Value::as_f64).unwrap_or(0.0);
                        carte.poser(haut as f32, gauche as f32);
                    }
                    None
                }
            };
        }
        if let Some((pose, echeance)) = l.pose.as_mut() {
            let fini = pose.essayer(carte) || maintenant_ms > *echeance;
            if fini {
                l.pose = None;
            }
        }
        let lue = lire_position(carte);
        l.lue = Some(lue.clone());
        l.lecture
            .image(maintenant_ms, carte.a_une_boite(), || Some(lue))
    }

    /// Un défilement de l'utilisateur : retenu à la prochaine image.
    pub fn defilement(&self) {
        self.lecteur.borrow_mut().lecture.signaler_defilement();
    }

    /// Un geste (molette, pointeur, clavier) : la reprise en cours est lâchée.
    pub fn geste(&self) {
        let mut l = self.lecteur.borrow_mut();
        l.pose = None;
        l.lecture.geste();
    }

    /// Une reprise court-elle ?
    pub fn reprise_en_cours(&self) -> bool {
        self.lecteur.borrow().pose.is_some()
    }
}

/// La vue `markdown`.
pub struct VueMarkdown {
    base: ItemViewBase,
    fichier: FileViewBase,
    texte: TextFileBase,
    liens: Liens,
}

/// Ce que la fabrique donne à chaque vue.
#[derive(Clone)]
pub struct ContexteMarkdown {
    pub vault: Rc<Vault>,
    pub workspace_events: Events,
    pub horloge: Horloge,
}

impl VueMarkdown {
    pub fn new(app_scope: &Scope, ctx: ContexteFichier, horloge: Horloge) -> Self {
        let vault = ctx.vault.clone();
        Self {
            base: ItemViewBase::new(app_scope),
            fichier: FileViewBase::new(ctx),
            texte: TextFileBase::new(),
            liens: Liens {
                note: Note::partagee(),
                lecteur: Rc::default(),
                horloge,
                montage: Rc::default(),
                fichier: Rc::default(),
                vault,
            },
        }
    }

    /// Les poignées du montage.
    pub fn liens(&self) -> &Liens {
        &self.liens
    }

    /// Le document (lecture).
    pub fn note(&self) -> std::cell::Ref<'_, Note> {
        self.liens.note.borrow()
    }

    pub fn lecture_seule(&self) -> bool {
        self.liens.note.borrow().lecture_seule()
    }

    /// `setReadOnly` : rien ne se modifie plus, ni frappe, ni commande, ni collage.
    pub fn set_read_only(&mut self, valeur: bool) {
        self.liens.note.borrow_mut().poser_lecture_seule(valeur);
    }

    /// Le bouton « Mode lecture seule » de l'en-tête.
    pub fn toggle_read_only(&mut self) {
        let v = !self.lecture_seule();
        self.set_read_only(v);
    }

    /// L'icône et le libellé du bouton de lecture seule (selon l'état courant).
    pub fn action_lecture_seule(&self) -> (&'static str, &'static str) {
        if self.lecture_seule() {
            ("pen", "Mode édition")
        } else {
            ("book-open", "Mode lecture seule")
        }
    }

    /// `getEditor` : la façade inscriptible, ou `None` en lecture seule.
    ///
    /// ★ En lecture seule, AUCUN éditeur n'est exposé : une commande qui l'atteindrait
    ///   (`activeEditor`) muterait sinon le document par-dessus le verrou.
    pub fn editeur(&self) -> Option<EditeurNote> {
        if self.lecture_seule() {
            return None;
        }
        Some(self.editeur_interne())
    }

    /// La façade, même en lecture seule (le menu l'utilise pour copier et tout sélectionner).
    pub fn editeur_interne(&self) -> EditeurNote {
        let liens = self.liens.clone();
        EditeurNote::new(
            self.liens.note.clone(),
            Rc::new(move || {
                let t = (liens.horloge)();
                liens.note.borrow_mut().demander_sauvegarde(t);
            }),
        )
    }

    /// Le contexte que reçoivent les `editor_callback` (`activeEditor`) : `None` en lecture seule.
    pub fn contexte_editeur(&self) -> Option<EditorContext> {
        let e: EditeurPartage = Rc::new(self.editeur()?);
        Some(EditorContext {
            editor: Rc::new(e),
            view: Rc::new(self.liens.clone()),
        })
    }

    /// La sauvegarde différée, à pomper par l'hôte (headless) ou par le montage.
    pub fn pomper(&self, maintenant_ms: u64) -> bool {
        self.liens.pomper(maintenant_ms)
    }
}

impl View for VueMarkdown {
    fn get_view_type(&self) -> String {
        MARKDOWN_VIEW_TYPE.to_owned()
    }
    fn get_display_text(&self) -> String {
        self.file_display_text()
    }
    fn get_icon(&self) -> String {
        "file-text".to_owned()
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
        if let Err(e) = self.save(false) {
            eprintln!("fragment : markdown : sauvegarde à la fermeture impossible : {e}");
        }
        self.desabonner_coffre();
    }
    /// L'état de fichier, plus `readOnly`.
    fn get_state(&self) -> Map<String, Value> {
        let mut s = self.file_state();
        s.insert(
            CLE_LECTURE_SEULE.to_owned(),
            Value::Bool(self.lecture_seule()),
        );
        s
    }
    /// ★ `readOnly` s'applique APRÈS l'état de fichier : le chargement repart d'un document neuf.
    fn set_state(&mut self, state: &Map<String, Value>, _: ViewStateResult) {
        self.set_file_state(state);
        if let Some(Value::Bool(ro)) = state.get(CLE_LECTURE_SEULE) {
            self.set_read_only(*ro);
        }
    }
    fn get_ephemeral_state(&self) -> Map<String, Value> {
        let l = self.liens.lecteur.borrow();
        l.lecture
            .etat_ephemere(Map::new(), l.a_une_boite, || l.lue.clone())
    }
    fn set_ephemeral_state(&mut self, state: &Value) {
        let mut l = self.liens.lecteur.borrow_mut();
        if let Some(p) = l.lecture.appliquer_etat(state) {
            l.a_poser = Some(p);
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ItemView for VueMarkdown {
    fn item_scope(&self) -> &Scope {
        &self.base.scope
    }
}

impl FileView for VueMarkdown {
    fn file_base(&self) -> &FileViewBase {
        &self.fichier
    }
    fn file_base_mut(&mut self) -> &mut FileViewBase {
        &mut self.fichier
    }
    fn on_load_file(&mut self, f: &TFile) {
        *self.liens.fichier.borrow_mut() = Some(f.clone());
        {
            let mut l = self.liens.lecteur.borrow_mut();
            l.lecture.document_change();
            l.pose = None;
            l.lue = None;
        }
        self.text_on_load_file(f);
    }
    fn on_unload_file(&mut self, f: &TFile) {
        self.text_on_unload_file(f);
        *self.liens.fichier.borrow_mut() = None;
    }
    fn on_rename(&mut self, f: &TFile) {
        *self.liens.fichier.borrow_mut() = Some(f.clone());
    }
    fn on_vault_modify(&mut self, f: &TFile) {
        self.text_on_vault_modify(f);
    }
    /// ★ Le garde armé : seule une note s'ouvre ici. Sans lui, une restauration de layout
    ///   portant `{type: markdown, file: x.pdf}` lirait un binaire en UTF-8 et le réécrirait.
    fn can_accept_extension(&self, extension: &str) -> bool {
        extension == "md"
    }
    fn est_modifiable(&self) -> bool {
        true
    }
}

impl EditableFileView for VueMarkdown {}

impl TextFileView for VueMarkdown {
    fn text_base(&self) -> &TextFileBase {
        &self.texte
    }
    fn text_base_mut(&mut self) -> &mut TextFileBase {
        &mut self.texte
    }
    fn get_view_data(&self) -> String {
        self.liens.note.borrow().texte().to_owned()
    }
    fn set_view_data(&mut self, data: &str, clear: bool) {
        self.liens.note.borrow_mut().poser_texte(data, clear);
    }
    fn clear(&mut self) {
        self.liens.note.borrow_mut().vider();
    }
}

/// L'horloge par défaut : les millisecondes depuis la création.
pub fn horloge_systeme() -> Horloge {
    let debut = std::time::Instant::now();
    Rc::new(move || u64::try_from(debut.elapsed().as_millis()).unwrap_or(u64::MAX))
}

/// La fabrique du type [`MARKDOWN_VIEW_TYPE`].
pub fn fabrique(ctx: ContexteMarkdown) -> ViewCreator {
    Rc::new(move |c: &ContexteVue| {
        Box::new(VueMarkdown::new(
            &c.app_scope,
            ContexteFichier {
                leaf: c.leaf,
                vault: ctx.vault.clone(),
                workspace_events: ctx.workspace_events.clone(),
            },
            ctx.horloge.clone(),
        ))
    })
}
