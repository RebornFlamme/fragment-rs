// Port de app/src/ui/views/agent/harness/registry.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Accès au registre `agent:harness@1`, relu à chaque usage et jamais mis de côté :
//  un plugin de harness peut apparaître, disparaître ou se recharger n'importe quand.
//
//  Résolution identique aux registres de l'annotation : pour un même id, on prend la
//  priorité maximale, puis l'inscription la plus récente. Claude étant à −100, un
//  harness tiers inscrit à 0 sous le même id prend sa place.
//
//  ★ Écart : le registre générique `RegistryHub` / `OrderedRegistry` vit dans
//    fragment-core (pas encore porté). `HarnessRegistry` en est la version
//    minimale locale, de même sémantique ; l'id public reste `agent:harness@1`.
// ═══════════════════════════════════════════════════════════════════════════

use std::rc::Rc;

use crate::contrat::AgentHarness;

/// L'id du registre (contrat gelé).
pub const AGENT_HARNESS_REGISTRY_ID: &str = "agent:harness@1";

/// L'id du harness natif (le Claude Agent SDK) — celui d'une session sans `meta.harness`.
pub const CLAUDE_HARNESS_ID: &str = "claude";

/// Le jeton rendu par `register` : il sert à retirer SA contribution (le `retirer()` du TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jeton(u64);

struct Contribution {
    jeton: Jeton,
    priority: i32,
    harness: Rc<dyn AgentHarness>,
}

/// Le registre ordonné des harness, par id.
// TODO(contrat) : remplacer par fragment_core::RegistryHub::define::<dyn AgentHarness>("agent:harness@1").
#[derive(Default)]
pub struct HarnessRegistry {
    /// Dans l'ordre d'inscription des ids ; chaque liste est triée par priorité croissante (stable).
    entrees: Vec<(String, Vec<Contribution>)>,
    suivant: u64,
}

impl HarnessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inscrit un harness sous `id` (priorité par défaut du TS : 0).
    pub fn register(&mut self, id: &str, harness: Rc<dyn AgentHarness>, priority: i32) -> Jeton {
        let jeton = Jeton(self.suivant);
        self.suivant += 1;
        let c = Contribution {
            jeton,
            priority,
            harness,
        };
        let pos = match self.entrees.iter().position(|(k, _)| k == id) {
            Some(p) => p,
            None => {
                self.entrees.push((id.to_string(), Vec::new()));
                self.entrees.len() - 1
            }
        };
        let liste = &mut self.entrees[pos].1;
        // Insérée APRÈS les contributions de priorité égale : « le dernier inscrit gagne ».
        let rang = liste
            .iter()
            .position(|x| x.priority > priority)
            .unwrap_or(liste.len());
        liste.insert(rang, c);
        jeton
    }

    /// Retire une contribution (son `retirer()`).
    pub fn unregister(&mut self, id: &str, jeton: Jeton) {
        if let Some(pos) = self.entrees.iter().position(|(k, _)| k == id) {
            self.entrees[pos].1.retain(|c| c.jeton != jeton);
            if self.entrees[pos].1.is_empty() {
                self.entrees.remove(pos);
            }
        }
    }

    fn resoudre(liste: &[Contribution]) -> Option<Rc<dyn AgentHarness>> {
        liste.last().map(|c| Rc::clone(&c.harness))
    }

    /// Le harness d'id `id`, ou `None` (plugin absent ou désactivé).
    pub fn harness_of(&self, id: &str) -> Option<Rc<dyn AgentHarness>> {
        self.entrees
            .iter()
            .find(|(k, _)| k == id)
            .and_then(|(_, l)| Self::resoudre(l))
    }

    /// Tous les harness disponibles, dans l'ordre d'inscription des ids.
    pub fn harness_list(&self) -> Vec<Rc<dyn AgentHarness>> {
        self.entrees
            .iter()
            .filter_map(|(_, l)| Self::resoudre(l))
            .collect()
    }
}

/// Ce qu'une session dit de son moteur (`meta` de l'artefact `agent:session`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HarnessMeta {
    /// Absent : le harness Claude (toutes les sessions d'avant la v2).
    pub harness: Option<String>,
    /// L'état OPAQUE du harness pour cette conversation (`HarnessIO::set_state`).
    pub harness_state: Option<serde_json::Value>,
}

/// Le harness d'une conversation : celui de sa session si elle existe (une
/// conversation ne change JAMAIS de moteur), sinon le choix en attente, sinon le
/// défaut des réglages. Pur.
pub fn harness_id_for(
    meta: Option<&HarnessMeta>,
    exists: bool,
    pending: Option<&str>,
    fallback: Option<&str>,
) -> String {
    let non_vide = |s: Option<&str>| s.filter(|s| !s.is_empty()).map(str::to_string);
    if exists {
        return non_vide(meta.and_then(|m| m.harness.as_deref()))
            .unwrap_or_else(|| CLAUDE_HARNESS_ID.to_string());
    }
    non_vide(pending)
        .or_else(|| non_vide(fallback))
        .unwrap_or_else(|| CLAUDE_HARNESS_ID.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrat::{
        ChatEntry, ErreurHarness, HarnessCapabilities, HarnessConversation, HarnessIO,
        HarnessModel, HarnessRun, HarnessTurn, PolicySupport, ShellSupport,
    };

    struct Faux {
        id: String,
        label: String,
    }

    impl AgentHarness for Faux {
        fn id(&self) -> &str {
            &self.id
        }
        fn label(&self) -> &str {
            &self.label
        }
        fn capabilities(&self) -> HarnessCapabilities {
            HarnessCapabilities {
                resume: true,
                fork: false,
                thinking: false,
                text_deltas: true,
                attachments: vec![],
                structured_output: false,
                permission_prompts: false,
                policy: PolicySupport {
                    tools: false,
                    folders: false,
                    network: false,
                    shell: ShellSupport::None,
                },
            }
        }
        fn models(&self) -> Vec<HarnessModel> {
            vec![]
        }
        fn run(
            &self,
            _turn: HarnessTurn,
            _io: &mut dyn HarnessIO,
        ) -> Result<Box<dyn HarnessRun>, ErreurHarness> {
            Err(ErreurHarness::NonSupporte("run"))
        }
        fn history(&self, _conv: &HarnessConversation) -> Vec<ChatEntry> {
            vec![]
        }
    }

    fn harness(id: &str, label: &str) -> Rc<dyn AgentHarness> {
        Rc::new(Faux {
            id: id.into(),
            label: label.into(),
        })
    }

    #[test]
    fn par_id_la_priorite_la_plus_haute_gagne_puis_le_dernier_inscrit() {
        let mut reg = HarnessRegistry::new();
        let natif = harness("claude", "natif");
        reg.register("claude", Rc::clone(&natif), -100);
        assert!(Rc::ptr_eq(&reg.harness_of("claude").unwrap(), &natif));
        let remplacant = harness("claude", "tiers");
        let retirer = reg.register("claude", Rc::clone(&remplacant), 0);
        assert!(Rc::ptr_eq(&reg.harness_of("claude").unwrap(), &remplacant));
        reg.unregister("claude", retirer);
        assert!(Rc::ptr_eq(&reg.harness_of("claude").unwrap(), &natif));
    }

    #[test]
    fn un_id_inconnu_ou_vide_undefined_la_liste_ne_garde_que_les_ids_resolus() {
        let mut reg = HarnessRegistry::new();
        let retirer = reg.register("codex", harness("codex", "codex"), 0);
        reg.register("claude", harness("claude", "claude"), -100);
        let ids = |r: &HarnessRegistry| {
            r.harness_list()
                .iter()
                .map(|h| h.id().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&reg), vec!["codex".to_string(), "claude".to_string()]);
        reg.unregister("codex", retirer);
        assert!(reg.harness_of("codex").is_none());
        assert_eq!(ids(&reg), vec!["claude".to_string()]);
        assert!(reg.harness_of("absent").is_none());
    }

    #[test]
    fn le_moteur_dune_conversation_fige_par_sa_session_sinon_le_choix_en_attente_sinon_le_defaut() {
        // Une session d'avant la v2 (sans `harness`) est une session Claude.
        let vide = HarnessMeta::default();
        assert_eq!(
            harness_id_for(Some(&vide), true, Some("codex"), Some("codex")),
            CLAUDE_HARNESS_ID
        );
        let codex = HarnessMeta {
            harness: Some("codex".into()),
            harness_state: None,
        };
        assert_eq!(
            harness_id_for(Some(&codex), true, Some("autre"), None),
            "codex"
        );
        assert_eq!(
            harness_id_for(None, false, Some("codex"), Some("echo")),
            "codex"
        );
        assert_eq!(harness_id_for(None, false, None, Some("echo")), "echo");
        assert_eq!(harness_id_for(None, false, None, None), CLAUDE_HARNESS_ID);
    }
}
