// Port de app/e2e/readerHarness.ts (`installerFauxAgent`) et des doubles propres aux specs
// agent-chat, agent-harness, agent-trace (`__trace`) et agent-permissions (`__perm`)
//
// Le FAUX HARNESS : il rejoue des tours SCRIPTÉS, sans aucun appel à `claude`. Il sert aux tests de
// tout T7 (fournisseur, trace, permissions, pièces jointes, barre, scènes `pixels:agent-script`).
//
// - un SCÉNARIO par tour : une suite de [`Pas`] choisie d'après le [`HarnessTurn`] (le prompt…) ;
// - des PAS libérés par le test : [`Pas::Pause`] appelle le crochet `au_pas`, qui rend la main au
//   test entre deux événements (observer l'état image par image) et dit s'il faut continuer ;
// - la GARDE et la CARTE appelées, comme le fait le vrai SDK (`req.permissions.guard` / `.ask`) :
//   [`Pas::Garde`] et [`Pas::Carte`], dont les réponses sont gardées pour les assertions ;
// - des TRANSCRIPTS en mémoire, branches comprises : un tour y écrit sa question puis sa réponse
//   (pensée, outils avec leur résultat, texte), un fork repart de celui du parent coupé au repère.
//
// ★ Écart assumé : tour synchrone (voir `contrat.rs`). Une interruption ne s'observe que de
//   l'intérieur du tour (un crochet de pas, un rappel d'événement qui appelle `stop`) : le
//   scénario s'arrête alors au pas suivant, comme le `stop = true` du double TS.
//
// `FauxHote` est l'HÔTE minimal du fournisseur (sessions en mémoire, registre des moteurs, run
// commun réduit à l'essentiel) ; le vrai est celui de `fr-t7-service`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use serde_json::{Value, json};

use crate::attachment_blocks::attachment_from_block;
use crate::contrat::{
    AgentEvent, AgentHarness, AgentPolicy, AttachmentKind, BlocOutil, ChatBlock, ChatEntry, Entree,
    ErreurHarness, ForkSource, GuardDecision, HarnessCapabilities, HarnessConversation, HarnessIO,
    HarnessModel, HarnessRun, HarnessTurn, PermanentRule, PermissionDecision, PermissionRequest,
    PolicySupport, Role, ShellSupport, ToolUse, TurnUsage,
};
use crate::fournisseur::hote::{AgentHote, Auteur, DemandeTour, SessionAgent, SortieTour};
use crate::fournisseur::pieces::{CodecImage, FormatImage};
use crate::harness_registry::{HarnessRegistry, Jeton};
use crate::policy_model::{open_vault_policy, rule_for};

/// Un pas du scénario d'un tour.
pub enum Pas {
    /// Un événement émis tel quel (`text-delta`, `usage`, `done`…).
    Evenement(AgentEvent),
    /// Un pas à libérer : le crochet `au_pas` rend la main au test ; `false` arrête le tour.
    Pause,
    /// La GARDE de la politique, appelée comme le ferait le moteur avant un outil.
    Garde(ToolUse),
    /// Une DEMANDE d'autorisation (la carte) : le tour attend la réponse.
    Carte(PermissionRequest),
    /// Un texte qui dit ce que les cartes ont obtenu (`Bash: oui`, `Write: non (pourquoi)`) ou
    /// « Rien à demander. » — la réponse du double `__perm`.
    Bilan,
    /// L'état opaque du moteur pour cette conversation (`HarnessIO::set_state`).
    Etat(Value),
}

impl Pas {
    pub fn texte(t: &str) -> Pas {
        Pas::Evenement(AgentEvent::TextDelta { text: t.into() })
    }

    /// Un nouveau bloc de pensée.
    pub fn pensee_debut() -> Pas {
        Pas::Evenement(AgentEvent::ThinkingDelta {
            text: String::new(),
            start: Some(true),
        })
    }

    pub fn pensee(t: &str) -> Pas {
        Pas::Evenement(AgentEvent::ThinkingDelta {
            text: t.into(),
            start: None,
        })
    }

    /// Un appel d'outil (`input` : un objet JSON).
    pub fn outil(id: &str, name: &str, summary: &str, input: Value) -> Pas {
        Pas::Evenement(AgentEvent::Assistant {
            blocks: vec![ChatBlock::Tool(BlocOutil {
                id: id.into(),
                name: name.into(),
                summary: summary.into(),
                input: input.as_object().cloned(),
                result: None,
                at: None,
                result_at: None,
            })],
        })
    }

    pub fn resultat(id: &str, texte: &str) -> Pas {
        Pas::Evenement(AgentEvent::ToolResult {
            tool_use_id: id.into(),
            is_error: false,
            text: texte.into(),
        })
    }

    pub fn usage(u: TurnUsage) -> Pas {
        Pas::Evenement(AgentEvent::Usage { usage: u })
    }

    /// Un tour qui échoue.
    pub fn erreur(m: &str) -> Pas {
        Pas::Evenement(AgentEvent::Error { message: m.into() })
    }
}

type Scenario = Box<dyn Fn(&HarnessTurn) -> Vec<Pas>>;
type CrochetPas = Box<dyn FnMut(usize) -> bool>;

/// Le moteur de test.
pub struct FauxHarness {
    id: String,
    label: String,
    models: RefCell<Vec<HarnessModel>>,
    capacites: RefCell<HarnessCapabilities>,
    scenario: RefCell<Scenario>,
    au_pas: RefCell<Option<CrochetPas>>,
    transcripts: RefCell<HashMap<String, Vec<ChatEntry>>>,
    appels: RefCell<Vec<HarnessTurn>>,
    gardes: RefCell<Vec<(ToolUse, Option<GuardDecision>)>>,
    cartes: RefCell<Vec<(PermissionRequest, PermissionDecision)>>,
    interrompus: RefCell<HashSet<String>>,
    tours: RefCell<HashMap<String, u64>>,
    horloge: Cell<i64>,
    ordre: Cell<u64>,
}

impl FauxHarness {
    pub fn new(id: &str) -> Rc<Self> {
        Rc::new(Self {
            id: id.into(),
            label: format!("Faux {id}"),
            models: RefCell::new(vec![HarnessModel {
                id: "faux-1".into(),
                label: "Faux 1".into(),
                efforts: Some(vec!["low".into(), "medium".into(), "high".into()]),
                default: Some(true),
                ..HarnessModel::default()
            }]),
            capacites: RefCell::new(HarnessCapabilities {
                resume: true,
                fork: true,
                thinking: true,
                text_deltas: true,
                attachments: vec![
                    AttachmentKind::Image,
                    AttachmentKind::Pdf,
                    AttachmentKind::Text,
                    AttachmentKind::File,
                ],
                structured_output: false,
                permission_prompts: true,
                policy: PolicySupport {
                    tools: true,
                    folders: true,
                    network: true,
                    shell: ShellSupport::Rules,
                },
            }),
            scenario: RefCell::new(Box::new(|t| {
                vec![Pas::texte(&format!(
                    "Écho ({}, {}) : {}",
                    t.model.as_deref().unwrap_or("faux-1"),
                    t.effort.as_deref().unwrap_or("sans effort"),
                    t.prompt
                ))]
            })),
            au_pas: RefCell::new(None),
            transcripts: RefCell::new(HashMap::new()),
            appels: RefCell::new(Vec::new()),
            gardes: RefCell::new(Vec::new()),
            cartes: RefCell::new(Vec::new()),
            interrompus: RefCell::new(HashSet::new()),
            tours: RefCell::new(HashMap::new()),
            horloge: Cell::new(1_792_497_600_000),
            ordre: Cell::new(0),
        })
    }

    /// Le scénario de chaque tour, choisi d'après le tour (son prompt…). Le `done` final est ajouté
    /// si le scénario n'en émet pas.
    pub fn scenario(&self, f: impl Fn(&HarnessTurn) -> Vec<Pas> + 'static) {
        *self.scenario.borrow_mut() = Box::new(f);
    }

    /// Le crochet des pas : appelé à chaque [`Pas::Pause`] avec le rang du pas libéré (1, 2…) ;
    /// rend `false` pour arrêter le tour.
    pub fn au_pas(&self, f: impl FnMut(usize) -> bool + 'static) {
        *self.au_pas.borrow_mut() = Some(Box::new(f));
    }

    pub fn avec_modeles(&self, m: Vec<HarnessModel>) {
        *self.models.borrow_mut() = m;
    }

    pub fn avec_capacites(&self, c: HarnessCapabilities) {
        *self.capacites.borrow_mut() = c;
    }

    /// Les tours reçus, dans l'ordre.
    pub fn appels(&self) -> Vec<HarnessTurn> {
        self.appels.borrow().clone()
    }

    /// Ce que la garde a répondu à chaque [`Pas::Garde`] (`None` : l'hôte n'a pas de garde).
    pub fn gardes(&self) -> Vec<(ToolUse, Option<GuardDecision>)> {
        self.gardes.borrow().clone()
    }

    /// Ce que l'utilisateur a répondu à chaque [`Pas::Carte`].
    pub fn cartes(&self) -> Vec<(PermissionRequest, PermissionDecision)> {
        self.cartes.borrow().clone()
    }

    /// Un transcript posé à la main (une conversation qui existait avant).
    pub fn poser_transcript(&self, conversation: &str, entrees: Vec<ChatEntry>) {
        self.transcripts
            .borrow_mut()
            .insert(conversation.into(), entrees);
    }

    pub fn transcript(&self, conversation: &str) -> Vec<ChatEntry> {
        self.transcripts
            .borrow()
            .get(conversation)
            .cloned()
            .unwrap_or_default()
    }

    /// Interrompt le tour en cours de cette conversation (au prochain pas).
    pub fn interrompre(&self, conversation: &str) {
        self.interrompus.borrow_mut().insert(conversation.into());
    }

    fn maintenant(&self) -> i64 {
        let t = self.horloge.get();
        self.horloge.set(t + 1);
        t
    }

    fn uuid(&self, conversation: &str) -> String {
        let n = self.ordre.get();
        self.ordre.set(n + 1);
        format!("{conversation}#{n}")
    }

    /// Le transcript d'une conversation NEUVE : vide, ou celui du parent coupé au repère `at`
    /// (inclus) quand c'est une branche.
    fn amorcer(&self, conversation: &str, fork: Option<&ForkSource>) {
        if self.transcripts.borrow().contains_key(conversation) {
            return;
        }
        let debut = fork
            .map(|f| {
                let parent = self.transcript(&f.from.id);
                match parent.iter().position(|e| e.last.as_deref() == Some(&f.at)) {
                    Some(i) => parent[..=i].to_vec(),
                    None => Vec::new(),
                }
            })
            .unwrap_or_default();
        self.transcripts
            .borrow_mut()
            .insert(conversation.into(), debut);
    }

    fn ecrire_tour(&self, conversation: &str, question: ChatEntry, reponse: Option<ChatEntry>) {
        let mut t = self.transcripts.borrow_mut();
        let v = t.entry(conversation.into()).or_default();
        v.push(question);
        v.extend(reponse);
    }
}

/// Les blocs de la réponse, tels que le transcript du moteur les range.
fn noter_bloc(blocs: &mut Vec<ChatBlock>, ev: &AgentEvent, now: i64) {
    match ev {
        AgentEvent::ThinkingDelta { text, start } => {
            let suite =
                start != &Some(true) && matches!(blocs.last(), Some(ChatBlock::Thinking { .. }));
            if suite {
                if let Some(ChatBlock::Thinking { text: t, .. }) = blocs.last_mut() {
                    t.push_str(text);
                }
            } else {
                blocs.push(ChatBlock::Thinking {
                    text: text.clone(),
                    at: Some(now),
                });
            }
        }
        AgentEvent::TextDelta { text } => {
            if let Some(ChatBlock::Text { text: t, .. }) = blocs.last_mut() {
                t.push_str(text);
            } else {
                blocs.push(ChatBlock::Text {
                    text: text.clone(),
                    at: Some(now),
                });
            }
        }
        AgentEvent::Assistant { blocks } => {
            for b in blocks {
                if let ChatBlock::Tool(o) = b {
                    let mut o = o.clone();
                    o.at = Some(now);
                    blocs.push(ChatBlock::Tool(o));
                }
            }
        }
        AgentEvent::ToolResult {
            tool_use_id,
            is_error,
            text,
        } => {
            for b in blocs.iter_mut() {
                if let ChatBlock::Tool(o) = b
                    && &o.id == tool_use_id
                {
                    o.result = Some(crate::contrat::ResultatOutil {
                        is_error: *is_error,
                        text: text.clone(),
                    });
                    o.result_at = Some(now);
                }
            }
        }
        _ => {}
    }
}

struct RunFaux;

impl HarnessRun for RunFaux {
    fn interrupt(&self) {}
    fn est_termine(&self) -> bool {
        true
    }
}

impl AgentHarness for FauxHarness {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> HarnessCapabilities {
        self.capacites.borrow().clone()
    }

    fn models(&self) -> Vec<HarnessModel> {
        self.models.borrow().clone()
    }

    fn run(
        &self,
        turn: HarnessTurn,
        io: &mut dyn HarnessIO,
    ) -> Result<Box<dyn HarnessRun>, ErreurHarness> {
        let conv = turn.conversation.id.clone();
        self.appels.borrow_mut().push(turn.clone());
        self.interrompus.borrow_mut().remove(&conv);
        self.amorcer(&conv, turn.fork.as_ref());

        // La question : son texte, puis ses pièces (les blocs de contenu).
        let t0 = self.maintenant();
        let mut blocs_question: Vec<ChatBlock> = Vec::new();
        if !turn.prompt.is_empty() {
            blocs_question.push(ChatBlock::Text {
                text: turn.prompt.clone(),
                at: None,
            });
        }
        blocs_question.extend(turn.content.iter().filter_map(attachment_from_block));
        let question = ChatEntry {
            role: Role::User,
            blocks: blocs_question,
            last: Some(self.uuid(&conv)),
            at: Some(t0),
        };

        let pas = (self.scenario.borrow())(&turn);
        let mut blocs: Vec<ChatBlock> = Vec::new();
        let mut lignes: Vec<String> = Vec::new();
        let mut libres = 0usize;
        let mut done = false;
        let finaliser = |blocs: &mut Vec<ChatBlock>| {
            let reponse = (!blocs.is_empty()).then(|| ChatEntry {
                role: Role::Assistant,
                blocks: std::mem::take(blocs),
                last: Some(self.uuid(&conv)),
                at: Some(self.maintenant()),
            });
            self.ecrire_tour(&conv, question.clone(), reponse);
        };
        for p in pas {
            if self.interrompus.borrow().contains(&conv) {
                break;
            }
            match p {
                Pas::Evenement(ev) => {
                    noter_bloc(&mut blocs, &ev, self.maintenant());
                    if matches!(ev, AgentEvent::Done { .. } | AgentEvent::Error { .. }) {
                        if !done {
                            finaliser(&mut blocs);
                        }
                        done = true;
                    }
                    io.event(ev);
                }
                Pas::Pause => {
                    libres += 1;
                    let suite = self.au_pas.borrow_mut().as_mut().is_none_or(|f| f(libres));
                    if !suite {
                        break;
                    }
                }
                Pas::Garde(u) => {
                    let d = io.guard(&u);
                    self.gardes.borrow_mut().push((u, d));
                }
                Pas::Carte(r) => {
                    let d = io.request_permission(r.clone());
                    lignes.push(match &d {
                        PermissionDecision::Allow { .. } => format!("{}: oui", r.tool),
                        PermissionDecision::Deny { message } => {
                            format!("{}: non ({})", r.tool, message.as_deref().unwrap_or(""))
                        }
                    });
                    self.cartes.borrow_mut().push((r, d));
                }
                Pas::Bilan => {
                    let t = if lignes.is_empty() {
                        "Rien à demander.".to_string()
                    } else {
                        lignes.join("\n")
                    };
                    noter_bloc(
                        &mut blocs,
                        &AgentEvent::TextDelta { text: t.clone() },
                        self.maintenant(),
                    );
                    io.event(AgentEvent::TextDelta { text: t });
                }
                Pas::Etat(v) => io.set_state(v),
            }
        }
        if !done {
            finaliser(&mut blocs);
        }
        let n = {
            let mut t = self.tours.borrow_mut();
            let n = t.entry(conv.clone()).or_insert(0);
            *n += 1;
            *n
        };
        io.set_state(json!({ "thread": format!("T-{conv}"), "tours": n }));
        if !done {
            io.event(AgentEvent::Done {
                session_id: conv,
                is_error: false,
                errors: Vec::new(),
                usage: None,
            });
        }
        Ok(Box::new(RunFaux))
    }

    fn history(&self, conv: &HarnessConversation) -> Vec<ChatEntry> {
        self.transcript(&conv.id)
    }

    fn forget(&self, conv: &HarnessConversation) {
        self.transcripts.borrow_mut().remove(&conv.id);
    }
}

// ── L'hôte de test ───────────────────────────────────────────────────────────

type Garde = Rc<dyn Fn(&ToolUse) -> GuardDecision>;
type Pompe = Box<dyn FnMut() -> bool>;

/// L'hôte minimal du fournisseur : sessions en mémoire, registre des moteurs, run commun réduit
/// à l'essentiel (création de la session, tour confié au harness, `started`, `harnessState`).
pub struct FauxHote {
    nom: RefCell<String>,
    sessions: RefCell<Vec<SessionAgent>>,
    registre: RefCell<HarnessRegistry>,
    jetons: RefCell<HashMap<String, Jeton>>,
    faux: RefCell<Vec<Rc<FauxHarness>>>,
    defaut: RefCell<Option<String>>,
    garde: RefCell<Option<Garde>>,
    pompe: RefCell<Option<Pompe>>,
    codec: RefCell<Option<Rc<dyn CodecImage>>>,
    dossier: PathBuf,
    horloge: Cell<i64>,
    compteur: Cell<u64>,
}

impl FauxHote {
    /// `dossier_pieces` : où l'on range les fichiers joints « autres » (un dossier temporaire).
    pub fn new(dossier_pieces: PathBuf) -> Rc<Self> {
        Rc::new(Self {
            nom: RefCell::new("Alice".into()),
            sessions: RefCell::new(Vec::new()),
            registre: RefCell::new(HarnessRegistry::new()),
            jetons: RefCell::new(HashMap::new()),
            faux: RefCell::new(Vec::new()),
            defaut: RefCell::new(None),
            garde: RefCell::new(None),
            pompe: RefCell::new(None),
            codec: RefCell::new(None),
            dossier: dossier_pieces,
            horloge: Cell::new(1_792_497_600_000),
            compteur: Cell::new(0),
        })
    }

    /// Inscrit un faux moteur (priorité 0).
    pub fn enregistrer(&self, h: &Rc<FauxHarness>) {
        let jeton =
            self.registre
                .borrow_mut()
                .register(&h.id, Rc::clone(h) as Rc<dyn AgentHarness>, 0);
        self.jetons.borrow_mut().insert(h.id.clone(), jeton);
        self.faux.borrow_mut().push(Rc::clone(h));
    }

    /// Le plugin du moteur est désactivé : il quitte le registre.
    pub fn desactiver(&self, id: &str) {
        if let Some(j) = self.jetons.borrow_mut().remove(id) {
            self.registre.borrow_mut().unregister(id, j);
        }
    }

    /// Les ids des moteurs inscrits (triés).
    pub fn harness_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .registre
            .borrow()
            .harness_list()
            .iter()
            .map(|h| h.id().to_string())
            .collect();
        ids.sort();
        ids
    }

    pub fn set_nom(&self, nom: &str) {
        *self.nom.borrow_mut() = nom.into();
    }

    pub fn set_harness_par_defaut(&self, id: Option<&str>) {
        *self.defaut.borrow_mut() = id.map(str::to_string);
    }

    /// La garde de la politique, appelée avant tout outil ([`Pas::Garde`]).
    pub fn set_garde(&self, f: impl Fn(&ToolUse) -> GuardDecision + 'static) {
        *self.garde.borrow_mut() = Some(Rc::new(f));
    }

    /// Ce qui avance pendant qu'une carte attend (un « clic » du test) ; rend `false` quand plus
    /// rien ne peut répondre.
    pub fn set_pompe(&self, f: impl FnMut() -> bool + 'static) {
        *self.pompe.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_codec(&self, c: Rc<dyn CodecImage>) {
        *self.codec.borrow_mut() = Some(c);
    }

    /// Une session posée à la main (une conversation ancrée ouverte avec son profil, sa branche…).
    pub fn poser_session(&self, id: &str, titre: &str, meta: Value) {
        let t = self.avancer();
        self.sessions.borrow_mut().push(SessionAgent {
            id: id.into(),
            title: titre.into(),
            created_at: t,
            updated_at: t,
            meta: meta.as_object().cloned().unwrap_or_default(),
        });
    }

    fn avancer(&self) -> i64 {
        let t = self.horloge.get();
        self.horloge.set(t + 1);
        t
    }
}

/// Le `HarnessIO` d'un tour de l'hôte de test.
struct IoFaux<'a> {
    sortie: &'a mut dyn SortieTour,
    garde: Option<Garde>,
    etat: Option<Value>,
}

impl HarnessIO for IoFaux<'_> {
    fn event(&mut self, e: AgentEvent) {
        self.sortie.evenement(e);
    }

    fn set_state(&mut self, state: Value) {
        self.etat = Some(state);
    }

    fn request_permission(&mut self, r: PermissionRequest) -> PermissionDecision {
        self.sortie.permission(r)
    }

    fn guard(&self, usage: &ToolUse) -> Option<GuardDecision> {
        self.garde.as_ref().map(|g| g(usage))
    }
}

impl AgentHote for FauxHote {
    fn nom_utilisateur(&self) -> String {
        self.nom.borrow().clone()
    }

    fn maintenant(&self) -> i64 {
        self.avancer()
    }

    fn nouvel_id(&self) -> String {
        let n = self.compteur.get() + 1;
        self.compteur.set(n);
        format!("id-{n}")
    }

    fn session(&self, id: &str) -> Option<SessionAgent> {
        self.sessions.borrow().iter().find(|s| s.id == id).cloned()
    }

    fn sessions(&self) -> Vec<SessionAgent> {
        let mut v = self.sessions.borrow().clone();
        v.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
        v
    }

    fn toucher(&self, id: &str, meta: Entree, _auteur: Auteur) {
        let t = self.avancer();
        if let Some(s) = self.sessions.borrow_mut().iter_mut().find(|s| s.id == id) {
            s.meta = meta;
            s.updated_at = t;
        }
    }

    fn supprimer(&self, id: &str) {
        self.sessions.borrow_mut().retain(|s| s.id != id);
    }

    fn harness(&self, id: &str) -> Option<Rc<dyn AgentHarness>> {
        self.registre.borrow().harness_of(id)
    }

    fn harness_par_defaut(&self) -> Option<String> {
        self.defaut.borrow().clone()
    }

    fn lancer(&self, d: DemandeTour, sortie: &mut dyn SortieTour) -> Result<(), ErreurHarness> {
        let Some(harness) = self.harness(&d.harness) else {
            return Err(ErreurHarness::Indisponible(format!(
                "Le moteur « {} » n’est pas disponible : réactive son plugin pour poursuivre cette conversation.",
                d.harness
            )));
        };
        // Une session NEUVE : son titre, sa meta (+ le moteur).
        if self.session(&d.session_id).is_none() {
            let mut meta = d.meta.clone().unwrap_or_default();
            meta.insert("harness".into(), json!(d.harness));
            let t = self.avancer();
            self.sessions.borrow_mut().push(SessionAgent {
                id: d.session_id.clone(),
                title: d.title.clone().unwrap_or_default(),
                created_at: t,
                updated_at: t,
                meta,
            });
        }
        let etat_de = |id: &str| {
            self.session(id)
                .and_then(|s| s.meta.get("harnessState").cloned())
        };
        let mut politique: AgentPolicy = open_vault_policy();
        if let Some(m) = d.mode {
            politique.mode = m;
        }
        let tour = HarnessTurn {
            conversation: HarnessConversation {
                id: d.session_id.clone(),
                state: etat_de(&d.session_id),
            },
            prompt: d.prompt.clone(),
            content: d.content.clone(),
            model: d.model.clone(),
            effort: d.effort.clone(),
            // ★ Un prompt FACTICE, écrit pour les tests : jamais celui de Fragment (décision de l'orchestrateur).
            instructions: "Prompt factice des tests : tu es un agent de démonstration.".into(),
            profile: d.system.clone(),
            cwd: "C:/coffre".into(),
            extra_dirs: vec![self.dossier.to_string_lossy().into_owned()],
            policy: politique,
            fork: d.fork.as_ref().map(|(from, at)| ForkSource {
                from: HarnessConversation {
                    id: from.clone(),
                    state: etat_de(from),
                },
                at: at.clone(),
            }),
        };
        let mut io = IoFaux {
            sortie,
            garde: self.garde.borrow().clone(),
            etat: None,
        };
        harness.run(tour, &mut io)?;
        // Le moteur est figé dès le premier tour ; son état est gardé pour la suite.
        let etat = io.etat;
        if let Some(s) = self.session(&d.session_id) {
            let mut meta = s.meta;
            meta.insert("started".into(), json!(true));
            if let Some(e) = etat {
                meta.insert("harnessState".into(), e);
            }
            self.toucher(&d.session_id, meta, Auteur::Plugin);
        }
        Ok(())
    }

    fn interrompre(&self, conversation: &str) {
        for h in self.faux.borrow().iter() {
            h.interrompre(conversation);
        }
    }

    fn pomper(&self) -> bool {
        self.pompe.borrow_mut().as_mut().is_some_and(|f| f())
    }

    fn dossier_pieces(&self) -> PathBuf {
        self.dossier.clone()
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
            "C:/coffre",
        )
    }
}

// ── Le codec d'image de test ─────────────────────────────────────────────────

/// Un codec scripté : toutes les images font `dim`, le n-ième réencodage rend `octets[n]` octets.
pub struct CodecImageFaux {
    dim: Option<(u32, u32)>,
    octets: Vec<usize>,
    appels: RefCell<Vec<(u32, u32, FormatImage)>>,
}

impl CodecImageFaux {
    pub fn new(dim: Option<(u32, u32)>, octets: &[usize]) -> Rc<Self> {
        Rc::new(Self {
            dim,
            octets: octets.to_vec(),
            appels: RefCell::new(Vec::new()),
        })
    }

    /// Les réencodages demandés : (largeur, hauteur, format).
    pub fn appels(&self) -> Vec<(u32, u32, FormatImage)> {
        self.appels.borrow().clone()
    }
}

impl CodecImage for CodecImageFaux {
    fn dimensions(&self, _data: &[u8], _mime: &str) -> Option<(u32, u32)> {
        self.dim
    }

    fn reencoder(
        &self,
        _data: &[u8],
        _mime: &str,
        largeur: u32,
        hauteur: u32,
        format: FormatImage,
        _qualite: f32,
    ) -> Option<Vec<u8>> {
        let n = self.appels.borrow().len();
        self.appels.borrow_mut().push((largeur, hauteur, format));
        self.octets.get(n).map(|o| vec![7; *o])
    }
}
