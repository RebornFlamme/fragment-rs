// Port de app/src/ui/views/markdown/properties/frontmatterField.ts (curseurHorsDuBloc, saisie,
// frappeApresBlocFinal, entrer, selectionnerBloc).
//! Ce que les Properties demandent à l'éditeur de texte : les règles du CURSEUR et de la FRAPPE autour
//! d'un bloc dont la source est cachée. Des fonctions pures sur (texte, positions en octets).
//!
//! ★ Le curseur ne se pose jamais DANS la source cachée : une frappe y casserait le bloc.

use fragment_markdown::syntaxe::modele_rangees::lignes_de;

use super::modele::{Edition, lire_bloc};

/// La ligne qui suit le bloc, ou la fin du document si le bloc le termine.
fn apres(texte: &str, a: usize) -> usize {
    (a + 1).min(texte.len())
}

/// Un curseur VIDE qui tomberait dans la source cachée (ou sur son bord) est renvoyé sur la ligne
/// d'après : `Some(position)` quand il faut le déplacer.
#[must_use]
pub fn curseur_hors_du_bloc(texte: &str, debut: usize, fin: usize) -> Option<usize> {
    if debut != fin {
        return None;
    }
    let bloc = lire_bloc(texte)?;
    if debut > bloc.a {
        return None;
    }
    let cible = apres(texte, bloc.a);
    (debut != cible).then_some(cible)
}

/// La frappe de `saisi` à `de` (sélection vide) : ce que fait l'éditeur à la place du comportement
/// ordinaire, ou `None` pour le laisser faire.
///
/// - le troisième `-` de la ligne 1, sans frontmatter : `---\n---` (le bloc est créé ; la vue ouvre
///   ensuite une propriété) ;
/// - un caractère collé à la fin du bloc : il part sur une ligne neuve.
#[must_use]
pub fn saisie(texte: &str, de: usize, saisi: &str) -> Option<Edition> {
    let l1 = lignes_de(texte).first().copied()?.octets;
    if saisi == "-" && de == l1.to && &texte[l1.from..l1.to] == "--" && lire_bloc(texte).is_none() {
        let insert = "---\n---";
        let reste = texte.len() > l1.to;
        return Some(Edition {
            de: l1.from,
            a: l1.to,
            texte: insert.to_owned(),
            curseur: Some(insert.len() + usize::from(reste)),
        });
    }
    let bloc = lire_bloc(texte)?;
    if de == bloc.a {
        let insert = format!("\n{saisi}");
        return Some(Edition {
            de,
            a: de,
            curseur: Some(de + insert.len()),
            texte: insert,
        });
    }
    None
}

/// Le bloc termine le document et le curseur est juste après : Entrée crée la ligne neuve, un
/// caractère s'y écrit (il n'y a pas de position où poser le caret après un bloc final).
#[must_use]
pub fn frappe_apres_bloc_final(
    texte: &str,
    curseur: usize,
    saisi: Option<&str>,
) -> Option<Edition> {
    let bloc = lire_bloc(texte)?;
    if curseur != bloc.a || bloc.a != texte.len() {
        return None;
    }
    let insert = format!("\n{}", saisi.unwrap_or(""));
    Some(Edition {
        de: texte.len(),
        a: texte.len(),
        curseur: Some(texte.len() + insert.len()),
        texte: insert,
    })
}

/// ↑ depuis la première ligne VISUELLE qui suit le bloc : on entre dans les Properties.
/// `meme_rangee` : le curseur est sur la première rangée de sa ligne.
#[must_use]
pub fn entrer_par_le_haut(texte: &str, curseur: usize, meme_rangee: bool) -> bool {
    let Some(bloc) = lire_bloc(texte) else {
        return false;
    };
    let debut = lignes_de(texte)
        .iter()
        .find(|l| l.octets.from <= curseur && curseur <= l.octets.to)
        .map(|l| l.octets.from);
    debut == Some(bloc.a + 1) && meme_rangee
}

/// Retour arrière en début de la ligne qui suit : sélectionne le bloc (le second appui le supprime).
#[must_use]
pub fn retour_arriere(texte: &str, curseur: usize) -> Option<(usize, usize)> {
    let bloc = lire_bloc(texte)?;
    (curseur == bloc.a + 1).then_some((bloc.de, bloc.a))
}
