// Port de app/src/ui/views/agent/harness/claudeHarness.ts (le harness natif) et de AgentHost.ts /
// sdkProcess.mjs (son hôte), réécrits sans Node ni SDK (ADR-016)
//
//! Harness Claude : le binaire `claude` piloté en stream-json — tours, sessions, reprise, fork,
//! interruption, garde et carte (tâche `fr-t7-harness-claude`).
//!
//! - [`commande`] : le choix de la session, la ligne de commande, l'environnement (PUR) ;
//! - [`protocole`] : les messages écrits et les réponses à la garde et à la carte (PUR) ;
//! - [`tour`] : le contrôleur d'un tour (lecture du flux, demandes, interruption, fin) ;
//! - [`processus`] : le transport (le vrai processus, le trait que les tests doublent) ;
//! - [`transcript`] : le transcript du binaire sur le disque (existence, `history`) ;
//! - [`dossier_agent`] : le dossier de l'agent de l'utilisateur (prompt, préprompt, skills) ;
//! - [`faux`] : le faux processus qui rejoue une fixture (tests, sans `claude`).
//!
//! ★ Un processus `claude` PAR TOUR (ADR-016 §1) : prompt et options sont relus à chaque tour.
//! ★ Inscrit à la priorité [`PRIORITE`] (−100), comme un tiers : il n'a aucun privilège.
//! ★ `one_shot` et la sortie structurée viennent avec T8.

pub mod commande;
pub mod dossier_agent;
pub mod faux;
pub mod processus;
pub mod protocole;
pub mod tour;
pub mod transcript;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::contrat::{
    AgentHarness, AttachmentKind, ChatEntry, ErreurHarness, HarnessCapabilities,
    HarnessConversation, HarnessIO, HarnessModel, HarnessRun, HarnessStatus, HarnessTurn,
    PolicySupport, ShellSupport,
};
use crate::harness_registry::CLAUDE_HARNESS_ID;
use crate::policy_model::{ClaudePermissionMode, compile_claude};

use commande::{OptionsTour, arguments, choisir_session, environnement};
use dossier_agent::{lire_dossier, prompt_systeme};
use processus::{Lancement, Lanceur, LanceurSysteme};
use protocole::masquer_courriels;
use tour::{Delais, ParamsTour, executer_tour};
use transcript::{chemin_transcript, lire_historique};

pub use tour::{Interrupteur, VERSION_MINIMALE};

/// La priorité d'inscription dans `agent:harness@1` : celle d'un tiers.
pub const PRIORITE: i32 = -100;

/// Le modèle par défaut quand ni le tour ni les réglages n'en choisissent.
pub const MODELE_PAR_DEFAUT: &str = "claude-opus-5-5";

/// Les modèles proposés (la famille Claude 5), comme le TS.
fn modeles_connus() -> Vec<HarnessModel> {
    let efforts = Some(commande::EFFORTS.iter().map(|e| e.to_string()).collect());
    let m = |id: &str, label: &str, description: Option<&str>, small: bool| HarnessModel {
        id: id.into(),
        label: label.into(),
        description: description.map(str::to_string),
        efforts: efforts.clone(),
        small: small.then_some(true),
        default: None,
    };
    vec![
        m(
            "claude-opus-5-5",
            "Opus 5.5",
            Some("Le plus capable"),
            false,
        ),
        m(
            "claude-sonnet-5-5",
            "Sonnet 5.5",
            Some("Rapide et capable"),
            false,
        ),
        m(
            "claude-haiku-5-5",
            "Haiku 5.5",
            Some("Le plus rapide"),
            true,
        ),
        m("claude-fable-5-1", "Fable 5.1", None, false),
    ]
}

/// Ce que le harness tient de l'app (réglages, dossiers). Lu à chaque tour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigClaude {
    /// Le binaire de Claude Code (résolu par le moteur, `fr-t7-moteur`) ; `claude` : celui du PATH.
    pub binaire: PathBuf,
    /// Le dossier de config du binaire (`~/.claude`) : ses transcripts sont dans `projects/`.
    pub dossier_config: PathBuf,
    /// La racine du coffre (pour relire un transcript sans tour : `history`).
    pub racine_coffre: Option<String>,
    /// Le nom du coffre (la variable `{{vault}}` du prompt).
    pub nom_coffre: String,
    /// Le dossier de l'agent de l'utilisateur (prompt, préprompt, skills), s'il existe.
    pub dossier_agent: Option<PathBuf>,
    /// Le dossier du shim `fragment`, mis en tête du `PATH`.
    pub shim: Option<String>,
    /// Des dossiers que Read doit pouvoir lire (captures, pièces jointes).
    pub dossiers_lisibles: Vec<String>,
    /// Le modèle des réglages (texte libre) ; absent : [`MODELE_PAR_DEFAUT`].
    pub modele: Option<String>,
    /// L'environnement de base ; `None` : celui de l'app (`std::env::vars`).
    pub env_base: Option<Vec<(String, String)>>,
    pub delais: Delais,
}

impl ConfigClaude {
    /// La configuration par défaut : `claude` du PATH, `~/.claude`.
    pub fn new(nom_coffre: &str) -> Self {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        Self {
            binaire: PathBuf::from("claude"),
            dossier_config: Path::new(&home).join(".claude"),
            racine_coffre: None,
            nom_coffre: nom_coffre.to_string(),
            dossier_agent: None,
            shim: None,
            dossiers_lisibles: Vec::new(),
            modele: None,
            env_base: None,
            delais: Delais::default(),
        }
    }
}

type Puits = Box<dyn Fn(&str)>;

/// Le harness Claude.
pub struct HarnessClaude {
    config: RefCell<ConfigClaude>,
    lanceur: Box<dyn Lanceur>,
    journal: Puits,
    /// Le drapeau d'arrêt de chaque conversation dont un tour est en cours (ou va partir).
    arrets: RefCell<HashMap<String, Interrupteur>>,
}

/// Le tour rendu par `run` : il est déjà fini (tour synchrone, voir `contrat.rs`).
struct TourClaude {
    arret: Interrupteur,
}

impl HarnessRun for TourClaude {
    fn interrupt(&self) {
        self.arret.interrompre();
    }

    fn est_termine(&self) -> bool {
        true
    }
}

impl HarnessClaude {
    /// Le harness sur le vrai binaire.
    pub fn new(config: ConfigClaude) -> Self {
        Self::avec_lanceur(config, Box::new(LanceurSysteme))
    }

    /// Le harness sur un autre transport (le faux des tests).
    pub fn avec_lanceur(config: ConfigClaude, lanceur: Box<dyn Lanceur>) -> Self {
        Self {
            config: RefCell::new(config),
            lanceur,
            journal: Box::new(|_| {}),
            arrets: RefCell::new(HashMap::new()),
        }
    }

    /// Où vont les lignes du journal (version du binaire, lignes ignorées…). ★ Jamais la réponse
    /// à `initialize` ; toute adresse de courriel est masquée.
    pub fn avec_journal(mut self, journal: impl Fn(&str) + 'static) -> Self {
        self.journal = Box::new(journal);
        self
    }

    /// Les réglages changent (modèle, dossiers…) : relus au prochain tour.
    pub fn configurer(&self, f: impl FnOnce(&mut ConfigClaude)) {
        f(&mut self.config.borrow_mut());
    }

    /// Le drapeau d'arrêt du prochain tour (ou du tour en cours) de cette conversation : un autre
    /// fil peut le lever pendant que `run` tourne.
    pub fn interrupteur(&self, conversation: &str) -> Interrupteur {
        self.arrets
            .borrow_mut()
            .entry(conversation.to_string())
            .or_default()
            .clone()
    }

    /// Arrête le tour en cours de cette conversation (sans effet s'il n'y en a pas).
    pub fn interrompre(&self, conversation: &str) {
        if let Some(a) = self.arrets.borrow().get(conversation) {
            a.interrompre();
        }
    }

    fn noter(&self, m: &str) {
        (self.journal)(&masquer_courriels(m));
    }

    /// Le lancement d'un tour : session, ligne de commande, environnement, prompt (PUR, hors
    /// lecture du dossier de l'agent et du transcript).
    fn preparer(&self, turn: &HarnessTurn) -> (Lancement, String, bool) {
        let c = self.config.borrow();
        let dossier = c
            .dossier_agent
            .as_deref()
            .filter(|d| d.is_dir())
            .map(lire_dossier);
        let systeme = prompt_systeme(
            dossier.as_ref(),
            &turn.instructions,
            &c.nom_coffre,
            turn.profile.as_deref(),
        );
        let dir_agent = dossier.as_ref().map(|d| d.dir.replace('\\', "/"));
        let politique = compile_claude(&turn.policy, dir_agent.as_deref());
        let fork = turn
            .fork
            .as_ref()
            .map(|f| (f.from.id.as_str(), f.at.as_str()));
        let session = choisir_session(&turn.conversation.id, fork, |id| {
            chemin_transcript(&c.dossier_config, &turn.cwd, id).is_some()
        });
        let mut dossiers = c.dossiers_lisibles.clone();
        dossiers.extend(dir_agent.clone());
        dossiers.extend(turn.extra_dirs.iter().cloned());
        let options = OptionsTour {
            modele: turn
                .model
                .clone()
                .or_else(|| c.modele.clone())
                .or_else(|| Some(MODELE_PAR_DEFAUT.to_string())),
            effort: turn.effort.clone(),
            dossier_agent: dir_agent,
            skills: dossier.map(|d| d.skills).unwrap_or_default(),
            dossiers,
        };
        let base = c
            .env_base
            .clone()
            .unwrap_or_else(|| std::env::vars().collect());
        let lancement = Lancement {
            programme: c.binaire.clone(),
            arguments: arguments(&session, &politique, &options),
            cwd: PathBuf::from(&turn.cwd),
            env: environnement(base, c.shim.as_deref(), cfg!(windows)),
        };
        let peut_demander = politique.permission_mode != ClaudePermissionMode::DontAsk;
        (lancement, systeme, peut_demander)
    }
}

impl AgentHarness for HarnessClaude {
    fn id(&self) -> &str {
        CLAUDE_HARNESS_ID
    }

    fn label(&self) -> &str {
        "Claude"
    }

    fn icon(&self) -> Option<&str> {
        Some("C")
    }

    fn capabilities(&self) -> HarnessCapabilities {
        HarnessCapabilities {
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
            // La sortie structurée (actions rapides) : T8.
            structured_output: false,
            // La politique compilée en options, la GARDE par le hook `PreToolUse`, la CARTE par
            // `can_use_tool` ; le terminal par des règles (pas de bac à sable sous Windows).
            permission_prompts: true,
            policy: PolicySupport {
                tools: true,
                folders: true,
                network: true,
                shell: ShellSupport::Rules,
            },
        }
    }

    fn status(&self) -> HarnessStatus {
        // ★ Sans sonde : `ready` ne dit que « le binaire est là ». La connexion ne se sait qu'en
        //   essayant (le test de « Préparer l'agent » vient avec le moteur).
        let c = self.config.borrow();
        let explicite = c.binaire.components().count() > 1;
        if explicite && !c.binaire.is_file() {
            return HarnessStatus {
                ready: false,
                message: Some("Le moteur de l’agent (Claude Code) est introuvable.".into()),
            };
        }
        HarnessStatus {
            ready: true,
            message: None,
        }
    }

    fn models(&self) -> Vec<HarnessModel> {
        // Le modèle des réglages (texte libre) reste proposé même hors de la liste.
        let regle = self
            .config
            .borrow()
            .modele
            .clone()
            .filter(|m| !m.is_empty());
        let connus = modeles_connus();
        let defaut = regle
            .clone()
            .unwrap_or_else(|| MODELE_PAR_DEFAUT.to_string());
        let marquer = |mut m: HarnessModel, d: bool| {
            m.default = Some(d);
            m
        };
        match regle {
            Some(r) if !connus.iter().any(|m| m.id == r) => {
                let mut v = vec![HarnessModel {
                    id: r.clone(),
                    label: r,
                    efforts: Some(commande::EFFORTS.iter().map(|e| e.to_string()).collect()),
                    default: Some(true),
                    ..HarnessModel::default()
                }];
                v.extend(connus.into_iter().map(|m| marquer(m, false)));
                v
            }
            _ => connus
                .into_iter()
                .map(|m| {
                    let d = m.id == defaut;
                    marquer(m, d)
                })
                .collect(),
        }
    }

    fn run(
        &self,
        turn: HarnessTurn,
        io: &mut dyn HarnessIO,
    ) -> Result<Box<dyn HarnessRun>, ErreurHarness> {
        let id = turn.conversation.id.clone();
        let arret = self.interrupteur(&id);
        let (lancement, systeme, peut_demander) = self.preparer(&turn);
        let delais = self.config.borrow().delais;
        let resultat = self.lanceur.lancer(&lancement);
        let mut p = match resultat {
            Ok(p) => p,
            Err(e) => {
                self.arrets.borrow_mut().remove(&id);
                return Err(ErreurHarness::Indisponible(masquer_courriels(
                    &e.to_string(),
                )));
            }
        };
        let params = ParamsTour {
            systeme: &systeme,
            prompt: &turn.prompt,
            contenu: &turn.content,
            session_id: &id,
            peut_demander,
            delais,
        };
        let journal = |m: &str| self.noter(m);
        executer_tour(p.as_mut(), &params, io, &arret, &journal);
        self.arrets.borrow_mut().remove(&id);
        Ok(Box::new(TourClaude { arret }))
    }

    fn history(&self, conv: &HarnessConversation) -> Vec<ChatEntry> {
        let c = self.config.borrow();
        lire_historique(
            &c.dossier_config,
            c.racine_coffre.as_deref().unwrap_or_default(),
            &conv.id,
        )
    }
}
