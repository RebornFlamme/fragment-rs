// Port de app/src/ui/views/agent/skillsPlugin.ts (`lireDossier`) et de AgentHost.ts
// (`systemPrompt`, la composition du prompt du tour)
//
// Le DOSSIER DE L'AGENT de l'utilisateur : ses consignes (`prompt.md`), son préprompt
// pédagogique (`preprompt.md`) et ses skills (un plugin local de Claude Code). Il est RELU à
// chaque tour : l'agent peut le perfectionner lui-même.
//
// ★ Décision de l'orchestrateur (13/11) : aucun prompt ni skill de Fragment n'est publié ou
//   réécrit dans ce dépôt. Rien n'est donc livré ici : sans `prompt.md`, le prompt du tour est
//   celui que l'hôte passe (`HarnessTurn::instructions`) ; sans `preprompt.md`, pas de section
//   pédagogique. Les tests utilisent un prompt FACTICE.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::skills_model::{MANIFEST_PATH, compose_system, plugin_name, render_prompt};

pub const PROMPT_PATH: &str = "prompt.md";
pub const PREPROMPT_PATH: &str = "preprompt.md";

/// Ce que le dossier de l'agent contient à ce tour.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DossierAgent {
    pub dir: String,
    /// Les skills présents, noms qualifiés `<plugin>:<skill>`, triés.
    pub skills: Vec<String>,
    pub prompt: Option<String>,
    pub preprompt: Option<String>,
}

/// Relit le dossier ; un fichier absent ou illisible vaut `None`.
pub fn lire_dossier(dir: &Path) -> DossierAgent {
    let lire = |rel: &str| fs::read_to_string(dir.join(rel)).ok();
    let plugin = lire(MANIFEST_PATH).and_then(|m| plugin_name(&m).ok());
    let mut skills = Vec::new();
    if let Some(plugin) = plugin
        && let Ok(entrees) = fs::read_dir(dir.join("skills"))
    {
        for e in entrees.flatten() {
            let p = e.path();
            if p.is_dir() && p.join("SKILL.md").is_file() {
                skills.push(format!("{plugin}:{}", e.file_name().to_string_lossy()));
            }
        }
    }
    skills.sort();
    DossierAgent {
        dir: dir.display().to_string(),
        skills,
        prompt: lire(PROMPT_PATH),
        preprompt: lire(PREPROMPT_PATH),
    }
}

/// Le prompt système d'un tour : le prompt du dossier (variables `{{vault}}`, `{{dir}}`), sinon
/// les consignes de l'hôte ; le préprompt du dossier ; le profil de la conversation en plus.
pub fn prompt_systeme(
    dossier: Option<&DossierAgent>,
    consignes: &str,
    coffre: &str,
    profil: Option<&str>,
) -> String {
    let base = match dossier.and_then(|d| d.prompt.as_deref()) {
        Some(gabarit) => {
            let vars: BTreeMap<String, String> = [
                ("vault".to_string(), coffre.to_string()),
                (
                    "dir".to_string(),
                    dossier.map(|d| d.dir.clone()).unwrap_or_default(),
                ),
            ]
            .into_iter()
            .collect();
            render_prompt(gabarit, &vars)
        }
        None => consignes.to_string(),
    };
    compose_system(&base, dossier.and_then(|d| d.preprompt.as_deref()), profil)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sans_dossier_les_consignes_de_l_hote_et_le_profil() {
        let p = prompt_systeme(None, "Consignes factices.", "Coffre", Some("Profil."));
        assert_eq!(p, "Consignes factices.\n\n# Cette conversation\n\nProfil.");
    }

    #[test]
    fn le_prompt_du_dossier_remplace_les_consignes() {
        let d = DossierAgent {
            dir: "C:/agent".into(),
            skills: vec![],
            prompt: Some("Coffre {{vault}}, dossier {{dir}}.".into()),
            preprompt: Some("Explique.".into()),
        };
        let p = prompt_systeme(Some(&d), "ignoré", "Notes", None);
        assert_eq!(
            p,
            "Coffre Notes, dossier C:/agent.\n\n# Pédagogie\n\nExplique."
        );
    }
}
