// Port de app/src/ui/core/editor/textDelta.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le changement minimal entre deux versions d'un texte REÇU (pas édité ici) et
//  le remappage d'un caractère à travers lui — pur. Partagé par les surfaces en
//  lecture seule dont le texte change sous elles : une part de conversation en
//  flux (chatSurfaceModel), une page HTML dont le JS réécrit le DOM (htmlTextModel).
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::Assoc;

/// Le changement minimal d'un texte (en octets).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextDelta {
    /// La plage remplacée dans l'ANCIEN texte…
    pub from: usize,
    pub to: usize,
    /// … par `insert` octets.
    pub insert: usize,
}

/// Un caractère remappé par [`map_through_delta`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MappedChar {
    pub ch: usize,
    pub deleted: bool,
}

/// Le changement minimal d'un texte (préfixe et suffixe communs).
///
/// ★ Comparé octet par octet (le TS comparait des unités UTF-16), puis les deux
///   bornes sont RAMENÉES à une frontière de caractère : « é » → « è » partage
///   son premier octet, et un `from` au milieu d'un caractère ne serait pas une
///   position découpable du texte.
pub fn text_delta(avant: &str, apres: &str) -> TextDelta {
    let a = avant.as_bytes();
    let b = apres.as_bytes();
    let max = a.len().min(b.len());
    let mut debut = 0;
    while debut < max && a[debut] == b[debut] {
        debut += 1;
    }
    while debut > 0 && !avant.is_char_boundary(debut) {
        debut -= 1;
    }
    let mut fin = 0;
    while fin < max - debut && a[a.len() - 1 - fin] == b[b.len() - 1 - fin] {
        fin += 1;
    }
    while fin > 0
        && !(avant.is_char_boundary(a.len() - fin) && apres.is_char_boundary(b.len() - fin))
    {
        fin -= 1;
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
