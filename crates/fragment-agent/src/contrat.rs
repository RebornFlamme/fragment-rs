// Port de app/src/ui/views/agent/contract.ts (+ types.ts, api.ts pour `AgentService`)
//
// Le contrat PUBLIC de la couche agentique : le vocabulaire PARTAGÉ par tous les
// harness (événements, bulles relues, politique de permissions) et les traits
// `AgentHarness` (`agent:harness@1`) / `AgentService` (`agent:service@1`).
//
// ★ HARNESS = un MOTEUR d'agent (Claude, Codex…). L'agent ne connaît aucun moteur
//   par son nom : il garde sessions, trace, pièces jointes, politique, et confie le
//   TOUR au harness de la conversation.
// ★ Écart assumé : les `Promise` du TS deviennent des appels SYNCHRONES ici (les
//   modèles sont purs, pas de runtime async dans cette crate). Le harness Claude
//   (processus `claude`) donnera l'enveloppe asynchrone ; les traits se
//   raffineront alors par ADR si besoin (voir la PR).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// L'entrée brute d'un outil (`Record<string, unknown>`).
pub type Entree = Map<String, Value>;

/// Le résultat d'un outil rattaché à son appel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultatOutil {
    pub is_error: bool,
    pub text: String,
}

/// Un appel d'outil dans une bulle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocOutil {
    pub id: String,
    pub name: String,
    /// L'entrée de l'outil, résumée en une ligne (chemin, motif…).
    pub summary: String,
    /// L'entrée brute (la trace en tire son verbe et sa cible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Entree>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultatOutil>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
    /// L'heure du résultat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_at: Option<i64>,
}

/// Un morceau de réponse affichable. `at` : l'heure (ms) du message qui le porte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChatBlock {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    Thinking {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    Tool(BlocOutil),
    /// Une pièce jointe d'un message de l'utilisateur (image, document, fichier posé sur le disque).
    Attachment {
        name: String,
        mime: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
}

impl ChatBlock {
    /// Un bloc texte sans heure.
    pub fn texte(text: impl Into<String>) -> Self {
        ChatBlock::Text {
            text: text.into(),
            at: None,
        }
    }
}

/// Une bulle du fil, telle que la RELIT un harness (`history`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatEntry {
    pub role: Role,
    pub blocks: Vec<ChatBlock>,
    /// Un repère OPAQUE du harness sur la DERNIÈRE entrée de la bulle (Claude : l'uuid
    /// de son transcript) — le point où brancher pour garder ce tour entier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
    /// L'heure (ms) de son premier message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// L'usage d'un tour. Tout est facultatif : chaque moteur dit ce qu'il sait.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    /// Ce qu'occupe la conversation dans la fenêtre au dernier appel du modèle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,
    /// La taille de la fenêtre de contexte du modèle, si connue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Le coût du tour en dollars, si connu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

/// Ce qu'un tour émet, au fil de l'eau — le flux NORMALISÉ que tout harness produit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum AgentEvent {
    TextDelta {
        text: String,
    },
    /// La pensée (résumée) qui s'écrit ; `start` : un nouveau bloc de pensée commence.
    ThinkingDelta {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<bool>,
    },
    /// Des appels d'outils (le texte, lui, arrive par `text-delta`).
    Assistant {
        blocks: Vec<ChatBlock>,
    },
    #[serde(rename_all = "camelCase")]
    ToolResult {
        tool_use_id: String,
        is_error: bool,
        text: String,
    },
    /// Un relevé d'usage en cours de tour (fusionné avec le précédent).
    Usage {
        usage: TurnUsage,
    },
    #[serde(rename_all = "camelCase")]
    Done {
        session_id: String,
        is_error: bool,
        errors: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<TurnUsage>,
    },
    Error {
        message: String,
    },
}

// ── La politique de permissions (réglée par l'utilisateur, appliquée par le harness) ──

/// Autoriser sans demander, demander (une carte dans la réponse), interdire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolAccess {
    Allow,
    Ask,
    Deny,
}

/// Les familles d'outils, NEUTRES (un harness les traduit dans ses propres outils).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolFamily {
    Read,
    Search,
    Write,
    Edit,
    Shell,
    Eval,
    FragmentCli,
    WebFetch,
    WebSearch,
    Subagents,
    Skills,
}

/// L'accès à un dossier : invisible, lisible, lisible et inscriptible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FolderAccess {
    Hidden,
    Read,
    Write,
}

/// Le MODE d'une conversation : `full` = la configuration de base, où « demander »
/// vaut « autoriser » ; `ask` = les « demander » deviennent des cartes ;
/// `acceptEdits` = comme `ask`, mais écrire et modifier sont autorisés ; `plan` =
/// lecture seule, puis un plan à valider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Full,
    Ask,
    AcceptEdits,
    Plan,
}

/// Une règle « toujours autoriser » donnée par une carte (ou écrite dans les réglages).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermanentRule {
    pub family: ToolFamily,
    /// Le motif (commande `git status*`, chemin `notes/**`, domaine) ; absent = toute la famille.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// Les domaines que `web-fetch` peut lire : `'*'` = tous, sinon une liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebDomains {
    Tous,
    Liste(Vec<String>),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum WebDomainsRepr {
    Etoile(String),
    Liste(Vec<String>),
}

impl Serialize for WebDomains {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            WebDomains::Tous => WebDomainsRepr::Etoile("*".into()).serialize(s),
            WebDomains::Liste(l) => WebDomainsRepr::Liste(l.clone()).serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for WebDomains {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match WebDomainsRepr::deserialize(d)? {
            WebDomainsRepr::Etoile(_) => WebDomains::Tous,
            WebDomainsRepr::Liste(l) => WebDomains::Liste(l),
        })
    }
}

/// Un dossier HORS du coffre rendu accessible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DossierExterne {
    pub path: String,
    pub access: FolderAccess,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPolicy {
    pub mode: PermissionMode,
    pub tools: BTreeMap<ToolFamily, ToolAccess>,
    /// Les dossiers du COFFRE, chemin relatif (`""` = la racine) → accès. Héritage :
    /// le plus proche ancêtre déclaré décide ; la racine absente vaut `write`.
    pub folders: BTreeMap<String, FolderAccess>,
    /// Des dossiers HORS du coffre (chemins absolus) rendus accessibles.
    pub outside: Vec<DossierExterne>,
    /// Des commandes autorisées même quand le shell est « demander » (motifs à `*` final).
    pub shell_rules: Vec<String>,
    /// Les domaines que `web-fetch` peut lire ; `'*'` = tous.
    pub web_domains: WebDomains,
    /// Les « toujours autoriser » (coffre + conversation, fusionnés par l'hôte).
    pub permanent: Vec<PermanentRule>,
}

/// Une demande d'autorisation, telle que la carte la montre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// L'outil tel que le harness le nomme (`Bash`, `Write`…).
    pub tool: String,
    pub family: Option<ToolFamily>,
    /// L'entrée de l'outil (commande, chemin…).
    pub input: Entree,
    /// Ce que le moteur dit de la demande (titre, raison).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Une demande de validation d'un PLAN (mode `plan`) : le texte du plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
}

/// Ce que décide la GARDE de Fragment pour un usage d'outil (`HarnessIO.guard`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardDecision {
    Allow,
    Ask,
    Deny { message: String },
}

/// La portée d'un « toujours ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RememberScope {
    Conversation,
    Vault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remember {
    pub scope: RememberScope,
    pub rule: PermanentRule,
}

/// La réponse de l'utilisateur. `remember` : la règle à retenir (portée).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow { remember: Option<Remember> },
    Deny { message: Option<String> },
}

// ── Le harness ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessModel {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Les niveaux d'effort que le modèle accepte (absent : pas de réglage d'effort).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
    /// Un modèle RAPIDE et économique (le petit agent des actions rapides le préfère).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small: Option<bool>,
    /// Le modèle par défaut du harness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

/// Le shell : par règles de commandes, par un bac à sable OS, ou pas du tout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellSupport {
    Rules,
    Sandbox,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySupport {
    /// La matrice des outils (par famille).
    pub tools: bool,
    /// Les dossiers masqués / en lecture seule.
    pub folders: bool,
    /// Le réseau (domaines).
    pub network: bool,
    pub shell: ShellSupport,
}

/// Les types de pièces jointes qu'un harness lit en blocs natifs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Image,
    Pdf,
    Text,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCapabilities {
    /// Reprendre une conversation (sinon : chaque tour repart de zéro).
    pub resume: bool,
    /// Brancher une conversation sur une autre.
    pub fork: bool,
    /// Une pensée visible (`thinking-delta`).
    pub thinking: bool,
    /// Le texte arrive en deltas (sinon : par blocs entiers).
    pub text_deltas: bool,
    pub attachments: Vec<AttachmentKind>,
    /// `one_shot` sait rendre une sortie au schéma donné.
    pub structured_output: bool,
    /// Sait DEMANDER (`HarnessIO::request_permission`) — sinon `ask` n'est pas proposé.
    pub permission_prompts: bool,
    pub policy: PolicySupport,
}

/// Une conversation, vue du harness : son id (choisi par Fragment) et SON état (opaque).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessConversation {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,
}

/// Un tour confié au harness.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessTurn {
    pub conversation: HarnessConversation,
    pub prompt: String,
    /// Les pièces jointes, en blocs de contenu (format de l'API Anthropic).
    pub content: Vec<Entree>,
    /// Le modèle choisi (absent : celui du harness).
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Les CONSIGNES de Fragment (le prompt de l'app : CLI, coffre, rendu).
    pub instructions: String,
    /// Le PROFIL de la conversation (un Side Chat a le sien), qui s'ajoute aux consignes.
    pub profile: Option<String>,
    /// La racine du coffre (le dossier de travail).
    pub cwd: String,
    /// Des dossiers hors du coffre que l'agent doit pouvoir lire.
    pub extra_dirs: Vec<String>,
    pub policy: AgentPolicy,
    /// Une conversation NEUVE qui naît d'une autre, coupée au repère `at` (`ChatEntry::last`).
    pub fork: Option<ForkSource>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForkSource {
    pub from: HarnessConversation,
    pub at: String,
}

/// Un usage d'outil, tel qu'un harness le décrit.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolUse {
    pub tool: String,
    pub input: Entree,
}

/// Ce que Fragment met à disposition du harness pendant un tour.
pub trait HarnessIO {
    fn event(&mut self, e: AgentEvent);
    /// Persiste l'état du harness pour cette conversation (ex. l'id de son fil).
    fn set_state(&mut self, state: Value);
    /// Demande (une carte dans la réponse). Toujours résolue (refus si la vue part).
    fn request_permission(&mut self, r: PermissionRequest) -> PermissionDecision;
    /// La GARDE de la politique, avant TOUT usage d'outil. `None` : pas de garde.
    fn guard(&self, _usage: &ToolUse) -> Option<GuardDecision> {
        None
    }
}

pub trait HarnessRun {
    fn interrupt(&self);
    /// Vrai quand le tour est fini (tous ses événements émis).
    fn est_termine(&self) -> bool;
}

/// Une question SANS conversation, à la sortie structurée (le petit agent des actions rapides).
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessOneShot {
    pub instructions: String,
    pub prompt: String,
    pub content: Vec<Entree>,
    /// Le JSON Schema de la sortie attendue.
    pub schema: Value,
    pub model: Option<String>,
    pub cwd: String,
}

/// La FORME des questions sans conversation à venir (`prepare_one_shot`).
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessOneShotSpec {
    pub instructions: String,
    pub schema: Value,
    pub model: Option<String>,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessStatus {
    pub ready: bool,
    pub message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErreurHarness {
    #[error("harness indisponible : {0}")]
    Indisponible(String),
    #[error("opération non prise en charge par ce harness : {0}")]
    NonSupporte(&'static str),
    #[error("échec du tour : {0}")]
    Tour(String),
}

/// `agent:harness@1` — un MOTEUR d'agent.
pub trait AgentHarness {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    /// Une icône (nom d'icône) ou un court texte.
    fn icon(&self) -> Option<&str> {
        None
    }
    fn capabilities(&self) -> HarnessCapabilities;
    /// Prêt ? (binaire présent, compte connecté) — le message explique sinon.
    fn status(&self) -> HarnessStatus {
        HarnessStatus {
            ready: true,
            message: None,
        }
    }
    fn models(&self) -> Vec<HarnessModel>;
    fn run(
        &self,
        turn: HarnessTurn,
        io: &mut dyn HarnessIO,
    ) -> Result<Box<dyn HarnessRun>, ErreurHarness>;
    /// Le fil d'une conversation (le harness est LA source de ses messages). Vide s'il ne la connaît pas.
    fn history(&self, conv: &HarnessConversation) -> Vec<ChatEntry>;
    fn one_shot(&self, _req: HarnessOneShot) -> Result<Value, ErreurHarness> {
        Err(ErreurHarness::NonSupporte("one_shot"))
    }
    /// « Garde-toi prêt » pour des `one_shot` de cette forme ; `None` libère.
    fn prepare_one_shot(&self, _spec: Option<HarnessOneShotSpec>) {}
    /// La conversation est supprimée de l'historique (le harness peut nettoyer).
    fn forget(&self, _conv: &HarnessConversation) {}
}

// ── agent:service@1 (api.ts) ─────────────────────────────────────────────────

/// Une session de l'historique, telle que la lit un plugin (`AgentService.getSession`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionInfo {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// D'où vient la session quand ce n'est pas le chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Entree>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentChatOptions {
    pub session_id: Option<String>,
    /// Prioritaire sur `session_id` ; ne réutilise pas un tour en cours.
    pub new_session: bool,
    pub effort: Option<String>,
}

/// Un tour SANS le chat.
pub struct AgentQuery {
    pub prompt: String,
    /// Absent = une nouvelle session ; un id déjà servi = sa suite.
    pub session_id: Option<String>,
    /// Le MOTEUR d'une session NEUVE (id d'un `agent:harness@1`).
    pub harness: Option<String>,
    pub model: Option<String>,
    pub system: Option<String>,
    pub effort: Option<String>,
    /// Les outils permis ; `Some(vec![])` = aucun.
    pub tools: Option<Vec<String>>,
    pub title: Option<String>,
    pub meta: Option<Entree>,
    pub fork: Option<(String, String)>,
    pub content: Vec<Entree>,
    pub mode: Option<PermissionMode>,
    pub on_event: Box<dyn FnMut(AgentEvent)>,
}

/// Le tour lancé par `AgentService::run`.
pub trait AgentQueryRun {
    fn session_id(&self) -> &str;
    fn interrupt(&self);
    fn est_termine(&self) -> bool;
}

/// `agent:service@1` — le service de l'agent ; clef = le plugin qui le fournit (`agent`).
pub trait AgentService {
    /// Montre le chat (le crée au besoin dans le dock droit).
    fn open_chat(&self);
    /// Ouvre le chat par sa commande et envoie `prompt`.
    fn ask(&self, prompt: &str, opts: AgentChatOptions);
    /// Un tour SANS le chat : ses événements vont à `on_event`.
    fn run(&self, query: AgentQuery) -> Result<Box<dyn AgentQueryRun>, ErreurHarness>;
    /// Une session de l'historique : son titre et sa `meta`.
    fn get_session(&self, id: &str) -> Option<AgentSessionInfo>;
    // TODO(contrat) : `openAnchored` (AgentAnchoredOpen) dépend de DurableAnchor
    // (fragment-artefacts) et de ChatAttachment (fragment-chat) ; ajouté à ces portages.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_evenements_se_serialisent_comme_le_ts() {
        let e = AgentEvent::TextDelta { text: "a".into() };
        assert_eq!(
            serde_json::to_value(&e).unwrap_or_default(),
            serde_json::json!({"type": "text-delta", "text": "a"})
        );
        let b = ChatBlock::Tool(BlocOutil {
            id: "t".into(),
            name: "Read".into(),
            summary: "a".into(),
            input: None,
            result: None,
            at: None,
            result_at: None,
        });
        assert_eq!(
            serde_json::to_value(&b).unwrap_or_default(),
            serde_json::json!({"kind": "tool", "id": "t", "name": "Read", "summary": "a"})
        );
    }

    #[test]
    fn domaines_web_aller_retour() {
        let t: WebDomains = serde_json::from_str("\"*\"").unwrap_or(WebDomains::Liste(vec![]));
        assert_eq!(t, WebDomains::Tous);
        let l: WebDomains = serde_json::from_str("[\"a.org\"]").unwrap_or(WebDomains::Tous);
        assert_eq!(l, WebDomains::Liste(vec!["a.org".into()]));
    }
}
