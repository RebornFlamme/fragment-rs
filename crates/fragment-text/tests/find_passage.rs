// Port de app/src/ui/core/editor/findPassage.test.ts

use fragment_text::{PassageError, find_in_text, pick};

// describe('findInText — un passage dans un texte, blancs repliés')

#[test]
fn trouve_un_passage_et_rend_les_index_du_texte_d_origine() {
    let t = "Le  pyruvate\n  entre dans le cycle.";
    let m = find_in_text(t, "pyruvate entre")[0];
    assert_eq!(&t[m.start..m.end], "pyruvate\n  entre");
}

#[test]
fn toutes_les_occurrences_sans_chevauchement() {
    assert_eq!(find_in_text("aa aa aa", "aa").len(), 3);
    assert_eq!(find_in_text("abc", "zz"), vec![]);
    assert_eq!(find_in_text("abc", "   "), vec![]);
}

#[test]
fn le_passage_recopie_avec_d_autres_blancs_que_la_source() {
    let t = "- 2 Il faut rendre l'editor   Markdown customisable";
    let m = find_in_text(t, "  editor Markdown\ncustomisable ")[0];
    assert_eq!(&t[m.start..m.end], "editor   Markdown customisable");
}

#[test]
fn non_ascii_les_index_sont_en_unites_utf16() {
    // « é » : 1 unité (2 octets) ; 😀 : 2 unités (4 octets).
    let t = "é😀 le  pyruvate\n entre";
    let m = find_in_text(t, "pyruvate entre")[0];
    assert_eq!((m.start, m.end), (8, 23));
    assert_eq!(
        t.encode_utf16()
            .skip(m.start)
            .take(m.end - m.start)
            .collect::<Vec<_>>(),
        "pyruvate\n entre".encode_utf16().collect::<Vec<_>>()
    );
    let accents = find_in_text("été été", "été");
    assert_eq!(
        accents.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
        vec![(0, 3), (4, 7)]
    );
}

// describe('pick — l’occurrence voulue')

#[test]
fn une_seule_elle_plusieurs_sans_precision_ambigu_hors_bornes_erreur() {
    assert_eq!(pick(vec![1], None), Ok(1));
    assert_eq!(
        pick(vec![1, 2], None),
        Err(PassageError::Ambigu { count: 2 })
    );
    assert_eq!(pick(vec![1, 2], Some(2)), Ok(2));
    assert_eq!(
        pick(vec![1, 2], Some(3)),
        Err(PassageError::Occurrence { count: 2 })
    );
    assert_eq!(
        pick(Vec::<i32>::new(), None),
        Err(PassageError::Introuvable)
    );
}
