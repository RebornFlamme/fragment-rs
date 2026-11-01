// Port de app/src/ui/core/external/types.ts
//
// LE PORT DE REQUÊTE EXTERNE — le contrat générique d'une porte d'entrée.
//
// Une requête arrive de l'EXTÉRIEUR de l'app (le terminal `fragment`, une URL
// `fragment://`, demain un endpoint local) sous une forme neutre — une action, des
// paramètres chaîne — et repart en un résultat structuré. Un seul registre
// (`externalCommands`), un seul dispatcher (`dispatch::dispatch_external`), plusieurs
// portes par-dessus.
//
// ★ Rien de CLI ici : ni tuyau, ni code de sortie shell, ni grammaire d'argv. Un handler
//   écrit une fois répond aux DEUX portes.
// ★ ÉCART : le TS accepte un handler asynchrone. Le cœur n'a pas d'exécuteur : le handler
//   est SYNCHRONE ; l'hôte (fragment-app / fragment-plugins) qui a de l'async l'enrobe
//   (bloque ou fait suivre le résultat par ses propres moyens).

use std::collections::BTreeMap;
use std::rc::Rc;

use thiserror::Error;

/// Les paramètres d'une requête : un dico de chaînes (un drapeau vaut `"true"`).
pub type ExternalParams = BTreeMap<String, String>;

/// Deux codes génériques par défaut, choisis ÉGAUX à la table EXIT du shell pour que
/// l'adaptateur CLI n'ait rien à retraduire (`UNKNOWN_SUBCOMMAND` = 4, `HANDLER_ERROR` = 1).
pub const EXTERNAL_UNKNOWN: i32 = 4;
pub const EXTERNAL_HANDLER_ERROR: i32 = 1;

/// Une erreur qui PORTE un code.
///
/// Un handler qui échoue autrement rend `EXTERNAL_HANDLER_ERROR`. Rendre une
/// `ExternalError` choisit le code — « action introuvable » n'est pas un plantage.
/// ★ `CliError` (fragment-cli) en est un ALIAS.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct ExternalError {
    pub code: i32,
    pub message: String,
}

impl ExternalError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Ce qu'un handler peut lever : une erreur codée, ou n'importe quoi d'autre (message seul).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerError {
    External(ExternalError),
    Other(String),
}

impl From<ExternalError> for HandlerError {
    fn from(e: ExternalError) -> Self {
        Self::External(e)
    }
}

impl From<String> for HandlerError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for HandlerError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_owned())
    }
}

/// Le handler. Il REND sa sortie — il n'écrit pas dans stdout. Ce qu'il rend est imprimé
/// tel quel par le CLI ; l'URL, elle, l'ignore.
pub type ExternalHandler = Rc<dyn Fn(&ExternalParams) -> Result<String, HandlerError>>;

/// Une entrée du registre `externalCommands`.
#[derive(Clone)]
pub struct ExternalCommandSpec {
    /// L'id complet : `help`, `command`, ou `templater:create` (préfixé pour les plugins).
    pub action: String,
    pub description: String,
    /// Doc des paramètres acceptés, pour l'aide : nom → description. Un nom qui se termine
    /// par `=` attend une valeur (`id=`), sinon c'est un drapeau. Purement documentaire.
    pub params: BTreeMap<String, String>,
    pub handler: ExternalHandler,
}

impl ExternalCommandSpec {
    pub fn new(
        action: &str,
        handler: impl Fn(&ExternalParams) -> Result<String, HandlerError> + 'static,
    ) -> Self {
        Self {
            action: action.to_owned(),
            description: String::new(),
            params: BTreeMap::new(),
            handler: Rc::new(handler),
        }
    }
}

impl std::fmt::Debug for ExternalCommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalCommandSpec")
            .field("action", &self.action)
            .finish_non_exhaustive()
    }
}

/// Ce que rend le dispatcher. Même forme que le `CliResult` du fil, par design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalResult {
    Ok { output: String },
    Err { error: String, code: i32 },
}

impl ExternalResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Ajoutés : types.ts n'a pas de test propre ───
    #[test]
    fn external_error_porte_son_code_et_affiche_le_message() {
        let e = ExternalError::new(8, "indispo");
        assert_eq!(e.code, 8);
        assert_eq!(e.to_string(), "indispo");
    }

    #[test]
    fn les_codes_par_defaut_sont_ceux_de_la_table_exit() {
        assert_eq!(EXTERNAL_UNKNOWN, 4);
        assert_eq!(EXTERNAL_HANDLER_ERROR, 1);
    }
}
