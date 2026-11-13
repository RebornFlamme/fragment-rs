// Port de app/src/ui/views/agent/AgentChatProvider.ts (`partTrace`) et de la forme JSON des parts
// `agent-trace` et `agent-permission` (agent/trace/traceModel.ts, agent/policy/permissionCard.ts)
//
// Un part du chat porte ses données en JSON (`ChatPart::data`). `Trace` n'est pas sérialisable
// par serde (modèle pur de trace_model.rs) : cette conversion écrit et relit la forme que la vue
// de la trace attend (clés camelCase, `kind` en minuscules), dans les deux sens.

use serde_json::{Map, Value, json};

use fragment_chat::api::ChatPart;

use crate::contrat::ResultatOutil;
use crate::fournisseur::agent_chat::AGENT_PERMISSION_PART;
use crate::permission_model::PermissionCardData;
use crate::trace_model::{AGENT_TRACE_PART, StepKind, Trace, TraceStatus, TraceStep};

/// L'id du part de la TRACE d'un tour, et celui de la RÉPONSE qui la suit.
pub const ID_TRACE: &str = "trace";
pub const ID_REPONSE: &str = "answer";

fn kind_str(k: StepKind) -> &'static str {
    match k {
        StepKind::Thinking => "thinking",
        StepKind::Note => "note",
        StepKind::Tool => "tool",
    }
}

fn status_str(s: TraceStatus) -> &'static str {
    match s {
        TraceStatus::Running => "running",
        TraceStatus::Done => "done",
        TraceStatus::Error => "error",
    }
}

fn etape_en_json(s: &TraceStep) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(s.id));
    m.insert("kind".into(), json!(kind_str(s.kind)));
    if let Some(t) = &s.text {
        m.insert("text".into(), json!(t));
    }
    if let Some(n) = &s.name {
        m.insert("name".into(), json!(n));
    }
    if let Some(i) = &s.input {
        m.insert("input".into(), Value::Object(i.clone()));
    }
    if let Some(r) = &s.result {
        m.insert(
            "result".into(),
            json!({ "isError": r.is_error, "text": r.text }),
        );
    }
    if let Some(t) = s.started_at {
        m.insert("startedAt".into(), json!(t));
    }
    if let Some(t) = s.ended_at {
        m.insert("endedAt".into(), json!(t));
    }
    Value::Object(m)
}

/// Une trace → ses données de part (`data` de `agent-trace`).
pub fn trace_en_json(t: &Trace) -> Value {
    let mut m = Map::new();
    m.insert("status".into(), json!(status_str(t.status)));
    if let Some(v) = t.started_at {
        m.insert("startedAt".into(), json!(v));
    }
    if let Some(v) = t.ended_at {
        m.insert("endedAt".into(), json!(v));
    }
    m.insert(
        "steps".into(),
        Value::Array(t.steps.iter().map(etape_en_json).collect()),
    );
    if let Some(w) = t.writing {
        m.insert("writing".into(), json!(w));
    }
    Value::Object(m)
}

fn etape_depuis_json(v: &Value) -> Option<TraceStep> {
    let o = v.as_object()?;
    let kind = match o.get("kind")?.as_str()? {
        "thinking" => StepKind::Thinking,
        "note" => StepKind::Note,
        "tool" => StepKind::Tool,
        _ => return None,
    };
    let chaine = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
    Some(TraceStep {
        id: chaine("id")?,
        kind,
        text: chaine("text"),
        name: chaine("name"),
        input: o.get("input").and_then(Value::as_object).cloned(),
        result: o
            .get("result")
            .and_then(Value::as_object)
            .map(|r| ResultatOutil {
                is_error: r.get("isError").and_then(Value::as_bool).unwrap_or(false),
                text: r
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            }),
        started_at: o.get("startedAt").and_then(Value::as_i64),
        ended_at: o.get("endedAt").and_then(Value::as_i64),
    })
}

/// Les données d'un part `agent-trace` → une trace (la vue de la trace relit ce que le fournisseur écrit).
pub fn trace_depuis_json(v: &Value) -> Option<Trace> {
    let o = v.as_object()?;
    let status = match o.get("status")?.as_str()? {
        "running" => TraceStatus::Running,
        "done" => TraceStatus::Done,
        "error" => TraceStatus::Error,
        _ => return None,
    };
    Some(Trace {
        status,
        started_at: o.get("startedAt").and_then(Value::as_i64),
        ended_at: o.get("endedAt").and_then(Value::as_i64),
        steps: o
            .get("steps")?
            .as_array()?
            .iter()
            .filter_map(etape_depuis_json)
            .collect(),
        writing: o.get("writing").and_then(Value::as_bool),
    })
}

/// Le part de la trace d'un tour (id fixe : une trace ré-émise REMPLACE la précédente).
pub fn part_trace(t: &Trace) -> ChatPart {
    ChatPart {
        kind: AGENT_TRACE_PART.into(),
        id: Some(ID_TRACE.into()),
        text: None,
        data: Some(trace_en_json(t)),
    }
}

/// Le part de la réponse (texte), qui suit la trace.
pub fn part_reponse(texte: &str) -> ChatPart {
    ChatPart {
        kind: "text".into(),
        id: Some(ID_REPONSE.into()),
        text: Some(texte.to_string()),
        data: None,
    }
}

/// Le part d'une carte d'autorisation (id = celui de la demande : l'état change, la carte se replie).
pub fn part_carte(d: &PermissionCardData) -> ChatPart {
    ChatPart {
        kind: AGENT_PERMISSION_PART.into(),
        id: Some(d.id.clone()),
        text: None,
        data: serde_json::to_value(d).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_model::{OutilEntrant, TraceBuilder};

    #[test]
    fn la_trace_fait_laller_retour_en_json() {
        let mut b = TraceBuilder::new(1000);
        b.thinking("Je cherche.", 1100, true);
        b.tool(
            OutilEntrant {
                id: "g1".into(),
                name: "Grep".into(),
                input: json!({ "pattern": "x" }).as_object().cloned(),
            },
            1200,
        );
        b.result(
            "g1",
            ResultatOutil {
                is_error: false,
                text: "a.md:1".into(),
            },
            1300,
        );
        b.finish(1400, false);
        let t = b.snapshot();
        let v = trace_en_json(&t);
        assert_eq!(v["status"], "done");
        assert_eq!(v["steps"][1]["result"]["isError"], false);
        assert_eq!(trace_depuis_json(&v), Some(t));
    }

    #[test]
    fn le_part_de_la_trace_porte_son_id_fixe() {
        let p = part_trace(&TraceBuilder::new(0).snapshot());
        assert_eq!(
            (p.kind.as_str(), p.id.as_deref()),
            ("agent-trace", Some("trace"))
        );
        assert_eq!(
            trace_depuis_json(&p.data.unwrap_or_default()).map(|t| t.steps.len()),
            Some(0)
        );
    }
}
