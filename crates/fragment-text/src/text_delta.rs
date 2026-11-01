// Port de app/src/ui/core/editor/textDelta.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le changement minimal entre deux versions d'un texte REÇU (pas édité ici) et
//  le remappage d'un caractère à travers lui — pur. Partagé par les surfaces en
//  lecture seule dont le texte change sous elles : une part de conversation en
//  flux (chatSurfaceModel), une page HTML dont le JS réécrit le DOM (htmlTextModel).
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::Assoc;

/// Le changement minimal d'un texte (en unités de code UTF-16, ADR-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextDelta {
    /// La plage remplacée dans l'ANCIEN texte (unités de code UTF-16, ADR-004)…
    pub from: usize,
    pub to: usize,
    /// … par `insert` unités de code UTF-16.
    pub insert: usize,
}

/// Un caractère remappé par [`map_through_delta`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MappedChar {
    /// Index UTF-16 dans le nouveau texte (ADR-004).
    pub ch: usize,
    pub deleted: bool,
}

/// Le changement minimal d'un texte (préfixe et suffixe communs).
///
/// ★ Comparé unité UTF-16 par unité, EXACTEMENT comme le TS (`charCodeAt`) : « é » →
///   « è » donne `{ from: 0, to: 1, insert: 1 }`. Deux émojis qui partagent leur
///   unité haute (😀 / 😁) donnent, comme en TS, un `from` au milieu de la paire :
///   ce ne sont que des nombres, jamais découpés dans un `&str`.
pub fn text_delta(avant: &str, apres: &str) -> TextDelta {
    let a: Vec<u16> = avant.encode_utf16().collect();
    let b: Vec<u16> = apres.encode_utf16().collect();
    let max = a.len().min(b.len());
    let mut debut = 0;
    while debut < max && a[debut] == b[debut] {
        debut += 1;
    }
    let mut fin = 0;
    while fin < max - debut && a[a.len() - 1 - fin] == b[b.len() - 1 - fin] {
        fin += 1;
    }
    TextDelta {
        from: debut,
        to: a.len() - fin,
        insert: b.len() - fin - debut,
    }
}

/// Un caractère `ch` de l'ancien texte → le nouveau. `assoc` comme `EditorChange::map_pos` :
/// `After` colle au caractère qui suit (une insertion pile dessus le pousse), `Before` à celui
/// qui précède.
pub fn map_through_delta(d: TextDelta, ch: usize, assoc: Assoc) -> MappedChar {
    if ch < d.from || (ch == d.from && (assoc == Assoc::Before || d.to > d.from)) {
        return MappedChar { ch, deleted: false };
    }
    if ch > d.to || (ch == d.to && d.to > d.from) {
        return MappedChar {
            ch: ch + d.insert - (d.to - d.from),
            deleted: false,
        };
    }
    MappedChar {
        ch: d.from + if assoc == Assoc::After { d.insert } else { 0 },
        deleted: d.from != d.to,
    }
}
