// Port de app/src/ui/views/doc-widget/clusterFilter.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Recherche et filtre sur la timeline d'une grappe (pur, testé). La grappe ignore le
//  chat et ne connaît d'un post que le titre du membre, le texte de l'aperçu (le tweet
//  rendu) et les attributs qu'il expose (`data-at`, `data-replies`). À défaut de ces
//  données, le post reste à sa place et passe le filtre « Tous ».
// ═══════════════════════════════════════════════════════════════════════════

use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterSort {
    Text,
    Recent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterShow {
    All,
    Replied,
    Unreplied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterFilter {
    pub query: String,
    pub sort: ClusterSort,
    pub show: ClusterShow,
}

impl Default for ClusterFilter {
    /// `FILTRE_DEFAUT`.
    fn default() -> Self {
        Self {
            query: String::new(),
            sort: ClusterSort::Text,
            show: ClusterShow::All,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClusterEntry {
    /// Sa place dans l'ordre du texte.
    pub order: usize,
    pub label: String,
    pub text: String,
    pub at: Option<f64>,
    pub replies: Option<u32>,
}

/// Ce que [`filtrer`] sait lire d'un post (le `T extends ClusterEntry` du TS : un post
/// porte en plus ce que veut l'hôte).
pub trait Entree {
    fn entry(&self) -> &ClusterEntry;
}

impl Entree for ClusterEntry {
    fn entry(&self) -> &ClusterEntry {
        self
    }
}

/// Les blancs de `\s` de JS.
fn est_blanc(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// Minuscules, sans accents, blancs repliés.
///
/// ★ Même ordre que le TS : NFD, retrait des marques U+0300–U+036F, puis minuscules.
pub fn normaliser(s: &str) -> String {
    let sans_accents: String = s
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect::<String>()
        .to_lowercase();
    let mut out = String::new();
    let mut dans_blanc = false;
    for c in sans_accents.chars() {
        if est_blanc(c) {
            dans_blanc = true;
        } else {
            if dans_blanc && !out.is_empty() {
                out.push(' ');
            }
            dans_blanc = false;
            out.push(c);
        }
    }
    out
}

/// Tous les mots de la requête se trouvent dans le titre ou le texte.
pub fn correspond(e: &ClusterEntry, query: &str) -> bool {
    let q = normaliser(query);
    if q.is_empty() {
        return true;
    }
    let foin = normaliser(&format!("{} {}", e.label, e.text));
    q.split(' ').all(|mot| foin.contains(mot))
}

/// Les posts à montrer, dans l'ordre à montrer.
pub fn filtrer<T: Entree + Clone>(entries: &[T], f: &ClusterFilter) -> Vec<T> {
    let mut gardes: Vec<T> = entries
        .iter()
        .filter(|t| {
            let e = t.entry();
            if !correspond(e, &f.query) {
                return false;
            }
            match f.show {
                ClusterShow::Replied => e.replies.is_none_or(|r| r > 0),
                ClusterShow::Unreplied => e.replies.is_none_or(|r| r == 0),
                ClusterShow::All => true,
            }
        })
        .cloned()
        .collect();
    match f.sort {
        // Les plus récents d'abord ; sans heure, à la fin, dans l'ordre du texte.
        ClusterSort::Recent => gardes.sort_by(|a, b| {
            let (a, b) = (a.entry(), b.entry());
            let d = b.at.unwrap_or(f64::NEG_INFINITY) - a.at.unwrap_or(f64::NEG_INFINITY);
            // ★ `-∞ - -∞` vaut NaN (faux en JS) : on retombe alors sur l'ordre du texte.
            if d == 0.0 || d.is_nan() {
                a.order.cmp(&b.order)
            } else if d < 0.0 {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }),
        ClusterSort::Text => gardes.sort_by_key(|t| t.entry().order),
    }
    gardes
}

/// Un filtre autre que celui par défaut (le bouton s'allume).
pub fn filtre_actif(f: &ClusterFilter) -> bool {
    let d = ClusterFilter::default();
    f.sort != d.sort || f.show != d.show
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(
        order: usize,
        label: &str,
        text: &str,
        at: Option<f64>,
        replies: Option<u32>,
    ) -> ClusterEntry {
        ClusterEntry {
            order,
            label: label.into(),
            text: text.into(),
            at,
            replies,
        }
    }

    fn liste() -> Vec<ClusterEntry> {
        vec![
            e(
                0,
                "Ask « vitesse »",
                "Pourquoi la lumière ?",
                Some(100.0),
                Some(2),
            ),
            e(1, "Ask « Éditeur »", "Les fragments", Some(300.0), Some(0)),
            e(2, "Une note", "", None, None),
        ]
    }

    fn ordres(v: Vec<ClusterEntry>) -> Vec<usize> {
        v.iter().map(|x| x.order).collect()
    }

    fn filtre(f: impl FnOnce(&mut ClusterFilter)) -> ClusterFilter {
        let mut d = ClusterFilter::default();
        f(&mut d);
        d
    }

    #[test]
    fn normaliser_minuscules_sans_accents_blancs_replies() {
        assert_eq!(normaliser("  Éditeur   À Faire "), "editeur a faire");
    }

    #[test]
    fn recherche_tous_les_mots_titre_ou_texte_sans_accents() {
        let l = liste();
        assert!(correspond(&l[1], "editeur"));
        assert!(correspond(&l[0], "lumiere pourquoi"));
        assert!(!correspond(&l[0], "lumiere fragments"));
        assert_eq!(
            ordres(filtrer(&l, &filtre(|f| f.query = "  ".into()))),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn plus_recents_d_abord_sans_heure_a_la_fin() {
        assert_eq!(
            ordres(filtrer(&liste(), &filtre(|f| f.sort = ClusterSort::Recent))),
            vec![1, 0, 2]
        );
    }

    #[test]
    fn avec_sans_reponse_un_post_sans_donnee_passe() {
        assert_eq!(
            ordres(filtrer(
                &liste(),
                &filtre(|f| f.show = ClusterShow::Replied)
            )),
            vec![0, 2]
        );
        assert_eq!(
            ordres(filtrer(
                &liste(),
                &filtre(|f| f.show = ClusterShow::Unreplied)
            )),
            vec![1, 2]
        );
    }

    #[test]
    fn filtre_actif_seulement_si_tri_ou_montrer_different_du_defaut() {
        assert!(!filtre_actif(&filtre(|f| f.query = "x".into())));
        assert!(filtre_actif(&filtre(|f| f.sort = ClusterSort::Recent)));
    }
}
