//! Le mélange alpha de GPUI ramené sur celui de Chromium (`fr-p4.1`, spike S8).
//!
//! ★ Les deux composent dans le MÊME espace : sRGB encodé, sans linéarisation. GPUI peint dans une
//! cible `B8G8R8A8_UNORM` (pas `_SRGB`) avec `SRC_ALPHA, INV_SRC_ALPHA`, comme Skia dans ses tuiles.
//! La simulation de son shader d'ombre reproduit le candidat au bit près (modale sombre, 100 % des
//! pixels du halo). Les écarts ne viennent donc pas de l'espace, mais de ce qui est arrondi, et
//! quand :
//! - l'opacité d'une couleur : GPUI la reçoit de nos jetons en OCTET (`Couleur::a`), Chromium garde
//!   l'opacité exacte d'un `color-mix(… 10 %, transparent)` ([`opacite_exacte`]) ;
//! - le calque : Chromium rastérise un conteneur `position: fixed` (la modale, son voile et son
//!   ombre) dans une texture RGBA8 prémultipliée, puis la compose ; GPUI peint tout à la suite
//!   dans la même cible (voir le spike, l'ombre `--shadow-l`).

use fragment_theme::Couleur;

use crate::theme_global::{CouleurGpui, couleur};

/// Décalage de l'opacité qui fait tomber une égalité (x,5) du côté de Chromium.
///
/// ★ Chromium arrondit `fond + p × (teinte − fond)` au plus proche, la demie vers le HAUT (232,5 →
/// 233 sur le blanc du menu clair, mesuré par fr-t1.1). Le GPU calcule en `f32` et la conversion
/// en octet tombe d'un côté ou de l'autre d'une demie selon le bruit d'arrondi : on pousse
/// l'opacité d'un cent-millième, invisible ailleurs (≤ 0,003 niveau), pour que la demie monte.
const BIAIS: f32 = 1e-5;

/// La couleur GPUI de `teinte` à l'opacité EXACTE `pourcent` (0 à 100), à poser sur `fond`.
///
/// C'est ce que Chromium fait d'un `color-mix(in srgb, teinte p %, transparent)` : l'opacité reste
/// un flottant, alors que notre jeton (`Couleur`) l'a ramenée à l'octet (26/255 pour 10 %), ce qui
/// faisait tomber le mélange un niveau à côté. Le fond ne sert qu'au sens du [`BIAIS`].
pub fn opacite_exacte(teinte: Couleur, pourcent: f32, fond: Couleur) -> CouleurGpui {
    let mut c = couleur(Couleur::octets(teinte.r, teinte.g, teinte.b, 255));
    let lum = |c: Couleur| u32::from(c.r) + u32::from(c.g) + u32::from(c.b);
    // Demie vers le haut : une teinte plus sombre que le fond monte quand l'opacité baisse.
    let sens = if lum(teinte) < lum(fond) { -1.0 } else { 1.0 };
    c.a = (pourcent / 100.0 + sens * BIAIS).clamp(0.0, 1.0);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_theme::Theme;
    use gpui_kit::Hsla;

    /// Le chemin de GPUI au bit près, en `f32` : `Rgba` → `Hsla` (CPU), `hsla_to_rgba` (shader),
    /// mélange `SRC_ALPHA, INV_SRC_ALPHA` sur le fond, conversion en octet au plus proche.
    fn gpui(c: CouleurGpui, fond: Couleur) -> [u8; 3] {
        let h = Hsla::from(c);
        let (hh, s, l) = (h.h * 6.0, h.s, h.l);
        let cc = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = cc * (1.0 - ((hh % 2.0) - 1.0).abs());
        let m = l - cc / 2.0;
        let (r, g, b) = match hh {
            v if v < 1.0 => (cc, x, 0.0),
            v if v < 2.0 => (x, cc, 0.0),
            v if v < 3.0 => (0.0, cc, x),
            v if v < 4.0 => (0.0, x, cc),
            v if v < 5.0 => (x, 0.0, cc),
            _ => (cc, 0.0, x),
        };
        let a = h.a;
        let canal = |t: f32, f: u8| {
            let f = f32::from(f) / 255.0;
            let v = t * a + f * (1.0 - a);
            (v * 255.0 + 0.5).floor() as u8
        };
        [
            canal(r + m, fond.r),
            canal(g + m, fond.g),
            canal(b + m, fond.b),
        ]
    }

    /// Valeurs relevées sur les captures de Fragment (fr-t1.1, run 43018474495) : côtés droits des
    /// bordures du menu (clair, sombre) et de la notice sombre.
    #[test]
    fn la_bordure_a_l_opacite_exacte_tombe_sur_chromium() {
        let clair = Theme::clair().palette;
        let sombre = Theme::sombre().palette;
        let cas = [
            (&clair, clair.menu_bg, [232, 232, 233]),
            (&sombre, sombre.menu_bg, [46, 46, 50]),
            (&sombre, sombre.background_secondary_alt, [51, 52, 56]),
        ];
        for (p, fond, attendu) in cas {
            let c = opacite_exacte(p.background_modifier_border, p.opacites.bordure, fond);
            assert_eq!(gpui(c, fond), attendu);
        }
    }

    /// L'opacité reste celle du pourcentage, au biais près, et la teinte n'est pas touchée.
    #[test]
    fn l_opacite_exacte_garde_la_teinte() {
        let p = Theme::clair().palette;
        let c = opacite_exacte(p.background_modifier_border, 10.0, p.menu_bg);
        assert!((c.a - 0.1).abs() < 1e-4);
        let t = couleur(p.background_modifier_border);
        assert_eq!((c.r, c.g, c.b), (t.r, t.g, t.b));
    }
}
