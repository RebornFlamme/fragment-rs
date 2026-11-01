// Port de app/src/ui/core/editor/textDelta.ts — le module TS n'a PAS de fichier
// de tests ; ces cas sont écrits d'après son contrat (et les données de
// htmlTextModel.test.ts qui l'exerce).

use fragment_text::{Assoc, MappedChar, TextDelta, map_through_delta, text_delta};

fn d(from: usize, to: usize, insert: usize) -> TextDelta {
    TextDelta { from, to, insert }
}

fn m(ch: usize, deleted: bool) -> MappedChar {
    MappedChar { ch, deleted }
}

// text_delta — le changement minimal (préfixe et suffixe communs)

#[test]
fn textes_identiques_un_changement_vide_a_la_fin() {
    assert_eq!(text_delta("abc", "abc"), d(3, 3, 0));
    assert_eq!(text_delta("", ""), d(0, 0, 0));
}

#[test]
fn insertion_au_milieu_prefixe_et_suffixe_gardes() {
    assert_eq!(text_delta("ab", "aXYb"), d(1, 1, 2));
    assert_eq!(text_delta("abc", "zzabc"), d(0, 0, 2));
    assert_eq!(text_delta("abc", "abczz"), d(3, 3, 2));
}

#[test]
fn remplacement_compteur_9_vers_10() {
    let avant = "Compteur 9\nLe passage cité.";
    let apres = "Compteur 10\nLe passage cité.";
    assert_eq!(text_delta(avant, apres), d(9, 10, 2));
}

#[test]
fn suppression_a_bcd_e_vers_a_e() {
    assert_eq!(text_delta("a bcd e", "a e"), d(2, 6, 0));
}

#[test]
fn non_ascii_les_positions_sont_des_unites_utf16_comme_le_ts() {
    // « é » et « è » : UNE unité UTF-16 chacun (le TS : charCodeAt).
    assert_eq!(text_delta("é", "è"), d(0, 1, 1));
    assert_eq!(text_delta("aé", "aè"), d(1, 2, 1));
    assert_eq!(text_delta("éa", "èa"), d(0, 1, 1));
    // Un émoji pèse DEUX unités (paire de substitution).
    assert_eq!(text_delta("a😀b", "ab"), d(1, 3, 0));
    assert_eq!(text_delta("ab", "a😀b"), d(1, 1, 2));
    // Comme en TS : 😀 (D83D DE00) et 😁 (D83D DE01) partagent leur unité haute.
    assert_eq!(text_delta("😀", "😁"), d(1, 2, 1));
}

// map_through_delta — un caractère de l'ancien texte → le nouveau

#[test]
fn avant_le_changement_inchange_apres_decale() {
    let delta = d(2, 6, 1);
    assert_eq!(map_through_delta(delta, 0, Assoc::After), m(0, false));
    assert_eq!(map_through_delta(delta, 1, Assoc::After), m(1, false));
    assert_eq!(map_through_delta(delta, 6, Assoc::After), m(3, false));
    assert_eq!(map_through_delta(delta, 10, Assoc::After), m(7, false));
}

#[test]
fn dans_la_plage_remplacee_effondre_et_deleted() {
    let delta = d(2, 6, 0);
    assert_eq!(map_through_delta(delta, 3, Assoc::After), m(2, true));
    assert_eq!(map_through_delta(delta, 5, Assoc::Before), m(2, true));
    // Les bornes d'une plage non vide restent : le début colle, la fin glisse.
    assert_eq!(map_through_delta(delta, 2, Assoc::After), m(2, false));
    assert_eq!(map_through_delta(delta, 6, Assoc::After), m(2, false));
}

#[test]
fn insertion_pile_sur_le_caractere_assoc_apres_pousse_avant_laisse() {
    let delta = d(2, 2, 3);
    assert_eq!(map_through_delta(delta, 2, Assoc::After), m(5, false));
    assert_eq!(map_through_delta(delta, 2, Assoc::Before), m(2, false));
    assert_eq!(map_through_delta(delta, 3, Assoc::After), m(6, false));
}
