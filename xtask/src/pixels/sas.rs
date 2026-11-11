//! Le sas par catégorie (ADR-009) : strict hors des boîtes de texte et d'icônes, anticrénelage
//! toléré DANS une boîte sous garde-fous.
//!
//! Hors des boîtes (élargies de [`PAD_TOL`] pixel) : 0 pixel de différence. Dans une boîte, quatre
//! garde-fous, mesurés sur la référence ET le candidat :
//! 1. **encre totale** : la couverture cumulée (distance au fond, normalisée par le contraste de
//!    la référence) reste à [`ECART_ENCRE`] près ;
//! 2. **boîte d'encre** : l'origine est strictement la même, le bord droit et le bord bas sont à
//!    ±1 px (la largeur à ±1 px est la dette T1 `fr-t1.3`) ;
//! 3. **forme** : mot par mot, les pixels de cœur d'un trait (couverture ≥ [`COEUR`]) trouvent un trait
//!    (≥ [`PRESENCE`]) au même endroit de l'autre image, au meilleur décalage de ±[`DECALAGE`] px
//!    (±[`DECALAGE_V`] en hauteur) ; la part d'orphelins reste sous [`ECART_FORME`]. Des traits plus
//!    fins ou décalés d'un pixel ne font pas d'orphelin, une lettre changée ou absente en fait ;
//! 4. **teinte** : la couleur de l'encre garde la direction de celle de la référence
//!    (cosinus ≥ [`TEINTE_MIN`]).
//!
//! Une lettre manquante ou changée, une autre graisse ou une autre couleur les font échouer.

use image::{Rgba, RgbaImage};

use super::boites::{BoiteEncre, Categorie, OmbreDeclaree};
use super::ombres::{self, ResultatOmbre};
use super::{Boite, Verdict, zones_connexes};

/// Marge autour d'une boîte d'encre où l'écart est toléré (anticrénelage qui déborde).
pub const PAD_TOL: u32 = 1;
/// Marge autour d'une boîte pour mesurer l'encre.
pub const PAD_MESURE: u32 = 2;
/// Marge autour d'une boîte pour estimer le fond.
pub const PAD_FOND: u32 = 4;
/// Écart relatif maximal d'encre totale.
pub const ECART_ENCRE: f64 = 0.25;
/// Part maximale de traits sans équivalent dans un mot (mesure réelle sur les captures : ≤ 2,3 %).
pub const ECART_FORME: f64 = 0.05;
/// Couverture à partir de laquelle un pixel est du coeur d'un trait.
pub const COEUR: f64 = 0.8;
/// Couverture à partir de laquelle un trait est présent dans le voisinage.
pub const PRESENCE: f64 = 0.2;
/// Rayon du voisinage (px) où chercher le trait de l'autre image.
pub const VOISINAGE: i64 = 0;
/// Décalage horizontal (px) toléré entre deux carreaux comparés.
pub const DECALAGE: i64 = 3;
/// Décalage vertical (px) toléré entre deux carreaux comparés.
pub const DECALAGE_V: i64 = 1;
/// Colonnes vides (au plus) à l'intérieur d'un mot : au-delà, un nouveau mot commence.
pub const ESPACE_MOT: i64 = 2;
/// Cosinus minimal entre les teintes d'encre.
pub const TEINTE_MIN: f64 = 0.995;
/// Couverture à partir de laquelle un pixel fait partie de la boîte d'encre.
pub const SEUIL_ENCRE: f64 = 0.10;
/// Écart maximal (px) d'un bord d'encre autre que l'origine.
pub const ECART_BORD: u32 = 1;

/// Rectangle de pixels, bords inclus : (x0, y0, x1, y1).
type Rect = (u32, u32, u32, u32);

/// La boîte `b` élargie de `pad` et rognée à l'image.
fn rect(b: &BoiteEncre, pad: u32, largeur: u32, hauteur: u32) -> Rect {
    (
        b.x.saturating_sub(pad),
        b.y.saturating_sub(pad),
        (b.x + b.largeur - 1 + pad).min(largeur - 1),
        (b.y + b.hauteur - 1 + pad).min(hauteur - 1),
    )
}

/// Distance L1 sur les canaux de couleur.
fn l1(a: Rgba<u8>, b: Rgba<u8>) -> u32 {
    (0..3).map(|i| u32::from(a.0[i].abs_diff(b.0[i]))).sum()
}

/// La couleur la plus fréquente de `r`.
fn fond(img: &RgbaImage, r: Rect) -> Rgba<u8> {
    let mut compte: std::collections::HashMap<[u8; 4], u32> = std::collections::HashMap::new();
    for y in r.1..=r.3 {
        for x in r.0..=r.2 {
            *compte.entry(img.get_pixel(x, y).0).or_default() += 1;
        }
    }
    compte
        .into_iter()
        .max_by_key(|&(c, n)| (n, c))
        .map_or(Rgba([0; 4]), |(c, _)| Rgba(c))
}

/// Ce que les garde-fous lisent d'une image dans une boîte.
#[derive(Debug, Clone)]
pub struct Mesure {
    /// Couverture cumulée.
    pub encre: f64,
    /// Boîte d'encre (x0, y0, x1, y1), `None` sans encre.
    pub boite: Option<Rect>,
    /// Couverture (0 à 1) de chaque pixel de la région, ligne par ligne, et sa largeur.
    pub couverture: Vec<f64>,
    pub largeur: u32,
    /// Somme des écarts au fond, par canal.
    pub teinte: [f64; 3],
}

/// Mesure `img` dans `r`, avec le fond et le contraste de référence `k` (somme L1 maximale).
fn mesurer(img: &RgbaImage, r: Rect, fond: Rgba<u8>, k: f64) -> Mesure {
    let (l, h) = (r.2 - r.0 + 1, r.3 - r.1 + 1);
    let mut couvertures = Vec::with_capacity((l * h) as usize);
    let mut encre = 0.0;
    let mut boite: Option<Rect> = None;
    let mut teinte = [0.0; 3];
    for y in r.1..=r.3 {
        for x in r.0..=r.2 {
            let p = *img.get_pixel(x, y);
            let couverture = (f64::from(l1(p, fond)) / k).min(1.0);
            encre += couverture;
            couvertures.push(couverture);
            if couverture >= SEUIL_ENCRE {
                boite = Some(match boite {
                    None => (x, y, x, y),
                    Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                });
            }
            if couverture >= 0.15 {
                for (i, t) in teinte.iter_mut().enumerate() {
                    *t += f64::from(p.0[i]) - f64::from(fond.0[i]);
                }
            }
        }
    }
    Mesure {
        encre,
        boite,
        couverture: couvertures,
        largeur: l,
        teinte,
    }
}

/// Garde-fou de forme, mot par mot. Le candidat est découpé en segments (colonnes d'encre séparées
/// de [`ESPACE_MOT`] colonnes vides au moins). Dans un segment, un pixel de COEUR (couverture ≥
/// [`COEUR`]) est orphelin si l'autre image n'a, dans son voisinage de [`VOISINAGE`] px, aucun pixel
/// d'au moins [`PRESENCE`] de couverture : le trait y manque. Un anticrénelage différent, des traits
/// plus fins ou un glyphe décalé d'un pixel ne font pas d'orphelin ; une lettre changée ou absente
/// en fait. L'autre image est décalée de -[`DECALAGE`] à +[`DECALAGE`] px en x (la dette de largeur
/// décale les mots d'un pixel ou deux le long d'une ligne) et de ±[`DECALAGE_V`] px en y, au meilleur
/// décalage. Rend la part d'orphelins du pire segment (orphelins des deux sens / cœurs du candidat).
fn ecart_forme(rf: &Mesure, cd: &Mesure) -> f64 {
    let l = cd.largeur as i64;
    let h = cd.couverture.len() as i64 / l.max(1);
    let lire = |m: &Mesure, x: i64, y: i64| -> f64 {
        if x < 0 || x >= l || y < 0 || y >= h {
            0.0
        } else {
            m.couverture[(y * l + x) as usize]
        }
    };
    let voisin_max = |m: &Mesure, x: i64, y: i64| -> f64 {
        (-VOISINAGE..=VOISINAGE)
            .flat_map(|u| (-VOISINAGE..=VOISINAGE).map(move |v| (u, v)))
            .map(|(u, v)| lire(m, x + u, y + v))
            .fold(0.0, f64::max)
    };
    let encre_col = |x: i64| (0..h).any(|y| lire(cd, x, y) >= 0.15);
    let mut segments: Vec<(i64, i64)> = Vec::new();
    for x in (0..l).filter(|&x| encre_col(x)) {
        match segments.last_mut() {
            Some(s) if x <= s.1 + ESPACE_MOT => s.1 = x,
            _ => segments.push((x, x)),
        }
    }
    let mut pire = 0.0f64;
    for (sa, sb) in segments {
        let coeurs = (0..h)
            .flat_map(|y| (sa..=sb).map(move |x| (x, y)))
            .filter(|&(x, y)| lire(cd, x, y) >= COEUR)
            .count()
            .max(1) as f64;
        let meilleur = (-DECALAGE..=DECALAGE)
            .flat_map(|dx| (-DECALAGE_V..=DECALAGE_V).map(move |dy| (dx, dy)))
            .map(|(dx, dy)| {
                let mut orphelins = 0u32;
                for y in 0..h {
                    for x in sa..=sb {
                        // Coeur du candidat sans trait de la référence alentour.
                        if lire(cd, x, y) >= COEUR && voisin_max(rf, x + dx, y + dy) < PRESENCE {
                            orphelins += 1;
                        }
                        // Coeur de la référence sans trait du candidat alentour.
                        if lire(rf, x + dx, y + dy) >= COEUR && voisin_max(cd, x, y) < PRESENCE {
                            orphelins += 1;
                        }
                    }
                }
                f64::from(orphelins) / coeurs
            })
            .fold(f64::INFINITY, f64::min);
        pire = pire.max(meilleur);
    }
    pire
}

fn cosinus(a: [f64; 3], b: [f64; 3]) -> f64 {
    let produit: f64 = (0..3).map(|i| a[i] * b[i]).sum();
    let na = a.iter().map(|v| v * v).sum::<f64>().sqrt();
    let nb = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return if na == nb { 1.0 } else { 0.0 };
    }
    produit / (na * nb)
}

/// Les chiffres d'une boîte et les garde-fous qu'elle viole.
#[derive(Debug, Clone)]
pub struct ResultatBoite {
    pub boite: BoiteEncre,
    /// Pixels divergents dans la boîte élargie, et plus fort delta de canal.
    pub differents: u64,
    pub delta_max: u8,
    /// Écart relatif d'encre totale (candidat contre référence).
    pub ecart_encre: f64,
    /// Plus grand écart de couverture sur une cellule.
    pub ecart_forme: f64,
    /// Cosinus des teintes.
    pub teinte: f64,
    /// Écart (candidat moins référence) des bords d'encre : x0, y0, x1, y1.
    pub ecart_bords: Option<[i64; 4]>,
    /// La largeur d'encre diffère d'un pixel (dette T1, `fr-t1.3`).
    pub dette_largeur: bool,
    /// Garde-fous violés, en clair.
    pub violations: Vec<String>,
}

impl ResultatBoite {
    /// Les chiffres des garde-fous, pour le rapport.
    pub fn mesures(&self) -> String {
        let bords = self.ecart_bords.map_or_else(
            || "—".to_owned(),
            |e| format!("{:+},{:+},{:+},{:+}", e[0], e[1], e[2], e[3]),
        );
        format!(
            "encre {:+.1} %, forme {:.1} %, teinte {:.4}, bords {bords}",
            100.0 * self.ecart_encre,
            100.0 * self.ecart_forme,
            self.teinte
        )
    }

    pub fn ok(&self) -> bool {
        self.violations.is_empty()
    }
}

fn juger(b: &BoiteEncre, rf: &Mesure, cd: &Mesure) -> ResultatBoite {
    let mut violations = Vec::new();
    let ecart_encre = if rf.encre > 0.0 {
        (cd.encre - rf.encre) / rf.encre
    } else if cd.encre > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };
    if ecart_encre.abs() > ECART_ENCRE {
        violations.push(format!(
            "encre totale {:+.0} % (borne ±{:.0} %)",
            100.0 * ecart_encre,
            100.0 * ECART_ENCRE
        ));
    }
    let ecart_forme = ecart_forme(rf, cd);
    if ecart_forme > ECART_FORME {
        violations.push(format!(
            "forme : {:.0} % des traits sans équivalent dans un mot (borne {:.0} %)",
            100.0 * ecart_forme,
            100.0 * ECART_FORME
        ));
    }
    let teinte = cosinus(rf.teinte, cd.teinte);
    if teinte < TEINTE_MIN {
        violations.push(format!("teinte : cosinus {teinte:.3} (min {TEINTE_MIN})"));
    }
    let (ecart_bords, dette_largeur) = match (rf.boite, cd.boite) {
        (Some(r), Some(c)) => {
            let e = [
                i64::from(c.0) - i64::from(r.0),
                i64::from(c.1) - i64::from(r.1),
                i64::from(c.2) - i64::from(r.2),
                i64::from(c.3) - i64::from(r.3),
            ];
            if e[0] != 0 || e[1] != 0 {
                violations.push(format!(
                    "origine de la boîte d'encre décalée de ({}, {}) px (strict)",
                    e[0], e[1]
                ));
            }
            if e[2].unsigned_abs() > u64::from(ECART_BORD)
                || e[3].unsigned_abs() > u64::from(ECART_BORD)
            {
                violations.push(format!(
                    "boîte d'encre : bord droit {:+}, bord bas {:+} px (borne ±{ECART_BORD})",
                    e[2], e[3]
                ));
            }
            (Some(e), e[2] != 0)
        }
        (None, None) => (None, false),
        _ => {
            violations.push("encre présente d'un seul côté".to_owned());
            (None, false)
        }
    };
    ResultatBoite {
        boite: *b,
        differents: 0,
        delta_max: 0,
        ecart_encre,
        ecart_forme,
        teinte,
        ecart_bords,
        dette_largeur,
        violations,
    }
}

/// Résultat d'une comparaison par catégorie.
#[derive(Debug)]
pub struct Sas {
    /// Divergences hors de toute boîte : géométrie, couleur, position (strict).
    pub differents_hors: u64,
    pub zones_hors: Vec<Boite>,
    /// Plus fort delta hors boîtes, et nombre de pixels hors boîtes de delta ≤ 2 (ombres, mélanges
    /// alpha) : de quoi savoir si l'écart est un arrondi ou une vraie divergence.
    pub delta_max_hors: u8,
    pub differents_hors_faibles: u64,
    /// Divergences dans l'union des boîtes, plus fort delta, aire de l'union.
    pub differents_boites: u64,
    pub delta_max_boites: u8,
    pub aire_boites: u64,
    pub boites: Vec<ResultatBoite>,
    /// Catégorie ombre (amendement du 11/11) : pixels tolérés (delta ≤ 2) dans les halos, plus
    /// fort delta parmi eux, aire des halos ; pixels de delta > 2 dans un halo (comptés aussi
    /// dans `differents_hors`).
    pub differents_ombre: u64,
    pub delta_max_ombre: u8,
    pub aire_halos: u64,
    pub forts_dans_halos: u64,
    pub ombres: Vec<ResultatOmbre>,
    pub verdict: Verdict,
    pub diff: RgbaImage,
}

impl Sas {
    /// Part de pixels divergents dans les boîtes (en %).
    pub fn pourcent_boites(&self) -> f64 {
        if self.aire_boites == 0 {
            0.0
        } else {
            100.0 * self.differents_boites as f64 / self.aire_boites as f64
        }
    }

    /// Part de pixels tolérés dans les halos d'ombre (en %).
    pub fn pourcent_ombre(&self) -> f64 {
        if self.aire_halos == 0 {
            0.0
        } else {
            100.0 * self.differents_ombre as f64 / self.aire_halos as f64
        }
    }

    pub fn dettes_largeur(&self) -> usize {
        self.boites.iter().filter(|b| b.dette_largeur).count()
    }
}

/// Compare `reference` et `candidat` avec les `boites` et les `ombres` déclarées par le candidat.
pub fn comparer(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    boites: &[BoiteEncre],
    ombres: &[OmbreDeclaree],
) -> anyhow::Result<Sas> {
    if reference.dimensions() != candidat.dimensions() {
        anyhow::bail!(
            "tailles différentes : référence {:?}, candidat {:?}",
            reference.dimensions(),
            candidat.dimensions()
        );
    }
    let (largeur, hauteur) = reference.dimensions();
    let mut dans = vec![false; (largeur * hauteur) as usize];
    for b in boites {
        let r = rect(b, PAD_TOL, largeur, hauteur);
        for y in r.1..=r.3 {
            for x in r.0..=r.2 {
                dans[(y * largeur + x) as usize] = true;
            }
        }
    }
    let aire_boites = dans.iter().filter(|&&d| d).count() as u64;
    // Les halos d'ombre, hors des boîtes de texte (qui gardent leurs propres garde-fous).
    let masques = ombres::masques(ombres, largeur, hauteur);
    let halo: Vec<bool> = masques
        .halo
        .iter()
        .zip(&dans)
        .map(|(&h, &d)| h && !d)
        .collect();
    let aire_halos = halo.iter().filter(|&&h| h).count() as u64;
    let (mut differents_ombre, mut delta_max_ombre, mut forts_dans_halos) = (0u64, 0u8, 0u64);
    let mut diff = RgbaImage::new(largeur, hauteur);
    let mut masque_hors = vec![false; dans.len()];
    let (mut differents, mut differents_hors, mut differents_boites) = (0u64, 0u64, 0u64);
    let mut delta_max_boites = 0u8;
    let (mut delta_max_hors, mut differents_hors_faibles) = (0u8, 0u64);
    for (x, y, r) in reference.enumerate_pixels() {
        let c = candidat.get_pixel(x, y);
        let delta = (0..4).map(|i| r.0[i].abs_diff(c.0[i])).max().unwrap_or(0);
        let i = (y * largeur + x) as usize;
        if delta == 0 {
            let g = (u16::from(r.0[0]) + u16::from(r.0[1]) + u16::from(r.0[2])) / 12;
            let g = u8::try_from(g).unwrap_or(u8::MAX);
            diff.put_pixel(x, y, Rgba([g, g, g, 255]));
            continue;
        }
        differents += 1;
        if dans[i] {
            differents_boites += 1;
            delta_max_boites = delta_max_boites.max(delta);
            // Orange : écart toléré dans une boîte ; rouge : hors boîte.
            diff.put_pixel(x, y, Rgba([255, 160, 0, 255]));
        } else if halo[i] && delta <= ombres::DELTA_OMBRE {
            differents_ombre += 1;
            delta_max_ombre = delta_max_ombre.max(delta);
            // Bleu : écart toléré dans un halo d'ombre.
            diff.put_pixel(x, y, Rgba([0, 140, 255, 255]));
        } else {
            forts_dans_halos += u64::from(halo[i]);
            differents_hors += 1;
            delta_max_hors = delta_max_hors.max(delta);
            differents_hors_faibles += u64::from(delta <= 2);
            masque_hors[i] = true;
            diff.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }
    let zones_hors = zones_connexes(&masque_hors, largeur, hauteur);
    let mut resultats = Vec::with_capacity(boites.len());
    for b in boites {
        let mesure = rect(b, PAD_MESURE, largeur, hauteur);
        let region_fond = rect(b, PAD_FOND, largeur, hauteur);
        let (fond_r, fond_c) = (fond(reference, region_fond), fond(candidat, region_fond));
        let k = (mesure.1..=mesure.3)
            .flat_map(|y| (mesure.0..=mesure.2).map(move |x| (x, y)))
            .map(|(x, y)| l1(*reference.get_pixel(x, y), fond_r))
            .max()
            .unwrap_or(0)
            .max(1);
        let k = f64::from(k);
        let mut res = juger(
            b,
            &mesurer(reference, mesure, fond_r, k),
            &mesurer(candidat, mesure, fond_c, k),
        );
        let tol = rect(b, PAD_TOL, largeur, hauteur);
        for y in tol.1..=tol.3 {
            for x in tol.0..=tol.2 {
                let (r, c) = (reference.get_pixel(x, y), candidat.get_pixel(x, y));
                let d = (0..4).map(|i| r.0[i].abs_diff(c.0[i])).max().unwrap_or(0);
                if d > 0 {
                    res.differents += 1;
                    res.delta_max = res.delta_max.max(d);
                }
            }
        }
        resultats.push(res);
    }
    // Les garde-fous d'ombre se mesurent hors des intérieurs et des boîtes de texte.
    let exclus: Vec<bool> = masques
        .interieur
        .iter()
        .zip(&dans)
        .map(|(&i, &d)| i || d)
        .collect();
    let resultats_ombres: Vec<ResultatOmbre> = ombres
        .iter()
        .map(|o| ombres::juger(reference, candidat, o, &halo, &exclus))
        .collect();
    let gardes = resultats.iter().all(ResultatBoite::ok) && resultats_ombres.iter().all(|o| o.ok());
    let verdict = if differents == 0 {
        Verdict::Strict
    } else if differents_hors == 0 && gardes {
        Verdict::Tolere
    } else {
        Verdict::Echec
    };
    // Une boîte qui échoue échoue aussi sans divergence de pixel (encre d'un seul côté).
    let verdict = if verdict == Verdict::Strict && !gardes {
        Verdict::Echec
    } else {
        verdict
    };
    Ok(Sas {
        differents_hors,
        zones_hors,
        delta_max_hors,
        differents_hors_faibles,
        differents_boites,
        delta_max_boites,
        aire_boites,
        boites: resultats,
        differents_ombre,
        delta_max_ombre,
        aire_halos,
        forts_dans_halos,
        ombres: resultats_ombres,
        verdict,
        diff,
    })
}

/// Le nombre de boîtes par catégorie : (texte, icônes).
pub fn par_categorie(boites: &[BoiteEncre]) -> (usize, usize) {
    let icones = boites
        .iter()
        .filter(|b| b.categorie == Categorie::Icone)
        .count();
    (boites.len() - icones, icones)
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::boites::detecter;
    use super::*;

    const FOND: [u8; 3] = [30, 30, 30];
    pub(in crate::pixels) const ENCRE: [u8; 3] = [220, 220, 220];

    /// Glyphes de 5 × 7 pixels, agrandis ×2 : assez pour qu'une lettre changée se voie.
    fn glyphe(c: char) -> [&'static str; 7] {
        match c {
            'a' => [
                ".###.", "#...#", "....#", ".####", "#...#", "#...#", ".####",
            ],
            'n' => [
                "#.##.", "##..#", "#...#", "#...#", "#...#", "#...#", "#...#",
            ],
            'e' => [
                ".###.", "#...#", "#...#", "#####", "#....", "#...#", ".###.",
            ],
            'o' => [
                ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
            ],
            'p' => [
                "####.", "#...#", "#...#", "####.", "#....", "#....", "#....",
            ],
            'u' => [
                "#...#", "#...#", "#...#", "#...#", "#...#", "#..##", ".##.#",
            ],
            _ => [""; 7],
        }
    }

    /// Dessine `texte` en (x0, y0) ; `couverture` imite un trait plus fin (< 1), `decalage_fin`
    /// pousse la dernière lettre de n pixels (la dette de largeur).
    pub(in crate::pixels) fn ligne(
        texte: &str,
        couverture: f64,
        encre: [u8; 3],
        decalage_fin: u32,
    ) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        let dernier = texte.chars().count() - 1;
        for (i, c) in texte.chars().enumerate() {
            let gx = 20 + 14 * i as u32 + if i == dernier { decalage_fin } else { 0 };
            for (r, rangee) in glyphe(c).iter().enumerate() {
                for (k, point) in rangee.chars().enumerate() {
                    if point != '#' {
                        continue;
                    }
                    let mut pixel = [0u8; 4];
                    for canal in 0..3 {
                        let (f, e) = (f64::from(FOND[canal]), f64::from(encre[canal]));
                        pixel[canal] = (f + couverture * (e - f)).round() as u8;
                    }
                    pixel[3] = 255;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        img.put_pixel(gx + 2 * k as u32 + dx, 20 + 2 * r as u32 + dy, Rgba(pixel));
                    }
                }
            }
        }
        img
    }

    fn sas(reference: &RgbaImage, candidat: &RgbaImage) -> Sas {
        comparer(reference, candidat, &detecter(candidat), &[]).unwrap()
    }

    fn motifs(s: &Sas) -> String {
        s.boites
            .iter()
            .flat_map(|b| b.violations.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ; ")
    }

    #[test]
    fn deux_images_identiques_sont_strictes() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let s = sas(&r, &r.clone());
        assert_eq!(s.verdict, Verdict::Strict);
        assert!(!s.boites.is_empty(), "le mot doit avoir sa boîte");
    }

    #[test]
    fn un_trait_plus_fin_est_tolere_dans_la_boite() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 0.85, ENCRE, 0);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
        assert_eq!(s.differents_hors, 0);
        assert!(s.differents_boites > 0 && s.delta_max_boites > 0);
    }

    /// ★ CONTRE-ÉPREUVE (ADR-009) : une lettre changée doit sortir rouge. « panneau » devient
    /// « ponneau » : même encre à peu près, même boîte, mais un trait manque et un autre est de trop.
    #[test]
    fn une_lettre_changee_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("ponneau", 0.85, ENCRE, 0);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert!(motifs(&s).contains("forme"), "{}", motifs(&s));
    }

    #[test]
    fn une_lettre_manquante_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("pa eau", 0.85, ENCRE, 0);
        assert_eq!(sas(&r, &c).verdict, Verdict::Echec);
    }

    #[test]
    fn une_autre_couleur_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 1.0, [220, 60, 60], 0);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert!(motifs(&s).contains("teinte"), "{}", motifs(&s));
    }

    #[test]
    fn un_pixel_hors_boite_reste_strict() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let mut c = r.clone();
        c.put_pixel(150, 55, Rgba([31, 30, 30, 255]));
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert_eq!(s.differents_hors, 1);
    }

    #[test]
    fn un_texte_decale_d_un_pixel_en_hauteur_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let mut c = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        for (x, y, p) in r.enumerate_pixels() {
            if y + 1 < 60 {
                c.put_pixel(x, y + 1, *p);
            }
        }
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert!(motifs(&s).contains("origine"), "{}", motifs(&s));
    }

    #[test]
    fn un_pixel_de_largeur_en_plus_est_une_dette_toleree() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 1.0, ENCRE, 1);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
        assert_eq!(s.dettes_largeur(), 1);
        // Deux pixels : au-delà du ±1 de l'ADR.
        let c2 = ligne("panneau", 1.0, ENCRE, 2);
        assert_eq!(sas(&r, &c2).verdict, Verdict::Echec);
    }

    #[test]
    fn de_l_encre_d_un_seul_cote_fait_echouer() {
        let r = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        let c = ligne("panneau", 1.0, ENCRE, 0);
        // Le candidat déclare ses boîtes : la référence, vide, n'a pas d'encre.
        assert_eq!(sas(&r, &c).verdict, Verdict::Echec);
    }

    #[test]
    fn des_tailles_differentes_sont_une_erreur() {
        let a = RgbaImage::new(2, 2);
        let b = RgbaImage::new(3, 2);
        assert!(comparer(&a, &b, &[], &[]).is_err());
    }
}
