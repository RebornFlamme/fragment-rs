// Port de app/src/ui/views/chat/scroll/searchModel.ts
//
// La recherche dans une conversation (Ctrl+F) — PURE, testée.
// Insensible à la casse et aux accents, SANS changer les longueurs : chaque
// caractère est replié seul (é → e), les offsets restent ceux du texte.
//
// ★ Les offsets (`from`, `to`) sont des index en unités de code UTF-16 (ADR-004), comme
//   les positions de `rangeHighlights` auxquelles ils servent.

use unicode_normalization::UnicodeNormalization;

/// Un caractère replié : minuscule, sans diacritique (garde la longueur).
fn plier(c: char) -> String {
    let d: Vec<char> = c.to_string().nfd().collect();
    // `d.length > 1 ? d[0] : c` : on garde la lettre de base d'une décomposition.
    let base = if d.len() > 1 { d[0] } else { c };
    base.to_lowercase().collect()
}

/// Casse et accents repliés. Les caractères hors du plan de base (2 unités UTF-16) sont
/// laissés tels quels, comme `c.length === 1 ? plier(c) : c` en TS.
pub fn fold_text(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    for c in s.chars() {
        if c.len_utf16() == 1 {
            r.push_str(&plier(c));
        } else {
            r.push(c);
        }
    }
    r
}

/// Une plage `[from, to)` en index UTF-16 du texte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub from: usize,
    pub to: usize,
}

/// Les occurrences de `query` dans `text` (au plus `max`, 1000 par défaut côté appelant).
pub fn find_all(text: &str, query: &str, max: usize) -> Vec<Occurrence> {
    let q: Vec<u16> = fold_text(query.trim()).encode_utf16().collect();
    if q.is_empty() {
        return Vec::new();
    }
    let t: Vec<u16> = fold_text(text).encode_utf16().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while out.len() < max {
        let Some(k) = t
            .get(i..)
            .and_then(|reste| reste.windows(q.len()).position(|w| w == q.as_slice()))
        else {
            break;
        };
        let from = i + k;
        out.push(Occurrence {
            from,
            to: from + q.len(),
        });
        i = from + q.len().max(1);
    }
    out
}

/// L'occurrence suivante (cyclique). `current` : l'occurrence courante, `None` si aucune ;
/// `forward` : sens (`dir > 0`). `None` en sortie : pas d'occurrence (`-1` en TS).
pub fn step(count: usize, current: Option<usize>, forward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let courant = current.unwrap_or(if forward { count - 1 } else { 0 });
    // `((courant + dir) + count) % count`, sans passer par un entier négatif.
    Some(if forward {
        (courant + 1) % count
    } else {
        (courant + count - 1) % count
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replie_casse_et_accents_sans_changer_la_longueur() {
        assert_eq!(fold_text("Élève À"), "eleve a");
        assert_eq!(fold_text("Élève").encode_utf16().count(), 5);
    }

    #[test]
    fn trouve_toutes_les_occurrences_offsets_du_texte_d_origine() {
        let o = |from, to| Occurrence { from, to };
        assert_eq!(
            find_all("Le café, un CAFE, des cafés.", "cafe", 1000),
            vec![o(3, 7), o(12, 16), o(22, 26)]
        );
        assert_eq!(find_all("abc", "  ", 1000), vec![]);
    }

    #[test]
    fn suivant_precedent_cyclique() {
        assert_eq!(step(3, None, true), Some(0));
        assert_eq!(step(3, Some(2), true), Some(0));
        assert_eq!(step(3, Some(0), false), Some(2));
        assert_eq!(step(0, Some(0), true), None);
    }
}
