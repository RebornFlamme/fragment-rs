// Port de app/src/ui/views/agent/controls/controls.ts (la puce de CONTEXTE, `agent-context`)
//! La puce de contexte du champ de la BARRE : le document que regarde l'utilisateur et, quand il
//! en sélectionne un passage, ce passage. Les deux partent avec la question (en tête du prompt,
//! voir `fragment_agent::controls_model::context_preamble`) ; la croix retire le contexte, la
//! question part alors seule.
//!
//! Pas dans le panneau : il parle de tout, y joindre d'office le document regardé changerait
//! chaque question. La barre, elle, appartient à un document.
//!
//! La puce ne lit ni le DOM ni la fenêtre : l'hôte lui donne le document regardé (un rappel) et
//! lui signale les changements — l'onglet actif change ([`PuceContexte::document_change`]), la
//! sélection se pose ou s'efface ([`PuceContexte::selection`], déjà filtrée sur le document de la
//! barre).

use std::cell::RefCell;
use std::rc::Rc;

use fragment_agent::controls_model::PromptContext;
use fragment_agent::utf16::{len_utf16, tete_utf16};
use fragment_chat::api::{
    ChatComposerExtension, ChatComposerHandle, ChatMode, ChatSlotContext, ComposerTake, SendOpts,
};
use serde_json::{Value, json};

use super::modele::FOURNISSEUR_AGENT;

/// L'id de l'extension du champ (`chat:composer@1`).
pub const EXTENSION_CONTEXTE: &str = "agent-context";

/// Le passage qui dépasse cette longueur (unités UTF-16) n'est pas joint à la question.
const PASSAGE_MAX: usize = 4000;

/// Le passage montré dans la puce : ses 40 premiers caractères.
const PASSAGE_AFFICHE: usize = 40;

/// Le document que regarde l'utilisateur (celui qui porte la barre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentVu {
    /// La clé du document (chemin du coffre ou `schéma:reste`).
    pub doc: String,
    pub titre: String,
    /// Le nom d'icône du document (le défaut du TS : `file-text`).
    pub icone: String,
}

/// Ce que la puce montre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VuePuce {
    pub icone: String,
    pub titre: String,
    /// Le passage retenu, entre guillemets (« … ») et coupé.
    pub passage: Option<String>,
    /// Le nom accessible de la puce.
    pub nom: String,
}

#[derive(Default)]
struct Etat {
    retire: bool,
    vu: Option<DocumentVu>,
    /// Le passage sélectionné, retenu quand il se pose : cliquer dans le champ ensuite ne
    /// l'efface pas.
    passage: String,
}

/// L'état de la puce, partagé entre sa zone (le rendu) et l'extension du champ (l'envoi).
#[derive(Clone)]
pub struct PuceContexte {
    etat: Rc<RefCell<Etat>>,
    source: Rc<dyn Fn() -> Option<DocumentVu>>,
}

impl PuceContexte {
    /// `source` rend le document que la barre regarde, à l'instant.
    pub fn new(source: Rc<dyn Fn() -> Option<DocumentVu>>) -> Self {
        let puce = Self {
            etat: Rc::new(RefCell::new(Etat::default())),
            source,
        };
        puce.etat.borrow_mut().vu = (puce.source)();
        puce
    }

    /// Visible : un document est regardé et la croix n'a pas été cliquée.
    #[must_use]
    pub fn visible(&self) -> bool {
        let e = self.etat.borrow();
        e.vu.is_some() && !e.retire
    }

    /// L'onglet actif ou le document change : la puce suit, le passage retenu s'efface avec le
    /// document. Sans effet une fois la puce retirée.
    pub fn document_change(&self) {
        if self.etat.borrow().retire {
            return;
        }
        let nouveau = (self.source)();
        let mut e = self.etat.borrow_mut();
        let meme = e.vu.as_ref().map(|v| &v.doc) == nouveau.as_ref().map(|v| &v.doc);
        if !meme {
            e.passage.clear();
        }
        e.vu = nouveau;
    }

    /// La sélection du document de la barre : `Some(texte)` quand un passage se pose, `None` quand
    /// elle se réduit à un point. Sans document regardé, rien ne se retient.
    pub fn selection(&self, texte: Option<&str>) {
        let mut e = self.etat.borrow_mut();
        if e.vu.is_none() {
            return;
        }
        e.passage = texte.unwrap_or_default().to_owned();
    }

    /// La croix : la question part sans contexte.
    pub fn retirer(&self) {
        self.etat.borrow_mut().retire = true;
    }

    /// Ce que dessine la puce, ou `None` (zone cachée).
    #[must_use]
    pub fn vue(&self) -> Option<VuePuce> {
        let e = self.etat.borrow();
        let v = e.vu.as_ref().filter(|_| !e.retire)?;
        let brut = e.passage.trim();
        let passage = (!brut.is_empty()).then(|| {
            let court = brut.split_whitespace().collect::<Vec<_>>().join(" ");
            let coupe = tete_utf16(&court, PASSAGE_AFFICHE);
            let points = if len_utf16(brut) > PASSAGE_AFFICHE {
                "…"
            } else {
                ""
            };
            format!("« {coupe}{points} »")
        });
        Some(VuePuce {
            icone: v.icone.clone(),
            titre: v.titre.clone(),
            passage,
            nom: format!("Contexte : {}", v.titre),
        })
    }

    /// À l'envoi : le contexte joint à la question. Le passage est consommé (il ne repart pas
    /// avec la question suivante) ; trop long, il n'est pas joint.
    pub fn prendre(&self) -> Option<PromptContext> {
        let mut e = self.etat.borrow_mut();
        if e.retire {
            return None;
        }
        let v = e.vu.clone()?;
        let sel = e.passage.trim().to_owned();
        e.passage.clear();
        let selection = (!sel.is_empty() && len_utf16(&sel) < PASSAGE_MAX).then_some(sel);
        Some(PromptContext {
            doc: v.doc,
            title: v.titre,
            selection,
        })
    }
}

/// Le contexte, dans les options d'un envoi (`opts.context` du TS).
#[must_use]
pub fn contexte_vers_options(c: &PromptContext) -> SendOpts {
    let mut ctx = serde_json::Map::new();
    ctx.insert("doc".into(), Value::String(c.doc.clone()));
    ctx.insert("title".into(), Value::String(c.title.clone()));
    if let Some(s) = &c.selection {
        ctx.insert("selection".into(), Value::String(s.clone()));
    }
    let mut opts = SendOpts::new();
    opts.insert("context".into(), json!(ctx));
    opts
}

/// L'extension `chat:composer@1` de la puce : barre seulement, fournisseur agent seulement.
pub struct ExtensionContexte {
    puce: PuceContexte,
}

impl ExtensionContexte {
    #[must_use]
    pub fn new(puce: PuceContexte) -> Self {
        Self { puce }
    }
}

impl ChatComposerExtension for ExtensionContexte {
    fn id(&self) -> &str {
        EXTENSION_CONTEXTE
    }

    fn applies_to(&self, ctx: &ChatSlotContext) -> bool {
        ctx.provider == FOURNISSEUR_AGENT && ctx.mode == ChatMode::Bar
    }

    fn create(&self, _ctx: &ChatSlotContext) -> Box<dyn ChatComposerHandle> {
        Box::new(PoigneeContexte {
            puce: self.puce.clone(),
        })
    }
}

struct PoigneeContexte {
    puce: PuceContexte,
}

impl ChatComposerHandle for PoigneeContexte {
    fn take(&mut self) -> Option<ComposerTake> {
        let c = self.puce.prendre()?;
        Some(ComposerTake {
            opts: Some(contexte_vers_options(&c)),
            parts: Vec::new(),
        })
    }
}
