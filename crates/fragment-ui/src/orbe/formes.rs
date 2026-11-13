// Port du moteur de « Thinking Orbs » (Jakub Antalik, MIT, voir LICENSE-THINKING-ORBS) :
// `engine/orbits.ts`, `engine/braid.ts` et `engine/web.ts` de la bibliothèque d'origine.
// Traduits en Rust, commentaires réécrits.
//
// Trois formes à points libres : les orbites (travaille), la tresse (tisse) et la
// constellation (relie).

use std::f64::consts::PI;

use super::noyau::{
    ImageOrbe, Point, Projecteur, Segment, bruit_valeur, direction_fibonacci, echelle_rayons,
    finaliser, fract, hachage, interp,
};
use super::reglages::Reglages;

fn compte(v: Option<f64>, defaut: f64) -> usize {
    v.unwrap_or(defaut).max(0.0) as usize
}

/// Orbites : des particules courent sur des cercles inclinés, sans noyau — l'agent travaille.
#[must_use]
pub fn image_orbites(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let c = cote / 2.0;
    let rayon = c * 0.82;
    let pt = Projecteur::new(t * 0.12, 0.3, c, c, 1.0);
    let rs = echelle_rayons(cote, o.rs_pow.unwrap_or(0.6));
    let nb_orbites = compte(o.orbit_n, 12.0);
    let nb_fantomes = compte(o.ghost_n, 40.0);
    let nb_particules = compte(o.particles, 3.0);

    let mut points = Vec::new();
    for orbite in 0..nb_orbites {
        let k = orbite as f64;
        let (h1, h2, h3) = (hachage(k, 1.7), hachage(k, 5.2), hachage(k, 8.9));
        let ro = rayon * (0.45 + 0.52 * h1);
        let theta = h1 * 2.0 * PI;
        let phi = (2.0 * h2 - 1.0).acos();
        // La normale du plan de l'orbite, puis deux axes (u, v) qui la complètent.
        let (nx, ny, nz) = (phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
        let longueur = (ny * ny + nx * nx).sqrt().max(1e-6);
        let (ux, uy, uz) = (-ny / longueur, nx / longueur, 0.0);
        let vx = ny * uz - nz * uy;
        let vy = nz * ux - nx * uz;
        let vz = nx * uy - ny * ux;
        let vitesse = (0.25 + 0.55 * h3) * if h3 > 0.5 { 1.0 } else { -1.0 };
        let sur_orbite = |a: f64| {
            let (s, co) = a.sin_cos();
            pt.projeter(
                (ux * co + vx * s) * ro,
                (uy * co + vy * s) * ro,
                (uz * co + vz * s) * ro,
            )
        };

        // Le chemin fantôme.
        for i in 0..nb_fantomes {
            let [x, y, z] = sur_orbite((i as f64 / nb_fantomes as f64) * 2.0 * PI);
            let profondeur = (z / ro + 1.0) / 2.0;
            points.push(Point {
                x,
                y,
                z,
                r: o.ghost_r.unwrap_or(0.9) * rs,
                blanc: 0.72,
                a: Some(o.ghost_a.unwrap_or(0.5) * (0.4 + 0.6 * profondeur)),
            });
        }
        // Les particules au travail.
        for m in 0..nb_particules {
            let a = t * vitesse + (m as f64 / nb_particules as f64) * 2.0 * PI + h2 * 6.0;
            let [x, y, z] = sur_orbite(a);
            let profondeur = (z / ro + 1.0) / 2.0;
            points.push(Point {
                x,
                y,
                z,
                r: (o.part_r.unwrap_or(1.2) + o.part_r_depth.unwrap_or(1.6) * profondeur) * rs,
                blanc: 0.3 - 0.22 * profondeur,
                a: None,
            });
        }
    }
    finaliser(points, Vec::new(), o.r_min)
}

/// La sphère pâle de points derrière la tresse et l'écharpe.
pub(super) fn sphere_fantome(
    points: &mut Vec<Point>,
    n: usize,
    rayon: f64,
    pt: &Projecteur,
    rs: f64,
) {
    for i in 0..n {
        let d = direction_fibonacci(i, n);
        let [x, y, z] = pt.projeter(d[0] * rayon, d[1] * rayon, d[2] * rayon);
        let profondeur = (z / rayon + 1.0) / 2.0;
        points.push(Point {
            x,
            y,
            z,
            r: 0.8 * rs,
            blanc: 0.78,
            a: Some(0.1 + 0.22 * profondeur),
        });
    }
}

/// Tresse : trois brins en hélice d'un pôle à l'autre, qui s'échangent dessus / dessous en
/// respirant sur le rayon — l'agent tisse.
#[must_use]
pub fn image_tresse(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let c = cote / 2.0;
    let rayon = c * 0.76;
    let pt = Projecteur::new(t * 0.4, 0.3, c, c, 1.0);
    let rs = echelle_rayons(cote, o.rs_pow.unwrap_or(0.6));

    let mut points = Vec::new();
    sphere_fantome(&mut points, compte(o.ghost_n, 150.0), rayon, &pt, rs);

    let nb = compte(o.strand_n, 52.0);
    let tours = o.turns.unwrap_or(3.0);
    for brin in 0..3 {
        let phase = (f64::from(brin) / 3.0) * 2.0 * PI;
        for i in 0..nb {
            // u va d'un pôle à l'autre ; la dérive fait glisser tout le brin.
            let u = (fract(i as f64 / nb as f64 + t * 0.045) * 2.0 - 1.0) * 0.96;
            let surface = (1.0 - u * u).max(0.0).sqrt();
            let fondu_bouts = ((1.0 - u.abs()) / 0.1).min(1.0);
            let a = u * PI * tours + phase;
            let tissage = 1.0 + 0.075 * (u * PI * tours * 2.0 + phase * 2.0 + t * 0.8).sin();
            let rr = surface * rayon * tissage;
            let [x, y, z] = pt.projeter(a.cos() * rr, u * rayon * tissage, a.sin() * rr);
            let profondeur = (z / rayon + 1.0) / 2.0;
            points.push(Point {
                x,
                y,
                z,
                r: (o.r_base.unwrap_or(1.2) + o.r_depth.unwrap_or(1.8) * profondeur) * rs,
                blanc: 0.55 - 0.45 * profondeur,
                a: Some(fondu_bouts * (0.45 + 0.55 * profondeur)),
            });
        }
    }
    finaliser(points, Vec::new(), o.r_min)
}

/// Constellation : des nœuds dérivent sur la sphère, les voisins proches se relient, et des
/// paquets lumineux courent entre des paires tirées au hasard — l'agent relie.
#[must_use]
pub fn image_toile(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let c = cote / 2.0;
    let rayon = c * 0.8 * o.spread.unwrap_or(1.0);
    // L'échelle du projecteur porte le rayon : les nœuds restent unitaires et les distances
    // ci-dessous se mesurent sur la sphère unité.
    let pt = Projecteur::new(t * 0.12, 0.32, c, c, rayon);
    let rs = echelle_rayons(cote, o.rs_pow.unwrap_or(0.6));
    let nb = compte(o.node_n, 30.0);
    let seuil = o.thr.unwrap_or(0.72);
    let node_r = o.node_r.unwrap_or(1.4);
    let node_r_depth = o.node_r_depth.unwrap_or(1.8);

    // Nœuds : la spirale de Fibonacci, déplacée par un bruit lent, ramenée sur la sphère.
    let noeuds: Vec<[f64; 3]> = (0..nb)
        .map(|i| {
            let d = direction_fibonacci(i, nb);
            let k = i as f64;
            let x = d[0] + 0.3 * (bruit_valeur(k * 0.31 + 9.0, t * 0.24) - 0.5) * 2.0;
            let y = d[1] + 0.3 * (bruit_valeur(k * 0.53 + 27.0, t * 0.21) - 0.5) * 2.0;
            let z = d[2] + 0.3 * (bruit_valeur(k * 0.77 + 55.0, t * 0.27) - 0.5) * 2.0;
            let l = (x * x + y * y + z * z).sqrt();
            [x / l, y / l, z / l]
        })
        .collect();

    // Arêtes entre voisins proches, opacité selon la proximité et la profondeur.
    let mut segments = Vec::new();
    for i in 0..nb {
        for j in (i + 1)..nb {
            let (a, b) = (noeuds[i], noeuds[j]);
            let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist >= seuil {
                continue;
            }
            let [x1, y1, z1] = pt.projeter(a[0], a[1], a[2]);
            let [x2, y2, z2] = pt.projeter(b[0], b[1], b[2]);
            let profondeur = ((z1 + z2) / 2.0 + 1.0) / 2.0;
            segments.push(Segment {
                x1,
                y1,
                x2,
                y2,
                blanc: 0.42,
                a: Some((1.0 - dist / seuil) * (0.3 + 0.55 * profondeur)),
                w: (o.line_w.unwrap_or(0.8) * rs).max(0.6),
            });
        }
    }

    let mut points = Vec::new();
    for (i, n) in noeuds.iter().enumerate() {
        let [x, y, z] = pt.projeter(n[0], n[1], n[2]);
        let profondeur = (z + 1.0) / 2.0;
        let pouls = 1.0 + 0.25 * (t * 1.4 + i as f64 * 2.7).sin();
        points.push(Point {
            x,
            y,
            z,
            r: (node_r + node_r_depth * profondeur) * pouls * rs,
            blanc: 0.55 - 0.45 * profondeur,
            a: None,
        });
    }

    // Les signaux : chaque paquet change de paire à chaque tranche de temps.
    let nb_signaux = compte(o.signals, 5.0);
    for s in 0..nb_signaux {
        let s = s as f64;
        let horloge = t * 0.55 + s * 7.31;
        let tranche = horloge.floor();
        let ia = (hachage(tranche, s * 3.1 + 1.7) * nb as f64).floor() as usize;
        let ib = (hachage(tranche, s * 5.7 + 4.2) * nb as f64).floor() as usize;
        if ia == ib || ia >= nb || ib >= nb {
            continue;
        }
        let f = fract(horloge);
        let (a, b) = (noeuds[ia], noeuds[ib]);
        let x = interp(a[0], b[0], f);
        let y = interp(a[1], b[1], f);
        let z = interp(a[2], b[2], f);
        let l = (x * x + y * y + z * z).sqrt().max(1e-6);
        let [px, py, zr] = pt.projeter(x / l, y / l, z / l);
        let profondeur = (zr + 1.0) / 2.0;
        points.push(Point {
            x: px,
            y: py,
            z: zr,
            r: (node_r * 1.5 + node_r_depth * profondeur) * rs,
            blanc: 0.05,
            a: Some(0.5 + 0.5 * profondeur),
        });
    }

    finaliser(points, segments, o.r_min)
}
