//! Le fournisseur de TEST de la vue chat : des conversations en mémoire et une réponse scriptée.
//!
//! ★ Réécrit, jamais traduit du double d'e2e de Fragment : il sert aux tests headless de la vue
//!   et à la commande de scène `pixels:chat` (conversations de fixture). La réponse est
//!   déterministe : elle cite la question et arrive en deux morceaux de texte.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use fragment_chat::api::{
    ChatConversationInfo, ChatError, ChatEvent, ChatMessage, ChatOrigin, ChatPart, ChatProvider,
    Participant, Role, SendOpts,
};

/// L'id du fournisseur de test.
pub const ID_FOURNISSEUR_TEST: &str = "e2e";

#[derive(Default)]
struct Conversation {
    messages: Vec<ChatMessage>,
    modifiee: i64,
    origine: Option<ChatOrigin>,
    titre: Option<String>,
}

/// Des conversations en mémoire.
pub struct FournisseurDeTest {
    conversations: RefCell<HashMap<String, Conversation>>,
    prochain: Cell<usize>,
    horloge: Cell<i64>,
    /// Combien de fois `load` a été appelé (le test vérifie qu'on relit chez le fournisseur).
    chargements: Cell<usize>,
    nom_utilisateur: String,
}

impl FournisseurDeTest {
    /// Un fournisseur vide ; `nom` : celui de l'utilisateur (la salutation de l'accueil).
    #[must_use]
    pub fn new(nom: &str) -> Self {
        Self::avec_horloge(nom, 1_000)
    }

    /// Comme [`Self::new`], avec l'instant (ms) que lisent les conversations écrites : l'historique
    /// range par jour (« Aujourd'hui ») d'après lui.
    #[must_use]
    pub fn avec_horloge(nom: &str, base_ms: i64) -> Self {
        Self {
            conversations: RefCell::new(HashMap::new()),
            prochain: Cell::new(1),
            horloge: Cell::new(base_ms),
            chargements: Cell::new(0),
            nom_utilisateur: nom.to_owned(),
        }
    }

    fn tic(&self) -> i64 {
        self.horloge.set(self.horloge.get() + 1);
        self.horloge.get()
    }

    /// Pose une conversation d'avance : `(auteur, texte)` par message, `modifiee` en ms.
    pub fn poser(&self, id: &str, titre: Option<&str>, tours: &[(Role, &str)], modifiee: i64) {
        let messages = tours
            .iter()
            .enumerate()
            .map(|(i, (role, texte))| ChatMessage {
                id: format!("{id}-m{}", i + 1),
                author: auteur(*role).to_owned(),
                parts: vec![ChatPart::text(*texte)],
                ..ChatMessage::default()
            })
            .collect();
        self.conversations.borrow_mut().insert(
            id.to_owned(),
            Conversation {
                messages,
                modifiee,
                origine: None,
                titre: titre.map(str::to_owned),
            },
        );
    }

    /// Dit d'où vient une conversation (le passage d'un document).
    pub fn poser_origine(&self, id: &str, origine: ChatOrigin) {
        if let Some(c) = self.conversations.borrow_mut().get_mut(id) {
            c.origine = Some(origine);
        }
    }

    /// Combien de fois la vue a relu une conversation chez le fournisseur.
    #[must_use]
    pub fn chargements(&self) -> usize {
        self.chargements.get()
    }

    /// Combien de conversations existent.
    #[must_use]
    pub fn nombre(&self) -> usize {
        self.conversations.borrow().len()
    }
}

fn auteur(role: Role) -> &'static str {
    match role {
        Role::User => "julien",
        Role::Assistant => "claude",
    }
}

fn texte_de(m: &ChatMessage) -> String {
    m.parts
        .iter()
        .filter_map(|p| p.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl ChatProvider for FournisseurDeTest {
    fn id(&self) -> &str {
        ID_FOURNISSEUR_TEST
    }

    fn label(&self) -> &str {
        "Test"
    }

    fn icon(&self) -> Option<&str> {
        Some("sparkles")
    }

    fn participants(&self, _conversation: &str) -> Vec<Participant> {
        vec![
            Participant {
                id: "julien".into(),
                name: self.nom_utilisateur.clone(),
                initials: None,
                role: Role::User,
            },
            Participant {
                id: "claude".into(),
                name: "Claude".into(),
                initials: None,
                role: Role::Assistant,
            },
        ]
    }

    /// Le titre : celui qui est posé, sinon la première question (40 caractères), sinon vide.
    fn title(&self, conversation: &str) -> String {
        let c = self.conversations.borrow();
        let Some(c) = c.get(conversation) else {
            return String::new();
        };
        if let Some(t) = &c.titre {
            return t.clone();
        }
        c.messages
            .iter()
            .find(|m| m.author == "julien")
            .map(|m| texte_de(m).chars().take(40).collect())
            .unwrap_or_default()
    }

    fn load(&self, conversation: &str) -> Result<Vec<ChatMessage>, ChatError> {
        self.chargements.set(self.chargements.get() + 1);
        Ok(self
            .conversations
            .borrow()
            .get(conversation)
            .map(|c| c.messages.clone())
            .unwrap_or_default())
    }

    fn send(
        &self,
        conversation: &str,
        text: &str,
        on_event: &mut dyn FnMut(ChatEvent),
        _opts: Option<&SendOpts>,
    ) -> Result<(), ChatError> {
        let t = self.tic();
        let reponse = format!("Réponse à « {text} ».");
        let (a, b) = reponse.split_at(reponse.len() / 2);
        let (a, b) = if reponse.is_char_boundary(reponse.len() / 2) {
            (a.to_owned(), b.to_owned())
        } else {
            (reponse.clone(), String::new())
        };
        on_event(ChatEvent::Text(a));
        on_event(ChatEvent::Text(b));
        let mut c = self.conversations.borrow_mut();
        let conv = c.entry(conversation.to_owned()).or_default();
        let n = conv.messages.len();
        conv.messages.push(ChatMessage {
            id: format!("{conversation}-m{}", n + 1),
            author: "julien".into(),
            parts: vec![ChatPart::text(text)],
            ..ChatMessage::default()
        });
        conv.messages.push(ChatMessage {
            id: format!("{conversation}-m{}", n + 2),
            author: "claude".into(),
            parts: vec![ChatPart::text(reponse)],
            ..ChatMessage::default()
        });
        conv.modifiee = t;
        drop(c);
        on_event(ChatEvent::Done { error: None });
        Ok(())
    }

    /// Un id neuf : rien n'est écrit avant le premier envoi.
    fn create(&self) -> Option<String> {
        let n = self.prochain.get();
        self.prochain.set(n + 1);
        Some(format!("neuve-{n}"))
    }

    fn delete(&self, conversation: &str) -> Result<(), ChatError> {
        self.conversations.borrow_mut().remove(conversation);
        Ok(())
    }

    fn list(&self) -> Vec<ChatConversationInfo> {
        let mut v: Vec<ChatConversationInfo> = self
            .conversations
            .borrow()
            .iter()
            .filter(|(_, c)| !c.messages.is_empty())
            .map(|(id, c)| ChatConversationInfo {
                id: id.clone(),
                title: self.title(id),
                updated_at: c.modifiee,
            })
            .collect();
        v.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
        v
    }

    fn origin(&self, conversation: &str) -> Option<ChatOrigin> {
        self.conversations
            .borrow()
            .get(conversation)
            .and_then(|c| c.origine.clone())
    }
}
