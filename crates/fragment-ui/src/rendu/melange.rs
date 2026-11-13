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
use gpui_kit::{App, ParentElement, Styled, div};

use crate::echelle::u;
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

/// L'aplat OPAQUE de `teinte` à `pourcent` % sur `fond`, comme Chromium mélange une bordure
/// translucide sur un fond opaque : `fond + p × (teinte − fond)` en flottant, la demie vers le haut
/// (232,5 → 233 sur le blanc du menu clair).
///
/// ★ Pas `Palette::aplat` : celui-là suit le chemin prémultiplié des REMPLISSAGES (opacité ramenée
/// à l'octet), qui donne 232 là où la bordure de Chromium donne 233. Le numérateur est calculé
/// entier avant la division, pour que la demie reste exacte en `f64`.
pub fn aplat_exact(teinte: Couleur, pourcent: f32, fond: Couleur) -> Couleur {
    let p = f64::from(pourcent);
    let canal = |t: u8, f: u8| -> u8 {
        let v = (f64::from(f) * (100.0 - p) + f64::from(t) * p) / 100.0;
        // Borné à [0, 255] : la conversion ne tronque rien.
        (v + 0.5).floor().clamp(0.0, 255.0) as u8
    };
    Couleur::octets(
        canal(teinte.r, fond.r),
        canal(teinte.g, fond.g),
        canal(teinte.b, fond.b),
        255,
    )
}

/// L'aplat OPAQUE de `teinte` à `pourcent` % sur `fond`, comme le compose un CALQUE composé de
/// Chromium : la teinte prémultipliée par l'opacité ramenée à l'octet, puis le fond par le reste,
/// chacun arrondi (`Couleur::sur_chromium`, le chemin des remplissages).
///
/// ★ Mesuré sur les menus qui chevauchent un calque composé (explorateur, vue PDF) : (45,46,50) en
/// sombre et (232,232,232) en clair, au pixel près, là où [`aplat_exact`] donne (46,46,50) et
/// (232,232,233), ce que gardent les menus hors calque (onglet, éditeur, sélecteur de coffre).
pub fn aplat_au_calque(teinte: Couleur, pourcent: f32, fond: Couleur) -> Couleur {
    teinte.sur_chromium(pourcent, fond)
}

/// Le rayon intérieur ajouté sous une bordure TRANSLUCIDE, en px CSS (ADR-013 §3).
///
/// ★ Mesuré sur la mire de S8 (bordure de 1 px à 10 %, r 8) : devant une bordure translucide,
/// Chromium n'évite pas le débordement du fond et Skia peint l'anneau par-dessus ; son arc
/// intérieur tombe comme celui d'un rayon `r − bordure + 0,4` peint par GPUI (écart de 4 niveaux
/// à 2, somme signée nulle ; 0,3 à 0,5 donnent tous ≤ 3). Devant une bordure OPAQUE (bleed
/// avoidance), le meilleur rayon reste `r − bordure` (mires de 1 et 2 px).
pub const RAYON_INTERIEUR_TRANSLUCIDE: f32 = 0.4;

/// Peint la boîte `e` comme Chromium peint une boîte arrondie à bordure : UN SEUL anticrénelage au
/// bord extérieur (ADR-013 §3, spike S8 cause 3).
///
/// ★ GPUI peint le fond (anticrénelé), puis la bordure (anticrénelée encore) : le pixel de bord reçoit
/// deux fois la couverture (31 niveaux d'écart sur la mire opaque). Ici, la boîte entière est
/// peinte de `bord`, couleur OPAQUE (pour une bordure translucide, son aplat sur le fond, au niveau
/// près de Chromium), et le fond est un enfant en retrait de la bordure, de rayon
/// `rayon − largeur` (`+ RAYON_INTERIEUR_TRANSLUCIDE` si `translucide`). La largeur de bordure reste
/// déclarée pour la mise en page, sans couleur : GPUI ne peint alors aucun second quad.
///
/// À appeler AVANT d'ajouter les enfants de `e` : le fond doit être son premier enfant peint.
pub fn bord_unique<E: Styled + ParentElement>(
    e: E,
    cx: &App,
    geometrie: (f32, f32),
    bord: Couleur,
    fond: Couleur,
    translucide: bool,
) -> E {
    let (rayon, largeur) = geometrie;
    let plus = if translucide {
        RAYON_INTERIEUR_TRANSLUCIDE
    } else {
        0.0
    };
    let interieur = (rayon - largeur + plus).max(0.0);
    crate::flottant::bordure(e, u(cx, largeur))
        .rounded(u(cx, rayon))
        .bg(couleur(bord))
        .child(
            div()
                .absolute()
                .top(u(cx, 0.0))
                .left(u(cx, 0.0))
                .size_full()
                .rounded(u(cx, interieur))
                .bg(couleur(fond)),
        )
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

    /// Le bord unique peint l'aplat OPAQUE de la bordure : l'aller-retour HSLA de GPUI le rend au
    /// niveau près des relevés de Chromium (côtés droits du menu, de la notice sombre).
    #[test]
    fn l_aplat_de_la_bordure_survit_au_chemin_de_gpui() {
        let clair = Theme::clair().palette;
        let sombre = Theme::sombre().palette;
        let cas = [
            (&clair, clair.menu_bg, [232, 232, 233]),
            (&sombre, sombre.menu_bg, [46, 46, 50]),
            (&sombre, sombre.background_secondary_alt, [51, 52, 56]),
        ];
        for (p, fond, attendu) in cas {
            let a = aplat_exact(p.background_modifier_border, p.opacites.bordure, fond);
            assert_eq!(gpui(couleur(a), fond), attendu);
        }
        for p in [&clair, &sombre] {
            let c = p.tab_outline_color;
            assert_eq!(gpui(couleur(c), p.workspace_bg), [c.r, c.g, c.b]);
        }
    }

    /// Relevés des références de l'explorateur (`t2-explorateur-menu-dossier`, côtés du menu) : la
    /// bordure du menu posé sur un calque composé, au bit près après le chemin de GPUI.
    #[test]
    fn la_bordure_du_menu_au_calque_tombe_sur_chromium() {
        let clair = Theme::clair().palette;
        let sombre = Theme::sombre().palette;
        for (p, attendu) in [(&sombre, [45, 46, 50]), (&clair, [232, 232, 232])] {
            let a = aplat_au_calque(p.background_modifier_border, p.opacites.bordure, p.menu_bg);
            assert_eq!([a.r, a.g, a.b], attendu);
            assert_eq!(gpui(couleur(a), p.menu_bg), attendu);
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
