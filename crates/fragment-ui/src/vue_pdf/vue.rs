// Port de app/src/ui/views/pdf/PdfView.ts (la vue `pdf` : FileView en lecture seule, garde
// d'extension, état éphémère page / zoom / barre latérale / recherche, position de lecture `scroll`)
// et de app/src/ui/core/workspace/FileView.ts (onLoadFile, onUnloadFile).
//! [`VuePdf`] : la vue de fichier des PDF, sans GPUI. Elle lit les octets du coffre et les donne
//! au montage ([`super::montage`]) par ses [`LiensPdf`] ; l'état éphémère passe par eux aussi.
//!
//! ★ Le cœur demande l'état éphémère SANS contexte GPUI : le montage en dépose un instantané à
//!   chaque image ([`LiensPdf::instantane`]), que la vue rend tel quel. Un état reçu avant le
//!   montage (restauration, retour d'onglet) attend dans [`LiensPdf::a_poser`] ; tant qu'il n'est
//!   pas appliqué, c'est LUI que la vue rend (sinon un aller-retour avant la première image le
//!   perdrait).

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use fragment_core::workspace::vue_fichier::{ContexteFichier, FileView, FileViewBase};
use fragment_core::{
    ContexteVue, DocumentKey, ItemView, ItemViewBase, Keymap, Modifier, Scope, TFile, Vault, View,
    ViewCreator, ViewStateResult,
};
use fragment_pdf::vue_modele::{Raccourci, raccourci_pour};
use serde_json::{Map, Value};

/// Le type de la vue (celui d'Obsidian, une API : layouts, `setViewState`).
pub const PDF_VIEW_TYPE: &str = "pdf";
/// L'extension que la vue ouvre.
pub const EXTENSION_PDF: &str = "pdf";

/// Un raccourci de la vue, à jouer par le montage (les liaisons n'ont pas le contexte GPUI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Touche {
    /// Ctrl+F : ouvrir la recherche.
    Chercher,
    /// Ctrl + `+` / `-` / `0` : le zoom du PDF (pas celui de l'application).
    Zoom(Raccourci),
}

/// Le document à monter : une révision (chaque chargement l'incrémente) et ses octets, ou
/// l'erreur de lecture.
#[derive(Clone, Default)]
pub struct Source {
    pub revision: u64,
    pub octets: Option<Result<Rc<Vec<u8>>, String>>,
}

/// Ce que le montage partage avec la vue.
#[derive(Clone)]
pub struct LiensPdf {
    pub source: Rc<RefCell<Source>>,
    /// Un état éphémère à appliquer à la prochaine image.
    pub a_poser: Rc<RefCell<Option<Value>>>,
    /// Le dernier état éphémère lu par le montage.
    pub instantane: Rc<RefCell<Option<Map<String, Value>>>>,
    /// Les raccourcis de la vue (Ctrl+F, Ctrl + − 0), déposés par la portée, joués par le montage.
    pub touches: Rc<RefCell<VecDeque<Touche>>>,
    /// Le montage GPUI (rempli par le rendu).
    pub montage: super::montage::CelluleMontage,
    pub keymap: Keymap,
}

impl LiensPdf {
    fn new(keymap: Keymap) -> Self {
        Self {
            source: Rc::default(),
            a_poser: Rc::default(),
            instantane: Rc::default(),
            touches: Rc::default(),
            montage: Rc::default(),
            keymap,
        }
    }
}

/// La vue `pdf`.
pub struct VuePdf {
    base: ItemViewBase,
    fichier: FileViewBase,
    vault: Rc<Vault>,
    liens: LiensPdf,
}

/// Ce que la fabrique donne à chaque vue.
#[derive(Clone)]
pub struct ContextePdf {
    pub vault: Rc<Vault>,
    pub workspace_events: fragment_core::Events,
    /// Le Keymap de l'application : la recherche y empile la portée de son champ.
    pub keymap: Keymap,
}

impl VuePdf {
    pub fn new(app_scope: &Scope, ctx: ContexteFichier, keymap: Keymap) -> Self {
        let vault = ctx.vault.clone();
        let base = ItemViewBase::new(app_scope);
        let liens = LiensPdf::new(keymap);
        // ★ Les raccourcis vivent dans la portée de la VUE, empilée par le shell dès l'activation
        //   (même avant le premier rendu) : Ctrl+F ouvre la recherche sans clic préalable.
        let file = liens.touches.clone();
        base.scope
            .register(Some(&[Modifier::Mod]), Some("f"), move |_, _| {
                file.borrow_mut().push_back(Touche::Chercher);
                Some(false)
            });
        let file = liens.touches.clone();
        base.scope
            .register(Some(&[Modifier::Mod]), None, move |ev, _| {
                let r = raccourci_pour(&ev.code, ev.mods.shift)?;
                file.borrow_mut().push_back(Touche::Zoom(r));
                Some(false)
            });
        Self {
            base,
            fichier: FileViewBase::new(ctx),
            vault,
            liens,
        }
    }

    /// Les poignées du montage.
    pub fn liens(&self) -> &LiensPdf {
        &self.liens
    }
}

impl View for VuePdf {
    fn get_view_type(&self) -> String {
        PDF_VIEW_TYPE.to_owned()
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
        self.desabonner_coffre();
    }
    /// Le fichier seul : page, zoom et barre latérale n'ont rien à faire dans workspace.json.
    fn get_state(&self) -> Map<String, Value> {
        self.file_state()
    }
    fn set_state(&mut self, state: &Map<String, Value>, _: ViewStateResult) {
        self.set_file_state(state);
    }
    fn get_ephemeral_state(&self) -> Map<String, Value> {
        if let Some(Value::Object(m)) = self.liens.a_poser.borrow().as_ref() {
            return m.clone();
        }
        self.liens.instantane.borrow().clone().unwrap_or_default()
    }
    fn set_ephemeral_state(&mut self, state: &Value) {
        if state.is_object() {
            *self.liens.a_poser.borrow_mut() = Some(state.clone());
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ItemView for VuePdf {
    fn item_scope(&self) -> &Scope {
        &self.base.scope
    }
}

impl FileView for VuePdf {
    fn file_base(&self) -> &FileViewBase {
        &self.fichier
    }
    fn file_base_mut(&mut self) -> &mut FileViewBase {
        &mut self.fichier
    }
    /// Les octets sont lus ici, une fois ; le montage ouvre le document à sa prochaine image.
    fn on_load_file(&mut self, f: &TFile) {
        let octets = self
            .vault
            .read_binary(f)
            .map(Rc::new)
            .map_err(|e| e.to_string());
        let mut s = self.liens.source.borrow_mut();
        s.revision += 1;
        s.octets = Some(octets);
        *self.liens.instantane.borrow_mut() = None;
    }
    fn on_unload_file(&mut self, _: &TFile) {
        let mut s = self.liens.source.borrow_mut();
        s.revision += 1;
        s.octets = None;
    }
    /// Un PDF réécrit sur le disque se relit (pas d'édition dans la vue).
    fn on_vault_modify(&mut self, f: &TFile) {
        self.on_load_file(f);
    }
    /// ★ Le garde armé : une restauration de layout `{type: pdf, file: x.md}` n'ouvre rien.
    fn can_accept_extension(&self, extension: &str) -> bool {
        extension == EXTENSION_PDF
    }
}

/// La fabrique du type [`PDF_VIEW_TYPE`].
pub fn fabrique(ctx: ContextePdf) -> ViewCreator {
    Rc::new(move |c: &ContexteVue| {
        Box::new(VuePdf::new(
            &c.app_scope,
            ContexteFichier {
                leaf: c.leaf,
                vault: ctx.vault.clone(),
                workspace_events: ctx.workspace_events.clone(),
            },
            ctx.keymap.clone(),
        ))
    })
}
