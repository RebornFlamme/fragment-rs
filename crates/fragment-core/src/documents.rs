// Port de app/src/ui/core/documents/DocumentManager.ts et documents/types.ts
//
// `app.documents` — les fournisseurs par schéma + le flux d'événements.
//
// Un document est ce qu'on ouvre, navigue, annote, lie : une note, un PDF, une
// conversation… Son ADRESSE est une clé (`DocumentKey`) ; son STOCKAGE ne regarde que son
// fournisseur. Le cœur n'y connaît AUCUN schéma par son nom : le coffre s'inscrit comme
// les autres (schéma vide, réservé au cœur).
//
// ★ SOURCE UNIQUE de `DocumentEvent` / `EventOrigin` : fragment-artefacts en porte des
//   copies marquées ★ (à dédoublonner, voir la PR fr-p1-core-b).

use std::rc::Rc;

use thiserror::Error;

use crate::document_key::{DocumentKey, parse_document_key};
use crate::events::{EventRef, Events};
use crate::registry::{Disposer, Registry};
use crate::workspace::ViewState;

/// Ce qu'un plugin fournit pour SON schéma.
pub trait DocumentProvider {
    /// Le schéma, sans `:` (`chat`). Le schéma vide est réservé au coffre.
    fn scheme(&self) -> &str;
    /// L'état de vue qui montre ce document (`None` : rien ne sait le montrer).
    fn open(&self, key: &str) -> Option<ViewState>;
    /// La clé que montre cet état de vue, si elle est de ce fournisseur.
    fn key_of(&self, _state: &ViewState) -> Option<DocumentKey> {
        None
    }
    /// Le nom humain (titre d'onglet restauré, historique, prompts).
    fn title(&self, key: &str) -> String;
    /// Un nom d'icône (sinon celui par défaut).
    fn icon(&self, _key: &str) -> Option<String> {
        None
    }
    /// Existe-t-il encore ? (une entrée d'historique morte est sautée)
    fn exists(&self, key: &str) -> bool;
}

/// `user` = geste dans l'app, `disk` = constaté de dehors (règle du Vault).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOrigin {
    User,
    Disk,
}

/// Ce qui arrive à un document. `folder` : la clé est un PRÉFIXE (dossier du coffre) —
/// tout ce qui est sous `<clé>/` est concerné aussi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentEvent {
    Create {
        key: DocumentKey,
    },
    Modify {
        key: DocumentKey,
    },
    Rename {
        key: DocumentKey,
        old_key: DocumentKey,
        origin: Option<EventOrigin>,
        folder: bool,
    },
    Delete {
        key: DocumentKey,
        origin: Option<EventOrigin>,
        folder: bool,
    },
}

/// Le `type` du TS, pour s'abonner à un genre d'événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentEventType {
    Create,
    Modify,
    Rename,
    Delete,
}

impl DocumentEventType {
    /// Le nom d'événement du TS (`create`, `modify`, `rename`, `delete`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Modify => "modify",
            Self::Rename => "rename",
            Self::Delete => "delete",
        }
    }
}

impl DocumentEvent {
    pub fn event_type(&self) -> DocumentEventType {
        match self {
            Self::Create { .. } => DocumentEventType::Create,
            Self::Modify { .. } => DocumentEventType::Modify,
            Self::Rename { .. } => DocumentEventType::Rename,
            Self::Delete { .. } => DocumentEventType::Delete,
        }
    }

    pub fn key(&self) -> &DocumentKey {
        match self {
            Self::Create { key }
            | Self::Modify { key }
            | Self::Rename { key, .. }
            | Self::Delete { key, .. } => key,
        }
    }
}

/// Les `throw` de `register`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DocumentError {
    #[error("[documents] schéma invalide « {0} » (ni « : », ni « / », ni espace)")]
    SchemaInvalide(String),
    #[error("[documents] le schéma vide est celui du coffre")]
    SchemaVideReserve,
}

/// Le propriétaire du cœur, seul autorisé au schéma vide.
pub const OWNER_CORE: &str = "core";

pub struct DocumentManager {
    /// Les fournisseurs, par schéma. Se lit au moment de l'usage, jamais copié.
    pub providers: Registry<Rc<dyn DocumentProvider>>,
    events: Events,
}

impl Default for DocumentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentManager {
    pub fn new() -> Self {
        Self {
            providers: Registry::new("documentProviders", "Fournisseurs de documents par schéma"),
            events: Events::new(),
        }
    }

    /// Inscrit un fournisseur ; rend son retrait. Le schéma vide est réservé au cœur.
    pub fn register(
        &self,
        provider: Rc<dyn DocumentProvider>,
        owner: &str,
    ) -> Result<Disposer, DocumentError> {
        let s = provider.scheme().to_owned();
        if s.chars().any(|c| c == ':' || c == '/' || c.is_whitespace()) {
            return Err(DocumentError::SchemaInvalide(s));
        }
        if s.is_empty() && owner != OWNER_CORE {
            return Err(DocumentError::SchemaVideReserve);
        }
        Ok(self.providers.register(&s, provider, owner))
    }

    pub fn provider(&self, key: &str) -> Option<Rc<dyn DocumentProvider>> {
        self.providers.get(parse_document_key(key).scheme)
    }

    /// L'état de vue qui montre ce document, ou `None`.
    pub fn view_state_for(&self, key: &str) -> Option<ViewState> {
        self.provider(key)?.open(key)
    }

    /// La clé que montre un état de vue (le premier fournisseur qui la reconnaît).
    pub fn key_of_state(&self, state: Option<&ViewState>) -> Option<DocumentKey> {
        let state = state?;
        self.providers
            .entries()
            .into_iter()
            .find_map(|e| e.value.key_of(state).filter(|k| !k.is_empty()))
    }

    pub fn title(&self, key: &str) -> String {
        self.provider(key)
            .map(|p| p.title(key))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| parse_document_key(key).rest.to_owned())
    }

    pub fn icon(&self, key: &str) -> Option<String> {
        self.provider(key)?.icon(key)
    }

    /// Faux sans fournisseur : un document que personne ne sait montrer n'existe pas (encore).
    pub fn exists(&self, key: &str) -> bool {
        self.provider(key).is_some_and(|p| p.exists(key))
    }

    /// Annonce un événement de document (un fournisseur, pour SES clés).
    pub fn notify(&self, e: &DocumentEvent) {
        if self.provider(e.key()).is_none() {
            log::warn!(
                "[documents] événement « {} » sur « {} » : aucun fournisseur pour ce schéma",
                e.event_type().name(),
                e.key()
            );
            return;
        }
        self.events.trigger(e.event_type().name(), e);
    }

    /// S'abonne à un genre d'événement (le `on` typé du TS).
    pub fn on<F>(&self, kind: DocumentEventType, cb: F) -> EventRef
    where
        F: Fn(&DocumentEvent) + 'static,
    {
        self.events.on(kind.name(), move |d| {
            if let Some(e) = d.downcast_ref::<DocumentEvent>() {
                cb(e);
            }
        })
    }

    /// Le flux brut, pour `register_event` d'un composant.
    pub fn events(&self) -> &Events {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Chat {
        scheme: String,
    }

    fn chat() -> Rc<dyn DocumentProvider> {
        Rc::new(Chat {
            scheme: "chat".into(),
        })
    }

    fn avec_schema(s: &str) -> Rc<dyn DocumentProvider> {
        Rc::new(Chat { scheme: s.into() })
    }

    impl DocumentProvider for Chat {
        fn scheme(&self) -> &str {
            &self.scheme
        }
        fn open(&self, k: &str) -> Option<ViewState> {
            Some(ViewState::with_state(
                "chat",
                serde_json::json!({"id": &k[5..]}),
            ))
        }
        fn key_of(&self, vs: &ViewState) -> Option<DocumentKey> {
            if vs.type_ != "chat" {
                return None;
            }
            let id = vs.state.as_ref()?.get("id")?.as_str()?;
            Some(DocumentKey::new(format!("chat:{id}")))
        }
        fn title(&self, k: &str) -> String {
            format!("Conv {}", &k[5..])
        }
        fn exists(&self, k: &str) -> bool {
            k != "chat:mort"
        }
    }

    #[test]
    fn route_par_schema_vue_titre_existence_cle_dun_etat() {
        let d = DocumentManager::new();
        let _ = d.register(chat(), "p").unwrap();
        assert_eq!(
            d.view_state_for("chat:1"),
            Some(ViewState::with_state(
                "chat",
                serde_json::json!({"id": "1"})
            ))
        );
        assert_eq!(d.title("chat:1"), "Conv 1");
        assert!(!d.exists("chat:mort"));
        let vs = ViewState::with_state("chat", serde_json::json!({"id": "9"}));
        assert_eq!(d.key_of_state(Some(&vs)).unwrap(), "chat:9");
        assert_eq!(d.key_of_state(Some(&ViewState::new("autre"))), None);
    }

    #[test]
    fn sans_fournisseur_titre_reste_nexiste_pas_evenement_ignore() {
        let d = DocumentManager::new();
        let appels = Rc::new(Cell::new(0));
        let a = appels.clone();
        d.on(DocumentEventType::Delete, move |_| a.set(a.get() + 1));
        d.notify(&DocumentEvent::Delete {
            key: "x:1".into(),
            origin: Some(EventOrigin::User),
            folder: false,
        });
        assert_eq!(appels.get(), 0);
        assert_eq!(d.title("x:1"), "1");
        assert!(!d.exists("x:1"));
    }

    #[test]
    fn le_schema_vide_est_reserve_au_coeur_schemas_invalides_refuses() {
        let d = DocumentManager::new();
        assert!(d.register(avec_schema(""), "plugin").is_err());
        assert!(d.register(avec_schema("a:b"), "plugin").is_err());
        assert!(d.register(avec_schema(""), "core").is_ok());
    }

    #[test]
    fn retrait_le_fournisseur_disparait() {
        let d = DocumentManager::new();
        let off = d.register(chat(), "p").unwrap();
        off();
        assert_eq!(d.view_state_for("chat:1"), None);
    }

    #[test]
    fn notify_relaie_aux_abonnes() {
        let d = DocumentManager::new();
        let _ = d.register(chat(), "p").unwrap();
        let vu: Rc<RefCell<Vec<DocumentEvent>>> = Rc::default();
        let v = vu.clone();
        d.on(DocumentEventType::Rename, move |e| {
            v.borrow_mut().push(e.clone())
        });
        d.notify(&DocumentEvent::Rename {
            key: "chat:2".into(),
            old_key: "chat:1".into(),
            origin: Some(EventOrigin::User),
            folder: false,
        });
        let vu = vu.borrow();
        assert_eq!(vu.len(), 1);
        assert_eq!(vu[0].key().as_str(), "chat:2");
        assert!(
            matches!(&vu[0], DocumentEvent::Rename { old_key, .. } if old_key.as_str() == "chat:1")
        );
    }

    // ─── Ajoutés ───
    #[test]
    fn un_abonne_ne_recoit_que_son_genre() {
        let d = DocumentManager::new();
        let _ = d.register(chat(), "p").unwrap();
        let n = Rc::new(Cell::new(0));
        let c = n.clone();
        d.on(DocumentEventType::Create, move |_| c.set(c.get() + 1));
        d.notify(&DocumentEvent::Modify {
            key: "chat:1".into(),
        });
        d.notify(&DocumentEvent::Create {
            key: "chat:1".into(),
        });
        assert_eq!(n.get(), 1);
    }
}
