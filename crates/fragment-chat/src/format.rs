// Port de app/src/ui/views/chat/format.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le rendu de BASE du texte d'un message — pur. Volontairement petit :
//  paragraphes, blocs de code ```, **gras**, *italique*, `code`. Un plugin qui veut
//  plus (puces ¶n, première phrase…) contribue `chat:text@1`.
//  (`renderText` du TS construit du DOM : il est refait dans fragment-ui.)
// ═══════════════════════════════════════════════════════════════════════════

use std::sync::OnceLock;

use regex::Regex;

use crate::api::{ChatMessage, Participant};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub bold: bool,
    pub em: bool,
    pub code: bool,
}

impl Inline {
    fn brut(text: &str) -> Self {
        Self {
            text: text.to_string(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    P { inlines: Vec<Inline> },
    Code { text: String, lang: String },
}

/// `^```(\S*)\s*$` : la langue d'une ouverture de bloc de code, si la ligne en est une.
fn ouverture_code(ligne: &str) -> Option<&str> {
    let reste = ligne.strip_prefix("```")?;
    let fin_lang = reste.find(char::is_whitespace).unwrap_or(reste.len());
    reste[fin_lang..]
        .chars()
        .all(char::is_whitespace)
        .then(|| &reste[..fin_lang])
}

/// `^```\s*$` : la fermeture.
fn fermeture_code(ligne: &str) -> bool {
    ligne
        .strip_prefix("```")
        .is_some_and(|r| r.chars().all(char::is_whitespace))
}

/// Le texte en blocs. Un bloc de code NON fermé (en plein flux) reste un bloc de code.
pub fn blocks(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let normalise = text.replace("\r\n", "\n");
    let lignes: Vec<&str> = normalise.split('\n').collect();
    let mut para: Vec<&str> = Vec::new();
    let finir_para = |para: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::P {
                inlines: inlines(&para.join("\n")),
            });
        }
        para.clear();
    };
    let mut i = 0;
    while i < lignes.len() {
        if let Some(lang) = ouverture_code(lignes[i]) {
            finir_para(&mut para, &mut out);
            let mut code: Vec<&str> = Vec::new();
            i += 1;
            while i < lignes.len() && !fermeture_code(lignes[i]) {
                code.push(lignes[i]);
                i += 1;
            }
            out.push(Block::Code {
                text: code.join("\n"),
                lang: lang.to_string(),
            });
            // La fermeture (ou la fin du texte) est consommée ici.
            i += 1;
            continue;
        }
        if lignes[i].trim().is_empty() {
            finir_para(&mut para, &mut out);
        } else {
            para.push(lignes[i]);
        }
        i += 1;
    }
    finir_para(&mut para, &mut out);
    out
}

fn re_inline() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\*\*[^*]+\*\*|`[^`]+`|\*[^*\s][^*]*\*|_[^_\s][^_]*_)").ok())
        .as_ref()
}

/// **gras**, *italique* / _italique_, `code` — sans imbrication (assez pour un chat).
pub fn inlines(text: &str) -> Vec<Inline> {
    let Some(re) = re_inline() else {
        return vec![Inline::brut(text)];
    };
    let mut out = Vec::new();
    let mut dernier = 0;
    for m in re.find_iter(text) {
        if m.start() > dernier {
            out.push(Inline::brut(&text[dernier..m.start()]));
        }
        let t = m.as_str();
        // Les délimiteurs (`*`, `_`, `` ` ``) sont d'un octet : on les retire par octets.
        if t.starts_with("**") {
            out.push(Inline {
                text: t[2..t.len() - 2].to_string(),
                bold: true,
                ..Inline::default()
            });
        } else if t.starts_with('`') {
            out.push(Inline {
                text: t[1..t.len() - 1].to_string(),
                code: true,
                ..Inline::default()
            });
        } else {
            out.push(Inline {
                text: t[1..t.len() - 1].to_string(),
                em: true,
                ..Inline::default()
            });
        }
        dernier = m.end();
    }
    if dernier < text.len() {
        out.push(Inline::brut(&text[dernier..]));
    }
    out
}

/// Les initiales d'un nom : « Julien Bernard » → « JB », « Claude » → « C ».
pub fn initials_of(name: &str) -> String {
    let s: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|m| m.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if s.is_empty() { "?".to_string() } else { s }
}

/// Une conversation en Markdown (télécharger, copier) : un titre, puis chaque
/// message — « **Nom** » et son texte (les parts non textuels, appels d'outils…,
/// ne sont pas recopiés).
pub fn conversation_markdown(
    titre: &str,
    messages: &[ChatMessage],
    participants: &[Participant],
) -> String {
    let nom = |id: &str| {
        participants
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| id.to_string(), |p| p.name.clone())
    };
    let mut blocs = vec![format!("# {titre}")];
    for m in messages {
        let texte = m
            .parts
            .iter()
            .filter(|p| p.kind == "text")
            .filter_map(|p| p.text.as_deref())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let texte = texte.trim();
        if !texte.is_empty() {
            blocs.push(format!("**{}**\n\n{texte}", nom(&m.author)));
        }
    }
    blocs.join("\n\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ChatPart, Role};

    fn i(text: &str) -> Inline {
        Inline::brut(text)
    }

    fn gras(text: &str) -> Inline {
        Inline {
            bold: true,
            ..i(text)
        }
    }

    fn em(text: &str) -> Inline {
        Inline {
            em: true,
            ..i(text)
        }
    }

    fn code(text: &str) -> Inline {
        Inline {
            code: true,
            ..i(text)
        }
    }

    mod format_rendu_de_base_du_texte {
        use super::*;

        #[test]
        fn paragraphes_separes_par_une_ligne_vide_lignes_jointes() {
            let textes: Vec<String> = blocks("a\nb\n\nc")
                .into_iter()
                .filter_map(|b| match b {
                    Block::P { inlines } => Some(inlines.iter().map(|x| x.text.as_str()).collect()),
                    Block::Code { .. } => None,
                })
                .collect();
            assert_eq!(textes, vec!["a\nb", "c"]);
        }

        #[test]
        fn bloc_de_code_avec_langue_meme_non_ferme_flux_en_cours() {
            assert_eq!(
                blocks("x\n```ts\nconst a = 1;\n```\ny"),
                vec![
                    Block::P {
                        inlines: vec![i("x")]
                    },
                    Block::Code {
                        text: "const a = 1;".into(),
                        lang: "ts".into()
                    },
                    Block::P {
                        inlines: vec![i("y")]
                    },
                ]
            );
            assert_eq!(
                blocks("```\nencore"),
                vec![Block::Code {
                    text: "encore".into(),
                    lang: String::new()
                }]
            );
        }

        #[test]
        fn gras_italique_code_en_ligne() {
            assert_eq!(
                inlines("a **b** *c* `d` _e_"),
                vec![
                    i("a "),
                    gras("b"),
                    i(" "),
                    em("c"),
                    i(" "),
                    code("d"),
                    i(" "),
                    em("e")
                ]
            );
            assert_eq!(inlines("2 * 3 * 4"), vec![i("2 * 3 * 4")]);
        }

        #[test]
        fn initiales() {
            assert_eq!(initials_of("Claude"), "C");
            assert_eq!(initials_of("julien bernard"), "JB");
            assert_eq!(initials_of("  "), "?");
        }
    }

    mod conversation_markdown_tests {
        use super::*;

        fn msg(author: &str, parts: Vec<ChatPart>) -> ChatMessage {
            ChatMessage {
                author: author.into(),
                parts,
                ..ChatMessage::default()
            }
        }

        fn outil() -> ChatPart {
            ChatPart {
                kind: "agent-tool".into(),
                ..ChatPart::default()
            }
        }

        fn participant(id: &str, name: &str, role: Role) -> Participant {
            Participant {
                id: id.into(),
                name: name.into(),
                initials: None,
                role,
            }
        }

        #[test]
        fn un_titre_puis_chaque_message_nom_en_gras_son_texte_les_parts_non_textuels_sautent() {
            let md = conversation_markdown(
                "Ask « x »",
                &[
                    msg("u", vec![ChatPart::text("Bonjour ?")]),
                    msg(
                        "a",
                        vec![outil(), ChatPart::text("Salut."), ChatPart::text("Deux.")],
                    ),
                    msg("a", vec![outil()]),
                ],
                &[
                    participant("u", "Julien", Role::User),
                    participant("a", "Claude", Role::Assistant),
                ],
            );
            assert_eq!(
                md,
                "# Ask « x »\n\n**Julien**\n\nBonjour ?\n\n**Claude**\n\nSalut.\n\nDeux.\n"
            );
        }
    }
}
