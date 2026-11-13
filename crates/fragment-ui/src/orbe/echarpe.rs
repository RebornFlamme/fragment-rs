// Port du moteur de « Thinking Orbs » (Jakub Antalik, MIT, voir LICENSE-THINKING-ORBS) :
// `engine/ribbon.ts` de la bibliothèque d'origine. Traduit en Rust, commentaires réécrits.
//
// L'écharpe : des brins parallèles ondulent le long d'un grand cercle — l'agent compose.
// Vue de face (`face_on`), la même géométrie fait l'anneau qui respire : l'ondulation passe
// alors sur le rayon dans le plan, et l'anneau gonfle et se pince au lieu de flotter.

use std::f64::consts::PI;

use super::formes::sphere_fantome;
use super::noyau::{ImageOrbe, Point, Projecteur, arrondi, echelle_rayons, finaliser};
use super::reglages::Reglages;

/// L'écharpe (et l'anneau, avec `face_on`).
#[must_use]
pub fn image_echarpe(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let c = cote / 2.0;
    let rayon = c * 0.78;
    // `spin` règle la culbute 3D ; à 0, l'orientation de la bande est figée.
    let rotation = o.spin.unwrap_or(1.0);
    let incl_camera = 0.3;
    let pt = Projecteur::new(t * 0.1 * rotation, incl_camera, c, c, 1.0);
    let rs = echelle_rayons(cote, o.rs_pow.unwrap_or(0.6));
    let de_face = o.face_on.is_some_and(|v| v != 0.0);

    let mut points = Vec::new();
    sphere_fantome(
        &mut points,
        o.ghost_n.unwrap_or(150.0).max(0.0) as usize,
        rayon,
        &pt,
        rs,
    );

    // Le plan de la bande. La projection écrase son cercle de cos(ta + inclinaison caméra) ; de
    // face, ta = -inclinaison annule ce terme : un vrai cercle au lieu d'une ellipse.
    let lacet = t * 0.24 * rotation;
    let ta = if de_face {
        -incl_camera
    } else {
        0.55 + 0.3 * (t * 0.18).sin() * rotation
    };
    let (ux, uy, uz) = (lacet.cos(), 0.0, lacet.sin());
    let (vx, vy, vz) = (-uz * ta.sin(), ta.cos(), ux * ta.sin());
    // La normale n = u × v.
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;

    // De face, les lobes dépassent le rayon : le rayon de base rentre d'une bonne part de
    // l'amplitude, pour que la silhouette reste dans le cadre même très déformée.
    let wob_mul = o.wob_mul.unwrap_or(1.0);
    let amplitude = 0.23 * wob_mul;
    let rayon_base = if de_face {
        rayon / (1.0 + 0.85 * amplitude)
    } else {
        rayon
    };

    let nb_voies = arrondi(o.lanes.unwrap_or(5.0) * o.band_mul.unwrap_or(1.0)).max(1.0);
    let segs = o.segs.unwrap_or(88.0).max(0.0) as usize;
    let milieu = (nb_voies - 1.0) / 2.0;
    for voie in 0..(nb_voies as usize) {
        let w = voie as f64;
        let decalage_voie = (w - milieu) * 0.075;
        let bord = (w - milieu).abs() / milieu.max(1.0);
        for k in 0..segs {
            let a = (k as f64 / segs as f64) * 2.0 * PI;
            // Deux ondes qui voyagent le long de la bande ; `wob_mul` à 0 = bande nette.
            let ondulation = (0.16 * (a * 3.0 - t * 1.7 + w * 0.22).sin()
                + 0.07 * (a * 5.0 + t * 1.1).sin())
                * wob_mul;
            // ★ Une ondulation selon la normale serait annulée par la renormalisation (le point
            // retombe sur la sphère) : de face, c'est le RAYON qui ondule ; l'écharpe garde
            // l'ondulation hors du plan.
            let (radial, decalage) = if de_face {
                (1.0 + ondulation, decalage_voie)
            } else {
                (1.0, decalage_voie + ondulation)
            };
            let (s, co) = a.sin_cos();
            let x = ux * co + vx * s + nx * decalage;
            let y = uy * co + vy * s + ny * decalage;
            let z = uz * co + vz * s + nz * decalage;
            let l = (x * x + y * y + z * z).sqrt();
            let rr = rayon_base * radial;
            let [px, py, zr] = pt.projeter((x / l) * rr, (y / l) * rr, (z / l) * rr);
            let profondeur = (zr / rayon + 1.0) / 2.0;
            points.push(Point {
                x: px,
                y: py,
                z: zr,
                r: (o.r_base.unwrap_or(1.1) + o.r_depth.unwrap_or(1.7) * profondeur)
                    * (1.0 - 0.25 * bord)
                    * rs,
                blanc: 0.52 - 0.44 * profondeur + 0.18 * bord,
                a: Some(0.4 + 0.6 * profondeur),
            });
        }
    }
    finaliser(points, Vec::new(), o.r_min)
}
