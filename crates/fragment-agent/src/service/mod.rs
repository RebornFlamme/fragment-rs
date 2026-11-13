// Port de app/src/ui/views/agent/AgentPlugin.ts (hors interface), SessionIndex.ts, api.ts
//
// Le SERVICE de l'agent : ce que le plugin `agent` de Fragment fait hors de l'interface.
//
// - [`sessions`] : l'historique, lu au journal d'artefacts (`agent:session`) ;
// - [`reglages`] : le `data.json` de l'agent (modèle, moteur, nom, orbe, politique) ;
// - [`dossiers`] : le dossier de l'agent de l'utilisateur et ceux de ses captures et pièces ;
// - [`vues`] : le trait d'hôte par où `open_chat` et `open_anchored` demandent le dock et les
//   widgets (branché au montage) ;
// - ici, [`ServiceAgent`] : le RUN COMMUN (il implémente `AgentHote`, le trait que le fournisseur
//   du chat appelle), la politique de permissions et l'`AgentService` des plugins tiers.
//
// ★ Le run commun garde ce qui est partagé par tous les moteurs : la session (artefact), l'état
//   du harness, les consignes, les dossiers hors coffre, la politique. Le moteur d'une conversation
//   est FIGÉ dès son premier tour (`meta.started`) ; son état opaque (`meta.harnessState`) est
//   persisté dans la session et relu au tour suivant.
// ★ Le prompt et les skills ne sont jamais livrés par fragment-rs (décision de l'orchestrateur,
//   ADR-007) : ils sont LUS au lancement dans le dossier de l'agent de l'utilisateur, relus à
//   chaque tour. Les tests passent un prompt factice.
// ★ Écart assumé : appels synchrones (voir `contrat.rs`). `lancer` rend la main à la fin du tour ;
//   l'interruption passe par un crochet que le montage branche sur le moteur.

pub mod dossiers;
pub mod reglages;
pub mod sessions;
pub mod vues;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use fragment_artefacts::clock::Now;
use fragment_artefacts::device::random_uuid;
use fragment_chat::api::chat_message_at;
use fragment_chat::chat_key::parse_chat_key;
use serde_json::{Value, json};

use crate::contrat::{
    AgentChatOptions, AgentEvent, AgentHarness, AgentPolicy, AgentQuery, AgentQueryRun,
    AgentService, AgentSessionInfo, DossierExterne, Entree, ErreurHarness, FolderAccess,
    ForkSource, GuardDecision, HarnessConversation, HarnessIO, HarnessTurn, PermanentRule,
    PermissionDecision, PermissionMode, PermissionRequest, RememberScope, ToolUse,
};
use crate::fournisseur::pieces::CodecImage;
use crate::fournisseur::{
    AgentHote, Auteur, DemandeTour, ID_FOURNISSEUR, SessionAgent, SortieTour,
};
use crate::harness_registry::{
    CLAUDE_HARNESS_ID, HarnessMeta, HarnessRegistry, Jeton, harness_id_for,
};
use crate::policy_model::{DecideContext, Decision, decide, rule_for};
use crate::skills_model::render_prompt;

use dossiers::Dossiers;
use reglages::{Reglages, StockageReglages, nom_utilisateur};
use sessions::{IndexSessions, Journal, ModifSession};
use vues::{EnvoiAncre, EtatOuverture, HoteVues, OuvertureAncree, OuvertureChat};

pub use crate::fournisseur::titre_depuis;

/// Ce que le service tient de l'app.
#[derive(Debug, Clone, Default)]
pub struct ConfigService {
    /// La racine du coffre (le dossier de travail de tout moteur) ; `None` hors bureau.
    pub racine_coffre: Option<String>,
    pub nom_coffre: String,
    /// `<userData>`.
    pub donnees: PathBuf,
    /// Le dossier personnel de l'utilisateur (le fichier de plan de Claude Code est sous `.claude/plans`).
    pub dossier_personnel: Option<PathBuf>,
    /// Le dossier de l'agent de l'installation de Fragment, lu à défaut du sien.
    pub installation_fragment: Option<PathBuf>,
    /// Les consignes de l'hôte, quand le dossier de l'agent n'a pas de `prompt.md`. ★ Le vrai
    /// prompt de Fragment n'est jamais livré : l'app y met ce qu'elle décide, les tests un factice.
    pub consignes: String,
    /// Le compte du système (`USERNAME`, `USER`), pour le nom par défaut.
    pub compte: Option<String>,
}

type Pompe = Box<dyn FnMut() -> bool>;
type Interruption = Rc<dyn Fn(&str)>;

/// Le service de l'agent.
pub struct ServiceAgent {
    config: ConfigService,
    sessions: IndexSessions,
    reglages: Reglages,
    dossiers: Dossiers,
    registre: RefCell<HarnessRegistry>,
    interruptions: RefCell<HashMap<String, Interruption>>,
    vues: RefCell<Option<Rc<dyn HoteVues>>>,
    codec: RefCell<Option<Rc<dyn CodecImage>>>,
    pompe: RefCell<Option<Pompe>>,
    now: Now,
}

impl ServiceAgent {
    /// `now` : l'horloge (ms) des sessions et de l'hôte ; `Rc::new(maintenant_systeme)` en prod.
    pub fn new(
        config: ConfigService,
        journal: Journal,
        stockage: Rc<dyn StockageReglages>,
        now: Now,
    ) -> Self {
        let dossiers = Dossiers::new(
            config.donnees.clone(),
            &config.nom_coffre,
            config.installation_fragment.clone(),
        );
        Self {
            sessions: IndexSessions::new(journal, now.clone()),
            reglages: Reglages::new(stockage),
            dossiers,
            registre: RefCell::new(HarnessRegistry::new()),
            interruptions: RefCell::new(HashMap::new()),
            vues: RefCell::new(None),
            codec: RefCell::new(None),
            pompe: RefCell::new(None),
            now,
            config,
        }
    }

    /// S'abonne au journal des sessions.
    pub fn demarrer(&self) {
        self.sessions.demarrer();
    }

    pub fn arreter(&self) {
        self.sessions.arreter();
    }

    pub fn sessions(&self) -> &IndexSessions {
        &self.sessions
    }

    pub fn reglages(&self) -> &Reglages {
        &self.reglages
    }

    pub fn dossiers(&self) -> &Dossiers {
        &self.dossiers
    }

    pub fn config(&self) -> &ConfigService {
        &self.config
    }

    // ── Les moteurs ──

    /// Inscrit un moteur (`agent:harness@1`) ; le natif Claude à [`crate::harness_claude::PRIORITE`].
    pub fn inscrire_harness(&self, id: &str, h: Rc<dyn AgentHarness>, priorite: i32) -> Jeton {
        self.registre.borrow_mut().register(id, h, priorite)
    }

    pub fn retirer_harness(&self, id: &str, jeton: Jeton) {
        self.registre.borrow_mut().unregister(id, jeton);
    }

    /// Les moteurs inscrits (l'onglet Agent des réglages les liste).
    pub fn harnesses(&self) -> Vec<Rc<dyn AgentHarness>> {
        self.registre.borrow().harness_list()
    }

    /// Comment interrompre un tour de ce moteur : le montage y branche `HarnessClaude::interrompre`.
    pub fn brancher_interruption(&self, harness: &str, f: impl Fn(&str) + 'static) {
        self.interruptions
            .borrow_mut()
            .insert(harness.to_string(), Rc::new(f));
    }

    // ── Le branchement à l'interface ──

    pub fn brancher_vues(&self, v: Rc<dyn HoteVues>) {
        *self.vues.borrow_mut() = Some(v);
    }

    pub fn brancher_codec(&self, c: Rc<dyn CodecImage>) {
        *self.codec.borrow_mut() = Some(c);
    }

    /// Ce qui avance pendant qu'une carte de permission attend (la boucle d'événements).
    pub fn brancher_pompe(&self, f: impl FnMut() -> bool + 'static) {
        *self.pompe.borrow_mut() = Some(Box::new(f));
    }

    // ── Le moteur d'une conversation, son modèle ──

    /// Le moteur d'une conversation : celui de sa session (JAMAIS changé), sinon le choix en
    /// attente `voulu`, sinon le défaut des réglages.
    pub fn harness_id_de(&self, session: &str, voulu: Option<&str>) -> String {
        let s = self.sessions.get(session);
        let meta = s.as_ref().map(|s| HarnessMeta {
            harness: s
                .meta
                .get("harness")
                .and_then(Value::as_str)
                .map(str::to_string),
            harness_state: s.meta.get("harnessState").cloned(),
        });
        harness_id_for(
            meta.as_ref(),
            s.is_some(),
            voulu,
            self.reglages.harness().as_deref(),
        )
    }

    /// Le modèle par défaut d'un moteur : le réglage, sinon celui qu'il déclare par défaut,
    /// sinon le premier. Claude lit lui-même le réglage `model` : `None`.
    pub fn modele_par_defaut(&self, h: &dyn AgentHarness) -> Option<String> {
        if h.id() == CLAUDE_HARNESS_ID {
            return None;
        }
        if let Some(m) = self.reglages.modele_du_harness(h.id()) {
            return Some(m);
        }
        let modeles = h.models();
        modeles
            .iter()
            .find(|m| m.default == Some(true))
            .or(modeles.first())
            .map(|m| m.id.clone())
    }

    // ── La politique ──

    /// La politique des réglages, complète (« Coffre ouvert » par défaut).
    pub fn politique_de_base(&self) -> AgentPolicy {
        self.reglages.politique()
    }

    /// La politique d'un tour : celle des réglages au MODE de la conversation (son choix, sinon
    /// `voulu`, sinon celui des réglages). Un moteur qui ne sait pas DEMANDER reste en accès
    /// complet (rien ne serait jamais répondu).
    pub fn politique_pour(
        &self,
        session: &str,
        voulu: Option<PermissionMode>,
        sait_demander: bool,
    ) -> AgentPolicy {
        let base = self.politique_de_base();
        let choisi = self
            .sessions
            .get(session)
            .and_then(|s| s.meta.get("profile")?.get("mode").cloned())
            .and_then(|m| serde_json::from_value::<PermissionMode>(m).ok());
        let mut mode = choisi.or(voulu).unwrap_or(base.mode);
        if !sait_demander {
            mode = PermissionMode::Full;
        }
        AgentPolicy { mode, ..base }
    }

    /// Les « toujours » d'une conversation (`meta.permanent`).
    fn regles_de_conversation(&self, session: &str) -> Vec<PermanentRule> {
        self.sessions
            .get(session)
            .and_then(|s| s.meta.get("permanent").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    /// Le contexte de décision : le coffre, les dossiers que l'app donne elle-même à l'agent, les
    /// « toujours » de la conversation. Relu à chaque usage.
    fn contexte_de_decision(&self, session: &str) -> DecideContext {
        let chemin = |p: PathBuf| p.to_string_lossy().into_owned();
        let mut agent_dirs = vec![
            DossierExterne {
                path: chemin(self.dossiers.ecrans()),
                access: FolderAccess::Read,
            },
            DossierExterne {
                path: chemin(self.dossiers.pieces()),
                access: FolderAccess::Read,
            },
            DossierExterne {
                path: chemin(self.dossiers.actif()),
                access: FolderAccess::Write,
            },
        ];
        // Le fichier de PLAN de Claude Code (mode plan) : hors du coffre, écrit par le moteur lui-même.
        if let Some(home) = &self.config.dossier_personnel {
            agent_dirs.push(DossierExterne {
                path: format!(
                    "{}/.claude/plans",
                    home.to_string_lossy().replace('\\', "/")
                ),
                access: FolderAccess::Write,
            });
        }
        DecideContext {
            vault_root: self.config.racine_coffre.clone().unwrap_or_default(),
            agent_dirs,
            conversation_rules: self.regles_de_conversation(session),
        }
    }

    /// Retient un « toujours » : dans la conversation (sa session) ou dans le coffre (les réglages).
    /// Sans effet si la règle existe déjà.
    pub fn retenir(
        &self,
        session: &str,
        portee: RememberScope,
        regle: &PermanentRule,
    ) -> std::io::Result<()> {
        let egal = |a: &PermanentRule| {
            a.family == regle.family
                && a.pattern.as_deref().unwrap_or("") == regle.pattern.as_deref().unwrap_or("")
        };
        match portee {
            RememberScope::Conversation => {
                let Some(s) = self.sessions.get(session) else {
                    return Ok(());
                };
                let mut avant = self.regles_de_conversation(session);
                if avant.iter().any(egal) {
                    return Ok(());
                }
                avant.push(regle.clone());
                let mut meta = s.meta;
                meta.insert(
                    "permanent".into(),
                    serde_json::to_value(&avant).map_err(std::io::Error::other)?,
                );
                self.sessions.toucher(
                    session,
                    ModifSession {
                        meta: Some(meta),
                        ..Default::default()
                    },
                    Auteur::Utilisateur,
                );
                Ok(())
            }
            RememberScope::Vault => {
                let mut p = self.politique_de_base();
                if p.permanent.iter().any(egal) {
                    return Ok(());
                }
                p.permanent.push(regle.clone());
                self.reglages.poser_politique(&p)
            }
        }
    }

    /// Un plan lancé : la suite de la conversation passe en « modifs auto ».
    fn passer_en_modifs_auto(&self, session: &str) {
        let Some(s) = self.sessions.get(session) else {
            return;
        };
        let mut meta = s.meta;
        let mut profil = meta
            .get("profile")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        profil.insert("mode".into(), json!("acceptEdits"));
        meta.insert("profile".into(), Value::Object(profil));
        self.sessions.toucher(
            session,
            ModifSession {
                meta: Some(meta),
                ..Default::default()
            },
            Auteur::Plugin,
        );
    }

    /// Le nom de l'utilisateur (avatar des chats) : le réglage, sinon le compte du système.
    pub fn nom(&self) -> String {
        nom_utilisateur(
            self.reglages.nom_saisi().as_deref(),
            self.config.compte.as_deref(),
        )
    }

    /// Les consignes du tour : le `prompt.md` du dossier de l'agent s'il y en a un, sinon celles
    /// de l'hôte ; `{{vault}}` et `{{dir}}` remplacés.
    fn consignes(&self, dossier: &crate::harness_claude::dossier_agent::DossierAgent) -> String {
        let vars = [
            ("vault".to_string(), self.config.nom_coffre.clone()),
            ("dir".to_string(), dossier.dir.clone()),
        ]
        .into_iter()
        .collect();
        render_prompt(
            dossier.prompt.as_deref().unwrap_or(&self.config.consignes),
            &vars,
        )
    }

    /// Écrit l'état opaque du harness dans la session (relu au tour suivant).
    fn poser_etat(&self, session: &str, etat: Value) {
        if let Some(s) = self.sessions.get(session) {
            let mut meta = s.meta;
            meta.insert("harnessState".into(), etat);
            self.sessions.toucher(
                session,
                ModifSession {
                    meta: Some(meta),
                    ..Default::default()
                },
                Auteur::Plugin,
            );
        }
    }

    // ── openChat / openAnchored (côté données) ──

    /// Montre le chat de l'agent dans le dock droit : la conversation demandée, sinon celle déjà
    /// affichée, sinon la plus récente — et envoie le prompt s'il y en a un. `new_session` : une
    /// conversation NEUVE, dans un chat libre du dock (un tour en cours garde sa vue).
    pub fn ouvrir_chat(&self, prompt: Option<&str>, opts: &AgentChatOptions) {
        let Some(vues) = self.vues.borrow().clone() else {
            return;
        };
        let chats = vues.chats_dock();
        let rang = if opts.new_session {
            chats.iter().position(|c| !c.occupe)
        } else if chats.is_empty() {
            None
        } else {
            Some(0)
        };
        let actuelle = rang.map(|i| chats[i].conversation.clone());
        let conversation = if opts.new_session {
            self.nouvel_id()
        } else {
            opts.session_id
                .clone()
                .or(actuelle)
                .or_else(|| self.sessions.liste().first().map(|s| s.id.clone()))
                .unwrap_or_else(|| self.nouvel_id())
        };
        vues.montrer_chat(OuvertureChat {
            reutiliser: rang,
            conversation,
            prompt: prompt.map(str::to_string),
            effort: opts.effort.clone(),
        });
    }

    /// Une conversation ANCRÉE à un passage : la session est créée avec son profil, son origine
    /// et ce qu'il faut AFFICHER de la première question ; la vue s'ouvre dans un widget. Rend l'id
    /// de la session (`None` : pas d'hôte de widgets, rien n'est écrit).
    ///
    /// ★ Un passage d'une de NOS conversations est une BRANCHE : la session neuve naîtra de
    ///   celle-ci (fork au premier envoi) et hérite de son historique réel. Une branche naît du
    ///   MÊME moteur que son parent ; un moteur qui ne sait pas brancher donne une conversation racine.
    pub fn ouvrir_ancre(&self, o: &OuvertureAncree) -> Option<String> {
        let vues = self.vues.borrow().clone()?;
        let session_id = o.session_id.clone().unwrap_or_else(|| self.nouvel_id());
        let parent = parse_chat_key(&o.file).filter(|k| {
            k.provider == ID_FOURNISSEUR && self.sessions.get(&k.conversation).is_some()
        });
        let harness_parent = parent
            .as_ref()
            .map(|p| self.harness_id_de(&p.conversation, None));
        let peut_brancher = harness_parent
            .as_deref()
            .and_then(|h| self.harness(h))
            .is_some_and(|h| h.capabilities().fork);
        let branche = match (&o.session_id, &parent) {
            (None, Some(p)) if peut_brancher => {
                Some((p.conversation.clone(), chat_message_at(o.from)))
            }
            _ => None,
        };
        let harness_id = match (&o.session_id, &branche, &harness_parent) {
            (Some(id), _, _) => self.harness_id_de(id, None),
            (None, Some(_), Some(h)) => h.clone(),
            _ => self.harness_id_de(&session_id, None),
        };
        let non_vide = |s: &Option<String>| s.clone().filter(|s| !s.is_empty());
        let preambule = match (&branche, non_vide(&o.preamble)) {
            (Some(_), Some(_)) => {
                let citation = o
                    .quote
                    .as_deref()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                Some(format!(
                    "À propos de « {citation} » (un passage de la conversation ci-dessus) :"
                ))
            }
            (_, p) => p,
        };
        let pieces = o.attachments.clone();
        let etat = if let Some(prompt) = non_vide(&o.prompt) {
            Some(EtatOuverture {
                envoi: Some(EnvoiAncre {
                    texte: non_vide(&o.display).unwrap_or_else(|| prompt.clone()),
                    prompt,
                    pieces,
                }),
                focus: true,
                ..Default::default()
            })
        } else if preambule.is_some() || o.draft.is_some() || !pieces.is_empty() {
            Some(EtatOuverture {
                envoi: None,
                focus: true,
                brouillon: non_vide(&o.draft),
                pieces_du_champ: pieces,
            })
        } else {
            None
        };
        // ★ La session n'est écrite QUE si le widget existe (sinon : une session orpheline dans
        //   l'historique).
        if !vues.ouvrir_ancre(o, &session_id, etat) {
            return None;
        }
        let mut meta = o.meta.clone().unwrap_or_default();
        meta.insert("file".into(), json!(o.file));
        let mut profil = Entree::new();
        if let Some(s) = &o.system {
            profil.insert("system".into(), json!(s));
        }
        if let Some(t) = &o.tools {
            profil.insert("tools".into(), json!(t));
        }
        if let Some(e) = &o.effort {
            profil.insert("effort".into(), json!(e));
        }
        meta.insert("profile".into(), Value::Object(profil));
        if harness_id != CLAUDE_HARNESS_ID {
            meta.insert("harness".into(), json!(harness_id));
        }
        // L'ORIGINE : d'où la conversation est née (le chat la montre en pièce jointe ; on remonte
        // ainsi jusqu'à la racine).
        let mut origine = json!({ "doc": o.file, "from": o.from, "to": o.to });
        if let (Some(q), Some(m)) = (&o.quote, origine.as_object_mut()) {
            m.insert("quote".into(), json!(q));
        }
        meta.insert("origin".into(), origine);
        if let Some(d) = non_vide(&o.display) {
            meta.insert("display".into(), json!(d));
        }
        if non_vide(&o.prompt).is_none()
            && let Some(p) = &preambule
        {
            meta.insert("preamble".into(), json!(p));
        }
        if let Some((de, message)) = branche {
            meta.insert("fork".into(), json!({ "from": de, "message": message }));
        }
        self.sessions.toucher(
            &session_id,
            ModifSession {
                title: Some(o.title.clone()),
                model: Some(self.reglages.modele()),
                meta: Some(meta),
            },
            Auteur::Utilisateur,
        );
        Some(session_id)
    }
}

/// Le `HarnessIO` d'un tour : la garde et les demandes passent par la politique, relue à chaque usage.
struct IoTour<'a> {
    svc: &'a ServiceAgent,
    sortie: &'a mut dyn SortieTour,
    session: String,
    mode: Option<PermissionMode>,
    sait_demander: bool,
}

impl HarnessIO for IoTour<'_> {
    fn event(&mut self, e: AgentEvent) {
        self.sortie.evenement(e);
    }

    fn set_state(&mut self, state: Value) {
        self.svc.poser_etat(&self.session, state);
    }

    /// ★ LA DEMANDE : la politique d'abord (« toujours », interdits) ; ce qui reste à demander va
    /// à la CARTE (la sortie du tour).
    fn request_permission(&mut self, r: PermissionRequest) -> PermissionDecision {
        let pol = self
            .svc
            .politique_pour(&self.session, self.mode, self.sait_demander);
        // Un plan à valider passe toujours par la carte : la politique ne le décide pas.
        if r.plan.is_none() {
            let usage = ToolUse {
                tool: r.tool.clone(),
                input: r.input.clone(),
            };
            match decide(&pol, &usage, &self.svc.contexte_de_decision(&self.session)) {
                Decision::Allow => return PermissionDecision::Allow { remember: None },
                Decision::Deny { reason } => {
                    return PermissionDecision::Deny {
                        message: Some(reason),
                    };
                }
                Decision::Ask => {}
            }
        }
        let plan = r.plan.is_some();
        let decision = self.sortie.permission(r);
        if let PermissionDecision::Allow { remember } = &decision {
            if let Some(rem) = remember {
                // Un trousseau de règles qui ne s'écrit pas ne doit pas faire échouer le tour : la
                // règle vaut pour ce tour-ci, elle sera redemandée au suivant.
                self.svc.retenir(&self.session, rem.scope, &rem.rule).ok();
            }
            if plan {
                self.svc.passer_en_modifs_auto(&self.session);
            }
        }
        decision
    }

    /// ★ LA GARDE : la politique décide, relue à chaque usage (un « toujours » donné pendant le
    /// tour compte tout de suite).
    fn guard(&self, usage: &ToolUse) -> Option<GuardDecision> {
        let pol = self
            .svc
            .politique_pour(&self.session, self.mode, self.sait_demander);
        Some(
            match decide(&pol, usage, &self.svc.contexte_de_decision(&self.session)) {
                Decision::Allow => GuardDecision::Allow,
                Decision::Ask => GuardDecision::Ask,
                Decision::Deny { reason } => GuardDecision::Deny { message: reason },
            },
        )
    }
}

impl AgentHote for ServiceAgent {
    fn nom_utilisateur(&self) -> String {
        self.nom()
    }

    fn maintenant(&self) -> i64 {
        (self.now)()
    }

    fn nouvel_id(&self) -> String {
        random_uuid()
    }

    fn session(&self, id: &str) -> Option<SessionAgent> {
        self.sessions.get(id)
    }

    fn sessions(&self) -> Vec<SessionAgent> {
        self.sessions.liste()
    }

    fn toucher(&self, id: &str, meta: Entree, auteur: Auteur) {
        // Le contrat de l'hôte : sans effet si la session n'existe pas (c'est `lancer` qui la crée).
        if self.sessions.get(id).is_some() {
            self.sessions.toucher(
                id,
                ModifSession {
                    meta: Some(meta),
                    ..Default::default()
                },
                auteur,
            );
        }
    }

    fn supprimer(&self, id: &str) {
        // Le moteur peut nettoyer ses fichiers : il ne garde que le transcript.
        if let Some(s) = self.sessions.get(id) {
            let hid = self.harness_id_de(id, None);
            if let Some(h) = self.harness(&hid) {
                h.forget(&HarnessConversation {
                    id: id.to_string(),
                    state: s.meta.get("harnessState").cloned(),
                });
            }
        }
        self.sessions.supprimer(id);
    }

    fn harness(&self, id: &str) -> Option<Rc<dyn AgentHarness>> {
        self.registre.borrow().harness_of(id)
    }

    fn harness_par_defaut(&self) -> Option<String> {
        self.reglages.harness()
    }

    /// Le RUN COMMUN (`AgentPlugin.run`).
    fn lancer(&self, d: DemandeTour, sortie: &mut dyn SortieTour) -> Result<(), ErreurHarness> {
        let Some(cwd) = self.config.racine_coffre.clone() else {
            return Err(ErreurHarness::Indisponible(
                "L’agent exige l’application de bureau.".into(),
            ));
        };
        let sid = d.session_id.clone();
        let voulu = Some(d.harness.as_str()).filter(|h| !h.is_empty());
        let hid = self.harness_id_de(&sid, voulu);
        let Some(harness) = self.harness(&hid) else {
            return Err(ErreurHarness::Indisponible(format!(
                "Le moteur « {hid} » n’est pas disponible : réactive son plugin pour poursuivre cette conversation."
            )));
        };
        // Relus à chaque tour : la fenêtre de réglages et l'agent lui-même ont pu les changer.
        self.reglages.recharger();
        let dossier = self.dossiers.lire();
        let session = self.sessions.get(&sid);
        let modele = d
            .model
            .clone()
            .or_else(|| self.modele_par_defaut(harness.as_ref()));
        // `started` : la conversation a eu un tour, son moteur est désormais FIGÉ.
        match &session {
            Some(s) => {
                let mut meta = s.meta.clone();
                meta.insert("started".into(), json!(true));
                self.sessions.toucher(
                    &sid,
                    ModifSession {
                        model: modele.clone(),
                        meta: Some(meta),
                        ..Default::default()
                    },
                    Auteur::Plugin,
                );
            }
            None => {
                let mut meta = d.meta.clone().unwrap_or_default();
                if hid != CLAUDE_HARNESS_ID {
                    meta.insert("harness".into(), json!(hid));
                }
                meta.insert("started".into(), json!(true));
                self.sessions.toucher(
                    &sid,
                    ModifSession {
                        title: Some(d.title.clone().unwrap_or_else(|| titre_depuis(&d.prompt))),
                        model: modele.clone().or_else(|| Some(self.reglages.modele())),
                        meta: Some(meta),
                    },
                    Auteur::Plugin,
                );
            }
        }
        let etat_de = |id: &str| {
            self.sessions
                .get(id)
                .and_then(|s| s.meta.get("harnessState").cloned())
        };
        let sait_demander = harness.capabilities().permission_prompts;
        let tour = HarnessTurn {
            conversation: HarnessConversation {
                id: sid.clone(),
                state: etat_de(&sid),
            },
            prompt: d.prompt.clone(),
            content: d.content.clone(),
            model: modele,
            effort: d.effort.clone(),
            instructions: self.consignes(&dossier),
            profile: d.system.clone(),
            cwd,
            extra_dirs: [
                self.dossiers.ecrans(),
                self.dossiers.pieces(),
                self.dossiers.actif(),
            ]
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
            policy: self.politique_pour(&sid, d.mode, sait_demander),
            fork: d.fork.as_ref().map(|(from, at)| ForkSource {
                from: HarnessConversation {
                    id: from.clone(),
                    state: etat_de(from),
                },
                at: at.clone(),
            }),
        };
        let mut io = IoTour {
            svc: self,
            sortie,
            session: sid.clone(),
            mode: d.mode,
            sait_demander,
        };
        harness.run(tour, &mut io)?;
        // La fin du tour date la session.
        if self.sessions.get(&sid).is_some() {
            self.sessions
                .toucher(&sid, ModifSession::default(), Auteur::Plugin);
        }
        Ok(())
    }

    fn interrompre(&self, conversation: &str) {
        let hid = self.harness_id_de(conversation, None);
        let crochets = self.interruptions.borrow();
        match crochets.get(&hid) {
            Some(f) => f(conversation),
            None => crochets.values().for_each(|f| f(conversation)),
        }
    }

    fn pomper(&self) -> bool {
        self.pompe.borrow_mut().as_mut().is_some_and(|f| f())
    }

    fn dossier_pieces(&self) -> PathBuf {
        self.dossiers.pieces()
    }

    fn codec_image(&self) -> Option<Rc<dyn CodecImage>> {
        self.codec.borrow().clone()
    }

    fn regle_toujours(&self, demande: &PermissionRequest) -> Option<PermanentRule> {
        rule_for(
            &ToolUse {
                tool: demande.tool.clone(),
                input: demande.input.clone(),
            },
            self.config.racine_coffre.as_deref().unwrap_or(""),
        )
    }
}

/// Un tour sans le chat : il est fini quand `run` rend la main (appels synchrones).
struct TourTermine {
    session: String,
    service: Rc<ServiceAgent>,
}

impl AgentQueryRun for TourTermine {
    fn session_id(&self) -> &str {
        &self.session
    }

    fn interrupt(&self) {
        self.service.interrompre(&self.session);
    }

    fn est_termine(&self) -> bool {
        true
    }
}

/// La sortie d'un tour sans le chat : les événements vont au rappel ; personne ne répond aux
/// cartes, elles sont refusées.
struct SortieRappel {
    on_event: Box<dyn FnMut(AgentEvent)>,
}

impl SortieTour for SortieRappel {
    fn evenement(&mut self, e: AgentEvent) {
        (self.on_event)(e);
    }

    fn permission(&mut self, _r: PermissionRequest) -> PermissionDecision {
        PermissionDecision::Deny {
            message: Some("Personne pour répondre à la demande d’autorisation.".into()),
        }
    }
}

/// `agent:service@1` pour les plugins tiers : le service est tenu par un `Rc`.
impl AgentService for Rc<ServiceAgent> {
    fn open_chat(&self) {
        self.ouvrir_chat(None, &AgentChatOptions::default());
    }

    fn ask(&self, prompt: &str, opts: AgentChatOptions) {
        self.ouvrir_chat(Some(prompt), &opts);
    }

    /// Un tour SANS le chat. La session entre dans l'historique (rouvrable dans le chat), avec
    /// `meta` pour dire d'où elle vient.
    fn run(&self, q: AgentQuery) -> Result<Box<dyn AgentQueryRun>, ErreurHarness> {
        let session = q.session_id.clone().unwrap_or_else(|| self.nouvel_id());
        let demande = DemandeTour {
            prompt: q.prompt,
            session_id: session.clone(),
            harness: q.harness.unwrap_or_default(),
            system: q.system,
            tools: q.tools,
            mode: q.mode,
            effort: q.effort,
            model: q.model,
            title: q.title,
            content: q.content,
            meta: q.meta,
            fork: q.fork,
        };
        let mut sortie = SortieRappel {
            on_event: q.on_event,
        };
        self.lancer(demande, &mut sortie)?;
        Ok(Box::new(TourTermine {
            session,
            service: Rc::clone(self),
        }))
    }

    fn get_session(&self, id: &str) -> Option<AgentSessionInfo> {
        self.sessions.info(id)
    }
}
