// Port du moteur de « Thinking Orbs » (Jakub Antalik, MIT, voir LICENSE-THINKING-ORBS) :
// `engine/morph.ts` de la bibliothèque d'origine. Traduit en Rust, commentaires réécrits.
//
// Les formes qui se changent : un contour pointillé passe du cercle au triangle, au carré, puis
// revient au cercle — l'agent façonne. Chaque contour est un chemin fermé paramétré par sa
// longueur (départ en haut au centre, sens horaire) ; à chaque image on mélange les deux chemins
// voisins, puis on répartit les points À ÉGALE DISTANCE sur le contour mélangé : l'espacement reste
// régulier pendant les poses comme pendant les transitions.
//
// La bibliothèque d'origine a été réglée avec un filtre « goo » (flou + seuil) ; elle peint, comme
// ici, de simples disques : la géométrie est la même, le bord est seulement plus doux. Ne pas
// réduire le rayon pour « compenser » : la marque deviendrait plus petite que le réglage.

use std::f64::consts::PI;

use super::noyau::{ImageOrbe, Point, arrondi, finaliser};
use super::reglages::Reglages;

/// Un contour fermé, échantillonné en `f` ∈ [0, 1) le long de sa longueur.
enum Contour {
    Cercle,
    Polygone {
        sommets: &'static [[f64; 2]],
        longueurs: Vec<f64>,
        total: f64,
    },
}

impl Contour {
    fn polygone(sommets: &'static [[f64; 2]]) -> Self {
        let n = sommets.len();
        let longueurs: Vec<f64> = (0..n)
            .map(|i| {
                let (a, b) = (sommets[i], sommets[(i + 1) % n]);
                (b[0] - a[0]).hypot(b[1] - a[1])
            })
            .collect();
        let total = longueurs.iter().sum();
        Self::Polygone {
            sommets,
            longueurs,
            total,
        }
    }

    fn en(&self, f: f64) -> [f64; 2] {
        match self {
            Self::Cercle => {
                let a = -PI / 2.0 + f * 2.0 * PI;
                [a.cos() * 0.24, a.sin() * 0.24]
            }
            Self::Polygone {
                sommets,
                longueurs,
                total,
            } => {
                let n = sommets.len();
                let mut reste = f * total;
                let mut i = 0;
                while i < n - 1 && reste > longueurs[i] {
                    reste -= longueurs[i];
                    i += 1;
                }
                let (a, b) = (sommets[i], sommets[(i + 1) % n]);
                let ff = if longueurs[i] != 0.0 {
                    (reste / longueurs[i]).min(1.0)
                } else {
                    0.0
                };
                [a[0] + (b[0] - a[0]) * ff, a[1] + (b[1] - a[1]) * ff]
            }
        }
    }
}

const TRIANGLE: [[f64; 2]; 3] = [[0.0, -0.26], [0.24, 0.16], [-0.24, 0.16]];
/// Cinq sommets : le carré PART lui aussi du haut au centre, comme les autres contours.
const CARRE: [[f64; 2]; 5] = [
    [0.0, -0.2],
    [0.2, -0.2],
    [0.2, 0.2],
    [-0.2, 0.2],
    [-0.2, -0.2],
];

/// Un plancher bas : des contours clairsemés restent possibles sans jamais dégénérer.
fn nombre_de_points(densite: f64) -> usize {
    arrondi(34.0 * densite).max(6.0) as usize
}

const POSE: f64 = 1.4;
const MUE: f64 = 0.9;
const ETAPE: f64 = POSE + MUE;
/// Le nombre d'échantillons du contour mélangé.
const ECHANTILLONS: usize = 160;

fn adoucir(x: f64) -> f64 {
    x * x * (3.0 - 2.0 * x)
}

/// Morph : cercle → triangle → carré → cercle.
#[must_use]
pub fn image_morph(cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    let cycle = [
        Contour::Cercle,
        Contour::polygone(&TRIANGLE),
        Contour::polygone(&CARRE),
    ];
    let k_total = cycle.len();
    let tc = t % (ETAPE * k_total as f64);
    // `shape` 0, 1 ou 2 garde un seul contour ; sinon la boucle réglée.
    let tenu = o
        .shape
        .filter(|s| *s >= 0.0 && *s < k_total as f64)
        .map(|s| s.floor() as usize);
    let k = tenu.unwrap_or_else(|| ((tc / ETAPE).floor() as usize).min(k_total - 1));
    let local = if tenu.is_some() {
        t % ETAPE
    } else {
        tc - k as f64 * ETAPE
    };
    let m = if tenu.is_none() && local > POSE {
        adoucir((local - POSE) / MUE)
    } else {
        0.0
    };
    let etalement = o.spread.unwrap_or(1.0);

    // Mélange des deux CHEMINS à m, puis mesure du contour mélangé.
    let a_chemin = &cycle[k];
    let b_chemin = if tenu.is_some() {
        a_chemin
    } else {
        &cycle[(k + 1) % k_total]
    };
    let pts: Vec<[f64; 2]> = (0..ECHANTILLONS)
        .map(|i| {
            let f = i as f64 / ECHANTILLONS as f64;
            let (a, b) = (a_chemin.en(f), b_chemin.en(f));
            [
                (a[0] + (b[0] - a[0]) * m) * etalement,
                (a[1] + (b[1] - a[1]) * m) * etalement,
            ]
        })
        .collect();
    let longueurs: Vec<f64> = (0..ECHANTILLONS)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % ECHANTILLONS]);
            (b[0] - a[0]).hypot(b[1] - a[1])
        })
        .collect();
    let total: f64 = longueurs.iter().sum();

    // Le rayon ne dépend QUE de `r_dot` ; le nombre règle les écarts. Une forme posée respire
    // un peu (pouls uniforme).
    let n = nombre_de_points(o.icon_d.unwrap_or(1.0));
    let rayon_point = o.r_dot.unwrap_or(0.021) * 1.35 * etalement;
    let pouls = 1.0 + 0.02 * (local * 3.1).sin();

    let centre = cote / 2.0;
    let mut points = Vec::with_capacity(n);
    let (mut seg, mut acc) = (0, 0.0);
    for i in 0..n {
        let cible = (i as f64 / n as f64) * total;
        while acc + longueurs[seg] < cible && seg < ECHANTILLONS - 1 {
            acc += longueurs[seg];
            seg += 1;
        }
        let (a, b) = (pts[seg], pts[(seg + 1) % ECHANTILLONS]);
        let f = if longueurs[seg] != 0.0 {
            ((cible - acc) / longueurs[seg]).min(1.0)
        } else {
            0.0
        };
        let x = (a[0] + (b[0] - a[0]) * f) * pouls;
        let y = (a[1] + (b[1] - a[1]) * f) * pouls;
        points.push(Point {
            x: centre + x * cote,
            y: centre + y * cote,
            z: 0.0,
            r: (rayon_point * cote).max(0.35),
            blanc: 0.1,
            a: None,
        });
    }
    finaliser(points, Vec::new(), o.r_min)
}
