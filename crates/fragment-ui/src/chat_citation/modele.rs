// Port de app/src/ui/views/chat/components/quote.ts (hors rendu DOM).
//! Le modèle de « Répondre » : l'état d'un champ, l'extension `quote`, la cible.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_chat::api::{
    ChatComposerExtension, ChatComposerHandle, ChatPart, ChatQuote, ChatReplyTarget,
    ChatSlotContext, ComposerTake, QuotePartData,
};
use serde_json::{Map, Value, json};

use super::vue::zone_citation;
use crate::chat_fil::ExtensionChamp;
use crate::chat_pieces::{part_data, piece_depuis_json, piece_json};

/// L'id de l'extension (registre `chat:composer@1`, et `ChatProvider::without`).
pub const QUOTE_EXTENSION: &str = "quote";

/// Sa priorité dans le registre : celle par défaut (le TS n'en donne pas).
pub const PRIORITE_CITATION: i32 = 0;

/// La borne par défaut de l'extrait (140 caractères).
pub const EXTRAIT_MAX: usize = 140;

/// Le rang d'affichage d'un part dans un message : le part `quote` passe AU-DESSUS du texte (−1),
/// les autres gardent l'ordre du message (0). Le message trie de façon stable ; la position du part
/// dans le modèle ne change pas.
#[must_use]
pub fn rang_d_affichage(kind: &str) -> i32 {
    if kind == "quote" { -1 } else { 0 }
}

/// L'extrait montré d'une citation : une ligne (blancs réduits), bornée à `max` caractères dont le
/// dernier est « … » quand on coupe.
#[must_use]
pub fn extrait(texte: &str, max: usize) -> String {
    let t = texte.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > max {
        let mut coupe: String = t.chars().take(max.saturating_sub(1)).collect();
        coupe.push('…');
        coupe
    } else {
        t
    }
}

/// La citation en tête du PROMPT envoyé : le passage cité en Markdown, ou la mention de l'image
/// jointe (une zone entourée). Un fournisseur s'en sert s'il veut.
#[must_use]
pub fn quote_for_prompt(q: &ChatQuote, prompt: &str) -> String {
    let tete = match q.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => t
            .split('\n')
            .map(|l| format!("> {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None if q.image.is_some() => {
            "> (the attached image: a region of the conversation above)".to_owned()
        }
        None => String::new(),
    };
    if tete.is_empty() {
        prompt.to_owned()
    } else {
        format!("In reply to this part of the conversation:\n{tete}\n\n{prompt}")
    }
}

/// Ce que garde le part (sérialisable : les octets d'une image deviennent une URL `data:`).
#[must_use]
pub fn donnees(q: &ChatQuote) -> QuotePartData {
    QuotePartData {
        from: q.from,
        to: q.to,
        text: q.text.clone().filter(|t| !t.is_empty()),
        image: q.image.as_ref().map(part_data),
    }
}

/// Le part `quote` d'une citation, tel que le message le garde.
#[must_use]
pub fn part_quote(q: &ChatQuote) -> ChatPart {
    ChatPart {
        kind: "quote".into(),
        data: serde_json::to_value(donnees(q)).ok(),
        ..ChatPart::default()
    }
}

/// Une citation en JSON, pour `SendOpts` et `save` : `{from, to, text?, image?: {name, mime, data}}`
/// (le format que lit `fragment-agent::fournisseur`).
#[must_use]
pub fn quote_json(q: &ChatQuote) -> Value {
    let mut m = Map::new();
    m.insert("from".into(), json!(q.from));
    m.insert("to".into(), json!(q.to));
    if let Some(t) = q.text.as_ref().filter(|t| !t.is_empty()) {
        m.insert("text".into(), json!(t));
    }
    if let Some(i) = &q.image {
        m.insert("image".into(), piece_json(i));
    }
    Value::Object(m)
}

/// L'inverse de [`quote_json`] ; `None` si la forme n'est pas celle d'une citation (`estCitation` :
/// `from` et `to` numériques).
#[must_use]
pub fn quote_depuis_json(v: &Value) -> Option<ChatQuote> {
    Some(ChatQuote {
        from: usize::try_from(v.get("from")?.as_u64()?).ok()?,
        to: usize::try_from(v.get("to")?.as_u64()?).ok()?,
        text: v.get("text").and_then(Value::as_str).map(str::to_owned),
        image: v.get("image").and_then(piece_depuis_json),
    })
}

/// La citation posée dans UN champ.
#[derive(Default)]
pub struct EtatCitation {
    citation: Option<ChatQuote>,
}

impl EtatCitation {
    #[must_use]
    pub fn citation(&self) -> Option<&ChatQuote> {
        self.citation.as_ref()
    }

    /// Pose (ou remplace) la citation : le chemin de `ChatView.reply` comme d'une vue reconstruite.
    pub fn poser(&mut self, q: ChatQuote) {
        self.citation = Some(q);
    }

    /// Le ✕.
    pub fn retirer(&mut self) {
        self.citation = None;
    }

    /// À l'envoi : `opts.quote` et un part `quote`. Vide ensuite. `None` sans citation.
    pub fn prendre(&mut self) -> Option<ComposerTake> {
        let q = self.citation.take()?;
        let mut opts = Map::new();
        opts.insert("quote".into(), quote_json(&q));
        Some(ComposerTake {
            opts: Some(opts),
            parts: vec![part_quote(&q)],
        })
    }

    #[must_use]
    pub fn sauver(&self) -> Option<Value> {
        self.citation.as_ref().map(quote_json)
    }

    /// Rend un état gardé ; ignoré s'il n'est pas une citation.
    pub fn restaurer(&mut self, etat: &Value) {
        if let Some(q) = quote_depuis_json(etat) {
            self.citation = Some(q);
        }
    }
}

/// L'état partagé entre la poignée, la zone et la cible.
pub type EtatPartage = Rc<RefCell<EtatCitation>>;

struct PoigneeCitation(EtatPartage);

impl ChatComposerHandle for PoigneeCitation {
    /// Une citation seule ne part pas : il faut écrire la réponse.
    fn pending(&self) -> bool {
        false
    }

    fn take(&mut self) -> Option<ComposerTake> {
        self.0.borrow_mut().prendre()
    }

    fn save(&self) -> Option<Value> {
        self.0.borrow().sauver()
    }

    fn restore(&mut self, etat: Value) {
        self.0.borrow_mut().restaurer(&etat);
    }
}

/// L'extension `quote` : la contribution du plugin chat à `chat:composer@1`.
#[derive(Clone, Default)]
pub struct ExtensionCitation {
    courant: Rc<RefCell<Option<EtatPartage>>>,
}

impl ExtensionCitation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// L'état du dernier champ monté.
    #[must_use]
    pub fn etat(&self) -> Option<EtatPartage> {
        self.courant.borrow().clone()
    }

    /// L'entrée du registre du fil : l'extension et sa zone (la puce).
    #[must_use]
    pub fn champ(&self) -> ExtensionChamp {
        ExtensionChamp::new(Rc::new(self.clone()))
            .avec_priorite(PRIORITE_CITATION)
            .avec_zone(zone_citation(self.courant.clone()))
    }

    /// La cible de « Répondre » d'une vue dont le fournisseur refuse (ou non) les citations.
    #[must_use]
    pub fn cible(&self, sans: &[String]) -> CibleReponse {
        CibleReponse {
            extension: self.clone(),
            refusee: sans.iter().any(|s| s == QUOTE_EXTENSION),
        }
    }
}

impl ChatComposerExtension for ExtensionCitation {
    fn id(&self) -> &str {
        QUOTE_EXTENSION
    }

    fn create(&self, _ctx: &ChatSlotContext) -> Box<dyn ChatComposerHandle> {
        let etat: EtatPartage = Rc::default();
        *self.courant.borrow_mut() = Some(etat.clone());
        Box::new(PoigneeCitation(etat))
    }
}

/// Une vue qui sait RÉPONDRE à une partie d'elle-même (`canReply`, `reply`).
pub struct CibleReponse {
    extension: ExtensionCitation,
    refusee: bool,
}

impl ChatReplyTarget for CibleReponse {
    /// Faux sans champ monté, ou si le fournisseur refuse les citations (`without: ["quote"]`).
    fn can_reply(&self) -> bool {
        !self.refusee && self.extension.etat().is_some()
    }

    fn reply(&mut self, q: fragment_chat::api::ChatQuote) -> bool {
        if self.refusee {
            return false;
        }
        match self.extension.etat() {
            Some(e) => {
                e.borrow_mut().poser(q);
                true
            }
            None => false,
        }
    }
}
