// Port de app/src/ui/views/explorer/FileTree.tsx (`sortChildren`, `TreeNode` : titres, ordre)
//! L'arbre tel qu'on le voit : l'ordre des enfants (dossiers d'abord, puis noms naturels) et la
//! liste PLATE des lignes visibles (dossiers ouverts seulement), dans l'ordre d'affichage.
//!
//! Le tri reproduit `localeCompare(…, { numeric: true })` : les suites de chiffres se comparent
//! comme des nombres (« note 2 » avant « note 10 »), la casse et les accents ne comptent qu'à
//! égalité des lettres de base (« a » avant « B », « e » avant « é » à lettres égales).

use std::cmp::Ordering;

use fragment_core::{TAbstractFile, Vault};

use super::etat::EtatArbre;

/// Une ligne visible de l'arbre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneVisible {
    /// Le chemin du coffre (la clé de la ligne).
    pub chemin: String,
    /// Le titre affiché : le nom d'un dossier, le nom d'un fichier SANS son extension.
    pub titre: String,
    /// La profondeur (0 sous la racine).
    pub profondeur: usize,
    /// Un dossier ?
    pub dossier: bool,
    /// Un dossier ouvert ?
    pub ouvert: bool,
}

/// Le titre d'un nœud : le nom d'un dossier, le nom d'un fichier sans l'extension.
pub fn titre_de(noeud: &TAbstractFile) -> String {
    match noeud {
        TAbstractFile::File(f) => f.basename().to_owned(),
        TAbstractFile::Folder(d) => d.name().to_owned(),
    }
}

/// La lettre de base d'un caractère : minuscule, sans accent (le latin courant).
fn base(c: char) -> char {
    let m = c.to_lowercase().next().unwrap_or(c);
    match m {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        autre => autre,
    }
}

/// Le rang de classe de la collation : blancs, ponctuation, chiffres, lettres.
fn classe(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_ascii_digit() {
        2
    } else if c.is_alphabetic() {
        3
    } else {
        1
    }
}

/// Compare deux suites de chiffres comme des nombres (sans zéros de tête, puis par longueur).
fn comparer_nombres(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// Détache la suite de chiffres de tête d'un reste de chaîne.
fn chiffres_de_tete(s: &str) -> (&str, &str) {
    let fin = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(fin)
}

/// L'ordre « naturel » des noms (`localeCompare` numérique).
pub fn comparer_noms(a: &str, b: &str) -> Ordering {
    let (mut ra, mut rb) = (a, b);
    loop {
        match (ra.chars().next(), rb.chars().next()) {
            (None, None) => break,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let ((na, suite_a), (nb, suite_b)) =
                        (chiffres_de_tete(ra), chiffres_de_tete(rb));
                    let o = comparer_nombres(na, nb);
                    if o != Ordering::Equal {
                        return o;
                    }
                    (ra, rb) = (suite_a, suite_b);
                } else {
                    let o = classe(ca)
                        .cmp(&classe(cb))
                        .then_with(|| base(ca).cmp(&base(cb)));
                    if o != Ordering::Equal {
                        return o;
                    }
                    (ra, rb) = (&ra[ca.len_utf8()..], &rb[cb.len_utf8()..]);
                }
            }
        }
    }
    // Lettres de base égales : l'accent départage, puis la casse (la minuscule d'abord).
    let accents = |s: &str| -> Vec<bool> {
        s.chars()
            .map(|c| c.to_lowercase().next() != Some(base(c)))
            .collect()
    };
    accents(a).cmp(&accents(b)).then_with(|| {
        let majuscules = |s: &str| -> Vec<bool> { s.chars().map(char::is_uppercase).collect() };
        majuscules(a).cmp(&majuscules(b))
    })
}

/// Trie des enfants : les dossiers d'abord, puis les noms par [`comparer_noms`] ; `inverse`
/// retourne l'ordre des NOMS (les dossiers restent en tête).
pub fn trier_enfants(mut enfants: Vec<TAbstractFile>, inverse: bool) -> Vec<TAbstractFile> {
    enfants.sort_by(|a, b| {
        b.is_folder().cmp(&a.is_folder()).then_with(|| {
            let o = comparer_noms(a.name(), b.name());
            if inverse { o.reverse() } else { o }
        })
    });
    enfants
}

/// Les enfants TRIÉS d'un dossier.
pub fn enfants_tries(vault: &Vault, dossier: &str, etat: &EtatArbre) -> Vec<TAbstractFile> {
    let Some(d) = vault.get_folder_by_path(dossier) else {
        return Vec::new();
    };
    trier_enfants(vault.children(&d), etat.tri_inverse())
}

/// Les lignes visibles, dans l'ordre d'affichage (parcours en profondeur, dossiers ouverts).
pub fn lignes_visibles(vault: &Vault, etat: &EtatArbre) -> Vec<LigneVisible> {
    fn descendre(
        vault: &Vault,
        etat: &EtatArbre,
        dossier: &str,
        profondeur: usize,
        sortie: &mut Vec<LigneVisible>,
    ) {
        for noeud in enfants_tries(vault, dossier, etat) {
            let chemin = noeud.path().to_owned();
            let ouvert = noeud.is_folder() && etat.est_ouvert(&chemin);
            sortie.push(LigneVisible {
                titre: titre_de(&noeud),
                profondeur,
                dossier: noeud.is_folder(),
                ouvert,
                chemin: chemin.clone(),
            });
            if ouvert {
                descendre(vault, etat, &chemin, profondeur + 1, sortie);
            }
        }
    }
    let mut sortie = Vec::new();
    descendre(vault, etat, "", 0, &mut sortie);
    sortie
}
