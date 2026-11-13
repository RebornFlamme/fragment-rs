//! Le globe du widget RIGIDE de test : une ellipse (`border-radius: 50 %`) remplie d'un dégradé
//! radial `circle` (centre → bord), comme le calque `e2e-globe` des fixtures de Fragment.
//!
//! GPUI n'a pas de dégradé radial : le globe est calculé pixel par pixel, une fois par taille, et
//! posé comme une image à l'échelle 1:1 de l'écran.
//!
//! ★ Le dégradé CSS `radial-gradient(circle, a, b)` a pour rayon la distance du centre au coin le
//! plus éloigné (`farthest-corner`) : sur une ellipse allongée, le bord de la forme n'atteint donc
//! JAMAIS la couleur finale.

use fragment_theme::fixture::{FIXTURE_GLOBE_BORD, FIXTURE_GLOBE_CENTRE};

/// Sous-échantillons par axe pour la couverture des pixels du bord de l'ellipse.
const SOUS: u32 = 8;

/// Le globe de `largeur` × `hauteur` pixels, en BGRA à alpha DROIT (l'ordre que GPUI attend).
pub fn globe_bgra(largeur: u32, hauteur: u32) -> Vec<u8> {
    let (w, h) = (f64::from(largeur), f64::from(hauteur));
    let (cx, cy) = (w / 2.0, h / 2.0);
    let rayon = cx.hypot(cy).max(f64::MIN_POSITIVE);
    let mut octets = Vec::with_capacity((largeur * hauteur * 4) as usize);
    for y in 0..hauteur {
        for x in 0..largeur {
            let couverture = couverture(f64::from(x), f64::from(y), cx, cy);
            if couverture == 0 {
                octets.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // Le centre du pixel donne la couleur.
            let d = (f64::from(x) + 0.5 - cx).hypot(f64::from(y) + 0.5 - cy);
            let t = (d / rayon).clamp(0.0, 1.0);
            let canal = |a: u8, b: u8| -> u8 {
                (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8
            };
            let (c, f) = (FIXTURE_GLOBE_CENTRE, FIXTURE_GLOBE_BORD);
            octets.extend_from_slice(&[
                canal(c.b, f.b),
                canal(c.g, f.g),
                canal(c.r, f.r),
                couverture,
            ]);
        }
    }
    octets
}

/// La part (0 à 255) du pixel `(x, y)` qui tombe dans l'ellipse de centre `(cx, cy)` et de demi-axes
/// `cx`, `cy` (l'ellipse inscrite dans la boîte).
fn couverture(x: f64, y: f64, cx: f64, cy: f64) -> u8 {
    let mut dedans = 0u32;
    for j in 0..SOUS {
        for i in 0..SOUS {
            let px = x + (f64::from(i) + 0.5) / f64::from(SOUS);
            let py = y + (f64::from(j) + 0.5) / f64::from(SOUS);
            let (u, v) = ((px - cx) / cx, (py - cy) / cy);
            if u * u + v * v <= 1.0 {
                dedans += 1;
            }
        }
    }
    (f64::from(dedans) * 255.0 / f64::from(SOUS * SOUS)).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(o: &[u8], l: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * l + x) * 4) as usize;
        [o[i], o[i + 1], o[i + 2], o[i + 3]]
    }

    #[test]
    fn le_centre_est_la_couleur_de_centre() {
        let o = globe_bgra(41, 41);
        // Pixel central d'une boîte impaire : sa distance au centre est nulle (à un demi-pixel).
        let [b, g, r, a] = pixel(&o, 41, 20, 20);
        assert_eq!(a, 255);
        assert!(r.abs_diff(0x2a) <= 1 && g.abs_diff(0x6f) <= 1 && b.abs_diff(0xd6) <= 1);
    }

    #[test]
    fn les_coins_sont_transparents() {
        let o = globe_bgra(60, 40);
        assert_eq!(pixel(&o, 60, 0, 0)[3], 0);
        assert_eq!(pixel(&o, 60, 59, 39)[3], 0);
    }

    #[test]
    fn le_milieu_des_bords_est_plein() {
        let o = globe_bgra(60, 40);
        assert_eq!(pixel(&o, 60, 30, 0)[3], 255);
        assert_eq!(pixel(&o, 60, 0, 20)[3], 255);
    }

    #[test]
    fn le_bord_d_une_ellipse_allongee_reste_loin_de_la_couleur_finale() {
        // Rayon = coin le plus éloigné : au milieu d'un grand côté, t < 1.
        let o = globe_bgra(200, 40);
        let [_, _, r, _] = pixel(&o, 200, 100, 0);
        assert!(r > 0x0b);
    }

    #[test]
    fn le_globe_est_symetrique() {
        let o = globe_bgra(50, 30);
        assert_eq!(pixel(&o, 50, 10, 7), pixel(&o, 50, 39, 22));
    }
}
