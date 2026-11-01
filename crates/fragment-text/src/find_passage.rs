// Port de app/src/ui/core/editor/findPassage.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  TROUVER UN PASSAGE dans un document — « à côté de cette phrase » → des
//  positions. Sur toute surface de texte (note, page HTML, conversation : le
//  texte entier ; PDF : page par page), sans nommer aucune vue.
//
//  Les blancs sont repliés des deux côtés (un passage recopié d'un rendu n'a pas
//  les retours ni les doubles espaces de la source). Plusieurs occurrences :
//  `occurrence` (1 = la première) choisit, sinon c'est AMBIGU — on ne devine pas.
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::{EditorPosition, EditorRange, FullText, TextSurface, full_text};

/// Une occurrence dans le texte d'ORIGINE : `[start, end)` en octets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextMatch {
    pub start: usize,
    pub end: usize,
}

/// Pourquoi un passage n'a pas été trouvé (TS : les variantes `{ error }` de `PassageFound`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PassageError {
    /// Le passage demandé est vide (ou blanc).
    Vide,
    /// Aucune occurrence.
    Introuvable,
    /// Le texte du document n'est pas encore chargé.
    PasEncore,
    /// Plusieurs occurrences et aucune n'est désignée.
    Ambigu { count: usize },
    /// `occurrence` hors de `1..=count`.
    Occurrence { count: usize },
}

/// Le `\s` de JavaScript, à l'identique — ★ pas `char::is_whitespace` : celui de
/// Rust (propriété White_Space) compte U+0085 que JS ignore, et ignore U+FEFF
/// que JS compte. Un passage recopié d'un rendu peut porter l'un comme l'autre.
pub(crate) fn est_blanc(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `s.replace(/\s+/g, ' ')` : chaque suite de blancs devient UN espace.
pub(crate) fn replier(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blanc = false;
    for c in s.chars() {
        if est_blanc(c) {
            if !blanc {
                out.push(' ');
            }
            blanc = true;
        } else {
            out.push(c);
            blanc = false;
        }
    }
    out
}

/// `s.trim()` de JavaScript (mêmes blancs que [`est_blanc`]).
pub(crate) fn trim_js(s: &str) -> &str {
    s.trim_matches(est_blanc)
}

/// Les occurrences de `passage` dans `text`, blancs repliés, en index (octets) du
/// texte d'ORIGINE. Pur.
pub fn find_in_text(text: &str, passage: &str) -> Vec<TextMatch> {
    let cherche = trim_js(&replier(passage)).to_owned();
    if cherche.is_empty() {
        return Vec::new();
    }
    // Le texte replié, et pour chaque OCTET replié l'index d'origine (un
    // caractère multi-octets garde ses octets consécutifs : la fin d'une
    // occurrence se relit par son dernier octet + 1).
    let mut plie = String::with_capacity(text.len());
    let mut origine: Vec<usize> = Vec::with_capacity(text.len());
    let mut blanc = false;
    for (i, c) in text.char_indices() {
        if est_blanc(c) {
            if !blanc {
                plie.push(' ');
                origine.push(i);
            }
            blanc = true;
        } else {
            plie.push(c);
            origine.extend(i..i + c.len_utf8());
            blanc = false;
        }
    }
    let mut out = Vec::new();
    let mut depuis = 0;
    while let Some(rel) = plie[depuis..].find(cherche.as_str()) {
        let k = depuis + rel;
        out.push(TextMatch {
            start: origine[k],
            end: origine[k + cherche.len() - 1] + 1,
        });
        depuis = k + cherche.len();
    }
    out
}

/// Choisit l'occurrence voulue parmi `n` trouvées (1 = la première). Pur.
pub fn pick<T>(found: Vec<T>, occurrence: Option<usize>) -> Result<T, PassageError> {
    let count = found.len();
    if count == 0 {
        return Err(PassageError::Introuvable);
    }
    let mut found = found;
    match occurrence {
        None if count == 1 => Ok(found.swap_remove(0)),
        None => Err(PassageError::Ambigu { count }),
        Some(n) if n < 1 || n > count => Err(PassageError::Occurrence { count }),
        Some(n) => Ok(found.swap_remove(n - 1)),
    }
}

/// Le passage dans une surface de texte ouverte → ses positions.
pub fn find_passage(
    surface: &dyn TextSurface,
    passage: &str,
    occurrence: Option<usize>,
) -> Result<EditorRange, PassageError> {
    if trim_js(passage).is_empty() {
        return Err(PassageError::Vide);
    }
    let found: Vec<EditorRange> = match full_text(surface) {
        FullText::NotYet => return Err(PassageError::PasEncore),
        FullText::Text(t) => find_in_text(&t.text, passage)
            .into_iter()
            .map(|m| EditorRange {
                from: t.to_pos(m.start),
                to: t.to_pos(m.end),
            })
            .collect(),
        FullText::Never => {
            // Sans texte entier (PDF) : unité par unité — un passage ne chevauche pas deux pages.
            let mut found = Vec::new();
            for n in 0..surface.line_count() {
                for m in find_in_text(&surface.get_line(n), passage) {
                    found.push(EditorRange {
                        from: surface.pos_to_offset(EditorPosition {
                            line: n,
                            ch: m.start,
                        }),
                        to: surface.pos_to_offset(EditorPosition { line: n, ch: m.end }),
                    });
                }
            }
            found
        }
    };
    pick(found, occurrence)
}

/// Le message d'erreur lisible d'une recherche ratée.
pub fn passage_error(r: PassageError, passage: &str) -> String {
    match r {
        PassageError::Vide => "passage vide".to_owned(),
        PassageError::PasEncore => {
            "le texte du document n’est pas encore chargé — réessaie dans un instant".to_owned()
        }
        PassageError::Introuvable => format!("passage introuvable : « {passage} »"),
        PassageError::Ambigu { count } => format!(
            "passage ambigu : {count} occurrences de « {passage} » — précise occurrence=1…{count} (ou un passage plus long)"
        ),
        PassageError::Occurrence { count } => {
            format!("occurrence hors bornes : il y en a {count}")
        }
    }
}
