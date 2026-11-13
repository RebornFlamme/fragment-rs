// Port de app/src/ui/views/chat/ChatView.ts (l'état de la vue, le mode d'après l'endroit, la barre
// repliée par la marge, ⋯, la carte d'origine) et de ChatPlugin.ts (les commandes).
//! L'ÉTAT de la vue chat, sans GPUI : tout ce que décide ChatView.ts hors du dessin, testable seul.
//!
//! La vue du cœur ([`super::vue::VueChat`]) et le contenu GPUI ([`super::contenu::ContenuChat`])
//! partagent cet état par `Rc<RefCell<…>>` : le cœur le pilote (`set_state`, état éphémère, menu),
//! le contenu le lit à chaque image (la révision dit quand reconstruire la structure).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use fragment_chat::api::{
    CHAT_VIEW_TYPE, ChatBarSubmit, ChatMode, ChatOrigin, ChatPart, ChatProvider, Role, SendOpts,
};
use fragment_chat::chat_key::chat_key;
use fragment_chat::chat_session::ChatSession;
use fragment_chat::format::{conversation_markdown, initials_of};
use fragment_core::{
    DocumentEvent, DocumentKey, DocumentManager, EventOrigin, ItemId, MenuPane, Registry,
};
use serde_json::{Map, Value};

use super::geometrie::{Disposition, LARGEUR_BARRE_OUVERTE, Transition, disposition};
use crate::chat_barre_laterale::Horloge;
use crate::chat_fil::{RegistreFil, SlotChat};
use crate::mode_marge::{Genre, MargeDeLecture};

/// Le type de la vue (`chat`), celui des layouts et de `setViewState`.
pub const TYPE_VUE_CHAT: &str = CHAT_VIEW_TYPE;

/// La section du ⋯ où vont télécharger et copier (`setSection('action')`).
pub const SECTION_ACTION: &str = "action";
/// La section de la suppression.
pub const SECTION_DANGER: &str = "danger";

/// La durée de glissement de la barre (`--anim-duration-moderate`, 150 ms).
pub const DUREE_BARRE_MS: f64 = 150.0;

/// La courbe `--anim-motion-smooth`.
pub const COURBE_LISSE: [f32; 4] = [0.2, 0.0, 0.0, 1.0];

/// Ouvre un document dans une feuille : `(feuille, clé, état éphémère)`.
pub type Ouvreur = Rc<dyn Fn(ItemId, &str, Option<Value>)>;

/// Télécharge un fichier : `(nom, contenu)`.
pub type Telechargeur = Rc<dyn Fn(&str, &str)>;

/// Ce que la vue reçoit de l'application : tout ce qu'`app` donnait à ChatView.ts.
#[derive(Clone)]
pub struct ContexteChat {
    /// `chat:provider@1`.
    pub fournisseurs: Registry<Rc<dyn ChatProvider>>,
    /// Les parts, textes, actions, outils et extensions du champ.
    pub contributions: RegistreFil,
    /// `chat:header-actions@1`.
    pub en_tete: Rc<RefCell<Vec<SlotChat>>>,
    /// `chat:footer@1`.
    pub pied: Rc<RefCell<Vec<SlotChat>>>,
    /// L'horloge de l'accueil et de la barre (figée par `pixels:horloge`).
    pub horloge: Rc<RefCell<Horloge>>,
    /// La marge de lecture 1/4 – 3/4 des feuilles (la réserve animée).
    pub marge: Option<Rc<MargeDeLecture>>,
    /// Les documents : titre et icône de l'origine.
    pub documents: Rc<DocumentManager>,
    /// Le temps, en ms : les transitions s'y mesurent (injectable pour les tests).
    pub ms: Rc<dyn Fn() -> f64>,
    /// La feuille est-elle dans un dock (mode `panel`) ?
    pub est_dans_dock: Rc<dyn Fn(ItemId) -> bool>,
    /// Ouvre un document DANS la feuille : une navigation (historique ← →), comme ouvrir une note.
    pub ouvrir: Ouvreur,
    /// Télécharge un fichier `(nom, contenu)` : le dialogue d'enregistrement est celui de l'app.
    pub telecharger: Telechargeur,
    /// `--font-text-size` : la taille du texte d'une note.
    pub taille_texte: f32,
    /// « Readable line length » (`--doc-width` à 720 px, sinon 100 %).
    pub ligne_lisible: Rc<Cell<bool>>,
}

/// Une action posée par le menu ⋯, exécutée par le contenu (qui a le contexte GPUI).
#[derive(Clone, Debug, PartialEq)]
pub enum ActionChat {
    /// Écrire la conversation en Markdown dans un fichier.
    Telecharger { nom: String, contenu: String },
    /// La mettre dans le presse-papiers.
    Copier(String),
    /// La supprimer chez son fournisseur.
    Supprimer,
}

/// Comment se présente une vue embarquée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    Fil,
    /// Un tweet : la question, la 1re réponse (la grappe).
    Post,
}

/// La carte d'origine : le document d'où vient la conversation, en tête du 1er message.
#[derive(Clone, Debug, PartialEq)]
pub struct CarteOrigine {
    pub doc: String,
    pub titre: String,
    pub icone: String,
    /// Le document n'existe plus : la carte est barrée, le clic ne fait rien.
    pub manquant: bool,
    /// Embarquée : un simple lien (pas d'embed dans un embed).
    pub lien_seul: bool,
    /// Le passage d'où elle est partie.
    pub passage: Option<ChatOrigin>,
}

/// L'état de la vue chat.
pub struct EtatChat {
    pub leaf: ItemId,
    pub ctx: ContexteChat,
    pub fournisseur: String,
    pub conversation: String,
    pub session: Option<Rc<RefCell<ChatSession>>>,
    /// Le fil a changé (la session a émis) : à redessiner.
    pub sale: Rc<Cell<bool>>,
    /// Compte les reconstructions de la structure (le contenu les suit).
    pub revision: u64,
    /// L'historique en liste (colonne seulement ; en cowork la barre en tient lieu).
    pub historique: bool,
    pub barre_ouverte: bool,
    marge_vue: bool,
    barre_repliee_par_marge: bool,
    pub presentation: Presentation,
    /// `{docWidget}` : la vue est ancrée dans un widget.
    pub placement: Option<Value>,
    /// Le mode demandé par l'hôte (une feuille détachée : embarquée, barre).
    pub mode_hote: Option<ChatMode>,
    pub brouillon: Option<String>,
    pub envoi_demande: Option<(String, Option<SendOpts>, Vec<ChatPart>)>,
    pub etats_champ: HashMap<String, Value>,
    pub focus_demande: bool,
    /// Une position demandée (lien de message, passage) : pas de placement par défaut.
    pub position_posee: bool,
    pub message_demande: Option<usize>,
    /// La largeur de la barre, animée.
    pub barre: Transition,
    pub actions: Rc<RefCell<VecDeque<ActionChat>>>,
    /// L'envoi d'une vue en mode barre : l'hôte décide (`chat-bar-submit`).
    pub soumissions: VecDeque<ChatBarSubmit>,
    /// Échap dans le champ vide d'une barre (`chat-bar-escape`).
    pub echappe: bool,
}

impl EtatChat {
    /// Un état neuf pour la feuille `leaf`.
    #[must_use]
    pub fn new(leaf: ItemId, ctx: ContexteChat) -> Self {
        Self {
            leaf,
            ctx,
            fournisseur: String::new(),
            conversation: String::new(),
            session: None,
            sale: Rc::new(Cell::new(true)),
            revision: 0,
            historique: false,
            barre_ouverte: false,
            marge_vue: false,
            barre_repliee_par_marge: false,
            presentation: Presentation::Fil,
            placement: None,
            mode_hote: None,
            brouillon: None,
            envoi_demande: None,
            etats_champ: HashMap::new(),
            focus_demande: false,
            position_posee: false,
            message_demande: None,
            barre: Transition::repos(0.0, COURBE_LISSE),
            actions: Rc::default(),
            soumissions: VecDeque::new(),
            echappe: false,
        }
    }

    // ── Où et comment ──────────────────────────────────────────────────────────

    /// Le mode d'après l'endroit : ancrée (`{docWidget}`), embarquée ou barre (l'hôte le dit), dans
    /// un dock : panneau ; sinon page.
    #[must_use]
    pub fn mode(&self) -> ChatMode {
        if self.placement.is_some() {
            return ChatMode::Anchored;
        }
        if let Some(m) = self.mode_hote {
            return m;
        }
        if (self.ctx.est_dans_dock)(self.leaf) {
            ChatMode::Panel
        } else {
            ChatMode::Page
        }
    }

    /// Cowork (barre latérale) en page quand la largeur le permet ; sinon la colonne.
    #[must_use]
    pub fn disposition(&self, largeur_contenu: f32) -> Disposition {
        disposition(self.mode(), largeur_contenu)
    }

    /// Le fournisseur de la vue.
    #[must_use]
    pub fn fournisseur_actif(&self) -> Option<Rc<dyn ChatProvider>> {
        self.ctx.fournisseurs.get(&self.fournisseur)
    }

    /// L'adresse de la conversation : `chat:<fournisseur>/<id>`.
    #[must_use]
    pub fn document(&self) -> Option<String> {
        (!self.fournisseur.is_empty() && !self.conversation.is_empty())
            .then(|| chat_key(&self.fournisseur, &self.conversation))
    }

    /// Le titre de l'onglet.
    #[must_use]
    pub fn titre(&self) -> String {
        match (self.fournisseur_actif(), self.conversation.is_empty()) {
            (Some(p), false) => {
                let t = p.title(&self.conversation);
                if t.is_empty() {
                    "Nouvelle conversation".to_owned()
                } else {
                    t
                }
            }
            _ => "Chat".to_owned(),
        }
    }

    /// L'icône d'onglet : celle du fournisseur.
    #[must_use]
    pub fn icone(&self) -> String {
        self.fournisseur_actif()
            .and_then(|p| p.icon().map(str::to_owned))
            .unwrap_or_else(|| "message-square".to_owned())
    }

    /// Les participants, par leurs initiales (« C », « J ») : les icônes du widget replié.
    #[must_use]
    pub fn icones(&self) -> Vec<String> {
        match self.fournisseur_actif() {
            Some(p) if !self.conversation.is_empty() => p
                .participants(&self.conversation)
                .into_iter()
                .map(|x| x.initials.unwrap_or_else(|| initials_of(&x.name)))
                .collect(),
            _ => vec![self.icone()],
        }
    }

    // ── L'état sérialisé ───────────────────────────────────────────────────────

    /// `{provider, conversation, sidebar}`.
    #[must_use]
    pub fn etat(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("provider".into(), Value::String(self.fournisseur.clone()));
        m.insert(
            "conversation".into(),
            Value::String(self.conversation.clone()),
        );
        m.insert("sidebar".into(), Value::Bool(self.barre_ouverte));
        m
    }

    /// Reçoit un état : une autre conversation relance la session.
    pub fn poser_etat(&mut self, etat: &Map<String, Value>) {
        let fournisseur = etat
            .get("provider")
            .and_then(Value::as_str)
            .map_or_else(|| self.fournisseur.clone(), str::to_owned);
        let conversation = etat
            .get("conversation")
            .and_then(Value::as_str)
            .map_or_else(|| self.conversation.clone(), str::to_owned);
        if let Some(b) = etat.get("sidebar").and_then(Value::as_bool) {
            self.barre_ouverte = b;
            // Un état restauré : la barre est posée là, sans glisser.
            self.barre
                .poser(if b { LARGEUR_BARRE_OUVERTE } else { 0.0 });
        }
        if fournisseur != self.fournisseur
            || conversation != self.conversation
            || self.session.is_none()
        {
            self.fournisseur = fournisseur;
            self.conversation = conversation;
            self.nouvelle_session();
        }
        self.revision += 1;
    }

    /// L'état éphémère : ce que l'hôte demande (état de widget, présentation, brouillon, envoi,
    /// lien de message).
    pub fn poser_etat_ephemere(&mut self, e: &Value) {
        let Some(o) = e.as_object() else { return };
        if o.contains_key("passage") || o.contains_key("scroll") || o.contains_key("subpath") {
            self.position_posee = true;
        }
        if let Some(rang) = o
            .get("subpath")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix("#m"))
            .and_then(|n| n.parse::<usize>().ok())
        {
            self.message_demande = Some(rang.saturating_sub(1));
        }
        match o.get("presentation").and_then(Value::as_str) {
            Some("post") => self.presentation = Presentation::Post,
            Some("thread") => self.presentation = Presentation::Fil,
            _ => {}
        }
        if let Some(m) = o.get("mode").and_then(Value::as_str) {
            self.mode_hote = match m {
                "embedded" => Some(ChatMode::Embedded),
                "bar" => Some(ChatMode::Bar),
                _ => None,
            };
        }
        if let Some(t) = o.get("draft").and_then(Value::as_str) {
            self.brouillon = Some(t.to_owned());
        }
        if let Some(c) = o.get("composer").and_then(Value::as_object) {
            for (k, v) in c {
                self.etats_champ.insert(k.clone(), v.clone());
            }
        }
        if let Some(s) = o.get("send").and_then(Value::as_object)
            && let Some(t) = s.get("text").and_then(Value::as_str)
        {
            self.envoi_demande = Some((
                t.to_owned(),
                s.get("opts").and_then(Value::as_object).cloned(),
                s.get("parts")
                    .and_then(|p| serde_json::from_value(p.clone()).ok())
                    .unwrap_or_default(),
            ));
        }
        if let Some(d) = o.get("docWidget").filter(|d| !d.is_null()) {
            self.placement = Some(d.clone());
        }
        if o.get("focus").and_then(Value::as_bool) == Some(true) {
            self.focus_demande = true;
        }
        // ★ La PLACE seule (`docWidgetRoom`, renvoyée à chaque passe de la marge) ne change rien au
        //   rendu : pas de reconstruction.
        if !o.keys().all(|k| k == "docWidgetRoom") {
            self.revision += 1;
        }
    }

    // ── La session ─────────────────────────────────────────────────────────────

    /// Une autre conversation : sa lecture repart de zéro. Ouverte sans fournisseur : le dernier
    /// inscrit ; sans conversation (hors widget) : une NEUVE (l'accueil), paresseuse chez le
    /// fournisseur.
    pub fn nouvelle_session(&mut self) {
        self.position_posee = false;
        self.message_demande = None;
        if self.fournisseur.is_empty()
            && let Some(e) = self.ctx.fournisseurs.entries().last()
        {
            self.fournisseur = e.value.id().to_owned();
        }
        let p = self.fournisseur_actif();
        if let Some(p) = &p
            && self.conversation.is_empty()
            && self.placement.is_none()
            && let Some(id) = p.create()
        {
            self.conversation = id;
        }
        self.session = p.filter(|_| !self.conversation.is_empty()).map(|p| {
            let mut s = ChatSession::new(p, self.conversation.clone());
            let sale = self.sale.clone();
            s.on_change(move || sale.set(true));
            Rc::new(RefCell::new(s))
        });
        self.sale.set(true);
        self.revision += 1;
    }

    /// Envoie un tour : l'envoi d'une vue en barre va à l'hôte, sinon à la session.
    pub fn envoyer(&mut self, texte: &str, opts: Option<SendOpts>, parts: Vec<ChatPart>) {
        if self.mode() == ChatMode::Bar {
            self.soumissions.push_back(ChatBarSubmit {
                text: texte.to_owned(),
                opts,
                parts,
            });
            return;
        }
        if let Some(s) = self.session.clone() {
            s.borrow_mut().send(texte, opts, parts);
        }
    }

    // ── La barre des conversations ─────────────────────────────────────────────

    /// Le geste de l'utilisateur : ouvrir / replier la barre (persisté dans l'état).
    pub fn basculer_barre(&mut self) {
        self.barre_repliee_par_marge = false;
        self.poser_barre(!self.barre_ouverte);
    }

    /// Ouvre / replie la barre : la largeur GLISSE, rien n'est reconstruit.
    pub fn poser_barre(&mut self, ouverte: bool) {
        self.barre_ouverte = ouverte;
        let cible = if ouverte { LARGEUR_BARRE_OUVERTE } else { 0.0 };
        self.barre.aller_a(cible, (self.ctx.ms)(), DUREE_BARRE_MS);
    }

    /// Marge de lecture 1/4 – 3/4 : la colonne de la note commence trop à gauche pour que la barre
    /// tienne à côté. Elle se REPLIE seule, et se rouvre au retour si c'est la marge qui l'avait
    /// repliée. Rouverte à la main en 3/4, elle pousse la colonne.
    pub fn suivre_marge(&mut self, marge: bool) {
        if marge == self.marge_vue {
            return;
        }
        self.marge_vue = marge;
        if marge && self.barre_ouverte {
            self.barre_repliee_par_marge = true;
            self.poser_barre(false);
        } else if !marge && self.barre_repliee_par_marge {
            self.barre_repliee_par_marge = false;
            self.poser_barre(true);
        }
    }

    /// La marge est-elle ouverte sur la feuille, d'après le registre de la marge ?
    #[must_use]
    pub fn marge_ouverte(&self) -> bool {
        self.ctx
            .marge
            .as_ref()
            .is_some_and(|m| m.est_ouverte(self.leaf))
    }

    /// La réserve de la marge à cet instant (fraction de l'onglet) et si elle glisse ; en page
    /// seulement.
    #[must_use]
    pub fn reserve(&self) -> (f32, bool) {
        if self.mode() != ChatMode::Page {
            return (0.0, false);
        }
        self.ctx.marge.as_ref().map_or((0.0, false), |m| {
            let r = m.reserve(self.leaf, Genre::Note);
            (r.fraction, r.en_transition)
        })
    }

    // ── ⋯ : télécharger, copier, supprimer ─────────────────────────────────────

    /// Les entrées que la vue ajoute au menu ⋯ du pane (`onPaneMenu`).
    pub fn contribuer_au_menu(&self, menu: &mut MenuPane) {
        let (Some(s), Some(p)) = (self.session.clone(), self.fournisseur_actif()) else {
            return;
        };
        let vide = s.borrow().messages.is_empty();
        let titre = self.titre();
        let markdown = {
            let msgs: Vec<_> = s
                .borrow()
                .messages
                .iter()
                .map(|m| m.message.clone())
                .collect();
            conversation_markdown(&titre, &msgs, &p.participants(&self.conversation))
        };
        let file = self.actions.clone();
        let (md, nom) = (markdown.clone(), format!("{}.md", nom_de_fichier(&titre)));
        menu.add_item(|i| {
            i.set_title("Télécharger la conversation (.md)")
                .set_icon("download")
                .set_section(SECTION_ACTION)
                .set_disabled(vide)
                .on_click(move || {
                    file.borrow_mut().push_back(ActionChat::Telecharger {
                        nom: nom.clone(),
                        contenu: md.clone(),
                    });
                });
        });
        let file = self.actions.clone();
        menu.add_item(|i| {
            i.set_title("Copier la conversation")
                .set_icon("copy")
                .set_section(SECTION_ACTION)
                .set_disabled(vide)
                .on_click(move || {
                    file.borrow_mut()
                        .push_back(ActionChat::Copier(markdown.clone()));
                });
        });
        // Supprimer : seulement s'il y a quelque chose à supprimer.
        if !vide {
            let file = self.actions.clone();
            menu.add_item(|i| {
                i.set_title("Supprimer la conversation")
                    .set_icon("trash-2")
                    .set_section(SECTION_DANGER)
                    .set_warning(true)
                    .on_click(move || file.borrow_mut().push_back(ActionChat::Supprimer));
            });
        }
    }

    // ── L'origine ──────────────────────────────────────────────────────────────

    /// L'origine de la conversation (son fournisseur la connaît), ou `None`.
    #[must_use]
    pub fn origine(&self) -> Option<ChatOrigin> {
        self.fournisseur_actif()?.origin(&self.conversation)
    }

    /// La carte d'origine du PREMIER message de l'utilisateur : le document d'où vient la
    /// conversation. Aucune dans un widget (le passage est juste à côté) ; embarquée, un simple
    /// lien. L'embed du document est à T9 (`embedDocument`) : ici l'en-tête et la légende.
    #[must_use]
    pub fn carte_origine(&self) -> Option<CarteOrigine> {
        let mode = self.mode();
        if matches!(mode, ChatMode::Anchored | ChatMode::Bar) {
            return None;
        }
        let o = self.origine()?;
        let docs = &self.ctx.documents;
        Some(CarteOrigine {
            titre: docs.title(&o.doc),
            icone: docs.icon(&o.doc).unwrap_or_else(|| "file-text".to_owned()),
            manquant: !docs.exists(&o.doc),
            lien_seul: mode == ChatMode::Embedded,
            doc: o.doc.clone(),
            passage: Some(o),
        })
    }

    /// L'id du premier message de l'utilisateur (celui qui porte la carte).
    #[must_use]
    pub fn premier_message_utilisateur(&self) -> Option<String> {
        let s = self.session.as_ref()?.borrow();
        let moi = self
            .fournisseur_actif()?
            .participants(&self.conversation)
            .into_iter()
            .find(|p| p.role == Role::User)?
            .id;
        s.messages
            .iter()
            .find(|m| m.author == moi)
            .map(|m| m.id.clone())
    }
}

// ── Les gestes qui OUVRENT un document ─────────────────────────────────────────
//
// ★ Ouvrir un document dans la feuille passe par le Workspace, qui rappelle `set_state` de la vue
//   : l'état ne doit PAS être emprunté pendant l'appel. Ces gestes lisent ce qu'il leur faut, rendent
//   l'emprunt, puis ouvrent.

/// Ouvre une autre conversation DANS cette vue (historique ← →), comme ouvrir une note.
pub fn ouvrir_conversation(etat: &Rc<RefCell<EtatChat>>, id: &str) {
    let (ouvrir, leaf, cle) = {
        let mut e = etat.borrow_mut();
        if id == e.conversation {
            return;
        }
        e.historique = false;
        (e.ctx.ouvrir.clone(), e.leaf, chat_key(&e.fournisseur, id))
    };
    ouvrir(leaf, &cle, None);
}

/// « Nouvelle conversation » : un id neuf chez le fournisseur, ouvert dans la feuille.
pub fn nouvelle_conversation(etat: &Rc<RefCell<EtatChat>>) -> bool {
    let (ouvrir, leaf, cle) = {
        let mut e = etat.borrow_mut();
        let Some(id) = e.fournisseur_actif().and_then(|p| p.create()) else {
            return false;
        };
        e.historique = false;
        e.focus_demande = true;
        (e.ctx.ouvrir.clone(), e.leaf, chat_key(&e.fournisseur, &id))
    };
    ouvrir(leaf, &cle, None);
    true
}

/// Supprime la conversation chez son fournisseur, l'ANNONCE (événement de document : ses fils et
/// surlignages sont enterrés par la règle générique des artefacts) puis passe à une neuve.
pub fn supprimer_conversation(etat: &Rc<RefCell<EtatChat>>) -> bool {
    let (p, cle, conversation, documents) = {
        let e = etat.borrow();
        let (Some(p), Some(cle)) = (e.fournisseur_actif(), e.document()) else {
            return false;
        };
        (p, cle, e.conversation.clone(), e.ctx.documents.clone())
    };
    if p.delete(&conversation).is_err() {
        return false;
    }
    documents.notify(&DocumentEvent::Delete {
        key: DocumentKey::new(cle),
        origin: Some(EventOrigin::User),
        folder: false,
    });
    nouvelle_conversation(etat)
}

/// Ouvre la source À SON PASSAGE dans la feuille (historique ← →).
pub fn ouvrir_origine(etat: &Rc<RefCell<EtatChat>>, carte: &CarteOrigine) {
    if carte.manquant {
        return;
    }
    let (ouvrir, leaf) = {
        let e = etat.borrow();
        (e.ctx.ouvrir.clone(), e.leaf)
    };
    let e_state = carte.passage.as_ref().map(|o| {
        serde_json::json!({ "passage": {
            "from": o.from.unwrap_or(0), "to": o.to.unwrap_or(0), "quote": o.quote,
        } })
    });
    ouvrir(leaf, &carte.doc, e_state);
}

/// Un nom de fichier sûr d'après un titre.
#[must_use]
pub fn nom_de_fichier(titre: &str) -> String {
    let n: String = titre
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let n = n.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() {
        "conversation".to_owned()
    } else {
        n.chars().take(80).collect()
    }
}
