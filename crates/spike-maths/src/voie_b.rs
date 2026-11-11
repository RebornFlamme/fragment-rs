//! Voie B : un moteur TeX en pur Rust. ★ `rex` (autorisé par program.md §4) n'est PAS ReX :
//! le nom `rex` de crates.io appartient à un langage fonctionnel sans rapport, et ReX (le
//! moteur de maths) n'a jamais été publié (dépôt git seul, abandonné). La voie B est donc tenue
//! par `latex-rust` 3.0.3 (MIT OR Apache-2.0, Appendix G de TeX, sortie SVG, polices OpenType
//! MATH), dépendance NOUVELLE notée dans la PR.

use latex_rust::{
    Color, Dim, MathFont, MathStyle, layout_with_em_size_pt, parse, render_svg_with_metrics,
};

use crate::Erreur;
use crate::raster::RenduSvg;

fn lr(e: latex_rust::Error) -> Erreur {
    Erreur::Moteur(format!("latex-rust : {e}"))
}

/// Une police OpenType MATH chargée (STIX Two Math, embarquée par la crate, ou une autre).
pub struct MoteurTex {
    police: MathFont,
}

impl MoteurTex {
    pub fn stix() -> Result<Self, Erreur> {
        Ok(Self {
            police: MathFont::stix_two_math().map_err(lr)?,
        })
    }

    pub fn avec_police(octets: &'static [u8]) -> Result<Self, Erreur> {
        Ok(Self {
            police: MathFont::from_bytes(octets).map_err(lr)?,
        })
    }

    /// Rend `tex` avec un em de `em_px` (px CSS) dans la couleur du texte.
    pub fn rendre(
        &self,
        tex: &str,
        bloc: bool,
        em_px: f64,
        couleur: [u8; 3],
    ) -> Result<RenduSvg, Erreur> {
        let ast = parse(tex).map_err(|e| Erreur::Moteur(format!("latex-rust : {e}")))?;
        let style = if bloc {
            MathStyle::Display
        } else {
            MathStyle::Text
        };
        // Le moteur travaille en points ; 1 px CSS = 0,75 pt.
        let em_pt = Dim::parse(&format!("{:.6}", em_px * 0.75));
        let arbre = layout_with_em_size_pt(&ast, &self.police, style, &em_pt).map_err(lr)?;
        let mut options = latex_rust::SvgOptions::new();
        options.font_size_pt = em_pt;
        options.color = Color::rgb(couleur[0], couleur[1], couleur[2]);
        options.display = bloc;
        let image = render_svg_with_metrics(&arbre, &self.police, &options).map_err(lr)?;
        let pt = |d: &Dim| d.to_svg_string().parse::<f64>().unwrap_or(0.0);
        let (l_pt, h_pt) = (pt(&image.width_pt), pt(&image.height_pt));
        let svg = image
            .svg
            .replacen(
                &format!("width=\"{}pt\"", image.width_pt.to_svg_string()),
                &format!("width=\"{}\"", l_pt / 0.75),
                1,
            )
            .replacen(
                &format!("height=\"{}pt\"", image.height_pt.to_svg_string()),
                &format!("height=\"{}\"", h_pt / 0.75),
                1,
            );
        Ok(RenduSvg {
            svg,
            largeur: l_pt / 0.75,
            hauteur: h_pt / 0.75,
            ligne_de_base: pt(&image.baseline_pt) / 0.75,
        })
    }
}
