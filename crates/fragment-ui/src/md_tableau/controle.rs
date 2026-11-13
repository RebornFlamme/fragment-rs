// Port de la logique de TableController (app/src/ui/views/markdown/table/TableWidget.ts : opérations
// des menus, `keysFor`, `commit` et son diff minimal) et de `selectionnerTableauVoisin`, `entrer`
// (tableField.ts).
//! La logique PURE du contrôleur de tableau : chaque opération des menus rend le nouveau modèle et la
//! cellule à focaliser ; chaque geste clavier d'une cellule rend ce qu'il faut faire ; l'écriture
//! dans la note se réduit à un remplacement minimal (l'historique reste fin : une frappe = un
//! caractère, et les frappes voisines se regroupent).

use fragment_markdown::table_model::{
    self as t, Align, SortDir, TableModel, col_count, get_cell, row_count,
};
use fragment_text::utf16_len;

/// Une opération de structure (une entrée de menu, une bande « + », un glisser, Suppr).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Trier(usize, SortDir),
    AjouterColonne(usize),
    AjouterLigne(usize),
    DeplacerColonne {
        de: usize,
        vers: usize,
    },
    DeplacerLigne {
        de: usize,
        vers: usize,
    },
    /// Re-cliquer l'alignement actif le retire.
    Aligner(usize, Align),
    DupliquerColonne(usize),
    DupliquerLigne(usize),
    SupprimerColonne(usize),
    SupprimerLigne(usize),
}

/// Le modèle après `op`, et la cellule à focaliser (`None` : personne).
#[must_use]
pub fn appliquer(m: &TableModel, op: Operation) -> (TableModel, Option<(usize, usize)>) {
    match op {
        Operation::Trier(c, sens) => (t::sort_by_column(m, c, sens), None),
        Operation::AjouterColonne(at) => (t::add_column(m, Some(at)), Some((0, at))),
        Operation::AjouterLigne(at) => (t::add_row(m, Some(at)), Some((at.max(1), 0))),
        Operation::DeplacerColonne { de, vers } => (t::move_column(m, de, vers), None),
        Operation::DeplacerLigne { de, vers } => (t::move_row(m, de, vers), None),
        Operation::Aligner(c, a) => {
            let actuel = m.aligns.get(c).copied().flatten();
            let nouveau = if actuel == Some(a) { None } else { Some(a) };
            (t::set_align(m, c, nouveau), None)
        }
        Operation::DupliquerColonne(c) => (t::duplicate_column(m, c), None),
        Operation::DupliquerLigne(r) => (t::duplicate_row(m, r), None),
        Operation::SupprimerColonne(c) => (t::delete_column(m, c), None),
        Operation::SupprimerLigne(r) => (t::delete_row(m, r), None),
    }
}

/// La suppression d'une colonne ou d'une ligne SÉLECTIONNÉE (Suppr, Retour arrière) : l'en-tête et
/// la dernière colonne ne se suppriment pas (`None`).
#[must_use]
pub fn suppression(m: &TableModel, colonne: bool, index: usize) -> Option<Operation> {
    if colonne {
        (col_count(m) > 1).then_some(Operation::SupprimerColonne(index))
    } else {
        (index > 0).then_some(Operation::SupprimerLigne(index))
    }
}

/// Un geste clavier qu'une cellule confie au tableau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Geste {
    /// Tab.
    Suivante,
    /// Maj+Tab.
    Precedente,
    /// Entrée.
    Entree,
    /// ↑ ; `bord` : le curseur est sur la PREMIÈRE ligne visuelle de la cellule.
    Haut { bord: bool },
    /// ↓ ; `bord` : sur la DERNIÈRE ligne visuelle.
    Bas { bord: bool },
    /// ← au début de la cellule (sélection vide).
    Gauche,
    /// → à la fin de la cellule.
    Droite,
}

/// Où sortir du tableau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sortie {
    Avant,
    Apres,
}

/// Ce que le tableau fait d'un geste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suite {
    /// Le geste n'est pas consommé : l'éditeur de la cellule le traite.
    Laisser,
    /// Focaliser (r, c), curseur à `curseur` (UTF-16 ; `None` : fin du texte).
    Focaliser {
        r: usize,
        c: usize,
        curseur: Option<usize>,
    },
    /// Une ligne ajoutée à la fin, puis le focus en (r, c).
    AjouterLigne {
        r: usize,
        c: usize,
    },
    Sortir(Sortie),
}

/// La suite du geste `g` dans la cellule (r, c) (`keysFor`).
#[must_use]
pub fn naviguer(m: &TableModel, r: usize, c: usize, g: Geste) -> Suite {
    let (lignes, cols) = (row_count(m), col_count(m));
    let long = |rr: usize, cc: usize| utf16_len(&get_cell(m, rr, cc));
    let focus = |r, c, curseur| Suite::Focaliser { r, c, curseur };
    match g {
        Geste::Suivante => {
            if c + 1 < cols {
                focus(r, c + 1, None)
            } else if r + 1 < lignes {
                focus(r + 1, 0, None)
            } else {
                Suite::AjouterLigne { r: r + 1, c: 0 }
            }
        }
        Geste::Precedente => {
            if c > 0 {
                focus(r, c - 1, None)
            } else if r > 0 {
                focus(r - 1, cols - 1, None)
            } else {
                // Consommé : Maj+Tab ne sort jamais du tableau.
                focus(r, c, None)
            }
        }
        Geste::Entree => {
            if r + 1 < lignes {
                focus(r + 1, c, None)
            } else {
                Suite::AjouterLigne { r: r + 1, c }
            }
        }
        Geste::Haut { bord } => {
            if !bord {
                Suite::Laisser
            } else if r > 0 {
                focus(r - 1, c, None)
            } else {
                Suite::Sortir(Sortie::Avant)
            }
        }
        Geste::Bas { bord } => {
            if !bord {
                Suite::Laisser
            } else if r + 1 < lignes {
                focus(r + 1, c, None)
            } else {
                Suite::Sortir(Sortie::Apres)
            }
        }
        Geste::Gauche => {
            if c > 0 {
                focus(r, c - 1, Some(long(r, c - 1)))
            } else if r > 0 {
                focus(r - 1, cols - 1, Some(long(r - 1, cols - 1)))
            } else {
                Suite::Sortir(Sortie::Avant)
            }
        }
        Geste::Droite => {
            if c + 1 < cols {
                focus(r, c + 1, Some(0))
            } else if r + 1 < lignes {
                focus(r + 1, 0, Some(0))
            } else {
                Suite::Sortir(Sortie::Apres)
            }
        }
    }
}

/// Le plus petit remplacement qui change `ancien` en `nouveau` : (début, fin dans `ancien`, texte
/// inséré), en OCTETS, sur des frontières de caractères ; `None` si rien ne change.
#[must_use]
pub fn diff_minimal(ancien: &str, nouveau: &str) -> Option<(usize, usize, String)> {
    if ancien == nouveau {
        return None;
    }
    let prefixe = ancien
        .char_indices()
        .zip(nouveau.chars())
        .take_while(|((_, a), b)| a == b)
        .last()
        .map_or(0, |((i, a), _)| i + a.len_utf8());
    let (a, n) = (&ancien[prefixe..], &nouveau[prefixe..]);
    let suffixe = a
        .chars()
        .rev()
        .zip(n.chars().rev())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| x.len_utf8())
        .sum::<usize>();
    Some((
        prefixe,
        ancien.len() - suffixe,
        nouveau[prefixe..nouveau.len() - suffixe].to_owned(),
    ))
}

/// La plage (octets) du texte de la cellule de grille (r, c) dans `source` : son contenu sans les
/// blancs qui l'entourent ; pour une cellule vide, une plage vide juste après la première espace.
/// `None` si la ligne n'a pas cette cellule.
#[must_use]
pub fn plage_cellule(source: &str, r: usize, c: usize) -> Option<(usize, usize)> {
    // La ligne 1 du source est celle des délimiteurs : elle n'est pas dans la grille.
    let li = if r == 0 { 0 } else { r + 1 };
    let mut debut = 0;
    let ligne = source.split('\n').enumerate().find_map(|(i, l)| {
        if i == li {
            Some(l)
        } else {
            debut += l.len() + 1;
            None
        }
    })?;
    let b = ligne.as_bytes();
    // Les `|` non échappés.
    let mut barres = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && b.get(i + 1) == Some(&b'|') {
            i += 2;
            continue;
        }
        if b[i] == b'|' {
            barres.push(i);
        }
        i += 1;
    }
    let tete = ligne.trim_start().starts_with('|');
    let mut bornes: Vec<usize> = Vec::new();
    if !tete {
        bornes.push(0);
    }
    bornes.extend(barres.iter().map(|&x| x + 1));
    let de = *bornes.get(c)?;
    let a = match barres.iter().copied().find(|&x| x >= de) {
        Some(x) => x,
        // Après le `|` final, il n'y a plus de cellule.
        None if ligne.trim_end().ends_with('|') && !barres.is_empty() => return None,
        None => b.len(),
    };
    let segment = &ligne[de..a];
    let contenu = segment.trim();
    let (x, y) = if contenu.is_empty() {
        let p = de + usize::from(segment.starts_with(' '));
        (p, p)
    } else {
        let gauche = segment.len() - segment.trim_start().len();
        (de + gauche, de + gauche + contenu.len())
    };
    Some((debut + x, debut + y))
}

/// Le source après l'écriture de `texte` dans la cellule (r, c), le reste INCHANGÉ (pas de
/// réalignement des colonnes) : une frappe reste une insertion pure, que l'historique de l'hôte
/// regroupe avec ses voisines. `None` si la cellule n'existe pas dans le source.
#[must_use]
pub fn ecrire_cellule(source: &str, r: usize, c: usize, texte: &str) -> Option<String> {
    let (de, a) = plage_cellule(source, r, c)?;
    let echappe = texte
        .replace("\r\n", " ")
        .replace('\n', " ")
        .replace('|', "\\|");
    let mut s = source.to_owned();
    s.replace_range(de..a, &echappe);
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::table_model::parse_table;

    fn m() -> TableModel {
        parse_table("| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |")
    }

    #[test]
    fn tab_passe_a_la_suivante_puis_ajoute_une_ligne() {
        let m = m();
        assert_eq!(
            naviguer(&m, 0, 0, Geste::Suivante),
            Suite::Focaliser {
                r: 0,
                c: 1,
                curseur: None
            }
        );
        assert_eq!(
            naviguer(&m, 0, 2, Geste::Suivante),
            Suite::Focaliser {
                r: 1,
                c: 0,
                curseur: None
            }
        );
        assert_eq!(
            naviguer(&m, 1, 2, Geste::Suivante),
            Suite::AjouterLigne { r: 2, c: 0 }
        );
        assert_eq!(
            naviguer(&m, 1, 0, Geste::Precedente),
            Suite::Focaliser {
                r: 0,
                c: 2,
                curseur: None
            }
        );
    }

    #[test]
    fn fleches_aux_bords_sortent_du_tableau() {
        let m = m();
        assert_eq!(
            naviguer(&m, 0, 1, Geste::Haut { bord: false }),
            Suite::Laisser
        );
        assert_eq!(
            naviguer(&m, 0, 1, Geste::Haut { bord: true }),
            Suite::Sortir(Sortie::Avant)
        );
        assert_eq!(
            naviguer(&m, 1, 2, Geste::Droite),
            Suite::Sortir(Sortie::Apres)
        );
        assert_eq!(
            naviguer(&m, 1, 0, Geste::Gauche),
            Suite::Focaliser {
                r: 0,
                c: 2,
                curseur: Some(1)
            }
        );
        assert_eq!(
            naviguer(&m, 1, 1, Geste::Entree),
            Suite::AjouterLigne { r: 2, c: 1 }
        );
    }

    #[test]
    fn realigner_l_alignement_actif_le_retire() {
        let (a, _) = appliquer(&m(), Operation::Aligner(0, Align::Center));
        assert_eq!(a.aligns[0], Some(Align::Center));
        let (b, _) = appliquer(&a, Operation::Aligner(0, Align::Center));
        assert_eq!(b.aligns[0], None);
    }

    #[test]
    fn en_tete_et_derniere_colonne_ne_se_suppriment_pas() {
        let m = m();
        assert_eq!(suppression(&m, false, 0), None);
        assert_eq!(
            suppression(&m, false, 1),
            Some(Operation::SupprimerLigne(1))
        );
        let une = parse_table("| A |\n| --- |\n| 1 |");
        assert_eq!(suppression(&une, true, 0), None);
    }

    #[test]
    fn ecrire_une_cellule_sans_realigner() {
        let src = "| A | B |\n| --- | --- |\n| 1 |     |";
        assert_eq!(plage_cellule(src, 0, 0), Some((2, 3)));
        assert_eq!(plage_cellule(src, 1, 1), Some((30, 30)));
        assert_eq!(
            ecrire_cellule(src, 0, 0, "Ax").as_deref(),
            Some("| Ax | B |\n| --- | --- |\n| 1 |     |")
        );
        assert_eq!(
            ecrire_cellule(src, 1, 1, "a|b").as_deref(),
            Some("| A | B |\n| --- | --- |\n| 1 | a\\|b    |")
        );
        assert_eq!(plage_cellule(src, 1, 2), None);
    }

    #[test]
    fn diff_minimal_d_une_frappe() {
        assert_eq!(
            diff_minimal("| A | B |", "| Ax | B |"),
            Some((3, 3, "x".to_owned()))
        );
        assert_eq!(diff_minimal("abc", "abc"), None);
        assert_eq!(diff_minimal("aéb", "ab"), Some((1, 3, String::new())));
        // Une répétition : le suffixe commun ne mange pas le préfixe.
        assert_eq!(diff_minimal("aa", "aaa"), Some((2, 2, "a".to_owned())));
    }
}
