// Port du moteur de « Thinking Orbs » (Jakub Antalik, MIT, voir LICENSE-THINKING-ORBS) :
// `engine/core.ts` de la bibliothèque d'origine. Traduit en Rust, commentaires réécrits.
//
// Les primitives partagées par toutes les formes : le point (un disque projeté), le trait (une
// arête de la constellation), l'image d'un instant, le bruit et le hachage déterministes, la
// projection orthographique (lacet + inclinaison) et la rampe d'encre.
//
// ★ Tout le calcul est en `f64`, comme les nombres de JavaScript : la géométrie se compare alors
// valeur pour valeur à celle du moteur d'origine (tests de `tests/orbe.rs`).

use std::cmp::Ordering;
use std::f64::consts::PI;

/// Un disque à peindre, déjà projeté : `z` sert à l'ordre (loin → près).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub r: f64,
    /// Valeur d'encre : 0 = l'encre la plus foncée sur fond clair ; inversée en sombre.
    pub blanc: f64,
    /// Opacité ; `None` = opaque.
    pub a: Option<f64>,
}

/// Un trait entre deux points projetés (la forme « constellation »).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    /// Même convention que [`Point::blanc`].
    pub blanc: f64,
    pub a: Option<f64>,
    /// Épaisseur du trait.
    pub w: f64,
}

/// Un instant complet : les traits d'abord, puis les points dans l'ordre de peinture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageOrbe {
    pub points: Vec<Point>,
    pub segments: Vec<Segment>,
}

/// Arrondi de JavaScript (`Math.round`) : la moitié va vers +∞, pas « loin de zéro ».
#[must_use]
pub fn arrondi(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Interpolation linéaire.
#[must_use]
pub fn interp(a: f64, b: f64, f: f64) -> f64 {
    a + (b - a) * f
}

/// Partie fractionnaire, toujours dans [0, 1).
#[must_use]
pub fn fract(x: f64) -> f64 {
    x - x.floor()
}

/// Hachage déterministe dans [0, 1) (le classique `sin · 43758`).
#[must_use]
pub fn hachage(a: f64, b: f64) -> f64 {
    let h = (a * 12.9898 + b * 78.233).sin() * 43758.5453;
    h - h.floor()
}

/// Bruit de valeur lissé sur une grille 2D : doux, déterministe, peu coûteux.
#[must_use]
pub fn bruit_valeur(x: f64, y: f64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (mut fx, mut fy) = (x - xi, y - yi);
    fx = fx * fx * (3.0 - 2.0 * fx);
    fy = fy * fy * (3.0 - 2.0 * fy);
    let coin00 = hachage(xi, yi);
    let coin10 = hachage(xi + 1.0, yi);
    let coin01 = hachage(xi, yi + 1.0);
    let coin11 = hachage(xi + 1.0, yi + 1.0);
    coin00
        + (coin10 - coin00) * fx
        + (coin01 - coin00) * fy
        + (coin00 - coin10 - coin01 + coin11) * fx * fy
}

/// La `i`-ième de `n` directions réparties sur la sphère unité (spirale de Fibonacci).
#[must_use]
pub fn direction_fibonacci(i: usize, n: usize) -> [f64; 3] {
    let angle_d_or = PI * (3.0 - 5.0_f64.sqrt());
    let (i, n) = (i as f64, n as f64);
    let y = 1.0 - (2.0 * (i + 0.5)) / n;
    let rayon = (1.0 - y * y).sqrt();
    let a = i * angle_d_or;
    [rayon * a.cos(), y, rayon * a.sin()]
}

/// L'écart angulaire signé le plus court entre deux angles, ramené dans (-π, π].
#[must_use]
pub fn ecart_angulaire(a: f64, b: f64) -> f64 {
    (a - b).sin().atan2((a - b).cos())
}

/// La caméra commune : rotation de lacet, puis inclinaison, puis projection orthographique.
#[derive(Debug, Clone, Copy)]
pub struct Projecteur {
    sin_incl: f64,
    cos_incl: f64,
    sin_lacet: f64,
    cos_lacet: f64,
    cx: f64,
    cy: f64,
    echelle: f64,
}

impl Projecteur {
    #[must_use]
    pub fn new(lacet: f64, inclinaison: f64, cx: f64, cy: f64, echelle: f64) -> Self {
        Self {
            sin_incl: inclinaison.sin(),
            cos_incl: inclinaison.cos(),
            sin_lacet: lacet.sin(),
            cos_lacet: lacet.cos(),
            cx,
            cy,
            echelle,
        }
    }

    /// `[x écran, y écran, profondeur]` (l'axe y de l'écran descend).
    #[must_use]
    pub fn projeter(&self, x: f64, y: f64, z: f64) -> [f64; 3] {
        let x1 = x * self.cos_lacet + z * self.sin_lacet;
        let z1 = -x * self.sin_lacet + z * self.cos_lacet;
        let y1 = y * self.cos_incl - z1 * self.sin_incl;
        let z2 = y * self.sin_incl + z1 * self.cos_incl;
        [self.cx + x1 * self.echelle, self.cy - y1 * self.echelle, z2]
    }
}

/// Les rayons ont été réglés pour un cadre de 300 ; une puissance < 1 les garde lisibles en petit.
#[must_use]
pub fn echelle_rayons(cote: f64, puissance: f64) -> f64 {
    (cote / 300.0).powf(puissance)
}

/// Termine une image : écarte l'invisible (opacité < 0,02), relève les rayons au plancher du
/// mode et trie les points du plus loin au plus près (tri STABLE, comme `Array.sort`).
#[must_use]
pub fn finaliser(points: Vec<Point>, segments: Vec<Segment>, r_min: Option<f64>) -> ImageOrbe {
    let plancher = r_min.unwrap_or(0.3);
    let mut visibles: Vec<Point> = points
        .into_iter()
        .filter(|p| p.a.unwrap_or(1.0) >= 0.02)
        .map(|mut p| {
            p.r = p.r.max(plancher);
            p
        })
        .collect();
    visibles.sort_by(|a, b| a.z.partial_cmp(&b.z).unwrap_or(Ordering::Equal));
    ImageOrbe {
        points: visibles,
        segments: segments
            .into_iter()
            .filter(|s| s.a.unwrap_or(1.0) >= 0.02)
            .collect(),
    }
}

/// La couleur d'une valeur d'encre, en canaux 0–1 `[r, v, b, a]`. Sans teinte : un gris. Avec
/// une teinte (l'encre du thème), la rampe de profondeur se fait SUR la teinte : vers le noir en
/// sombre, vers le blanc en clair — la profondeur se lit pareil en couleur.
#[must_use]
pub fn encre(valeur: f64, alpha: f64, sombre: bool, teinte: Option<[u8; 3]>) -> [f32; 4] {
    let w = valeur.clamp(0.0, 1.0);
    let canal = |c: f64| -> f32 {
        let v = if sombre {
            c * (1.0 - w)
        } else {
            c + (255.0 - c) * w
        };
        (arrondi(v) / 255.0) as f32
    };
    match teinte {
        None => {
            let g = (arrondi((if sombre { 1.0 - w } else { w }) * 255.0) / 255.0) as f32;
            [g, g, g, alpha as f32]
        }
        Some([r, v, b]) => [
            canal(f64::from(r)),
            canal(f64::from(v)),
            canal(f64::from(b)),
            alpha as f32,
        ],
    }
}
