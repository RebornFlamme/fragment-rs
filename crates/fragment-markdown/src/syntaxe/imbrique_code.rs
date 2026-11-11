// Port du principe de coloration par langage des blocs de code de Fragment (`codeLanguages` de
// app/src/ui/views/markdown/livePreview.ts + `codeHighlightStyle` de language.ts). Analyseur écrit
// pour fragment-rs : pas de grammaire Lezer, une table par langage (voir imbrique_langues.rs).
//! L'analyseur générique des blocs de code : ligne par ligne, avec un petit état qui passe d'une
//! ligne à l'autre (commentaire de bloc, chaîne sur plusieurs lignes, profondeur d'accolades CSS,
//! balise ouverte).
//!
//! Sortie : des (début, fin, jeton) en octets, triés, sans chevauchement ; les jetons voisins de
//! même nom sont fusionnés (comme `mergeTokens` de CodeMirror). Le texte sans jeton garde la couleur
//! du texte du bloc.

use super::jetons::JetonCode;
use super::langues::{Famille, Langue, dans};
use super::stex::poser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    /// Dans un commentaire de bloc ; on attend la fin.
    Bloc,
    /// Dans une chaîne qui couvre plusieurs lignes (guillemet, triple ou non).
    Chaine(u8, bool),
    /// Dans une balise HTML ouverte.
    Balise,
}

#[derive(Debug, Clone, Copy)]
struct Etat {
    mode: Mode,
    /// Profondeur d'accolades (CSS).
    profondeur: u32,
}

/// Colore les `segments` (plages en octets de `texte`, dans l'ordre) en `langue`. Chaque segment
/// est une suite de lignes ; l'état passe d'un segment au suivant.
pub(super) fn colorer(
    langue: &Langue,
    texte: &str,
    segments: &[(usize, usize)],
) -> Vec<(usize, usize, JetonCode)> {
    let mut sortie = Vec::new();
    let mut etat = Etat {
        mode: Mode::Normal,
        profondeur: 0,
    };
    for &(de, a) in segments {
        let Some(corps) = texte.get(de..a) else {
            continue;
        };
        let mut debut = de;
        for ligne in corps.split('\n') {
            colorer_ligne(langue, ligne, debut, &mut etat, &mut sortie);
            debut += ligne.len() + 1;
        }
    }
    sortie
}

fn est_ident(langue: &Langue, c: char) -> bool {
    c.is_alphanumeric() || langue.ident_extra.contains(c)
}

fn debut_ident(langue: &Langue, c: char) -> bool {
    c.is_alphabetic() || (langue.ident_extra.contains(c) && c != '-')
}

/// Le premier caractère non blanc de `s`.
fn prochain_non_blanc(s: &str) -> Option<char> {
    s.chars().find(|c| !c.is_whitespace())
}

/// Fin d'une chaîne ouverte en `i` (APRÈS le guillemet ouvrant) : (fin, fermée ?).
fn fin_chaine(s: &str, mut i: usize, guillemet: u8, triple: bool) -> (usize, bool) {
    let b = s.as_bytes();
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == guillemet {
            if !triple {
                return (i + 1, true);
            }
            if b.get(i + 1) == Some(&guillemet) && b.get(i + 2) == Some(&guillemet) {
                return (i + 3, true);
            }
        }
        i += 1;
    }
    (s.len(), false)
}

fn colorer_ligne(
    l: &Langue,
    s: &str,
    base: usize,
    st: &mut Etat,
    out: &mut Vec<(usize, usize, JetonCode)>,
) {
    if l.famille == Famille::Balises {
        return colorer_balises(s, base, st, out);
    }
    let b = s.as_bytes();
    let mut i = 0;
    let mut tete = true; // rien de significatif avant, sur cette ligne
    if let Famille::Cles(sep) = l.famille
        && st.mode == Mode::Normal
    {
        i = cle_en_tete(s, sep, base, out);
        tete = i == 0;
    }
    while i < s.len() {
        match st.mode {
            Mode::Bloc => {
                let fin = l.bloc.map_or("", |(_, f)| f);
                let (a, ferme) = match s[i..].find(fin) {
                    Some(k) => (i + k + fin.len(), true),
                    None => (s.len(), false),
                };
                poser(out, base + i, base + a, JetonCode::Commentaire);
                if ferme {
                    st.mode = Mode::Normal;
                }
                i = a;
                continue;
            }
            Mode::Chaine(q, triple) => {
                let (a, fermee) = fin_chaine(s, i, q, triple);
                poser(out, base + i, base + a, JetonCode::Chaine);
                if fermee {
                    st.mode = Mode::Normal;
                }
                i = a;
                continue;
            }
            Mode::Balise | Mode::Normal => {}
        }
        let Some(c) = s[i..].chars().next() else {
            break;
        };
        let reste = &s[i..];
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        if l.ligne.iter().any(|p| reste.starts_with(p)) {
            poser(out, base + i, base + s.len(), JetonCode::Commentaire);
            return;
        }
        if let Some((ouvre, _)) = l.bloc
            && reste.starts_with(ouvre)
        {
            st.mode = Mode::Bloc;
            let apres = i + ouvre.len();
            poser(out, base + i, base + apres, JetonCode::Commentaire);
            i = apres;
            continue;
        }
        if c.is_ascii() && (l.chaines.contains(c) || (l.apostrophe_rust && c == '\'')) {
            let q = c as u8;
            if l.apostrophe_rust && q == b'\'' {
                i = apostrophe_rust(s, i, base, out);
                tete = false;
                continue;
            }
            let triple = l.triple && b.get(i + 1) == Some(&q) && b.get(i + 2) == Some(&q);
            let ouvre = if triple { 3 } else { 1 };
            let (a, fermee) = fin_chaine(s, i + ouvre, q, triple);
            let jeton =
                if l.famille == Famille::Json && fermee && prochain_non_blanc(&s[a..]) == Some(':')
                {
                    JetonCode::Propriete
                } else {
                    JetonCode::Chaine
                };
            poser(out, base + i, base + a, jeton);
            if !fermee && (triple || l.multiligne.contains(c)) {
                st.mode = Mode::Chaine(q, triple);
            }
            i = a;
            tete = false;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let mut a = i + 1;
            while a < s.len() {
                let x = b[a];
                let point_decimal = x == b'.' && b.get(a + 1).is_some_and(u8::is_ascii_digit);
                if x.is_ascii_alphanumeric() || x == b'_' || point_decimal {
                    a += 1;
                } else {
                    break;
                }
            }
            poser(out, base + i, base + a, JetonCode::Type);
            i = a;
            tete = false;
            continue;
        }
        if l.famille == Famille::Css {
            i = css(l, s, i, base, st, out);
            continue;
        }
        if l.directive && c == '#' && tete {
            let a = s[i + 1..]
                .find(|x: char| !x.is_alphanumeric())
                .map_or(s.len(), |k| i + 1 + k);
            poser(out, base + i, base + a, JetonCode::MotCle);
            i = a;
            tete = false;
            continue;
        }
        if let Some(sigil) = l.variable
            && c == sigil
        {
            let a = i
                + 1
                + s[i + 1..]
                    .find(|x: char| !(x.is_alphanumeric() || x == '_'))
                    .unwrap_or(s.len() - i - 1);
            if a > i + 1 {
                poser(out, base + i, base + a, JetonCode::Valeur);
                i = a;
                tete = false;
                continue;
            }
        }
        if debut_ident(l, c) {
            let a = s[i..]
                .find(|x: char| !est_ident(l, x))
                .map_or(s.len(), |k| i + k);
            let mot = &s[i..a];
            let suite = &s[a..];
            let jeton = if dans(l.mots, mot, l.insensible) {
                Some(JetonCode::MotCle)
            } else if dans(l.valeurs, mot, l.insensible) {
                Some(JetonCode::Valeur)
            } else if dans(l.types, mot, l.insensible) {
                Some(JetonCode::Type)
            } else if l.macros && suite.starts_with('!') && !suite.starts_with("!=") {
                Some(JetonCode::Propriete)
            } else if suite.starts_with('(') {
                Some(JetonCode::Fonction)
            } else {
                None
            };
            if let Some(j) = jeton {
                poser(out, base + i, base + a, j);
            }
            i = a;
            tete = false;
            continue;
        }
        let jeton = match c {
            '{' | '}' => Some(JetonCode::Ponctuation),
            '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?' => {
                Some(JetonCode::Operateur)
            }
            _ => None,
        };
        if let Some(j) = jeton {
            poser(out, base + i, base + i + c.len_utf8(), j);
        }
        i += c.len_utf8();
        tete = false;
    }
    // Une chaîne simple non fermée s'arrête à la fin de la ligne (l'état Chaine n'est posé que pour
    // les chaînes multilignes).
}

/// Rust : `'a'` et `'\n'` sont des caractères, `'a` une durée de vie. Rend l'indice qui suit.
fn apostrophe_rust(
    s: &str,
    i: usize,
    base: usize,
    out: &mut Vec<(usize, usize, JetonCode)>,
) -> usize {
    let b = s.as_bytes();
    let reste = &s[i + 1..];
    let mut chars = reste.char_indices();
    let litteral = match chars.next() {
        Some((_, '\\')) => reste[1..].find('\'').map(|k| i + 1 + 1 + k + 1),
        Some((_, c)) if c != '\'' => {
            let fin = i + 1 + c.len_utf8();
            (b.get(fin) == Some(&b'\'')).then_some(fin + 1)
        }
        _ => None,
    };
    if let Some(a) = litteral {
        poser(out, base + i, base + a, JetonCode::Chaine);
        return a;
    }
    let a = reste
        .find(|x: char| !(x.is_alphanumeric() || x == '_'))
        .map_or(s.len(), |k| i + 1 + k);
    poser(out, base + i, base + a.max(i + 1), JetonCode::Type);
    a.max(i + 1)
}

/// YAML et TOML : une clé en début de ligne (`  - clé: valeur`, `clé = valeur`) ou, pour TOML, un
/// en-tête de table `[table]`. Rend l'indice où reprend l'analyse (0 si rien de reconnu).
fn cle_en_tete(s: &str, sep: char, base: usize, out: &mut Vec<(usize, usize, JetonCode)>) -> usize {
    let indent = s.len() - s.trim_start().len();
    let mut debut = indent;
    if sep == '=' && s[indent..].starts_with('[') {
        let fin = s.trim_end().len();
        if s[indent..fin].ends_with(']') {
            poser(out, base + indent, base + fin, JetonCode::Type);
            return fin;
        }
        return 0;
    }
    if sep == ':' {
        while s[debut..].starts_with("- ") {
            debut += 2;
            debut += s[debut..].len() - s[debut..].trim_start().len();
        }
    }
    let reste = &s[debut..];
    let Some(k) = reste.find(sep) else {
        return 0;
    };
    let cle = reste[..k].trim_end();
    let apres_sep = &reste[k + sep.len_utf8()..];
    let valide_cle = !cle.is_empty()
        && !cle.starts_with('#')
        && cle
            .chars()
            .all(|c| c.is_alphanumeric() || "_-. \"'".contains(c));
    let valide_sep = sep == '=' || apres_sep.is_empty() || apres_sep.starts_with([' ', '\t']);
    if valide_cle && valide_sep {
        poser(
            out,
            base + debut,
            base + debut + cle.len(),
            JetonCode::Propriete,
        );
        return debut + k;
    }
    0
}

/// CSS : sélecteurs hors accolades, propriétés et valeurs dedans. Rend l'indice qui suit.
fn css(
    l: &Langue,
    s: &str,
    i: usize,
    base: usize,
    st: &mut Etat,
    out: &mut Vec<(usize, usize, JetonCode)>,
) -> usize {
    let Some(c) = s[i..].chars().next() else {
        return s.len();
    };
    let fin_ident = |de: usize| {
        s[de..]
            .find(|x: char| !est_ident(l, x))
            .map_or(s.len(), |k| de + k)
    };
    match c {
        '{' => {
            st.profondeur += 1;
            poser(out, base + i, base + i + 1, JetonCode::Ponctuation);
            i + 1
        }
        '}' => {
            st.profondeur = st.profondeur.saturating_sub(1);
            poser(out, base + i, base + i + 1, JetonCode::Ponctuation);
            i + 1
        }
        '@' => {
            let a = fin_ident(i + 1);
            poser(out, base + i, base + a, JetonCode::MotCle);
            a
        }
        '!' => {
            let a = fin_ident(i + 1);
            poser(out, base + i, base + a, JetonCode::Important);
            a
        }
        '#' if st.profondeur > 0 => {
            let a = fin_ident(i + 1);
            poser(out, base + i, base + a, JetonCode::Valeur);
            a
        }
        '.' | '#' => {
            let a = fin_ident(i + 1);
            poser(out, base + i, base + a, JetonCode::Type);
            a
        }
        c if c.is_alphabetic() || c == '-' || c == '_' => {
            let a = fin_ident(i);
            let suite = &s[a..];
            let jeton = if suite.starts_with('(') {
                Some(JetonCode::Fonction)
            } else if st.profondeur > 0 {
                (prochain_non_blanc(suite) == Some(':')).then_some(JetonCode::Propriete)
            } else {
                Some(JetonCode::Type)
            };
            if let Some(j) = jeton {
                poser(out, base + i, base + a, j);
            }
            a
        }
        _ => i + c.len_utf8(),
    }
}

/// HTML et XML : balises, attributs, commentaires `<!-- -->`.
fn colorer_balises(s: &str, base: usize, st: &mut Etat, out: &mut Vec<(usize, usize, JetonCode)>) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        match st.mode {
            Mode::Bloc => {
                let a = s[i..].find("-->").map_or(s.len(), |k| i + k + 3);
                poser(out, base + i, base + a, JetonCode::Commentaire);
                if s[i..].contains("-->") {
                    st.mode = Mode::Normal;
                }
                i = a;
            }
            Mode::Balise => {
                let Some(c) = s[i..].chars().next() else {
                    break;
                };
                if c == '>' {
                    st.mode = Mode::Normal;
                    i += 1;
                } else if c == '"' || c == '\'' {
                    let (a, _) = fin_chaine(s, i + 1, c as u8, false);
                    poser(out, base + i, base + a, JetonCode::Chaine);
                    i = a;
                } else if c.is_alphabetic() || c == '_' {
                    let a = s[i..]
                        .find(|x: char| !(x.is_alphanumeric() || "_-:.".contains(x)))
                        .map_or(s.len(), |k| i + k);
                    poser(out, base + i, base + a, JetonCode::Propriete);
                    i = a;
                } else {
                    i += c.len_utf8();
                }
            }
            _ => {
                let Some(k) = s[i..].find('<') else {
                    break;
                };
                i += k;
                if s[i..].starts_with("<!--") {
                    st.mode = Mode::Bloc;
                    poser(out, base + i, base + i + 4, JetonCode::Commentaire);
                    i += 4;
                    continue;
                }
                let nom = i + 1 + usize::from(b.get(i + 1) == Some(&b'/'));
                let a = s[nom..]
                    .find(|x: char| !(x.is_alphanumeric() || "_-:.".contains(x)))
                    .map_or(s.len(), |k| nom + k);
                if a > nom {
                    poser(out, base + nom, base + a, JetonCode::Type);
                    st.mode = Mode::Balise;
                }
                i = a.max(i + 1);
            }
        }
    }
}
