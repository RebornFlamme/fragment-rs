// Port de app/src/ui/views/agent/AgentChatProvider.ts (+ chatContributions.ts : l'id `agent`)
//
// L'agent, FOURNISSEUR du plugin cœur `chat` (`chat:provider@1`, id `agent`).
//
// ★ Une conversation = une session : son id EST l'id de session du moteur et celui de l'artefact
//   `agent:session` (titre, dates, meta). L'agent garde ses sessions ; le transcript du moteur
//   reste la source des messages (`load` le relit à la demande, rien n'est dupliqué).
// ★ Le PROFIL d'une session (`meta.profile` : system, tools, effort, model, mode) est celui qu'un
//   plugin d'interaction a choisi : chaque suite le garde. `meta.display` : ce qu'on MONTRE de la
//   première question quand le prompt envoyé est plus long. `meta.preamble` : ce qui précède la
//   première question quand elle reste à écrire (Ask), consommé au premier envoi.
// ★ Un tour = UNE trace qui vit (pensée, outils, narration) + une réponse seule. Un texte suivi
//   d'un outil n'était pas la réponse : c'était l'agent qui se parlait, il passe dans la trace.
//
// Écarts assumés (voir la PR) : appels synchrones (le tour rend la main à sa fin, `stop` est un
// appel à part) ; le plugin `agent` entier devient le trait `AgentHote` ; les options d'envoi
// (`opts`) passent par `EnvoiAgent`, typé, que `ChatProvider::send` décode du JSON.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_chat::api::{
    ChatAttachment, ChatConversationInfo, ChatError, ChatEvent, ChatMessage, ChatOrigin, ChatPart,
    ChatProvider, ChatQuote, Participant, Role as RoleChat, SendOpts,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::attachment_blocks::{ChatAttachment as PieceAgent, titre_des_pieces, to_content_blocks};
use crate::contrat::{
    AgentEvent, ChatBlock, Entree, ErreurHarness, HarnessConversation, PermissionDecision,
    PermissionMode, PermissionRequest, ResultatOutil, Role, TurnUsage,
};
use crate::controls_model::{PromptContext, context_preamble};
use crate::events::fork_point;
use crate::fournisseur::hote::{AgentHote, Auteur, DemandeTour, SortieTour};
use crate::fournisseur::parts::{ID_REPONSE, ID_TRACE, part_carte, part_reponse, part_trace};
use crate::fournisseur::pieces::{decoder_base64, ranger, reduire_image};
use crate::harness_registry::{HarnessMeta, harness_id_for};
use crate::permission_model::{EtatCarte, PermissionCardData, answer_note};
use crate::trace_model::{OutilEntrant, TraceBuilder, split_turn};
use crate::utf16::{len_utf16, tete_utf16};

/// L'id du fournisseur dans `chat:provider@1`.
pub const ID_FOURNISSEUR: &str = "agent";
/// Le type du part d'une carte d'autorisation.
pub const AGENT_PERMISSION_PART: &str = "agent-permission";

/// Le titre d'une session : sa première demande, sur une ligne, bornée.
pub fn titre_depuis(prompt: &str) -> String {
    let ligne = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if len_utf16(&ligne) > 60 {
        format!("{}…", tete_utf16(&ligne, 59))
    } else if ligne.is_empty() {
        "Nouvelle session".to_string()
    } else {
        ligne
    }
}

/// Le profil d'une conversation (`meta.profile`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilAgent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// `Some(vec![])` : aucun outil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// Un niveau d'effort du modèle (`HarnessModel::efforts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Le modèle de la conversation ; absent : le défaut du harness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Le MODE de permission de la conversation ; absent : celui des réglages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PermissionMode>,
}

/// Les choix faits dans le champ pour une conversation PAS ENCORE créée (le 1er envoi les écrit).
#[derive(Debug, Clone, Default)]
struct EnAttente {
    effort: Option<String>,
    model: Option<String>,
    mode: Option<PermissionMode>,
    harness: Option<String>,
    /// Des métadonnées de session (ex. la barre fixe : `{bar: true, origin}`).
    meta: Option<Entree>,
}

enum Champ {
    Effort(Option<String>),
    Model(Option<String>),
    Mode(Option<PermissionMode>),
}

/// Ce qu'un envoi emporte en plus du texte (le `opts` du TS, typé).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvoiAgent {
    /// Remplace le texte comme prompt (la question montrée reste `text`).
    pub prompt: Option<String>,
    /// RÉPONDRE à une partie de la conversation (extension `quote`).
    pub quote: Option<ChatQuote>,
    /// Le document regardé, le passage sélectionné (extension `agent-context`).
    pub context: Option<PromptContext>,
    pub attachments: Vec<ChatAttachment>,
    pub effort: Option<String>,
    pub model: Option<String>,
}

fn piece_depuis_json(v: &Value) -> Option<ChatAttachment> {
    Some(ChatAttachment {
        name: v.get("name")?.as_str()?.to_string(),
        mime: v.get("mime")?.as_str()?.to_string(),
        data: decoder_base64(v.get("data")?.as_str()?)?,
    })
}

impl EnvoiAgent {
    /// Décode le `SendOpts` JSON du chat. ★ Les octets d'une pièce y sont en base64 :
    /// `attachments: [{name, mime, data}]`, `quote: {from, to, text?, image?: {name, mime, data}}`,
    /// `context: {doc, title, selection?}`, `prompt`, `effort`, `model`.
    pub fn depuis_opts(opts: &SendOpts) -> Self {
        let chaine = |k: &str| opts.get(k).and_then(Value::as_str).map(str::to_string);
        let quote = opts.get("quote").and_then(|q| {
            Some(ChatQuote {
                from: usize::try_from(q.get("from")?.as_u64()?).ok()?,
                to: usize::try_from(q.get("to")?.as_u64()?).ok()?,
                text: q.get("text").and_then(Value::as_str).map(str::to_string),
                image: q.get("image").and_then(piece_depuis_json),
            })
        });
        let context = opts.get("context").and_then(|c| {
            Some(PromptContext {
                doc: c.get("doc")?.as_str()?.to_string(),
                title: c
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                selection: c
                    .get("selection")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        });
        Self {
            prompt: chaine("prompt"),
            quote,
            context,
            attachments: opts
                .get("attachments")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(piece_depuis_json).collect())
                .unwrap_or_default(),
            effort: chaine("effort"),
            model: chaine("model"),
        }
    }
}

/// La citation en tête du PROMPT envoyé : le passage cité en Markdown, ou la mention de l'image
/// jointe (une zone entourée).
fn citation_pour_prompt(q: &ChatQuote, prompt: &str) -> String {
    let tete = match q.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => t
            .split('\n')
            .map(|l| format!("> {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None if q.image.is_some() => {
            "> (the attached image: a region of the conversation above)".to_string()
        }
        None => String::new(),
    };
    if tete.is_empty() {
        prompt.to_string()
    } else {
        format!("In reply to this part of the conversation:\n{tete}\n\n{prompt}")
    }
}

fn message_erreur(e: &ErreurHarness) -> String {
    match e {
        ErreurHarness::Indisponible(m) | ErreurHarness::Tour(m) => m.clone(),
        autre => autre.to_string(),
    }
}

fn fusionner_usage(a: &mut TurnUsage, b: &TurnUsage) {
    macro_rules! champ {
        ($($f:ident),*) => { $( if b.$f.is_some() { a.$f = b.$f; } )* };
    }
    champ!(
        input_tokens,
        output_tokens,
        cached_input_tokens,
        context_tokens,
        context_window,
        cost_usd
    );
}

fn profil_de_meta(meta: &Entree) -> ProfilAgent {
    meta.get("profile")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

fn harness_meta(meta: &Entree) -> HarnessMeta {
    HarnessMeta {
        harness: meta
            .get("harness")
            .and_then(Value::as_str)
            .map(str::to_string),
        harness_state: meta.get("harnessState").cloned(),
    }
}

fn non_vide(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

/// L'agent comme fournisseur du chat.
pub struct AgentChatProvider {
    hote: Rc<dyn AgentHote>,
    /// Les choix faits pour une conversation pas encore créée (effort, modèle, mode, moteur).
    en_attente: RefCell<HashMap<String, EnAttente>>,
    /// Les demandes d'autorisation ouvertes : id de la carte → sa conversation.
    permissions: RefCell<HashMap<String, String>>,
    /// Les réponses données, pas encore relevées par le tour qui attend.
    reponses: RefCell<HashMap<String, PermissionDecision>>,
}

impl AgentChatProvider {
    pub fn new(hote: Rc<dyn AgentHote>) -> Self {
        Self {
            hote,
            en_attente: RefCell::new(HashMap::new()),
            permissions: RefCell::new(HashMap::new()),
            reponses: RefCell::new(HashMap::new()),
        }
    }

    /// Le PROFIL de la conversation : celui de sa session, surchargé par les choix en attente.
    pub fn profile(&self, conversation: &str) -> ProfilAgent {
        let mut p = self
            .hote
            .session(conversation)
            .map(|s| profil_de_meta(&s.meta))
            .unwrap_or_default();
        if let Some(a) = self.en_attente.borrow().get(conversation) {
            if let Some(e) = non_vide(&a.effort) {
                p.effort = Some(e.to_string());
            }
            if let Some(m) = non_vide(&a.model) {
                p.model = Some(m.to_string());
            }
            if a.mode.is_some() {
                p.mode = a.mode;
            }
        }
        p
    }

    /// Le MOTEUR de la conversation (`agent:harness@1`) : celui de sa session, sinon le choix en
    /// attente, sinon le défaut des réglages, sinon Claude.
    pub fn harness_of(&self, conversation: &str) -> String {
        let session = self.hote.session(conversation);
        let meta = session.as_ref().map(|s| harness_meta(&s.meta));
        let attente = self
            .en_attente
            .borrow()
            .get(conversation)
            .and_then(|a| a.harness.clone());
        harness_id_for(
            meta.as_ref(),
            session.is_some(),
            attente.as_deref(),
            self.hote.harness_par_defaut().as_deref(),
        )
    }

    /// Peut-on encore changer de moteur ? Jusqu'au PREMIER tour seulement (pas de transfert).
    pub fn can_change_harness(&self, conversation: &str) -> bool {
        !self
            .hote
            .session(conversation)
            .and_then(|s| s.meta.get("started").and_then(Value::as_bool))
            .unwrap_or(false)
    }

    /// Un champ du profil (outil du champ). Persisté dans la session si elle existe, sinon gardé
    /// pour le 1er envoi.
    fn poser(&self, conversation: &str, champ: Champ) {
        let Some(s) = self.hote.session(conversation) else {
            let mut m = self.en_attente.borrow_mut();
            let a = m.entry(conversation.to_string()).or_default();
            match champ {
                Champ::Effort(v) => a.effort = v,
                Champ::Model(v) => a.model = v,
                Champ::Mode(v) => a.mode = v,
            }
            return;
        };
        let mut p = profil_de_meta(&s.meta);
        match champ {
            Champ::Effort(v) => p.effort = v,
            Champ::Model(v) => p.model = v,
            Champ::Mode(v) => p.mode = v,
        }
        let mut meta = s.meta;
        meta.insert(
            "profile".into(),
            serde_json::to_value(p).unwrap_or_default(),
        );
        self.hote.toucher(conversation, meta, Auteur::Utilisateur);
    }

    /// L'effort d'une conversation (outil du champ).
    pub fn set_effort(&self, conversation: &str, effort: Option<&str>) {
        self.poser(conversation, Champ::Effort(effort.map(str::to_string)));
    }

    /// Le mode de permission d'une conversation (outil du champ, ou un plan lancé).
    pub fn set_mode(&self, conversation: &str, mode: Option<PermissionMode>) {
        self.poser(conversation, Champ::Mode(mode));
    }

    /// Le modèle d'une conversation (outil du champ).
    pub fn set_model(&self, conversation: &str, model: Option<&str>) {
        self.poser(conversation, Champ::Model(model.map(str::to_string)));
    }

    /// Le moteur d'une conversation qui n'a pas encore eu de tour ; `false` si elle en a eu un
    /// (moteur figé). Le modèle choisi est oublié : c'était celui de l'ancien.
    pub fn set_harness(&self, conversation: &str, harness: &str) -> bool {
        if !self.can_change_harness(conversation) {
            return false;
        }
        if let Some(s) = self.hote.session(conversation) {
            let mut profil = profil_de_meta(&s.meta);
            profil.model = None;
            let mut meta = s.meta;
            meta.insert("harness".into(), json!(harness));
            meta.insert(
                "profile".into(),
                serde_json::to_value(profil).unwrap_or_default(),
            );
            self.hote.toucher(conversation, meta, Auteur::Utilisateur);
        } else {
            let mut m = self.en_attente.borrow_mut();
            let a = m.entry(conversation.to_string()).or_default();
            a.model = None;
            a.harness = Some(harness.to_string());
        }
        true
    }

    /// Des métadonnées pour une conversation PAS ENCORE créée (la barre fixe d'un document :
    /// `{bar: true, origin: {doc}}`) — écrites avec la session au 1er envoi.
    pub fn set_pending_meta(&self, conversation: &str, meta: Entree) {
        if self.hote.session(conversation).is_some() {
            return;
        }
        let mut m = self.en_attente.borrow_mut();
        let a = m.entry(conversation.to_string()).or_default();
        a.meta.get_or_insert_with(Entree::new).extend(meta);
    }

    /// Les CHOIX en attente d'une conversation (moteur, modèle, effort, mode) recopiés sur une
    /// autre, pas encore créée : la barre d'un document garde ses choix sur une conversation
    /// « modèle » jamais envoyée, et en lance une neuve à chaque Entrée. La `meta` n'est PAS copiée.
    pub fn copy_pending(&self, from: &str, to: &str) {
        let Some(a) = self.en_attente.borrow().get(from).cloned() else {
            return;
        };
        if self.hote.session(to).is_some() {
            return;
        }
        let mut m = self.en_attente.borrow_mut();
        let b = m.entry(to.to_string()).or_default();
        b.effort = a.effort.or(b.effort.take());
        b.model = a.model.or(b.model.take());
        b.mode = a.mode.or(b.mode);
        b.harness = a.harness.or(b.harness.take());
    }

    /// Les métadonnées d'une conversation : celles de sa session, sinon celles en attente.
    pub fn meta_of(&self, conversation: &str) -> Entree {
        let mut meta = self
            .en_attente
            .borrow()
            .get(conversation)
            .and_then(|a| a.meta.clone())
            .unwrap_or_default();
        if let Some(s) = self.hote.session(conversation) {
            meta.extend(s.meta);
        }
        meta
    }

    /// La RÉPONSE de l'utilisateur à une carte d'autorisation (part `agent-permission`) : la
    /// demande en attente est résolue, la carte se replie en une ligne (le tour l'émet quand il
    /// relève la réponse). Sans effet pour une carte inconnue ou déjà répondue.
    pub fn respond_permission(&self, id: &str, d: PermissionDecision) {
        if !self.permissions.borrow().contains_key(id) {
            return;
        }
        self.reponses
            .borrow_mut()
            .entry(id.to_string())
            .or_insert(d);
    }

    /// Les cartes encore ouvertes d'une conversation : refusées (« Tour arrêté. »).
    fn refuser_cartes(&self, conversation: &str) {
        let ouvertes: Vec<String> = self
            .permissions
            .borrow()
            .iter()
            .filter(|(_, c)| c.as_str() == conversation)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ouvertes {
            self.respond_permission(
                &id,
                PermissionDecision::Deny {
                    message: Some("Tour arrêté.".into()),
                },
            );
        }
    }

    /// Une BRANCHE (`meta.fork`, posé à l'ouverture de la conversation ancrée) : au premier
    /// envoi, son point dans le transcript du parent — la fin du tour du message visé — et le
    /// nombre de bulles héritées (que `load` ne remontre pas : l'embed du parent les montre).
    /// Figé ensuite (la branche part de là).
    fn point_de_branche(&self, conversation: &str) -> Option<(String, String)> {
        let s = self.hote.session(conversation)?;
        let fork = s.meta.get("fork")?.as_object()?.clone();
        let de = fork.get("from")?.as_str()?.to_string();
        if let Some(at) = fork.get("at").and_then(Value::as_str) {
            return Some((de, at.to_string()));
        }
        let parent = self.hote.session(&de)?;
        let harness = self.hote.harness(&self.harness_of(&de))?;
        if !harness.capabilities().fork {
            return None;
        }
        let rang = usize::try_from(fork.get("message")?.as_u64()?).ok()?;
        let entrees = harness.history(&HarnessConversation {
            id: de.clone(),
            state: parent.meta.get("harnessState").cloned(),
        });
        let p = fork_point(&entrees, rang)?;
        let mut fork = fork;
        fork.insert("at".into(), json!(p.at));
        fork.insert("inherited".into(), json!(p.inherited));
        let mut meta = self
            .hote
            .session(conversation)
            .map(|s| s.meta)
            .unwrap_or_default();
        meta.insert("fork".into(), Value::Object(fork));
        self.hote.toucher(conversation, meta, Auteur::Plugin);
        Some((de, p.at))
    }

    /// Un envoi. `ChatProvider::send` y mène, avec les options décodées du JSON.
    pub fn envoyer(
        &self,
        conversation: &str,
        text: &str,
        envoi: EnvoiAgent,
        on_event: &mut dyn FnMut(ChatEvent),
    ) {
        let profile = self.profile(conversation);
        let mut prompt = envoi.prompt.clone().unwrap_or_else(|| text.to_string());
        // Le PRÉAMBULE (posé à l'ouverture d'une conversation dont la première question reste à
        // écrire : le document + la consigne) part avec la première question, une fois ; on
        // retient ce qu'il faut en MONTRER.
        if let Some(s) = self.hote.session(conversation)
            && let Some(pre) = s
                .meta
                .get("preamble")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
        {
            prompt = format!("{pre} {prompt}").trim().to_string();
            let mut meta = s.meta.clone();
            meta.remove("preamble");
            meta.insert("display".into(), json!(text));
            self.hote.toucher(conversation, meta, Auteur::Plugin);
        }
        let fork = self.point_de_branche(conversation);
        // RÉPONDRE à une partie de la conversation : la citation en tête du prompt ; une zone
        // entourée part en image jointe.
        if let Some(q) = &envoi.quote {
            prompt = citation_pour_prompt(q, &prompt);
        }
        // Le CONTEXTE : le document regardé, le passage sélectionné.
        if let Some(c) = &envoi.context {
            prompt = context_preamble(c, &prompt);
        }
        // Les pièces jointes → des blocs de contenu (image native, document, fichier sur le disque).
        let mut pieces = envoi.attachments.clone();
        if let Some(img) = envoi.quote.as_ref().and_then(|q| q.image.clone()) {
            pieces.push(img);
        }
        let mut erreur_rangement: Option<String> = None;
        let content = if pieces.is_empty() {
            Vec::new()
        } else {
            let codec = self.hote.codec_image();
            let reduites: Vec<PieceAgent> = pieces
                .iter()
                .map(|a| reduire_image(a, codec.as_deref()))
                .map(|a| PieceAgent {
                    name: a.name,
                    mime: a.mime,
                    data: a.data,
                })
                .collect();
            let dossier = self.hote.dossier_pieces();
            let maintenant = self.hote.maintenant();
            to_content_blocks(&reduites, |a| {
                let c = ChatAttachment {
                    name: a.name.clone(),
                    mime: a.mime.clone(),
                    data: a.data.clone(),
                };
                match ranger(&dossier, &c, maintenant) {
                    Ok(p) => p.to_string_lossy().into_owned(),
                    Err(e) => {
                        erreur_rangement.get_or_insert(format!(
                            "Impossible de ranger la pièce jointe « {} » : {e}",
                            a.name
                        ));
                        String::new()
                    }
                }
            })
        };
        let effort = envoi.effort.clone().or_else(|| profile.effort.clone());
        let model = envoi.model.clone().or_else(|| profile.model.clone());
        let harness = self.harness_of(conversation);
        let nouvelle = self.hote.session(conversation).is_none();
        let meta_en_attente = self
            .en_attente
            .borrow()
            .get(conversation)
            .and_then(|a| a.meta.clone());

        let mut tour = Tour {
            p: self,
            conversation,
            on_event,
            trace: TraceBuilder::new(self.hote.maintenant()),
            reponse: String::new(),
            usage: TurnUsage::default(),
            fini: false,
            cartes: Vec::new(),
        };
        // ★ La trace part TOUT DE SUITE (vide) : le message a un part dès l'envoi, donc l'orbe au
        //   lieu des « ••• » du chat pendant que le modèle démarre.
        tour.emettre_trace();
        if let Some(e) = erreur_rangement {
            tour.finir(Some(e));
            return;
        }
        let titre = nouvelle.then(|| {
            if text.trim().is_empty() {
                let noms: Vec<&str> = pieces.iter().map(|a| a.name.as_str()).collect();
                titre_des_pieces(&noms)
            } else {
                titre_depuis(text)
            }
        });
        // Une session NEUVE garde ce qui a été choisi pour elle (effort, modèle) : chaque suite le relit.
        let meta = (nouvelle && (effort.is_some() || model.is_some() || meta_en_attente.is_some()))
            .then(|| {
                let mut m = meta_en_attente.unwrap_or_default();
                if effort.is_some() || model.is_some() {
                    let mut profil = Entree::new();
                    if let Some(e) = &effort {
                        profil.insert("effort".into(), json!(e));
                    }
                    if let Some(mo) = &model {
                        profil.insert("model".into(), json!(mo));
                    }
                    m.insert("profile".into(), Value::Object(profil));
                }
                m
            });
        let demande = DemandeTour {
            prompt,
            session_id: conversation.to_string(),
            harness,
            system: profile.system,
            tools: profile.tools,
            mode: profile.mode,
            effort,
            model,
            title: titre,
            content,
            meta,
            fork,
        };
        // Un tour qui ne PART pas (moteur absent, hors bureau) : une erreur dans le tour, et la
        // trace (déjà émise, vide) se retire — pas d'orbe qui tourne pour rien.
        match self.hote.lancer(demande, &mut tour) {
            Ok(()) => {
                self.en_attente.borrow_mut().remove(conversation);
                tour.finir(None);
            }
            Err(e) => tour.finir(Some(message_erreur(&e))),
        }
    }
}

/// L'état d'un tour en cours : la trace qui vit, la réponse, les cartes ouvertes.
struct Tour<'a> {
    p: &'a AgentChatProvider,
    conversation: &'a str,
    on_event: &'a mut dyn FnMut(ChatEvent),
    trace: TraceBuilder,
    reponse: String,
    usage: TurnUsage,
    fini: bool,
    /// Les cartes ouvertes de ce tour (les données en attente, pour les replier si le tour finit).
    cartes: Vec<PermissionCardData>,
}

impl Tour<'_> {
    fn emettre(&mut self, e: ChatEvent) {
        (self.on_event)(e);
    }

    fn emettre_trace(&mut self) {
        let part = part_trace(&self.trace.snapshot());
        self.emettre(ChatEvent::Part(part));
    }

    fn emettre_reponse(&mut self) {
        let part = part_reponse(&self.reponse);
        self.emettre(ChatEvent::Part(part));
    }

    fn garder_usage(&self) {
        if self.usage == TurnUsage::default() {
            return;
        }
        if let Some(s) = self.p.hote.session(self.conversation) {
            let mut meta = s.meta;
            meta.insert(
                "usage".into(),
                serde_json::to_value(&self.usage).unwrap_or_default(),
            );
            self.p.hote.toucher(self.conversation, meta, Auteur::Plugin);
        }
    }

    fn finir(&mut self, error: Option<String>) {
        if self.fini {
            return;
        }
        self.fini = true;
        // Des cartes encore ouvertes (tour arrêté, erreur) : refusées, repliées.
        for mut d in std::mem::take(&mut self.cartes) {
            self.p.permissions.borrow_mut().remove(&d.id);
            self.p.reponses.borrow_mut().remove(&d.id);
            d.state = EtatCarte::Denied;
            d.note = answer_note(None, Some("Tour arrêté."));
            self.emettre(ChatEvent::Part(part_carte(&d)));
        }
        self.trace.finish(self.p.hote.maintenant(), error.is_some());
        // Ni pensée ni outil : pas de trace (comme à la relecture, `split_turn`).
        if self.trace.size() > 0 {
            self.emettre_trace();
        } else {
            self.emettre(ChatEvent::Remove(ID_TRACE.into()));
        }
        self.emettre(ChatEvent::Done { error });
    }

    fn traduire(&mut self, ev: AgentEvent) {
        let now = self.p.hote.maintenant();
        match ev {
            AgentEvent::TextDelta { text } => {
                if self.reponse.is_empty() {
                    // La réponse commence : la pensée en cours se termine, la trace aussi (son
                    // chrono s'arrête) ; l'orbe passe à « écrit ».
                    self.trace.ecrire(now);
                    self.emettre_trace();
                }
                self.reponse.push_str(&text);
                self.emettre_reponse();
            }
            AgentEvent::ThinkingDelta { text, start } => {
                self.trace.thinking(&text, now, start.unwrap_or(false));
                self.emettre_trace();
            }
            AgentEvent::Assistant { blocks } => {
                for b in blocks {
                    let ChatBlock::Tool(outil) = b else { continue };
                    if !self.reponse.trim().is_empty() {
                        self.trace.note(&self.reponse, now);
                        self.emettre(ChatEvent::Remove(ID_REPONSE.into()));
                    }
                    self.reponse.clear();
                    self.trace.tool(
                        OutilEntrant {
                            id: outil.id,
                            name: outil.name,
                            input: outil.input,
                        },
                        now,
                    );
                }
                self.emettre_trace();
            }
            AgentEvent::ToolResult {
                tool_use_id,
                is_error,
                text,
            } => {
                self.trace
                    .result(&tool_use_id, ResultatOutil { is_error, text }, now);
                self.emettre_trace();
            }
            AgentEvent::Usage { usage } => fusionner_usage(&mut self.usage, &usage),
            AgentEvent::Done {
                is_error,
                errors,
                usage,
                ..
            } => {
                if let Some(u) = usage {
                    fusionner_usage(&mut self.usage, &u);
                }
                self.garder_usage();
                self.finir(is_error.then(|| {
                    errors
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Échec du tour.".to_string())
                }));
            }
            AgentEvent::Error { message } => self.finir(Some(message)),
        }
    }
}

impl SortieTour for Tour<'_> {
    fn evenement(&mut self, e: AgentEvent) {
        self.traduire(e);
    }

    /// ★ LES CARTES D'AUTORISATION de ce tour : un part chacune ; la réponse arrive par
    /// `respond_permission` (ou `stop`, qui refuse). Le tour attend en pompant l'hôte.
    fn permission(&mut self, r: PermissionRequest) -> PermissionDecision {
        let id = format!("perm-{}", self.p.hote.nouvel_id());
        let rule = self.p.hote.regle_toujours(&r);
        let mut data = PermissionCardData {
            id: id.clone(),
            request: r,
            state: EtatCarte::Pending,
            rule,
            note: None,
        };
        self.cartes.push(data.clone());
        self.p
            .permissions
            .borrow_mut()
            .insert(id.clone(), self.conversation.to_string());
        self.emettre(ChatEvent::Part(part_carte(&data)));
        let decision = loop {
            if let Some(d) = self.p.reponses.borrow_mut().remove(&id) {
                break d;
            }
            if !self.p.hote.pomper() {
                // Plus personne ne peut répondre : un refus, pas un blocage.
                break PermissionDecision::Deny {
                    message: Some("Personne pour répondre.".into()),
                };
            }
        };
        self.p.permissions.borrow_mut().remove(&id);
        self.cartes.retain(|c| c.id != id);
        let (etat, note) = match &decision {
            PermissionDecision::Allow { remember } => (
                EtatCarte::Allowed,
                answer_note(remember.as_ref().map(|r| r.scope), None),
            ),
            PermissionDecision::Deny { message } => {
                (EtatCarte::Denied, answer_note(None, message.as_deref()))
            }
        };
        data.state = etat;
        data.note = note;
        self.emettre(ChatEvent::Part(part_carte(&data)));
        decision
    }
}

fn parts_utilisateur(blocs: &[ChatBlock]) -> Vec<ChatPart> {
    blocs
        .iter()
        .filter_map(|b| match b {
            ChatBlock::Text { text, .. } => Some(ChatPart::text(text.clone())),
            ChatBlock::Attachment {
                name, mime, url, ..
            } => {
                let mut d = json!({ "name": name, "mime": mime });
                if let (Some(u), Some(o)) = (url, d.as_object_mut()) {
                    o.insert("url".into(), json!(u));
                }
                Some(ChatPart {
                    kind: "attachment".into(),
                    data: Some(d),
                    ..ChatPart::default()
                })
            }
            _ => None,
        })
        .collect()
}

impl ChatProvider for AgentChatProvider {
    fn id(&self) -> &str {
        ID_FOURNISSEUR
    }

    fn label(&self) -> &str {
        "Agent"
    }

    fn icon(&self) -> Option<&str> {
        Some("sparkles")
    }

    fn participants(&self, _conversation: &str) -> Vec<Participant> {
        vec![
            Participant {
                id: "user".into(),
                name: self.hote.nom_utilisateur(),
                initials: None,
                role: RoleChat::User,
            },
            Participant {
                id: "claude".into(),
                name: "Claude".into(),
                initials: Some("C".into()),
                role: RoleChat::Assistant,
            },
        ]
    }

    fn title(&self, conversation: &str) -> String {
        self.hote
            .session(conversation)
            .map(|s| s.title)
            .unwrap_or_default()
    }

    /// Le fil est relu chez SON moteur (le harness est la source de ses messages).
    fn load(&self, conversation: &str) -> Result<Vec<ChatMessage>, ChatError> {
        let Some(session) = self.hote.session(conversation) else {
            return Ok(Vec::new());
        };
        let id = self.harness_of(conversation);
        let Some(harness) = self.hote.harness(&id) else {
            return Ok(vec![ChatMessage {
                id: "indisponible".into(),
                author: "claude".into(),
                parts: vec![ChatPart::text(format!(
                    "*Le moteur « {id} » n’est pas disponible : réactive son plugin pour relire et poursuivre cette conversation.*"
                ))],
                ..ChatMessage::default()
            }]);
        };
        let entrees = harness.history(&HarnessConversation {
            id: conversation.to_string(),
            state: session.meta.get("harnessState").cloned(),
        });
        let display = session
            .meta
            .get("display")
            .and_then(Value::as_str)
            .map(str::to_string);
        // Une branche : son transcript commence par l'historique HÉRITÉ du parent — montré par
        // l'embed du parent, pas ici.
        let herite = session
            .meta
            .get("fork")
            .and_then(|f| f.get("inherited"))
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0);
        let mut premier_user = true;
        let mut messages = Vec::new();
        for (i, e) in entrees.iter().enumerate().skip(herite) {
            let at = e.at;
            if e.role == Role::Assistant {
                // Le tour relu comme il s'est affiché : sa TRACE (pensée, outils, narration),
                // puis sa réponse — mêmes parts, même ordre qu'en direct.
                let debut = i.checked_sub(1).and_then(|k| entrees[k].at);
                let tour = split_turn(&e.blocks, debut);
                let mut parts: Vec<ChatPart> = tour.trace.iter().map(part_trace).collect();
                parts.extend(tour.answer.into_iter().map(ChatPart::text));
                messages.push(ChatMessage {
                    id: format!("m{i}"),
                    author: "claude".into(),
                    parts,
                    at,
                    ..ChatMessage::default()
                });
                continue;
            }
            let mut parts = parts_utilisateur(&e.blocks);
            if premier_user {
                premier_user = false;
                // Ce qu'on MONTRE de la première question (son texte envoyé porte le document) ;
                // ses pièces jointes restent.
                if let Some(d) = &display {
                    let mut montre = vec![ChatPart::text(d.clone())];
                    montre.extend(parts.into_iter().filter(|p| p.kind == "attachment"));
                    parts = montre;
                }
            }
            messages.push(ChatMessage {
                id: format!("m{i}"),
                author: "user".into(),
                parts,
                at,
                ..ChatMessage::default()
            });
        }
        Ok(messages)
    }

    fn send(
        &self,
        conversation: &str,
        text: &str,
        on_event: &mut dyn FnMut(ChatEvent),
        opts: Option<&SendOpts>,
    ) -> Result<(), ChatError> {
        let envoi = opts.map(EnvoiAgent::depuis_opts).unwrap_or_default();
        self.envoyer(conversation, text, envoi, on_event);
        Ok(())
    }

    /// Arrêter : les cartes encore ouvertes sont refusées d'abord (le moteur n'attend plus).
    fn stop(&self, conversation: &str) {
        self.refuser_cartes(conversation);
        self.hote.interrompre(conversation);
    }

    fn create(&self) -> Option<String> {
        Some(self.hote.nouvel_id())
    }

    /// La session quitte l'index ; le transcript du moteur reste sur le disque.
    fn delete(&self, conversation: &str) -> Result<(), ChatError> {
        self.hote.supprimer(conversation);
        Ok(())
    }

    fn list(&self) -> Vec<ChatConversationInfo> {
        self.hote
            .sessions()
            .into_iter()
            .map(|s| ChatConversationInfo {
                id: s.id,
                title: s.title,
                updated_at: s.updated_at,
            })
            .collect()
    }

    /// L'origine (`meta.origin`, posée à l'ouverture) ; une session plus ancienne : `meta.file` + `meta.quote`.
    fn origin(&self, conversation: &str) -> Option<ChatOrigin> {
        let meta = self.hote.session(conversation)?.meta;
        if let Some(o) = meta.get("origin")
            && o.get("doc").is_some_and(Value::is_string)
            && let Ok(origine) = serde_json::from_value::<ChatOrigin>(o.clone())
        {
            return Some(origine);
        }
        let doc = meta.get("file")?.as_str()?.to_string();
        Some(ChatOrigin {
            doc,
            from: None,
            to: None,
            quote: meta
                .get("quote")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }
}
