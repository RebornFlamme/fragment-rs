// Port de app/src/ui/views/agent/AgentHost.ts (`send` : le tour, ses demandes, sa fin) et de
// sdkProcess.mjs (`send`, `demander` : une demande toujours résolue), réécrits pour le binaire
// `claude` piloté en stream-json (ADR-016 §1, §4, §5)
//
// Le CONTRÔLEUR d'un tour, sur le fil de l'appelant : il possède stdin, lit stdout ligne à ligne
// et sert chaque ligne — message ordinaire → `events::to_agent_events` → `HarnessIO::event` ;
// `hook_callback` → la garde ; `can_use_tool` → la carte ; `result` → `done`, fin du tour.
//
// ★ Le tour est SYNCHRONE (voir `contrat.rs`) : une interruption arrive par un drapeau partagé
//   ([`Interrupteur`]), levé par un autre fil ou depuis un rappel de `HarnessIO` (le bouton
//   « Arrêter » pendant une carte). Le contrôleur le regarde entre deux lignes et après chaque
//   rappel ; il envoie alors `interrupt` une seule fois.
// ★ Une demande du binaire reçoit TOUJOURS une réponse : après l'arrêt, un refus « Tour arrêté. ».

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::contrat::{AgentEvent, Entree, HarnessIO, PermissionDecision};
use crate::engine_model::at_least;
use crate::events::to_agent_events;
use crate::harness_claude::processus::{Lecture, Processus};
use crate::harness_claude::protocole::{
    REFUS_ARRET, demande_permission, masquer_courriels, message_utilisateur, reponse,
    reponse_erreur, reponse_garde, reponse_garde_arretee, reponse_permission, requete_initialize,
    requete_interruption, usage_de_la_garde,
};

/// La version minimale du binaire (ADR-016 §7) : les deux versions mesurées par S9.
pub const VERSION_MINIMALE: &str = "2.1.295";

const ID_INITIALIZE: &str = "fr-1";
const ID_INTERRUPTION: &str = "fr-2";

/// Le drapeau d'arrêt d'un tour. Clonable et `Send` : un autre fil peut le lever.
#[derive(Debug, Clone, Default)]
pub struct Interrupteur(Arc<AtomicBool>);

impl Interrupteur {
    pub fn new() -> Self {
        Self::default()
    }

    /// Demande l'arrêt du tour (sans effet une fois le tour fini).
    pub fn interrompre(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn est_demande(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Les délais d'un tour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delais {
    /// Entre deux regards sur le drapeau d'arrêt.
    pub sondage: Duration,
    /// Au plus, du lancement à la réponse à `initialize`.
    pub initialisation: Duration,
    /// Au plus, de la fermeture de stdin à la sortie (le processus est tué au-delà).
    pub sortie: Duration,
}

impl Default for Delais {
    fn default() -> Self {
        Self {
            sondage: Duration::from_millis(50),
            initialisation: Duration::from_secs(60),
            sortie: Duration::from_secs(10),
        }
    }
}

/// Ce qu'il faut pour mener un tour.
pub struct ParamsTour<'a> {
    pub systeme: &'a str,
    pub prompt: &'a str,
    pub contenu: &'a [Entree],
    /// L'id de la conversation (le `done` d'un tour qui n'a pas tourné le porte).
    pub session_id: &'a str,
    /// Le mode demande-t-il des cartes (tout sauf `dontAsk`) ?
    pub peut_demander: bool,
    pub delais: Delais,
}

/// Le journal du harness : ★ jamais la réponse à `initialize`, et chaque ligne passe par
/// [`masquer_courriels`].
pub type Journal<'a> = &'a dyn Fn(&str);

struct Controleur<'a, 'p> {
    p: &'a mut dyn Processus,
    io: &'a mut dyn HarnessIO,
    arret: &'a Interrupteur,
    journal: Journal<'a>,
    params: &'a ParamsTour<'p>,
    interruption_envoyee: bool,
}

/// Comment la lecture de la réponse à `initialize` a fini.
enum Init {
    Prete,
    Finie,
}

impl Controleur<'_, '_> {
    fn noter(&self, m: &str) {
        (self.journal)(&masquer_courriels(m));
    }

    fn erreur(&mut self, m: &str) {
        let message = masquer_courriels(m.trim());
        self.io.event(AgentEvent::Error { message });
    }

    /// La mort du processus : son code et la fin de stderr.
    fn mort(&mut self) {
        let code = self.p.attendre(self.params.delais.sortie);
        let stderr = self.p.fin_stderr();
        let code = code.map_or_else(|| "inconnu".to_string(), |c| c.to_string());
        let m = format!(
            "Le processus de l'agent s'est arrêté (code {code}). {}",
            stderr.trim()
        );
        self.erreur(&m);
    }

    fn ecrire(&mut self, v: &Value) -> bool {
        match self.p.ecrire(&v.to_string()) {
            Ok(()) => true,
            Err(e) => {
                self.noter(&format!("écriture impossible : {e}"));
                false
            }
        }
    }

    fn lire(&mut self) -> Lecture {
        self.p.lire(self.params.delais.sondage)
    }

    fn analyser(&self, ligne: &str) -> Option<Value> {
        match serde_json::from_str::<Value>(ligne) {
            Ok(v) => Some(v),
            Err(_) => {
                // Une ligne non JSON est journalisée, jamais fatale.
                let court: String = ligne.chars().take(200).collect();
                self.noter(&format!("ligne non JSON ignorée : {court}"));
                None
            }
        }
    }

    /// `initialize`, puis l'attente de sa réponse. ★ Sa réponse porte le COMPTE : elle n'est ni
    /// journalisée ni transmise, on n'en lit que le sous-type.
    fn initialiser(&mut self) -> Init {
        let req = requete_initialize(ID_INITIALIZE, true, self.params.systeme);
        if !self.ecrire(&req) {
            self.mort();
            return Init::Finie;
        }
        let debut = Instant::now();
        loop {
            match self.lire() {
                Lecture::Ligne(l) => {
                    let Some(v) = self.analyser(&l) else { continue };
                    match v.get("type").and_then(Value::as_str) {
                        Some("control_response")
                            if v.pointer("/response/request_id").and_then(Value::as_str)
                                == Some(ID_INITIALIZE) =>
                        {
                            if v.pointer("/response/subtype").and_then(Value::as_str)
                                == Some("error")
                            {
                                let m = v
                                    .pointer("/response/error")
                                    .and_then(Value::as_str)
                                    .unwrap_or("initialize refusé")
                                    .to_string();
                                self.erreur(&m);
                                return Init::Finie;
                            }
                            return Init::Prete;
                        }
                        // Une session absente à la reprise : un `result` AVANT la réponse.
                        Some("result") => {
                            let m = message_du_resultat(&v).unwrap_or_else(|| self.p.fin_stderr());
                            self.erreur(&m);
                            return Init::Finie;
                        }
                        Some("control_request") => self.servir(&v),
                        _ => {}
                    }
                }
                Lecture::Rien => {
                    if debut.elapsed() > self.params.delais.initialisation {
                        self.erreur("Le processus de l'agent ne répond pas (initialize).");
                        let _ = self.p.attendre(Duration::ZERO);
                        return Init::Finie;
                    }
                }
                Lecture::Fin => {
                    // Sorti avant `initialize` (option inconnue, UUID invalide…) : la fin de stderr.
                    self.mort();
                    return Init::Finie;
                }
            }
        }
    }

    fn arret_demande(&self) -> bool {
        self.arret.est_demande()
    }

    fn envoyer_interruption(&mut self) {
        if self.arret_demande() && !self.interruption_envoyee {
            self.interruption_envoyee = true;
            let v = requete_interruption(ID_INTERRUPTION);
            self.ecrire(&v);
        }
    }

    /// Une demande du binaire : la garde, la carte, ou un refus poli.
    fn servir(&mut self, v: &Value) {
        let id = v.get("request_id").cloned().unwrap_or(Value::Null);
        let req = v.get("request").cloned().unwrap_or(Value::Null);
        let corps = match req.get("subtype").and_then(Value::as_str) {
            Some("hook_callback") => {
                if self.arret_demande() {
                    reponse_garde_arretee()
                } else {
                    let usage = usage_de_la_garde(&req);
                    let d = self.io.guard(&usage);
                    reponse_garde(d.as_ref(), self.params.peut_demander)
                }
            }
            Some("can_use_tool") => {
                let demande = demande_permission(&req);
                let input = demande.input.clone();
                let d = if self.arret_demande() {
                    PermissionDecision::Deny {
                        message: Some(REFUS_ARRET.into()),
                    }
                } else {
                    let d = self.io.request_permission(demande);
                    // Arrêté pendant que la carte attendait : refusée, quoi qu'on ait répondu.
                    if self.arret_demande() {
                        PermissionDecision::Deny {
                            message: Some(REFUS_ARRET.into()),
                        }
                    } else {
                        d
                    }
                };
                reponse_permission(&d, &input)
            }
            autre => {
                self.noter(&format!(
                    "demande non prise en charge : {}",
                    autre.unwrap_or("?")
                ));
                let r = reponse_erreur(&id, "non pris en charge par Fragment");
                self.ecrire(&r);
                return;
            }
        };
        let r = reponse(&id, corps);
        self.ecrire(&r);
    }

    /// `system/init` : la version du binaire est journalisée (jamais le reste).
    fn verifier_version(&self, v: &Value) {
        if let Some(version) = v.get("claude_code_version").and_then(Value::as_str) {
            self.noter(&format!("claude {version}"));
            if !at_least(version, VERSION_MINIMALE) {
                self.noter(&format!(
                    "claude {version} est plus ancien que {VERSION_MINIMALE} : protocole non vérifié"
                ));
            }
        }
    }

    /// Le `done` du tour, corrigé : un tour qu'on a arrêté n'est pas une erreur ; une erreur
    /// du modèle en `success` + `is_error` porte son texte.
    fn fin(&mut self, v: &Value) {
        for e in to_agent_events(v) {
            let e = match e {
                AgentEvent::Done {
                    session_id, usage, ..
                } if self.interruption_envoyee => AgentEvent::Done {
                    session_id,
                    is_error: false,
                    errors: Vec::new(),
                    usage,
                },
                AgentEvent::Done {
                    session_id,
                    is_error: true,
                    errors,
                    usage,
                } if errors.is_empty() => AgentEvent::Done {
                    session_id,
                    is_error: true,
                    errors: message_du_resultat(v)
                        .map(|m| vec![masquer_courriels(&m)])
                        .unwrap_or_default(),
                    usage,
                },
                e => e,
            };
            self.io.event(e);
        }
    }

    fn mener(&mut self) {
        match self.initialiser() {
            Init::Finie => return,
            Init::Prete => {}
        }
        // Arrêté avant même le message : rien ne part, le tour finit sans erreur.
        if self.arret_demande() {
            self.p.fermer_entree();
            let _ = self.p.attendre(self.params.delais.sortie);
            self.io.event(AgentEvent::Done {
                session_id: self.params.session_id.to_string(),
                is_error: false,
                errors: Vec::new(),
                usage: None,
            });
            return;
        }
        let m = message_utilisateur(self.params.prompt, self.params.contenu);
        if !self.ecrire(&m) {
            self.mort();
            return;
        }
        loop {
            self.envoyer_interruption();
            let ligne = match self.lire() {
                Lecture::Ligne(l) => l,
                Lecture::Rien => continue,
                Lecture::Fin => {
                    self.mort();
                    return;
                }
            };
            let Some(v) = self.analyser(&ligne) else {
                continue;
            };
            match v.get("type").and_then(Value::as_str) {
                Some("control_request") => self.servir(&v),
                Some("control_response") => {}
                Some("result") => {
                    self.fin(&v);
                    break;
                }
                Some("system") => {
                    if v.get("subtype").and_then(Value::as_str) == Some("init") {
                        self.verifier_version(&v);
                    }
                }
                _ => {
                    for e in to_agent_events(&v) {
                        self.io.event(e);
                    }
                }
            }
        }
        // Le tour est fini : stdin fermé, le binaire sort (tué au-delà du délai).
        self.p.fermer_entree();
        let _ = self.p.attendre(self.params.delais.sortie);
    }
}

/// Le texte d'un `result` en erreur : ses `errors`, sinon son `result`.
fn message_du_resultat(v: &Value) -> Option<String> {
    let errors: Vec<&str> = v
        .get("errors")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !errors.is_empty() {
        return Some(errors.join(" "));
    }
    v.get("result")
        .and_then(Value::as_str)
        .filter(|r| !r.is_empty())
        .map(str::to_string)
}

/// Mène un tour sur un processus déjà lancé : tous ses événements passent par `io`, et le tour
/// finit toujours par un `done` ou une `error`.
pub fn executer_tour(
    p: &mut dyn Processus,
    params: &ParamsTour<'_>,
    io: &mut dyn HarnessIO,
    arret: &Interrupteur,
    journal: Journal<'_>,
) {
    let mut c = Controleur {
        p,
        io,
        arret,
        journal,
        params,
        interruption_envoyee: false,
    };
    c.mener();
}
