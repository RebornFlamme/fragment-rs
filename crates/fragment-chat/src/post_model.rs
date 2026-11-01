// Port de app/src/ui/views/chat/components/postModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  La présentation `post` d'une conversation — un TWEET (demande utilisateur :
//  « je veux être sur Twitter, littéralement »). Pur (testé) : ce qu'on montre
//  d'un fil — sa question, sa première réponse, combien de réponses — et l'heure
//  relative à la manière de X.
// ═══════════════════════════════════════════════════════════════════════════

use crate::api::{ChatMessage, Participant};
use crate::calendrier::{Fuseau, JOUR_MS, date_courte, date_longue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostMessage {
    pub author: Option<Participant>,
    pub text: String,
    pub at: Option<i64>,
    /// Nombre de pièces jointes (une puce, pas l'image en grand).
    pub attachments: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostModel {
    /// Le tweet : le 1er message de la conversation.
    pub post: Option<PostMessage>,
    /// La 1re réponse (le message suivant), reliée par le trait du fil.
    pub reply: Option<PostMessage>,
    /// Les réponses du fil (tous les messages après le premier).
    pub replies: usize,
    /// D'autres messages suivent la 1re réponse (« Afficher ce fil »).
    pub more: bool,
}

/// Le texte d'un message : ses parts texte, dans l'ordre.
fn texte(m: &ChatMessage) -> String {
    m.parts
        .iter()
        .filter(|p| p.kind == "text")
        .filter_map(|p| p.text.as_deref())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn en_post(m: Option<&ChatMessage>, participants: &[Participant]) -> Option<PostMessage> {
    let m = m?;
    Some(PostMessage {
        author: participants.iter().find(|p| p.id == m.author).cloned(),
        text: texte(m),
        // `m.at ? { at } : {}` : 0 est « absent ».
        at: m.at.filter(|&a| a != 0),
        attachments: m.parts.iter().filter(|p| p.kind == "attachment").count(),
    })
}

pub fn post_model(messages: &[ChatMessage], participants: &[Participant]) -> PostModel {
    PostModel {
        post: en_post(messages.first(), participants),
        reply: en_post(messages.get(1), participants),
        replies: messages.len().saturating_sub(1),
        more: messages.len() > 2,
    }
}

/// L'heure relative à la manière de X : « à l'instant », « 5 min », « 2 h », « hier »,
/// « 12 oct. » (« 12 oct. 2025 » une autre année). `t` et `now` en ms.
pub fn relative_time(t: i64, now: i64, fuseau: &dyn Fuseau) -> String {
    let ecart = now - t;
    if ecart < 60_000 {
        return "à l’instant".to_string();
    }
    if ecart < 3_600_000 {
        return format!("{} min", ecart / 60_000);
    }
    let minuit = fuseau.debut_du_jour(now);
    if t >= minuit {
        return format!("{} h", ecart / 3_600_000);
    }
    if t >= minuit - JOUR_MS {
        return "hier".to_string();
    }
    let c = fuseau.civil(t);
    if c.annee == fuseau.civil(now).annee {
        date_courte(c)
    } else {
        date_longue(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ChatPart, Role};
    use crate::calendrier::{DecalageFixe, instant};
    use serde_json::json;

    fn julien() -> Participant {
        Participant {
            id: "u".into(),
            name: "Julien".into(),
            initials: None,
            role: Role::User,
        }
    }

    fn claude() -> Participant {
        Participant {
            id: "c".into(),
            name: "Claude".into(),
            initials: None,
            role: Role::Assistant,
        }
    }

    fn m(id: &str, author: &str, parts: Vec<ChatPart>, at: Option<i64>) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            author: author.into(),
            parts,
            error: None,
            at,
        }
    }

    fn part(kind: &str, data: Option<serde_json::Value>) -> ChatPart {
        ChatPart {
            kind: kind.into(),
            data,
            ..ChatPart::default()
        }
    }

    mod post_model_un_fil_vu_comme_un_tweet {
        use super::*;

        #[test]
        fn le_1er_message_le_tweet_le_2e_la_reponse_le_nombre_de_reponses_la_suite() {
            let r = post_model(
                &[
                    m(
                        "1",
                        "u",
                        vec![
                            ChatPart::text(" test 2 "),
                            part("attachment", Some(json!({}))),
                        ],
                        Some(1000),
                    ),
                    m(
                        "2",
                        "c",
                        vec![
                            part("agent-trace", None),
                            ChatPart::text("Réponse"),
                            ChatPart::text("suite"),
                        ],
                        None,
                    ),
                    m("3", "u", vec![ChatPart::text("et ?")], None),
                ],
                &[julien(), claude()],
            );
            assert_eq!(
                r.post,
                Some(PostMessage {
                    author: Some(julien()),
                    text: "test 2".into(),
                    at: Some(1000),
                    attachments: 1
                })
            );
            assert_eq!(
                r.reply,
                Some(PostMessage {
                    author: Some(claude()),
                    text: "Réponse\n\nsuite".into(),
                    at: None,
                    attachments: 0
                })
            );
            assert_eq!(r.replies, 2);
            assert!(r.more);
        }

        #[test]
        fn vide_ou_sans_reponse() {
            assert_eq!(
                post_model(&[], &[julien()]),
                PostModel {
                    post: None,
                    reply: None,
                    replies: 0,
                    more: false
                }
            );
            let r = post_model(
                &[m("1", "x", vec![ChatPart::text("seul")], None)],
                &[julien()],
            );
            assert!(r.post.is_some_and(|p| p.author.is_none()));
            assert_eq!(r.reply, None);
            assert_eq!(r.replies, 0);
        }
    }

    mod relative_time_a_la_maniere_de_x {
        use super::*;

        const FUSEAU: DecalageFixe = DecalageFixe(0);

        #[test]
        fn instant_minutes_heures_aujourd_hui_hier_date_autre_annee() {
            let now = instant(&FUSEAU, 2026, 10, 20, 15);
            let il = |ms: i64| now - ms;
            let r = |t| relative_time(t, now, &FUSEAU);
            assert_eq!(r(il(20_000)), "à l’instant");
            assert_eq!(r(il(5 * 60_000)), "5 min");
            assert_eq!(r(il(2 * 3_600_000)), "2 h");
            assert_eq!(r(instant(&FUSEAU, 2026, 10, 19, 23)), "hier");
            assert_eq!(r(instant(&FUSEAU, 2026, 10, 12, 9)), "12 oct.");
            assert_eq!(r(instant(&FUSEAU, 2025, 3, 3, 0)), "3 mars 2025");
        }
    }
}
