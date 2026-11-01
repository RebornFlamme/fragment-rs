// Port de app/src/ui/views/html/htmlTextModel.test.ts

use fragment_text::{Assoc, EditorPosition, MappedPos, PageText, diff_page_text};

fn p(line: usize, ch: usize) -> EditorPosition {
    EditorPosition { line, ch }
}

fn mp(pos: usize, deleted: bool) -> MappedPos {
    MappedPos { pos, deleted }
}

// describe('PageText — le texte aplati d’une page, positions = index + 1')

fn t() -> PageText {
    PageText::new("Titre\nUn paragraphe.\nFin")
}

#[test]
fn lignes_morceaux_entre_deux_sauts_de_ligne() {
    let t = t();
    assert_eq!(t.line_count(), 3);
    assert_eq!(t.get_line(0), "Titre");
    assert_eq!(t.get_line(1), "Un paragraphe.");
    assert_eq!(t.get_line(2), "Fin");
    assert_eq!(t.get_line(3), "");
}

#[test]
fn position_0_le_coin_pas_un_caractere_1_le_premier_caractere() {
    let t = t();
    assert_eq!(t.offset_to_pos(0), p(0, 0));
    assert_eq!(t.offset_to_pos(1), p(0, 0));
    assert_eq!(t.offset_to_pos(7), p(1, 0));
    assert_eq!(t.offset_to_pos(t.end()), p(2, 3));
}

#[test]
fn aller_retour_position_ligne_caractere() {
    let t = t();
    for pos in 1..=t.end() {
        assert_eq!(t.pos_to_offset(t.offset_to_pos(pos)), pos);
    }
}

#[test]
fn le_texte_entier_et_sa_carte() {
    let t = t();
    let s = t.surface_text();
    assert_eq!(s.text, "Titre\nUn paragraphe.\nFin");
    let i = s.text.find("paragraphe").unwrap();
    assert_eq!(s.to_index(s.to_pos(i)), i);
    assert_eq!(s.to_pos(i), i + 1);
}

#[test]
fn page_vide_une_ligne_vide() {
    let v = PageText::new("");
    assert_eq!(v.line_count(), 1);
    assert_eq!(v.end(), 1);
    assert_eq!(v.offset_to_pos(5), p(0, 0));
}

#[test]
fn non_ascii_lignes_colonnes_et_positions_en_unites_utf16() {
    // « é » : 1 unité ; 😀 : 2 unités. Texte de 8 unités (un \n compris).
    let t = PageText::new("é😀\nÀ😀b");
    assert_eq!(t.end(), 9);
    assert_eq!(t.line_count(), 2);
    assert_eq!(t.get_line(0), "é😀");
    assert_eq!(t.get_line(1), "À😀b");
    assert_eq!(t.offset_to_pos(5), p(1, 0));
    assert_eq!(t.offset_to_pos(t.end()), p(1, 4));
    assert_eq!(t.pos_to_offset(p(1, 3)), 8);
    // La colonne est bornée à la longueur UTF-16 de la ligne.
    assert_eq!(t.pos_to_offset(p(0, 99)), 4);
    for pos in 1..=t.end() {
        assert_eq!(t.pos_to_offset(t.offset_to_pos(pos)), pos);
    }
    let s = t.surface_text();
    assert_eq!(s.to_pos(4), 5);
    assert_eq!(s.to_index(99), 8);
}

#[test]
fn non_ascii_le_diff_est_en_unites_utf16() {
    let avant = "😀 9\nLe passage";
    let apres = "😀 10\nLe passage";
    let d = diff_page_text(avant, apres);
    // « passage » : unité 8 avant (position 9), 9 après (position 10).
    assert_eq!(d.map_pos(9, Assoc::After), mp(10, false));
    assert_eq!(d.map_pos(2, Assoc::After), mp(2, false));
    // La fin du texte d'avant est bornée en unités (15), pas en octets.
    assert_eq!(d.map_pos(500, Assoc::After), mp(17, false));
}

// describe('diffPageText — le JS de la page réécrit son texte')

#[test]
fn rien_n_a_change_identite() {
    let d = diff_page_text("abc", "abc");
    assert!(!d.changed);
    assert_eq!(d.map_pos(2, Assoc::After), mp(2, false));
}

#[test]
fn insertion_avant_un_passage_il_glisse_apres_il_reste() {
    let avant = "Compteur 9\nLe passage cité.";
    let apres = "Compteur 10\nLe passage cité.";
    let d = diff_page_text(avant, apres);
    let pos = avant.find("passage").unwrap() + 1;
    assert_eq!(
        d.map_pos(pos, Assoc::After),
        mp(apres.find("passage").unwrap() + 1, false)
    );
    assert_eq!(d.map_pos(2, Assoc::After), mp(2, false));
}

#[test]
fn le_passage_supprime_marque_deleted() {
    let d = diff_page_text("a bcd e", "a e");
    assert!(d.map_pos(4, Assoc::After).deleted);
}

#[test]
fn le_coin_reste_le_coin() {
    let d = diff_page_text("abc", "zzabc");
    assert_eq!(d.map_pos(0, Assoc::After), mp(0, false));
}
