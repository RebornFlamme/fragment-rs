// Port de app/src/ui/core/documents/vaultProvider.ts
//! Le coffre comme fournisseur de documents (schéma vide), et le relais de ses événements
//! vers le flux de `DocumentManager` — par la même API qu'un plugin.

use std::rc::Rc;

use serde_json::json;

use super::coffre::{Vault, VaultEvent, VaultEventType};
use super::fichier::TAbstractFile;
use crate::document_key::DocumentKey;
use crate::documents::{DocumentEvent, DocumentManager, DocumentProvider};
use crate::events::EventRef;
use crate::workspace::ViewState;

/// L'extension d'une clé : le dernier segment, après son dernier point (un point en tête
/// n'en est pas un).
fn extension_de(key: &str) -> &str {
    let nom = key.rsplit('/').next().unwrap_or(key);
    match nom.rfind('.') {
        Some(i) if i > 0 => &nom[i + 1..],
        _ => "",
    }
}

/// `app.extensionRegistry.get` : extension → type de vue.
pub type ExtensionLookup = Rc<dyn Fn(&str) -> Option<String>>;

/// Le fournisseur des chemins du coffre (schéma vide).
pub struct VaultDocumentProvider {
    vault: Rc<Vault>,
    /// `app.extensionRegistry.get(ext)` : le type de vue d'une extension.
    extensions: ExtensionLookup,
    /// La vue d'attente d'une extension que personne ne sait (encore) ouvrir. Passée par
    /// l'App, pour ne pas tirer le workspace dans ce module.
    empty_view_type: String,
}

impl VaultDocumentProvider {
    pub fn new(
        vault: Rc<Vault>,
        extensions: ExtensionLookup,
        empty_view_type: impl Into<String>,
    ) -> Self {
        Self {
            vault,
            extensions,
            empty_view_type: empty_view_type.into(),
        }
    }
}

impl DocumentProvider for VaultDocumentProvider {
    fn scheme(&self) -> &str {
        ""
    }

    fn open(&self, key: &str) -> Option<ViewState> {
        let ext = extension_de(key);
        let type_ = (self.extensions)(ext).unwrap_or_else(|| self.empty_view_type.clone());
        Some(ViewState::with_state(
            type_,
            json!({ "file": key, "extension": ext }),
        ))
    }

    fn key_of(&self, state: &ViewState) -> Option<DocumentKey> {
        let file = state.state.as_ref()?.get("file")?.as_str()?;
        (!file.is_empty()).then(|| DocumentKey::new(file))
    }

    fn title(&self, key: &str) -> String {
        let nom = key.rsplit('/').next().unwrap_or(key);
        match nom.rfind('.') {
            Some(i) if i > 0 => nom[..i].to_owned(),
            _ => nom.to_owned(),
        }
    }

    fn icon(&self, _key: &str) -> Option<String> {
        Some("file-text".to_owned())
    }

    fn exists(&self, key: &str) -> bool {
        self.vault.get_file_by_path(key).is_some()
    }
}

/// Relaie les événements du coffre dans le flux des documents. Rend les abonnements (à
/// retirer par `off()` si le coffre change).
///
/// `create` et `modify` ne concernent que les fichiers ; `delete` et `rename` portent leur
/// origine et un drapeau `folder` (un dossier est un PRÉFIXE : tout ce qui est dessous
/// est concerné).
pub fn relay_vault_events(documents: &Rc<DocumentManager>, vault: &Vault) -> Vec<EventRef> {
    let mut refs = Vec::new();

    let d = Rc::clone(documents);
    refs.push(vault.on(VaultEventType::Rename, move |e| {
        if let VaultEvent::Rename {
            file,
            old_path,
            info,
        } = e
        {
            d.notify(&DocumentEvent::Rename {
                key: DocumentKey::new(file.path()),
                old_key: DocumentKey::new(old_path.as_str()),
                origin: Some(info.origin),
                folder: file.is_folder(),
            });
        }
    }));

    let d = Rc::clone(documents);
    refs.push(vault.on(VaultEventType::Delete, move |e| {
        if let VaultEvent::Delete { file, info } = e {
            d.notify(&DocumentEvent::Delete {
                key: DocumentKey::new(file.path()),
                origin: Some(info.origin),
                folder: file.is_folder(),
            });
        }
    }));

    let d = Rc::clone(documents);
    refs.push(vault.on(VaultEventType::Create, move |e| {
        if let VaultEvent::Create {
            file: TAbstractFile::File(f),
        } = e
        {
            d.notify(&DocumentEvent::Create {
                key: DocumentKey::new(f.path.as_str()),
            });
        }
    }));

    let d = Rc::clone(documents);
    refs.push(vault.on(VaultEventType::Modify, move |e| {
        if let VaultEvent::Modify { file } = e {
            d.notify(&DocumentEvent::Modify {
                key: DocumentKey::new(file.path.as_str()),
            });
        }
    }));

    refs
}
