//! Écart pixel entre la capture de Fragment et un candidat de même taille.

/// Ce que mesure une comparaison (même grille, RVB, fond opaque).
#[derive(Clone, Copy, Debug, Default)]
pub struct Ecart {
    pub pixels: u32,
    /// Pixels dont au moins un canal diffère.
    pub differents: u32,
    /// Pixels dont l'écart dépasse 16 (l'ancien seuil du sas) : ce qui se voit.
    pub au_dela_16: u32,
    pub delta_max: u8,
    /// Encre : somme des distances au fond (0..1 par pixel), référence et candidat.
    pub encre_ref: f64,
    pub encre_cand: f64,
}

impl Ecart {
    pub fn pourcent(&self) -> f64 {
        100.0 * f64::from(self.differents) / f64::from(self.pixels.max(1))
    }
    /// Écart relatif d'encre (garde-fou d'ADR-009 : ±25 % pour un même texte).
    pub fn encre_relative(&self) -> f64 {
        (self.encre_cand - self.encre_ref) / self.encre_ref.max(1e-9)
    }
}

fn distance(a: &image::Rgb<u8>, b: [u8; 3]) -> u8 {
    (0..3).map(|i| a.0[i].abs_diff(b[i])).max().unwrap_or(0)
}

/// Compare deux images de même taille ; `fond` sert à mesurer l'encre.
pub fn comparer(reference: &image::RgbImage, candidat: &image::RgbImage, fond: [u8; 3]) -> Ecart {
    let mut e = Ecart::default();
    for (x, y, r) in reference.enumerate_pixels() {
        let Some(c) = candidat.get_pixel_checked(x, y) else {
            continue;
        };
        e.pixels += 1;
        let d = (0..3).map(|i| r.0[i].abs_diff(c.0[i])).max().unwrap_or(0);
        if d > 0 {
            e.differents += 1;
        }
        if d > 16 {
            e.au_dela_16 += 1;
        }
        e.delta_max = e.delta_max.max(d);
        e.encre_ref += f64::from(distance(r, fond)) / 255.0;
        e.encre_cand += f64::from(distance(c, fond)) / 255.0;
    }
    e
}

/// Image de diff : la référence éteinte, les pixels divergents en rouge (intensité = écart).
pub fn image_de_diff(reference: &image::RgbImage, candidat: &image::RgbImage) -> image::RgbImage {
    let mut sortie = image::RgbImage::new(reference.width(), reference.height());
    for (x, y, r) in reference.enumerate_pixels() {
        let c = candidat
            .get_pixel_checked(x, y)
            .copied()
            .unwrap_or(image::Rgb([0, 0, 0]));
        let d = (0..3).map(|i| r.0[i].abs_diff(c.0[i])).max().unwrap_or(0);
        let gris = (u16::from(r.0[0]) + u16::from(r.0[1]) + u16::from(r.0[2])) / 12;
        let p = if d > 0 {
            [d.max(64), 0, 0]
        } else {
            [gris as u8; 3]
        };
        sortie.put_pixel(x, y, image::Rgb(p));
    }
    sortie
}
