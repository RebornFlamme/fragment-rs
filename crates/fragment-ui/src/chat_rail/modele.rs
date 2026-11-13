// Port de app/src/ui/views/chat/ChatView.ts (`majDefilement` : tours du rail, ligne « Nouveau »,
// `allerAuMessage`) et de app/src/ui/views/chat/ChatPlugin.ts (action « Copier le lien du message »)
//! Les décisions PURES du rail, des non-lus et des liens de message : aucune vue, aucun GPUI.
//!
//! - [`tours_du_rail`] : un tour par question (texte, début de réponse, premier fichier) ;
//! - [`premier_non_lu`] : le message avant lequel se pose la ligne « Nouveau » ;
//! - [`lien_du_message`] / [`rang_du_lien`] : `[[chat:<fournisseur>/<conversation>#m12]]` et son
//!   chemin retour. Le numéro du lien compte à partir de 1, le rang à partir de 0.

use fragment_chat::api::ChatPart;
use fragment_chat::chat_key::chat_key;
use fragment_chat::chat_session::LiveMessage;
use fragment_chat::rail::RailTurn;

/// Longueur gardée de la question d'un aperçu (caractères).
pub const QUESTION_MAX: usize = 240;
/// Longueur gardée de la réponse d'un aperçu (caractères).
pub const REPONSE_MAX: usize = 320;

/// Le texte d'un message : ses parts `text`, séparées par une espace, espaces repliées.
fn texte_du_message(m: Option<&LiveMessage>) -> String {
    let brut = m
        .map(|m| {
            m.parts
                .iter()
                .filter(|p| p.kind == "text")
                .map(|p| p.text.as_deref().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    brut.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Garde les `n` premiers caractères.
fn couper(s: String, n: usize) -> String {
    if s.chars().count() <= n {
        s
    } else {
        s.chars().take(n).collect()
    }
}

/// Le nom du premier fichier joint d'un part `attachment`.
fn nom_de_piece(p: &ChatPart) -> Option<String> {
    if p.kind != "attachment" {
        return None;
    }
    p.data.as_ref()?.get("name")?.as_str().map(str::to_owned)
}

/// Les tours du rail : un par message de `utilisateur` (la question) ; la réponse est le message
/// qui SUIT s'il est d'un autre auteur ; le fichier est la première pièce jointe de la question,
/// à défaut de la réponse.
#[must_use]
pub fn tours_du_rail(messages: &[LiveMessage], utilisateur: &str) -> Vec<RailTurn> {
    let mut tours = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        if m.author != utilisateur {
            continue;
        }
        let reponse = messages.get(i + 1).filter(|r| r.author != utilisateur);
        let file = m
            .parts
            .iter()
            .chain(reponse.iter().flat_map(|r| r.parts.iter()))
            .find_map(nom_de_piece);
        tours.push(RailTurn {
            id: m.id.clone(),
            question: couper(texte_du_message(Some(m)), QUESTION_MAX),
            answer: couper(texte_du_message(reponse), REPONSE_MAX),
            file,
        });
    }
    tours
}

/// Le message avant lequel se pose la ligne « Nouveau » : le premier, après le dernier lu
/// (`lus_jusqua`, exclu), qui n'est pas de `utilisateur`.
#[must_use]
pub fn premier_non_lu<'a>(
    messages: &'a [LiveMessage],
    lus_jusqua: usize,
    utilisateur: &str,
) -> Option<&'a str> {
    messages
        .iter()
        .skip(lus_jusqua)
        .find(|m| m.author != utilisateur)
        .map(|m| m.id.as_str())
}

/// Le lien d'un message : `[[chat:<fournisseur>/<conversation>#m<n>]]`, `n` = rang + 1.
#[must_use]
pub fn lien_du_message(fournisseur: &str, conversation: &str, rang: usize) -> String {
    format!("[[{}#m{}]]", chat_key(fournisseur, conversation), rang + 1)
}

/// Le rang (à partir de 0) du message visé par un sous-chemin `#m12`. `None` : pas un lien de
/// message (ni `#m0`, qui ne vise rien).
#[must_use]
pub fn rang_du_lien(sous_chemin: &str) -> Option<usize> {
    let n = sous_chemin.strip_prefix("#m")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse::<usize>().ok()?.checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_chat::api::ChatMessage;
    use serde_json::json;

    fn texte(t: &str) -> ChatPart {
        ChatPart {
            kind: "text".into(),
            id: None,
            text: Some(t.into()),
            data: None,
        }
    }

    fn piece(nom: &str) -> ChatPart {
        ChatPart {
            kind: "attachment".into(),
            id: None,
            text: None,
            data: Some(json!({ "name": nom })),
        }
    }

    fn msg(id: &str, auteur: &str, parts: Vec<ChatPart>) -> LiveMessage {
        LiveMessage {
            message: ChatMessage {
                id: id.into(),
                author: auteur.into(),
                parts,
                error: None,
                at: None,
            },
            rev: 0,
            streaming: false,
        }
    }

    fn fil() -> Vec<LiveMessage> {
        vec![
            msg("u1", "moi", vec![texte("Question   1\n suite")]),
            msg(
                "a1",
                "agent",
                vec![texte("Réponse 1."), piece("schema.png")],
            ),
            msg("u2", "moi", vec![texte("Question 2"), piece("a.pdf")]),
            msg("u3", "moi", vec![texte("Question 3")]),
            msg("a3", "agent", vec![texte("Réponse 3")]),
        ]
    }

    #[test]
    fn un_tour_par_question_la_reponse_est_le_message_suivant_d_un_autre_auteur() {
        let t = tours_du_rail(&fil(), "moi");
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].id, "u1");
        assert_eq!(t[0].question, "Question 1 suite");
        assert_eq!(t[0].answer, "Réponse 1.");
        // u2 est suivi d'une autre question : pas de réponse.
        assert_eq!(t[1].answer, "");
        assert_eq!(t[2].answer, "Réponse 3");
    }

    #[test]
    fn le_fichier_est_la_premiere_piece_de_la_question_puis_de_la_reponse() {
        let t = tours_du_rail(&fil(), "moi");
        assert_eq!(t[0].file.as_deref(), Some("schema.png"));
        assert_eq!(t[1].file.as_deref(), Some("a.pdf"));
        assert_eq!(t[2].file, None);
    }

    #[test]
    fn question_et_reponse_sont_coupees() {
        let long = "é".repeat(500);
        let f = vec![
            msg("u", "moi", vec![texte(&long)]),
            msg("a", "agent", vec![texte(&long)]),
        ];
        let t = tours_du_rail(&f, "moi");
        assert_eq!(t[0].question.chars().count(), QUESTION_MAX);
        assert_eq!(t[0].answer.chars().count(), REPONSE_MAX);
    }

    #[test]
    fn la_ligne_nouveau_va_avant_le_premier_message_d_un_autre_apres_le_dernier_lu() {
        let f = fil();
        assert_eq!(premier_non_lu(&f, 1, "moi"), Some("a1"));
        assert_eq!(premier_non_lu(&f, 2, "moi"), Some("a3"));
        assert_eq!(premier_non_lu(&f, 5, "moi"), None);
        // Mes propres messages ne sont jamais « nouveaux ».
        assert_eq!(premier_non_lu(&f, 2, "agent"), Some("u2"));
    }

    #[test]
    fn le_lien_d_un_message_compte_a_partir_de_1() {
        assert_eq!(
            lien_du_message("agent", "c-lien", 4),
            "[[chat:agent/c-lien#m5]]"
        );
    }

    #[test]
    fn le_sous_chemin_m5_vise_le_rang_4() {
        assert_eq!(rang_du_lien("#m5"), Some(4));
        assert_eq!(rang_du_lien("#m1"), Some(0));
        assert_eq!(rang_du_lien("#m0"), None);
        assert_eq!(rang_du_lien("#m"), None);
        assert_eq!(rang_du_lien("#m5x"), None);
        assert_eq!(rang_du_lien("m5"), None);
        assert_eq!(rang_du_lien("#^bloc"), None);
    }
}
