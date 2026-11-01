// Port de app/src/ui/core/editor/mapRange.test.ts

use fragment_text::{Assoc, MappedPos, MappedRange, map_range};

/// Un mapPos de CM réduit : une insertion de `n` caractères en `at`.
fn insertion(at: usize, n: usize) -> impl Fn(usize, Assoc) -> MappedPos {
    move |pos, assoc| MappedPos {
        pos: if pos > at || (pos == at && assoc == Assoc::After) {
            pos + n
        } else {
            pos
        },
        deleted: false,
    }
}

// describe('mapRange — LA règle des plages (CM6 : décoration non inclusive)')

#[test]
fn taper_juste_avant_ou_juste_apres_ne_l_agrandit_pas_dedans_si() {
    assert_eq!(
        map_range(&insertion(10, 3), 10, 20),
        MappedRange {
            from: 13,
            to: 23,
            deleted: false
        }
    );
    assert_eq!(
        map_range(&insertion(20, 3), 10, 20),
        MappedRange {
            from: 10,
            to: 20,
            deleted: false
        }
    );
    assert_eq!(
        map_range(&insertion(15, 3), 10, 20),
        MappedRange {
            from: 10,
            to: 23,
            deleted: false
        }
    );
}

#[test]
fn jamais_a_l_envers() {
    let effondre = |_: usize, _: Assoc| MappedPos {
        pos: 5,
        deleted: true,
    };
    assert_eq!(
        map_range(&effondre, 10, 20),
        MappedRange {
            from: 5,
            to: 5,
            deleted: true
        }
    );
}
