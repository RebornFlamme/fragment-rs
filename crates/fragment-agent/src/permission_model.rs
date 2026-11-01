// Port de app/src/ui/views/agent/policy/permissionCard.ts (le modèle pur ; le rendu DOM reste côté UI)
//
// La carte d'autorisation — ses DONNÉES et son texte, purs.
// (Le fichier de tests TS s'appelle permissionModel.test.ts : il importe `permissionModel`,
//  qui est le nom du module dans la spec de la tâche.)

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::contrat::{Entree, PermanentRule, PermissionRequest, RememberScope, ToolFamily};
use crate::utf16::tete_utf16;

/// L'état d'une carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EtatCarte {
    Pending,
    Allowed,
    Denied,
}

/// Les données d'un part `agent-permission`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionCardData {
    /// L'id de la demande (le fournisseur la tient en attente).
    pub id: String,
    pub request: PermissionRequest,
    pub state: EtatCarte,
    /// La règle qu'un « toujours » retiendrait (absente : pas de « toujours »).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<PermanentRule>,
    /// Ce qui a été répondu (le message du refus, « toujours dans ce coffre »…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Ce que la carte dit d'une demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionPermission {
    pub icon: String,
    pub verb: String,
    pub detail: String,
    pub short: String,
}

fn s(i: &Entree, k: &str) -> String {
    i.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn d(
    icon: &str,
    verb: impl Into<String>,
    detail: impl Into<String>,
    short: impl Into<String>,
) -> DescriptionPermission {
    DescriptionPermission {
        icon: icon.into(),
        verb: verb.into(),
        detail: detail.into(),
        short: short.into(),
    }
}

/// Le premier non vide (`a || b || c` du JS).
fn ou(valeurs: [String; 3]) -> String {
    valeurs
        .into_iter()
        .find(|v| !v.is_empty())
        .unwrap_or_default()
}

/// `JSON.stringify(i, null, 2)` tronqué à 600 unités.
fn json_indente(i: &Entree) -> String {
    serde_json::to_string_pretty(&Value::Object(i.clone())).unwrap_or_default()
}

/// Ce que la carte dit d'une demande : l'icône, le verbe, le détail (code), la forme courte.
pub fn describe_permission(r: &PermissionRequest) -> DescriptionPermission {
    let i = &r.input;
    let chemin = ou([s(i, "file_path"), s(i, "notebook_path"), s(i, "path")]);
    let nom = chemin
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&chemin)
        .to_string();
    if r.plan.is_some() {
        return d("list-checks", "Propose un plan", "", "le plan");
    }
    match r.family {
        Some(f @ (ToolFamily::Shell | ToolFamily::FragmentCli | ToolFamily::Eval)) => {
            let cmd = s(i, "command");
            let verbe = if f == ToolFamily::Eval {
                "Veut exécuter du JavaScript dans l’app"
            } else {
                "Veut exécuter une commande"
            };
            d("terminal", verbe, cmd.clone(), tete_utf16(&cmd, 80))
        }
        Some(ToolFamily::Write) => d(
            "file-plus",
            format!("Veut créer « {nom} »"),
            chemin.clone(),
            chemin,
        ),
        Some(ToolFamily::Edit) => d(
            "file-pen",
            format!("Veut modifier « {nom} »"),
            chemin.clone(),
            chemin,
        ),
        Some(ToolFamily::Read) => d(
            "file-text",
            format!("Veut lire « {nom} »"),
            chemin.clone(),
            chemin,
        ),
        Some(ToolFamily::Search) => {
            let detail = [s(i, "pattern"), chemin.clone()]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join("  dans  ");
            let court = if s(i, "pattern").is_empty() {
                chemin
            } else {
                s(i, "pattern")
            };
            d("search", "Veut chercher", detail, court)
        }
        Some(ToolFamily::WebFetch) => {
            d("globe", "Veut lire une page web", s(i, "url"), s(i, "url"))
        }
        Some(ToolFamily::WebSearch) => d(
            "globe",
            "Veut chercher sur le web",
            s(i, "query"),
            s(i, "query"),
        ),
        Some(ToolFamily::Subagents) => {
            let detail = if s(i, "description").is_empty() {
                tete_utf16(&s(i, "prompt"), 200).to_string()
            } else {
                s(i, "description")
            };
            let court = if s(i, "description").is_empty() {
                "sous-agent".to_string()
            } else {
                s(i, "description")
            };
            d("users", "Veut lancer un sous-agent", detail, court)
        }
        _ => {
            let verbe = r
                .title
                .clone()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| format!("Veut utiliser {}", r.tool));
            d(
                "wrench",
                verbe,
                tete_utf16(&json_indente(i), 600),
                r.tool.clone(),
            )
        }
    }
}

/// La note d'une réponse (la ligne repliée).
pub fn answer_note(remember: Option<RememberScope>, message: Option<&str>) -> Option<String> {
    match remember {
        Some(RememberScope::Conversation) => Some("toujours dans cette conversation".to_string()),
        Some(RememberScope::Vault) => Some("toujours dans ce coffre".to_string()),
        None => message
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(
        tool: &str,
        famille: Option<ToolFamily>,
        input: Value,
        plan: Option<&str>,
    ) -> PermissionRequest {
        PermissionRequest {
            tool: tool.into(),
            family: famille,
            input: input.as_object().cloned().unwrap_or_default(),
            title: None,
            reason: None,
            plan: plan.map(str::to_string),
        }
    }

    #[test]
    fn dit_ce_que_veut_lagent_commande_fichier_plan() {
        let c = describe_permission(&req(
            "Bash",
            Some(ToolFamily::Shell),
            json!({ "command": "git status" }),
            None,
        ));
        assert_eq!(
            (c.verb.as_str(), c.detail.as_str()),
            ("Veut exécuter une commande", "git status")
        );
        assert_eq!(
            describe_permission(&req(
                "Edit",
                Some(ToolFamily::Edit),
                json!({ "file_path": "Cours/a.md" }),
                None
            ))
            .verb,
            "Veut modifier « a.md »"
        );
        assert_eq!(
            describe_permission(&req("ExitPlanMode", None, json!({}), Some("# p"))).verb,
            "Propose un plan"
        );
    }

    #[test]
    fn la_note_dune_reponse() {
        assert_eq!(
            answer_note(Some(RememberScope::Vault), None).as_deref(),
            Some("toujours dans ce coffre")
        );
        assert_eq!(answer_note(None, Some(" non ")).as_deref(), Some("non"));
        assert_eq!(answer_note(None, None), None);
    }
}
