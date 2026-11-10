// Port de app/src/ui/core/app.ts (l'assemblage des gestionnaires déjà portés ; hors apparence
// et zoom, qui viennent avec fr-t1-reglages, et hors Vault, T2).
//
// L'App de Fragment est un sac de gestionnaires que tout le monde atteint par `this.app`. Ici :
// - le TRAIT `App` est ce qu'un plugin ou une vue voit de l'application (des accesseurs) ;
// - `AppCoeur` l'implémente en assemblant le routeur clavier, les commandes, les registres
//   (vues par type, extensions), les documents et le Workspace, et tient les deux câblages que
//   Fragment faisait dans le constructeur du Workspace : un registre des vues qui change
//   reconstruit les remplaçants ; un document renommé recale les historiques ← →.
//
// ★ Le Workspace est dans un `Rc<RefCell<…>>` : les commandes (des fermetures rangées dans le
//   CommandManager) l'atteignent par un lien faible et l'empruntent le temps d'agir. Un
//   emprunt déjà pris (une commande lancée depuis un écouteur du Workspace) rend la commande
//   indisponible au lieu de paniquer.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::command::{CommandHost, CommandManager, EditorContext};
use crate::documents::{DocumentEvent, DocumentEventType, DocumentManager};
use crate::events::EventRef;
use crate::keymap::hotkey_manager::{HotkeyManager, HotkeyStore};
use crate::keymap::{InputEvent, Keymap, Scope};
use crate::plugin::PluginHost;
use crate::registry::Registry;
use crate::registry_hub::RegistryHub;
use crate::workspace::espace::{ContexteApp, Workspace};
use crate::workspace::vue::ViewCreator;

/// Ce qu'un plugin ou une vue voit de l'application.
pub trait App {
    /// Le routeur clavier (`app.keymap`).
    fn keymap(&self) -> &Keymap;
    /// La portée racine (`app.scope`) : celle des raccourcis globaux.
    fn scope(&self) -> Scope {
        self.keymap().get_root_scope()
    }
    /// Les commandes (`app.commands`).
    fn commands(&self) -> Rc<CommandManager>;
    /// Les fabriques de vues par type (`app.viewRegistry`).
    fn view_registry(&self) -> &Registry<ViewCreator>;
    /// Extension de fichier → type de vue (`app.extensionRegistry`, `"md"` → `"markdown"`).
    fn extension_registry(&self) -> &Registry<String>;
    /// Les documents par schéma (`app.documents`).
    fn documents(&self) -> &Rc<DocumentManager>;
    /// Le Workspace (`app.workspace`).
    fn workspace(&self) -> &Rc<RefCell<Workspace>>;
}

/// L'application du cœur : l'assemblage headless, sans fenêtre.
pub struct AppCoeur {
    keymap: Keymap,
    commands: Rc<CommandManager>,
    hotkey_manager: RefCell<Option<Rc<HotkeyManager>>>,
    view_registry: Registry<ViewCreator>,
    extension_registry: Registry<String>,
    documents: Rc<DocumentManager>,
    workspace: Rc<RefCell<Workspace>>,
    registry: RegistryHub,
    last_event: RefCell<Option<InputEvent>>,
    /// Les câblages vers le Workspace, vivants aussi longtemps que l'App.
    abonnements: RefCell<Vec<EventRef>>,
}

impl std::fmt::Debug for AppCoeur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppCoeur")
            .field("commands", &self.commands)
            .field("workspace", &self.workspace)
            .finish()
    }
}

impl AppCoeur {
    /// Une application neuve : routeur clavier, commandes, registres vides, Workspace vide
    /// (le layout n'est pas chargé : `workspace.borrow_mut().load_layout()`).
    pub fn new() -> Rc<Self> {
        let app = Rc::new_cyclic(|faible: &Weak<AppCoeur>| {
            let hote: Weak<dyn CommandHost> = faible.clone();
            let keymap = Keymap::new();
            let view_registry: Registry<ViewCreator> =
                Registry::new("views", "Fabriques de vues par type");
            let documents = Rc::new(DocumentManager::new());
            let workspace = Rc::new(RefCell::new(Workspace::new(ContexteApp {
                keymap: keymap.clone(),
                views: view_registry.clone(),
                documents: documents.clone(),
            })));
            AppCoeur {
                keymap,
                commands: CommandManager::new(hote),
                hotkey_manager: RefCell::new(None),
                view_registry,
                extension_registry: Registry::new("extensions", "Type de vue par extension"),
                documents,
                workspace,
                registry: RegistryHub::new(),
                last_event: RefCell::new(None),
                abonnements: RefCell::new(Vec::new()),
            }
        });
        app.cabler();
        app
    }

    /// Les deux câblages du constructeur du Workspace de Fragment.
    fn cabler(&self) {
        let ws = Rc::downgrade(&self.workspace);
        // Une vue tardive (plugin chargé après la restauration) : les remplaçants la retentent.
        let r1 = self.view_registry.on("changed", move |_| {
            if let Some(ws) = ws.upgrade()
                && let Ok(mut w) = ws.try_borrow_mut()
            {
                w.rebuild_placeholders();
            }
        });
        let ws = Rc::downgrade(&self.workspace);
        // Un document renommé : les historiques ← → de toutes les feuilles suivent sa clé.
        let r2 = self.documents.on(DocumentEventType::Rename, move |e| {
            if let DocumentEvent::Rename { key, old_key, .. } = e
                && let Some(ws) = ws.upgrade()
                && let Ok(mut w) = ws.try_borrow_mut()
            {
                w.rename_document(old_key, key);
            }
        });
        self.abonnements.borrow_mut().extend([r1, r2]);
    }

    /// Construit le gestionnaire de raccourcis sur la portée racine (après les commandes,
    /// comme dans Fragment), avec son stockage.
    pub fn installer_raccourcis(&self, store: Rc<dyn HotkeyStore>) -> Rc<HotkeyManager> {
        let hm = HotkeyManager::new(self.keymap.root_scope(), self.commands.clone(), store);
        *self.hotkey_manager.borrow_mut() = Some(hm.clone());
        hm
    }

    /// La frappe ou le clic qui a déclenché la dernière commande (`app.lastEvent`).
    pub fn last_event(&self) -> Option<InputEvent> {
        self.last_event.borrow().clone()
    }
}

impl App for AppCoeur {
    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn commands(&self) -> Rc<CommandManager> {
        self.commands.clone()
    }

    fn view_registry(&self) -> &Registry<ViewCreator> {
        &self.view_registry
    }

    fn extension_registry(&self) -> &Registry<String> {
        &self.extension_registry
    }

    fn documents(&self) -> &Rc<DocumentManager> {
        &self.documents
    }

    fn workspace(&self) -> &Rc<RefCell<Workspace>> {
        &self.workspace
    }
}

impl CommandHost for AppCoeur {
    fn set_last_event(&self, evt: Option<InputEvent>) {
        *self.last_event.borrow_mut() = evt;
    }

    /// Pas d'éditeur de texte dans le cœur de T1 (les vues textuelles viennent avec T3).
    fn active_editor(&self) -> Option<EditorContext> {
        None
    }

    fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>> {
        self.hotkey_manager.borrow().clone()
    }
}

impl PluginHost for AppCoeur {
    fn commands(&self) -> Rc<CommandManager> {
        self.commands.clone()
    }

    fn registry(&self) -> RegistryHub {
        self.registry.clone()
    }
}
