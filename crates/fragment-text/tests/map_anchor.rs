// Port de app/src/ui/core/layers/mapAnchor.test.ts

use fragment_text::{Assoc, MappedAnchor, MappedPos, TextAnchored, map_anchor};

/// L'union `WidgetAnchor` du TS, telle que fragment-layers la déclarera : ici,
/// une copie locale au test — la règle ne connaît que `TextAnchored`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WidgetAnchor {
    Viewport {
        x: i64,
        y: i64,
    },
    Document {
        pos: usize,
        to: Option<usize>,
        dx: i64,
        dy: i64,
    },
    Gutter {
        side: Side,
        pos: usize,
        to: Option<usize>,
        dy: i64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Right,
}

impl TextAnchored for WidgetAnchor {
    fn span(&self) -> Option<(usize, Option<usize>)> {
        match *self {
            WidgetAnchor::Viewport { .. } => None,
            WidgetAnchor::Document { pos, to, .. } | WidgetAnchor::Gutter { pos, to, .. } => {
                Some((pos, to))
            }
        }
    }

    fn with_span(self, pos: usize, to: Option<usize>) -> Self {
        match self {
            WidgetAnchor::Viewport { .. } => self,
            WidgetAnchor::Document { dx, dy, .. } => WidgetAnchor::Document { pos, to, dx, dy },
            WidgetAnchor::Gutter { side, dy, .. } => WidgetAnchor::Gutter { side, pos, to, dy },
        }
    }
}

/// Un mapPos de CM réduit : `n` caractères insérés en `at` (n < 0 : supprimés).
fn edition(at: usize, n: i64) -> impl Fn(usize, Assoc) -> MappedPos {
    move |pos, assoc| {
        if n < 0 {
            let fin = at + n.unsigned_abs() as usize;
            if pos > at && pos < fin {
                return MappedPos {
                    pos: at,
                    deleted: true,
                };
            }
            return MappedPos {
                pos: if pos >= fin {
                    pos - n.unsigned_abs() as usize
                } else {
                    pos
                },
                deleted: false,
            };
        }
        MappedPos {
            pos: if pos > at || (pos == at && assoc == Assoc::After) {
                pos + n as usize
            } else {
                pos
            },
            deleted: false,
        }
    }
}

fn mapped(anchor: WidgetAnchor, deleted: bool) -> MappedAnchor<WidgetAnchor> {
    MappedAnchor { anchor, deleted }
}

// describe('mapAnchor — LA règle du cœur (CM6 : RangeSet.map)')

#[test]
fn un_point_map_pos_pos_1_exactement() {
    let a = WidgetAnchor::Document {
        pos: 10,
        to: None,
        dx: 3,
        dy: 4,
    };
    let a12 = WidgetAnchor::Document {
        pos: 12,
        to: None,
        dx: 3,
        dy: 4,
    };
    assert_eq!(map_anchor(a, &edition(10, 2)), mapped(a12, false));
    assert_eq!(map_anchor(a, &edition(5, 2)), mapped(a12, false));
    assert_eq!(map_anchor(a, &edition(12, 2)), mapped(a, false));
    assert!(map_anchor(a, &edition(8, -4)).deleted);
}

#[test]
fn une_plage_la_regle_des_plages_supprimee_seulement_si_tout_son_texte_part() {
    let a = WidgetAnchor::Gutter {
        side: Side::Right,
        pos: 10,
        to: Some(20),
        dy: 0,
    };
    assert_eq!(
        map_anchor(a, &edition(15, 3)).anchor,
        WidgetAnchor::Gutter {
            side: Side::Right,
            pos: 10,
            to: Some(23),
            dy: 0,
        }
    );
    assert!(!map_anchor(a, &edition(8, -4)).deleted);
    assert!(map_anchor(a, &edition(9, -12)).deleted);
}

#[test]
fn une_ancre_a_l_ecran_passe_inchangee() {
    let a = WidgetAnchor::Viewport { x: 1, y: 2 };
    assert_eq!(map_anchor(a, &edition(0, 5)), mapped(a, false));
}
