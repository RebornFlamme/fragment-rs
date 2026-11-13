// Port de app/src/ui/views/agent/AgentPlugin.ts (`AgentSettingTab.getSettingDefinitions`)
//
//! L'onglet « Agent » de la fenêtre de réglages (`fr-t7-reglages-agent`), en définitions
//! DÉCLARATIVES : moteur, modèle, nom, orbe, barre, clé d'API ([`generales`]), puis les
//! permissions ([`permissions`]). Les actions rapides sont à T8.
//!
//! ★ Le `data.json` de l'agent n'est pas relu par une structure fermée : on passe par
//!   [`Reglages`] du service, qui garde les clés des autres tâches. Chaque réglage écrit sa clé
//!   (`null` la retire : le choix « automatique » ne laisse pas de trace dans le fichier).
//! ★ La fenêtre de réglages et le service sont deux instances de [`Reglages`] sur le même
//!   fichier : une règle « toujours » posée par une carte n'est vue ici qu'après un
//!   `recharger()`, que [`definitions_agent`] fait à CHAQUE construction.
//! ★ Les définitions sont reconstruites à chaque rendu : les brouillons des lignes « Ajouter »
//!   vivent donc dans le contexte, pas dans la définition.

pub mod generales;
pub mod permissions;

use std::cell::RefCell;
use std::rc::Rc;

use fragment_agent::harness_registry::CLAUDE_HARNESS_ID;
use fragment_agent::service::reglages::Reglages;
use fragment_agent::trace_model::OrbState;
use fragment_core::{CommandManager, SecretStorage, SettingDefinition, SettingDefinitionBase};

use crate::agent_trace::OrbeChoix;
use crate::reglages::{OngletReglages, SectionOnglet};

/// Les sous-dossiers directs d'un dossier du coffre.
pub type ListeDossiers = Rc<dyn Fn(&str) -> Vec<String>>;

/// Les textes saisis dans les lignes « Ajouter », entre deux rendus.
#[derive(Debug, Default)]
pub(crate) struct Brouillons {
    pub commande: String,
    pub domaine: String,
    pub dehors: String,
}

/// Ce dont l'onglet a besoin du monde extérieur.
#[derive(Clone)]
pub struct ContexteAgent {
    /// Le `data.json` de l'agent.
    pub reglages: Rc<Reglages>,
    /// Les moteurs inscrits (id, libellé) : Claude, plus ceux des plugins.
    pub moteurs: Vec<(String, String)>,
    /// Le stockage des secrets (la clé d'API s'y écrit).
    pub secrets: Rc<dyn SecretStorage>,
    /// Le compte du système (placeholder du nom).
    pub compte: Option<String>,
    /// `win32`, `darwin`, `linux` : les avertissements en dépendent.
    pub plateforme: String,
    /// Les sous-dossiers directs d'un dossier du coffre (`""` = la racine), chemins complets.
    pub dossiers: ListeDossiers,
    pub(crate) brouillons: Rc<RefCell<Brouillons>>,
}

impl ContexteAgent {
    pub fn new(reglages: Rc<Reglages>, secrets: Rc<dyn SecretStorage>) -> Self {
        let plateforme = match std::env::consts::OS {
            "windows" => "win32",
            "macos" => "darwin",
            autre => autre,
        };
        Self {
            reglages,
            moteurs: vec![(CLAUDE_HARNESS_ID.to_string(), "Claude".to_string())],
            secrets,
            compte: None,
            plateforme: plateforme.to_string(),
            dossiers: Rc::new(|_| Vec::new()),
            brouillons: Rc::default(),
        }
    }

    #[must_use]
    pub fn avec_moteurs(mut self, moteurs: Vec<(String, String)>) -> Self {
        self.moteurs = moteurs;
        self
    }

    #[must_use]
    pub fn avec_compte(mut self, compte: Option<String>) -> Self {
        self.compte = compte;
        self
    }

    #[must_use]
    pub fn avec_plateforme(mut self, plateforme: &str) -> Self {
        self.plateforme = plateforme.to_string();
        self
    }

    #[must_use]
    pub fn avec_dossiers(mut self, f: ListeDossiers) -> Self {
        self.dossiers = f;
        self
    }
}

pub(crate) fn base(id: Option<&str>) -> SettingDefinitionBase {
    SettingDefinitionBase {
        id: id.map(str::to_owned),
    }
}

/// Les définitions de l'onglet : réglages généraux, puis permissions.
pub fn definitions_agent(ctx: &ContexteAgent) -> Vec<SettingDefinition> {
    ctx.reglages.recharger();
    let mut defs = generales::definitions(ctx);
    defs.extend(permissions::definitions(ctx));
    defs
}

/// L'onglet « Agent » du rail (icône `sparkles`, section cœur).
pub fn onglet_agent(ctx: &ContexteAgent) -> OngletReglages {
    let ctx = ctx.clone();
    OngletReglages::new("agent", "Agent", "sparkles", SectionOnglet::Core)
        .avec_definitions(Rc::new(move |_theme| definitions_agent(&ctx)))
}

/// Les orbes, dans l'ordre de leur menu : (état, nom écrit au `data.json`, libellé).
pub const ORBES: [(OrbState, &str, &str); 9] = [
    (OrbState::Working, "working", "Orbites"),
    (OrbState::Searching, "searching", "Globe"),
    (OrbState::Solving, "solving", "Résolution"),
    (OrbState::Listening, "listening", "Onde"),
    (OrbState::Connecting, "connecting", "Constellation"),
    (OrbState::Weaving, "weaving", "Tresse"),
    (OrbState::Composing, "composing", "Écharpe"),
    (OrbState::Breathing, "breathing", "Anneau"),
    (OrbState::Shaping, "shaping", "Formes"),
];

/// L'orbe choisie, telle que la trace la lit : un nom inconnu vaut « automatique ».
pub fn choix_orbe(reglages: &Reglages) -> OrbeChoix {
    let nom = reglages.orbe();
    ORBES
        .iter()
        .find(|(_, n, _)| Some(*n) == nom.as_deref())
        .map_or(OrbeChoix::Auto, |(e, _, _)| OrbeChoix::Etat(*e))
}

/// Les commandes de scène `pixels:agent_reglages` : aucune (l'onglet se rend par la fenêtre).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
