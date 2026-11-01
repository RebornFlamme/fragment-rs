// Port de app/src/ui/views/pdf/pdfFindOffsets.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  La recherche est faite par le moteur ; ce module ne fait qu'une chose :
//  ramener SES offsets dans NOTRE espace.
//
//  ★ DEUX TEXTES pour une même page :
//
//    - le contrôleur de recherche extrait le texte BRUT : ses correspondances
//      sont des offsets dans la concaténation des `str` bruts des items ;
//    - notre couche de texte (et donc `PdfEditor`, la sélection, les ancres
//      d'annotations PERSISTÉES) vit dans le texte NORMALISÉ (`normalizeUnicode` :
//      ligatures « ﬁ » → « fi », espaces insécables…).
//
//    On ne bascule pas la couche en brut : les annotations déjà posées sur une
//    page à ligatures glisseraient. On convertit, item par item — le worker
//    normalise CHAQUE item séparément, donc les deux listes ont les mêmes items,
//    et l'offset normalisé d'un préfixe brut est la longueur de ce préfixe
//    normalisé.
//
//  ★ UNITÉS : tous les offsets sont en UNITÉS UTF-16, comme les `string.length`
//    du TS — c'est l'espace des ancres d'annotations déjà persistées (sinon un
//    texte avec émoji ou caractère hors plan de base glisserait).
// ═══════════════════════════════════════════════════════════════════════════

/// Une correspondance dans notre espace : [from, to[ dans le texte de la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plage {
    pub from: usize,
    pub to: usize,
}

/// Longueur d'une chaîne en unités UTF-16 (le `.length` de JavaScript).
fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Le préfixe d'une chaîne, en unités UTF-16 (coupe à `n` unités).
fn prefixe16(s: &str, n: usize) -> String {
    let unites: Vec<u16> = s.encode_utf16().take(n).collect();
    String::from_utf16_lossy(&unites)
}

/// (index, longueur) du contrôleur → plage dans notre espace, ou `None` si hors texte.
///
/// Se construit avec [`creer_convertisseur`].
pub struct Convertisseur<'a> {
    bruts: Vec<&'a str>,
    normalises: Vec<&'a str>,
    longueurs_brutes: Vec<usize>,
    longueurs_norm: Vec<usize>,
    /// Débuts cumulés des items dans chacun des deux espaces.
    debuts_bruts: Vec<usize>,
    debuts_norm: Vec<usize>,
    total_brut: usize,
    normaliser: Box<dyn Fn(&str) -> String + 'a>,
}

/// Prépare la conversion pour une page.
///
/// - `bruts` : les `str` des items, texte brut (celui du contrôleur)
/// - `normalises` : les `str` des mêmes items, texte normalisé (celui de la couche)
/// - `normaliser` : la normalisation du worker — `normalizeUnicode` de pdf.js
///
/// Rend `None` si les deux listes ne se correspondent pas (on ne devine pas).
pub fn creer_convertisseur<'a>(
    bruts: &[&'a str],
    normalises: &[&'a str],
    normaliser: impl Fn(&str) -> String + 'a,
) -> Option<Convertisseur<'a>> {
    if bruts.len() != normalises.len() {
        return None;
    }
    let longueurs_brutes: Vec<usize> = bruts.iter().map(|s| len16(s)).collect();
    let longueurs_norm: Vec<usize> = normalises.iter().map(|s| len16(s)).collect();
    let mut debuts_bruts = Vec::with_capacity(bruts.len());
    let mut debuts_norm = Vec::with_capacity(bruts.len());
    let mut b = 0;
    let mut n = 0;
    for i in 0..bruts.len() {
        debuts_bruts.push(b);
        debuts_norm.push(n);
        b += longueurs_brutes[i];
        n += longueurs_norm[i];
    }
    Some(Convertisseur {
        bruts: bruts.to_vec(),
        normalises: normalises.to_vec(),
        longueurs_brutes,
        longueurs_norm,
        debuts_bruts,
        debuts_norm,
        total_brut: b,
        normaliser: Box::new(normaliser),
    })
}

impl Convertisseur<'_> {
    /// Un offset brut → normalisé. `fin` départage une frontière d'items : un
    /// début de correspondance appartient à l'item qui COMMENCE là, une fin à
    /// celui qui s'y TERMINE.
    fn convertir(&self, p: i64, fin: bool) -> Option<usize> {
        if p < 0 || p as usize > self.total_brut {
            return None;
        }
        let p = p as usize;
        let mut i = 0;
        let n = self.bruts.len();
        for k in 0..n {
            let d = self.debuts_bruts[k];
            let f = d + self.longueurs_brutes[k];
            let dedans = if fin {
                p > d && p <= f
            } else {
                p >= d && p < f
            };
            if dedans {
                i = k;
                break;
            }
            // ★ Aucun item ne contient `p` (frontière de fin, ou items vides) :
            //   repli sur le dernier, comme le TS.
            if k == n - 1 {
                i = k;
            }
        }
        if n == 0 {
            return None;
        }
        let local = p - self.debuts_bruts[i];
        let brut = self.bruts[i];
        // Cas écrasant : l'item n'a rien à normaliser, offsets identiques.
        if brut == self.normalises[i] {
            return Some(self.debuts_norm[i] + local);
        }
        if local >= self.longueurs_brutes[i] {
            return Some(self.debuts_norm[i] + self.longueurs_norm[i]);
        }
        Some(self.debuts_norm[i] + len16(&(self.normaliser)(&prefixe16(brut, local))))
    }

    /// Convertit une correspondance `(index, longueur)` du contrôleur.
    pub fn convertir_plage(&self, index: i64, longueur: i64) -> Option<Plage> {
        let from = self.convertir(index, false)?;
        let to = self.convertir(index + longueur, true)?;
        if to <= from {
            return None;
        }
        Some(Plage { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La normalisation du worker, réduite à ce que les tests utilisent (ligatures ﬀ..ﬆ).
    fn nfkc(s: &str) -> String {
        s.chars()
            .flat_map(|c| match c {
                '\u{FB00}' => "ff".chars().collect::<Vec<_>>(),
                '\u{FB01}' => "fi".chars().collect(),
                '\u{FB02}' => "fl".chars().collect(),
                '\u{FB03}' => "ffi".chars().collect(),
                '\u{FB04}' => "ffl".chars().collect(),
                '\u{FB05}' | '\u{FB06}' => "st".chars().collect(),
                autre => vec![autre],
            })
            .collect()
    }

    /// Longueur en unités UTF-16 de la partie de `s` avant l'octet `o`.
    fn index16(s: &str, mot: &str) -> i64 {
        let o = s.find(mot).unwrap_or(0);
        len16(&s[..o]) as i64
    }

    /// Cherche `mot` dans le texte BRUT (comme le contrôleur) et rend le texte couvert dans le NORMALISÉ.
    fn couvert(bruts: &[&str], mot: &str) -> Option<String> {
        let normalises_owned: Vec<String> = bruts.iter().map(|s| nfkc(s)).collect();
        let normalises: Vec<&str> = normalises_owned.iter().map(String::as_str).collect();
        let conv = creer_convertisseur(bruts, &normalises, nfkc)?;
        let index = index16(&bruts.concat(), mot);
        let plage = conv.convertir_plage(index, len16(mot) as i64)?;
        let tout: Vec<u16> = normalises.concat().encode_utf16().collect();
        Some(String::from_utf16_lossy(&tout[plage.from..plage.to]))
    }

    #[test]
    fn creer_convertisseur_identite_quand_rien_n_est_normalise() {
        assert_eq!(
            couvert(&["Volume estimates", " for unions"], "estimates").as_deref(),
            Some("estimates")
        );
        assert_eq!(
            couvert(&["Volume estimates", " for unions"], "unions").as_deref(),
            Some("unions")
        );
    }

    #[test]
    fn creer_convertisseur_une_ligature_avant_le_mot_decale_la_suite() {
        // « ﬁ » (1 unité) devient « fi » (2) dans la couche.
        assert_eq!(
            couvert(&["la \u{FB01}n du volume"], "volume").as_deref(),
            Some("volume")
        );
    }

    #[test]
    fn creer_convertisseur_un_mot_qui_contient_une_ligature_la_couvre_entiere() {
        assert_eq!(
            couvert(&["une \u{FB01}gure"], "\u{FB01}gure").as_deref(),
            Some("figure")
        );
    }

    #[test]
    fn creer_convertisseur_une_correspondance_a_cheval_sur_deux_items() {
        assert_eq!(
            couvert(&["moment ", "cinétique"], "moment cin").as_deref(),
            Some("moment cin")
        );
    }

    #[test]
    fn creer_convertisseur_une_fin_pile_en_bout_d_item_reste_dans_cet_item() {
        assert_eq!(couvert(&["spin", " seul"], "spin").as_deref(), Some("spin"));
    }

    #[test]
    fn creer_convertisseur_refuse_des_listes_d_items_qui_ne_se_correspondent_pas() {
        assert!(creer_convertisseur(&["a", "b"], &["ab"], nfkc).is_none());
    }

    #[test]
    fn creer_convertisseur_null_hors_du_texte() {
        let conv = creer_convertisseur(&["abc"], &["abc"], nfkc).expect("listes alignées");
        assert_eq!(conv.convertir_plage(5, 2), None);
        assert_eq!(conv.convertir_plage(1, 0), None);
    }
}
