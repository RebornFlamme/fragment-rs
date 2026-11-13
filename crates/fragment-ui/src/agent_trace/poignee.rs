// Port de app/src/ui/views/agent/trace/traceView.ts (renderTrace : la poignée du part)
//! La POIGNÉE d'une trace : l'état vivant du part `agent-trace`, mis à jour à chaque version du
//! part au lieu d'être redessiné (chrono, état déplié, animations). Modèle pur : la vue GPUI
//! ([`super::TraceView`]) la tient, lui passe l'heure et peint son instantané ([`VueTrace`]).
//!
//! Ce que l'utilisateur a choisi (corps déplié, étapes dépliées) survit à une reconstruction du
//! message : cet état se range hors de la poignée, dans un [`EtatPartage`] clé par message.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use fragment_agent::trace_model::{
    OrbState, StepKind, Trace, TraceStatus, TraceStep, current_step, describe_step, duree,
    orb_state_of, seule_pensee, summarize,
};

use super::animation::{CaseVue, CoucheVue, Odometre, Repliable, TexteFondu, entree_etape};

/// L'orbe choisie dans les réglages de l'agent : `Auto` la laisse suivre l'étape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrbeChoix {
    #[default]
    Auto,
    Etat(OrbState),
}

/// Le nom de l'animation de l'orbe (`data-orb`).
#[must_use]
pub fn nom_orbe(o: OrbState) -> &'static str {
    match o {
        OrbState::Working => "working",
        OrbState::Searching => "searching",
        OrbState::Solving => "solving",
        OrbState::Listening => "listening",
        OrbState::Connecting => "connecting",
        OrbState::Weaving => "weaving",
        OrbState::Composing => "composing",
        OrbState::Breathing => "breathing",
        OrbState::Shaping => "shaping",
    }
}

/// Ce que l'utilisateur a choisi d'une trace, gardé d'une reconstruction à l'autre.
#[derive(Debug, Default)]
pub struct EtatUi {
    /// Le corps déplié par l'utilisateur (sinon : replié).
    corps: Option<bool>,
    /// Les étapes dépliées / repliées par l'utilisateur.
    etapes: HashMap<String, bool>,
    /// Les étapes déjà montrées (pas d'animation d'entrée à la reconstruction).
    vues: HashSet<String>,
    /// La trace était-elle déjà finie au premier rendu (relecture : rien n'anime) ?
    fini_au_depart: Option<bool>,
}

/// Un état d'interface partagé entre la poignée et ce qui la reconstruit.
pub type EtatPartage = Rc<RefCell<EtatUi>>;

/// Les états d'interface des traces, par clé (l'id du message).
#[derive(Debug, Default)]
pub struct EtatsTrace(HashMap<String, EtatPartage>);

impl EtatsTrace {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// L'état de la clé (créé au besoin).
    pub fn pour(&mut self, cle: &str) -> EtatPartage {
        self.0.entry(cle.to_string()).or_default().clone()
    }
}

/// L'état d'une étape à l'écran (`data-state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatEtape {
    Running,
    Done,
    Error,
}

/// Le marqueur d'une étape : l'arc qui tourne pendant, un point après.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marque {
    Arc,
    Point,
}

/// La cible d'une étape, en puce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CibleVue {
    pub texte: String,
    pub icone: Option<String>,
    pub mono: bool,
}

/// L'état d'une étape qui dure d'un rendu à l'autre.
#[derive(Debug)]
struct EtapeVivante {
    verbe: TexteFondu,
    detail: Repliable,
    /// Début de l'entrée depuis le flou (étape neuve), s'il y en a une.
    entree: Option<i64>,
    cible: Option<CibleVue>,
    temps: String,
    contenu: String,
    a_detail: bool,
    etat: EtatEtape,
    marque: Marque,
    kind: StepKind,
}

/// Ce que la vue peint d'une étape.
#[derive(Debug, Clone, PartialEq)]
pub struct EtapeVue {
    pub id: String,
    pub kind: StepKind,
    pub etat: EtatEtape,
    pub marque: Marque,
    pub verbe: Vec<CoucheVue>,
    pub cible: Option<CibleVue>,
    /// La durée d'une pensée finie (« 3s »), sinon vide.
    pub temps: String,
    /// Le détail (la pensée, ou les premières lignes du résultat).
    pub contenu: String,
    pub a_detail: bool,
    /// Le détail est ouvert (cible) ?
    pub ouvert: bool,
    /// Fraction de hauteur et opacité du détail à l'instant.
    pub detail_hauteur: f32,
    pub detail_opacite: f32,
    /// 0..1 : l'entrée de l'étape (1 : posée).
    pub entree: f32,
    /// L'étape vient d'entrer depuis le flou (`is-entering` posé à un moment).
    pub est_entree: bool,
}

/// Ce que la vue peint de la trace entière, à un instant.
#[derive(Debug, Clone, PartialEq)]
pub struct VueTrace {
    pub status: TraceStatus,
    /// Aucune étape : rien à déplier (pas de chevron).
    pub vide: bool,
    /// Une pensée SEULE : pas de liste.
    pub seule: bool,
    pub ouvert: bool,
    /// L'animation de l'orbe pendant le tour (`None` : tour fini, pas d'orbe).
    pub orbe: Option<OrbState>,
    pub titre: Vec<CoucheVue>,
    pub titre_texte: String,
    pub chrono: String,
    pub chrono_cases: Vec<CaseVue>,
    pub corps_hauteur: f32,
    pub corps_opacite: f32,
    /// Le corps est en mode nerveux (`is-gesture`).
    pub geste: bool,
    /// Durée et retard (ms) de la transition de hauteur du corps.
    pub transition_corps: (i64, i64),
    pub etapes: Vec<EtapeVue>,
}

/// La poignée d'une trace : `update` à chaque version du part, `vue` à chaque image.
#[derive(Debug)]
pub struct Poignee {
    ui: EtatPartage,
    trace: Trace,
    orbe: OrbeChoix,
    reduit: bool,
    titre: TexteFondu,
    chrono: Odometre,
    corps: Repliable,
    etapes: HashMap<String, EtapeVivante>,
    orbe_courante: Option<OrbState>,
}

/// Le texte d'un résultat montré : les 6 premières lignes non vides.
#[must_use]
pub fn extrait(texte: &str) -> String {
    let brut = texte.replace('\r', "");
    let lignes: Vec<&str> = brut.split('\n').filter(|l| !l.trim().is_empty()).collect();
    let mut sortie = lignes
        .iter()
        .take(6)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    if lignes.len() > 6 {
        sortie.push_str(&format!("\n… {} ligne(s) de plus", lignes.len() - 6));
    }
    sortie
}

fn en_cours(s: &TraceStep, trace: &Trace) -> bool {
    trace.status == TraceStatus::Running && s.ended_at.is_none()
}

impl Poignee {
    /// Le premier rendu d'une trace (rien ne s'anime : on pose l'état).
    #[must_use]
    pub fn new(
        ui: EtatPartage,
        trace: Trace,
        orbe: OrbeChoix,
        reduit: bool,
        maintenant: i64,
    ) -> Self {
        let mut p = Self {
            ui,
            trace,
            orbe,
            reduit,
            titre: TexteFondu::new(),
            chrono: Odometre::new(),
            corps: Repliable::new(),
            etapes: HashMap::new(),
            orbe_courante: None,
        };
        {
            let mut ui = p.ui.borrow_mut();
            if ui.fini_au_depart.is_none() {
                ui.fini_au_depart = Some(p.trace.status != TraceStatus::Running);
            }
        }
        p.corps.poser(p.ui.borrow().corps.unwrap_or(false));
        p.maj(false, maintenant);
        p
    }

    /// Une nouvelle version du part.
    pub fn update(&mut self, trace: Trace, maintenant: i64) {
        self.trace = trace;
        self.maj(true, maintenant);
    }

    /// L'orbe choisie dans les réglages (relayée depuis la fenêtre de réglages).
    pub fn definir_orbe(&mut self, orbe: OrbeChoix, maintenant: i64) {
        self.orbe = orbe;
        self.maj(false, maintenant);
    }

    #[must_use]
    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    fn fini_au_depart(&self) -> bool {
        self.ui.borrow().fini_au_depart.unwrap_or(false)
    }

    /// Un clic sur l'en-tête : bascule le corps (sans effet sans étape : la réponse s'écrit sans trace).
    pub fn clic_en_tete(&mut self, maintenant: i64) {
        if self.trace.steps.is_empty() {
            return;
        }
        let ouvert = !self.corps.ouvert();
        self.ui.borrow_mut().corps = Some(ouvert);
        self.corps.geste(maintenant);
        self.maj(false, maintenant);
    }

    /// Un clic sur la ligne d'une étape : bascule son détail (sans effet s'il n'en a pas).
    pub fn clic_etape(&mut self, id: &str, maintenant: i64) {
        let Some(e) = self.etapes.get_mut(id) else {
            return;
        };
        if !e.a_detail {
            return;
        }
        let ouvert = !e.detail.ouvert();
        e.detail.geste(maintenant);
        self.ui.borrow_mut().etapes.insert(id.to_string(), ouvert);
        self.maj(false, maintenant);
    }

    /// Le battement du chrono (toutes les 250 ms pendant le tour).
    pub fn tic(&mut self, maintenant: i64) {
        let fin = self.trace.ended_at.unwrap_or(maintenant);
        let valeur = self
            .trace
            .started_at
            .map(|d| duree(fin - d))
            .unwrap_or_default();
        let anime = !self.fini_au_depart();
        self.chrono.set(&valeur, anime, self.reduit, maintenant);
    }

    fn ouvrir_etape(&self, s: &TraceStep, derniere: bool) -> bool {
        if let Some(choix) = self.ui.borrow().etapes.get(&s.id) {
            return *choix;
        }
        if s.kind == StepKind::Thinking {
            return en_cours(s, &self.trace);
        }
        // La dernière étape finie montre son résultat tant que rien ne la suit.
        self.trace.status == TraceStatus::Running
            && derniere
            && s.result.as_ref().is_some_and(|r| !r.text.is_empty())
    }

    /// Pendant : l'étape en cours (« Lit cours.md… »), sinon la dernière (« A lu cours.md »).
    fn titre_en_cours(&self, ouvert: bool, seule: bool) -> String {
        if self.trace.writing == Some(true) {
            return "Écrit la réponse…".into();
        }
        let cur = current_step(&self.trace);
        // Déplié, l'étape en cours est juste dessous : l'en-tête ne la répète pas (sauf une
        // pensée seule : sans ligne, rien ne la répète).
        let Some(cur) = cur else {
            return "Au travail…".into();
        };
        if (ouvert && !seule) || cur.kind == StepKind::Note {
            return "Au travail…".into();
        }
        let actif = en_cours(cur, &self.trace);
        let d = describe_step(cur, actif);
        format!(
            "{}{}{}",
            d.verb,
            d.target.map(|t| format!(" {t}")).unwrap_or_default(),
            if actif { "…" } else { "" }
        )
    }

    fn maj_etape(&mut self, i: usize, anime: bool, seule: bool, maintenant: i64) {
        let n = self.trace.steps.len();
        let s = self.trace.steps[i].clone();
        let derniere = i + 1 == n;
        let running = en_cours(&s, &self.trace);
        let d = describe_step(&s, running);
        let ouvrir = seule || self.ouvrir_etape(&s, derniere);
        let reduit = self.reduit;
        let neuve = !self.etapes.contains_key(&s.id);
        let entre = {
            let ui = self.ui.borrow();
            neuve && !ui.vues.contains(&s.id) && ui.fini_au_depart != Some(true) && !reduit
        };
        self.ui.borrow_mut().vues.insert(s.id.clone());
        let v = self
            .etapes
            .entry(s.id.clone())
            .or_insert_with(|| EtapeVivante {
                verbe: TexteFondu::new(),
                detail: Repliable::new(),
                entree: entre.then_some(maintenant),
                cible: None,
                temps: String::new(),
                contenu: String::new(),
                a_detail: false,
                etat: EtatEtape::Done,
                marque: Marque::Point,
                kind: s.kind,
            });
        v.etat = if running {
            EtatEtape::Running
        } else if s.kind == StepKind::Tool && s.result.as_ref().is_some_and(|r| r.is_error) {
            EtatEtape::Error
        } else {
            EtatEtape::Done
        };
        v.marque = if running { Marque::Arc } else { Marque::Point };
        let suffixe = if running && s.kind != StepKind::Note {
            "…"
        } else {
            ""
        };
        v.verbe.set(
            &format!("{}{suffixe}", d.verb),
            anime,
            running,
            reduit,
            maintenant,
        );
        v.cible = d.target.map(|texte| CibleVue {
            texte,
            icone: d.target_icon,
            mono: d.mono == Some(true),
        });
        // La durée d'une pensée finie (« 3s »).
        v.temps = match (s.kind, s.started_at, s.ended_at) {
            (StepKind::Thinking, Some(a), Some(b)) if !running && b - a >= 1000 => duree(b - a),
            _ => String::new(),
        };
        // Le détail : la pensée (qui défile) ou le résultat.
        let texte = match s.kind {
            StepKind::Thinking => s.text.clone().unwrap_or_default(),
            StepKind::Tool => s
                .result
                .as_ref()
                .filter(|r| !r.text.is_empty())
                .map(|r| extrait(&r.text))
                .unwrap_or_default(),
            StepKind::Note => String::new(),
        };
        v.a_detail = !texte.trim().is_empty();
        v.contenu = texte;
        // Une pensée SEULE n'a pas de ligne : le corps de la trace EST sa pensée.
        let ouvert = v.a_detail && ouvrir;
        if neuve {
            v.detail.poser(ouvert);
        } else {
            v.detail.set(ouvert, maintenant);
        }
    }

    fn maj(&mut self, anime: bool, maintenant: i64) {
        let running = self.trace.status == TraceStatus::Running;
        let seule = seule_pensee(&self.trace);
        let ouvert = self.ui.borrow().corps.unwrap_or(false);
        self.corps.set(ouvert, maintenant);
        let titre = if running {
            self.titre_en_cours(ouvert, seule)
        } else {
            summarize(&self.trace)
        };
        self.titre
            .set(&titre, anime, running, self.reduit, maintenant);
        self.orbe_courante = running.then(|| match self.orbe {
            OrbeChoix::Auto => orb_state_of(&self.trace),
            OrbeChoix::Etat(o) => o,
        });
        self.tic(maintenant);
        for i in 0..self.trace.steps.len() {
            self.maj_etape(i, anime, seule, maintenant);
        }
        // Une étape qui n'est plus dans la trace s'en va avec elle.
        let ids: HashSet<&str> = self.trace.steps.iter().map(|s| s.id.as_str()).collect();
        self.etapes.retain(|id, _| ids.contains(id.as_str()));
    }

    /// Faut-il redessiner à la prochaine image (un geste, un chrono, un reflet en cours) ?
    #[must_use]
    pub fn anime(&self, maintenant: i64) -> bool {
        (self.trace.status == TraceStatus::Running && !self.reduit)
            || self.corps.anime(maintenant)
            || self.titre.anime(maintenant)
            || self.chrono.anime(maintenant)
            || self.etapes.values().any(|e| {
                e.detail.anime(maintenant)
                    || e.verbe.anime(maintenant)
                    || e.entree.is_some_and(|d| entree_etape(d, maintenant) < 1.0)
            })
    }

    /// L'instantané à peindre à `maintenant`.
    pub fn vue(&mut self, maintenant: i64) -> VueTrace {
        let titre = self.titre.couches(maintenant);
        let titre_texte = self.titre.texte().to_string();
        let chrono_cases = self.chrono.cases(maintenant);
        let seule = seule_pensee(&self.trace);
        let etapes = self
            .trace
            .steps
            .iter()
            .filter_map(|s| {
                let v = self.etapes.get_mut(&s.id)?;
                Some(EtapeVue {
                    id: s.id.clone(),
                    kind: v.kind,
                    etat: v.etat,
                    marque: v.marque,
                    verbe: v.verbe.couches(maintenant),
                    cible: v.cible.clone(),
                    temps: v.temps.clone(),
                    contenu: v.contenu.clone(),
                    a_detail: v.a_detail,
                    ouvert: v.detail.ouvert(),
                    detail_hauteur: v.detail.hauteur(maintenant),
                    detail_opacite: v.detail.opacite(maintenant),
                    entree: v.entree.map_or(1.0, |d| entree_etape(d, maintenant)),
                    est_entree: v.entree.is_some(),
                })
            })
            .collect();
        VueTrace {
            status: self.trace.status,
            vide: self.trace.steps.is_empty(),
            seule,
            ouvert: self.corps.ouvert(),
            orbe: self.orbe_courante,
            titre,
            titre_texte,
            chrono: self.chrono.texte().to_string(),
            chrono_cases,
            corps_hauteur: self.corps.hauteur(maintenant),
            corps_opacite: self.corps.opacite(maintenant),
            geste: self.corps.geste_actif(maintenant),
            transition_corps: self.corps.transition_hauteur(maintenant),
            etapes,
        }
    }

    /// La fin de la vie de la poignée (la minuterie du chrono tombe avec la vue).
    pub fn destroy(&mut self) {
        self.orbe_courante = None;
    }
}
