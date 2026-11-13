// Port de app/src/ui/core/workspace/FileView.ts (la vue de fichier : chargement par l'état
// `{file}`, titre, abonnement au coffre). Côté UI, une vue de fichier SIMPLE : elle ne dessine
// rien d'elle-même, elle sert de support aux scènes et aux tests de l'en-tête (← →, fil d'Ariane).
//! La vue `fichier-simple` : une vue de fichier minimale branchée sur le cœur.

use std::any::Any;
use std::rc::Rc;

use fragment_core::workspace::vue_fichier::{ContexteFichier, FileView, FileViewBase};
use fragment_core::{
    CommandManager, ContexteVue, DocumentKey, Events, ItemView, ItemViewBase, Scope, Vault, View,
    ViewCreator, ViewStateResult,
};
use serde_json::{Map, Value};

/// Le type de la vue de fichier simple.
pub const VUE_FICHIER_SIMPLE: &str = "fichier-simple";

/// Une vue de fichier sans rendu propre : son fichier vient de l'état `{file}`.
pub struct VueFichierSimple {
    base: ItemViewBase,
    fichier: FileViewBase,
}

impl VueFichierSimple {
    /// Monte la vue sur la feuille décrite par `ctx` (la portée vient de l'application).
    pub fn new(app_scope: &Scope, ctx: ContexteFichier) -> Self {
        Self {
            base: ItemViewBase::new(app_scope),
            fichier: FileViewBase::new(ctx),
        }
    }
}

impl View for VueFichierSimple {
    fn get_view_type(&self) -> String {
        VUE_FICHIER_SIMPLE.to_owned()
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

impl ItemView for VueFichierSimple {
    fn item_scope(&self) -> &Scope {
        &self.base.scope
    }
}

impl FileView for VueFichierSimple {
    fn file_base(&self) -> &FileViewBase {
        &self.fichier
    }
    fn file_base_mut(&mut self) -> &mut FileViewBase {
        &mut self.fichier
    }
}

/// La fabrique du type [`VUE_FICHIER_SIMPLE`], à ranger dans le registre des vues.
pub fn fabrique(vault: Rc<Vault>, workspace_events: Events) -> ViewCreator {
    Rc::new(move |c: &ContexteVue| {
        Box::new(VueFichierSimple::new(
            &c.app_scope,
            ContexteFichier {
                leaf: c.leaf,
                vault: vault.clone(),
                workspace_events: workspace_events.clone(),
            },
        ))
    })
}

/// Enregistre les commandes de scène `pixels:vue_fichier` : vide jusqu'à la tâche propriétaire.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
