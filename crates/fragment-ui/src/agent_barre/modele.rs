// Port de app/src/ui/views/agent/bar/AgentBar.ts (l'état, le lancement, la cible, rendre le clavier).
//! Le MODÈLE de la barre de l'agent d'une vue, sans GPUI : l'état (repliée / ouverte), la
//! conversation « modèle » qui garde les choix du champ, l'écran partagé réutilisé, et le geste
//! qui compte, ENTRÉE : une conversation NEUVE ouverte à côté du document ou au panneau.
//!
//! ★ Un LANCEUR (décisions de l'utilisateur, 31/10) : le champ seul, ni fil ni carte. La barre
//!   garde une conversation « modèle », jamais envoyée, qui porte les choix du champ (moteur,
//!   modèle, effort, mode) ; chaque lancement les recopie sur la neuve (`copy_pending`).
//! ★ L'écran partagé ouvert par la barre est RÉUTILISÉ tant qu'il existe et montre une
//!   conversation : son historique ← → garde les précédentes.
//! ★ Replier REND le clavier au document : le champ avait fait du widget l'actif hébergé, sa
//!   portée restait au-dessus de celle du document (Ctrl+F d'un PDF muet jusqu'au clic suivant).
//!
//! Tout ce que la barre demande au workspace passe par [`HoteBarre`], tout ce qu'elle demande au
//! fournisseur par [`FournisseurBarre`] : les tests branchent des doubles, le montage les vrais.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_chat::api::{CHAT_VIEW_TYPE, ChatBarSubmit};
use fragment_chat::chat_key::chat_key;
use fragment_core::ItemId;
use serde_json::{Map, Value, json};

use crate::chat_vue::EtatChat;

/// Le calque auquel la barre appartient : son œil la masque avec les Side Chats.
pub const CALQUE_AGENT: &str = "agent";

/// La largeur maximale du champ (`--agent-bar-width`).
pub const LARGEUR_CHAMP: f32 = 720.0;
/// L'espace gardé de chaque côté quand le pane est plus étroit que le champ (`100% - 32px`).
pub const GOUTTIERE_CHAMP: f32 = 32.0;

/// Replié ou ouvert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EtatBarre {
    Repliee,
    Ouverte,
}

/// Où Entrée ouvre la conversation (réglage `barTarget`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cible {
    /// À côté du document (écran partagé) : le défaut.
    #[default]
    Split,
    /// Au panneau de droite.
    Dock,
}

impl Cible {
    /// Depuis le réglage persisté (`"dock"` ; tout le reste vaut l'écran partagé).
    #[must_use]
    pub fn depuis_reglage(v: Option<&str>) -> Self {
        if v == Some("dock") {
            Self::Dock
        } else {
            Self::Split
        }
    }

    /// Ce qu'on persiste : rien pour le défaut.
    #[must_use]
    pub fn reglage(self) -> Option<&'static str> {
        match self {
            Self::Split => None,
            Self::Dock => Some("dock"),
        }
    }

    /// L'icône de la bascule : elle DIT où Entrée ouvre la conversation.
    #[must_use]
    pub fn icone(self) -> &'static str {
        match self {
            Self::Split => "columns-2",
            Self::Dock => "panel-right",
        }
    }

    /// L'étiquette de la bascule : la destination d'abord, puis ce que le clic change.
    #[must_use]
    pub fn libelle(self) -> &'static str {
        match self {
            Self::Split => {
                "Entrée ouvre la conversation à côté (écran partagé) — cliquer : au panneau de droite"
            }
            Self::Dock => {
                "Entrée ouvre la conversation au panneau de droite — cliquer : à côté (écran partagé)"
            }
        }
    }

    /// L'autre destination.
    #[must_use]
    pub fn autre(self) -> Self {
        match self {
            Self::Split => Self::Dock,
            Self::Dock => Self::Split,
        }
    }
}

/// Les réglages de la barre (persistés par le plugin agent : `barTarget`, `barHidden`, `barStart`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reglages {
    pub cible: Cible,
    /// Masquée par l'œil de sa ligne du menu « Calques ».
    pub masquee: bool,
    /// Ouverte au départ (`barStart == "open"`) ; sinon repliée.
    pub depart_ouverte: bool,
}

/// Ce que la barre demande au fournisseur agent.
pub trait FournisseurBarre {
    /// L'id du fournisseur (`agent`).
    fn id(&self) -> String;
    /// Une conversation neuve (pas encore créée chez le fournisseur : paresseuse).
    fn create(&self) -> Option<String>;
    /// Recopie les CHOIX en attente de `de` sur `vers` (pas la `meta`).
    fn copy_pending(&self, de: &str, vers: &str);
    /// Des métadonnées pour une conversation pas encore créée, écrites avec sa session.
    fn set_pending_meta(&self, conversation: &str, meta: Map<String, Value>);
}

/// Ce que la barre demande au workspace.
pub trait HoteBarre {
    /// La feuille active (la cible du raccourci).
    fn feuille_active(&self) -> Option<ItemId>;
    /// Une vue de DOCUMENT d'une feuille du workspace (pas une conversation : elle a son champ).
    fn est_document(&self, vue: ItemId) -> bool;
    /// La clé du document de la vue.
    fn document(&self, vue: ItemId) -> Option<String>;
    /// L'écran partagé où ouvrir : `reutilisable` s'il est encore là, au centre et montre une
    /// conversation ; sinon une feuille neuve à droite de `depuis`.
    fn ecran_partage(&self, depuis: ItemId, reutilisable: Option<ItemId>) -> Option<ItemId>;
    /// La feuille du panneau de droite : une conversation déjà là, sinon une neuve.
    fn feuille_dock(&self) -> Option<ItemId>;
    /// Ouvre un document dans la feuille avec cet état éphémère.
    fn ouvrir(&self, feuille: ItemId, cle: &str, e_state: Map<String, Value>) -> bool;
    /// Rend la feuille visible et active.
    fn reveler(&self, feuille: ItemId);
    /// Le clavier revient au document de la vue (le focus quitte le champ).
    fn rendre_le_clavier(&self, vue: ItemId);
}

/// Ce que la barre commande à sa présentation (GPUI) ; chacun est facultatif.
#[derive(Clone, Default)]
pub struct Crochets {
    /// La vue chat du champ est montée au premier dépli (une pastille ne coûte rien) ; `bool` :
    /// avec le focus. Reçoit l'id de la conversation modèle.
    pub monter: Option<Rc<dyn Fn(&str, bool)>>,
    /// Donne le focus au champ.
    pub focaliser: Option<Rc<dyn Fn()>>,
    /// Le focus est-il dans le champ ?
    pub a_le_focus: Option<Rc<dyn Fn() -> bool>>,
    /// Le widget cesse d'être l'actif hébergé (sa portée de clavier part).
    pub relacher: Option<Rc<dyn Fn()>>,
}

/// Le contexte partagé de toutes les barres.
#[derive(Clone)]
pub struct ContexteBarre {
    pub fournisseur: Rc<dyn FournisseurBarre>,
    pub hote: Rc<dyn HoteBarre>,
    pub reglages: Rc<RefCell<Reglages>>,
    /// Persiste les réglages (le plugin agent les écrit dans ses données).
    pub sauver: Rc<dyn Fn(&Reglages)>,
}

impl ContexteBarre {
    /// Change la destination d'Entrée (écran partagé ↔ panneau) et la persiste.
    pub fn basculer_cible(&self) -> Cible {
        let c = {
            let mut r = self.reglages.borrow_mut();
            r.cible = r.cible.autre();
            *r
        };
        (self.sauver)(&c);
        c.cible
    }
}

/// La barre d'une vue.
pub struct BarreAgent {
    vue: ItemId,
    ctx: ContexteBarre,
    etat: Cell<EtatBarre>,
    masquee: Cell<bool>,
    /// La conversation « modèle » du champ : jamais envoyée, elle porte ses choix.
    modele: RefCell<Option<String>>,
    /// L'écran partagé ouvert par la barre (réutilisé tant qu'il existe).
    split: Cell<Option<ItemId>>,
    /// La dernière bascule vient d'un geste : l'ouverture s'anime.
    anime: Cell<bool>,
    crochets: RefCell<Crochets>,
    detruite: Cell<bool>,
}

impl BarreAgent {
    /// Une barre pour `vue`, dans l'état de départ.
    #[must_use]
    pub fn new(vue: ItemId, ctx: ContexteBarre, depart: EtatBarre) -> Rc<Self> {
        let masquee = ctx.reglages.borrow().masquee;
        let b = Rc::new(Self {
            vue,
            ctx,
            etat: Cell::new(depart),
            masquee: Cell::new(masquee),
            modele: RefCell::new(None),
            split: Cell::new(None),
            anime: Cell::new(false),
            crochets: RefCell::new(Crochets::default()),
            detruite: Cell::new(false),
        });
        if depart == EtatBarre::Ouverte {
            b.monter(false);
        }
        b
    }

    /// La vue qui porte la barre.
    #[must_use]
    pub fn vue(&self) -> ItemId {
        self.vue
    }

    /// Branche la présentation.
    pub fn poser_crochets(&self, c: Crochets) {
        *self.crochets.borrow_mut() = c;
    }

    #[must_use]
    pub fn etat(&self) -> EtatBarre {
        self.etat.get()
    }

    #[must_use]
    pub fn est_ouverte(&self) -> bool {
        self.etat.get() == EtatBarre::Ouverte
    }

    /// La dernière bascule s'anime-t-elle ? (`data-anime`)
    #[must_use]
    pub fn anime(&self) -> bool {
        self.anime.get()
    }

    /// La conversation modèle (la vue chat du champ la montre).
    #[must_use]
    pub fn modele(&self) -> Option<String> {
        self.modele.borrow().clone()
    }

    #[must_use]
    pub fn masquee(&self) -> bool {
        self.masquee.get()
    }

    /// Visible = un document dans la vue, et pas masquée. (Le document arrive APRÈS le montage des
    /// calques : la barre l'attend.)
    #[must_use]
    pub fn visible(&self) -> bool {
        !self.detruite.get() && !self.masquee.get() && self.ctx.hote.document(self.vue).is_some()
    }

    /// Masquer / afficher (l'œil de sa ligne du menu « Calques »). Masquée, elle n'existe plus à
    /// l'écran : le focus du champ est lâché.
    pub fn poser_masquee(&self, m: bool) {
        self.masquee.set(m);
        if m && self.focus_dans_le_champ() {
            self.rendre_le_clavier();
        }
    }

    /// Ouvre / replie (la commande, la pastille, Échap). `focus` : le champ prend le focus.
    pub fn definir_etat(&self, s: EtatBarre, focus: bool) {
        if s == self.etat.get() {
            if s == EtatBarre::Ouverte && focus {
                self.focaliser();
            }
            return;
        }
        self.etat.set(s);
        self.anime.set(true);
        if s == EtatBarre::Ouverte {
            self.monter(focus);
        } else {
            self.rendre_le_clavier();
        }
    }

    /// Le raccourci : ouvre si repliée, replie si ouverte ; le champ prend le focus à l'ouverture.
    pub fn basculer(&self) {
        let suivant = if self.est_ouverte() {
            EtatBarre::Repliee
        } else {
            EtatBarre::Ouverte
        };
        self.definir_etat(suivant, true);
    }

    fn focus_dans_le_champ(&self) -> bool {
        let f = self.crochets.borrow().a_le_focus.clone();
        f.is_some_and(|f| f())
    }

    fn focaliser(&self) {
        let f = self.crochets.borrow().focaliser.clone();
        if let Some(f) = f {
            f();
        }
    }

    /// La conversation modèle est créée au premier dépli ; la présentation monte la vue chat.
    fn monter(&self, focus: bool) {
        if self.ctx.hote.document(self.vue).is_none() {
            return;
        }
        let neuve = {
            let mut m = self.modele.borrow_mut();
            if m.is_none() {
                *m = self.ctx.fournisseur.create();
                true
            } else {
                false
            }
        };
        if !neuve {
            if focus {
                self.focaliser();
            }
            return;
        }
        let (id, monter) = (self.modele(), self.crochets.borrow().monter.clone());
        if let (Some(id), Some(monter)) = (id, monter) {
            monter(&id, focus);
        }
    }

    /// ★ Repliée : le clavier REVIENT au document. On relâche d'abord l'actif hébergé (sa portée
    ///   part), puis, si le focus était dans le champ, le texte du document le reprend.
    fn rendre_le_clavier(&self) {
        let avait = self.focus_dans_le_champ();
        let relacher = self.crochets.borrow().relacher.clone();
        if let Some(r) = relacher {
            r();
        }
        if avait {
            self.ctx.hote.rendre_le_clavier(self.vue);
        }
    }

    /// Les demandes du champ hébergé : Entrée (lancer), Échap dans le champ vide (replier). À
    /// appeler à chaque image par la présentation, avec l'état de la vue chat du champ.
    pub fn pomper(&self, champ: &Rc<RefCell<EtatChat>>) {
        let (envois, echap) = {
            let mut e = champ.borrow_mut();
            (
                std::mem::take(&mut e.soumissions),
                std::mem::take(&mut e.echappe),
            )
        };
        for envoi in envois {
            self.lancer(&envoi);
        }
        if echap {
            self.definir_etat(EtatBarre::Repliee, true);
        }
    }

    /// Le bouton « Replier » de la barre du champ.
    pub fn replier(&self) {
        self.definir_etat(EtatBarre::Repliee, true);
    }

    /// La bascule de destination (le bouton du champ) : persistée.
    pub fn basculer_cible(&self) -> Cible {
        self.ctx.basculer_cible()
    }

    /// La destination courante.
    #[must_use]
    pub fn cible_courante(&self) -> Cible {
        self.ctx.reglages.borrow().cible
    }

    /// ENTRÉE dans le champ : une conversation NEUVE (les choix du champ recopiés, rangée pour ce
    /// document), ouverte à côté ou au panneau, son premier message envoyé.
    pub fn lancer(&self, envoi: &ChatBarSubmit) -> bool {
        if self.detruite.get() {
            return false;
        }
        let (Some(doc), Some(modele)) = (self.ctx.hote.document(self.vue), self.modele()) else {
            return false;
        };
        let Some(id) = self.ctx.fournisseur.create() else {
            return false;
        };
        self.ctx.fournisseur.copy_pending(&modele, &id);
        let mut meta = Map::new();
        meta.insert("bar".into(), json!(true));
        meta.insert("origin".into(), json!({ "doc": doc }));
        self.ctx.fournisseur.set_pending_meta(&id, meta);
        let Some(feuille) = self.destination() else {
            return false;
        };
        let mut message = Map::new();
        message.insert("text".into(), json!(envoi.text));
        if let Some(o) = &envoi.opts {
            message.insert("opts".into(), Value::Object(o.clone()));
        }
        message.insert(
            "parts".into(),
            serde_json::to_value(&envoi.parts).unwrap_or_else(|_| json!([])),
        );
        let mut e_state = Map::new();
        e_state.insert("send".into(), Value::Object(message));
        e_state.insert("focus".into(), json!(true));
        let cle = chat_key(&self.ctx.fournisseur.id(), &id);
        let ouvert = self.ctx.hote.ouvrir(feuille, &cle, e_state);
        if ouvert {
            self.ctx.hote.reveler(feuille);
        }
        ouvert
    }

    /// Où la conversation s'ouvre : l'écran partagé de la barre (réutilisé), ou le panneau.
    fn destination(&self) -> Option<ItemId> {
        if self.ctx.reglages.borrow().cible == Cible::Dock {
            return self.ctx.hote.feuille_dock();
        }
        let f = self.ctx.hote.ecran_partage(self.vue, self.split.get())?;
        self.split.set(Some(f));
        Some(f)
    }

    /// L'écran partagé que la barre a ouvert (le test le lit).
    #[must_use]
    pub fn ecran_partage(&self) -> Option<ItemId> {
        self.split.get()
    }

    /// La vue se ferme.
    pub fn detruire(&self) {
        self.detruite.set(true);
    }
}

/// Le type de vue d'une conversation (la barre ne se monte pas sur elle).
#[must_use]
pub fn est_conversation(type_vue: &str) -> bool {
    type_vue == CHAT_VIEW_TYPE
}
