// Port de sdkProcess.mjs (le hook `PreToolUse`, `canUseTool`, l'entrée en flux) et de
// claudeHarness.ts (`permissionsDuTour` : garde et carte), réécrits pour le protocole de contrôle
// du binaire `claude` (ADR-016 §4)
//
// Les messages que le harness ÉCRIT, et ses réponses aux demandes du binaire. PUR : ni processus
// ni état. Ce que le SDK faisait par des rappels (des fonctions, que le JSON ne porte pas), le
// binaire le demande par des `control_request` ; on y répond par des `control_response`.

use std::sync::LazyLock;

use serde_json::{Value, json};

use crate::contrat::{Entree, GuardDecision, PermissionDecision, PermissionRequest, ToolUse};
use crate::motif::Motif;
use crate::policy_model::family_of;

/// L'id du rappel de garde déclaré dans `initialize`.
pub const ID_GARDE: &str = "garde";
/// La réponse d'une demande que l'arrêt du tour a coupée.
pub const REFUS_ARRET: &str = "Tour arrêté.";
/// La réponse d'un refus sans « pourquoi ».
pub const REFUS_DEFAUT: &str = "Refusé par l’utilisateur.";

/// `initialize` : la garde (un hook `PreToolUse` avant CHAQUE outil) et le prompt système.
/// ★ Le prompt passe ici, jamais en ligne de commande (32 k caractères au plus sous Windows).
pub fn requete_initialize(request_id: &str, garde: bool, systeme: &str) -> Value {
    let mut req = json!({"subtype": "initialize", "systemPrompt": [systeme]});
    if garde {
        req["hooks"] = json!({"PreToolUse": [{"matcher": null, "hookCallbackIds": [ID_GARDE]}]});
    }
    json!({"type": "control_request", "request_id": request_id, "request": req})
}

/// L'interruption du tour en cours.
pub fn requete_interruption(request_id: &str) -> Value {
    json!({"type": "control_request", "request_id": request_id, "request": {"subtype": "interrupt"}})
}

/// Le message de l'utilisateur. Avec des pièces jointes : un seul message [blocs…, texte].
pub fn message_utilisateur(prompt: &str, contenu: &[Entree]) -> Value {
    let content = if contenu.is_empty() {
        Value::String(prompt.to_string())
    } else {
        let mut blocs: Vec<Value> = contenu.iter().cloned().map(Value::Object).collect();
        blocs.push(json!({"type": "text", "text": prompt}));
        Value::Array(blocs)
    };
    json!({
        "type": "user",
        "message": {"role": "user", "content": content},
        "parent_tool_use_id": null,
        "session_id": "",
    })
}

/// Une réponse réussie à une demande du binaire.
pub fn reponse(request_id: &Value, corps: Value) -> Value {
    json!({
        "type": "control_response",
        "response": {"subtype": "success", "request_id": request_id, "response": corps},
    })
}

/// Une demande que le harness ne sait pas servir.
pub fn reponse_erreur(request_id: &Value, message: &str) -> Value {
    json!({
        "type": "control_response",
        "response": {"subtype": "error", "request_id": request_id, "error": message},
    })
}

/// L'usage d'outil d'un `hook_callback`.
pub fn usage_de_la_garde(request: &Value) -> ToolUse {
    let input = request.get("input");
    ToolUse {
        tool: input
            .and_then(|i| i.get("tool_name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        input: input
            .and_then(|i| i.get("tool_input"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    }
}

/// La réponse de la garde : `{}` laisse les règles du binaire décider ; `ask` force la carte (même
/// pour ce que Claude Code tient pour lecture seule) — sauf quand le mode ne demande jamais.
pub fn reponse_garde(decision: Option<&GuardDecision>, peut_demander: bool) -> Value {
    match decision {
        Some(GuardDecision::Deny { message }) => json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": message,
        }}),
        Some(GuardDecision::Ask) if peut_demander => json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "ask",
        }}),
        _ => json!({}),
    }
}

/// La garde d'un tour arrêté : refusé.
pub fn reponse_garde_arretee() -> Value {
    reponse_garde(
        Some(&GuardDecision::Deny {
            message: REFUS_ARRET.into(),
        }),
        false,
    )
}

fn chaine(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(str::to_string)
}

/// Un `can_use_tool` → la demande que montre la carte.
pub fn demande_permission(request: &Value) -> PermissionRequest {
    let tool = chaine(request.get("tool_name")).unwrap_or_default();
    let input: Entree = request
        .get("input")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let family = family_of(&ToolUse {
        tool: tool.clone(),
        input: input.clone(),
    });
    // Comme le TS : la raison du binaire, sinon sa description ; vide = rien.
    let reason = chaine(request.get("decision_reason"))
        .or_else(|| chaine(request.get("description")))
        .filter(|r| !r.is_empty());
    let plan = (tool == "ExitPlanMode")
        .then(|| chaine(input.get("plan")))
        .flatten();
    PermissionRequest {
        tool,
        family,
        title: chaine(request.get("title")).filter(|t| !t.is_empty()),
        reason,
        plan,
        input,
    }
}

/// La réponse de la carte. ★ « Toujours » reste chez Fragment (`PermanentRule` + garde) : on
/// n'envoie jamais `updatedPermissions` (la suggestion du binaire, `setMode: acceptEdits`, est
/// bien trop large).
pub fn reponse_permission(decision: &PermissionDecision, input: &Entree) -> Value {
    match decision {
        PermissionDecision::Allow { .. } => {
            json!({"behavior": "allow", "updatedInput": Value::Object(input.clone())})
        }
        PermissionDecision::Deny { message } => json!({
            "behavior": "deny",
            "message": message.as_deref().filter(|m| !m.is_empty()).unwrap_or(REFUS_DEFAUT),
        }),
    }
}

static COURRIEL: LazyLock<Motif> =
    LazyLock::new(|| Motif::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+"));

/// ★ Toute adresse de courriel remplacée : la réponse à `initialize` porte le COMPTE de
/// l'utilisateur, et rien de ce que le harness journalise ou montre ne doit la contenir.
pub fn masquer_courriels(texte: &str) -> String {
    COURRIEL.replace_all(texte, "<courriel>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrat::ToolFamily;

    #[test]
    fn initialize_porte_la_garde_et_le_prompt() {
        let v = requete_initialize("fr-1", true, "P");
        assert_eq!(v["request"]["systemPrompt"], json!(["P"]));
        assert_eq!(
            v["request"]["hooks"]["PreToolUse"][0]["hookCallbackIds"],
            json!([ID_GARDE])
        );
        assert!(
            requete_initialize("fr-1", false, "P")["request"]
                .get("hooks")
                .is_none()
        );
    }

    #[test]
    fn le_message_porte_les_pieces_avant_le_texte() {
        assert_eq!(
            message_utilisateur("a", &[])["message"]["content"],
            json!("a")
        );
        let bloc = json!({"type": "image"})
            .as_object()
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            message_utilisateur("a", &[bloc])["message"]["content"],
            json!([{"type": "image"}, {"type": "text", "text": "a"}])
        );
    }

    #[test]
    fn la_garde_demander_ne_force_rien_en_dontask() {
        assert_eq!(reponse_garde(Some(&GuardDecision::Ask), false), json!({}));
        assert_eq!(
            reponse_garde(Some(&GuardDecision::Ask), true)["hookSpecificOutput"]["permissionDecision"],
            "ask"
        );
        assert_eq!(reponse_garde(None, true), json!({}));
        assert_eq!(reponse_garde(Some(&GuardDecision::Allow), true), json!({}));
    }

    #[test]
    fn la_carte_du_plan_porte_son_texte() {
        let d = demande_permission(&json!({
            "subtype": "can_use_tool", "tool_name": "ExitPlanMode",
            "input": {"plan": "# Mon plan"}, "description": "",
        }));
        assert_eq!(d.plan.as_deref(), Some("# Mon plan"));
        assert_eq!(d.reason, None);
        let d = demande_permission(&json!({
            "tool_name": "Bash", "input": {"command": "ls"}, "description": "Lister",
        }));
        assert_eq!(d.family, Some(ToolFamily::Shell));
        assert_eq!(d.reason.as_deref(), Some("Lister"));
    }

    #[test]
    fn un_refus_sans_pourquoi_dit_refuse_par_l_utilisateur() {
        let r = reponse_permission(&PermissionDecision::Deny { message: None }, &Entree::new());
        assert_eq!(r["message"], REFUS_DEFAUT);
    }

    #[test]
    fn les_courriels_sont_masques() {
        let c = format!("{}@{}", "titulaire", "exemple.test");
        assert_eq!(
            masquer_courriels(&format!("compte {c} !")),
            "compte <courriel> !"
        );
    }
}
