// Port de app/src/ui/core/layers/WidgetLayer.ts (la fonction `mapAnchor` seulement)
//
// ═══════════════════════════════════════════════════════════════════════════
//  ★ LA RÈGLE DU CŒUR pour remapper une ancre à travers une édition — et rien
//    d'autre (CM6 : `RangeSet.map` / `ChangeSet.mapPos`). Chaque propriétaire
//    garde SES données, la forme de son ancre et sa POLITIQUE (un widget dont le
//    texte a disparu est détaché à l'écran, un trait d'annotation est retiré…) et
//    appelle cette règle sur ce qu'il possède — il ne la réécrit pas.
//
//    Un point colle au texte qui SUIT (assoc 1) ; une plage suit `map_range`.
//    `deleted` : le point a disparu — ou TOUT le texte de la plage (pas son seul début).
//    Une ancre à l'écran (`viewport`) passe inchangée.
//
//  ★ Transposition : en TS la règle est écrite sur l'union `WidgetAnchor`, qui vit
//    chez les calques (fragment-layers dépend de fragment-text, pas l'inverse :
//    docs/contrats.md). La règle ne lit d'une ancre que « suit-elle le texte, et
//    de quelle plage ? » : c'est le trait `TextAnchored`, que `WidgetAnchor`
//    (fragment-layers) et toute ancre d'un plugin implémentent.
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::{Assoc, MappedPos, map_range};

/// Ce que la règle a besoin de lire et d'écrire sur une ancre.
pub trait TextAnchored: Sized {
    // Les positions de la plage sont celles de la surface (texte : unités de code
    // UTF-16, ADR-004) ; la règle ne les interprète pas.
    /// La plage suivie : `(pos, to)` — `to` absent = un point. `None` : une ancre
    /// à l'écran (`viewport`), qui ne suit pas le texte.
    fn span(&self) -> Option<(usize, Option<usize>)>;
    /// La même ancre, sa plage remplacée.
    fn with_span(self, pos: usize, to: Option<usize>) -> Self;
}

/// Une ancre remappée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappedAnchor<A> {
    pub anchor: A,
    pub deleted: bool,
}

pub fn map_anchor<A: TextAnchored>(
    anchor: A,
    map_pos: &dyn Fn(usize, Assoc) -> MappedPos,
) -> MappedAnchor<A> {
    let Some((pos, to)) = anchor.span() else {
        return MappedAnchor {
            anchor,
            deleted: false,
        };
    };
    match to {
        None => {
            let m = map_pos(pos, Assoc::After);
            MappedAnchor {
                anchor: anchor.with_span(m.pos, None),
                deleted: m.deleted,
            }
        }
        Some(to) => {
            let m = map_range(map_pos, pos, to);
            MappedAnchor {
                anchor: anchor.with_span(m.from, Some(m.to)),
                deleted: m.deleted && m.to <= m.from,
            }
        }
    }
}
