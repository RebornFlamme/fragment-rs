// Port de app/src/ui/views/markdown/decorationPlugin.ts (toogleCheckbox, détection des cases)
//! Les cases à cocher des tâches : la bascule et le relevé.

use std::sync::LazyLock;

use fragment_text::{EditorChangeSpec, EditorTransactionSpec, utf16_len};
use regex::Regex;

use super::contexte::Doc;
use crate::syntaxe::{Arbre, Genre};

static RE_TACHE_SANS_TEXTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*[-*+]\s+)\[[ xX]\]$").unwrap_or_else(|_| unreachable!("motif constant"))
});

/// Une case à cocher du document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseTache {
    /// Position du `[` (UTF-16).
    pub pos: usize,
    pub cochee: bool,
}

/// Bascule la case dont le `[` est en `pos` : `[ ]` devient `[x]` et inversement.
///
/// ★ UN changement d'un seul caractère (celui du milieu), aucune sélection : le texte autour ne
///   bouge pas d'une unité, et « annuler » restitue exactement la case d'avant.
/// ★ L'état est lu dans le document AU MOMENT de l'appel (pas dans une valeur mémorisée au rendu) :
///   une case dessinée avant une frappe ne bascule jamais à l'envers.
#[must_use]
pub fn basculer_case(doc: &str, pos: usize) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let marqueur = d.slice(pos, pos + 3);
    let cochee = marqueur.contains(['x', 'X']);
    EditorTransactionSpec {
        changes: vec![EditorChangeSpec {
            from: pos + 1,
            to: Some(pos + 2),
            insert: if cochee { " " } else { "x" }.to_owned(),
        }],
        ..EditorTransactionSpec::default()
    }
}

/// Les cases du document, dans l'ordre : les `TaskMarker` de l'arbre, plus les `- [ ]` en FIN de
/// ligne que l'arbre ne reconnaît pas comme tâches (il exige du texte après le `]`) et que la vue
/// dessine quand même en case.
#[must_use]
pub fn cases_de_taches(doc: &str, arbre: &Arbre) -> Vec<CaseTache> {
    let d = Doc::new(doc);
    let mut cases: Vec<CaseTache> = Vec::new();
    for n in arbre.noeuds() {
        match n.genre {
            Genre::TaskMarker => {
                let marqueur = d.slice(n.from(), n.to());
                cases.push(CaseTache {
                    pos: n.from(),
                    cochee: marqueur.contains(['x', 'X']),
                });
            }
            Genre::ListMark => {
                let ligne = d.ligne_a(n.from());
                let texte = d.slice(ligne.from, ligne.to);
                if let Some(m) = RE_TACHE_SANS_TEXTE.captures(&texte) {
                    let debut = m.get(1).map_or(0, |g| utf16_len(g.as_str()));
                    let b = ligne.from + debut;
                    cases.push(CaseTache {
                        pos: b,
                        cochee: d.slice(b, b + 3).contains(['x', 'X']),
                    });
                }
            }
            _ => {}
        }
    }
    cases.sort_by_key(|c| c.pos);
    cases.dedup_by_key(|c| c.pos);
    cases
}
