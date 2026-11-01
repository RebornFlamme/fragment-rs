// Port de app/src/ui/views/chat/api.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le contrat du plugin cœur `chat` — LES CHATS de Fragment, indépendants de
//  qui répond. Deux portes, toutes deux des registres ouverts :
//
//    - un FOURNISSEUR (`chat:provider@1`) donne les conversations : participants,
//      messages (relus à la demande), envoi en flux ;
//    - des SLOTS (`chat:*@1`, ordonnés) : un plugin « s'empare » de la vue et
//      ajoute des choses AUTOUR (docs/contrats.md).
//
//  ★ Écarts assumés (voir la PR) :
//    - les `Promise` du TS deviennent des appels SYNCHRONES (les modèles sont purs,
//      comme `fragment-agent::contrat`) ; le flux d'un tour passe par un rappel
//      `&mut dyn FnMut(ChatEvent)` ;
//    - le `render(el, …)` DOM des slots n'a pas d'équivalent (docs/contrats.md, point 8) :
//      ici `id` / `align` / `appliesTo`, le rendu est dans fragment-ui ;
//    - `ChatSlotContext` perd `app` et `view` (objets de la vue) : il garde ce qui est
//      du modèle (fournisseur, conversation, mode, message, flux).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::chat_surface_model::MESSAGE_STRIDE;

/// Un participant d'une conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub name: String,
    /// Les initiales de l'avatar ; déduites du nom si absentes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initials: Option<String>,
    pub role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Un morceau de message : du texte, ou un type que rend une contribution `chat:part@1`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatPart {
    /// Le type du part (`type` en TS) : « text », « attachment », « agent-trace »…
    #[serde(rename = "type")]
    pub kind: String,
    /// Un part émis de nouveau avec le même id REMPLACE l'ancien (un appel d'outil qui reçoit son résultat).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ChatPart {
    /// Un part de texte.
    pub fn text(texte: impl Into<String>) -> Self {
        Self {
            kind: "text".into(),
            text: Some(texte.into()),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    /// L'id d'un [`Participant`].
    pub author: String,
    pub parts: Vec<ChatPart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Quand il a été écrit (ms) — facultatif ; sert à « · 2 h » (présentation `post`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
}

/// Le flux d'un tour, du fournisseur vers la vue.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatEvent {
    /// Un morceau de texte (ajouté au dernier part texte).
    Text(String),
    /// Un part entier (appel d'outil…) ; même id = mise à jour.
    Part(ChatPart),
    /// Le part de cet id quitte le message (un texte devenu autre chose).
    Remove(String),
    Done {
        error: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatConversationInfo {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
}

/// D'où vient une conversation : le DOCUMENT (sa clé) et le passage d'où elle est partie (Ask).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatOrigin {
    /// La clé du document d'origine (chemin du coffre ou `schéma:reste`).
    pub doc: String,
    /// Positions UTF-16 (ADR-004).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<usize>,
    /// Le texte du passage (relocalise des offsets périmés).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
}

/// Une PIÈCE JOINTE envoyée avec un message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatAttachment {
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

/// Les données d'un part `attachment` (ce qu'on MONTRE d'une pièce jointe).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentPartData {
    pub name: String,
    pub mime: String,
    /// Une image à montrer (URL d'objet, data: URL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// RÉPONDRE à une partie de la conversation : une plage de la surface (positions UTF-16, ADR-004)
/// et son TEXTE, ou l'IMAGE d'une zone entourée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatQuote {
    pub from: usize,
    pub to: usize,
    pub text: Option<String>,
    pub image: Option<ChatAttachment>,
}

/// Les données d'un part `quote` (ce qu'on MONTRE d'une citation).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotePartData {
    pub from: usize,
    pub to: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<AttachmentPartData>,
}

/// Une vue qui sait RÉPONDRE à une partie d'elle-même. `can_reply` : faux sans champ, ou si le
/// fournisseur refuse les citations (`without: ["quote"]`).
pub trait ChatReplyTarget {
    fn can_reply(&self) -> bool;
    fn reply(&mut self, q: ChatQuote) -> bool;
}

/// Une erreur de fournisseur (chargement, envoi) : un message à montrer.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ChatError(pub String);

impl ChatError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Les options d'un envoi (`Record<string, unknown>` en TS).
pub type SendOpts = Map<String, Value>;

/// Un FOURNISSEUR de conversations (`chat:provider@1`).
pub trait ChatProvider {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    fn icon(&self) -> Option<&str> {
        None
    }
    /// Les extensions du champ (`chat:composer@1`, par leur id) dont ce fournisseur ne veut PAS
    /// (ex. `["attachments"]`). Vide : toutes celles qui s'appliquent.
    fn without(&self) -> &[String] {
        &[]
    }
    fn participants(&self, conversation: &str) -> Vec<Participant>;
    fn title(&self, conversation: &str) -> String;
    /// Les messages, relus à la demande (le fournisseur garde la source).
    fn load(&self, conversation: &str) -> Result<Vec<ChatMessage>, ChatError>;
    /// Envoie un tour : le flux arrive par `on_event`, l'appel rend à la fin du tour.
    fn send(
        &self,
        conversation: &str,
        text: &str,
        on_event: &mut dyn FnMut(ChatEvent),
        opts: Option<&SendOpts>,
    ) -> Result<(), ChatError>;
    /// Interrompt le tour en cours (`ChatRun.stop` du TS, ici par conversation).
    fn stop(&self, _conversation: &str) {}
    /// Un nouvel id de conversation (vide).
    fn create(&self) -> Option<String> {
        None
    }
    /// Supprime une conversation (ses messages, chez le fournisseur).
    fn delete(&self, _conversation: &str) -> Result<(), ChatError> {
        Ok(())
    }
    /// L'historique, le plus récent d'abord.
    fn list(&self) -> Vec<ChatConversationInfo> {
        Vec::new()
    }
    /// L'origine de la conversation (née d'un passage d'un document), ou `None`.
    fn origin(&self, _conversation: &str) -> Option<ChatOrigin> {
        None
    }
}

/// Une position de la surface d'une conversation → le RANG du message qui la porte.
pub fn chat_message_at(pos: usize) -> usize {
    pos / MESSAGE_STRIDE
}

// ── Les slots ────────────────────────────────────────────────────────────────

/// Les identifiants des registres (docs/contrats.md).
pub const REGISTRY_PROVIDER: &str = "chat:provider@1";
pub const REGISTRY_COMPOSER: &str = "chat:composer@1";
pub const REGISTRY_HEADER_ACTIONS: &str = "chat:header-actions@1";
pub const REGISTRY_MESSAGE_ACTIONS: &str = "chat:message-actions@1";
pub const REGISTRY_COMPOSER_TOOLS: &str = "chat:composer-tools@1";
pub const REGISTRY_FOOTER: &str = "chat:footer@1";
pub const REGISTRY_PART: &str = "chat:part@1";
pub const REGISTRY_TEXT: &str = "chat:text@1";

/// Où vit la vue : le dock, la zone centrale, un widget ancré, EMBARQUÉE, ou une BARRE
/// (le champ seul).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatMode {
    Panel,
    Page,
    Anchored,
    Embedded,
    Bar,
}

/// Événements (bulle) de la vue en mode `bar`.
pub const CHAT_BAR_ESCAPE_EVENT: &str = "chat-bar-escape";
pub const CHAT_BAR_SUBMIT_EVENT: &str = "chat-bar-submit";

/// Ce que porte `CHAT_BAR_SUBMIT_EVENT` : ce que l'envoi aurait porté.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatBarSubmit {
    pub text: String,
    pub opts: Option<SendOpts>,
    pub parts: Vec<ChatPart>,
}

/// Ce que reçoit une contribution (la part « modèle » ; voir l'écart en tête de fichier).
#[derive(Clone, Debug, PartialEq)]
pub struct ChatSlotContext {
    pub provider: String,
    pub conversation: String,
    pub mode: ChatMode,
    /// Le message (actions de message, rendu de part et de texte).
    pub message: Option<ChatMessage>,
    /// Le tour en cours ? (rendu du texte pendant le flux)
    pub streaming: bool,
}

/// Où se place un outil du champ (`chat:composer-tools@1`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlotAlign {
    /// Au DÉBUT de la barre du champ (défaut).
    #[default]
    Start,
    /// À sa FIN, à côté du bouton d'envoi.
    End,
}

type AppliesTo = Box<dyn Fn(&ChatSlotContext) -> bool>;

/// Une contribution d'emplacement. Le rendu (`render(el, ctx)`) est dans fragment-ui.
pub struct ChatSlot {
    pub id: String,
    pub align: Option<SlotAlign>,
    applies_to: Option<AppliesTo>,
}

impl ChatSlot {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            align: None,
            applies_to: None,
        }
    }

    pub fn with_align(mut self, align: SlotAlign) -> Self {
        self.align = Some(align);
        self
    }

    pub fn with_applies_to(mut self, f: impl Fn(&ChatSlotContext) -> bool + 'static) -> Self {
        self.applies_to = Some(Box::new(f));
        self
    }

    /// `appliesTo?.(ctx) ?? true`.
    pub fn applies(&self, ctx: &ChatSlotContext) -> bool {
        self.applies_to.as_ref().is_none_or(|f| f(ctx))
    }
}

/// Le rendu d'un type de part (`chat:part@1`) : côté modèle, le type qu'il prend en charge.
/// Le `render(el, part, ctx)` et la poignée (`update`/`destroy`) sont dans fragment-ui.
pub trait ChatPartRenderer {
    fn part_type(&self) -> &str;
}

/// Un rendu du TEXTE d'un message (`chat:text@1`) ; le premier qui s'applique gagne.
/// Le `render(text, ctx)` est dans fragment-ui.
pub trait ChatTextRenderer {
    fn id(&self) -> &str;
    fn applies_to(&self, _ctx: &ChatSlotContext) -> bool {
        true
    }
}

// ── Le COMPORTEMENT du champ : `chat:composer@1` ───────────────────────────────

/// Ce qu'une extension ajoute à l'envoi.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComposerTake {
    /// Des options pour `ChatProvider::send` (fusionnées).
    pub opts: Option<SendOpts>,
    /// Des parts ajoutés au message de l'utilisateur (APRÈS le texte).
    pub parts: Vec<ChatPart>,
}

/// Ce qu'une extension du champ rend : tout est facultatif.
pub trait ChatComposerHandle {
    /// Quelque chose est prêt à partir : on peut envoyer sans texte.
    fn pending(&self) -> bool {
        false
    }
    /// À l'envoi.
    fn take(&mut self) -> Option<ComposerTake> {
        None
    }
    /// Son état, gardé quand la vue reconstruit le champ…
    fn save(&self) -> Option<Value> {
        None
    }
    /// …et rendu au nouveau champ.
    fn restore(&mut self, _state: Value) {}
}

/// Une extension du champ (`chat:composer@1`, par id ; à id égal, la plus haute priorité gagne).
/// ★ `create` ne reçoit pas la `ChatComposerAPI` (el, textarea…) : c'est de l'interface.
pub trait ChatComposerExtension {
    fn id(&self) -> &str;
    fn applies_to(&self, _ctx: &ChatSlotContext) -> bool {
        true
    }
    fn create(&self, ctx: &ChatSlotContext) -> Box<dyn ChatComposerHandle>;
}

/// L'état d'une vue `chat` (persisté dans le layout ou un widget).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatViewState {
    pub provider: String,
    pub conversation: String,
}

pub const CHAT_VIEW_TYPE: &str = "chat";

#[cfg(test)]
mod tests {
    use super::*;

    // Test AJOUTÉ par le port (api.ts n'a pas de test TS) : le rang du message d'une position.
    #[test]
    fn chat_message_at_rang_du_message() {
        assert_eq!(chat_message_at(0), 0);
        assert_eq!(chat_message_at(MESSAGE_STRIDE - 1), 0);
        assert_eq!(chat_message_at(3 * MESSAGE_STRIDE + 7), 3);
    }

    // Test AJOUTÉ : un part se sérialise avec la clé `type`, comme en TS (transcript, layout).
    #[test]
    fn part_serialise_avec_la_cle_type() {
        let p = ChatPart::text("a");
        let v = serde_json::to_value(&p).unwrap_or_default();
        assert_eq!(v, serde_json::json!({ "type": "text", "text": "a" }));
    }
}
