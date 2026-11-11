// Port de app/src/ui/views/markdown/table/tableField.ts (separerDuTableau, saisie et collage sous un tableau)
//! Insérer du texte à la sélection sans casser un tableau GFM.
//!
//! ★ La règle GFM : un tableau ne s'arrête qu'à une ligne VIDE. Le texte tapé sur la ligne qui
//!   le suit directement en deviendrait une rangée. Si on insère sur une ligne vide collée à un
//!   tableau, on la garde vide et le texte part sur la ligne d'après.

use fragment_text::{
    EditorChangeSpec, EditorRange, EditorSelectionSpec, EditorTransactionSpec, utf16_len,
};

use crate::syntaxe::arbre::{Arbre, Parcours};
use crate::syntaxe::genre::Genre;

const LF: u16 = b'\n' as u16;

/// Début de la ligne qui contient `pos` (UTF-16).
fn debut_de_ligne(doc: &[u16], pos: usize) -> usize {
    let pos = pos.min(doc.len());
    doc[..pos]
        .iter()
        .rposition(|&u| u == LF)
        .map_or(0, |i| i + 1)
}

/// Fin (sans le `\n`) de la ligne qui contient `pos` (UTF-16).
fn fin_de_ligne(doc: &[u16], pos: usize) -> usize {
    let pos = pos.min(doc.len());
    doc[pos..]
        .iter()
        .position(|&u| u == LF)
        .map_or(doc.len(), |i| pos + i)
}

/// La plage du tableau qui contient `pos`, en LIGNES ENTIÈRES (UTF-16) : ce que la décoration du
/// tableau remplace. `None` hors d'un tableau.
#[must_use]
pub fn plage_du_tableau_a(doc: &str, pos: usize) -> Option<(usize, usize)> {
    let u: Vec<u16> = doc.encode_utf16().collect();
    let arbre = Arbre::analyser(doc);
    let mut trouve = None;
    arbre.parcourir(pos, pos, |_, n| {
        if trouve.is_some() {
            return Parcours::Sauter;
        }
        if n.genre != Genre::Table {
            return Parcours::Entrer;
        }
        trouve = Some((debut_de_ligne(&u, n.from()), fin_de_ligne(&u, n.to())));
        Parcours::Sauter
    });
    trouve
}

/// Le texte à insérer en `from`, préfixé d'un `\n` si `from` est une ligne vide collée sous un
/// tableau ; `insert` tel quel sinon.
#[must_use]
pub fn separer_du_tableau(doc: &str, from: usize, insert: &str) -> String {
    let u: Vec<u16> = doc.encode_utf16().collect();
    let debut = debut_de_ligne(&u, from);
    let fin = fin_de_ligne(&u, from);
    if debut != fin || debut == 0 || insert.is_empty() {
        return insert.to_owned();
    }
    match plage_du_tableau_a(doc, debut - 1) {
        Some((_, to)) if to == debut - 1 => format!("\n{insert}"),
        _ => insert.to_owned(),
    }
}

/// La transaction qui remplace la sélection par `texte` (frappe, collage de texte, embeds
/// d'images collées), curseur après le texte. Avec un curseur seul, `texte` passe par
/// [`separer_du_tableau`] ; une sélection non vide est remplacée telle quelle.
#[must_use]
pub fn inserer_a_la_selection(doc: &str, sel: EditorRange, texte: &str) -> EditorTransactionSpec {
    let insert = if sel.from == sel.to {
        separer_du_tableau(doc, sel.from, texte)
    } else {
        texte.to_owned()
    };
    EditorTransactionSpec {
        selection: Some(EditorSelectionSpec {
            anchor: sel.from + utf16_len(&insert),
            head: None,
        }),
        changes: vec![EditorChangeSpec {
            from: sel.from,
            to: Some(sel.to),
            insert,
        }],
        scroll_into_view: true,
    }
}
