// Port de app/src/ui/core/cli/types.ts, app/src/ui/core/cli/CliHost.ts,
// app/src/ui/core/external/types.ts (ExternalError) et de `InstanceInfo` (app/src/ui/core/app.ts).
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les types du CLI vus du CŒUR — les noms publics gelés de docs/contrats.md.
//
//  Côté public, l'API reste celle d'Obsidian 1.12 : un plugin inscrit son handler par
//  `plugin.registerCliHandler(command, description, flags, handler)`, et ce handler
//  reçoit un `CliData` pour rendre une chaîne, directement ou par une promesse.
//
//  ★ `CliData` a LA MÊME FORME que les paramètres d'`obsidian://` — un dictionnaire
//    de chaînes, où un drapeau vaut `"true"`. Ce n'est pas un raccourci : c'est ce qui
//    fait qu'un handler écrit pour le CLI se branche tel quel sur une URL.
//
//  ★ `CliHost` est un CONTRAT, pas une implémentation : `eval` s'exécute hors de la
//    CSP de la page et `install_shim` écrit sur le disque et le PATH — deux choses que
//    le cœur ne fait pas seul. Le cœur ne touche jamais l'hôte directement : il passe
//    par ce service nommé, câblé au boot par `fragment-app`.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;

use crate::protocol::{CliResult, EXIT};

/// Ce qu'un handler reçoit : les `clé=valeur`, plus chaque `--drapeau` en `"true"`.
pub type CliData = BTreeMap<String, String>;

/// Ce qu'un handler ACCEPTE, pour `help` : nom → description. Un nom qui se termine
/// par `=` attend une valeur (`id=`), sinon c'est un drapeau (`total`). Documentaire.
pub type CliFlags = BTreeMap<String, String>;

/// Le handler. Il REND sa sortie — il n'écrit pas dans stdout, il n'en a pas.
///
/// ⚠ Doute (signalé dans la PR) : le TS accepte `string | Promise<string>`. Le contrat
/// Rust est SYNCHRONE (`fragment-cli` ne connaît aucun exécuteur asynchrone) ; un
/// handler qui attend le fait en interne ou l'hôte l'exécute sur son propre fil.
pub type CliHandler = Box<dyn Fn(&CliData) -> Result<String, CliError> + Send + Sync>;

/// Une entrée du registre des commandes externes (alias fidèle d'`ExternalCommandSpec`).
pub struct CliHandlerSpec {
    /// L'id complet : `help`, `command`, ou `templater:create` (préfixé pour les plugins).
    pub action: String,
    pub description: String,
    /// Doc des paramètres acceptés — purement documentaire, rien n'est validé contre.
    pub params: CliFlags,
    pub handler: CliHandler,
}

impl std::fmt::Debug for CliHandlerSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliHandlerSpec")
            .field("action", &self.action)
            .field("description", &self.description)
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

/// Une erreur qui PORTE un code de sortie.
///
/// Un handler qui échoue autrement rend `EXIT.HANDLER_ERROR`. Lever une `CliError`
/// choisit le code — « commande introuvable » (7) n'est pas un plantage.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct CliError {
    pub code: i32,
    pub message: String,
}

impl CliError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Le code par défaut d'un handler qui échoue : `EXIT.HANDLER_ERROR`.
    pub fn handler(message: impl Into<String>) -> Self {
        Self::new(EXIT.HANDLER_ERROR, message)
    }
}

/// Le résultat d'`install_shim`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliInstallResult {
    pub message: String,
    /// Le shim écrit (chemin absolu) — pour qui lance `fragment` lui-même (l'agent).
    pub shim: String,
}

/// Les capacités CLI que le processus principal fournit au cœur. Injecté au boot ;
/// jamais construit par le cœur.
pub trait CliHost {
    /// Évalue `code` hors CSP et rend sa valeur de complétion sérialisée. Rend un
    /// [`CliResult`] (A21) : `code` = `EXIT.TIMEOUT` si l'éval a dépassé `timeout_ms`,
    /// `EXIT.HANDLER_ERROR` sinon — le même vocabulaire que toute la CLI.
    fn eval(&self, code: &str, timeout_ms: u64) -> CliResult;

    /// Écrit le shim `fragment` et le met sur le PATH.
    fn install_shim(&self) -> Result<CliInstallResult, CliError>;
}

/// L'identité de CETTE instance — le coffre ouvert, où sont ses données, quelle
/// version. Générique (pas CLI) : tout ce qui écrit par-instance ou coordonne des
/// fenêtres lit ici, plutôt que de reconstruire ces chemins.
///
/// ★ « Fenêtre active » n'est PAS modélisé : un coffre = une fenêtre = une instance.
///   Le jour où le multi-fenêtre arrive, ce type s'élargit ici.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceInfo {
    /// La racine absolue du coffre.
    pub vault_path: String,
    /// Le nom du dossier du coffre.
    pub vault_name: String,
    /// Le dossier de données de l'app — où vit l'index des instances CLI, p. ex.
    pub user_data_dir: String,
    /// La version de Fragment.
    pub app_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pas de test TS pour ces types (aucun `.test.ts` ne les couvre) : ces tests
    // vérifient seulement la forme du contrat Rust.

    #[test]
    fn un_handler_recoit_les_drapeaux_en_true_et_rend_sa_sortie() {
        let spec = CliHandlerSpec {
            action: "tasks".into(),
            description: "liste les tâches".into(),
            params: CliFlags::from([("total".to_string(), "compte seulement".to_string())]),
            handler: Box::new(|d: &CliData| Ok(format!("total={}", d["total"]))),
        };
        let data = CliData::from([("total".to_string(), "true".to_string())]);
        assert_eq!((spec.handler)(&data), Ok("total=true".to_string()));
    }

    #[test]
    fn une_cli_error_porte_son_code_et_son_message() {
        let e = CliError::new(EXIT.UNKNOWN_COMMAND, "commande introuvable");
        assert_eq!(e.code, 7);
        assert_eq!(e.to_string(), "commande introuvable");
        assert_eq!(CliError::handler("boum").code, EXIT.HANDLER_ERROR);
    }
}
