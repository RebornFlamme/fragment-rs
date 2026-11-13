// Port de app/src/ui/views/agent/AgentPlugin.ts (`openChat`, `openAnchored`) et de la part
// `AgentAnchoredOpen` / `AgentChatOptions` de api.ts
//
// Ce que le service décide quand on lui demande une conversation, côté DONNÉES : quelle
// conversation montrer, laquelle reprendre, de quel parent naît une branche, quel moteur elle
// hérite, ce qu'on écrit dans la session. Le dock, la feuille et le widget ne sont pas ici : le
// service les demande à un trait d'hôte, [`HoteVues`], que le montage branche (`fr-t7-montage`).
//
// ★ Écart assumé : les appels sont synchrones (voir `contrat.rs`).

use fragment_artefacts::DurableAnchor;
use fragment_chat::api::ChatAttachment;

use crate::contrat::Entree;

/// Un chat de l'agent déjà affiché dans le dock droit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatOuvert {
    pub conversation: String,
    /// Un tour est en cours dans cette vue : une demande de conversation neuve ne la prend pas.
    pub occupe: bool,
}

/// « Montre ce chat » : le service a choisi la conversation et la vue à réutiliser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OuvertureChat {
    /// Le rang (dans [`HoteVues::chats_dock`]) du chat à réutiliser ; `None` : une feuille neuve
    /// dans le dock droit.
    pub reutiliser: Option<usize>,
    pub conversation: String,
    /// Un premier message à envoyer tout de suite.
    pub prompt: Option<String>,
    pub effort: Option<String>,
}

/// Ce que le service demande à une conversation ANCRÉE (`AgentAnchoredOpen`).
#[derive(Debug, Clone, Default)]
pub struct OuvertureAncree {
    /// La CLÉ du document hôte : chemin du coffre ou `schéma:reste` (une conversation).
    pub file: String,
    pub from: usize,
    pub to: usize,
    /// Le texte du passage : la conversation garde son origine (relocalisée si le document change).
    pub quote: Option<String>,
    /// Le premier tour (absent : la vue s'ouvre sur son champ).
    pub prompt: Option<String>,
    /// Ce qu'il faut MONTRER de la première question (le prompt peut contenir tout un document).
    pub display: Option<String>,
    /// Sans prompt : ce qui précédera la première question écrite.
    pub preamble: Option<String>,
    pub attachments: Vec<ChatAttachment>,
    /// Le calque qui possède le widget (décoché : masqué).
    pub layer: Option<String>,
    /// Sans prompt : le texte déjà tapé, posé dans le champ.
    pub draft: Option<String>,
    /// Reprendre une session existante au lieu d'en créer une.
    pub session_id: Option<String>,
    pub anchor: Option<DurableAnchor>,
    /// Le widget s'ouvre-t-il déplié ? Défaut : oui.
    pub open: Option<bool>,
    /// Le profil de la conversation, gardé pour chaque suite.
    pub system: Option<String>,
    pub tools: Option<Vec<String>>,
    pub effort: Option<String>,
    pub title: String,
    pub meta: Option<Entree>,
}

/// Le premier tour d'une conversation ancrée, tel que la vue l'envoie à l'ouverture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvoiAncre {
    /// Ce que le fil MONTRE de la question.
    pub texte: String,
    /// Ce qui PART au moteur.
    pub prompt: String,
    pub pieces: Vec<ChatAttachment>,
}

/// L'état dans lequel la vue d'une conversation ancrée s'ouvre.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EtatOuverture {
    pub envoi: Option<EnvoiAncre>,
    pub focus: bool,
    /// Le texte déjà tapé.
    pub brouillon: Option<String>,
    /// Des pièces POSÉES dans le champ (extension `attachments`) : l'utilisateur les voit et les retire.
    pub pieces_du_champ: Vec<ChatAttachment>,
}

/// Le dock et les widgets, vus du service. Branché au montage.
pub trait HoteVues {
    /// Les chats de l'agent affichés dans le dock droit, dans l'ordre des feuilles.
    fn chats_dock(&self) -> Vec<ChatOuvert>;
    /// Montre cette conversation (crée la feuille si `reutiliser` est `None`) et envoie le
    /// premier message s'il y en a un.
    fn montrer_chat(&self, o: OuvertureChat);
    /// Ouvre la conversation `session` dans un widget de doc-widget. Rend faux s'il n'y a pas
    /// d'hôte de widgets (la session n'est alors PAS écrite : pas de session orpheline).
    fn ouvrir_ancre(&self, o: &OuvertureAncree, session: &str, etat: Option<EtatOuverture>)
    -> bool;
}
