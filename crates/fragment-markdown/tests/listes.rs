// Port de app/e2e/editor-hell.spec.ts (describe « listes et tâches ») et de
// app/src/ui/views/markdown/listKeys.ts.
//
// Le e2e pilote un vrai CodeMirror (frappe, touches) ; ici l'éditeur est un faux minimal (`Etat`) :
// un texte UTF-16, une sélection, et les touches Entrée / Retour arrière / Tab qui appellent le
// MODÈLE (`fragment_markdown::listes`) puis appliquent la transaction rendue. Quand le modèle rend
// `None`, la touche tombe sur le comportement par défaut de l'éditeur (nouvelle ligne, effacement
// d'un caractère), comme dans CodeMirror. Les cas ajoutés par le port sont préfixés `port_`.
//
// Les cas écrits comme `taper("- un")` simulent la frappe : on insère à la sélection.
// Non porté : « alignement : sous-puce bien décalée, case alignée… » (mesures de pixels du DOM,
// c'est de la présentation : tâche fr-t3-widgets-inline).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use fragment_markdown::listes::{
    CaseTache, basculer_case, cases_de_taches, desindenter, entree, indenter, retour_arriere,
};
use fragment_markdown::syntaxe::analyser;
use fragment_text::{EditorRange, EditorTransactionSpec, utf16_len};

/// Largeur d'indentation de test : celle du réglage par défaut de Fragment (`tabSize: 4`).
const UNITE: usize = 4;

struct Etat {
    doc: Vec<u16>,
    sel: (usize, usize),
}

impl Etat {
    /// Document vide, curseur en tête (`note(page, '')` puis un clic dans l'éditeur).
    fn vide() -> Self {
        Self::avec("")
    }

    /// Une note neuve, curseur en fin de document (`note(page, texte)` puis `finDoc`).
    fn avec(texte: &str) -> Self {
        let n = utf16_len(texte);
        Self {
            doc: texte.encode_utf16().collect(),
            sel: (n, n),
        }
    }

    fn source(&self) -> String {
        String::from_utf16_lossy(&self.doc)
    }

    fn plage(&self) -> EditorRange {
        EditorRange {
            from: self.sel.0.min(self.sel.1),
            to: self.sel.0.max(self.sel.1),
        }
    }

    /// Applique une transaction : `changes` dans le document d'avant, `selection` dans celui d'après.
    fn appliquer(&mut self, spec: &EditorTransactionSpec) {
        let mut changes: Vec<_> = spec.changes.iter().collect();
        changes.sort_by_key(|c| c.from);
        // Sans `selection` explicite, le curseur est reporté à travers les changements.
        let curseur = self.sel.0;
        let mut decalage: isize = 0;
        let mut a_appliquer: Vec<(usize, usize, Vec<u16>)> = Vec::new();
        for c in &changes {
            let to = c.to.unwrap_or(c.from);
            let ins: Vec<u16> = c.insert.encode_utf16().collect();
            if curseur > to {
                decalage += ins.len() as isize - (to - c.from) as isize;
            }
            a_appliquer.push((c.from, to, ins));
        }
        for (from, to, ins) in a_appliquer.into_iter().rev() {
            self.doc.splice(from..to, ins);
        }
        self.sel = match spec.selection {
            Some(s) => (s.anchor, s.head.unwrap_or(s.anchor)),
            None => {
                let p = (curseur as isize + decalage) as usize;
                (p, p)
            }
        };
    }

    fn taper(&mut self, texte: &str) {
        let r = self.plage();
        let ins: Vec<u16> = texte.encode_utf16().collect();
        let n = ins.len();
        self.doc.splice(r.from..r.to, ins);
        self.sel = (r.from + n, r.from + n);
    }

    fn entree(&mut self) {
        let source = self.source();
        let arbre = analyser(&source);
        match entree(&source, &arbre, self.plage()) {
            Some(spec) => self.appliquer(&spec),
            None => self.taper("\n"),
        }
    }

    fn retour_arriere(&mut self) {
        let source = self.source();
        let arbre = analyser(&source);
        match retour_arriere(&source, &arbre, self.plage()) {
            Some(spec) => self.appliquer(&spec),
            None => {
                let r = self.plage();
                let from = if r.from == r.to {
                    r.from.saturating_sub(1)
                } else {
                    r.from
                };
                self.doc.drain(from..r.to);
                self.sel = (from, from);
            }
        }
    }

    fn tab(&mut self) {
        let source = self.source();
        let spec = indenter(&source, self.plage(), UNITE);
        self.appliquer(&spec);
    }

    fn maj_tab(&mut self) {
        let source = self.source();
        let spec = desindenter(&source, self.plage(), UNITE);
        self.appliquer(&spec);
    }
}

#[test]
fn puce_entree_continue_entree_sur_item_vide_sort_de_la_liste() {
    let mut e = Etat::vide();
    e.taper("- un");
    e.entree();
    assert_eq!(e.source(), "- un\n- ");
    e.taper("deux");
    e.entree();
    e.entree();
    assert_eq!(e.source(), "- un\n- deux\n");
    e.taper("fin");
    assert_eq!(e.source(), "- un\n- deux\nfin");
}

#[test]
fn tache_vide_puis_entree_la_tache_vide_disparait_sortie_de_liste_rien_de_casse() {
    let mut e = Etat::vide();
    e.taper("- [ ]");
    e.entree();
    assert_eq!(e.source(), "");
    e.taper("texte");
    assert_eq!(e.source(), "texte");
}

#[test]
fn tache_vide_en_fin_de_ligne_sans_texte_se_rend_en_case_pas_en_crochets_bruts() {
    let doc = "- [ ]\n- [x]\n\nautre";
    let arbre = analyser(doc);
    // Deux cases, la seconde cochée : ni `[ ]` ni `[x]` ne reste brut.
    assert_eq!(
        cases_de_taches(doc, &arbre),
        vec![
            CaseTache {
                pos: 2,
                cochee: false
            },
            CaseTache {
                pos: 8,
                cochee: true
            },
        ]
    );
}

#[test]
fn tache_entree_continue_avec_case_vide_meme_depuis_une_cochee_entree_sur_vide_sort() {
    let mut e = Etat::vide();
    e.taper("- [ ] a");
    e.entree();
    assert_eq!(e.source(), "- [ ] a\n- [ ] ");
    e.taper("b");
    e.entree();
    e.entree();
    assert_eq!(e.source(), "- [ ] a\n- [ ] b\n");

    let mut e = Etat::avec("- [x] fait");
    e.entree();
    assert_eq!(e.source(), "- [x] fait\n- [ ] ");
}

#[test]
fn liste_lache_items_separes_par_une_ligne_vide_entree_n_insere_jamais_de_ligne_vide() {
    let mut e = Etat::avec("- [ ] Un\n\n- [ ] Test");
    e.entree();
    assert_eq!(e.source(), "- [ ] Un\n\n- [ ] Test\n- [ ] ");
    e.taper("b");
    e.entree();
    assert_eq!(e.source(), "- [ ] Un\n\n- [ ] Test\n- [ ] b\n- [ ] ");
    // ★ « les lignes se suivent à l'écran » (le trou de ≤ 2 px du e2e) : sur le modèle, aucune
    //   ligne vide entre « Test » et « b ».
    let source = e.source();
    let lignes: Vec<&str> = source.lines().collect();
    let test = lignes.iter().position(|l| l.ends_with("Test")).unwrap();
    assert_eq!(lignes[test + 1], "- [ ] b");
}

#[test]
fn entree_sur_un_2e_item_vide_sortie_de_liste_pas_de_ligne_vide_inseree() {
    let mut e = Etat::avec("- a\n- ");
    e.entree();
    assert_eq!(e.source(), "- a\n");
}

#[test]
fn backspace_juste_apres_case_vide_retire_la_case_et_garde_la_puce_puis_la_puce() {
    let mut e = Etat::avec("- [ ] ");
    e.retour_arriere();
    assert_eq!(e.source(), "- ");
    e.retour_arriere();
    assert_eq!(e.source(), "");
}

#[test]
fn liste_numerotee_entree_incremente() {
    let mut e = Etat::vide();
    e.taper("1. a");
    e.entree();
    e.taper("b");
    assert_eq!(e.source(), "1. a\n2. b");
}

#[test]
fn tab_et_maj_tab_imbriquent_et_desimbriquent_un_item() {
    let mut e = Etat::avec("- a\n- b");
    e.tab();
    let imbrique = e.source();
    // Attendu : /^- a\n[\t ]+- b$/
    let (tete, queue) = imbrique.split_once('\n').unwrap();
    assert_eq!(tete, "- a");
    let sans_blancs = queue.trim_start_matches(['\t', ' ']);
    assert!(
        sans_blancs.len() < queue.len(),
        "indentation attendue : {imbrique:?}"
    );
    assert_eq!(sans_blancs, "- b");
    e.maj_tab();
    assert_eq!(e.source(), "- a\n- b");
}

#[test]
fn cocher_decocher_au_clic_sans_deplacer_le_texte_undo() {
    let mut e = Etat::avec("- [ ] tâche\nfin");
    // La case est le `[` du marqueur (position 2).
    let source = e.source();
    let spec = basculer_case(&source, 2);
    // ★ UN seul changement, d'un caractère, et la sélection n'est pas touchée : le texte reste en place.
    assert_eq!(spec.changes.len(), 1);
    assert_eq!(spec.changes[0].from, 3);
    assert_eq!(spec.changes[0].to, Some(4));
    assert!(spec.selection.is_none());
    e.appliquer(&spec);
    assert_eq!(e.source(), "- [x] tâche\nfin");
    let source = e.source();
    e.appliquer(&basculer_case(&source, 2));
    assert_eq!(e.source(), "- [ ] tâche\nfin");
}

// ── Cas ajoutés par le port ──────────────────────────────────────────────

#[test]
fn port_entree_sur_item_vide_apres_une_liste_lache_sort_sans_ligne_vide() {
    let mut e = Etat::avec("- a\n\n- b\n- ");
    e.entree();
    assert_eq!(e.source(), "- a\n\n- b\n");
}

#[test]
fn port_entree_dans_une_sous_liste_vide_remonte_d_un_niveau() {
    let mut e = Etat::avec("- a\n    - ");
    e.entree();
    assert_eq!(e.source(), "- a\n- ");
}

#[test]
fn port_entree_dans_une_liste_numerotee_renumerote_les_items_suivants() {
    let mut e = Etat::avec("1. a\n2. b\n3. c");
    e.sel = (4, 4);
    e.entree();
    assert_eq!(e.source(), "1. a\n2. \n3. b\n4. c");
}

#[test]
fn port_entree_hors_liste_rend_none() {
    let doc = "un paragraphe";
    let arbre = analyser(doc);
    assert!(entree(doc, &arbre, EditorRange { from: 5, to: 5 }).is_none());
}

#[test]
fn port_entree_avec_une_selection_non_vide_rend_none() {
    let doc = "- abc";
    let arbre = analyser(doc);
    assert!(entree(doc, &arbre, EditorRange { from: 2, to: 4 }).is_none());
}

#[test]
fn port_entree_continue_une_citation() {
    let mut e = Etat::avec("> a");
    e.entree();
    assert_eq!(e.source(), "> a\n> ");
}

#[test]
fn port_entree_au_milieu_d_une_puce_coupe_la_ligne() {
    let mut e = Etat::avec("- ab");
    e.sel = (3, 3);
    e.entree();
    assert_eq!(e.source(), "- a\n- b");
}

#[test]
fn port_entree_depuis_une_puce_etoile_garde_le_marqueur() {
    let mut e = Etat::avec("* a");
    e.entree();
    assert_eq!(e.source(), "* a\n* ");
}

#[test]
fn port_entree_depuis_un_item_indente_garde_l_indentation() {
    let mut e = Etat::avec("- a\n  - b");
    e.entree();
    assert_eq!(e.source(), "- a\n  - b\n  - ");
}

#[test]
fn port_backspace_hors_liste_rend_none() {
    let doc = "abc";
    let arbre = analyser(doc);
    assert!(retour_arriere(doc, &arbre, EditorRange { from: 3, to: 3 }).is_none());
}

#[test]
fn port_backspace_apres_case_avec_texte_ne_retire_que_la_case() {
    // Le curseur juste derrière « - [ ] » : la case part, la puce et le texte restent.
    let mut e = Etat::avec("- [ ] a");
    e.sel = (6, 6);
    e.retour_arriere();
    assert_eq!(e.source(), "- a");
    assert_eq!(e.sel, (2, 2));
}

#[test]
fn port_backspace_apres_case_cochee() {
    let mut e = Etat::avec("- [x] ");
    e.retour_arriere();
    assert_eq!(e.source(), "- ");
}

#[test]
fn port_backspace_dans_le_texte_d_un_item_rend_none() {
    let doc = "- abc";
    let arbre = analyser(doc);
    assert!(retour_arriere(doc, &arbre, EditorRange { from: 4, to: 4 }).is_none());
}

#[test]
fn port_tab_indente_toutes_les_lignes_selectionnees() {
    let mut e = Etat::avec("- a\n- b\n- c");
    e.sel = (2, 9);
    e.tab();
    assert_eq!(e.source(), "    - a\n    - b\n    - c");
}

#[test]
fn port_maj_tab_sans_indentation_ne_fait_rien() {
    let doc = "- a";
    // La touche est consommée (comme `indentLess`), mais rien ne change.
    assert!(
        desindenter(doc, EditorRange { from: 3, to: 3 }, UNITE)
            .changes
            .is_empty()
    );
}

#[test]
fn port_maj_tab_retire_au_plus_une_unite() {
    let mut e = Etat::avec("        - a");
    e.maj_tab();
    assert_eq!(e.source(), "    - a");
    let mut e = Etat::avec("  - a");
    e.maj_tab();
    assert_eq!(e.source(), "- a");
}

#[test]
fn port_bascule_d_une_case_cochee_majuscule() {
    let spec = basculer_case("- [X] a", 2);
    assert_eq!(spec.changes[0].insert, " ");
}

#[test]
fn port_cases_de_taches_avec_texte_et_sans() {
    let doc = "- [ ] a\n- [x] b\n- c";
    let arbre = analyser(doc);
    assert_eq!(
        cases_de_taches(doc, &arbre),
        vec![
            CaseTache {
                pos: 2,
                cochee: false
            },
            CaseTache {
                pos: 10,
                cochee: true
            },
        ]
    );
}

#[test]
fn port_positions_utf16_apres_un_emoji() {
    // Un émoji compte pour 2 unités UTF-16 : la case de la 2e ligne est en 2 + 1 + 2.
    let doc = "😀\n- [ ] a";
    let spec = basculer_case(doc, 5);
    assert_eq!(spec.changes[0].from, 6);
    let mut e = Etat::avec("😀\n- a");
    e.entree();
    assert_eq!(e.source(), "😀\n- a\n- ");
}
