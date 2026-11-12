// Port du rendu de MathBlockWidget d'app/src/ui/views/markdown/decorationPlugin.ts et de la boîte de
// `.cm-math-block` (editor.css), sur le moteur partagé de fragment-maths (ADR-011).
//! Les maths en bloc (`$$…$$` sur des lignes entières) : la hauteur dictée au fork et l'image peinte.
//!
//! La boîte suit `.cm-math-block` : bordure transparente, padding de 1,3 em × 0,4 em, et dedans le
//! conteneur `display` de MathJax centré. Ce conteneur est un bloc qui hérite de l'interligne de la
//! note : le `<svg>` (en ligne, abaissé de sa profondeur) partage sa boîte de ligne avec le « strut »
//! du texte. Une petite formule prend donc au moins une ligne de texte ; une grande, sa propre hauteur.
//!
//! ★ Le moteur travaille sur son fil : une formule absente du cache mesure 0 et est DEMANDÉE ; l'hôte
//!   relit le cache et reprojette quand elle arrive (jamais d'attente sur le fil d'interface).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use fragment_maths::moteur::{Cle, Moteur};
use fragment_maths::raster::{RenduSvg, vers_bitmap};
use gpui_kit::{App, Bounds, Corners, Pixels, RenderImage, Window, point, px, size};

use super::geometrie::{PADDING_MATHS_EM, hauteur_bloc_maths, interligne};

/// Au-delà, les images rastérisées sont rendues au GPU et le cache repart de zéro.
const IMAGES_MAX: usize = 128;

/// Le « strut » du texte de la note : ascent et descent de DM Sans au corps du texte, ARRONDIS au
/// pixel comme la boîte de ligne de Chromium.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strut {
    pub ascent: f32,
    pub descent: f32,
}

/// La boîte de ligne du conteneur de la formule : (hauteur, décalage du haut du `<svg>` sous le haut
/// de la ligne).
///
/// Le strut place sa ligne de base à mi-interligne (demi-interlignage au-dessus de l'ascent) ; le
/// `<svg>` pose sa propre ligne de base dessus. La ligne englobe les deux.
#[must_use]
pub fn boite_de_ligne(
    hauteur_svg: f32,
    ligne_de_base_svg: f32,
    interligne: f32,
    strut: Strut,
) -> (f32, f32) {
    let au_dessus = strut.ascent + (interligne - (strut.ascent + strut.descent)) / 2.0;
    let au_dessous = interligne - au_dessus;
    let haut = au_dessus.max(ligne_de_base_svg);
    let bas = au_dessous.max(hauteur_svg - ligne_de_base_svg);
    (haut + bas, haut - ligne_de_base_svg)
}

/// Les maths en bloc d'un hôte : le moteur partagé, la couleur du texte, les images rastérisées.
pub struct MathsBlocs {
    moteur: Rc<Moteur>,
    strut: Strut,
    couleur: Cell<[u8; 3]>,
    /// Une formule manquait à la dernière mesure : l'hôte doit relire le cache.
    manque: Cell<bool>,
    images: RefCell<HashMap<(Cle, u32), Arc<RenderImage>>>,
}

impl MathsBlocs {
    #[must_use]
    pub fn new(moteur: Rc<Moteur>, strut: Strut, couleur: [u8; 3]) -> Self {
        MathsBlocs {
            moteur,
            strut,
            couleur: Cell::new(couleur),
            manque: Cell::new(false),
            images: RefCell::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn moteur(&self) -> &Rc<Moteur> {
        &self.moteur
    }

    /// Pose la couleur du texte (`--text-normal`) ; vrai si elle a changé.
    pub fn poser_couleur(&self, c: [u8; 3]) -> bool {
        self.couleur.replace(c) != c
    }

    /// Vrai (une seule fois) si une formule manquait depuis le dernier appel.
    pub fn prendre_manque(&self) -> bool {
        self.manque.replace(false)
    }

    /// Le nombre d'images rastérisées gardées.
    #[must_use]
    pub fn images(&self) -> usize {
        self.images.borrow().len()
    }

    fn cle(&self, tex: &str, taille: f32) -> Cle {
        Cle::new(tex, true, f64::from(taille), self.couleur.get())
    }

    /// Le rendu de `tex` s'il est en cache ; sinon il est demandé et `None`. Une formule invalide
    /// rend `None` aussi (la source reste seule, comme `.cm-math-error` en repli).
    pub fn rendu(&self, tex: &str, taille: f32) -> Option<Arc<RenduSvg>> {
        let cle = self.cle(tex, taille);
        match self.moteur.en_cache(&cle) {
            Some(r) => r.ok(),
            None => {
                self.moteur.demander(&cle);
                self.manque.set(true);
                None
            }
        }
    }

    /// La hauteur dictée du bloc de `tex` (bordure, padding, boîte de ligne), ou `None` tant que le
    /// moteur ne l'a pas rendu.
    pub fn hauteur(&self, tex: &str, taille: f32, bordure: f32) -> Option<f32> {
        let r = self.rendu(tex, taille)?;
        let (ligne, _) = boite_de_ligne(
            r.hauteur as f32,
            r.ligne_de_base as f32,
            interligne(taille),
            self.strut,
        );
        Some(hauteur_bloc_maths(taille, ligne, bordure))
    }

    /// Peint la formule dans la boîte du bloc (`boite` : la colonne, sur la hauteur dictée), centrée,
    /// à l'origine ARRONDIE au pixel et à sa taille de mise en page (ADR-011).
    pub fn peindre(
        &self,
        tex: &str,
        taille: f32,
        bordure: f32,
        boite: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let cle = self.cle(tex, taille);
        let Some(Ok(rendu)) = self.moteur.en_cache(&cle) else {
            return;
        };
        let echelle = window.scale_factor();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cle_image = (cle, (echelle * 100.0) as u32);
        let image = {
            let mut m = self.images.borrow_mut();
            if let Some(i) = m.get(&cle_image) {
                i.clone()
            } else {
                let Ok(b) = vers_bitmap(&rendu, echelle) else {
                    return;
                };
                let Some(tampon) = image::RgbaImage::from_raw(b.largeur, b.hauteur, b.bgra) else {
                    return;
                };
                if m.len() >= IMAGES_MAX {
                    for (_, i) in m.drain() {
                        cx.drop_image(i, Some(window));
                    }
                }
                let i = Arc::new(RenderImage::new(vec![image::Frame::new(tampon)]));
                m.insert(cle_image, i.clone());
                i
            }
        };
        let (l, h) = (rendu.largeur as f32, rendu.hauteur as f32);
        let (_, decalage) = boite_de_ligne(
            h,
            rendu.ligne_de_base as f32,
            interligne(taille),
            self.strut,
        );
        let [pv, ph] = PADDING_MATHS_EM;
        let gauche = f32::from(boite.origin.x) + bordure + ph * taille;
        let largeur = f32::from(boite.size.width) - 2.0 * (bordure + ph * taille);
        // `margin: 0 auto` : centrée ; une formule plus large que la colonne part du bord gauche
        // (le bloc défile, `overflow-x: auto`).
        let x = (gauche + ((largeur - l) / 2.0).max(0.0)).round();
        let y = (f32::from(boite.origin.y) + bordure + pv * taille + decalage).round();
        let cible = Bounds::new(point(px(x), px(y)), size(px(l), px(h)));
        window.with_content_mask(Some(gpui_kit::ContentMask { bounds: boite }), |window| {
            let _ = window.paint_image(cible, cible, Corners::default(), image, 0, false);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRUT: Strut = Strut {
        ascent: 16.0,
        descent: 5.0,
    };

    #[test]
    fn petite_formule_une_ligne_de_texte() {
        // Interligne 22,4 : le strut donne 16,7 au-dessus, 5,7 au-dessous.
        let (h, d) = boite_de_ligne(10.0, 8.0, 22.4, STRUT);
        assert!((h - 22.4).abs() < 1e-4, "{h}");
        assert!((d - 8.7).abs() < 1e-4, "{d}");
    }

    #[test]
    fn grande_formule_sa_propre_hauteur() {
        let (h, d) = boite_de_ligne(60.0, 40.0, 22.4, STRUT);
        assert!((h - 60.0).abs() < 1e-4, "{h}");
        assert!(d.abs() < 1e-4, "{d}");
    }
}
