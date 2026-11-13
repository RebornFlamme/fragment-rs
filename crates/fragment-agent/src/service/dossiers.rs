// Port de app/src/ui/views/agent/AgentPlugin.ts (`dossierAgent`, `screensDir`, `attachmentsDir`,
// `skillsPlugin`) et de skillsPlugin.ts (`lireDossier`, côté lecture)
//
// Les dossiers que l'agent touche, tous sous `<userData>/agent/` (hors du coffre : pas de bruit
// dans ses fichiers) :
// - `skills-plugin/` : LE DOSSIER DE L'AGENT (consignes, préprompt, skills), que l'agent édite
//   lui-même ; relu à chaque tour ;
// - `screens/<coffre>/` : les captures de ce que voit l'utilisateur ;
// - `attachments/<coffre>/` : les fichiers joints que le modèle ne lit pas en bloc.
//
// ★ Décision de l'orchestrateur (13/11) : fragment-rs ne livre ni ne réécrit AUCUN prompt ni skill
//   de Fragment. Il lit au lancement celui de l'utilisateur : d'abord son dossier, puis celui de
//   l'installation de Fragment s'il existe. Là où Fragment « livrait » son dossier (`planMirror`
//   écrit les fichiers d'une version livrée), ici il n'y a rien à écrire : le miroir est une
//   LECTURE, refaite à chaque tour.

use std::path::{Path, PathBuf};

use crate::harness_claude::dossier_agent::{
    DossierAgent, PREPROMPT_PATH, PROMPT_PATH, lire_dossier,
};
use crate::skills_model::MANIFEST_PATH;

/// Le nom d'un coffre rendu sûr pour un nom de dossier : tout ce qui n'est ni lettre ASCII, ni
/// chiffre, ni `_`, `.` ou `-` devient `_` (une suite d'exclus ne donne qu'un `_`).
pub fn nom_de_dossier(coffre: &str) -> String {
    let mut s = String::new();
    let mut dans_un_exclu = false;
    for c in coffre.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
            s.push(c);
            dans_un_exclu = false;
        } else if !dans_un_exclu {
            s.push('_');
            dans_un_exclu = true;
        }
    }
    s
}

/// Un dossier est-il celui d'un agent (a-t-il des consignes, un préprompt ou un manifeste de
/// plugin) ?
pub fn est_un_dossier_d_agent(d: &Path) -> bool {
    [PROMPT_PATH, PREPROMPT_PATH, MANIFEST_PATH]
        .iter()
        .any(|f| d.join(f).is_file())
}

/// Les chemins de l'agent pour un coffre.
#[derive(Debug, Clone)]
pub struct Dossiers {
    donnees: PathBuf,
    coffre: String,
    installation: Option<PathBuf>,
}

impl Dossiers {
    /// `donnees` : `<userData>` ; `coffre` : le nom du coffre ; `installation` : le dossier de
    /// l'agent de l'installation de Fragment, s'il y en a une.
    pub fn new(donnees: impl Into<PathBuf>, coffre: &str, installation: Option<PathBuf>) -> Self {
        Self {
            donnees: donnees.into(),
            coffre: nom_de_dossier(coffre),
            installation,
        }
    }

    /// `<userData>/agent/skills-plugin` : celui de l'utilisateur, que l'agent peut éditer.
    pub fn propre(&self) -> PathBuf {
        self.donnees.join("agent").join("skills-plugin")
    }

    /// `<userData>/agent/screens/<coffre>`.
    pub fn ecrans(&self) -> PathBuf {
        self.donnees
            .join("agent")
            .join("screens")
            .join(&self.coffre)
    }

    /// `<userData>/agent/attachments/<coffre>`.
    pub fn pieces(&self) -> PathBuf {
        self.donnees
            .join("agent")
            .join("attachments")
            .join(&self.coffre)
    }

    /// Le dossier LU à ce tour : le sien s'il est un dossier d'agent, sinon celui de
    /// l'installation de Fragment s'il en est un, sinon le sien (vide : rien à lire, et c'est
    /// là que l'agent écrira ses skills).
    pub fn actif(&self) -> PathBuf {
        let propre = self.propre();
        if est_un_dossier_d_agent(&propre) {
            return propre;
        }
        match &self.installation {
            Some(i) if est_un_dossier_d_agent(i) => i.clone(),
            _ => propre,
        }
    }

    /// Le dossier relu : consignes, préprompt, skills. Un dossier absent donne un dossier vide.
    pub fn lire(&self) -> DossierAgent {
        lire_dossier(&self.actif())
    }
}
