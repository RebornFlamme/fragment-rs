// Port de app/src/ui/core/fuzzySearch.ts
//! §14 — LA RECHERCHE FLOUE
//!
//! Le noyau que le quick switcher et la palette de commandes partagent : une
//! requête, un texte, un score et les plages qui ont matché. Rien de plus.
//!
//! ★ L'API est en DEUX TEMPS — `prepare_fuzzy_search(query)` rend un chercheur
//!   qu'on applique ensuite à chaque texte. C'est ce qui fait que le pliage de
//!   la requête (minuscules, accents) n'est calculé qu'UNE fois pour cent
//!   candidats, pas cent fois.
//!
//! L'algorithme est une sous-séquence gloutonne : chaque caractère de la
//! requête doit apparaître dans le texte, dans l'ordre, pas forcément
//! contigus. « tb » trouve « Toggle Bold ». Le score départage : un match en
//! début de mot vaut plus, un match collé au précédent aussi, un match loin du
//! début vaut moins. Plus le score est ÉLEVÉ, meilleur est le match — 0 n'est
//! pas une borne.
//!
//! ★ Unité des plages : des index de CARACTÈRES (scalaires Unicode) dans le texte
//!   d'origine, là où le TS comptait des unités UTF-16. Identiques sur le plan
//!   multilingue de base (toute la palette) ; ils divergent seulement après un
//!   caractère hors BMP (emoji), que le TS comptait double. `render_matches`
//!   (présentation DOM) n'est pas porté : le surlignage est l'affaire de l'UI.

use unicode_normalization::UnicodeNormalization;

/// Les plages `[from, to)` du texte qui ont matché, fusionnées si adjacentes.
pub type SearchMatches = Vec<(usize, usize)>;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub score: f64,
    pub matches: SearchMatches,
}

/// Ce que rend une FuzzySuggestModal : l'élément et son match.
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyMatch<T> {
    pub item: T,
    pub r#match: SearchResult,
}

/// Un signe diacritique combinant (le bloc U+0300–U+036F que le TS retirait).
fn est_diacritique(c: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&c)
}

/// Plie une chaîne pour la comparaison : minuscules, sans accents.
///
/// ★ La longueur est PRÉSERVÉE caractère par caractère : les plages de match
///   sont des index dans le texte d'ORIGINE, et c'est là qu'on surligne. Un
///   pliage qui changerait la longueur (« ﬁ » → « fi ») décalerait tous les
///   surlignages qui suivent. On normalise donc caractère par caractère, et on
///   garde l'original si le pliage n'a pas la même taille.
fn plier(s: &str) -> Vec<char> {
    let mut sortie = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let bas: Vec<char> = ch.to_lowercase().collect();
        let nu: Vec<char> = bas
            .iter()
            .collect::<String>()
            .nfd()
            .filter(|c| !est_diacritique(*c))
            .collect();
        match (nu.first(), bas.first()) {
            (Some(&n), _) if nu.len() == 1 => sortie.push(n),
            (_, Some(&b)) if bas.len() == 1 => sortie.push(b),
            _ => sortie.push(ch),
        }
    }
    sortie
}

/// Un début de mot : premier caractère, ou précédé d'un séparateur.
fn debut_de_mot(texte: &[char], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let p = texte[i - 1];
    p.is_whitespace() || matches!(p, '-' | '_' | ':' | '.' | '/' | '(' | ')' | '[' | ']')
}

/// Le chercheur préparé pour une requête : `search(texte)` rend son score et
/// ses plages — ou `None` si la requête n'y est pas. Une requête vide matche
/// TOUT, avec un score nul et aucune plage : c'est ce qu'une palette affiche
/// avant la première frappe.
#[derive(Debug, Clone)]
pub struct FuzzySearch {
    requete: Vec<char>,
}

impl FuzzySearch {
    pub fn search(&self, text: &str) -> Option<SearchResult> {
        let q = &self.requete;
        if q.is_empty() {
            return Some(SearchResult {
                score: 0.0,
                matches: Vec::new(),
            });
        }
        let t = plier(text);

        // ★ On essaie CHAQUE point de départ possible et on garde le meilleur.
        //   Une seule passe gloutonne prendrait le premier « e » venu — celui
        //   d'« Éditeur » — et raterait le bloc contigu « Écrire » juste après.
        //   Le coût est borné par le nombre d'occurrences du premier caractère,
        //   dérisoire à l'échelle d'une palette.
        let mut meilleur: Option<SearchResult> = None;
        for depart in t
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == q[0])
            .map(|(i, _)| i)
        {
            // Si l'alignement échoue d'ici, il échouera de tout point plus
            // loin : il reste strictement moins de texte pour le même reste.
            let Some(r) = aligner_depuis(&t, q, depart) else {
                break;
            };
            if meilleur.as_ref().is_none_or(|m| r.score > m.score) {
                meilleur = Some(r);
            }
        }
        meilleur
    }
}

/// Prépare la requête (pliée une fois) ; le chercheur rendu s'applique à chaque texte.
pub fn prepare_fuzzy_search(query: &str) -> FuzzySearch {
    FuzzySearch {
        requete: plier(query.trim()),
    }
}

/// L'alignement glouton de `q` dans `t`, le premier caractère fixé à `depart`.
fn aligner_depuis(t: &[char], q: &[char], depart: usize) -> Option<SearchResult> {
    let mut matches: SearchMatches = Vec::new();
    let mut score = 0.0;
    let mut qi = 0;
    let mut dernier: Option<usize> = None;

    for i in depart..t.len() {
        if qi >= q.len() {
            break;
        }
        if t[i] != q[qi] {
            continue;
        }

        if let Some(d) = dernier {
            // Collé au précédent : bonus. Sinon, l'écart coûte un peu.
            if i == d + 1 {
                score += 1.0;
            } else {
                score -= (i - d - 1) as f64 * 0.1;
            }
        }
        if debut_de_mot(t, i) {
            score += 2.0;
        }

        match matches.last_mut() {
            Some(last) if last.1 == i => last.1 = i + 1,
            _ => matches.push((i, i + 1)),
        }

        dernier = Some(i);
        qi += 1;
    }

    if qi < q.len() {
        return None;
    }

    // Un match qui commence tard vaut moins : « bold » doit sortir
    // « Toggle bold » avant « Insert a bold callout ».
    score -= matches.first().map_or(0.0, |m| m.0 as f64) * 0.05;
    Some(SearchResult { score, matches })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_requete_vide_matche_tout_sans_plage() {
        let chercher = prepare_fuzzy_search("");
        assert_eq!(
            chercher.search("N’importe quoi"),
            Some(SearchResult {
                score: 0.0,
                matches: vec![]
            })
        );
    }

    #[test]
    fn trouve_une_sous_sequence_tb_dans_toggle_bold() {
        let r = prepare_fuzzy_search("tb").search("Toggle Bold").unwrap();
        assert_eq!(r.matches, vec![(0, 1), (7, 8)]);
    }

    #[test]
    fn rend_null_quand_un_caractere_manque() {
        assert_eq!(prepare_fuzzy_search("tbz").search("Toggle Bold"), None);
    }

    #[test]
    fn ignore_la_casse_et_les_accents_sans_decaler_les_plages() {
        let r = prepare_fuzzy_search("ecr")
            .search("Éditeur : Écrire")
            .unwrap();
        // « Écr » commence à l'index 10 dans le texte d'ORIGINE.
        assert_eq!(r.matches[0], (10, 13));
    }

    #[test]
    fn fusionne_les_plages_adjacentes() {
        let r = prepare_fuzzy_search("bold").search("Toggle bold").unwrap();
        assert_eq!(r.matches, vec![(7, 11)]);
    }

    #[test]
    fn prefere_un_match_en_debut_de_mot_et_contigu() {
        let chercher = prepare_fuzzy_search("bold");
        let direct = chercher.search("Toggle bold").unwrap().score;
        let tardif = chercher.search("Insert a callout in bold").unwrap().score;
        let eparpille = chercher.search("brought old").unwrap().score; // b…old, pas contigu
        assert!(direct > tardif);
        assert!(direct > eparpille);
    }
}
