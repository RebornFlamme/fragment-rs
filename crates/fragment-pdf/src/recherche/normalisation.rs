// Port de la `normalize` du contrôleur de recherche de pdf.js (web/pdf_find_controller,
// Apache-2.0) — la recherche de app/src/ui/views/pdf/PdfFindBar.ts s'appuie sur elle.
//
// Le texte d'une page est replié avant la recherche : ligatures et formes de
// compatibilité (NFKC), guillemets et tirets typographiques, signes diacritiques
// séparés (NFD), coupures de mot en fin de ligne, sauts de ligne en espaces. Chaque
// remplacement change la longueur ; la table `Diffs` retient où et de combien, pour
// que [`index_d_origine`] ramène une correspondance dans le texte BRUT.
//
// ★ Tous les offsets sont en unités UTF-16 (le `.length` de JS), même si le texte est
//   un `String` Rust : c'est l'espace des ancres d'annotations déjà persistées.

use std::sync::OnceLock;

use fragment_text::utf16::utf16_len;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// Caractères remplacés un à un (tirets, apostrophes, guillemets, fractions).
const CARS_A_NORMALISER: [(char, &str); 12] = [
    ('\u{2010}', "-"),
    ('\u{2018}', "'"),
    ('\u{2019}', "'"),
    ('\u{201A}', "'"),
    ('\u{201B}', "'"),
    ('\u{201C}', "\""),
    ('\u{201D}', "\""),
    ('\u{201E}', "\""),
    ('\u{201F}', "\""),
    ('\u{BC}', "1/4"),
    ('\u{BD}', "1/2"),
    ('\u{BE}', "3/4"),
];

/// Les signes combinants qui ne sont PAS des diacritiques à ignorer (voyelles
/// indiennes, virama…) : la requête les garde.
pub(crate) const EXCEPTIONS_DIACRITIQUES: &[u32] = &[
    0x3099, 0x309a, 0x094d, 0x09cd, 0x0a4d, 0x0acd, 0x0b4d, 0x0bcd, 0x0c4d, 0x0ccd, 0x0d3b, 0x0d3c,
    0x0d4d, 0x0dca, 0x0e3a, 0x0eba, 0x0f84, 0x1039, 0x103a, 0x1714, 0x1734, 0x17d2, 0x1a60, 0x1b44,
    0x1baa, 0x1bab, 0x1bf2, 0x1bf3, 0x2d7f, 0xa806, 0xa82c, 0xa8c4, 0xa953, 0xa9c0, 0xaaf6, 0xabed,
    0x0c56, 0x0f71, 0x0f72, 0x0f7a, 0x0f7b, 0x0f7c, 0x0f7d, 0x0f80, 0x0f74,
];

/// Un signe est-il dans la liste des exceptions ?
pub(crate) fn est_exception_diacritique(c: char) -> bool {
    EXCEPTIONS_DIACRITIQUES.contains(&(c as u32))
}

/// `c` appartient-il à la classe Unicode décrite par `motif` ? (un caractère, ancré)
fn est_classe(cellule: &'static OnceLock<Option<Regex>>, motif: &str, c: char) -> bool {
    let re = cellule.get_or_init(|| Regex::new(motif).ok());
    let mut tampon = [0u8; 4];
    re.as_ref()
        .is_some_and(|re| re.is_match(c.encode_utf8(&mut tampon)))
}

/// `\p{M}` : signe combinant.
pub(crate) fn est_marque(c: char) -> bool {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    c as u32 >= 0x300 && est_classe(&RE, r"^\p{M}$", c)
}

/// `\p{P}` : ponctuation.
pub(crate) fn est_ponctuation(c: char) -> bool {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    est_classe(&RE, r"^\p{P}$", c)
}

/// `\p{L}` : lettre.
pub(crate) fn est_lettre(c: char) -> bool {
    c.is_alphabetic() && {
        static RE: OnceLock<Option<Regex>> = OnceLock::new();
        est_classe(&RE, r"^\p{L}$", c)
    }
}

fn est_minuscule(c: char) -> bool {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    est_classe(&RE, r"^\p{Ll}$", c)
}

fn est_majuscule(c: char) -> bool {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    est_classe(&RE, r"^\p{Lu}$", c)
}

fn est_ideogramme(c: char) -> bool {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    est_classe(&RE, r"^\p{Ideographic}$", c) || ('\u{3040}'..='\u{30FF}').contains(&c)
}

/// `\s` de JavaScript (le mode Unicode y ajoute l'octet de marque BOM).
pub(crate) fn est_blanc(c: char) -> bool {
    c.is_whitespace() || c == '\u{FEFF}'
}

/// Un caractère du bloc des syllabes hangul (celles que NFD éclate en jamos).
fn est_syllabe(c: char) -> bool {
    matches!(c, '\u{AC00}'..='\u{D7AF}' | '\u{FA6C}' | '\u{FACF}'..='\u{FAD1}' | '\u{FAD5}'..='\u{FAD7}')
}

/// Premier jamo d'une syllabe décomposée.
fn est_jamo_initial(c: char) -> bool {
    matches!(c, '\u{1100}'..='\u{1112}' | '\u{D7A4}'..='\u{D7AF}')
}

/// Les caractères que NFKC change (hors signes combinants) : ligatures, espaces
/// insécables, formes pleine largeur…, dans le plan de base seulement.
fn change_par_nfkc(c: char) -> bool {
    if (c as u32) < 0xA0 || (c as u32) > 0xFFFF {
        return false;
    }
    let mut tampon = [0u8; 4];
    let s: &str = c.encode_utf8(&mut tampon);
    s.nfkc().ne(s.chars()) && !est_marque(c)
}

fn nfkc(c: char) -> String {
    let mut tampon = [0u8; 4];
    c.encode_utf8(&mut tampon).nfkc().collect()
}

/// La table de décalages : pour chaque début `starts[k]` (offset dans le texte
/// normalisé), ajouter `shifts[k]` pour retrouver l'offset dans le texte brut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diffs {
    starts: Vec<i64>,
    shifts: Vec<i64>,
}

/// Le texte normalisé d'une page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalise {
    /// Le texte replié, celui où l'on cherche.
    pub texte: String,
    /// Le retour vers le texte brut.
    pub diffs: Diffs,
    /// Le texte brut contenait des signes combinants (la requête doit alors les tolérer).
    pub a_diacritiques: bool,
}

/// Un cas de l'alternance de la `normalize` du TS (dans l'ordre où elle les essaie).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cas {
    /// Un caractère de [`CARS_A_NORMALISER`].
    Unitaire,
    /// Un caractère que NFKC change.
    Compat,
    /// Signe japonais combinant suivi d'un saut de ligne.
    JaponaisSaut,
    /// Une suite de signes combinants, avec un éventuel « -\n ».
    Marques,
    /// Mot coupé : lettre, tiret, saut de ligne, lettre qui suit.
    MotCoupe,
    /// Un caractère non blanc, un tiret, un saut de ligne.
    TiretSaut,
    /// Idéogramme ou kana suivi d'un saut de ligne.
    CjcSaut,
    /// Un saut de ligne seul.
    Saut,
    /// Le premier jamo d'une syllabe hangul décomposée.
    Jamo,
}

/// Quel cas s'applique en `k`, et sur combien de caractères ?
fn essayer(chars: &[char], k: usize, avec_syllabes: bool) -> Option<(Cas, usize)> {
    let c = chars[k];
    let a = |d: usize| chars.get(k + d).copied();
    if CARS_A_NORMALISER.iter().any(|(x, _)| *x == c) {
        return Some((Cas::Unitaire, 1));
    }
    if change_par_nfkc(c) {
        return Some((Cas::Compat, 1));
    }
    if matches!(c, '\u{3099}' | '\u{309A}') && a(1) == Some('\n') {
        return Some((Cas::JaponaisSaut, 2));
    }
    if est_marque(c) {
        let mut m = 1;
        while chars.get(k + m).is_some_and(|x| est_marque(*x)) {
            m += 1;
        }
        let avec_tiret = a(m) == Some('-') && a(m + 1) == Some('\n');
        return Some((Cas::Marques, if avec_tiret { m + 2 } else { m }));
    }
    if a(1) == Some('-') && a(2) == Some('\n') {
        let suivant = a(3);
        let coupe = (est_minuscule(c) && suivant.is_some_and(est_minuscule))
            || (est_majuscule(c) && suivant.is_some_and(est_lettre));
        if coupe {
            return Some((Cas::MotCoupe, 3));
        }
        if !est_blanc(c) {
            return Some((Cas::TiretSaut, 3));
        }
    }
    if est_ideogramme(c) && a(1) == Some('\n') {
        return Some((Cas::CjcSaut, 2));
    }
    if c == '\n' {
        return Some((Cas::Saut, 1));
    }
    if avec_syllabes && est_jamo_initial(c) {
        return Some((Cas::Jamo, 1));
    }
    None
}

/// Replie le texte d'une page (la `normalize` de pdf.js, options par défaut).
pub fn normaliser(texte: &str) -> Normalise {
    // Les signes combinants et les syllabes se repèrent dans le texte BRUT, avant NFD.
    let mut syllabes: Vec<(i64, i64)> = Vec::new();
    let mut marques_brutes: Vec<(i64, i64)> = Vec::new();
    let mut suite: Option<(i64, i64)> = None;
    let mut o: i64 = 0;
    for c in texte.chars() {
        let l = c.len_utf16() as i64;
        if est_syllabe(c) {
            let nfd_len = c.nfd().map(char::len_utf16).sum::<usize>() as i64;
            syllabes.push((nfd_len, o));
        }
        if est_marque(c) {
            match suite.as_mut() {
                Some(m) => m.0 += l,
                None => suite = Some((l, o)),
            }
        } else if let Some(m) = suite.take() {
            marques_brutes.push(m);
        }
        o += l;
    }
    if let Some(m) = suite.take() {
        marques_brutes.push(m);
    }
    let avec_syllabes = !syllabes.is_empty();

    let chars: Vec<char> = texte.nfd().collect();
    let mut off16 = Vec::with_capacity(chars.len() + 1);
    let mut acc = 0usize;
    for c in &chars {
        off16.push(acc);
        acc += c.len_utf16();
    }
    off16.push(acc);

    let mut positions: Vec<(i64, i64)> = vec![(0, 0)];
    let mut sortie = String::with_capacity(texte.len());
    let (mut idx_marque, mut idx_syllabe) = (0usize, 0usize);
    let (mut shift, mut shift_origine, mut eol) = (0i64, 0i64, 0i64);
    let mut a_diacritiques = false;

    let mut k = 0;
    while k < chars.len() {
        let Some((cas, n)) = essayer(&chars, k, avec_syllabes) else {
            sortie.push(chars[k]);
            k += 1;
            continue;
        };
        let brut: String = chars[k..k + n].iter().collect();
        let long = (off16[k + n] - off16[k]) as i64;
        let mut i = off16[k] as i64 - shift_origine;
        match cas {
            Cas::Unitaire | Cas::Compat => {
                let remplacement = if cas == Cas::Unitaire {
                    CARS_A_NORMALISER
                        .iter()
                        .find(|(x, _)| *x == chars[k])
                        .map(|(_, r)| (*r).to_string())
                        .unwrap_or_default()
                } else {
                    nfkc(chars[k])
                };
                let jj = utf16_len(&remplacement) as i64;
                for j in 1..jj {
                    positions.push((i - shift + j, shift - j));
                }
                shift -= jj - 1;
                sortie.push_str(&remplacement);
            }
            Cas::JaponaisSaut => {
                a_diacritiques = true;
                if marques_brutes
                    .get(idx_marque)
                    .is_some_and(|m| m.1 == i + eol)
                {
                    idx_marque += 1;
                } else {
                    positions.push((i - 1 - shift + 1, shift - 1));
                    shift -= 1;
                    shift_origine += 1;
                }
                positions.push((i - shift + 1, shift));
                shift_origine += 1;
                eol += 1;
                sortie.push(chars[k]);
            }
            Cas::Marques => {
                let fin_tiret = brut.ends_with('\n');
                let len = if fin_tiret { long - 2 } else { long };
                a_diacritiques = true;
                let mut jj = len;
                if let Some(m) = marques_brutes.get(idx_marque).filter(|m| m.1 == i + eol) {
                    jj -= m.0;
                    idx_marque += 1;
                }
                for j in 1..=jj {
                    positions.push((i - 1 - shift + j, shift - j));
                }
                shift -= jj;
                shift_origine += jj;
                if fin_tiret {
                    i += len - 1;
                    positions.push((i - shift + 1, 1 + shift));
                    shift += 1;
                    shift_origine += 1;
                    eol += 1;
                    sortie.extend(chars[k..k + n - 2].iter());
                } else {
                    sortie.push_str(&brut);
                }
            }
            Cas::MotCoupe => {
                let len = long - 2;
                positions.push((i - shift + len, 1 + shift));
                shift += 1;
                shift_origine += 1;
                eol += 1;
                sortie.push(chars[k]);
            }
            Cas::TiretSaut => {
                shift_origine += 1;
                eol += 1;
                sortie.extend(chars[k..k + 2].iter());
            }
            Cas::CjcSaut => {
                let len = long - 1;
                positions.push((i - shift + len, shift));
                shift_origine += 1;
                eol += 1;
                sortie.push(chars[k]);
            }
            Cas::Saut => {
                positions.push((i - shift + 1, shift - 1));
                shift -= 1;
                shift_origine += 1;
                eol += 1;
                sortie.push(' ');
            }
            Cas::Jamo => {
                if let Some(s) = syllabes.get(idx_syllabe).filter(|s| s.1 == i + eol) {
                    let nouvelle = s.0 - 1;
                    idx_syllabe += 1;
                    for j in 1..=nouvelle {
                        positions.push((i - (shift - j), shift - j));
                    }
                    shift -= nouvelle;
                    shift_origine += nouvelle;
                }
                sortie.push(chars[k]);
            }
        }
        k += n;
    }
    positions.push((utf16_len(&sortie) as i64, shift));
    Normalise {
        texte: sortie,
        diffs: Diffs {
            starts: positions.iter().map(|p| p.0).collect(),
            shifts: positions.iter().map(|p| p.1).collect(),
        },
        a_diacritiques,
    }
}

/// Le premier indice de `items[debut..]` qui vérifie `cond` (la recherche binaire du
/// TS), ou `items.len()` si aucun.
fn premier_indice(items: &[i64], cond: impl Fn(i64) -> bool, debut: usize) -> usize {
    let Some(&dernier) = items.last() else {
        return items.len();
    };
    if !cond(dernier) {
        return items.len();
    }
    if cond(items[debut]) {
        return debut;
    }
    let (mut min, mut max) = (debut, items.len() - 1);
    while min < max {
        let milieu = (min + max) >> 1;
        if cond(items[milieu]) {
            max = milieu;
        } else {
            min = milieu + 1;
        }
    }
    min
}

/// Une correspondance `(pos, len)` du texte normalisé → `(pos, len)` du texte brut.
///
/// Rend `None` si la plage brute est vide (la correspondance n'existe que dans ce que
/// la normalisation a ajouté).
pub fn index_d_origine(diffs: &Diffs, pos: usize, len: usize) -> Option<(usize, usize)> {
    if len == 0 {
        return None;
    }
    let debut = pos as i64;
    let fin = debut + len as i64 - 1;
    let starts = &diffs.starts;
    let mut i = premier_indice(starts, |x| x >= debut, 0);
    if starts.get(i).is_some_and(|s| *s > debut) {
        i -= 1;
    }
    let mut j = premier_indice(starts, |x| x >= fin, i);
    if starts.get(j).is_some_and(|s| *s > fin) {
        j -= 1;
    }
    let vieux_debut = debut + diffs.shifts.get(i).copied().unwrap_or(0);
    let vieille_fin = fin + diffs.shifts.get(j).copied().unwrap_or(0);
    let vieille_len = vieille_fin + 1 - vieux_debut;
    // ★ Le TS garde même une longueur négative (truthy) ; ici une plage vide ou
    //   inversée n'est pas une correspondance.
    (vieille_len > 0).then(|| (vieux_debut.max(0) as usize, vieille_len as usize))
}
