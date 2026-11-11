// Port de la correspondance du contrôleur de recherche de pdf.js (web/pdf_find_controller
// et pdf_find_utils, Apache-2.0), réglée par les options de app/src/ui/views/pdf/PdfFindBar.ts
// (Aa, mots entiers, diacritiques, tout surligner).
//
// La requête (déjà normalisée comme la page) devient une expression régulière : les
// blancs valent « un ou plusieurs espaces », la ponctuation tolère des espaces autour
// d'elle, les signes combinants sont ignorés sauf demande contraire. La page, elle,
// est le texte normalisé de [`super::normalisation`].
//
// Écarts avec le TS, assumés :
// - l'expression est celle du crate `regex`, qui n'a pas d'anticipation : celle de
//   « Match diacritics » (un signe combinant ne doit pas suivre) est vérifiée à la main
//   sur chaque occurrence, sans le repli du moteur JS vers une occurrence plus courte ;
// - les requêtes multiples (tableau de phrases) ne sont pas portées : la barre n'envoie
//   qu'une chaîne.

use regex::RegexBuilder;
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

use super::normalisation::{
    est_blanc, est_exception_diacritique, est_lettre, est_marque, est_ponctuation,
};

/// Les réglages de la barre — Aa et le menu des réglages. Ils survivent à la navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptionsRecherche {
    /// Aa : respecter la casse.
    pub casse: bool,
    /// « Whole words » : ne garder que les mots entiers.
    pub mot_entier: bool,
    /// « Match diacritics » : les accents comptent.
    pub diacritiques: bool,
    /// « Highlight all » : surligner toutes les occurrences, pas la seule courante.
    pub tout_surligner: bool,
}

/// L'expression régulière d'une requête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotifRequete {
    /// Le motif, à compiler avec la casse voulue.
    pub motif: String,
    /// Une occurrence ne doit pas être suivie d'un signe combinant (accents demandés
    /// sur une page qui en contient) : l'anticipation du TS.
    pub sans_marque_apres: bool,
}

/// Une suite de caractères de la requête, d'après `SPECIAL_CHARS_REG_EXP` du TS.
enum Morceau<'a> {
    /// `+ ^ $ |` : un caractère à échapper.
    Special(&'a str),
    /// Une suite de signes de ponctuation.
    Ponctuation(&'a str),
    /// Une suite de blancs.
    Blancs,
    /// Un signe combinant.
    Marque(char),
    /// Une lettre.
    Lettre(char),
    /// Tout le reste (chiffres, symboles), recopié tel quel.
    Autre(char),
}

/// Découpe la requête en morceaux, dans l'ordre d'essai du TS.
fn decouper(requete: &str) -> Vec<Morceau<'_>> {
    let mut morceaux = Vec::new();
    let indices: Vec<(usize, char)> = requete.char_indices().collect();
    let mut k = 0;
    while k < indices.len() {
        let (o, c) = indices[k];
        let suite = |pred: fn(char) -> bool| {
            let mut e = k;
            while e < indices.len() && pred(indices[e].1) {
                e += 1;
            }
            e
        };
        if matches!(c, '+' | '^' | '$' | '|') {
            morceaux.push(Morceau::Special(&requete[o..o + c.len_utf8()]));
            k += 1;
        } else if est_ponctuation(c) {
            let e = suite(est_ponctuation);
            let fin = indices.get(e).map_or(requete.len(), |p| p.0);
            morceaux.push(Morceau::Ponctuation(&requete[o..fin]));
            k = e;
        } else if est_blanc(c) {
            k = suite(est_blanc);
            morceaux.push(Morceau::Blancs);
        } else if est_marque(c) {
            morceaux.push(Morceau::Marque(c));
            k += 1;
        } else if est_lettre(c) {
            morceaux.push(Morceau::Lettre(c));
            k += 1;
        } else {
            morceaux.push(Morceau::Autre(c));
            k += 1;
        }
    }
    morceaux
}

/// La requête (normalisée) → expression régulière.
///
/// - `diacritiques` : l'option « Match diacritics » ;
/// - `a_diacritiques` : la page contient des signes combinants.
pub fn convertir_requete(requete: &str, diacritiques: bool, a_diacritiques: bool) -> MotifRequete {
    // Les espaces tolérés autour d'un bloc de ponctuation dépendent de sa place dans la requête.
    let avec_espaces = |original: &str, corrige: String| -> String {
        if original == requete {
            corrige
        } else if requete.starts_with(original) {
            format!("{corrige}[ ]*")
        } else if requete.ends_with(original) {
            format!("[ ]*{corrige}")
        } else {
            format!("[ ]*{corrige}[ ]*")
        }
    };
    let mut motif = String::new();
    for m in decouper(requete) {
        match m {
            Morceau::Special(s) | Morceau::Ponctuation(s) => {
                motif.push_str(&avec_espaces(s, regex::escape(s)));
            }
            Morceau::Blancs => motif.push_str("[ ]+"),
            Morceau::Marque(c) => {
                if diacritiques || est_exception_diacritique(c) {
                    motif.push(c);
                }
            }
            Morceau::Lettre(c) => {
                motif.push(c);
                if !diacritiques && a_diacritiques {
                    motif.push_str("\\p{M}*");
                }
            }
            Morceau::Autre(c) => motif.push(c),
        }
    }
    if let Some(sans) = motif.strip_suffix("[ ]*") {
        motif = sans.to_string();
    }
    MotifRequete {
        motif,
        sans_marque_apres: diacritiques && a_diacritiques,
    }
}

/// Y a-t-il une rupture de mot (UAX n° 29) à l'octet `pos` de `contenu` ?
fn rupture_de_mot(contenu: &str, pos: usize) -> bool {
    let mut curseur = GraphemeCursor::new(pos, contenu.len(), true);
    if !matches!(curseur.is_boundary(contenu, 0), Ok(true)) {
        return false;
    }
    let mut avant = GraphemeCursor::new(pos, contenu.len(), true);
    let Ok(Some(debut)) = avant.prev_boundary(contenu, 0) else {
        return false;
    };
    let Ok(Some(fin)) = curseur.next_boundary(contenu, 0) else {
        return false;
    };
    contenu[debut..fin]
        .split_word_bound_indices()
        .any(|(i, _)| debut + i == pos)
}

/// La plage `[debut, fin[` (octets) est-elle un mot entier ?
pub fn est_mot_entier(contenu: &str, debut: usize, fin: usize) -> bool {
    (debut == 0 || rupture_de_mot(contenu, debut))
        && (fin == contenu.len() || rupture_de_mot(contenu, fin))
}

/// Les occurrences de `requete` (déjà normalisée) dans `contenu` (la page normalisée),
/// en `(index, longueur)` UTF-16 de `contenu`. Une requête vide, ou une expression que
/// le moteur refuse, ne trouve rien.
pub fn trouver(
    requete: &str,
    contenu: &str,
    a_diacritiques: bool,
    options: OptionsRecherche,
) -> Vec<(usize, usize)> {
    let motif = convertir_requete(requete, options.diacritiques, a_diacritiques);
    if motif.motif.is_empty() {
        return Vec::new();
    }
    let Ok(re) = RegexBuilder::new(&motif.motif)
        .case_insensitive(!options.casse)
        .build()
    else {
        return Vec::new();
    };
    let mut octets: Vec<(usize, usize)> = Vec::new();
    let mut pos = 0;
    while pos <= contenu.len() {
        let Some(m) = re.find_at(contenu, pos) else {
            break;
        };
        let apres = contenu[m.end()..].chars().next();
        let pas_d_ajout = apres.is_none_or(|c| est_exception_diacritique(c) || !est_marque(c));
        if motif.sans_marque_apres && !pas_d_ajout {
            // L'anticipation échoue : on repart un caractère après le début.
            pos = m.start()
                + contenu[m.start()..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
            continue;
        }
        pos = if m.is_empty() {
            m.end() + contenu[m.end()..].chars().next().map_or(1, char::len_utf8)
        } else {
            m.end()
        };
        if m.is_empty() || (options.mot_entier && !est_mot_entier(contenu, m.start(), m.end())) {
            continue;
        }
        octets.push((m.start(), m.end()));
    }
    if octets.is_empty() {
        return Vec::new();
    }
    // Octets → unités UTF-16, en une passe.
    let mut en_utf16 = vec![0usize; contenu.len() + 1];
    let mut n = 0;
    for (o, c) in contenu.char_indices() {
        for d in 0..c.len_utf8() {
            en_utf16[o + d] = n;
        }
        n += c.len_utf16();
    }
    en_utf16[contenu.len()] = n;
    octets
        .into_iter()
        .map(|(d, f)| (en_utf16[d], en_utf16[f] - en_utf16[d]))
        .collect()
}
