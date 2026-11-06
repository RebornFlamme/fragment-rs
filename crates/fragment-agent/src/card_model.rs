// Port de app/src/ui/views/agent/side-chat/cardModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le vocabulaire d'« Ask » — pur.
//
//  Poser une question sur un passage ne crée aucun objet propre à ce plugin : on
//  ouvre une conversation de l'agent ancrée à ce passage (vue `chat` dans un widget
//  de doc-widget). Ce module ne garde que le titre, et l'ancien état `reader:card`
//  pour migrer les cartes antérieures à la vue chat.
// ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

use crate::utf16::{len_utf16, tete_utf16};

/// L'ancien type des cartes (avant la vue chat) — migré au chargement.
pub const LEGACY_CARD_TYPE: &str = "reader:card";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyTurn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    pub answer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyDepth {
    Low,
    High,
}

/// L'état d'une ancienne carte `reader:card` (lu par la migration seule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCardState {
    pub action: String,
    pub quote: String,
    pub len: i64,
    pub para: i64,
    pub session_id: String,
    pub turns: Vec<LegacyTurn>,
    pub depth: LegacyDepth,
    pub created_at: i64,
}

/// Le titre d'une conversation sur un passage : la question, sinon `Side Chat « passage »`.
pub fn titre_side_chat(quote: &str, question: Option<&str>) -> String {
    if let Some(q) = question.filter(|q| !q.is_empty()) {
        return if len_utf16(q) > 60 {
            format!("{}…", tete_utf16(q, 59))
        } else {
            q.to_string()
        };
    }
    let q = quote.split_whitespace().collect::<Vec<_>>().join(" ");
    if q.is_empty() {
        return "Side Chat".to_string();
    }
    format!(
        "Side Chat « {} »",
        if len_utf16(&q) > 40 {
            format!("{}…", tete_utf16(&q, 39))
        } else {
            q
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titre_dune_conversation_side_chat_passage_ou_la_question() {
        assert_eq!(titre_side_chat("NatSec", None), "Side Chat « NatSec »");
        assert_eq!(
            titre_side_chat("Kepler", Some("whats kepler?")),
            "whats kepler?"
        );
        assert_eq!(titre_side_chat("", None), "Side Chat");
        assert_eq!(
            titre_side_chat(&"x".repeat(80), None),
            format!("Side Chat « {}… »", "x".repeat(39))
        );
    }
}
