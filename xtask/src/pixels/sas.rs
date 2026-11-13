//! Le sas par catégorie (ADR-009) : strict hors des boîtes de texte et d'icônes, anticrénelage
//! toléré DANS une boîte sous garde-fous.
//!
//! Hors des boîtes (élargies de [`PAD_TOL`] pixel) : 0 pixel de différence. Dans une boîte, quatre
//! garde-fous, mesurés sur la référence ET le candidat :
//! 1. **encre totale** : la couverture cumulée (distance au fond, normalisée par le contraste de
//!    la référence) reste à [`ECART_ENCRE`] près ;
//! 2. **position** (amendement du 11/11, 12:00) : le BARYCENTRE de l'encre du passage est à
//!    ±[`ECART_BARYCENTRE`] px de celui de la référence, en x ET en y ; un passage d'au moins
//!    [`LONGUEUR_MOITIES`] px d'encre est aussi mesuré par moitié gauche et moitié droite (un côté
//!    décalé ne se compense pas de l'autre). Les quatre bords de la boîte d'encre restent à ±1 px (la
//!    largeur à ±1 px est la dette T1 `fr-t1.3`). ★ Le bord à 10 % de couverture ne sert plus
//!    d'origine : GPUI cale un fût sur la grille (1,0 px net) quand Chromium l'étale (0,27 / 0,87 /
//!    0,27, même centre), la colonne de bord passait sous le seuil d'un seul côté ;
//! 3. **forme** : mot par mot, les pixels de cœur d'un trait (couverture ≥ [`COEUR`]) trouvent un trait
//!    (≥ [`PRESENCE`]) au même endroit de l'autre image, au meilleur décalage de ±[`DECALAGE`] px
//!    (±[`DECALAGE_V`] en hauteur) ; la part d'orphelins reste sous [`ECART_FORME`]. Des traits plus
//!    fins ou décalés d'un pixel ne font pas d'orphelin, une lettre changée ou absente en fait ;
//! 4. **teinte** : la couleur de l'encre garde la direction de celle de la référence
//!    (cosinus ≥ [`TEINTE_MIN`]).
//!
//! Une lettre manquante ou changée, une autre graisse ou une autre couleur les font échouer.

use image::{Rgba, RgbaImage};

use super::arrondis::{self, ResultatArrondi};
use super::boites::{ArrondiDeclare, BoiteEncre, Categorie, OmbreDeclaree};
use super::exceptions::{self, Exception};
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
/// Écart maximal (px) d'un bord de la boîte d'encre.
pub const ECART_BORD: u32 = 1;
/// Écart maximal (px) du barycentre de l'encre, en x et en y (ADR-009, amendement de 12:00).
pub const ECART_BARYCENTRE: f64 = 0.5;
/// Largeur d'encre (px) à partir de laquelle un passage est aussi mesuré par moitié. ★ Choisie sur
/// les 20 PNG de T1 : une moitié doit porter au moins deux glyphes de 13 px (≈ 7 px d'avance
/// chacun), sinon la masse d'un glyphe coupé par le milieu fait bouger le barycentre de la moitié
/// sans qu'aucun trait ne se déplace. Mesure : voir `docs/parite-pixels.md`.
pub const LONGUEUR_MOITIES: u32 = 28;
/// Couverture de colonne qui fait le coeur d'un glyphe, pour découper un passage en glyphes : sous
/// ce seuil des deux côtés, la colonne est un creux entre deux lettres (franges d'anticrénelage).
pub const COUPURE: f64 = 0.4;

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
    formes(rf, cd).iter().map(|s| s.part).fold(0.0, f64::max)
}

/// Un mot du garde-fou de forme : ses colonnes `[sa, sb]` dans la région et la part d'orphelins au
/// décalage de la référence qui l'explique le mieux.
#[derive(Debug, Clone, Copy)]
struct Segment {
    sa: i64,
    sb: i64,
    part: f64,
}

/// Couverture de `m` en (x, y) de sa région, 0 hors de la région.
fn lire(m: &Mesure, x: i64, y: i64) -> f64 {
    let l = m.largeur as i64;
    let h = m.couverture.len() as i64 / l.max(1);
    if x < 0 || x >= l || y < 0 || y >= h {
        0.0
    } else {
        m.couverture[(y * l + x) as usize]
    }
}

/// Les mots du candidat et, pour chacun, son meilleur décalage (voir [`ecart_forme`]).
fn formes(rf: &Mesure, cd: &Mesure) -> Vec<Segment> {
    let l = cd.largeur as i64;
    let h = cd.couverture.len() as i64 / l.max(1);
    let encre_col = |x: i64| (0..h).any(|y| lire(cd, x, y) >= 0.15);
    let mut segments: Vec<(i64, i64)> = Vec::new();
    for x in (0..l).filter(|&x| encre_col(x)) {
        match segments.last_mut() {
            Some(s) if x <= s.1 + ESPACE_MOT => s.1 = x,
            _ => segments.push((x, x)),
        }
    }
    let mut resultat = Vec::with_capacity(segments.len());
    for (sa, sb) in segments {
        let coeurs = (0..h)
            .flat_map(|y| (sa..=sb).map(move |x| (x, y)))
            .filter(|&(x, y)| lire(cd, x, y) >= COEUR)
            .count()
            .max(1) as f64;
        let part = (-DECALAGE..=DECALAGE)
            .flat_map(|dx| (-DECALAGE_V..=DECALAGE_V).map(move |dy| (dx, dy)))
            .map(|(dx, dy)| f64::from(orphelins(rf, cd, (sa, 0, sb, h - 1), dx, dy)) / coeurs)
            .fold(f64::INFINITY, f64::min);
        resultat.push(Segment { sa, sb, part });
    }
    resultat
}

/// Orphelins des deux sens dans la région `(x0, y0, x1, y1)` (bords compris) du candidat, la
/// référence décalée de (dx, dy) : coeurs d'une image sans trait de l'autre au même endroit.
fn orphelins(rf: &Mesure, cd: &Mesure, r: (i64, i64, i64, i64), dx: i64, dy: i64) -> u32 {
    let voisin_max = |m: &Mesure, x: i64, y: i64| -> f64 {
        (-VOISINAGE..=VOISINAGE)
            .flat_map(|u| (-VOISINAGE..=VOISINAGE).map(move |v| (u, v)))
            .map(|(u, v)| lire(m, x + u, y + v))
            .fold(0.0, f64::max)
    };
    let mut n = 0u32;
    for y in r.1..=r.3 {
        for x in r.0..=r.2 {
            // Coeur du candidat sans trait de la référence alentour.
            if lire(cd, x, y) >= COEUR && voisin_max(rf, x + dx, y + dy) < PRESENCE {
                n += 1;
            }
            // Coeur de la référence sans trait du candidat alentour.
            if lire(rf, x + dx, y + dy) >= COEUR && voisin_max(cd, x, y) < PRESENCE {
                n += 1;
            }
        }
    }
    n
}

/// Couverture de `m` au point réel (x, y) de sa région, interpolée entre ses quatre voisins.
fn lire_fin(m: &Mesure, x: f64, y: f64) -> f64 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    lire(m, x0, y0) * (1.0 - fx) * (1.0 - fy)
        + lire(m, x0 + 1, y0) * fx * (1.0 - fy)
        + lire(m, x0, y0 + 1) * (1.0 - fx) * fy
        + lire(m, x0 + 1, y0 + 1) * fx * fy
}

/// La couverture de `m` floutée d'un noyau 3×3 en tente (1/4, 1/2, 1/4) : deux phases sous le
/// pixel d'un même trait se ressemblent, deux traits différents non.
fn flouter(m: &Mesure) -> Mesure {
    let l = i64::from(m.largeur);
    let h = m.couverture.len() as i64 / l.max(1);
    let w = [0.25, 0.5, 0.25];
    let mut c = Vec::with_capacity(m.couverture.len());
    for y in 0..h {
        for x in 0..l {
            let mut s = 0.0;
            for (i, u) in (-1..=1).enumerate() {
                for (j, v) in (-1..=1).enumerate() {
                    s += w[i] * w[j] * lire(m, x + u, y + v);
                }
            }
            c.push(s);
        }
    }
    Mesure {
        couverture: c,
        ..m.clone()
    }
}

/// Les décalages fins de ±1 px au pas de 1/[`FIN`] px, en x et en y.
fn decalages_fins() -> impl Iterator<Item = (f64, f64)> {
    (-FIN..=FIN).flat_map(|i| {
        (-FIN..=FIN).map(move |j| (f64::from(i) / f64::from(FIN), f64::from(j) / f64::from(FIN)))
    })
}

/// Pas des décalages fins : un quart de pixel.
const FIN: i32 = 4;

/// Les points du rectangle `g` élargi de `e` px.
fn points(g: Rect, e: i64) -> Vec<(i64, i64)> {
    let (x0, y0) = (i64::from(g.0) - e, i64::from(g.1) - e);
    let (x1, y1) = (i64::from(g.2) + e, i64::from(g.3) + e);
    (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
        .collect()
}

/// Écart L1 normalisé de `a` (décalée de `oa`) et `b` (décalée de `ob` plus un décalage fin, au
/// meilleur), sur `pts`. Masse égalisée : `b` est multipliée par le rapport des masses (entre 1/2
/// et 2), une encre plus fine de 10 % ne se lit pas comme une autre forme.
fn ecart_egalise(
    a: &Mesure,
    oa: (i64, i64),
    b: &Mesure,
    ob: (i64, i64),
    pts: &[(i64, i64)],
) -> f64 {
    let va: Vec<f64> = pts
        .iter()
        .map(|&(x, y)| lire(a, x + oa.0, y + oa.1))
        .collect();
    let sa: f64 = va.iter().sum();
    let mut vb = vec![0.0; pts.len()];
    decalages_fins()
        .map(|(fx, fy)| {
            for (v, &(x, y)) in vb.iter_mut().zip(pts) {
                *v = lire_fin(b, (x + ob.0) as f64 + fx, (y + ob.1) as f64 + fy);
            }
            let sb: f64 = vb.iter().sum();
            let k = if sb > 0.0 {
                (sa / sb).clamp(0.5, 2.0)
            } else {
                1.0
            };
            let d: f64 = va.iter().zip(&vb).map(|(p, q)| (p - k * q).abs()).sum();
            let s = sa + k * sb;
            if s > 0.0 { d / s } else { 0.0 }
        })
        .fold(f64::INFINITY, f64::min)
}

/// Masses nettes du glyphe `g` du candidat contre la référence décalée de (dx, dy) plus un décalage
/// fin, au meilleur : le plus fort excès (ou défaut) d'encre sur une fenêtre de 3×3 puis de 2×2 px,
/// rapporté à la couverture maximale du glyphe. Un trait ajouté ou ôté se voit ici même quand le
/// reste de la lettre est commun.
fn masses_nettes(rf: &Mesure, cd: &Mesure, g: Rect, dx: i64, dy: i64) -> (f64, f64) {
    let pts = points(g, 0);
    let (l, h) = (i64::from(g.2 - g.0 + 1), i64::from(g.3 - g.1 + 1));
    let c: Vec<f64> = pts.iter().map(|&(x, y)| lire(cd, x, y)).collect();
    let mx = c.iter().copied().fold(0.0, f64::max).max(0.2);
    let sc: f64 = c.iter().sum();
    let mut f = vec![0.0; pts.len()];
    let mut nets = (f64::INFINITY, f64::INFINITY);
    for (fx, fy) in decalages_fins() {
        for (v, &(x, y)) in f.iter_mut().zip(&pts) {
            *v = lire_fin(rf, (x + dx) as f64 + fx, (y + dy) as f64 + fy);
        }
        let sf: f64 = f.iter().sum();
        let k = if sf > 0.0 {
            (sc / sf).clamp(0.5, 2.0)
        } else {
            1.0
        };
        let net = |w: i64| -> f64 {
            let mut m = 0.0f64;
            for y in 1 - w..h {
                for x in 1 - w..l {
                    let mut s = 0.0;
                    for v in 0..w {
                        for u in 0..w {
                            let (yy, xx) = (y + v, x + u);
                            if (0..h).contains(&yy) && (0..l).contains(&xx) {
                                let n = (yy * l + xx) as usize;
                                s += c[n] - k * f[n];
                            }
                        }
                    }
                    m = m.max(s.abs());
                }
            }
            m / mx
        };
        nets = (nets.0.min(net(3)), nets.1.min(net(2)));
    }
    nets
}

/// Le décalage entier (dx, dy) de la référence qui aligne le mieux le mot `[sa, sb]` du candidat :
/// le plus petit écart L1 des couvertures (à égalité, le plus petit |dx| + |dy|). ★ Pas le minimum
/// d'orphelins : un texte de faible contraste (palette, couverture max 0,70 < [`COEUR`]) n'a aucun
/// coeur, la part d'orphelins y est nulle à tout décalage et le plus lointain gagnait.
fn decalage_mot(rf: &Mesure, cd: &Mesure, sa: i64, sb: i64) -> (i64, i64) {
    let h = cd.couverture.len() as i64 / i64::from(cd.largeur).max(1);
    let mut meilleur = (f64::INFINITY, 0i64, 0i64);
    for dx in -DECALAGE..=DECALAGE {
        for dy in -DECALAGE_V..=DECALAGE_V {
            let mut d = 0.0;
            for y in 0..h {
                for x in sa..=sb {
                    d += (lire(cd, x, y) - lire(rf, x + dx, y + dy)).abs();
                }
            }
            let d = d + 1e-9 * (dx.abs() + dy.abs()) as f64;
            if d < meilleur.0 {
                meilleur = (d, dx, dy);
            }
        }
    }
    (meilleur.1, meilleur.2)
}

/// Bornes du garde-fou par glyphe (fr-p3.1, accordé par le chef le 13/11). ★ Calées sur 2007
/// glyphes légitimes de 22 captures réelles (marge 1,1 sur le pire) : L1 floutée 0,121, masse
/// nette 3×3 1,40, 2×2 0,99, rapport relatif 0,929 au plus bas. Aucune mesure seule ne sépare une
/// lettre changée d'un anticrénelage différent (c → o à 0,116, r → t à 0,118 en L1) : il faut les
/// quatre.
pub const GLYPHE_L1: f64 = 0.133;
pub const GLYPHE_NET3: f64 = 1.54;
pub const GLYPHE_NET2: f64 = 1.09;
/// Le glyphe du candidat ressemble à une AUTRE lettre de la référence mieux que ce rapport fois la
/// sienne : il a changé.
pub const GLYPHE_RELATIF: f64 = 0.7;
/// L1 floutée sous laquelle le test relatif n'est pas mené : le glyphe est si proche du sien qu'une
/// autre lettre (au moins [`MEME_LETTRE`] plus loin) ne peut pas faire mieux que
/// [`GLYPHE_RELATIF`] fois lui (coût : le test est quadratique dans le passage).
pub const GLYPHE_RELATIF_DES: f64 = 0.04;

/// Les quatre mesures d'un glyphe peint.
#[derive(Debug, Clone, Copy, Default)]
pub struct MesureGlyphe {
    pub l1: f64,
    pub net3: f64,
    pub net2: f64,
    /// Meilleur rapport (écart à une autre lettre de la référence) / (écart à la sienne) ; infini
    /// sans autre lettre comparable.
    pub relatif: f64,
}

impl MesureGlyphe {
    /// La marge du glyphe : au-delà de 1, une mesure franchit sa borne.
    pub fn marge(&self) -> f64 {
        (self.l1 / GLYPHE_L1)
            .max(self.net3 / GLYPHE_NET3)
            .max(self.net2 / GLYPHE_NET2)
            .max(GLYPHE_RELATIF / self.relatif.max(1e-9))
    }
}

/// Garde-fou de forme PAR GLYPHE (fr-p3.1), sur les glyphes peints déclarés par le candidat
/// (rectangles dans la région). Chaque glyphe se compare à la référence au décalage de son mot
/// ([`decalage_mot`]), plus un décalage fin de ±1 px, par quatre mesures : L1 floutée à masse
/// égalisée ([`ecart_egalise`]), masses nettes 3×3 et 2×2 ([`masses_nettes`]), et le test relatif
/// (ressemble-t-il mieux à une autre lettre de la référence qu'à la sienne ?). ★ Rapportée au mot
/// (garde-fou [`ecart_forme`]), une lettre changée dans un mot de dix ne laissait que 2 à 4 %
/// d'orphelins ; la part d'orphelins du glyphe seul ratait encore 11 % des lettres changées.
/// Rend la mesure de chaque glyphe.
fn mesurer_glyphes(rf: &Mesure, cd: &Mesure, glyphes: &[Rect]) -> Vec<MesureGlyphe> {
    if glyphes.is_empty() {
        return Vec::new();
    }
    let mots = formes(rf, cd);
    let decalages: Vec<(i64, i64)> = glyphes
        .iter()
        .map(|g| {
            let centre = (i64::from(g.0) + i64::from(g.2)) / 2;
            mots.iter()
                .find(|s| s.sa <= centre && centre <= s.sb)
                .or_else(|| {
                    mots.iter()
                        .min_by_key(|s| (s.sa - centre).abs().min((s.sb - centre).abs()))
                })
                .map_or((0, 0), |s| decalage_mot(rf, cd, s.sa, s.sb))
        })
        .collect();
    let (rb, cb) = (flouter(rf), flouter(cd));
    glyphes
        .iter()
        .enumerate()
        .map(|(i, &g)| {
            let (dx, dy) = decalages[i];
            let l1 = ecart_egalise(&cb, (0, 0), &rb, (dx, dy), &points(g, 1));
            let (net3, net2) = masses_nettes(rf, cd, g, dx, dy);
            let mut relatif = f64::INFINITY;
            if l1 >= GLYPHE_RELATIF_DES {
                for (k, &gk) in glyphes.iter().enumerate() {
                    if k == i {
                        continue;
                    }
                    // La partie commune des deux glyphes, à l'abscisse de i.
                    let (li, lk) = (g.2 - g.0, gk.2 - gk.0);
                    let gc = (g.0, g.1.max(gk.1), g.0 + li.min(lk), g.3.min(gk.3));
                    if gc.3 < gc.1 {
                        continue;
                    }
                    let pts = points(gc, 1);
                    let ok = (
                        i64::from(gk.0) - i64::from(g.0) + decalages[k].0,
                        decalages[k].1,
                    );
                    // La lettre k de la référence est-elle une autre lettre que la i ?
                    if ecart_egalise(&rb, (dx, dy), &rb, ok, &pts) < MEME_LETTRE {
                        continue;
                    }
                    let soi = ecart_egalise(&cb, (0, 0), &rb, (dx, dy), &pts);
                    let autre = ecart_egalise(&cb, (0, 0), &rb, ok, &pts);
                    relatif = relatif.min(autre / soi.max(1e-6));
                }
            }
            MesureGlyphe {
                l1,
                net3,
                net2,
                relatif,
            }
        })
        .collect()
}

/// Le pire glyphe : sa marge ([`MesureGlyphe::marge`]), son rang et ses mesures.
fn ecart_glyphes(
    rf: &Mesure,
    cd: &Mesure,
    glyphes: &[Rect],
) -> (f64, Option<(usize, MesureGlyphe)>) {
    mesurer_glyphes(rf, cd, glyphes)
        .into_iter()
        .enumerate()
        .map(|(i, m)| (m.marge(), Some((i, m))))
        .fold((0.0, None), |p, q| if q.0 > p.0 { q } else { p })
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
    /// Marge du pire glyphe peint ([`MesureGlyphe::marge`], au-delà de 1 : échec ; 0 sans glyphe
    /// déclaré).
    pub ecart_glyphe: f64,
    /// Cosinus des teintes.
    pub teinte: f64,
    /// Écarts (candidat moins référence) des barycentres d'encre : (partie, x, y).
    pub barycentres: Vec<(&'static str, f64, f64)>,
    /// Écart (candidat moins référence) des bords d'encre : x0, y0, x1, y1.
    pub ecart_bords: Option<[i64; 4]>,
    /// La largeur d'encre diffère d'un pixel (dette T1, `fr-t1.3`).
    pub dette_largeur: bool,
    /// Dès T2, boîte de texte en échec sur sa SEULE largeur, de [`ECART_PLACEMENT`] px au plus :
    /// dette nommée « placement des glyphes » (`fr-p4.1.1`), comptée par PNG dans le cliquet.
    pub dette_placement: bool,
    /// L'exception nommée de teinte qui couvre la boîte (`exceptions::Nature::Teinte`), le cas échéant.
    pub nommee: Option<&'static str>,
    /// Boîte d'encre de la RÉFÉRENCE (x0, y0, x1, y1), dans le repère de `boite`.
    pub encre_reference: Option<Rect>,
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
        let barycentres = self
            .barycentres
            .iter()
            .map(|(p, x, y)| format!("{p} {x:+.2},{y:+.2}"))
            .collect::<Vec<_>>()
            .join(" ; ");
        format!(
            "encre {:+.1} %, forme {:.1} %, glyphe {:.2}, teinte {:.4}, bords {bords}, barycentres {barycentres}",
            100.0 * self.ecart_encre,
            100.0 * self.ecart_forme,
            self.ecart_glyphe,
            self.teinte
        )
    }

    pub fn ok(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Barycentre (x, y) de la couverture de `m` sur les colonnes `[xa, xb]` de sa région, en px depuis
/// son coin haut gauche ; `None` sans encre.
fn barycentre(m: &Mesure, xa: u32, xb: u32) -> Option<(f64, f64)> {
    let l = m.largeur as usize;
    let (mut masse, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for (i, &c) in m.couverture.iter().enumerate() {
        let (x, y) = ((i % l) as u32, (i / l) as u32);
        if x < xa || x > xb {
            continue;
        }
        masse += c;
        sx += c * f64::from(x);
        sy += c * f64::from(y);
    }
    (masse > 0.0).then(|| (sx / masse, sy / masse))
}

/// Les glyphes d'un passage, en colonnes `[a, b]` de la région. Coeurs : suites de colonnes où
/// l'une OU l'autre image atteint [`COUPURE`] de couverture ; chaque creux entre deux coeurs est
/// partagé en son milieu (la colonne du milieu d'un creux impair, frange commune aux deux glyphes,
/// n'est à aucun), et les bords de la région vont au premier et au dernier glyphe. La même découpe
/// sert aux deux images.
fn glyphes(rf: &Mesure, cd: &Mesure) -> Vec<(u32, u32)> {
    let l = rf.largeur as usize;
    let h = rf.couverture.len() / l.max(1);
    let plein = |x: usize| {
        (0..h).any(|y| rf.couverture[y * l + x] >= COUPURE || cd.couverture[y * l + x] >= COUPURE)
    };
    let mut coeurs: Vec<(u32, u32)> = Vec::new();
    for x in (0..l).filter(|&x| plein(x)) {
        let x = x as u32;
        match coeurs.last_mut() {
            Some(s) if x == s.1 + 1 => s.1 = x,
            _ => coeurs.push((x, x)),
        }
    }
    let mut glyphes = coeurs.clone();
    for k in 0..coeurs.len().saturating_sub(1) {
        let (fin, debut) = (coeurs[k].1, coeurs[k + 1].0);
        let creux = debut - fin - 1;
        glyphes[k].1 = fin + creux / 2;
        glyphes[k + 1].0 = debut - creux / 2;
    }
    if let Some(g) = glyphes.first_mut() {
        g.0 = 0;
    }
    if let Some(g) = glyphes.last_mut() {
        g.1 = (l as u32).saturating_sub(1);
    }
    glyphes
}

/// Écarts (candidat moins référence) du barycentre de l'encre : le passage entier puis, s'il est
/// assez long, sa moitié gauche et sa moitié droite. Chaque entrée : (partie, écart en x, écart en y).
///
/// ★ Le barycentre se prend glyphe par glyphe (voir [`glyphes`]) puis se moyenne, pondéré par la
/// largeur du coeur du glyphe. Un barycentre unique sur tout le passage lisait la RÉPARTITION de l'encre, pas
/// la position : sur 413 px, des traits de GPUI plus fins de 10 % à gauche qu'à droite déplaçaient
/// le barycentre de −0,9 px, alors que chaque moitié restait à ±0,2 px (run 43011857108). Un glyphe
/// est symétrique sous le calage de GPUI (fût net contre fût étalé, même centre) : son barycentre ne
/// bouge pas ; un glyphe déplacé d'1 px, si.
fn ecarts_barycentres(rf: &Mesure, cd: &Mesure) -> Vec<(&'static str, f64, f64)> {
    let decalages: Vec<(f64, f64, f64, f64)> = glyphes(rf, cd)
        .into_iter()
        .filter_map(|(a, b)| {
            let (r, c) = (barycentre(rf, a, b)?, barycentre(cd, a, b)?);
            Some((
                f64::from(a + b) / 2.0,
                f64::from(b - a + 1),
                c.0 - r.0,
                c.1 - r.1,
            ))
        })
        .collect();
    let moyenne = |nom: &'static str, garde: &dyn Fn(f64) -> bool| {
        let (mut poids, mut dx, mut dy) = (0.0, 0.0, 0.0);
        for &(_, largeur, x, y) in decalages.iter().filter(|d| garde(d.0)) {
            poids += largeur;
            dx += largeur * x;
            dy += largeur * y;
        }
        (poids > 0.0).then(|| (nom, dx / poids, dy / poids))
    };
    let mut parties: Vec<_> = moyenne("passage", &|_| true).into_iter().collect();
    let longueur = rf.boite.map_or(0, |b| b.2 - b.0 + 1);
    if let (Some(&(premier, ..)), Some(&(dernier, ..))) = (decalages.first(), decalages.last())
        && longueur >= LONGUEUR_MOITIES
        && dernier > premier
    {
        let milieu = (premier + dernier) / 2.0;
        parties.extend(moyenne("moitié gauche", &|c| c <= milieu));
        parties.extend(moyenne("moitié droite", &|c| c > milieu));
    }
    parties
}

fn juger(
    b: &BoiteEncre,
    rf: &Mesure,
    cd: &Mesure,
    largeur_stricte: bool,
    glyphes: &[Rect],
) -> ResultatBoite {
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
            "{VIOLATION_ENCRE}{:+.0} % (borne ±{:.0} %)",
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
    let (ecart_glyphe, glyphe) = ecart_glyphes(rf, cd, glyphes);
    if ecart_glyphe > 1.0
        && let Some((i, m)) = glyphe
    {
        violations.push(format!(
            "{VIOLATION_GLYPHE}{} changé (L1 {:.3}/{GLYPHE_L1}, masse 3×3 {:.2}/{GLYPHE_NET3}, 2×2 {:.2}/{GLYPHE_NET2}, relatif {:.2}/{GLYPHE_RELATIF})",
            i + 1,
            m.l1,
            m.net3,
            m.net2,
            m.relatif
        ));
    }
    let teinte = cosinus(rf.teinte, cd.teinte);
    if teinte < TEINTE_MIN {
        violations.push(format!(
            "{VIOLATION_TEINTE}cosinus {teinte:.3} (min {TEINTE_MIN})"
        ));
    }
    let (ecart_bords, dette_largeur) = match (rf.boite, cd.boite) {
        (Some(r), Some(c)) => {
            let e = [
                i64::from(c.0) - i64::from(r.0),
                i64::from(c.1) - i64::from(r.1),
                i64::from(c.2) - i64::from(r.2),
                i64::from(c.3) - i64::from(r.3),
            ];
            if e.iter().any(|v| v.unsigned_abs() > u64::from(ECART_BORD)) {
                violations.push(format!(
                    "boîte d'encre : bords gauche {:+}, haut {:+}, droit {:+}, bas {:+} px (borne ±{ECART_BORD})",
                    e[0], e[1], e[2], e[3]
                ));
            }
            // ★ fr-p3.2 : la largeur à ±1 est la dette de T1 SEULEMENT (ADR-009). Dès T2, la largeur
            // d'encre (bord droit moins bord gauche) doit être celle de la référence ; les bords
            // gardent leur ±1 en position.
            if largeur_stricte && e[2] - e[0] != 0 {
                violations.push(format!(
                    "{VIOLATION_LARGEUR}{:+} px (strict depuis T2, la dette de largeur est réservée à T1)",
                    e[2] - e[0]
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
    let barycentres = ecarts_barycentres(rf, cd);
    for &(partie, dx, dy) in &barycentres {
        if dx.abs() > ECART_BARYCENTRE || dy.abs() > ECART_BARYCENTRE {
            violations.push(format!(
                "origine : barycentre ({partie}) décalé de ({dx:+.2}, {dy:+.2}) px (borne ±{ECART_BARYCENTRE})"
            ));
        }
    }
    ResultatBoite {
        boite: *b,
        barycentres,
        differents: 0,
        delta_max: 0,
        ecart_encre,
        ecart_forme,
        ecart_glyphe,
        teinte,
        ecart_bords,
        dette_largeur,
        dette_placement: false,
        nommee: None,
        encre_reference: rf.boite,
        violations,
    }
}

/// Les garde-fous qu'une exception nommée de T1 lève sur la boîte `b` (teinte, encre, glyphes),
/// et son nom dans `res.nommee`.
pub(crate) fn lever_nommees(
    res: &mut ResultatBoite,
    nommees: &[&'static Exception],
    b: &BoiteEncre,
) {
    // Exception nommée de teinte (fr-t1.5, « N coupé ClearType ») : ne lève QUE le garde-fou de
    // teinte de cette boîte ; encre, forme, bords et barycentres restent exigés.
    if let Some(element) = exceptions::pour_boite(nommees, b) {
        res.violations.retain(|v| !v.starts_with(VIOLATION_TEINTE));
        res.nommee = Some(element);
    }
    // ★ fr-p3.1 : la boîte peinte d'une icône est sa TUILE (marge d'anticrénelage comprise), plus
    // large que l'encre ; l'exception garde sa boîte d'encre et vise la boîte qui la contient.
    let encre = res.encre_reference.map(|(x0, y0, x1, y1)| {
        let (dx, dy) = (
            i64::from(b.x) - i64::from(res.boite.x),
            i64::from(b.y) - i64::from(res.boite.y),
        );
        let p = |v: u32, d: i64| u32::try_from(i64::from(v) + d).unwrap_or(0);
        (p(x0, dx), p(y0, dy), p(x1, dx), p(y1, dy))
    });
    // Exception nommée d'encre (fr-t1.5, « icône en hairline ») : ne lève QUE le garde-fou
    // d'encre totale, jusqu'à `ECART_ENCRE_HAIRLINE`.
    if let Some(element) =
        exceptions::pour_boite_ou_encre(nommees, b, encre, exceptions::Nature::Encre)
    {
        if res.ecart_encre.abs() <= exceptions::ECART_ENCRE_HAIRLINE {
            res.violations.retain(|v| !v.starts_with(VIOLATION_ENCRE));
        }
        res.nommee = Some(element);
    }
    // Exception nommée des glyphes (fr-t1.5, « glyphes à 8,8 px en clair ») : l'encre jusqu'à
    // 30 % et les barycentres des PARTIES jusqu'à 0,75 px ; le passage (l'origine) reste à 0,5.
    if let Some(element) = exceptions::pour_boite_de(nommees, b, exceptions::Nature::Glyphes) {
        if res.ecart_encre.abs() <= exceptions::ECART_ENCRE_HAIRLINE {
            res.violations.retain(|v| !v.starts_with(VIOLATION_ENCRE));
        }
        let parties_ok = res.barycentres.iter().all(|&(partie, dx, dy)| {
            partie == "passage"
                || (dx.abs() <= exceptions::ECART_BARYCENTRE_GLYPHES
                    && dy.abs() <= exceptions::ECART_BARYCENTRE_GLYPHES)
        });
        if parties_ok {
            res.violations
                .retain(|v| !(v.starts_with("origine : barycentre (") && !v.contains("(passage)")));
        }
        res.nommee = Some(element);
    }
    // Exception nommée d'un glyphe déplacé (fr-p4.1.1, placement des glyphes) : la SEULE forme par
    // glyphe, jusqu'à la marge `MARGE_GLYPHE_PLACEMENT` ; le passage reste à sa borne.
    if let Some(element) = exceptions::pour_boite_de(nommees, b, exceptions::Nature::Placement) {
        if res.ecart_glyphe <= exceptions::MARGE_GLYPHE_PLACEMENT {
            res.violations.retain(|v| !v.starts_with(VIOLATION_GLYPHE));
        }
        res.nommee = Some(element);
    }
}

/// Préfixe de la violation du garde-fou de forme par glyphe (voir [`juger`]).
const VIOLATION_GLYPHE: &str = "forme : glyphe n° ";

/// Préfixe de la violation de largeur stricte des tranches ≥ T2 (voir [`juger`]).
const VIOLATION_LARGEUR: &str = "boîte d'encre : largeur ";

/// Écart de largeur d'encre (px) que la dette « placement des glyphes » couvre.
pub const ECART_PLACEMENT: i64 = 2;

/// Dette nommée « placement des glyphes » (`fr-p4.1.1`, orchestrateur du 13/11, 03:30) : GPUI cale
/// les glyphes au pixel quand Chromium les étale en sous-pixel, la boîte d'encre d'un libellé perd ou
/// gagne 1 à 2 px de largeur. Une boîte de TEXTE dont la largeur est le SEUL garde-fou violé, de
/// [`ECART_PLACEMENT`] px au plus, passe en dette ; toute autre violation la garde stricte.
fn lever_placement(res: &mut ResultatBoite) {
    let largeur = res.ecart_bords.map_or(0, |e| e[2] - e[0]);
    if res.boite.categorie == Categorie::Texte
        && largeur != 0
        && largeur.abs() <= ECART_PLACEMENT
        && !res.violations.is_empty()
        && res
            .violations
            .iter()
            .all(|v| v.starts_with(VIOLATION_LARGEUR))
    {
        res.violations.clear();
        res.dette_placement = true;
    }
}

/// Préfixe de la violation du garde-fou de teinte (voir [`juger`]).
const VIOLATION_TEINTE: &str = "teinte : ";

/// Préfixe de la violation du garde-fou d'encre totale (voir [`juger`]).
const VIOLATION_ENCRE: &str = "encre totale ";

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
    /// Exception nommée `--shadow-l` : pixels de delta > 2 dans son halo (non bloquants, en
    /// cliquet) et leur plus fort delta.
    pub forts_exception: u64,
    pub delta_max_exception: u8,
    /// Catégorie bord d'arrondi (amendement de 11:15) : pixels tolérés (delta ≤ 6) dans les bandes
    /// des arcs déclarés, plus fort delta parmi eux, aire des bandes ; pixels de delta > 6 dans une
    /// bande (comptés aussi dans `differents_hors`).
    pub differents_arrondi: u64,
    pub delta_max_arrondi: u8,
    pub aire_bandes: u64,
    pub forts_dans_bandes: u64,
    pub arrondis: Vec<ResultatArrondi>,
    /// Les exceptions nommées temporaires de T1 (à retirer par `fr-p4.1`) : par élément, ses
    /// mesures en valeur absolue (clés du cliquet).
    pub nommees:
        std::collections::BTreeMap<&'static str, std::collections::BTreeMap<&'static str, f64>>,
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

    /// Part de pixels tolérés dans les bandes d'arrondi (en %).
    pub fn pourcent_arrondi(&self) -> f64 {
        if self.aire_bandes == 0 {
            0.0
        } else {
            100.0 * self.differents_arrondi as f64 / self.aire_bandes as f64
        }
    }

    /// Les garde-fous mesurés de l'exception nommée : le pire, en valeur absolue, des surfaces de
    /// `--shadow-l` (`None` sans telle surface).
    pub fn gardes_exception(&self) -> Option<ombres::Gardes> {
        let liste: Vec<&ombres::Gardes> = self
            .ombres
            .iter()
            .filter(|o| o.exception)
            .map(|o| &o.gardes)
            .collect();
        if liste.is_empty() {
            return None;
        }
        let pire = |f: fn(&ombres::Gardes) -> f64| {
            liste
                .iter()
                .map(|g| f(g))
                .fold(0.0f64, |p, v| p.max(v.abs()))
        };
        Some(ombres::Gardes {
            etalement: pire(|g| g.etalement),
            encre: pire(|g| g.encre),
            glissement: pire(|g| g.glissement),
        })
    }

    /// Le pire écart absolu des barycentres d'encre, en x et en y, et où se trouve le pire des deux.
    pub fn pire_barycentre(&self) -> Option<(f64, f64, String)> {
        let mut pire: Option<(f64, f64, String, f64)> = None;
        for b in &self.boites {
            for &(partie, dx, dy) in &b.barycentres {
                let (x, y, ou, m) = pire.take().unwrap_or((0.0, 0.0, String::new(), -1.0));
                let (x, y) = (x.max(dx.abs()), y.max(dy.abs()));
                let ici = dx.abs().max(dy.abs());
                pire = Some(if ici > m {
                    (
                        x,
                        y,
                        format!(
                            "{} ({},{}), {partie}",
                            b.boite.categorie.nom(),
                            b.boite.x,
                            b.boite.y
                        ),
                        ici,
                    )
                } else {
                    (x, y, ou, m)
                });
            }
        }
        pire.map(|(x, y, ou, _)| (x, y, ou))
    }

    /// La pire marge d'un glyphe peint (garde-fou de forme par glyphe, fr-p3.1), parmi les boîtes
    /// de texte qui passent : la marge réelle sous 1.
    pub fn pire_glyphe(&self) -> Option<(f64, String)> {
        self.boites
            .iter()
            .filter(|b| b.ok() && b.ecart_glyphe > 0.0)
            .map(|b| (b.ecart_glyphe, format!("({},{})", b.boite.x, b.boite.y)))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    }

    pub fn dettes_largeur(&self) -> usize {
        self.boites.iter().filter(|b| b.dette_largeur).count()
    }

    /// Boîtes en dette « placement des glyphes » (`fr-p4.1.1`), clé du cliquet.
    pub fn dettes_placement(&self) -> usize {
        self.boites.iter().filter(|b| b.dette_placement).count()
    }
}

/// Les glyphes peints de la boîte de texte `b` (centre dans la boîte), en rectangles de la région `r`.
fn glyphes_dans(b: &BoiteEncre, glyphes: &[BoiteEncre], r: Rect) -> Vec<Rect> {
    if b.categorie != Categorie::Texte {
        return Vec::new();
    }
    glyphes
        .iter()
        .filter(|g| contient(b, g))
        .map(|g| {
            (
                g.x.saturating_sub(r.0),
                g.y.saturating_sub(r.1),
                (g.x + g.largeur - 1).min(r.2).saturating_sub(r.0),
                (g.y + g.hauteur - 1).min(r.3).saturating_sub(r.1),
            )
        })
        .collect()
}

/// Le centre du glyphe `g` est dans la boîte `b`.
fn contient(b: &BoiteEncre, g: &BoiteEncre) -> bool {
    let (cx, cy) = (g.x + g.largeur / 2, g.y + g.hauteur / 2);
    b.x <= cx && cx < b.x + b.largeur && b.y <= cy && cy < b.y + b.hauteur
}

/// Les garde-fous d'une boîte, sans exception nommée ni compte des pixels divergents.
pub fn juger_boite(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    b: &BoiteEncre,
    glyphes: &[BoiteEncre],
    depuis: Option<u32>,
) -> ResultatBoite {
    let (largeur, hauteur) = reference.dimensions();
    let mesure = rect(b, PAD_MESURE, largeur, hauteur);
    let region_fond = rect(b, PAD_FOND, largeur, hauteur);
    let (fond_r, fond_c) = (fond(reference, region_fond), fond(candidat, region_fond));
    let k = contraste(reference, mesure, fond_r);
    juger(
        b,
        &mesurer(reference, mesure, fond_r, k),
        &mesurer(candidat, mesure, fond_c, k),
        largeur_stricte(depuis),
        &glyphes_dans(b, glyphes, mesure),
    )
}

/// Le contraste de `img` dans `r` : la plus grande distance L1 au fond (1 au moins).
fn contraste(img: &RgbaImage, r: Rect, fond: Rgba<u8>) -> f64 {
    let k = (r.1..=r.3)
        .flat_map(|y| (r.0..=r.2).map(move |x| (x, y)))
        .map(|(x, y)| l1(*img.get_pixel(x, y), fond))
        .max()
        .unwrap_or(0)
        .max(1);
    f64::from(k)
}

/// Essais au plus par boîte, et par PNG, de la contre-épreuve (temps du job `pixels`).
pub const ESSAIS_PAR_BOITE: usize = 12;
pub const ESSAIS_PAR_PNG: usize = 200;

/// Écart sous lequel deux glyphes sont la même lettre (voir [`ecart_substitution`]). ★ Orchestrateur,
/// 13/11 03:15 : 0,086, calé sur les captures réelles. Les mêmes lettres à deux phases sous le pixel
/// montent jusqu'à 0,085 (« d », « e », « m », « r ») : 0,08 en faisait des lettres changées. Le prix :
/// « a » remplacé par « e » (0,085) sort aussi du banc ; « e » remplacé par « a » (0,090) y reste.
/// Testé aux deux bords sur la même fixture réelle.
pub const MEME_LETTRE: f64 = 0.086;

/// Part minimale des lettres changées que la contre-épreuve doit détecter, tous PNG confondus
/// (orchestrateur, 13/11 03:15 : fusion à 99,2 %). Les manquées sont une dette nommée, comptée par
/// PNG en cliquet ; la cible reste 100 % (`fr-p3.4`).
pub const DETECTION_MIN: f64 = 0.992;

/// La contre-épreuve d'une lettre changée sur du texte RÉEL (fr-p3.1, ADR-009 point 2).
#[derive(Debug, Default)]
pub struct ContreEpreuve {
    /// Lettres changées essayées, et combien le sas en a fait échouer.
    pub essais: usize,
    pub detectes: usize,
    /// Dont les essais sur un petit glyphe entier (point, virgule, tiret : 3×3 px au plus).
    pub petits: usize,
    /// Glyphes qui n'ont dans leur passage aucune AUTRE lettre qui tienne dans leur avance pour les
    /// remplacer : pas d'essai (le banc ne génère pas d'essai « même lettre ailleurs »).
    pub sans_autre_lettre: usize,
    /// Les essais passés inaperçus, en clair.
    pub manques: Vec<String>,
}

/// Écart de `a` à `b` sur `pts` : L1 normalisé (somme des écarts sur somme des couvertures), `b`
/// lue au meilleur décalage fin de ±1 px.
fn ecart_fin(a: &Mesure, b: &Mesure, pts: &[(i64, i64)]) -> f64 {
    let va: Vec<f64> = pts.iter().map(|&(x, y)| lire(a, x, y)).collect();
    decalages_fins()
        .map(|(fx, fy)| {
            let (mut d, mut s) = (0.0, 0.0);
            for (p, &(x, y)) in va.iter().zip(pts) {
                let q = lire_fin(b, x as f64 + fx, y as f64 + fy);
                d += (p - q).abs();
                s += p + q;
            }
            if s > 0.0 { d / s } else { 0.0 }
        })
        .fold(f64::INFINITY, f64::min)
}

/// Écart d'une lettre changée : le candidat `avant` et `apres` (couvertures floutées, coordonnées
/// de leur région), sur chaque glyphe de `glyphes` qui touche les colonnes `[x0, x1]` de la
/// substitution (rectangle élargi d'un pixel) ; rend le plus grand. ★ Mesuré sur le RÉSULTAT de la
/// substitution, pas entre les deux glyphes : un glyphe plus large collé sur un plus étroit déborde
/// sur son voisin, et « r » remplacé par « m » devant un « m » redonnait « rm » au pixel près.
pub fn ecart_substitution(
    avant: &Mesure,
    apres: &Mesure,
    glyphes: &[BoiteEncre],
    x0: u32,
    x1: u32,
) -> f64 {
    glyphes
        .iter()
        .filter(|g| g.x <= x1 + 1 && g.x + g.largeur >= x0)
        .map(|g| {
            let r = (g.x, g.y, g.x + g.largeur - 1, g.y + g.hauteur - 1);
            ecart_fin(apres, avant, &points(r, 1))
        })
        .fold(0.0, f64::max)
}

/// La substitution laisse-t-elle la MÊME lettre ? Écart ([`ecart_substitution`]) sous
/// [`MEME_LETTRE`] : la même lettre à une autre phase sous le pixel (« o » pour « o ») ; le banc ne
/// génère pas un tel essai.
fn meme_lettre(avant: &Mesure, apres: &Mesure, glyphes: &[BoiteEncre], x0: u32, x1: u32) -> bool {
    ecart_substitution(avant, apres, glyphes, x0, x1) < MEME_LETTRE
}

/// Le glyphe `gj` peut-il remplacer `gi` dans un essai ? ★ Il tient dans l'avance de `gi` (1 px de
/// plus au plus) : collé plus large, il recouvrait le glyphe voisin, et « r » remplacé par « m »
/// devant un « m » redonnait « rm » à un demi-pixel près (écart 0,083, sous le pixel : pas une
/// lettre changée). Deux petits glyphes de même taille à 1 px près (les points d'un « ... ») sont le
/// même signe : à 2×2 px, la phase sous le pixel suffit à porter l'écart au-dessus de
/// [`MEME_LETTRE`] (0,085 mesuré).
fn substituable(gi: &BoiteEncre, gj: &BoiteEncre) -> bool {
    let meme_petit = petit(gi)
        && petit(gj)
        && gi.largeur.abs_diff(gj.largeur) <= 1
        && gi.hauteur.abs_diff(gj.hauteur) <= 1;
    gj.largeur <= gi.largeur + 1 && !meme_petit
}

/// Le candidat `img` où le glyphe `gi` est remplacé par `gj` : `gi` effacé au fond `fond`, `gj`
/// collé à l'abscisse de `gi`, à sa propre hauteur (même ligne de base).
fn substituer(img: &RgbaImage, fond: Rgba<u8>, gi: &BoiteEncre, gj: &BoiteEncre) -> RgbaImage {
    let mut mute = img.clone();
    for y in gi.y..gi.y + gi.hauteur {
        for x in gi.x..gi.x + gi.largeur {
            mute.put_pixel(x, y, fond);
        }
    }
    for v in 0..gj.hauteur {
        for u in 0..gj.largeur {
            let (x, y) = (gi.x + u, gj.y + v);
            if x < mute.width() && y < mute.height() {
                mute.put_pixel(x, y, *img.get_pixel(gj.x + u, gj.y + v));
            }
        }
    }
    mute
}

/// Un petit glyphe (point, virgule, tiret) : 3×3 px au plus.
fn petit(g: &BoiteEncre) -> bool {
    g.largeur <= 3 && g.hauteur <= 3
}

/// Pour chaque boîte de texte qui PASSE le sas, change une lettre du candidat (le glyphe peint `i`
/// remplacé par un autre glyphe `j` du même passage, d'une autre forme, collé à l'abscisse de `i`
/// sur la même ligne de base) et vérifie que la boîte échoue alors. Sur des captures réelles, c'est
/// la contre-épreuve d'ADR-009 sur du vrai texte de 13 px, et non sur des glyphes agrandis.
pub fn contre_epreuve(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    boites: &[BoiteEncre],
    glyphes: &[BoiteEncre],
    nommees: &[&'static Exception],
    depuis: Option<u32>,
) -> ContreEpreuve {
    let mut ce = ContreEpreuve::default();
    if glyphes.is_empty() || reference.dimensions() != candidat.dimensions() {
        return ce;
    }
    let (largeur, hauteur) = reference.dimensions();
    for b in boites.iter().filter(|b| b.categorie == Categorie::Texte) {
        if ce.essais >= ESSAIS_PAR_PNG {
            break;
        }
        let mut gl: Vec<BoiteEncre> = glyphes.iter().filter(|g| contient(b, g)).copied().collect();
        if gl.len() < 2 {
            continue;
        }
        gl.sort_by_key(|g| (g.x, g.y));
        let mut base = juger_boite(reference, candidat, b, glyphes, depuis);
        lever_nommees(&mut base, nommees, b);
        if !base.ok() {
            continue;
        }
        // Le travail se fait sur un extrait (la boîte et sa marge de fond) : la boîte y est jugée
        // comme dans l'image entière, ses régions y étant rognées de la même façon.
        let r = rect(b, PAD_FOND, largeur, hauteur);
        let extrait = |img: &RgbaImage| {
            image::imageops::crop_imm(img, r.0, r.1, r.2 - r.0 + 1, r.3 - r.1 + 1).to_image()
        };
        let (ref_e, cd_e) = (extrait(reference), extrait(candidat));
        let local = |g: &BoiteEncre| BoiteEncre {
            x: g.x - r.0,
            y: g.y - r.1,
            ..*g
        };
        let b_local = local(b);
        let gl_local: Vec<BoiteEncre> = gl.iter().map(local).collect();
        let tout = (0, 0, cd_e.width() - 1, cd_e.height() - 1);
        let fond_c = fond(&cd_e, tout);
        let k = contraste(&cd_e, tout, fond_c);
        let flouee = |img: &RgbaImage| flouter(&mesurer(img, tout, fond_c, k));
        let avant = flouee(&cd_e);
        // Les essais, répartis sur le passage, plus chaque petit glyphe entier (point, virgule,
        // tiret) : il reste jugeable et doit être vu.
        let pas = gl.len().div_ceil(ESSAIS_PAR_BOITE).max(1);
        let mut rangs: Vec<usize> = (0..gl.len()).step_by(pas).collect();
        rangs.extend((0..gl.len()).filter(|&i| i % pas != 0 && petit(&gl[i])));
        for i in rangs {
            if ce.essais >= ESSAIS_PAR_PNG {
                break;
            }
            let gi = gl_local[i];
            // Un petit glyphe entier (point, virgule, tiret) est EFFACÉ : aucune lettre ne tient dans
            // son avance, et un point manquant doit se voir.
            let efface = BoiteEncre {
                largeur: 0,
                hauteur: 0,
                ..gi
            };
            let choix = if petit(&gi) {
                Some((efface, substituer(&cd_e, fond_c, &gi, &efface)))
            } else {
                (1..gl.len())
                    .map(|d| gl_local[(i + d) % gl.len()])
                    .filter(|gj| substituable(&gi, gj))
                    .map(|gj| (gj, substituer(&cd_e, fond_c, &gi, &gj)))
                    .find(|(gj, mute)| {
                        let x1 = gi.x + gi.largeur.max(gj.largeur) - 1;
                        !meme_lettre(&avant, &flouee(mute), &gl_local, gi.x, x1)
                    })
            };
            let Some((gj, mute)) = choix else {
                ce.sans_autre_lettre += 1;
                continue;
            };
            if petit(&gi) {
                ce.petits += 1;
            }
            let mut res = juger_boite(&ref_e, &mute, &b_local, &gl_local, depuis);
            lever_nommees(&mut res, nommees, b);
            ce.essais += 1;
            if res.ok() {
                ce.manques.push(format!(
                    "boîte ({},{}) {}×{} : glyphe ({},{}) {}×{} {} sans échec [{}]",
                    b.x,
                    b.y,
                    b.largeur,
                    b.hauteur,
                    gi.x + r.0,
                    gi.y + r.1,
                    gi.largeur,
                    gi.hauteur,
                    if gj.largeur == 0 {
                        "effacé".to_owned()
                    } else {
                        format!(
                            "remplacé par ({},{}) {}×{}",
                            gj.x + r.0,
                            gj.y + r.1,
                            gj.largeur,
                            gj.hauteur
                        )
                    },
                    res.mesures()
                ));
            } else {
                ce.detectes += 1;
            }
        }
    }
    ce
}

/// Compare `reference` et `candidat` avec les `boites` et les `ombres` déclarées, sans arrondis.
#[cfg(test)]
pub fn comparer(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    boites: &[BoiteEncre],
    ombres: &[OmbreDeclaree],
) -> anyhow::Result<Sas> {
    comparer_tout(reference, candidat, boites, ombres, &[], &[])
}

/// Première tranche où la largeur d'encre est stricte (fr-p3.2).
pub const TRANCHE_LARGEUR_STRICTE: u32 = 2;

/// La largeur d'encre est-elle stricte pour une scène marquée `"depuis": depuis` ? Une scène sans
/// marque est de T1 : la dette de largeur (`fr-t1.3`) lui reste permise.
pub fn largeur_stricte(depuis: Option<u32>) -> bool {
    depuis.is_some_and(|t| t >= TRANCHE_LARGEUR_STRICTE)
}

/// Compare `reference` et `candidat` avec les `boites`, les `ombres` et les `arrondis` déclarés par
/// le candidat, sous les exceptions nommées `nommees` du PNG (`exceptions::du_png`). Scène de T1.
#[cfg(test)]
pub fn comparer_tout(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    boites: &[BoiteEncre],
    ombres: &[OmbreDeclaree],
    arrondis: &[ArrondiDeclare],
    nommees: &[&'static Exception],
) -> anyhow::Result<Sas> {
    comparer_tranche(
        reference,
        candidat,
        boites,
        &[],
        ombres,
        arrondis,
        nommees,
        None,
    )
}

/// Comme `comparer_tout`, pour une scène de la tranche `depuis` (`None` = T1) : dès T2 la largeur
/// d'encre est stricte.
#[allow(clippy::too_many_arguments)] // les déclarations du candidat, une par catégorie
pub fn comparer_tranche(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    boites: &[BoiteEncre],
    glyphes: &[BoiteEncre],
    ombres: &[OmbreDeclaree],
    arrondis: &[ArrondiDeclare],
    nommees: &[&'static Exception],
    depuis: Option<u32>,
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
    // fr-t1.5 : un trait droit (bordure, séparateur, bord de fond) reste STRICT, même sous la boîte
    // gonflée d'un texte qui le longe. Ses pixels sont ceux du détecteur, sur la référence ET sur le
    // candidat : un trait décalé d'1 px diverge sur des pixels de trait des deux images.
    // ★ Les deux, pas l'un OU l'autre : sous le bas d'un glyphe posé sur une bordure, le fond entre
    // deux jambages est un pixel de trait d'une image et de l'encre dans l'autre ; l'union rendrait
    // strict l'anticrénelage même que la boîte tolère.
    let traits_reference = super::boites::pixels_de_trait(reference);
    let traits_candidat = super::boites::pixels_de_trait(candidat);
    for ((d, &r), &c) in dans.iter_mut().zip(&traits_reference).zip(&traits_candidat) {
        if r && c {
            *d = false;
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
    let exception: Vec<bool> = masques
        .exception
        .iter()
        .zip(&dans)
        .map(|(&e, &d)| e && !d)
        .collect();
    // Les bandes des arcs, hors des boîtes de texte.
    let bande: Vec<bool> = arrondis::masque(arrondis, largeur, hauteur)
        .iter()
        .zip(&dans)
        .map(|(&b, &d)| b && !d)
        .collect();
    let aire_bandes = bande.iter().filter(|&&b| b).count() as u64;
    // Les bandes des arcs d'une exception nommée (à retirer par fr-p4.1) : l'élément de chaque pixel.
    let elements_arcs: Vec<Option<&'static str>> = arrondis
        .iter()
        .map(|a| exceptions::pour_arrondi(nommees, a, ombres))
        .collect();
    let mut bande_nommee: Vec<Option<&'static str>> = vec![None; dans.len()];
    for (a, element) in arrondis.iter().zip(&elements_arcs) {
        let Some(element) = element else { continue };
        for arc in arrondis::arcs(a) {
            for (x, y) in arrondis::pixels_bande(&arc, largeur, hauteur) {
                let i = (y * largeur + x) as usize;
                if !dans[i] {
                    bande_nommee[i] = Some(element);
                }
            }
        }
    }
    // Les côtés du contour de l'onglet actif, au delta ≤ 1 seulement (`exceptions::DELTA_COTES`).
    let mut cote_nomme: Vec<Option<&'static str>> = vec![None; dans.len()];
    for (a, element) in arrondis.iter().zip(&elements_arcs) {
        let Some(element) = element else { continue };
        for (x, y) in exceptions::pixels_cotes(a, largeur, hauteur) {
            let i = (y * largeur + x) as usize;
            if !dans[i] {
                cote_nomme[i] = Some(element);
            }
        }
    }
    let mut forts_nommes: std::collections::BTreeMap<&'static str, (u64, u8)> =
        std::collections::BTreeMap::new();
    let (mut differents_arrondi, mut delta_max_arrondi, mut forts_dans_bandes) = (0u64, 0u8, 0u64);
    let (mut forts_exception, mut delta_max_exception) = (0u64, 0u8);
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
        } else if bande[i] && delta <= arrondis::DELTA_ARRONDI {
            differents_arrondi += 1;
            delta_max_arrondi = delta_max_arrondi.max(delta);
            // Violet : écart toléré dans la bande d'un arc.
            diff.put_pixel(x, y, Rgba([190, 0, 255, 255]));
        } else if halo[i] && delta <= ombres::DELTA_OMBRE {
            differents_ombre += 1;
            delta_max_ombre = delta_max_ombre.max(delta);
            // Bleu : écart toléré dans un halo d'ombre.
            diff.put_pixel(x, y, Rgba([0, 140, 255, 255]));
        } else if let Some(element) = bande_nommee[i] {
            let e = forts_nommes.entry(element).or_default();
            e.0 += 1;
            e.1 = e.1.max(delta);
            // Magenta : exception nommée temporaire de T1, mesurée par le cliquet.
            diff.put_pixel(x, y, Rgba([255, 0, 160, 255]));
        } else if let Some(element) = cote_nomme[i].filter(|_| delta <= exceptions::DELTA_COTES) {
            // Côté du contour de l'onglet actif, delta ≤ 1 : même élément, même cliquet.
            let e = forts_nommes.entry(element).or_default();
            e.0 += 1;
            e.1 = e.1.max(delta);
            diff.put_pixel(x, y, Rgba([255, 0, 160, 255]));
        } else if let Some(element) = exceptions::demi_couverture(nommees, reference, (x, y), delta)
        {
            // Demi-couverture exacte de la référence, delta ≤ 1 (exception nommée de T1).
            let e = forts_nommes.entry(element).or_default();
            e.0 += 1;
            e.1 = e.1.max(delta);
            diff.put_pixel(x, y, Rgba([255, 0, 160, 255]));
        } else if exception[i] && !bande[i] {
            forts_exception += 1;
            delta_max_exception = delta_max_exception.max(delta);
            // Cyan : exception nommée `--shadow-l`, mesurée par le cliquet.
            diff.put_pixel(x, y, Rgba([0, 230, 230, 255]));
        } else {
            forts_dans_bandes += u64::from(bande[i]);
            forts_dans_halos += u64::from(halo[i] && !bande[i]);
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
        let mut res = juger_boite(reference, candidat, b, glyphes, depuis);
        lever_nommees(&mut res, nommees, b);
        lever_placement(&mut res);
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
        .map(|o| {
            let mut r = ombres::juger(reference, candidat, o, &halo, &exclus);
            r.nommee = exceptions::pour_ombre(nommees, o);
            r
        })
        .collect();
    let resultats_arrondis: Vec<ResultatArrondi> = arrondis
        .iter()
        .zip(&elements_arcs)
        .map(|(a, element)| {
            let mut r = arrondis::juger(reference, candidat, a, &dans);
            r.nommee = *element;
            r
        })
        .collect();
    // Les mesures des exceptions nommées, en valeur absolue : le pire de chaque élément.
    let mut mesures_nommees: std::collections::BTreeMap<
        &'static str,
        std::collections::BTreeMap<&'static str, f64>,
    > = std::collections::BTreeMap::new();
    let mut garder = |element: &'static str, cle: &'static str, v: f64| {
        let e = mesures_nommees
            .entry(element)
            .or_default()
            .entry(cle)
            .or_insert(0.0);
        *e = e.max(v.abs());
    };
    for o in &resultats_ombres {
        if let Some(element) = o.nommee {
            garder(element, "etalement", o.gardes.etalement);
            garder(element, "encre", o.gardes.encre);
            garder(element, "glissement", o.gardes.glissement);
        }
    }
    for b in &resultats {
        if let Some(element) = b.nommee {
            if element == exceptions::GLYPHES_8_8 {
                garder(element, "encre", b.ecart_encre);
                let pire = b
                    .barycentres
                    .iter()
                    .filter(|(partie, _, _)| *partie != "passage")
                    .map(|&(_, dx, dy)| dx.abs().max(dy.abs()))
                    .fold(0.0, f64::max);
                garder(element, "barycentre", pire);
            } else if element == exceptions::GLYPHE_DEPLACE {
                garder(element, "glyphe", b.ecart_glyphe);
            } else if element == exceptions::ICONE_HAIRLINE {
                // L'écart d'encre relatif, en valeur absolue.
                garder(element, "encre", b.ecart_encre);
            } else {
                // L'écart de teinte : 1 − cosinus (0 pour une teinte identique).
                garder(element, "teinte", 1.0 - b.teinte);
            }
        }
    }
    for a in &resultats_arrondis {
        if let Some(element) = a.nommee {
            let (forts, delta) = forts_nommes.get(element).copied().unwrap_or_default();
            garder(element, "deplacement", a.pire_aire());
            garder(element, "forts", forts as f64);
            garder(element, "delta_max", f64::from(delta));
        }
    }
    for e in nommees
        .iter()
        .filter(|e| e.nature == exceptions::Nature::DemiCouverture)
    {
        let (forts, delta) = forts_nommes.get(e.element).copied().unwrap_or_default();
        garder(e.element, "forts", forts as f64);
        garder(e.element, "delta_max", f64::from(delta));
    }
    let gardes = resultats.iter().all(ResultatBoite::ok)
        && resultats_ombres.iter().all(|o| o.ok())
        && resultats_arrondis.iter().all(ResultatArrondi::ok);
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
        forts_exception,
        delta_max_exception,
        differents_arrondi,
        delta_max_arrondi,
        aire_bandes,
        forts_dans_bandes,
        arrondis: resultats_arrondis,
        nommees: mesures_nommees,
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
            '.' => ["", "", "", "", "", "", "#"],
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
        let n = texte.chars().count();
        let decalages: Vec<i64> = (0..n)
            .map(|i| {
                if i + 1 == n {
                    i64::from(decalage_fin)
                } else {
                    0
                }
            })
            .collect();
        ligne_decalee(texte, couverture, encre, &decalages)
    }

    /// Comme [`ligne`], chaque lettre poussée de `decalages[i]` px en x.
    fn ligne_decalee(texte: &str, couverture: f64, encre: [u8; 3], decalages: &[i64]) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        for (i, c) in texte.chars().enumerate() {
            let gx = (20 + 14 * i as i64 + decalages[i]) as u32;
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

    /// Les glyphes peints de [`ligne`] : un rectangle de 10 × 14 px par lettre dessinée.
    fn glyphes_de(texte: &str, pas: u32) -> Vec<BoiteEncre> {
        texte
            .chars()
            .enumerate()
            .filter(|(_, c)| *c != ' ')
            .map(|(i, _)| BoiteEncre {
                categorie: Categorie::Texte,
                x: 20 + pas * i as u32,
                y: 20,
                largeur: 10,
                hauteur: 14,
            })
            .collect()
    }

    fn sas_glyphes(reference: &RgbaImage, candidat: &RgbaImage, texte: &str, pas: u32) -> Sas {
        let glyphes = glyphes_de(texte, pas);
        let boites = super::super::boites::regrouper(&glyphes);
        comparer_tranche(reference, candidat, &boites, &glyphes, &[], &[], &[], None).unwrap()
    }

    /// ★ fr-p3.1 : rapportée au MOT, une lettre changée qui partage ses traits avec l'autre passe ;
    /// rapportée à SON glyphe peint, elle échoue. « oooooooooo » dont un « o » devient « e » : la
    /// barre du « e » et le bas de l'« o » ne font que 2 % des traits du mot, lettres serrées (1 colonne
    /// vide entre deux glyphes, comme à 13 px : un seul mot).
    #[test]

    fn une_lettre_proche_changee_dans_un_long_mot_echoue_par_son_glyphe() {
        let serre: Vec<i64> = (0..10).map(|i| -3 * i).collect();
        let r = ligne_decalee("oooooooooo", 1.0, ENCRE, &serre);
        let c = ligne_decalee("ooooeooooo", 1.0, ENCRE, &serre);
        let sans = sas(&r, &c);
        assert_eq!(sans.verdict, Verdict::Tolere, "{}", motifs(&sans));
        let avec = sas_glyphes(&r, &c, "ooooeooooo", 11);
        assert_eq!(avec.verdict, Verdict::Echec);
        assert!(motifs(&avec).contains("glyphe n° 5"), "{}", motifs(&avec));
    }

    /// L'écart d'une substitution sur un extrait RÉEL de candidat (`xtask/fixtures/`, capture de
    /// fragment-rs) : le glyphe `i` remplacé par `j`, comme dans la contre-épreuve.
    fn ecart_fixture(png: &[u8], glyphes: &[(u32, u32, u32, u32)], i: usize, j: usize) -> f64 {
        let img = image::load_from_memory(png).expect("fixture").to_rgba8();
        let gl: Vec<BoiteEncre> = glyphes
            .iter()
            .map(|&(x, y, largeur, hauteur)| BoiteEncre {
                categorie: Categorie::Texte,
                x,
                y,
                largeur,
                hauteur,
            })
            .collect();
        let tout = (0, 0, img.width() - 1, img.height() - 1);
        let f = fond(&img, tout);
        let k = contraste(&img, tout, f);
        let flouee = |im: &RgbaImage| flouter(&mesurer(im, tout, f, k));
        let mute = substituer(&img, f, &gl[i], &gl[j]);
        let x1 = gl[i].x + gl[i].largeur.max(gl[j].largeur) - 1;
        ecart_substitution(&flouee(&img), &flouee(&mute), &gl, gl[i].x, x1)
    }

    /// Les glyphes de la fixture réelle `a-e-0085.png`.
    const GLYPHES_A_E: [(u32, u32, u32, u32); 5] = [
        (0, 6, 8, 11),
        (8, 6, 8, 9),
        (17, 6, 7, 8),
        (25, 6, 7, 8),
        (33, 6, 8, 9),
    ];

    #[test]
    fn deux_o_a_deux_phases_sont_la_meme_lettre() {
        // Bord bas du seuil (chef, 13/11) : « Zoom », le premier « o » remplacé par le second.
        let e = ecart_fixture(
            include_bytes!("../../fixtures/o-o-0073.png"),
            &[(4, 4, 7, 10), (12, 6, 8, 9), (20, 6, 8, 9), (29, 6, 11, 8)],
            1,
            2,
        );
        assert!((0.070..MEME_LETTRE).contains(&e), "{e}");
    }

    #[test]
    fn un_a_pour_un_e_est_sous_le_seuil() {
        // Bord bas du seuil 0,086 : un « a » remplacé par un « e » du même passage (0,085) passe
        // pour la même lettre ; le banc ne génère pas cet essai.
        let e = ecart_fixture(
            include_bytes!("../../fixtures/a-e-0085.png"),
            &GLYPHES_A_E,
            1,
            4,
        );
        assert!((0.080..MEME_LETTRE).contains(&e), "{e}");
    }

    #[test]
    fn un_e_pour_un_a_est_une_autre_lettre() {
        // Bord haut du seuil : l'essai inverse, « e » remplacé par « a » (0,090), est une lettre
        // changée.
        let e = ecart_fixture(
            include_bytes!("../../fixtures/a-e-0085.png"),
            &GLYPHES_A_E,
            4,
            1,
        );
        assert!((MEME_LETTRE..0.095).contains(&e), "{e}");
    }

    #[test]
    fn un_trait_plus_fin_passe_glyphe_par_glyphe() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 0.85, ENCRE, 0);
        let s = sas_glyphes(&r, &c, "panneau", 14);
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
    }

    /// La contre-épreuve elle-même : sur un passage qui passe, chaque lettre changée est vue, et une
    /// lettre remplacée par la même (« n » par « n ») n'est pas un essai.
    #[test]
    fn la_contre_epreuve_change_une_lettre_par_essai_et_les_voit_toutes() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 0.85, ENCRE, 0);
        let glyphes = glyphes_de("panneau", 14);
        let boites = super::super::boites::regrouper(&glyphes);
        let ce = contre_epreuve(&r, &c, &boites, &glyphes, &[], None);
        assert_eq!(ce.essais, 7);
        assert_eq!(ce.detectes, ce.essais, "{:?}", ce.manques);
        // Sans glyphes déclarés (boîtes déduites), pas de contre-épreuve.
        assert_eq!(contre_epreuve(&r, &c, &boites, &[], &[], None).essais, 0);
    }

    #[test]
    fn la_contre_epreuve_efface_un_petit_glyphe_entier_et_le_voit() {
        // Un point ENTIER (non rogné) reste jugeable (orchestrateur, 13/11) : effacé, il fait échouer.
        let r = ligne("pan.", 1.0, ENCRE, 0);
        let c = ligne("pan.", 0.85, ENCRE, 0);
        let mut glyphes = glyphes_de("pan", 14);
        glyphes.push(BoiteEncre {
            categorie: Categorie::Texte,
            x: 20 + 14 * 3,
            y: 32,
            largeur: 2,
            hauteur: 2,
        });
        let boites = super::super::boites::regrouper(&glyphes);
        let ce = contre_epreuve(&r, &c, &boites, &glyphes, &[], None);
        assert_eq!(ce.petits, 1);
        assert_eq!(ce.essais, 4);
        assert_eq!(ce.detectes, ce.essais, "{:?}", ce.manques);
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

    /// ★ CONTRE-ÉPREUVE (ADR-009, amendement de 12:00) : un passage déplacé d'1 px en x échoue par
    /// son barycentre, alors que ses quatre bords d'encre restent dans le ±1 px.
    #[test]
    fn un_passage_decale_d_un_pixel_en_x_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne_decalee("panneau", 1.0, ENCRE, &[1; 7]);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        let m = motifs(&s);
        assert!(m.contains("barycentre (passage)"), "{m}");
        assert!(!m.contains("boîte d'encre"), "{m}");
    }

    /// Un côté poussé à gauche, l'autre à droite : le barycentre du passage ne bouge presque pas,
    /// ceux des moitiés bougent d'un pixel.
    #[test]
    fn une_moitie_decalee_ne_se_compense_pas_de_l_autre() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne_decalee("panneau", 1.0, ENCRE, &[-1, -1, -1, 0, 1, 1, 1]);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        let m = motifs(&s);
        assert!(
            m.contains("moitié gauche") && m.contains("moitié droite"),
            "{m}"
        );
        assert!(!m.contains("(passage)"), "{m}");
    }

    /// Le calage des fûts de GPUI : un trait net d'1 px (couverture 1,0) contre le même trait étalé
    /// par Chromium (0,15 / 0,7 / 0,15, même centre). Le bord à 10 % recule d'une colonne d'un seul
    /// côté ; le barycentre, lui, ne bouge pas : toléré.
    #[test]
    fn un_fut_cale_sur_la_grille_est_tolere() {
        let c = ligne("panneau", 1.0, ENCRE, 0);
        let mut r = c.clone();
        let (l, h) = c.dimensions();
        for y in 0..h {
            for x in 1..l - 1 {
                let couv = |x: u32| {
                    let p = c.get_pixel(x, y).0[0];
                    (f64::from(p) - f64::from(FOND[0])) / (f64::from(ENCRE[0]) - f64::from(FOND[0]))
                };
                let v = 0.15 * couv(x - 1) + 0.7 * couv(x) + 0.15 * couv(x + 1);
                let mut pixel = [255u8; 4];
                for canal in 0..3 {
                    let (f, e) = (f64::from(FOND[canal]), f64::from(ENCRE[canal]));
                    pixel[canal] = (f + v * (e - f)).round() as u8;
                }
                r.put_pixel(x, y, Rgba(pixel));
            }
        }
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
        let bords = s.boites[0].ecart_bords.unwrap();
        assert_eq!(
            bords[0], 1,
            "le bord à 10 % recule d'une colonne : {bords:?}"
        );
    }

    /// ★ CONTRE-ÉPREUVE sur une vraie capture : dans la référence de `t1-menu-dark`, chaque passage
    /// de texte déplacé d'1 px (à droite, puis en bas) fait échouer son barycentre ; les autres
    /// passages, intacts, restent stricts.
    #[test]
    fn un_passage_de_la_reference_deplace_d_un_pixel_fait_echouer() {
        let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pixels/reference/t1-menu-dark.png");
        let r = image::open(chemin)
            .map(|i| i.to_rgba8())
            .unwrap_or_else(|e| panic!("référence illisible : {e}"));
        let boites = detecter(&r);
        let textes: Vec<&BoiteEncre> = boites
            .iter()
            .filter(|b| b.categorie == Categorie::Texte)
            .collect();
        assert!(textes.len() >= 5, "{boites:?}");
        for b in textes {
            for (dx, dy) in [(1u32, 0u32), (0, 1)] {
                let mut c = r.clone();
                let (x0, y0, x1, y1) = rect(b, PAD_MESURE, r.width(), r.height());
                for y in (y0..=y1).rev() {
                    for x in (x0..=x1).rev() {
                        let source = *r.get_pixel(x - dx, y - dy);
                        c.put_pixel(x, y, source);
                    }
                }
                let s = comparer(&r, &c, &boites, &[]).unwrap();
                assert_eq!(s.verdict, Verdict::Echec, "{b:?} poussé de ({dx}, {dy})");
                let fautive = s.boites.iter().find(|x| x.boite == *b).unwrap();
                let m = fautive.violations.join(" ; ");
                assert!(
                    m.contains("barycentre (passage)"),
                    "{b:?} ({dx}, {dy}) : {m}"
                );
                assert!(!m.contains("boîte d'encre"), "{b:?} ({dx}, {dy}) : {m}");
            }
        }
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

    /// Comme `sas`, pour une scène de la tranche `depuis`.
    fn sas_depuis(reference: &RgbaImage, candidat: &RgbaImage, depuis: Option<u32>) -> Sas {
        comparer_tranche(
            reference,
            candidat,
            &detecter(candidat),
            &[],
            &[],
            &[],
            &[],
            depuis,
        )
        .unwrap()
    }

    #[test]
    fn la_largeur_stricte_commence_a_t2() {
        assert!(!largeur_stricte(None));
        assert!(!largeur_stricte(Some(1)));
        assert!(largeur_stricte(Some(2)));
        assert!(largeur_stricte(Some(11)));
    }

    /// ★ fr-p4.1.1 (orchestrateur, 13/11, 03:30) : dès T2, une largeur à ±1 n'est plus la dette de
    /// T1 mais la dette « placement des glyphes », comptée à part (cliquet par PNG).
    #[test]
    fn un_pixel_de_largeur_en_plus_est_la_dette_de_t1_puis_celle_du_placement_des_glyphes() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 1.0, ENCRE, 1);
        for t1 in [None, Some(1)] {
            let s = sas_depuis(&r, &c, t1);
            assert_eq!(s.verdict, Verdict::Tolere, "{t1:?} : {}", motifs(&s));
            assert_eq!(s.dettes_largeur(), 1);
            assert_eq!(s.dettes_placement(), 0);
        }
        for t in [2, 4] {
            let s = sas_depuis(&r, &c, Some(t));
            assert_eq!(s.verdict, Verdict::Tolere, "T{t} : {}", motifs(&s));
            assert_eq!(s.dettes_placement(), 1, "T{t}");
        }
    }

    /// Contre-épreuve : la dette ne couvre que la largeur SEULE. Un bord à 2 px (au-delà du ±1) ou
    /// une autre violation dans la même boîte la laisse en échec, largeur comprise.
    #[test]
    fn la_dette_de_placement_ne_couvre_que_la_largeur_seule() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c2 = ligne("panneau", 1.0, ENCRE, 2);
        let s = sas_depuis(&r, &c2, Some(2));
        assert_eq!(s.verdict, Verdict::Echec, "{}", motifs(&s));
        assert_eq!(s.dettes_placement(), 0);
        assert!(motifs(&s).contains("largeur +2 px"), "{}", motifs(&s));
        let autre = ligne("panneau", 1.0, [200, 40, 40], 1);
        let s = sas_depuis(&r, &autre, Some(2));
        assert_eq!(s.verdict, Verdict::Echec, "{}", motifs(&s));
        assert_eq!(s.dettes_placement(), 0);
    }

    /// ★ CONTRE-ÉPREUVE (fr-p3.2) : en T2 la largeur exacte passe, même avec un trait plus fin
    /// (anticrénelage) ; le refus ci-dessus vient donc bien de la largeur seule.
    #[test]
    fn une_largeur_exacte_passe_en_t2() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let s = sas_depuis(&r, &r, Some(2));
        assert_eq!(s.verdict, Verdict::Strict, "{}", motifs(&s));
        let fin = ligne("panneau", 0.85, ENCRE, 0);
        let s = sas_depuis(&r, &fin, Some(2));
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
        assert_eq!(s.dettes_largeur(), 0);
    }

    #[test]
    fn de_l_encre_d_un_seul_cote_fait_echouer() {
        let r = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        let c = ligne("panneau", 1.0, ENCRE, 0);
        // Le candidat déclare ses boîtes : la référence, vide, n'a pas d'encre.
        assert_eq!(sas(&r, &c).verdict, Verdict::Echec);
    }

    /// `img` avec un trait horizontal de 1 px sur la rangée `y`, de x = 10 à 149.
    fn avec_trait_horizontal(mut img: RgbaImage, y: u32) -> RgbaImage {
        for x in 10..150 {
            img.put_pixel(x, y, Rgba([90, 90, 90, 255]));
        }
        img
    }

    /// `img` avec une bordure verticale de 1 px sur la colonne `x`, sur toute la hauteur.
    fn avec_bordure_verticale(mut img: RgbaImage, x: u32) -> RgbaImage {
        for y in 0..img.height() {
            img.put_pixel(x, y, Rgba([90, 90, 90, 255]));
        }
        img
    }

    /// ★ CONTRE-ÉPREUVE (fr-t1.5) : le bas des lettres (rangée 33) touche un trait (rangée 34) ;
    /// le texte garde sa boîte, mais le trait décalé d'1 px reste hors boîte et fait échouer.
    #[test]
    fn un_trait_decale_contre_un_texte_fait_echouer() {
        let r = avec_trait_horizontal(ligne("panneau", 1.0, ENCRE, 0), 34);
        let ok = sas(
            &r,
            &avec_trait_horizontal(ligne("panneau", 0.85, ENCRE, 0), 34),
        );
        assert_eq!(ok.verdict, Verdict::Tolere, "{}", motifs(&ok));
        assert!(
            !ok.boites.is_empty(),
            "le texte posé sur le trait garde sa boîte"
        );
        let c = avec_trait_horizontal(ligne("panneau", 1.0, ENCRE, 0), 35);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        // Les deux rangées du trait divergent sur toute sa longueur, sous les lettres comprises.
        assert!(
            s.differents_hors >= 2 * 140 - 2 * 14,
            "{}",
            s.differents_hors
        );
    }

    /// ★ CONTRE-ÉPREUVE (fr-t1.5, le A de « Aucun » contre le ruban) : la boîte du « a » gonflée
    /// d'1 px couvre la bordure qu'il touche ; la bordure décalée d'1 px sous le « a » échoue.
    #[test]
    fn une_bordure_decalee_sous_le_a_fait_echouer() {
        // Le « a » commence en x = 20 : la bordure en x = 19 touche son flanc gauche.
        let r = avec_bordure_verticale(ligne("anneau", 1.0, ENCRE, 0), 19);
        let ok = sas(
            &r,
            &avec_bordure_verticale(ligne("anneau", 0.85, ENCRE, 0), 19),
        );
        assert_eq!(ok.verdict, Verdict::Tolere, "{}", motifs(&ok));
        assert!(
            ok.boites.iter().any(|b| b.boite.x <= 21),
            "le « a » garde sa boîte : {:?}",
            ok.boites.iter().map(|b| b.boite).collect::<Vec<_>>()
        );
        let c = avec_bordure_verticale(ligne("anneau", 1.0, ENCRE, 0), 18);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        // Les deux colonnes divergent sur toute la hauteur, à côté du « a » compris.
        assert_eq!(s.differents_hors, 2 * 60);
    }

    #[test]
    fn des_tailles_differentes_sont_une_erreur() {
        let a = RgbaImage::new(2, 2);
        let b = RgbaImage::new(3, 2);
        assert!(comparer(&a, &b, &[], &[]).is_err());
    }
}
