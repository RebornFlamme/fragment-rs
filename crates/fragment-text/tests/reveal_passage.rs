// Port de app/src/ui/core/editor/revealPassage.test.ts

use fragment_text::{EditorRange, PassageRef, SurfaceText, locate_passage, utf16_len};

fn texte(text: &str) -> SurfaceText {
    SurfaceText::identity(text)
}

fn plage(from: usize, to: usize) -> Option<EditorRange> {
    Some(EditorRange { from, to })
}

// describe('locatePassage')

const T: &str = "Intro.\n\nThe third is Kepler, a telescope.\n\nKepler again later.";

#[test]
fn sans_citation_les_offsets_bornes() {
    let t = texte(T);
    assert_eq!(locate_passage(&t, &PassageRef::new(2, 5)), plage(2, 5));
    assert_eq!(
        locate_passage(&t, &PassageRef::new(2, 9999)),
        plage(2, t.text.len())
    );
    assert_eq!(locate_passage(&t, &PassageRef::new(3, 3)), None);
}

#[test]
fn offsets_perimes_la_citation_relocalise_au_plus_pres_de_l_indice() {
    let t = texte(T);
    let i = T.rfind("Kepler").unwrap();
    assert_eq!(
        locate_passage(&t, &PassageRef::with_quote(i - 3, i, "Kepler")),
        plage(i, i + 6)
    );
    let j = T.find("Kepler").unwrap();
    assert_eq!(
        locate_passage(&t, &PassageRef::with_quote(j + 1, j + 2, "Kepler")),
        plage(j, j + 6)
    );
}

#[test]
fn citation_aux_blancs_replies_selection_sur_deux_paragraphes() {
    let t = texte(T);
    let a = T.find("telescope.").unwrap();
    let p = locate_passage(&t, &PassageRef::with_quote(0, 0, "telescope. Kepler again"));
    assert_eq!(p, plage(a, T.find(" later").unwrap()));
}

#[test]
fn carte_position_indice_d_une_surface_a_pas_fixe() {
    // TS : `toIndex: (p) => Math.round(p / 10)`.
    let s = SurfaceText::new("abc def", |i| i * 10, |p| (p + 5) / 10);
    assert_eq!(
        locate_passage(&s, &PassageRef::with_quote(0, 0, "def")),
        plage(40, 70)
    );
}

#[test]
fn non_ascii_positions_et_citation_en_unites_utf16() {
    // « É » = 1 unité, 😀 = 2 : « Kepler » commence à l'unité 9 (et non à l’octet 12).
    let txt = "Été 😀 : Kepler, puis Kepler.";
    let t = texte(txt);
    let n = utf16_len(txt);
    assert_eq!(n, 29);
    // Au plus près de l'indice : le second « Kepler » (unité 22).
    assert_eq!(
        locate_passage(&t, &PassageRef::with_quote(20, 21, "Kepler")),
        plage(22, 28)
    );
    assert_eq!(
        locate_passage(&t, &PassageRef::with_quote(0, 1, "Kepler")),
        plage(9, 15)
    );
    // Sans citation : bornes en unités.
    assert_eq!(locate_passage(&t, &PassageRef::new(2, 9999)), plage(2, n));
    // Citation aux blancs repliés après des caractères multi-unités.
    let u = texte("😀 a\n\n  b é");
    assert_eq!(
        locate_passage(&u, &PassageRef::with_quote(0, 0, "a b é")),
        plage(3, 11)
    );
}

#[test]
fn introuvable_null() {
    let t = texte(T);
    assert_eq!(
        locate_passage(&t, &PassageRef::with_quote(0, 0, "zzzzzzzzzzzz qqqqqqqqqq")),
        None
    );
}
