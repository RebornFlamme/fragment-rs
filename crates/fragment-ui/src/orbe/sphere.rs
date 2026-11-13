// Port du moteur de « Thinking Orbs » (Jakub Antalik, MIT, voir LICENSE-THINKING-ORBS) :
// `engine/lattice.ts` de la bibliothèque d'origine. Traduit en Rust, commentaires réécrits.
//
// Les trois formes en grille de latitude / longitude : le globe (cherche), le cube qui se
// résout (résout) et l'onde (écoute). Chacune pose son champ de points, puis `finaliser` trie.

use std::f64::consts::PI;

use super::noyau::{
    ImageOrbe, Point, Projecteur, arrondi, ecart_angulaire, echelle_rayons, finaliser, hachage,
};
use super::reglages::Reglages;

/// Les points d'une grille de sphère : `(longitude, cos lat, sin lat, indice d'anneau)`.
fn grille(anneaux: f64, densite: f64, mut f: impl FnMut(f64, f64, f64, usize)) {
    let n = anneaux.max(0.0) as usize;
    for li in 0..=n {
        let lat = -PI / 2.0 + (li as f64 / anneaux) * PI;
        let (sin_lat, cos_lat) = lat.sin_cos();
        let nb_lon = arrondi(cos_lat.abs() * densite).max(1.0) as usize;
        for lj in 0..nb_lon {
            let lon = (lj as f64 / nb_lon as f64) * 2.0 * PI;
            f(lon, cos_lat, sin_lat, li);
        }
    }
}

/// Globe : un méridien de balayage parcourt la grille — l'agent cherche.
#[must_use]
pub fn image_globe(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let rotation = 0.5;
    let c = cote / 2.0;
    let rayon = c * 0.82;
    let inclinaison = 0.4 + 0.06 * (t * 0.35).sin();
    let pt = Projecteur::new(t * rotation, inclinaison, c, c, rayon);
    // Le balayage tourne par rapport à la rotation ; `scan_mul` règle cet écart.
    let balayage = t * (rotation + (1.7 - rotation) * o.scan_mul.unwrap_or(1.0));
    let rs = echelle_rayons(cote, o.rs_pow.unwrap_or(0.6));
    let terne = o.dim_base.unwrap_or(1.0);

    let mut points = Vec::new();
    grille(
        o.lat_rings.unwrap_or(17.0),
        o.lon_density.unwrap_or(44.0),
        |lon, cos_lat, sin_lat, _| {
            let [x, y, z] = pt.projeter(cos_lat * lon.cos(), sin_lat, cos_lat * lon.sin());
            let profondeur = (z + 1.0) / 2.0;
            // Le méridien se lit en taille, pas en éclat.
            let d = ecart_angulaire(lon + t * rotation, balayage);
            let elan = (-(d * d) / 0.18).exp() * z.max(0.0);
            points.push(Point {
                x,
                y,
                z,
                r: (o.r_base.unwrap_or(0.6)
                    + o.r_depth.unwrap_or(1.7) * profondeur
                    + o.r_boost.unwrap_or(1.0) * elan)
                    * rs,
                blanc: o.ink_far.unwrap_or(0.62) - o.ink_span.unwrap_or(0.54) * profondeur,
                // Sous 1, les points hors du méridien pâlissent : le balayage se détache.
                a: Some(terne + (1.0 - terne) * elan.min(1.0)),
            });
        },
    );
    finaliser(points, Vec::new(), o.r_min)
}

/// Un quart de tour d'une tranche du cube.
#[derive(Debug, Clone, Copy)]
struct Mouvement {
    axe: u8,
    bas: f64,
    haut: f64,
    angle: f64,
}

/// L'avancée de chaque mouvement à `temps` : brouillage par quarts de tour rapides, puis
/// rejeu à l'envers (palindrome) jusqu'à l'état résolu, un repos, et ça recommence.
fn cycle_de_resolution(
    temps: f64,
    nombre: usize,
    duree: f64,
    repos: f64,
) -> (Vec<f64>, Option<usize>) {
    let total = 2.0 * nombre as f64 * duree;
    let tc = temps % (total + repos);
    let mut avancee = vec![0.0; nombre];
    let mut actif = None;
    if tc < total {
        let case = (tc / duree).floor() as usize;
        let p = (tc - case as f64 * duree) / duree;
        let serre = (p / 0.7).min(1.0);
        let adouci = 1.0 - (1.0 - serre).powi(3);
        if case < nombre {
            for a in avancee.iter_mut().take(case) {
                *a = 1.0;
            }
            avancee[case] = adouci;
            actif = Some(case);
        } else {
            let u = 2 * nombre - 1 - case;
            for a in avancee.iter_mut().take(u) {
                *a = 1.0;
            }
            avancee[u] = 1.0 - adouci;
            actif = Some(u);
        }
    }
    (avancee, actif)
}

fn mouvements(nombre: usize) -> Vec<Mouvement> {
    (0..nombre)
        .map(|i| {
            let i = i as f64;
            let axe = (hachage(i, 2.3) * 3.0).floor().min(2.0) as u8;
            let bas = -1.0 + 0.5 * (hachage(i, 5.9) * 4.0).floor().min(3.0);
            let sens = if hachage(i, 7.7) < 0.5 { 1.0 } else { -1.0 };
            Mouvement {
                axe,
                bas,
                haut: bas + 0.5,
                angle: sens * PI / 2.0,
            }
        })
        .collect()
}

/// Applique les tranches tournées à un point ; dit s'il est dans la tranche en mouvement.
fn appliquer(
    p: [f64; 3],
    mvts: &[Mouvement],
    avancee: &[f64],
    actif: Option<usize>,
) -> ([f64; 3], bool) {
    let [mut x, mut y, mut z] = p;
    let mut dans_actif = false;
    for (i, m) in mvts.iter().enumerate() {
        if avancee[i] <= 0.0 {
            continue;
        }
        let coord = match m.axe {
            0 => x,
            1 => y,
            _ => z,
        };
        if coord < m.bas || coord >= m.haut {
            continue;
        }
        if actif == Some(i) {
            dans_actif = true;
        }
        let (sa, ca) = (m.angle * avancee[i]).sin_cos();
        match m.axe {
            0 => {
                let y2 = y * ca - z * sa;
                z = y * sa + z * ca;
                y = y2;
            }
            1 => {
                let x2 = x * ca + z * sa;
                z = -x * sa + z * ca;
                x = x2;
            }
            _ => {
                let x2 = x * ca - y * sa;
                y = x * sa + y * ca;
                x = x2;
            }
        }
    }
    ([x, y, z], dans_actif)
}

/// Rubik : des tranches tournent par quarts, se brouillent puis se résolvent — l'agent résout.
#[must_use]
pub fn image_rubik(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let c = cote / 2.0;
    let rayon = c * 0.82;
    let pt = Projecteur::new(t * 0.55, 0.35 + 0.1 * (t * 0.9).sin(), c, c, rayon);
    let rs = echelle_rayons(cote, o.rs_pow.unwrap_or(0.6));
    let nombre = o.move_count.unwrap_or(14.0).max(0.0) as usize;
    let mvts = mouvements(nombre);
    let (avancee, actif) = cycle_de_resolution(t, nombre, 0.42, 1.2);

    let mut points = Vec::new();
    grille(
        o.lat_rings.unwrap_or(15.0),
        o.lon_density.unwrap_or(40.0),
        |lon, cos_lat, sin_lat, _| {
            let ([x, y, z], dans_actif) = appliquer(
                [cos_lat * lon.cos(), sin_lat, cos_lat * lon.sin()],
                &mvts,
                &avancee,
                actif,
            );
            let [px, py, zr] = pt.projeter(x, y, z);
            let profondeur = (zr + 1.0) / 2.0;
            // La tranche qui tourne s'encre un peu plus : la « main ».
            let (r_actif, encre_active) = if dans_actif {
                (o.r_active.unwrap_or(0.3), 0.14)
            } else {
                (0.0, 0.0)
            };
            points.push(Point {
                x: px,
                y: py,
                z: zr,
                r: (o.r_base.unwrap_or(0.6) + o.r_depth.unwrap_or(1.7) * profondeur + r_actif) * rs,
                blanc: o.ink_far.unwrap_or(0.62)
                    - o.ink_span.unwrap_or(0.54) * profondeur
                    - encre_active,
                a: None,
            });
        },
    );
    finaliser(points, Vec::new(), o.r_min)
}

/// Onde : une forme d'onde roule à travers les anneaux — l'agent écoute.
#[must_use]
pub fn image_onde(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let c = cote / 2.0;
    // L'ondulation rentre la sphère : le rayon de base (0,76) est grossi de 15 % pour que l'onde
    // paraisse de la même taille que les autres grilles.
    let rayon = c * 0.874;
    let pt = Projecteur::new(t * 0.18, 0.38, c, c, 1.0);
    let rs = echelle_rayons(cote, o.rs_pow.unwrap_or(0.6));

    let mut points = Vec::new();
    grille(
        o.rings.unwrap_or(15.0),
        o.lon_density.unwrap_or(40.0),
        |lon, cos_lat, sin_lat, ri| {
            let ri = ri as f64;
            // Deux ondes à des tempos différents : organique, jamais tout à fait pareil.
            let w = 0.62 * (t * 2.1 - ri * 0.52).sin() + 0.38 * (t * 1.27 + ri * 0.83).sin();
            let rr = rayon * (0.88 + 0.105 * w);
            let [x, y, z] = pt.projeter(
                cos_lat * lon.cos() * rr,
                sin_lat * rr,
                cos_lat * lon.sin() * rr,
            );
            let profondeur = (z / rayon + 1.0) / 2.0;
            let crete = w.max(0.0);
            points.push(Point {
                x,
                y,
                z,
                r: (o.r_base.unwrap_or(0.6) + o.r_depth.unwrap_or(1.7) * profondeur)
                    * (1.0 + 0.4 * crete)
                    * rs,
                blanc: 0.66 - 0.56 * profondeur - 0.1 * crete,
                a: None,
            });
        },
    );
    finaliser(points, Vec::new(), o.r_min)
}
