// Port de app/src/ui/views/agent/oneShotOptions.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les options SDK d'une question SANS conversation (le petit agent des actions
//  rapides) — PUR. Une seule fonction pour l'appel ET la RÉSERVE
//  (`AgentHost.warmOneShot`) : ce qu'une réserve fige est identique, par
//  construction, à ce que l'appel demande ; `one_shot_key` le résume (sans `cwd`, que
//  la réserve reçoit au `claim`).
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;

use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct OneShotSpec {
    pub system: String,
    pub schema: Value,
    pub model: Option<String>,
}

/// Les options FIGÉES (tout sauf `cwd`).
pub fn one_shot_options(spec: &OneShotSpec) -> Value {
    let mut o = json!({
        "settingSources": [],
        "systemPrompt": spec.system,
        "tools": [],
        "allowedTools": [],
        "permissionMode": "dontAsk",
        "persistSession": false,
        "maxTurns": 3,
        "strictMcpConfig": true,
        "mcpServers": {},
        // ★ Pas de réflexion : une réponse courte et structurée, attendue au plus vite
        //   (sinon le défaut du modèle — adaptative — pensait avant de répondre).
        "thinking": { "type": "disabled" },
        // La sortie en FLUX (le JSON au fil de l'écriture) : les boutons arrivent un à un.
        "includePartialMessages": true,
        "outputFormat": { "type": "json_schema", "schema": spec.schema },
    });
    // `model: spec.model || undefined` : une chaîne vide n'est pas un modèle.
    if let (Some(m), Some(obj)) = (
        spec.model.as_deref().filter(|m| !m.is_empty()),
        o.as_object_mut(),
    ) {
        obj.insert("model".into(), Value::String(m.to_string()));
    }
    o
}

/// La clé d'une réserve : ses options figées et son environnement (la clé API change le processus).
/// ★ Hash et longueur en unités UTF-16, comme en TS. (Les clés JSON sont triées par `serde_json` :
///   la clé n'est comparée qu'à elle-même, dans le processus.)
pub fn one_shot_key(options: &Value, env: &BTreeMap<String, String>) -> String {
    let paires: Vec<Value> = env.iter().map(|(k, v)| json!([k, v])).collect();
    let s = json!([options, paires]).to_string();
    let mut h: i32 = 0;
    let mut longueur = 0_usize;
    for u in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(i32::from(u));
        longueur += 1;
    }
    format!("{longueur}:{h}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> OneShotSpec {
        OneShotSpec {
            system: "consignes".into(),
            schema: json!({ "type": "object" }),
            model: Some("claude-sonnet-5-5".into()),
        }
    }

    #[test]
    fn sans_reflexion_en_flux_sans_outil_jamais_de_cwd_il_va_au_claim() {
        let o = one_shot_options(&spec());
        assert_eq!(o["thinking"], json!({ "type": "disabled" }));
        assert_eq!(o["includePartialMessages"], true);
        assert_eq!(o["tools"], json!([]));
        assert_eq!(o["persistSession"], false);
        assert_eq!(o["model"], "claude-sonnet-5-5");
        assert!(o.get("cwd").is_none());
    }

    #[test]
    fn meme_specification_meme_cle_consignes_schema_modele_ou_cle_api_la_changent() {
        let vide = BTreeMap::new();
        let k = one_shot_key(&one_shot_options(&spec()), &vide);
        assert_eq!(one_shot_key(&one_shot_options(&spec().clone()), &vide), k);
        assert_ne!(
            one_shot_key(
                &one_shot_options(&OneShotSpec {
                    system: "autres".into(),
                    ..spec()
                }),
                &vide
            ),
            k
        );
        assert_ne!(
            one_shot_key(
                &one_shot_options(&OneShotSpec {
                    schema: json!({ "type": "array" }),
                    ..spec()
                }),
                &vide
            ),
            k
        );
        assert_ne!(
            one_shot_key(
                &one_shot_options(&OneShotSpec {
                    model: Some("claude-haiku-5-5".into()),
                    ..spec()
                }),
                &vide
            ),
            k
        );
        let env = BTreeMap::from([("ANTHROPIC_API_KEY".to_string(), "x".to_string())]);
        assert_ne!(one_shot_key(&one_shot_options(&spec()), &env), k);
    }
}
