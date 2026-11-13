// Port de app/src/ui/core/workspace/View.ts, ItemView.ts (la part modèle : portée de la vue,
// état éphémère), EmptyView.ts et DeferredView.ts.
//
// Une vue, côté cœur, n'est que ce que le Workspace lui demande : son type, son titre, son
// icône, le document qu'elle montre, son état (sérialisable) et son état éphémère (la
// lecture), sa portée clavier. Son DOM, son en-tête, ses boutons sont du rendu : la vue GPUI
// est construite par fragment-ui à partir de celle-ci (docs/contrats.md, cas douteux 7).
//
// ★ Aucune vue ne connaît le Workspace ni l'App : le Workspace l'interroge et la pilote.
//   C'est l'inverse de Fragment (où `view.app.workspace` est partout), et c'est voulu : pas
//   de lien retour, donc pas de cycle de propriété ni d'emprunt réentrant.

use std::any::Any;
use std::rc::Rc;

use serde_json::{Map, Value};

use super::ViewState;
use super::ViewStateResult;
use super::arbre::ItemId;
use crate::document_key::DocumentKey;
use crate::documents::DocumentManager;
pub use crate::embed::EMPTY_VIEW_TYPE;
use crate::keymap::Scope;

/// Le type réservé de la coquille différée. Aucun plugin ne doit l'enregistrer.
pub const DEFERRED_VIEW_TYPE: &str = "deferred";

/// Une vue hébergée par une feuille.
pub trait View: Any {
    /// `"markdown"`, `"pdf"`… Le type MONTÉ — le Workspace, lui, interroge la feuille
    /// (`WorkspaceLeaf::get_view_type`), qui rend le type DEMANDÉ.
    fn get_view_type(&self) -> String;

    /// Ce qu'affiche l'onglet.
    fn get_display_text(&self) -> String;

    /// Un nom d'icône Lucide (kebab-case). `"file"` par défaut.
    fn get_icon(&self) -> String {
        "file".to_owned()
    }

    /// Les icônes qui représentent la vue réduite à elles ; défaut : son icône d'onglet.
    fn get_icons(&self) -> Vec<String> {
        vec![self.get_icon()]
    }

    /// La CLÉ du document montré (chemin du coffre ou `schéma:reste`), `None` si la vue ne
    /// montre aucun document (explorateur, réglages…).
    fn get_document(&self) -> Option<DocumentKey> {
        None
    }

    /// Vrai pour une vue « dont on part » (un document) : l'historique ← → ne retient que
    /// les départs de ces vues-là. Faux pour un panneau statique.
    fn navigation(&self) -> bool {
        false
    }

    /// La portée clavier de la vue, empilée par le Workspace quand sa feuille est active.
    /// Toute `ItemView` en a une.
    fn scope(&self) -> Option<Scope> {
        None
    }

    /// Le montage (après construction) et le démontage.
    fn on_open(&mut self) {}
    fn on_close(&mut self) {}

    /// L'état sérialisable (ce qui finit dans workspace.json).
    fn get_state(&self) -> Map<String, Value> {
        Map::new()
    }

    /// `result.history` : ce changement mérite-t-il une entrée d'historique ?
    fn set_state(&mut self, state: &Map<String, Value>, result: ViewStateResult) {
        let _ = (state, result);
    }

    /// L'état ÉPHÉMÈRE : la lecture (défilement, page, calques actifs…), opaque au cœur.
    fn get_ephemeral_state(&self) -> Map<String, Value> {
        Map::new()
    }

    fn set_ephemeral_state(&mut self, state: &Value) {
        let _ = state;
    }

    fn on_resize(&mut self) {}

    /// L'éditeur que la vue expose à `activeEditor` (commandes `editor:*`) ; `None` par défaut
    /// (vue sans éditeur, ou en lecture seule). ★ Le cœur ne nomme aucune vue : c'est elle qui
    /// fournit son contexte, opaque (`Rc<dyn Any>`).
    fn editor_context(&self) -> Option<crate::command::EditorContext> {
        None
    }

    /// Le menu « ⋯ » du pane (`onPaneMenu`). Interception par mutation : la vue reçoit un menu EN
    /// CONSTRUCTION et y AJOUTE ses entrées ; l'ouvreur (l'en-tête) l'a créé, a déclaré l'ordre des
    /// sections, et le dessine. `source` : `more-options`, `tab-header`…
    fn on_pane_menu(&self, menu: &mut super::menu_pane::MenuPane, source: &str) {
        let _ = (menu, source);
    }

    /// Pour la couche de rendu et les tests : retrouver le type concret.
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Une vue « pleine » : en-tête, contenu, et TOUJOURS une portée clavier chaînée à la racine
/// (`Scope::pour_vue`). Une vue OU un calque y pose ses touches.
pub trait ItemView: View {
    fn item_scope(&self) -> &Scope;
}

/// Ce qu'une `ItemView` embarque de commun : sa portée.
#[derive(Debug, Clone)]
pub struct ItemViewBase {
    pub scope: Scope,
}

impl ItemViewBase {
    pub fn new(app_scope: &Scope) -> Self {
        Self {
            scope: Scope::pour_vue(app_scope),
        }
    }
}

/// Ce qu'une fabrique de vue reçoit : la feuille qui l'hébergera et la portée racine (pour
/// chaîner la sienne).
#[derive(Debug, Clone)]
pub struct ContexteVue {
    pub leaf: ItemId,
    pub app_scope: Scope,
}

/// La fabrique d'un type de vue, rangée dans le registre des vues par type.
///
/// ★ Le contrat écrit `Box<dyn Fn(WorkspaceLeaf) -> Box<dyn View>>` ; la feuille ne peut pas
///   être cédée (elle vit dans l'arbre) : la fabrique reçoit un `ContexteVue`. `Rc` et non
///   `Box` : le registre rend ses valeurs par clone.
pub type ViewCreator = Rc<dyn Fn(&ContexteVue) -> Box<dyn View>>;

// ─── EmptyView ───

/// Ce qu'une feuille montre quand ce qu'elle demande n'est dans aucun registre — ou quand
/// elle est vraiment vide (onglet neuf).
///
/// ★ « Inconnu » n'est PAS une erreur : c'est un état normal et transitoire (§8.3). Un plugin
///   pas encore chargé, désactivé, un layout qui cite une vue tierce absente : la feuille
///   MÉMORISE ce qu'elle voulait, et le Workspace la reconstruit quand le registre change.
#[derive(Debug)]
pub struct EmptyView {
    base: ItemViewBase,
    /// Le type réellement demandé ; `None` pour une feuille vraiment vide.
    pub requested_type: Option<String>,
    extension: Option<String>,
    file_name: Option<String>,
    /// Montée par le Workspace (`on_open`) ? Une feuille NEUVE porte une vue vide jamais
    /// montée : un panneau blanc, sans message.
    montee: bool,
}

impl EmptyView {
    pub fn new(app_scope: &Scope, requested_type: Option<String>) -> Self {
        Self {
            base: ItemViewBase::new(app_scope),
            requested_type,
            extension: None,
            file_name: None,
            montee: false,
        }
    }

    /// Le texte du panneau. ★ Vide tant que la vue n'est pas montée : l'onglet neuf (bouton
    ///   « + », feuille créée dans une pile) et l'onglet vide RESTAURÉ sont des panneaux
    ///   blancs ; une vue vide montée par `set_view_state` dit qu'aucun fichier n'est ouvert.
    pub fn message(&self) -> String {
        if !self.montee {
            return String::new();
        }
        if let Some(ext) = &self.extension {
            return format!("Aucune vue n'est enregistrée pour les fichiers .{ext}");
        }
        match &self.requested_type {
            Some(t) => format!("Aucune vue enregistrée pour « {t} »"),
            None => "Aucun fichier ouvert".to_owned(),
        }
    }
}

impl View for EmptyView {
    fn get_view_type(&self) -> String {
        EMPTY_VIEW_TYPE.to_owned()
    }

    fn get_display_text(&self) -> String {
        if let Some(f) = &self.file_name {
            return f.clone();
        }
        if self.requested_type.is_some() {
            "Vue indisponible".to_owned()
        } else {
            "New Tab".to_owned()
        }
    }

    fn scope(&self) -> Option<Scope> {
        Some(self.base.scope.clone())
    }

    fn on_open(&mut self) {
        self.montee = true;
    }

    fn on_close(&mut self) {
        self.montee = false;
    }

    /// L'extension et le nom du fichier en attente arrivent par l'état, APRÈS le montage.
    fn set_state(&mut self, state: &Map<String, Value>, _result: ViewStateResult) {
        self.extension = state
            .get("extension")
            .and_then(Value::as_str)
            .filter(|e| !e.is_empty())
            .map(str::to_owned);
        self.file_name = state
            .get("file")
            .and_then(Value::as_str)
            .and_then(|f| f.rsplit('/').next())
            .map(str::to_owned);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ItemView for EmptyView {
    fn item_scope(&self) -> &Scope {
        &self.base.scope
    }
}

// ─── DeferredView ───

/// La coquille d'une feuille restaurée mais pas encore regardée (§8.4).
///
/// ★ À ne pas confondre avec `EmptyView` : celle-ci dit « je ne SAIS PAS ouvrir ça » (un échec
///   subi, à expliquer) ; la coquille dit « je sais, mais personne ne me regarde encore » (un
///   report voulu). Pas de portée, pas d'en-tête : elle ne paie rien.
/// ★ Son seul vrai travail : SE NOMMER. Sans titre deviné depuis l'état demandé, tous les
///   onglets restaurés s'appelleraient « New Tab » jusqu'au premier clic.
#[derive(Debug, Clone)]
pub struct DeferredView {
    requested: ViewState,
    titre: String,
    icone: String,
}

impl DeferredView {
    /// Le titre et l'icône sont devinés TOUT DE SUITE, sans instancier la vraie vue : par le
    /// fournisseur du document s'il le connaît, sinon le nom du fichier sans extension,
    /// sinon le type demandé.
    pub fn new(requested: ViewState, documents: &DocumentManager) -> Self {
        let doc = documents.key_of_state(Some(&requested));
        let fichier = requested
            .state
            .as_ref()
            .and_then(|s| s.get("file"))
            .and_then(Value::as_str)
            .filter(|f| !f.is_empty())
            .map(str::to_owned);
        let titre = match (&doc, &fichier) {
            (Some(d), _) => documents.title(d),
            (None, Some(f)) => sans_extension(f.rsplit('/').next().unwrap_or(f)).to_owned(),
            (None, None) => requested.type_.clone(),
        };
        let icone = doc
            .as_ref()
            .and_then(|d| documents.icon(d))
            .unwrap_or_else(|| {
                // ★ Heuristique assumée : l'icône est un champ d'INSTANCE, la connaître
                //   exigerait de construire la vue. Un état qui désigne un fichier prend
                //   l'icône des vues de fichier, le reste l'icône par défaut.
                if fichier.is_some() {
                    "file-text".to_owned()
                } else {
                    "file".to_owned()
                }
            });
        Self {
            requested,
            titre,
            icone,
        }
    }

    pub fn requested(&self) -> &ViewState {
        &self.requested
    }
}

fn sans_extension(nom: &str) -> &str {
    match nom.rfind('.') {
        Some(i) if i > 0 => &nom[..i],
        _ => nom,
    }
}

impl View for DeferredView {
    fn get_view_type(&self) -> String {
        DEFERRED_VIEW_TYPE.to_owned()
    }

    fn get_display_text(&self) -> String {
        self.titre.clone()
    }

    fn get_icon(&self) -> String {
        self.icone.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Keymap;
    use serde_json::json;

    // ─── Ajoutés : EmptyView / DeferredView n'ont pas de test TS ───

    #[test]
    fn la_vue_vide_neuve_est_un_panneau_blanc() {
        let k = Keymap::new();
        let v = EmptyView::new(k.root_scope(), None);
        assert_eq!(v.get_display_text(), "New Tab");
        assert_eq!(v.message(), "");
        assert!(v.scope().is_some());
    }

    #[test]
    fn la_vue_vide_montee_dit_quaucun_fichier_nest_ouvert() {
        let k = Keymap::new();
        let mut v = EmptyView::new(k.root_scope(), None);
        v.on_open();
        assert_eq!(v.message(), "Aucun fichier ouvert");
    }

    #[test]
    fn la_vue_vide_explique_ce_quelle_ne_sait_pas_ouvrir() {
        let k = Keymap::new();
        let mut v = EmptyView::new(k.root_scope(), Some("kanban".into()));
        v.on_open();
        assert_eq!(v.get_display_text(), "Vue indisponible");
        assert!(v.message().contains("kanban"));
        let etat = json!({"file": "dossier/doc.xyz", "extension": "xyz"});
        v.set_state(etat.as_object().unwrap(), ViewStateResult::default());
        assert_eq!(v.get_display_text(), "doc.xyz");
        assert!(v.message().contains(".xyz"));
    }

    #[test]
    fn la_coquille_se_nomme_depuis_letat_demande() {
        let docs = DocumentManager::new();
        let v = DeferredView::new(
            ViewState::with_state("markdown", json!({"file": "notes/trois.md"})),
            &docs,
        );
        assert_eq!(v.get_display_text(), "trois");
        assert_eq!(v.get_icon(), "file-text");
        assert_eq!(v.get_view_type(), DEFERRED_VIEW_TYPE);
        let nu = DeferredView::new(ViewState::new("graph"), &docs);
        assert_eq!(
            (nu.get_display_text(), nu.get_icon()),
            ("graph".into(), "file".into())
        );
    }
}
