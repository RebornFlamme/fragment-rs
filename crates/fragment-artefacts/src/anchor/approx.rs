// Remplace la bibliothèque `approx-string-match` (robertknight) qu'anchor.ts importe.
//
// ═══════════════════════════════════════════════════════════════════════════
//  Recherche approchée d'un motif dans un texte — distance de Levenshtein, le
//  motif entier contre N'IMPORTE QUELLE sous-chaîne du texte (Sellers 1980).
//
//  Algorithme de Myers (1999) en vecteurs de bits, formulation d'Hyyrö (2003),
//  pour un motif de 64 caractères au plus — le cas des citations (32) ; au-delà,
//  la programmation dynamique colonne par colonne, même résultat (un test de
//  propriété le vérifie).
//
//  Le résultat est celui de la bibliothèque : des correspondances `{start, end,
//  errors}`. ★ Des fins consécutives (le même passage, décalé d'un caractère)
//  sont une seule correspondance, celle qui a le moins d'erreurs ; le début est
//  retrouvé par la même recherche sur le motif et le texte retournés, en prenant
//  le début le plus à gauche (la correspondance la plus longue), comme la
//  bibliothèque. Aucune dépendance : l'algorithme tient en cent lignes.
// ═══════════════════════════════════════════════════════════════════════════

/// Une correspondance : `text[start..end]` ressemble au motif à `errors` près.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub errors: usize,
}

/// Toutes les correspondances du motif dans le texte avec au plus `max_errors`
/// erreurs (insertion, suppression, substitution), de gauche à droite.
pub fn search(text: &[u16], pattern: &[u16], max_errors: usize) -> Vec<Match> {
    if pattern.is_empty() || text.is_empty() {
        return Vec::new();
    }
    let ends = coalesce(match_ends(text, pattern, max_errors));
    let pat_rev: Vec<u16> = pattern.iter().rev().copied().collect();
    ends.into_iter()
        .map(|(end, errors)| {
            // Le début : le motif retourné, cherché dans le texte retourné qui précède
            // la fin ; plusieurs débuts possibles → le plus à gauche.
            let min_start = end.saturating_sub(pattern.len() + errors);
            let window: Vec<u16> = text[min_start..end].iter().rev().copied().collect();
            let start = match_ends(&window, &pat_rev, errors)
                .into_iter()
                .map(|(re, _)| end - re)
                .min()
                .unwrap_or(end);
            Match { start, end, errors }
        })
        .collect()
}

/// Fusionne les fins consécutives : une seule par passage, la meilleure (la
/// première à égalité).
fn coalesce(ends: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    // La série en cours : sa dernière position, et sa meilleure fin.
    let mut run: Option<(usize, (usize, usize))> = None;
    for (end, errors) in ends {
        match run {
            Some((last, best)) if last + 1 == end => {
                run = Some((end, if errors < best.1 { (end, errors) } else { best }));
            }
            Some((_, best)) => {
                out.push(best);
                run = Some((end, (end, errors)));
            }
            None => run = Some((end, (end, errors))),
        }
    }
    if let Some((_, best)) = run {
        out.push(best);
    }
    out
}

/// Les fins `(end exclusif, erreurs)` de toutes les sous-chaînes du texte à
/// `max_errors` près du motif — une par position du texte qui qualifie.
fn match_ends(text: &[u16], pattern: &[u16], max_errors: usize) -> Vec<(usize, usize)> {
    let scores = if pattern.len() <= 64 {
        myers_scores(text, pattern)
    } else {
        dp_scores(text, pattern)
    };
    scores
        .into_iter()
        .enumerate()
        .filter(|(_, d)| *d <= max_errors)
        .map(|(j, d)| (j + 1, d))
        .collect()
}

/// Pour chaque position j du texte, la distance minimale entre le motif et une
/// sous-chaîne qui se termine en j (inclus) — vecteurs de bits, m ≤ 64.
fn myers_scores(text: &[u16], pattern: &[u16]) -> Vec<usize> {
    let m = pattern.len();
    debug_assert!((1..=64).contains(&m));
    let mask: u64 = if m == 64 { u64::MAX } else { (1u64 << m) - 1 };
    let top: u64 = 1u64 << (m - 1);
    let mut peq: std::collections::HashMap<u16, u64> = std::collections::HashMap::new();
    for (i, c) in pattern.iter().enumerate() {
        *peq.entry(*c).or_insert(0) |= 1u64 << i;
    }
    let mut pv: u64 = mask;
    let mut mv: u64 = 0;
    let mut score = m;
    let mut out = Vec::with_capacity(text.len());
    for c in text {
        let eq = peq.get(c).copied().unwrap_or(0);
        let xv = eq | mv;
        let xh = (((eq & pv).wrapping_add(pv)) ^ pv) | eq;
        let ph = mv | (!(xh | pv) & mask);
        let mh = pv & xh;
        if ph & top != 0 {
            score += 1;
        } else if mh & top != 0 {
            score -= 1;
        }
        // Recherche (début libre) : la première ligne reste à 0, le décalage n'injecte rien.
        let ph = (ph << 1) & mask;
        let mh = (mh << 1) & mask;
        pv = mh | (!(xv | ph) & mask);
        mv = ph & xv;
        out.push(score);
    }
    out
}

/// La même chose par programmation dynamique (Sellers), pour les motifs longs et
/// comme oracle du test de propriété.
fn dp_scores(text: &[u16], pattern: &[u16]) -> Vec<usize> {
    let m = pattern.len();
    let mut col: Vec<usize> = (0..=m).collect();
    let mut next = vec![0usize; m + 1];
    let mut out = Vec::with_capacity(text.len());
    for c in text {
        next[0] = 0;
        for i in 1..=m {
            let sub = col[i - 1] + usize::from(pattern[i - 1] != *c);
            next[i] = sub.min(col[i] + 1).min(next[i - 1] + 1);
        }
        out.push(next[m]);
        std::mem::swap(&mut col, &mut next);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn exact_et_approche() {
        let t = chars("Un paragraphe. Le store garde le point d’ancrage du widget. Fin.");
        let p = chars("point d’ancrage");
        let m = search(&t, &p, 0);
        assert_eq!(
            m,
            vec![Match {
                start: 33,
                end: 48,
                errors: 0
            }]
        );
        let p2 = chars("point d'ancrXge");
        let m2 = search(&t, &p2, 3);
        assert_eq!(m2.len(), 1);
        assert_eq!((m2[0].start, m2[0].end, m2[0].errors), (33, 48, 2));
        assert!(search(&t, &chars("zzzzzzzzzz"), 2).is_empty());
        assert!(search(&t, &[], 2).is_empty());
    }

    #[test]
    fn un_motif_long_passe_par_la_programmation_dynamique() {
        let p: Vec<u16> = chars(&"abcdefghij".repeat(7)); // 70 > 64
        let mut t = chars("xxx");
        t.extend_from_slice(&p);
        t.extend(chars("yyy"));
        t[10] = u16::from(b'#');
        let m = search(&t, &p, 5);
        assert_eq!(
            m,
            vec![Match {
                start: 3,
                end: 73,
                errors: 1
            }]
        );
    }

    #[test]
    fn myers_et_dp_donnent_les_memes_distances_graines_1_a_200() {
        let alphabet: Vec<u16> = chars("abcd ");
        for seed in 1..=200u32 {
            let mut rand = crate::fold::tests::prng(seed);
            let n = 1 + (rand() * 120.0).floor() as usize;
            let m = 1 + (rand() * 64.0).floor() as usize;
            let t: Vec<u16> = (0..n)
                .map(|_| alphabet[(rand() * 5.0).floor() as usize])
                .collect();
            let p: Vec<u16> = (0..m)
                .map(|_| alphabet[(rand() * 5.0).floor() as usize])
                .collect();
            assert_eq!(myers_scores(&t, &p), dp_scores(&t, &p), "graine {seed}");
        }
    }
}
