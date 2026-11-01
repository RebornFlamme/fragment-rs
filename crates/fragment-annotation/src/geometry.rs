// Port de app/src/ui/views/annotation/geometry.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  La géométrie du tracé — pure, sans DOM, sans éditeur.
//
//  ★ Toutes les coordonnées sont des ÉCARTS au glyphe d'ancrage (dx, dy), pas des
//    positions absolues : un trait ne connaît pas sa place dans le document,
//    seulement sa forme relative à son ancre. C'est ce qui le fait suivre le texte.
// ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

/// Un point du tracé, en écart au glyphe d'ancrage.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub dx: f64,
    pub dy: f64,
}

/// Raccourci de construction : `p(dx, dy)`.
pub fn p(dx: f64, dy: f64) -> Point {
    Point { dx, dy }
}

/// La boîte englobante d'un tracé, dans le même repère que ses points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// Boîte englobante ; nulle sur un tracé vide.
pub fn bounds(points: &[Point]) -> Bounds {
    if points.is_empty() {
        return Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        };
    }
    let mut b = Bounds {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for q in points {
        b.min_x = b.min_x.min(q.dx);
        b.min_y = b.min_y.min(q.dy);
        b.max_x = b.max_x.max(q.dx);
        b.max_y = b.max_y.max(q.dy);
    }
    b
}

/// Tolérance par défaut de [`simplify`] (`epsilon = 0.6px`).
pub const EPSILON_PAR_DEFAUT: f64 = 0.6;

/// Simplification Ramer–Douglas–Peucker : jette les points qui n'apportent rien
/// à la forme, en gardant toujours les deux extrémités.
///
/// ★ Un `pointermove` tire 60 à 240 points par seconde, dont l'écrasante majorité
///   est à moins d'un pixel de la corde ; sans ce filtre un trait de deux secondes
///   pèse ~400 points. Itératif (pile explicite) : pas de débordement de pile sur
///   les tracés très denses.
pub fn simplify(points: &[Point], epsilon: f64) -> Vec<Point> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;

    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((first, last)) = stack.pop() {
        if last <= first + 1 {
            continue;
        }
        let mut worst = 0.0;
        let mut worst_index: Option<usize> = None;
        for i in first + 1..last {
            let d = distance_to_segment(points[i], points[first], points[last]);
            if d > worst {
                worst = d;
                worst_index = Some(i);
            }
        }
        if let Some(i) = worst_index.filter(|_| worst > epsilon) {
            keep[i] = true;
            stack.push((first, i));
            stack.push((i, last));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(q, k)| k.then_some(*q))
        .collect()
}

/// Arrondi à 2 décimales puis affichage « JS » (`3`, pas `3.0` ; jamais `-0`).
fn n(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    // `+ 0.0` : -0.0 + 0.0 = +0.0 (Number.toString() de JS écrit « 0 »).
    format!("{}", r + 0.0)
}

/// Le `d` d'un `<path>` SVG, lissé et TRANSLATÉ dans le repère de la boîte.
///
/// Le lissage passe par les MILIEUX des segments comme points d'ancrage de courbes
/// quadratiques, chaque point d'origine servant de point de contrôle : courbe C¹,
/// pas de cassure aux jointures. `origin` est le coin haut-gauche de la boîte.
pub fn to_path_data(points: &[Point], origin: Point) -> String {
    if points.is_empty() {
        return String::new();
    }
    let x = |q: Point| q.dx - origin.dx;
    let y = |q: Point| q.dy - origin.dy;

    // ★ Un point isolé (un simple clic) : un segment nul. Avec stroke-linecap:round
    //   ça donne le point rond attendu — un `M` seul ne dessinerait rien.
    if points.len() == 1 {
        let q = points[0];
        return format!("M {} {} L {} {}", n(x(q)), n(y(q)), n(x(q)), n(y(q)));
    }
    let mut d = format!("M {} {}", n(x(points[0])), n(y(points[0])));
    for i in 1..points.len() - 1 {
        let mid_x = (x(points[i]) + x(points[i + 1])) / 2.0;
        let mid_y = (y(points[i]) + y(points[i + 1])) / 2.0;
        d.push_str(&format!(
            " Q {} {} {} {}",
            n(x(points[i])),
            n(y(points[i])),
            n(mid_x),
            n(mid_y)
        ));
    }
    let last = points[points.len() - 1];
    d.push_str(&format!(" L {} {}", n(x(last)), n(y(last))));
    d
}

/// La distance du point (px, py) à la polyligne — le hit-test de la gomme.
///
/// ★ Géométrique et pas par pointer-events : un marker peut être détruit à tout
///   instant par le cycle de measure de l'éditeur, et son `pointerleave` ne part
///   alors jamais. La gomme interroge le MODÈLE, jamais le DOM.
pub fn distance_to_stroke(points: &[Point], px: f64, py: f64) -> f64 {
    if points.is_empty() {
        return f64::INFINITY;
    }
    let probe = p(px, py);
    if points.len() == 1 {
        return distance(probe, points[0]);
    }
    points
        .windows(2)
        .map(|w| distance_to_segment(probe, w[0], w[1]))
        .fold(f64::INFINITY, f64::min)
}

fn distance(a: Point, b: Point) -> f64 {
    (a.dx - b.dx).hypot(a.dy - b.dy)
}

/// Distance d'un point au SEGMENT [a, b] — bornée aux extrémités, pas à la droite.
fn distance_to_segment(q: Point, a: Point, b: Point) -> f64 {
    let vx = b.dx - a.dx;
    let vy = b.dy - a.dy;
    let len2 = vx * vx + vy * vy;
    if len2 == 0.0 {
        return distance(q, a);
    }
    // Projection scalaire ramenée dans [0, 1] : au-delà, le plus proche est l'extrémité.
    let t = (((q.dx - a.dx) * vx + (q.dy - a.dy) * vy) / len2).clamp(0.0, 1.0);
    distance(q, p(a.dx + t * vx, a.dy + t * vy))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proche(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.005
    }

    // ── bounds ──
    #[test]
    fn rend_une_boite_nulle_sur_un_trace_vide() {
        assert_eq!(
            bounds(&[]),
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0
            }
        );
    }

    #[test]
    fn englobe_tous_les_points_coordonnees_negatives_comprises() {
        assert_eq!(
            bounds(&[p(-5.0, 3.0), p(10.0, -2.0), p(0.0, 8.0)]),
            Bounds {
                min_x: -5.0,
                min_y: -2.0,
                max_x: 10.0,
                max_y: 8.0
            }
        );
    }

    // ── simplify ──
    #[test]
    fn laisse_intact_un_trace_de_deux_points_ou_moins() {
        let e = EPSILON_PAR_DEFAUT;
        assert_eq!(simplify(&[], e), vec![]);
        assert_eq!(simplify(&[p(1.0, 1.0)], e), vec![p(1.0, 1.0)]);
        assert_eq!(
            simplify(&[p(0.0, 0.0), p(9.0, 9.0)], e),
            vec![p(0.0, 0.0), p(9.0, 9.0)]
        );
    }

    #[test]
    fn jette_les_points_alignes_et_garde_les_deux_extremites() {
        let droite = [
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(2.0, 0.0),
            p(3.0, 0.0),
            p(4.0, 0.0),
        ];
        assert_eq!(
            simplify(&droite, EPSILON_PAR_DEFAUT),
            vec![p(0.0, 0.0), p(4.0, 0.0)]
        );
    }

    #[test]
    fn garde_le_point_qui_porte_vraiment_la_forme() {
        // Un pic de 10px au milieu : bien au-delà de la tolérance.
        let pic = [p(0.0, 0.0), p(5.0, 10.0), p(10.0, 0.0)];
        assert_eq!(simplify(&pic, EPSILON_PAR_DEFAUT), pic.to_vec());
    }

    #[test]
    fn respecte_la_tolerance_donnee() {
        // Un écart de 0.3px : sous le défaut (0.6), donc effacé…
        let presque_droit = [p(0.0, 0.0), p(5.0, 0.3), p(10.0, 0.0)];
        assert_eq!(
            simplify(&presque_droit, EPSILON_PAR_DEFAUT),
            vec![p(0.0, 0.0), p(10.0, 0.0)]
        );
        // …mais conservé si on resserre la tolérance.
        assert_eq!(simplify(&presque_droit, 0.1), presque_droit.to_vec());
    }

    #[test]
    fn reduit_massivement_un_trace_dense_sans_perdre_ses_bornes() {
        // 400 points le long d'une courbe douce : ce que produit un vrai geste.
        let dense: Vec<Point> = (0..400)
            .map(|i| p(i as f64 * 0.5, (i as f64 / 40.0).sin() * 20.0))
            .collect();
        let simple = simplify(&dense, EPSILON_PAR_DEFAUT);
        assert!(simple.len() < dense.len() / 4);
        assert_eq!(simple[0], dense[0]);
        assert_eq!(simple[simple.len() - 1], dense[dense.len() - 1]);
    }

    // ── toPathData ──
    #[test]
    fn rend_une_chaine_vide_sur_un_trace_vide() {
        assert_eq!(to_path_data(&[], p(0.0, 0.0)), "");
    }

    #[test]
    fn rend_un_segment_nul_pour_un_point_isole_le_clic_simple() {
        assert_eq!(to_path_data(&[p(3.0, 4.0)], p(0.0, 0.0)), "M 3 4 L 3 4");
    }

    #[test]
    fn translate_dans_le_repere_de_la_boite() {
        let d = to_path_data(&[p(10.0, 20.0), p(30.0, 20.0)], p(10.0, 20.0));
        assert!(d.starts_with("M 0 0"));
        assert!(!d.contains("10 20"));
    }

    #[test]
    fn lisse_par_courbes_quadratiques_et_finit_sur_le_dernier_point() {
        let d = to_path_data(&[p(0.0, 0.0), p(10.0, 10.0), p(20.0, 0.0)], p(0.0, 0.0));
        assert!(d.contains('Q'));
        assert!(d.ends_with("L 20 0"));
    }

    // ── distanceToStroke ──
    #[test]
    fn rend_l_infini_sur_un_trace_vide() {
        assert_eq!(distance_to_stroke(&[], 0.0, 0.0), f64::INFINITY);
    }

    #[test]
    fn mesure_la_distance_au_segment_pas_a_la_droite_qui_le_porte() {
        let segment = [p(0.0, 0.0), p(10.0, 0.0)];
        // Au-dessus du milieu : la perpendiculaire.
        assert!(proche(distance_to_stroke(&segment, 5.0, 3.0), 3.0));
        // Au-delà de l'extrémité : la distance au BOUT, pas 3.
        assert!(proche(distance_to_stroke(&segment, 14.0, 3.0), 5.0));
    }

    #[test]
    fn rend_la_plus_petite_distance_parmi_tous_les_segments() {
        let coude = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)];
        assert!(proche(distance_to_stroke(&coude, 12.0, 5.0), 2.0));
    }

    #[test]
    fn gere_le_trace_reduit_a_un_point() {
        assert!(proche(distance_to_stroke(&[p(0.0, 0.0)], 3.0, 4.0), 5.0));
    }
}
