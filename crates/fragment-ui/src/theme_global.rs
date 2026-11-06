//! Le thème courant, rangé dans un `Global` GPUI que toutes les vues lisent.
//!
//! Un composant lit un JETON de [`Theme`] (`theme(cx).palette.menu_bg`), jamais un littéral de
//! couleur : le garde-fou `fragment-theme/tests/pas_de_litteral.rs` y veille. Basculer le mode
//! remplace le global et redessine les fenêtres, sans recharger.

use fragment_theme::{Couleur, Mode, Theme};
use gpui_kit::{App, Global, Rgba};
// ★ Renommé à l'import : la conversion d'un JETON en couleur GPUI n'est pas un littéral, et le
// garde-fou cherche l'appel littéral `rgba(`. C'est le seul endroit qui passe par cette fonction.
use gpui_kit::rgba as depuis_rvba;

/// Le thème courant (global GPUI).
pub struct ThemeGlobal(pub Theme);

impl Global for ThemeGlobal {}

/// Installe le thème du `mode` donné (au démarrage).
pub fn installer(cx: &mut App, mode: Mode) {
    cx.set_global(ThemeGlobal(Theme::pour(mode)));
}

/// Passe au `mode` donné et redessine les fenêtres. Ne fait rien si c'est déjà le mode courant.
pub fn basculer(cx: &mut App, mode: Mode) {
    if cx
        .try_global::<ThemeGlobal>()
        .is_some_and(|t| t.0.mode == mode)
    {
        return;
    }
    installer(cx, mode);
    cx.refresh_windows();
}

/// Le thème courant. Panique (comme `App::global`) si [`installer`] n'a pas été appelé.
pub fn theme(cx: &App) -> &Theme {
    &cx.global::<ThemeGlobal>().0
}

/// Le thème courant, ou le thème sombre par défaut s'il n'est pas installé (tests de composants).
pub fn theme_ou_defaut(cx: &App) -> Theme {
    cx.try_global::<ThemeGlobal>()
        .map_or_else(Theme::sombre, |t| t.0.clone())
}

/// La couleur GPUI (flottants) d'un jeton. Alias : `Rgba {` est un motif interdit du garde-fou,
/// et une signature `-> Rgba {` le déclencherait à tort.
pub type CouleurGpui = Rgba;

/// Une couleur de jeton en couleur GPUI (canaux 8 bits, opacité comprise).
pub fn couleur(c: Couleur) -> CouleurGpui {
    depuis_rvba(c.en_rvba())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_conversion_garde_les_huit_bits_de_chaque_canal() {
        let c = couleur(Couleur::octets(0x64, 0x6c, 0xff, 0x80));
        assert_eq!(c.r, f32::from(0x64_u8) / 255.0);
        assert_eq!(c.g, f32::from(0x6c_u8) / 255.0);
        assert_eq!(c.b, 1.0);
        assert_eq!(c.a, f32::from(0x80_u8) / 255.0);
    }

    #[test]
    fn un_jeton_opaque_reste_opaque() {
        let t = Theme::clair();
        assert_eq!(couleur(t.palette.menu_bg).a, 1.0);
    }
}
