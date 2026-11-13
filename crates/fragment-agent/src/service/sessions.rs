// Port de app/src/ui/views/agent/SessionIndex.ts et de la part « sessions » de api.ts
//
// L'historique des sessions est LU au journal d'artefacts : une session est un record
// `agent:session` (scope plugin, collection `agent/sessions`) dont l'id est celui de la
// conversation. Rien n'est copié : la liste se recalcule de l'index, et l'abonnement au journal
// suit les écritures d'où qu'elles viennent (une autre fenêtre, un autre appareil).
//
// ★ Le transcript reste celui du moteur ; le record ne porte que les métadonnées (titre, dates,
//   modèle, `meta`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_artefacts::artefact_manager::SubscriptionId;
use fragment_artefacts::clock::Now;
use fragment_artefacts::{ArtefactManager, ArtefactScope, Fields, Filter, Origin, ScopeFilter};
use serde_json::{Value, json};

use crate::contrat::{AgentSessionInfo, Entree};
use crate::fournisseur::{Auteur, SessionAgent};

/// Le type d'artefact d'une session.
pub const TYPE_SESSION: &str = "agent:session";
/// La collection du plugin qui les range.
pub const COLLECTION_SESSIONS: &str = "agent/sessions";

/// Le journal partagé : l'index des sessions et les autres plugins y écrivent tous.
pub type Journal = Rc<RefCell<ArtefactManager>>;

/// Ce qu'une écriture change (`Partial<AgentSessionState>` du TS) ; l'absent ne change rien.
#[derive(Debug, Clone, Default)]
pub struct ModifSession {
    pub title: Option<String>,
    pub model: Option<String>,
    /// Remplace TOUTE la `meta` (le TS fait de même : l'appelant fusionne).
    pub meta: Option<Entree>,
}

type Ligne = (SessionAgent, Option<String>);
type Cache = Rc<RefCell<Option<(bool, Vec<Ligne>)>>>;

/// L'historique des sessions.
pub struct IndexSessions {
    journal: Journal,
    now: Now,
    /// Le cache, avec l'état `ready` du journal quand il a été calculé : un index lu avant la fin
    /// du démarrage est vide, il est recalculé dès que le journal est prêt.
    cache: Cache,
    version: Rc<Cell<u64>>,
    abonnement: Cell<Option<SubscriptionId>>,
}

impl IndexSessions {
    pub fn new(journal: Journal, now: Now) -> Self {
        Self {
            journal,
            now,
            cache: Rc::new(RefCell::new(None)),
            version: Rc::new(Cell::new(0)),
            abonnement: Cell::new(None),
        }
    }

    /// S'abonne au journal (une fois). Le rappel ne fait qu'invalider : il tourne pendant une
    /// écriture du journal, il ne doit jamais le rappeler.
    pub fn demarrer(&self) {
        if self.abonnement.get().is_some() {
            return;
        }
        let cache = Rc::clone(&self.cache);
        let version = Rc::clone(&self.version);
        let id = self.journal.borrow_mut().subscribe(
            Filter::scope(ScopeFilter::collection(COLLECTION_SESSIONS)),
            move |_, _| {
                *cache.borrow_mut() = None;
                version.set(version.get() + 1);
            },
        );
        self.abonnement.set(Some(id));
    }

    /// Le retrait de l'abonnement.
    pub fn arreter(&self) {
        if let Some(id) = self.abonnement.take() {
            self.journal.borrow_mut().unsubscribe(id);
        }
    }

    /// Un compteur qui grimpe à chaque écriture de l'index (le « changed » du TS : la vue le
    /// compare à sa dernière valeur au lieu de recevoir un rappel).
    pub fn version(&self) -> u64 {
        self.version.get()
    }

    fn lignes(&self) -> Vec<Ligne> {
        let pret = self.journal.borrow().ready();
        if let Some((p, v)) = self.cache.borrow().as_ref()
            && *p == pret
        {
            return v.clone();
        }
        let filtre = Filter {
            id: None,
            type_: Some(TYPE_SESSION.to_string()),
            scope: Some(ScopeFilter::collection(COLLECTION_SESSIONS)),
        };
        let mut v: Vec<Ligne> = self
            .journal
            .borrow()
            .list(Some(&filtre))
            .into_iter()
            .filter_map(|r| {
                let s = r.state.as_ref()?.as_object()?.clone();
                let nombre = |k: &str| {
                    s.get(k)
                        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                        .unwrap_or(0)
                };
                Some((
                    SessionAgent {
                        id: r.id,
                        title: s
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("Session")
                            .to_string(),
                        created_at: nombre("createdAt"),
                        updated_at: nombre("updatedAt"),
                        meta: s
                            .get("meta")
                            .and_then(Value::as_object)
                            .cloned()
                            .unwrap_or_default(),
                    },
                    s.get("model").and_then(Value::as_str).map(str::to_string),
                ))
            })
            .collect();
        // La plus récente d'abord ; à date égale l'ordre du journal (stable).
        v.sort_by_key(|(s, _)| std::cmp::Reverse(s.updated_at));
        *self.cache.borrow_mut() = Some((pret, v.clone()));
        v
    }

    /// Les sessions, la plus récente d'abord.
    pub fn liste(&self) -> Vec<SessionAgent> {
        self.lignes().into_iter().map(|(s, _)| s).collect()
    }

    pub fn get(&self, id: &str) -> Option<SessionAgent> {
        self.lignes()
            .into_iter()
            .find(|(s, _)| s.id == id)
            .map(|(s, _)| s)
    }

    /// La session telle que la lit un plugin tiers (`AgentService.getSession`).
    pub fn info(&self, id: &str) -> Option<AgentSessionInfo> {
        self.lignes()
            .into_iter()
            .find(|(s, _)| s.id == id)
            .map(|(s, model)| AgentSessionInfo {
                id: s.id,
                title: s.title,
                created_at: s.created_at,
                updated_at: s.updated_at,
                model,
                meta: (!s.meta.is_empty()).then_some(s.meta),
            })
    }

    /// Crée la session (au premier message) ou met à jour ses métadonnées, et la date.
    pub fn toucher(&self, id: &str, patch: ModifSession, auteur: Auteur) {
        let now = (self.now)();
        let cur = self.journal.borrow().get(id).and_then(|r| r.state);
        let existe = cur.is_some();
        let mut etat = Entree::new();
        etat.insert("title".into(), json!("Nouvelle session"));
        etat.insert("createdAt".into(), json!(now));
        if let Some(Value::Object(c)) = cur {
            etat.extend(c);
        }
        etat.insert("updatedAt".into(), json!(now));
        if let Some(t) = patch.title {
            etat.insert("title".into(), json!(t));
        }
        if let Some(m) = patch.model {
            etat.insert("model".into(), json!(m));
        }
        if let Some(m) = patch.meta {
            etat.insert("meta".into(), Value::Object(m));
        }
        let champs = if existe {
            Fields::new().state(Value::Object(etat))
        } else {
            Fields::new()
                .type_(TYPE_SESSION)
                .scope(ArtefactScope::plugin(COLLECTION_SESSIONS))
                .state(Value::Object(etat))
        };
        let origine = match auteur {
            Auteur::Utilisateur => Origin::User,
            Auteur::Plugin => Origin::Plugin,
        };
        self.journal.borrow_mut().set(Some(id), champs, origine);
    }

    /// Retire la session de l'historique (une tombe ; le transcript du moteur reste).
    pub fn supprimer(&self, id: &str) {
        self.journal.borrow_mut().del(id, Origin::User);
    }
}
