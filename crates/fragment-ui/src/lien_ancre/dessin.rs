// Port de app/src/ui/views/doc-widget/anchor-link.css et du dessin de `LinkMarker` (anchorLink.ts)
//! Le dessin du lien : une polyligne en pointillés (3 de trait, 3 de blanc, extrémités plates,
//! épaisseur 2) et un petit cercle de 2,5 au bout, tous deux en `color_accent_2`.
//!
//! ★ GPUI ne sait pas tirer un trait pointillé : les pointillés sont calculés ([`pointilles`], pur)
//! et chacun est peint comme un petit trait. Le pointillé se poursuit d'un sommet à l'autre, comme
//! le `stroke-dasharray` d'une polyligne SVG, et part du premier point.

use fragment_annotation::anchor_link::Point;
use fragment_theme::Couleur;
use gpui_kit::{Bounds, Corners, PathBuilder, Pixels, Window, fill, point, px, size};

use super::modele::{EPAISSEUR, POINTILLE, RAYON_BOUT};
use crate::theme_global::couleur;

/// Découpe `pts` en traits de `trait_` séparés de `blanc` (le dernier peut être plus court).
pub fn pointilles(pts: &[Point], trait_: f64, blanc: f64) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    if pts.len() < 2 || trait_ <= 0.0 {
        return out;
    }
    let periode = trait_ + blanc;
    // La distance parcourue depuis le départ, modulo la période : < trait_ → on est dans un trait.
    let mut phase = 0.0_f64;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let long = dx.hypot(dy);
        if long <= f64::EPSILON {
            continue;
        }
        let at = |d: f64| Point {
            x: a.x + dx * d / long,
            y: a.y + dy * d / long,
        };
        let mut d = 0.0;
        while d < long - 1e-9 {
            if phase < trait_ - 1e-9 {
                let fin = (d + trait_ - phase).min(long);
                out.push((at(d), at(fin)));
                phase += fin - d;
                d = fin;
            } else {
                let fin = (d + periode - phase).min(long);
                phase += fin - d;
                d = fin;
            }
            if phase >= periode - 1e-9 {
                phase = 0.0;
            }
        }
    }
    out
}

/// Peint le lien : `pts` sont en repère du plan, `origine` est le coin du plan à l'écran, `echelle`
/// le facteur plan → écran (trait et pointillés restent à taille d'écran).
pub fn peindre(
    window: &mut Window,
    origine: gpui_kit::Point<Pixels>,
    pts: &[Point],
    teinte: Couleur,
    echelle: f32,
) {
    let c = couleur(teinte);
    let pt = |p: Point| point(origine.x + px(p.x as f32), origine.y + px(p.y as f32));
    // Les pointillés se comptent en pixels ÉCRAN : on met le tracé à l'échelle avant de le découper.
    let k = f64::from(echelle);
    let ecran: Vec<Point> = pts
        .iter()
        .map(|p| Point {
            x: p.x * k,
            y: p.y * k,
        })
        .collect();
    let (trait_, blanc) = (f64::from(POINTILLE.0), f64::from(POINTILLE.1));
    for (a, b) in pointilles(&ecran, trait_, blanc) {
        let mut p = PathBuilder::stroke(px(EPAISSEUR));
        p.move_to(pt(a));
        p.line_to(pt(b));
        if let Ok(chemin) = p.build() {
            window.paint_path(chemin, c);
        }
    }
    if let Some(&bout) = ecran.last() {
        let r = px(RAYON_BOUT);
        let centre = pt(bout);
        let boite = Bounds::new(point(centre.x - r, centre.y - r), size(r * 2.0, r * 2.0));
        let mut q = fill(boite, c);
        q.corner_radii = Corners::all(r);
        window.paint_quad(q);
    }
}
