//! Couleurs 8 bits et ombres des jetons, avec le mélange `color-mix(in srgb, …)` de Chromium.
//!
//! ★ Une couleur de jeton est un quadruplet d'octets (rouge, vert, bleu, opacité) : c'est ce que
//! Chromium garde après résolution d'un `color-mix`, et ce qui rend les pixels stricts.

/// Une couleur sRGB non prémultipliée, 8 bits par canal (`a` = opacité, 255 = opaque).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Couleur {
    /// Rouge.
    pub r: u8,
    /// Vert.
    pub g: u8,
    /// Bleu.
    pub b: u8,
    /// Opacité (0 = transparent, 255 = opaque).
    pub a: u8,
}

/// Arrondi au plus proche, 0,5 vers le haut : celui de Chromium pour un canal 8 bits.
fn arrondi(x: f64) -> u8 {
    // Borné à 0..=255 : la conversion ne peut ni déborder ni perdre de signe.
    (x + 0.5).floor().clamp(0.0, 255.0) as u8
}

impl Couleur {
    /// Le noir opaque (mot-clé `black`).
    pub const NOIR: Self = Self::octets(0, 0, 0, 255);
    /// Le blanc opaque (mot-clé `white`).
    pub const BLANC: Self = Self::octets(255, 255, 255, 255);
    /// Le transparent (mot-clé `transparent` : noir d'opacité nulle).
    pub const TRANSPARENT: Self = Self::octets(0, 0, 0, 0);

    /// Quatre octets, tels quels.
    pub const fn octets(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Une couleur opaque depuis `0xRRGGBB` (notation `#rrggbb`).
    pub const fn hex(rvb: u32) -> Self {
        Self::octets((rvb >> 16) as u8, (rvb >> 8) as u8, rvb as u8, 255)
    }

    /// Une couleur d'opacité décimale, comme la fonction CSS à quatre arguments (`0.22`).
    /// L'opacité devient un octet par l'arrondi de Chromium.
    pub fn css(r: u8, g: u8, b: u8, opacite: f32) -> Self {
        Self::octets(r, g, b, arrondi(f64::from(opacite) * 255.0))
    }

    /// `color-mix(in srgb, self pourcentage%, autre)` : `autre` prend `100 - pourcentage`.
    ///
    /// Le mélange se fait en sRGB non linéaire, sur des canaux PRÉMULTIPLIÉS (spec CSS Color 4),
    /// puis chaque canal est arrondi à l'octet le plus proche. Mélanger avec `transparent` garde
    /// donc la teinte de `self` et ne fait varier que l'opacité.
    pub fn melange(self, pourcentage: f32, autre: Self) -> Self {
        // Poids en pourcents × opacité : exacts en flottant pour les pourcentages entiers, ce qui
        // évite qu'un « .5 » exact (146,5) retombe sous la demie par erreur d'arrondi.
        let (p, q) = (f64::from(pourcentage), 100.0 - f64::from(pourcentage));
        let (w1, w2) = (
            p * f64::from(self.a) / 255.0,
            q * f64::from(autre.a) / 255.0,
        );
        let poids = w1 + w2;
        if poids <= 0.0 {
            return Self::TRANSPARENT;
        }
        let canal = |c1: u8, c2: u8| (w1 * f64::from(c1) + w2 * f64::from(c2)) / poids;
        Self::octets(
            arrondi(canal(self.r, autre.r)),
            arrondi(canal(self.g, autre.g)),
            arrondi(canal(self.b, autre.b)),
            arrondi(poids * 255.0 / 100.0),
        )
    }

    /// `self` (teinte) à l'opacité EXACTE `pourcent` (0 à 100), posée sur un fond opaque, comme le
    /// fait Chromium : le résultat est une couleur opaque.
    ///
    /// ★ Mesuré sur les captures de Fragment (palette sombre et claire, menu clair, survol de
    /// l'explorateur) : le canal de la teinte est PRÉMULTIPLIÉ par l'opacité exacte et arrondi
    /// (`round(c × p / 100)`), le fond est atténué par l'opacité ramenée à l'octet
    /// (`round(fond × (255 − A) / 255)`, `A = round(p × 2,55)`), puis les deux s'additionnent.
    /// Composer avec l'octet d'opacité des deux côtés (`sur`) tombe un niveau à côté.
    pub fn sur_chromium(self, pourcent: f32, fond: Self) -> Self {
        // Pourcentage entier en pratique : `p × 255 / 100` est exact (25,5 pour 10 %).
        let p = f64::from(pourcent);
        let octet = arrondi(p * 255.0 / 100.0);
        let reste = f64::from(255 - octet) / 255.0;
        let canal = |c: u8, f: u8| {
            let (teinte, dessous) = (
                arrondi(f64::from(c) * p / 100.0),
                arrondi(f64::from(f) * reste),
            );
            teinte.saturating_add(dessous)
        };
        Self::octets(
            canal(self.r, fond.r),
            canal(self.g, fond.g),
            canal(self.b, fond.b),
            255,
        )
    }

    /// Le résultat de `self` posée sur un fond opaque (composition sRGB, arrondie à l'octet).
    ///
    /// Approximation : elle se trompe d'un niveau quand l'opacité n'est pas un multiple exact de
    /// l'octet (voir `sur_chromium`, qui est celle à utiliser pour un aplat).
    pub fn sur(self, fond: Self) -> Self {
        let a = f64::from(self.a) / 255.0;
        let canal = |c: u8, f: u8| arrondi(a * f64::from(c) + (1.0 - a) * f64::from(f));
        Self::octets(
            canal(self.r, fond.r),
            canal(self.g, fond.g),
            canal(self.b, fond.b),
            255,
        )
    }

    /// `0xRRGGBBAA`, la notation à huit chiffres hexadécimaux.
    pub const fn en_rvba(self) -> u32 {
        (self.r as u32) << 24 | (self.g as u32) << 16 | (self.b as u32) << 8 | self.a as u32
    }
}

/// Une couche d'ombre portée (`box-shadow` sans `inset`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ombre {
    /// Décalage horizontal, en pixels.
    pub decalage_x: f32,
    /// Décalage vertical, en pixels.
    pub decalage_y: f32,
    /// Rayon de flou, en pixels.
    pub flou: f32,
    /// Étendue (spread), en pixels.
    pub etendue: f32,
    /// Couleur de l'ombre.
    pub couleur: Couleur,
}

impl Ombre {
    /// Une couche d'ombre.
    pub const fn new(
        decalage_x: f32,
        decalage_y: f32,
        flou: f32,
        etendue: f32,
        couleur: Couleur,
    ) -> Self {
        Self {
            decalage_x,
            decalage_y,
            flou,
            etendue,
            couleur,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Aplat, Mode, Palette, Theme};
    use std::path::PathBuf;

    /// Le pixel `(x, y)` d'une capture de référence de Fragment.
    fn pixel_de_reference(nom: &str, x: u32, y: u32) -> Couleur {
        let chemin: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "pixels",
            "reference",
            nom,
        ]
        .iter()
        .collect();
        let image = image::open(&chemin)
            .expect("capture de référence lisible")
            .to_rgb8();
        let p = image.get_pixel(x, y);
        Couleur::octets(p[0], p[1], p[2], 255)
    }

    #[test]
    fn hex_et_octets() {
        assert_eq!(
            Couleur::hex(0x646cff),
            Couleur::octets(0x64, 0x6c, 0xff, 255)
        );
        assert_eq!(Couleur::hex(0x646cff).en_rvba(), 0x646cffff);
    }

    #[test]
    fn l_opacite_css_s_arrondit_comme_chromium() {
        assert_eq!(Couleur::css(0, 0, 0, 0.5).a, 128, "127,5 monte");
        assert_eq!(Couleur::css(255, 238, 0, 0.22).a, 56);
        assert_eq!(Couleur::css(0, 0, 0, 0.75).a, 191, "191,25");
        assert_eq!(Couleur::css(1, 2, 3, 1.0).a, 255);
    }

    // --- Dix valeurs de color-mix (les deux thèmes), calculées à la main depuis la table. ---

    #[test]
    fn melange_accent_hover_sombre() {
        // 85 % de #646cff + 15 % de #dfded9 : 100*.85+223*.15 = 118,45 ; 108*.85+222*.15 = 125,1 ;
        // 255*.85+217*.15 = 249,3.
        let t = Theme::sombre().palette;
        assert_eq!(t.interactive_accent_hover, Couleur::hex(0x767df9));
    }

    #[test]
    fn melange_accent_hover_clair() {
        // 85 % de #4b52d6 + 15 % de #1a1b1e : 75*.85+26*.15 = 67,65 ; 82*.85+27*.15 = 73,75 ;
        // 214*.85+30*.15 = 186,4.
        let t = Theme::clair().palette;
        assert_eq!(t.interactive_accent_hover, Couleur::hex(0x444aba));
    }

    #[test]
    fn melange_accent_1_avec_du_blanc() {
        // 70 % de #646cff + 30 % de blanc : 100*.7+255*.3 = 146,5 -> 147 ; 108*.7+76,5 = 152,1 ; 255.
        let t = Theme::sombre().palette;
        assert_eq!(t.color_accent_1, Couleur::hex(0x9398ff));
    }

    #[test]
    fn melange_accent_2_avec_du_noir() {
        // 75 % de #646cff + 25 % de noir : 75 ; 81 ; 191,25.
        let t = Theme::sombre().palette;
        assert_eq!(t.color_accent_2, Couleur::hex(0x4b51bf));
    }

    #[test]
    fn melange_text_accent_sombre_eclaircit_l_accent() {
        // L'accent à 75 % + 25 % de blanc : 100*.75+63,75 = 138,75 ; 108*.75+63,75 = 144,75 ; 255.
        let t = Theme::sombre().palette;
        assert_eq!(t.text_accent, Couleur::hex(0x8b91ff));
    }

    #[test]
    fn melange_bordure_sombre_9_pour_cent() {
        // Le texte à 9 % d'opacité : teinte du texte, opacité 22,95 -> 23.
        let t = Theme::sombre().palette;
        assert_eq!(
            t.background_modifier_border,
            Couleur::octets(0xdf, 0xde, 0xd9, 23)
        );
    }

    #[test]
    fn melange_bordure_claire_10_pour_cent() {
        // Opacité 25,5 -> 26 (0,5 vers le haut).
        let t = Theme::clair().palette;
        assert_eq!(
            t.background_modifier_border,
            Couleur::octets(0x1a, 0x1b, 0x1e, 26)
        );
    }

    #[test]
    fn melange_survol_6_et_5_pour_cent() {
        // 15,3 -> 15 en sombre ; 12,75 -> 13 en clair.
        assert_eq!(Theme::sombre().palette.background_modifier_hover.a, 15);
        assert_eq!(Theme::clair().palette.background_modifier_hover.a, 13);
    }

    #[test]
    fn melange_survol_actif_20_et_barres_de_defilement() {
        // Commun aux deux thèmes : 20 % -> 51 ; 30 % -> 76,5 -> 77 ; 10 % -> 25,5 -> 26.
        for t in [Theme::sombre().palette, Theme::clair().palette] {
            assert_eq!(t.background_modifier_border_hover.a, 51);
            assert_eq!(t.scrollbar_thumb_bg.a, 51);
            assert_eq!(t.scrollbar_active_thumb_bg.a, 77);
            assert_eq!(t.background_modifier_active_hover.a, 26);
        }
    }

    #[test]
    fn melange_garde_la_teinte_du_texte_dans_chaque_theme() {
        let s = Theme::sombre().palette;
        let c = Theme::clair().palette;
        assert_eq!(s.scrollbar_thumb_bg.r, s.text_normal.r);
        assert_eq!(c.scrollbar_thumb_bg.b, c.text_normal.b);
        assert_ne!(s.scrollbar_thumb_bg.r, c.scrollbar_thumb_bg.r);
    }

    #[test]
    fn melange_avec_deux_transparents_reste_transparent() {
        assert_eq!(
            Couleur::TRANSPARENT.melange(50.0, Couleur::TRANSPARENT),
            Couleur::TRANSPARENT
        );
    }

    // --- Trois valeurs relues dans les captures de référence de Fragment (pixels/reference). ---

    #[test]
    fn survol_de_l_explorateur_sombre_relu_dans_la_reference() {
        // Ligne survolée de l'explorateur : fond du panneau + survol (6 %) = (29, 30, 32).
        let t = Theme::sombre().palette;
        let attendu = pixel_de_reference("survol-explorateur-dark.png", 250, 117);
        assert_eq!(
            t.background_modifier_hover.sur(t.background_secondary),
            attendu
        );
    }

    #[test]
    fn survol_de_l_explorateur_clair_relu_dans_la_reference() {
        // Même ligne en clair : survol à 5 % sur le panneau clair = (228, 227, 223).
        let t = Theme::clair().palette;
        let attendu = pixel_de_reference("survol-explorateur-light.png", 60, 117);
        assert_eq!(
            t.background_modifier_hover.sur(t.background_secondary),
            attendu
        );
    }

    #[test]
    fn ligne_de_menu_survolee_claire_relue_dans_la_reference() {
        // Ligne de menu survolée en clair : survol actif (10 %) sur le fond du menu = (232, 232, 232).
        let t = Theme::clair().palette;
        let attendu = pixel_de_reference("menu-light.png", 600, 481);
        assert_eq!(t.background_modifier_active_hover.sur(t.menu_bg), attendu);
    }

    // --- Aplats : un jeton translucide posé sur un fond connu (ADR-009 c). ---

    /// Les 32 paires (thème, jeton, fond) avec la valeur attendue, calculée à part, hors du code
    /// Rust, par la formule de Chromium (voir `sur_chromium`).
    #[allow(clippy::type_complexity)]
    fn paires() -> [(Mode, Aplat, &'static str, fn(&Palette) -> Couleur, u32); 32] {
        [
            (
                Mode::Sombre,
                Aplat::Bordure,
                "workspace_bg",
                |p| p.workspace_bg,
                0x262729,
            ),
            (
                Mode::Sombre,
                Aplat::BordureSurvol,
                "workspace_bg",
                |p| p.workspace_bg,
                0x3d3d3d,
            ),
            (
                Mode::Sombre,
                Aplat::Survol,
                "workspace_bg",
                |p| p.workspace_bg,
                0x202123,
            ),
            (
                Mode::Sombre,
                Aplat::SurvolActif,
                "workspace_bg",
                |p| p.workspace_bg,
                0x28292b,
            ),
            (
                Mode::Sombre,
                Aplat::Bordure,
                "background_secondary",
                |p| p.background_secondary,
                0x232426,
            ),
            (
                Mode::Sombre,
                Aplat::BordureSurvol,
                "background_secondary",
                |p| p.background_secondary,
                0x3b3a3b,
            ),
            (
                Mode::Sombre,
                Aplat::Survol,
                "background_secondary",
                |p| p.background_secondary,
                0x1d1e20,
            ),
            (
                Mode::Sombre,
                Aplat::SurvolActif,
                "background_secondary",
                |p| p.background_secondary,
                0x252628,
            ),
            (
                Mode::Sombre,
                Aplat::Bordure,
                "menu_bg",
                |p| p.menu_bg,
                0x2d2e32,
            ),
            (
                Mode::Sombre,
                Aplat::BordureSurvol,
                "menu_bg",
                |p| p.menu_bg,
                0x434345,
            ),
            (
                Mode::Sombre,
                Aplat::Survol,
                "menu_bg",
                |p| p.menu_bg,
                0x27282c,
            ),
            (
                Mode::Sombre,
                Aplat::SurvolActif,
                "menu_bg",
                |p| p.menu_bg,
                0x2f3034,
            ),
            (
                Mode::Sombre,
                Aplat::Bordure,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0x212223,
            ),
            (
                Mode::Sombre,
                Aplat::BordureSurvol,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0x383839,
            ),
            (
                Mode::Sombre,
                Aplat::Survol,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0x1a1b1d,
            ),
            (
                Mode::Sombre,
                Aplat::SurvolActif,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0x232325,
            ),
            (
                Mode::Clair,
                Aplat::Bordure,
                "workspace_bg",
                |p| p.workspace_bg,
                0xe1e0dc,
            ),
            (
                Mode::Clair,
                Aplat::BordureSurvol,
                "workspace_bg",
                |p| p.workspace_bg,
                0xcbcac8,
            ),
            (
                Mode::Clair,
                Aplat::Survol,
                "workspace_bg",
                |p| p.workspace_bg,
                0xebeae8,
            ),
            (
                Mode::Clair,
                Aplat::SurvolActif,
                "workspace_bg",
                |p| p.workspace_bg,
                0xe1e0dc,
            ),
            (
                Mode::Clair,
                Aplat::Bordure,
                "background_secondary",
                |p| p.background_secondary,
                0xdad9d4,
            ),
            (
                Mode::Clair,
                Aplat::BordureSurvol,
                "background_secondary",
                |p| p.background_secondary,
                0xc4c3c0,
            ),
            (
                Mode::Clair,
                Aplat::Survol,
                "background_secondary",
                |p| p.background_secondary,
                0xe4e3df,
            ),
            (
                Mode::Clair,
                Aplat::SurvolActif,
                "background_secondary",
                |p| p.background_secondary,
                0xdad9d4,
            ),
            (
                Mode::Clair,
                Aplat::Bordure,
                "menu_bg",
                |p| p.menu_bg,
                0xe8e8e8,
            ),
            (
                Mode::Clair,
                Aplat::BordureSurvol,
                "menu_bg",
                |p| p.menu_bg,
                0xd1d1d2,
            ),
            (
                Mode::Clair,
                Aplat::Survol,
                "menu_bg",
                |p| p.menu_bg,
                0xf3f3f4,
            ),
            (
                Mode::Clair,
                Aplat::SurvolActif,
                "menu_bg",
                |p| p.menu_bg,
                0xe8e8e8,
            ),
            (
                Mode::Clair,
                Aplat::Bordure,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0xd3d2cd,
            ),
            (
                Mode::Clair,
                Aplat::BordureSurvol,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0xbfbeba,
            ),
            (
                Mode::Clair,
                Aplat::Survol,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0xdddcd8,
            ),
            (
                Mode::Clair,
                Aplat::SurvolActif,
                "tab_bar_bg",
                |p| p.tab_bar_bg,
                0xd3d2cd,
            ),
        ]
    }

    #[test]
    fn chaque_paire_jeton_et_fond_est_pre_composee_comme_chromium() {
        for (mode, jeton, nom, fond, attendu) in paires() {
            let palette = Theme::pour(mode).palette;
            assert_eq!(
                palette.aplat(jeton, fond(&palette)),
                Couleur::hex(attendu),
                "{mode:?} {jeton:?} sur {nom}"
            );
        }
    }

    #[test]
    fn ligne_selectionnee_de_la_palette_sombre_relue_dans_la_reference() {
        // Le cas qui a motivé la tâche : (40, 41, 43), pas (41, 41, 43) ; fond de la boîte (20, 21, 23).
        let t = Theme::sombre().palette;
        let attendu = pixel_de_reference("t1-palette-selection-dark.png", 700, 200);
        assert_eq!(attendu, Couleur::hex(0x28292b));
        assert_eq!(t.aplat(Aplat::SurvolActif, t.workspace_bg), attendu);
        assert_eq!(
            t.background_modifier_active_hover.sur(t.workspace_bg),
            Couleur::hex(0x29292b),
            "l'ancienne composition à l'octet se trompait d'un niveau"
        );
    }

    #[test]
    fn ligne_selectionnee_de_la_palette_claire_relue_dans_la_reference() {
        let t = Theme::clair().palette;
        let attendu = pixel_de_reference("t1-palette-selection-light.png", 700, 200);
        assert_eq!(t.aplat(Aplat::SurvolActif, t.workspace_bg), attendu);
    }

    #[test]
    fn survol_de_menu_clair_et_survols_de_l_explorateur_relus_dans_la_reference() {
        let c = Theme::clair().palette;
        let s = Theme::sombre().palette;
        let menu = pixel_de_reference("menu-light.png", 600, 481);
        assert_eq!(c.aplat(Aplat::SurvolActif, c.menu_bg), menu);
        let explo_s = pixel_de_reference("survol-explorateur-dark.png", 250, 117);
        assert_eq!(s.aplat(Aplat::Survol, s.background_secondary), explo_s);
        let explo_c = pixel_de_reference("survol-explorateur-light.png", 60, 117);
        assert_eq!(c.aplat(Aplat::Survol, c.background_secondary), explo_c);
    }
}
