// Port de app/src/ui/views/agent/events.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  SDK → contrat de Fragment. Fonctions PURES. Les messages du SDK (et les
//  lignes `stream-json` du CLI `claude`, de même forme) sont des `serde_json::Value`.
//
//  ★ Le texte arrive par les `stream_event` (deltas) ; le message `assistant`
//    complet qui suit le répète. On ne retient donc de lui que les appels
//    d'outils — sinon chaque réponse s'afficherait deux fois.
//  ★ Les messages d'un sous-agent (`parent_tool_use_id` posé) sont ignorés : le
//    fil montre la conversation principale.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::attachment_blocks::attachment_from_block;
use crate::contrat::{
    AgentEvent, BlocOutil, ChatBlock, ChatEntry, Entree, ResultatOutil, Role, TurnUsage,
};

/// Les champs d'entrée qui disent le mieux ce que fait un outil, par ordre de préférence.
const CHAMPS_RESUME: [&str; 7] = [
    "file_path",
    "path",
    "pattern",
    "command",
    "url",
    "query",
    "description",
];

/// L'entrée d'un outil en une ligne : `Read` → le chemin, `Grep` → le motif…
pub fn resumer_entree(input: &Value) -> String {
    let Some(o) = input.as_object() else {
        return String::new();
    };
    for champ in CHAMPS_RESUME {
        if let Some(s) = o.get(champ).and_then(Value::as_str) {
            return s.to_string();
        }
    }
    String::new()
}

/// Le texte d'un `tool_result` : une chaîne, ou une liste de blocs dont on garde le texte.
fn texte_resultat(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocs)) => blocs
            .iter()
            .map(
                |b| match (b.get("type").and_then(Value::as_str), b.get("text")) {
                    (Some("text"), Some(Value::String(t))) => t.clone(),
                    (Some("text"), Some(Value::Null) | None) => String::new(),
                    (Some("text"), Some(autre)) => autre.to_string(),
                    _ => String::new(),
                },
            )
            .collect(),
        _ => String::new(),
    }
}

/// `String(x)` du JS pour un identifiant.
fn en_chaine(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => "undefined".to_string(),
        Some(autre) => autre.to_string(),
    }
}

/// Les blocs de contenu d'un message : une chaîne devient un bloc texte.
fn blocs(message: Option<&Value>) -> Vec<Entree> {
    match message.and_then(|m| m.get("content")) {
        Some(Value::String(s)) => vec![
            json!({ "type": "text", "text": s })
                .as_object()
                .cloned()
                .unwrap_or_default(),
        ],
        Some(Value::Array(a)) => a.iter().filter_map(|b| b.as_object().cloned()).collect(),
        _ => Vec::new(),
    }
}

fn bloc_type(b: &Entree) -> Option<&str> {
    b.get("type").and_then(Value::as_str)
}

fn bloc_outil(b: &Entree, at: Option<i64>) -> BlocOutil {
    let input = b.get("input").and_then(Value::as_object).cloned();
    BlocOutil {
        id: en_chaine(b.get("id")),
        name: en_chaine(b.get("name")),
        summary: resumer_entree(b.get("input").unwrap_or(&Value::Null)),
        input,
        result: None,
        at,
        result_at: None,
    }
}

/// L'heure (ms) d'un message du transcript, si elle y est (`Date.parse`).
fn heure(m: &Value) -> Option<i64> {
    let t = m.get("timestamp")?.as_str()?;
    let d = OffsetDateTime::parse(t, &Rfc3339).ok()?;
    i64::try_from(d.unix_timestamp_nanos() / 1_000_000).ok()
}

/// `parent_tool_use_id` posé (truthy) : le message d'un sous-agent.
fn est_sous_agent(m: &Value) -> bool {
    match m.get("parent_tool_use_id") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn nombre(v: Option<&Value>) -> Option<u64> {
    v.and_then(Value::as_u64).or_else(|| {
        v.and_then(Value::as_f64)
            .filter(|f| *f >= 0.0)
            .map(|f| f as u64)
    })
}

/// Un message du SDK pendant un tour → 0..n événements pour le renderer.
pub fn to_agent_events(msg: &Value) -> Vec<AgentEvent> {
    if est_sous_agent(msg) {
        return Vec::new();
    }
    match msg.get("type").and_then(Value::as_str) {
        Some("stream_event") => {
            let ev = msg.get("event").unwrap_or(&Value::Null);
            let genre = ev.get("type").and_then(Value::as_str);
            let delta = ev.get("delta");
            let genre_delta = delta.and_then(|d| d.get("type")).and_then(Value::as_str);
            if genre == Some("content_block_delta") && genre_delta == Some("text_delta") {
                let text = delta
                    .and_then(|d| d.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                return vec![AgentEvent::TextDelta {
                    text: text.to_string(),
                }];
            }
            // La PENSÉE (résumée) : un bloc qui commence, puis son texte en flux.
            if genre == Some("content_block_start")
                && ev
                    .get("content_block")
                    .and_then(|c| c.get("type"))
                    .and_then(Value::as_str)
                    == Some("thinking")
            {
                return vec![AgentEvent::ThinkingDelta {
                    text: String::new(),
                    start: Some(true),
                }];
            }
            if genre == Some("content_block_delta") && genre_delta == Some("thinking_delta") {
                let text = delta
                    .and_then(|d| d.get("thinking"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                return vec![AgentEvent::ThinkingDelta {
                    text: text.to_string(),
                    start: None,
                }];
            }
            Vec::new()
        }
        Some("assistant") => {
            let message = msg.get("message");
            let outils: Vec<ChatBlock> = blocs(message)
                .iter()
                .filter(|b| bloc_type(b) == Some("tool_use"))
                .map(|b| ChatBlock::Tool(bloc_outil(b, None)))
                .collect();
            let mut out = Vec::new();
            if !outils.is_empty() {
                out.push(AgentEvent::Assistant { blocks: outils });
            }
            // Ce qu'occupe la conversation à CET appel du modèle (l'anneau de contexte).
            if let Some(u) = message.and_then(|m| m.get("usage"))
                && let Some(entree) = nombre(u.get("input_tokens"))
            {
                let context_tokens = entree
                    + nombre(u.get("cache_read_input_tokens")).unwrap_or(0)
                    + nombre(u.get("cache_creation_input_tokens")).unwrap_or(0)
                    + nombre(u.get("output_tokens")).unwrap_or(0);
                out.push(AgentEvent::Usage {
                    usage: TurnUsage {
                        context_tokens: Some(context_tokens),
                        ..TurnUsage::default()
                    },
                });
            }
            out
        }
        Some("user") => blocs(msg.get("message"))
            .iter()
            .filter(|b| bloc_type(b) == Some("tool_result"))
            .map(|b| AgentEvent::ToolResult {
                tool_use_id: en_chaine(b.get("tool_use_id")),
                is_error: b.get("is_error") == Some(&Value::Bool(true)),
                text: texte_resultat(b.get("content")),
            })
            .collect(),
        Some("result") => {
            // L'usage du tour (une requête = un tour chez nous) : tokens de la boucle
            // principale, coût estimé, fenêtre du modèle (la plus grande des modèles appelés).
            let u = msg.get("usage");
            let champ = |k: &str| nombre(u.and_then(|u| u.get(k)));
            let fenetre = msg
                .get("modelUsage")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|m| m.values())
                .filter_map(|m| nombre(m.get("contextWindow")))
                .filter(|n| *n > 0)
                .max();
            let errors = if msg.get("subtype").and_then(Value::as_str) == Some("success") {
                Vec::new()
            } else {
                msg.get("errors")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|e| e.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            vec![AgentEvent::Done {
                session_id: en_chaine(msg.get("session_id")),
                is_error: msg.get("is_error") == Some(&Value::Bool(true)),
                errors,
                usage: Some(TurnUsage {
                    input_tokens: champ("input_tokens"),
                    output_tokens: champ("output_tokens"),
                    cached_input_tokens: champ("cache_read_input_tokens"),
                    context_tokens: None,
                    context_window: fenetre,
                    cost_usd: msg.get("total_cost_usd").and_then(Value::as_f64),
                }),
            }]
        }
        _ => Vec::new(),
    }
}

/// Où se trouve un appel d'outil déjà placé dans les entrées : (entrée, bloc).
type Emplacements = HashMap<String, (usize, usize)>;

/// Le transcript d'une session (`getSessionMessages`) → les bulles du fil.
/// Les messages assistant consécutifs se fondent en une bulle ; un message
/// utilisateur qui ne porte que des résultats d'outils n'est pas une bulle : ses
/// résultats s'accrochent aux appels correspondants.
pub fn to_chat_entries(messages: &[Value]) -> Vec<ChatEntry> {
    let mut entries: Vec<ChatEntry> = Vec::new();
    let mut outils: Emplacements = HashMap::new();

    for m in messages {
        if est_sous_agent(m) {
            continue;
        }
        let uuid = m.get("uuid").and_then(Value::as_str).map(str::to_string);
        let derniere_assistant = entries.last().is_some_and(|e| e.role == Role::Assistant);
        let at = heure(m);
        match m.get("type").and_then(Value::as_str) {
            Some("assistant") => {
                let mut out: Vec<ChatBlock> = Vec::new();
                for b in blocs(m.get("message")) {
                    match bloc_type(&b) {
                        Some("text") => {
                            if let Some(t) = b
                                .get("text")
                                .and_then(Value::as_str)
                                .filter(|t| !t.is_empty())
                            {
                                out.push(ChatBlock::Text {
                                    text: t.to_string(),
                                    at,
                                });
                            }
                        }
                        Some("thinking") => {
                            if let Some(t) = b
                                .get("thinking")
                                .and_then(Value::as_str)
                                .filter(|t| !t.trim().is_empty())
                            {
                                out.push(ChatBlock::Thinking {
                                    text: t.to_string(),
                                    at,
                                });
                            }
                        }
                        Some("tool_use") => out.push(ChatBlock::Tool(bloc_outil(&b, at))),
                        _ => {}
                    }
                }
                if out.is_empty() {
                    // Rien d'affichable (une réflexion vide) : l'entrée appartient au tour en cours.
                    if derniere_assistant && let Some(e) = entries.last_mut() {
                        e.last = uuid;
                    }
                    continue;
                }
                let (idx, base) = if derniere_assistant {
                    let i = entries.len() - 1;
                    (i, entries[i].blocks.len())
                } else {
                    (entries.len(), 0)
                };
                for (j, b) in out.iter().enumerate() {
                    if let ChatBlock::Tool(t) = b {
                        outils.insert(t.id.clone(), (idx, base + j));
                    }
                }
                if derniere_assistant {
                    if let Some(e) = entries.last_mut() {
                        e.blocks.extend(out);
                        e.last = uuid;
                    }
                } else {
                    entries.push(ChatEntry {
                        role: Role::Assistant,
                        blocks: out,
                        last: uuid,
                        at,
                    });
                }
            }
            Some("user") => {
                let mut textes: Vec<ChatBlock> = Vec::new();
                let mut pieces: Vec<ChatBlock> = Vec::new();
                for b in blocs(m.get("message")) {
                    if bloc_type(&b) == Some("tool_result") {
                        if let Some(&(e, i)) = outils.get(&en_chaine(b.get("tool_use_id")))
                            && let Some(ChatBlock::Tool(outil)) =
                                entries.get_mut(e).and_then(|e| e.blocks.get_mut(i))
                        {
                            outil.result = Some(ResultatOutil {
                                is_error: b.get("is_error") == Some(&Value::Bool(true)),
                                text: texte_resultat(b.get("content")),
                            });
                            if at.is_some() {
                                outil.result_at = at;
                            }
                        }
                        continue;
                    }
                    // Une pièce jointe (image, document, fichier sur le disque) — gardée APRÈS le texte.
                    if let Some(piece) = attachment_from_block(&b) {
                        pieces.push(piece);
                    } else if bloc_type(&b) == Some("text")
                        && let Some(t) = b
                            .get("text")
                            .and_then(Value::as_str)
                            .filter(|t| !t.is_empty())
                    {
                        textes.push(ChatBlock::texte(t));
                    }
                }
                if !textes.is_empty() || !pieces.is_empty() {
                    textes.extend(pieces);
                    entries.push(ChatEntry {
                        role: Role::User,
                        blocks: textes,
                        last: uuid,
                        at,
                    });
                } else if derniere_assistant {
                    // Des résultats d'outils seuls : ils FONT PARTIE du tour assistant en cours.
                    if let Some(e) = entries.last_mut() {
                        e.last = uuid;
                    }
                }
            }
            _ => {}
        }
    }
    entries
}

/// Le point de branche : l'uuid où couper et le nombre de bulles héritées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointDeBranche {
    pub at: String,
    pub inherited: usize,
}

/// Où brancher pour garder la bulle de rang `rang` : à la FIN DE SON TOUR (une
/// question → la fin de la réponse qui la suit ; le SDK refuse un point qui
/// laisserait un appel d'outil sans résultat). Rend l'uuid et le nombre de bulles
/// héritées, ou `None` (transcript vide). Pur.
pub fn fork_point(entries: &[ChatEntry], rang: usize) -> Option<PointDeBranche> {
    if entries.is_empty() {
        return None;
    }
    let mut i = rang.min(entries.len() - 1);
    if entries[i].role == Role::User {
        if entries
            .get(i + 1)
            .is_some_and(|e| e.role == Role::Assistant)
        {
            i += 1;
        } else {
            i = i.saturating_sub(1); // une question restée sans réponse : le tour d'avant
        }
    }
    entries[i].last.clone().map(|at| PointDeBranche {
        at,
        inherited: i + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_792_497_600_000; // 2026-10-20T12:00:00.000Z

    fn sdk(v: Value) -> Value {
        v
    }

    fn outil(id: &str, name: &str, summary: &str, input: Value) -> ChatBlock {
        ChatBlock::Tool(BlocOutil {
            id: id.into(),
            name: name.into(),
            summary: summary.into(),
            input: input.as_object().cloned(),
            result: None,
            at: None,
            result_at: None,
        })
    }

    // ── toAgentEvents ──

    #[test]
    fn init_rien_lid_de_session_est_celui_que_fragment_a_choisi() {
        assert_eq!(
            to_agent_events(&sdk(
                json!({ "type": "system", "subtype": "init", "session_id": "s1", "model": "m" })
            )),
            vec![]
        );
    }

    #[test]
    fn les_deltas_de_texte_passent_le_reste_du_flux_non() {
        let delta = sdk(
            json!({ "type": "stream_event", "parent_tool_use_id": null, "event": { "type": "content_block_delta", "delta": { "type": "text_delta", "text": "Bon" } } }),
        );
        assert_eq!(
            to_agent_events(&delta),
            vec![AgentEvent::TextDelta { text: "Bon".into() }]
        );
        let start = sdk(
            json!({ "type": "stream_event", "parent_tool_use_id": null, "event": { "type": "message_start" } }),
        );
        assert_eq!(to_agent_events(&start), vec![]);
    }

    #[test]
    fn assistant_seuls_les_appels_doutils_le_texte_est_deja_venu_en_deltas() {
        let msg = sdk(json!({
            "type": "assistant", "parent_tool_use_id": null,
            "message": { "content": [{ "type": "text", "text": "Je regarde" }, { "type": "tool_use", "id": "t1", "name": "Read", "input": { "file_path": "a.md" } }] },
        }));
        assert_eq!(
            to_agent_events(&msg),
            vec![AgentEvent::Assistant {
                blocks: vec![outil("t1", "Read", "a.md", json!({ "file_path": "a.md" }))]
            }]
        );
    }

    #[test]
    fn resultats_doutils_sous_agents_ignores() {
        let res = sdk(
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": [{ "type": "text", "text": "ok" }] }] } }),
        );
        assert_eq!(
            to_agent_events(&res),
            vec![AgentEvent::ToolResult {
                tool_use_id: "t1".into(),
                is_error: false,
                text: "ok".into()
            }]
        );
        assert_eq!(
            to_agent_events(&sdk(
                json!({ "type": "assistant", "parent_tool_use_id": "x", "message": { "content": [] } })
            )),
            vec![]
        );
    }

    #[test]
    fn result_done_avec_lusage_du_tour_tokens_cout_fenetre_la_plus_grande_des_modeles_appeles() {
        assert_eq!(
            to_agent_events(&sdk(
                json!({ "type": "result", "subtype": "error_max_turns", "session_id": "s", "is_error": true, "errors": ["trop"], "total_cost_usd": 0.1 })
            )),
            vec![AgentEvent::Done {
                session_id: "s".into(),
                is_error: true,
                errors: vec!["trop".into()],
                usage: Some(TurnUsage {
                    cost_usd: Some(0.1),
                    ..TurnUsage::default()
                }),
            }]
        );
        let ok = sdk(json!({
            "type": "result", "subtype": "success", "session_id": "s", "is_error": false, "total_cost_usd": 0.02,
            "usage": { "input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 100 },
            "modelUsage": { "a": { "contextWindow": 200_000 }, "b": { "contextWindow": 1_000_000 } },
        }));
        assert_eq!(
            to_agent_events(&ok),
            vec![AgentEvent::Done {
                session_id: "s".into(),
                is_error: false,
                errors: vec![],
                usage: Some(TurnUsage {
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    cached_input_tokens: Some(100),
                    context_window: Some(1_000_000),
                    cost_usd: Some(0.02),
                    ..TurnUsage::default()
                }),
            }]
        );
    }

    #[test]
    fn assistant_avec_usage_un_releve_du_contexte_occupe_a_cet_appel() {
        let msg = sdk(json!({
            "type": "assistant", "parent_tool_use_id": null,
            "message": { "content": [{ "type": "text", "text": "x" }], "usage": { "input_tokens": 3, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 20, "output_tokens": 7 } },
        }));
        assert_eq!(
            to_agent_events(&msg),
            vec![AgentEvent::Usage {
                usage: TurnUsage {
                    context_tokens: Some(1030),
                    ..TurnUsage::default()
                }
            }]
        );
    }

    // ── toChatEntries ──

    #[test]
    fn fond_les_messages_assistant_accroche_les_resultats_garde_les_messages_utilisateur() {
        let entries = to_chat_entries(&[
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "role": "user", "content": "Liste les notes" } }),
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "content": [{ "type": "tool_use", "id": "t1", "name": "Glob", "input": { "pattern": "**/*.md" } }] } }),
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": "a.md" }] } }),
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "content": [{ "type": "text", "text": "Une note : a.md" }] } }),
        ]);
        let mut glob = outil("t1", "Glob", "**/*.md", json!({ "pattern": "**/*.md" }));
        if let ChatBlock::Tool(t) = &mut glob {
            t.result = Some(ResultatOutil {
                is_error: false,
                text: "a.md".into(),
            });
        }
        assert_eq!(
            entries,
            vec![
                ChatEntry {
                    role: Role::User,
                    blocks: vec![ChatBlock::texte("Liste les notes")],
                    last: None,
                    at: None
                },
                ChatEntry {
                    role: Role::Assistant,
                    blocks: vec![glob, ChatBlock::texte("Une note : a.md")],
                    last: None,
                    at: None
                },
            ]
        );
    }

    // ── la PENSÉE (résumée) ──

    #[test]
    fn flux_un_bloc_qui_commence_puis_son_texte() {
        assert_eq!(
            to_agent_events(&sdk(
                json!({ "type": "stream_event", "parent_tool_use_id": null, "event": { "type": "content_block_start", "index": 0, "content_block": { "type": "thinking", "thinking": "" } } })
            )),
            vec![AgentEvent::ThinkingDelta {
                text: String::new(),
                start: Some(true)
            }]
        );
        assert_eq!(
            to_agent_events(&sdk(
                json!({ "type": "stream_event", "parent_tool_use_id": null, "event": { "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": "Hmm" } } })
            )),
            vec![AgentEvent::ThinkingDelta {
                text: "Hmm".into(),
                start: None
            }]
        );
    }

    #[test]
    fn transcript_bloc_thinking_garde_avec_son_heure_outil_avec_son_entree_et_lheure_de_son_resultat()
     {
        let entries = to_chat_entries(&[
            json!({ "type": "user", "parent_tool_use_id": null, "timestamp": "2026-10-20T12:00:00.000Z", "message": { "role": "user", "content": "Q" } }),
            json!({ "type": "assistant", "parent_tool_use_id": null, "timestamp": "2026-10-20T12:00:02.000Z", "message": { "content": [{ "type": "thinking", "thinking": "Je réfléchis." }] } }),
            json!({ "type": "assistant", "parent_tool_use_id": null, "timestamp": "2026-10-20T12:00:03.000Z", "message": { "content": [{ "type": "tool_use", "id": "t", "name": "Read", "input": { "file_path": "a.md" } }] } }),
            json!({ "type": "user", "parent_tool_use_id": null, "timestamp": "2026-10-20T12:00:04.000Z", "message": { "content": [{ "type": "tool_result", "tool_use_id": "t", "content": "x" }] } }),
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "content": [{ "type": "thinking", "thinking": "" }, { "type": "text", "text": "R" }] } }),
        ]);
        assert_eq!(entries[0].at, Some(T0));
        assert_eq!(
            entries[1].blocks,
            vec![
                ChatBlock::Thinking {
                    text: "Je réfléchis.".into(),
                    at: Some(T0 + 2000)
                },
                ChatBlock::Tool(BlocOutil {
                    id: "t".into(),
                    name: "Read".into(),
                    summary: "a.md".into(),
                    input: json!({ "file_path": "a.md" }).as_object().cloned(),
                    result: Some(ResultatOutil {
                        is_error: false,
                        text: "x".into()
                    }),
                    at: Some(T0 + 3000),
                    result_at: Some(T0 + 4000),
                }),
                ChatBlock::texte("R"),
            ]
        );
    }

    // ── toChatEntries — pièces jointes ──

    #[test]
    fn un_message_utilisateur_image_document_texte_le_texte_puis_les_pieces_une_image_seule_reste_une_bulle()
     {
        let entries = to_chat_entries(&[
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "role": "user", "content": [
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "QUJD" } },
                { "type": "document", "title": "n.md", "source": { "type": "text", "media_type": "text/plain", "data": "x" } },
                { "type": "text", "text": "Que vois-tu ?" },
            ] } }),
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "content": [{ "type": "text", "text": "Un carré." }] } }),
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "role": "user", "content": [
                { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": "REVG" } },
            ] } }),
        ]);
        assert_eq!(
            entries[0],
            ChatEntry {
                role: Role::User,
                blocks: vec![
                    ChatBlock::texte("Que vois-tu ?"),
                    ChatBlock::Attachment {
                        name: "image".into(),
                        mime: "image/png".into(),
                        url: Some("data:image/png;base64,QUJD".into()),
                        path: None
                    },
                    ChatBlock::Attachment {
                        name: "n.md".into(),
                        mime: "text/plain".into(),
                        url: None,
                        path: None
                    },
                ],
                last: None,
                at: None,
            }
        );
        assert_eq!(
            entries[2],
            ChatEntry {
                role: Role::User,
                blocks: vec![ChatBlock::Attachment {
                    name: "image".into(),
                    mime: "image/jpeg".into(),
                    url: Some("data:image/jpeg;base64,REVG".into()),
                    path: None
                }],
                last: None,
                at: None,
            }
        );
    }

    // ── le point de BRANCHE (fork du SDK) ──

    fn entries_de_branche() -> Vec<ChatEntry> {
        // Deux tours : le 1er avec un appel d'outil (son résultat est une entrée « user »).
        to_chat_entries(&[
            json!({ "type": "user", "uuid": "u1", "parent_tool_use_id": null, "message": { "role": "user", "content": "Q1" } }),
            json!({ "type": "assistant", "uuid": "a1", "parent_tool_use_id": null, "message": { "content": [{ "type": "tool_use", "id": "t1", "name": "Read", "input": { "file_path": "n.md" } }] } }),
            json!({ "type": "user", "uuid": "r1", "parent_tool_use_id": null, "message": { "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": "x" }] } }),
            json!({ "type": "assistant", "uuid": "a2", "parent_tool_use_id": null, "message": { "content": [{ "type": "text", "text": "R1" }] } }),
            json!({ "type": "assistant", "uuid": "a3", "parent_tool_use_id": null, "message": { "content": [{ "type": "thinking", "thinking": "…" }] } }),
            json!({ "type": "user", "uuid": "u2", "parent_tool_use_id": null, "message": { "role": "user", "content": "Q2" } }),
            json!({ "type": "assistant", "uuid": "a4", "parent_tool_use_id": null, "message": { "content": [{ "type": "text", "text": "R2" }] } }),
            json!({ "type": "user", "uuid": "u3", "parent_tool_use_id": null, "message": { "role": "user", "content": "Q3" } }),
        ])
    }

    #[test]
    fn chaque_bulle_retient_la_derniere_entree_de_son_tour_outils_resultats_reflexion_compris() {
        let entries = entries_de_branche();
        let vu: Vec<String> = entries
            .iter()
            .map(|e| format!("{:?}:{}", e.role, e.last.clone().unwrap_or_default()))
            .collect();
        assert_eq!(
            vu,
            vec![
                "User:u1",
                "Assistant:a3",
                "User:u2",
                "Assistant:a4",
                "User:u3"
            ]
        );
    }

    #[test]
    fn une_reponse_la_fin_de_son_tour_une_question_la_fin_de_la_reponse_qui_suit_une_question_sans_reponse_le_tour_davant()
     {
        let entries = entries_de_branche();
        let pt = |at: &str, inherited| {
            Some(PointDeBranche {
                at: at.into(),
                inherited,
            })
        };
        assert_eq!(fork_point(&entries, 1), pt("a3", 2));
        assert_eq!(fork_point(&entries, 0), pt("a3", 2));
        assert_eq!(fork_point(&entries, 2), pt("a4", 4));
        assert_eq!(fork_point(&entries, 4), pt("a4", 4));
        assert_eq!(fork_point(&entries, 99), pt("a4", 4));
        assert_eq!(fork_point(&[], 0), None);
    }

    // ── resumerEntree ──

    #[test]
    fn prend_le_champ_le_plus_parlant() {
        assert_eq!(resumer_entree(&json!({ "pattern": "x", "path": "d" })), "d");
        assert_eq!(resumer_entree(&Value::Null), "");
    }
}
