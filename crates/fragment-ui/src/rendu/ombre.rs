//! Les ombres des surfaces `position: fixed` (modale, palette) peintes comme Chromium les compose
//! (`fr-p4.1`, ADR-013 §2, spike S8 cause 1).
//!
//! ★ Chromium rastérise le conteneur fixe de la modale (son voile ET l'ombre de la boîte) dans un
//! calque RGBA8 PRÉMULTIPLIÉ, quantifié à l'octet, puis compose ce calque sur l'espace de travail :
//! `L + fond × (1 − A)`. GPUI peint le voile, l'arrondit, puis l'ombre par-dessus : l'arrondi
//! intermédiaire n'est pas le même (étalement de +1,6 px en sombre, −0,6 en clair, signe inversé).
//! Ici, l'ombre n'est plus le `BoxShadow` de GPUI : c'est une image calculée sur le CPU
//! (gaussienne exacte d'écart-type `flou / 2`), où voile et ombre sont quantifiés comme dans le
//! calque, puis peinte par le voile lui-même, avant la boîte. Le voile ne se peint plus qu'autour de
//! l'image. La boîte déclare sa géométrie au prépeint ([`ombrer_au_calque`]), qui précède toute
//! peinture ; le voile ([`voile`]) la relit à sa peinture.
//!
//! Le reste (forme du flou de Skia sur GPU, 0,1 à 0,2 px) est une dette chiffrée de l'ADR-013.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use fragment_theme::{Couleur, Ombre};
use gpui_kit::{
    App, Bounds, Corners, IntoElement, ParentElement, Pixels, RenderImage, Styled, Window, canvas,
    fill, point, px, size,
};

use crate::echelle::u;
use crate::flottant::ombrer;
use crate::theme_global::couleur;

/// Une couche d'ombre en pixels PHYSIQUES : décalage, écart-type, étendue, couleur (opacité en octet,
/// comme Skia la reçoit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoucheCpu {
    pub dx: f32,
    pub dy: f32,
    pub sigma: f32,
    pub etendue: f32,
    pub couleur: Couleur,
}

/// Ce qu'il faut pour rastériser l'ombre d'une boîte, en pixels physiques. `fx`, `fy` : la partie
/// fractionnaire de l'origine de la boîte (l'image est posée sur la grille des pixels).
#[derive(Debug, Clone, PartialEq)]
pub struct Geometrie {
    pub largeur: f32,
    pub hauteur: f32,
    pub rayon: f32,
    pub fx: f32,
    pub fy: f32,
    pub couches: Vec<CoucheCpu>,
    /// L'opacité du voile du calque, en octet (128 pour `rgba(0,0,0,.5)`).
    pub voile: u8,
}

/// Une image d'ombre : son coin (en pixels physiques, relatif au pixel de l'origine de la boîte),
/// sa taille et ses pixels BGRA à alpha DROIT (le mélange des images de GPUI : `SRC_ALPHA`).
#[derive(Debug, Clone)]
pub struct ImageOmbre {
    pub x0: i32,
    pub y0: i32,
    pub largeur: u32,
    pub hauteur: u32,
    pub bgra: Vec<u8>,
}

/// `erf`, Abramowitz et Stegun 7.1.26 (erreur ≤ 1,5 × 10⁻⁷) : `std` n'en a pas.
fn erf(x: f64) -> f64 {
    let s = x.signum();
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let p = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    s * (1.0 - p * (-x * x).exp())
}

/// Le noyau gaussien d'écart-type `sigma`, intégré sur chaque pixel (somme 1), rayon `⌈3σ⌉ + 1`.
fn noyau(sigma: f64) -> Vec<f64> {
    if sigma <= 0.0 {
        return vec![1.0];
    }
    let r = (3.0 * sigma).ceil() as i64 + 1;
    let phi = |x: f64| 0.5 * (1.0 + erf(x / (sigma * std::f64::consts::SQRT_2)));
    let mut k: Vec<f64> = (-r..=r)
        .map(|i| phi(i as f64 + 0.5) - phi(i as f64 - 0.5))
        .collect();
    let somme: f64 = k.iter().sum();
    for v in &mut k {
        *v /= somme;
    }
    k
}

/// La couverture d'un rectangle arrondi au centre du pixel (`0,5 − distance`, comme Skia et GPUI).
fn couverture(px: f64, py: f64, b: [f64; 4], r: f64) -> f64 {
    let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
    let r = r.min(b[2] / 2.0).min(b[3] / 2.0).max(0.0);
    let qx = (px - cx).abs() - (b[2] / 2.0 - r);
    let qy = (py - cy).abs() - (b[3] / 2.0 - r);
    let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// Arrondi au plus proche d'une valeur en [0, 255].
fn octet(v: f64) -> u8 {
    // Borné : la conversion ne tronque rien.
    (v + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// Rastérise l'ombre de `g` dans le calque de Chromium : voile puis ombre, prémultipliés et
/// quantifiés à l'octet, rendus en alpha droit pour GPUI (`rgb = L × 255 / A`).
pub fn rasteriser(g: &Geometrie) -> ImageOmbre {
    // L'étendue de l'image : l'union des couches, gonflées de leur noyau.
    let (mut x0, mut y0, mut x1, mut y1) =
        (0.0f64, 0.0f64, f64::from(g.largeur), f64::from(g.hauteur));
    for c in &g.couches {
        let m = f64::from(c.etendue) + (3.0 * f64::from(c.sigma)).ceil() + 2.0;
        x0 = x0.min(f64::from(g.fx + c.dx) - m);
        y0 = y0.min(f64::from(g.fy + c.dy) - m);
        x1 = x1.max(f64::from(g.fx + c.dx + g.largeur) + m);
        y1 = y1.max(f64::from(g.fy + c.dy + g.hauteur) + m);
    }
    let (ix0, iy0) = (x0.floor() as i32, y0.floor() as i32);
    let (l, h) = (
        (x1.ceil() as i32 - ix0) as usize,
        (y1.ceil() as i32 - iy0) as usize,
    );
    // Prémultiplié flottant : (r, g, b, a) de l'ombre seule, couches composées l'une sur l'autre.
    let mut pre = vec![[0.0f64; 4]; l * h];
    for c in g.couches.iter().rev() {
        let e = f64::from(c.etendue);
        let b = [
            f64::from(g.fx + c.dx) - e,
            f64::from(g.fy + c.dy) - e,
            f64::from(g.largeur) + 2.0 * e,
            f64::from(g.hauteur) + 2.0 * e,
        ];
        let r = f64::from(g.rayon) + e;
        let k = noyau(f64::from(c.sigma));
        let rk = (k.len() / 2) as i64;
        // Convolution 1D de `src` (longueur `n`) par le noyau, bords à zéro.
        let flou = |src: &[f64], n: usize| -> Vec<f64> {
            (0..n)
                .map(|i| {
                    k.iter()
                        .enumerate()
                        .filter_map(|(j, kv)| {
                            let s = i as i64 + j as i64 - rk;
                            (s >= 0 && (s as usize) < n).then(|| kv * src[s as usize])
                        })
                        .sum()
                })
                .collect()
        };
        // ★ Le flou gaussien est séparable, et les rangées du masque d'un rectangle arrondi ne
        // prennent que quelques formes (une par rangée de coin, plus la rangée pleine). Le masque
        // est donc `Σ_t M_t(x) · 1[forme(y) = t]`, et son flou `Σ_t flou(M_t)(x) · flou(1_t)(y)` :
        // une vingtaine de termes par pixel au lieu de deux noyaux de 145 (modale : 2 s → ~0,1 s
        // en debug, mesuré par `cout_d_une_image_de_modale`).
        let mut formes: Vec<(Vec<f64>, Vec<f64>)> = Vec::new();
        for y in 0..h {
            let py = f64::from(iy0 + y as i32) + 0.5;
            let rangee: Vec<f64> = (0..l)
                .map(|x| couverture(f64::from(ix0 + x as i32) + 0.5, py, b, r))
                .collect();
            if rangee.iter().all(|v| *v == 0.0) {
                continue;
            }
            match formes.iter_mut().find(|(m, _)| *m == rangee) {
                Some((_, ind)) => ind[y] = 1.0,
                None => {
                    let mut ind = vec![0.0; h];
                    ind[y] = 1.0;
                    formes.push((rangee, ind));
                }
            }
        }
        let termes: Vec<(Vec<f64>, Vec<f64>)> = formes
            .iter()
            .map(|(m, ind)| (flou(m, l), flou(ind, h)))
            .collect();
        let mut m = vec![0.0f64; l * h];
        for (hx, vy) in &termes {
            for (y, v) in vy.iter().enumerate() {
                if *v == 0.0 {
                    continue;
                }
                for (dst, hv) in m[y * l..(y + 1) * l].iter_mut().zip(hx) {
                    *dst += hv * v;
                }
            }
        }
        let a = f64::from(c.couleur.a) / 255.0;
        let rgb = [c.couleur.r, c.couleur.g, c.couleur.b].map(|v| f64::from(v) / 255.0);
        for (p, mv) in pre.iter_mut().zip(m) {
            let s = a * mv;
            for k in 0..3 {
                p[k] = rgb[k] * s + p[k] * (1.0 - s);
            }
            p[3] = s + p[3] * (1.0 - s);
        }
    }
    let v = f64::from(g.voile) / 255.0;
    let mut bgra = vec![0u8; l * h * 4];
    for (i, p) in pre.iter().enumerate() {
        // Le calque : le voile (noir) sous l'ombre, quantifié à l'octet.
        let aa = octet(255.0 * (v + p[3] * (1.0 - v)));
        let lc = [0, 1, 2].map(|k| octet(255.0 * p[k]));
        let droit = |c: u8| -> u8 {
            if aa == 0 {
                0
            } else {
                octet(f64::from(c) * 255.0 / f64::from(aa))
            }
        };
        bgra[i * 4] = droit(lc[2]);
        bgra[i * 4 + 1] = droit(lc[1]);
        bgra[i * 4 + 2] = droit(lc[0]);
        bgra[i * 4 + 3] = aa;
    }
    ImageOmbre {
        x0: ix0,
        y0: iy0,
        largeur: l as u32,
        hauteur: h as u32,
        bgra,
    }
}

/// La clé du cache : la géométrie au 1/64 de pixel près, couleurs et voile compris.
fn cle(g: &Geometrie) -> Vec<i64> {
    let q = |v: f32| (f64::from(v) * 64.0).round() as i64;
    let mut c = vec![
        q(g.largeur),
        q(g.hauteur),
        q(g.rayon),
        q(g.fx),
        q(g.fy),
        i64::from(g.voile),
    ];
    for k in &g.couches {
        c.extend([q(k.dx), q(k.dy), q(k.sigma), q(k.etendue)]);
        c.push(i64::from(u32::from_be_bytes([
            k.couleur.r,
            k.couleur.g,
            k.couleur.b,
            k.couleur.a,
        ])));
    }
    c
}

/// Ce que la boîte déclare au prépeint et que le voile relit : sa boîte de bordure (pixels
/// logiques) et la géométrie de son ombre.
#[derive(Debug, Clone)]
struct Pose {
    origine: (f32, f32),
    geometrie: Geometrie,
}

/// Une image prête pour GPUI et son coin relatif.
type Pret = (Arc<RenderImage>, i32, i32, u32, u32);

thread_local! {
    /// La boîte posée dans l'image en cours (prise par le voile à sa peinture).
    static POSE: RefCell<Option<Pose>> = const { RefCell::new(None) };
    /// Les images d'ombre déjà calculées, par géométrie.
    static CACHE: RefCell<HashMap<Vec<i64>, Pret>> = RefCell::new(HashMap::new());
}

fn image(g: &Geometrie) -> Option<Pret> {
    let k = cle(g);
    if let Some(p) = CACHE.with(|c| c.borrow().get(&k).cloned()) {
        return Some(p);
    }
    let img = rasteriser(g);
    let tampon = image::RgbaImage::from_raw(img.largeur, img.hauteur, img.bgra)?;
    let rendu = Arc::new(RenderImage::new(vec![image::Frame::new(tampon)]));
    let pret = (rendu, img.x0, img.y0, img.largeur, img.hauteur);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        // Une poignée de tailles au plus : la fenêtre redimensionnée ne fait pas grossir le cache.
        if c.len() > 16 {
            c.clear();
        }
        c.insert(k, pret.clone());
    });
    Some(pret)
}

/// Comme [`ombrer`] (ombre du jeton, arrondi, déclaration pour `fragment-pixels`), mais l'ombre
/// n'est PAS peinte par GPUI : elle est posée pour le [`voile`] du calque, qui la peint avant la
/// boîte. `voile` est la couleur du voile (son opacité en octet entre dans le calque).
pub fn ombrer_au_calque<E: Styled + ParentElement>(
    e: E,
    cx: &App,
    liste: &[Ombre],
    jeton: &'static str,
    rayon: Pixels,
    bordure: Pixels,
    voile: Couleur,
) -> E {
    // Les couches en pixels LOGIQUES (zoom compris), mises au facteur d'échelle au prépeint.
    let l = |v: f32| f32::from(u(cx, v));
    let couches: Rc<Vec<CoucheCpu>> = Rc::new(
        liste
            .iter()
            .map(|o| CoucheCpu {
                dx: l(o.decalage_x),
                dy: l(o.decalage_y),
                // ★ Le flou CSS est un diamètre : σ = flou / 2 (voir `flottant::ombres`).
                sigma: l(o.flou / 2.0),
                etendue: l(o.etendue),
                couleur: o.couleur,
            })
            .collect(),
    );
    let e = ombrer(e, cx, liste, jeton, rayon, bordure).shadow(Vec::new());
    e.child(
        canvas(
            move |bounds: Bounds<Pixels>, window: &mut Window, _: &mut App| {
                let k = window.scale_factor();
                let b = f32::from(bordure);
                let (x, y) = (
                    f32::from(bounds.origin.x) - b,
                    f32::from(bounds.origin.y) - b,
                );
                let (xp, yp) = (x * k, y * k);
                let geometrie = Geometrie {
                    largeur: (f32::from(bounds.size.width) + 2.0 * b) * k,
                    hauteur: (f32::from(bounds.size.height) + 2.0 * b) * k,
                    rayon: f32::from(rayon) * k,
                    fx: xp - xp.floor(),
                    fy: yp - yp.floor(),
                    couches: couches
                        .iter()
                        .map(|c| CoucheCpu {
                            dx: c.dx * k,
                            dy: c.dy * k,
                            sigma: c.sigma * k,
                            etendue: c.etendue * k,
                            couleur: c.couleur,
                        })
                        .collect(),
                    voile: voile.a,
                };
                POSE.with(|p| {
                    *p.borrow_mut() = Some(Pose {
                        origine: (x, y),
                        geometrie,
                    })
                });
            },
            |_, (), _, _| {},
        )
        // ★ Sans top/left, un enfant absolu garde sa position statique (voir `flottant::ombrer`).
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full(),
    )
}

/// Le voile du calque, à poser sous la boîte : il peint l'image d'ombre posée par
/// [`ombrer_au_calque`] et, autour, le voile seul (`couleur`). Sans boîte posée, le voile entier.
pub fn voile(couleur_voile: Couleur) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
            let c = couleur(couleur_voile);
            let pose = POSE.with(|p| p.borrow_mut().take());
            let pret = pose
                .as_ref()
                .and_then(|p| image(&p.geometrie).map(|i| (p, i)));
            let Some((pose, (rendu, x0, y0, l, h))) = pret else {
                window.paint_quad(fill(bounds, c));
                return;
            };
            let k = window.scale_factor();
            // Le coin de l'image sur la grille des pixels physiques, en pixels logiques.
            let gx = (pose.origine.0 * k).floor() + x0 as f32;
            let gy = (pose.origine.1 * k).floor() + y0 as f32;
            let ib = Bounds::new(
                point(px(gx / k), px(gy / k)),
                size(px(l as f32 / k), px(h as f32 / k)),
            );
            // Le voile seul autour de l'image : quatre bandes.
            let (bx0, by0) = (bounds.origin.x, bounds.origin.y);
            let (bx1, by1) = (bx0 + bounds.size.width, by0 + bounds.size.height);
            let (ix0, iy0) = (ib.origin.x, ib.origin.y);
            let (ix1, iy1) = (ix0 + ib.size.width, iy0 + ib.size.height);
            let bande = |x0: Pixels, y0: Pixels, x1: Pixels, y1: Pixels| {
                (x1 > x0 && y1 > y0).then(|| Bounds::new(point(x0, y0), size(x1 - x0, y1 - y0)))
            };
            for b in [
                bande(bx0, by0, bx1, iy0.max(by0)),
                bande(bx0, iy1.min(by1), bx1, by1),
                bande(bx0, iy0.max(by0), ix0.max(bx0), iy1.min(by1)),
                bande(ix1.min(bx1), iy0.max(by0), bx1, iy1.min(by1)),
            ]
            .into_iter()
            .flatten()
            {
                window.paint_quad(fill(b, c));
            }
            // Une image refusée par l'atlas laisse le trou : mieux vaut le voile entier.
            if window
                .paint_image(ib, ib, Corners::default(), rendu, 0, false)
                .is_err()
            {
                window.paint_quad(fill(ib, c));
            }
        },
    )
    .absolute()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modale(voile: u8, couleur: Couleur) -> Geometrie {
        Geometrie {
            largeur: 610.0,
            hauteur: 102.0,
            rayon: 12.0,
            fx: 0.0,
            fy: 0.0,
            couches: vec![CoucheCpu {
                dx: 0.0,
                dy: 16.0,
                sigma: 24.0,
                etendue: 0.0,
                couleur,
            }],
            voile,
        }
    }

    /// Loin de la boîte, le calque n'est que le voile : noir à 128.
    #[test]
    fn loin_de_la_boite_le_calque_est_le_voile() {
        let img = rasteriser(&modale(128, Couleur::css(0, 0, 0, 0.6)));
        assert_eq!(&img.bgra[0..4], &[0, 0, 0, 128]);
        assert!(img.x0 < -70 && img.y0 < -50);
    }

    /// Sous la boîte, loin des bords, l'ombre est pleine : `A = q(v + a (1 − v))`.
    #[test]
    fn sous_la_boite_l_ombre_est_pleine() {
        let mut g = modale(128, Couleur::css(0, 0, 0, 0.6));
        g.hauteur = 400.0;
        let img = rasteriser(&g);
        let (x, y) = ((305 - img.x0) as usize, (216 - img.y0) as usize);
        let i = (y * img.largeur as usize + x) * 4;
        let a: f64 = 153.0 / 255.0;
        let attendu = (255.0 * (128.0 / 255.0 + a * (1.0 - 128.0 / 255.0)) + 0.5).floor() as u8;
        assert_eq!(img.bgra[i + 3], attendu);
    }

    /// Le flou par formes de rangées est le flou séparable direct (masque entier, deux noyaux).
    #[test]
    fn le_flou_par_formes_est_le_flou_direct() {
        let g = Geometrie {
            largeur: 40.0,
            hauteur: 30.0,
            rayon: 6.0,
            fx: 0.25,
            fy: 0.5,
            couches: vec![CoucheCpu {
                dx: 1.0,
                dy: 3.0,
                sigma: 4.0,
                etendue: 0.0,
                couleur: Couleur::octets(0, 0, 0, 255),
            }],
            voile: 0,
        };
        let img = rasteriser(&g);
        let (l, h) = (img.largeur as usize, img.hauteur as usize);
        let k = noyau(4.0);
        let rk = (k.len() / 2) as i64;
        let b = [1.25, 3.5, 40.0, 30.0];
        let mut m = vec![0.0; l * h];
        for y in 0..h {
            for x in 0..l {
                let (px, py) = (
                    f64::from(img.x0 + x as i32) + 0.5,
                    f64::from(img.y0 + y as i32) + 0.5,
                );
                m[y * l + x] = couverture(px, py, b, 6.0);
            }
        }
        let passe = |src: &[f64], hz: bool| {
            let mut d = vec![0.0; l * h];
            for y in 0..h {
                for x in 0..l {
                    d[y * l + x] = k
                        .iter()
                        .enumerate()
                        .filter_map(|(j, kv)| {
                            let o = j as i64 - rk;
                            let (sx, sy) = if hz {
                                (x as i64 + o, y as i64)
                            } else {
                                (x as i64, y as i64 + o)
                            };
                            (sx >= 0 && sy >= 0 && (sx as usize) < l && (sy as usize) < h)
                                .then(|| kv * src[sy as usize * l + sx as usize])
                        })
                        .sum();
                }
            }
            d
        };
        let direct = passe(&passe(&m, true), false);
        for (i, v) in direct.iter().enumerate() {
            assert_eq!(img.bgra[i * 4 + 3], octet(255.0 * v), "pixel {i}");
        }
    }

    /// Le calque reproduit la composition de Chromium : `L + fond × (1 − A)` retombe, par le
    /// mélange à alpha droit de GPUI, à un niveau au plus.
    #[test]
    fn l_alpha_droit_retombe_sur_le_calque() {
        let g = modale(128, Couleur::css(17, 18, 20, 0.16));
        let img = rasteriser(&g);
        for i in (0..img.bgra.len()).step_by(4 * 97) {
            let (b, a) = (f64::from(img.bgra[i]), f64::from(img.bgra[i + 3]) / 255.0);
            for fond in [20.0, 242.0] {
                let gpui = (b / 255.0 * a + fond / 255.0 * (1.0 - a)) * 255.0;
                // L = q(b × A) à un demi-niveau près.
                let calque = (b * a + 0.5).floor() + fond * (1.0 - a);
                assert!((gpui - calque).abs() <= 1.0, "{gpui} {calque}");
            }
        }
    }

    /// La clé du cache distingue les géométries.
    #[test]
    fn le_cache_distingue_les_geometries() {
        let a = modale(128, Couleur::css(0, 0, 0, 0.6));
        let mut b = a.clone();
        b.hauteur = 168.0;
        assert_ne!(cle(&a), cle(&b));
        assert_eq!(cle(&a), cle(&a.clone()));
    }

    /// Le coût d'une image de modale (mesure, `--nocapture`).
    #[test]
    fn cout_d_une_image_de_modale() {
        let t = std::time::Instant::now();
        let img = rasteriser(&modale(128, Couleur::css(0, 0, 0, 0.6)));
        eprintln!("ombre {}×{} : {:?}", img.largeur, img.hauteur, t.elapsed());
    }
}
