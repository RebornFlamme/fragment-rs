//! `struct Theme` : les jetons de Fragment, un champ par jeton (ADR-005).
//!
//! Chaque champ porte le nom du jeton CSS en `snake_case` (`--radius-m` devient `radius_m`).
//! Les valeurs sont celles de la table de `docs/constantes-rendu.md`, au bit près ; alias,
//! `calc()` et `color-mix` sont résolus à la construction (voir `sombre.rs`, `clair.rs`).
//! Les longueurs sont en pixels logiques (f32), les durées en millisecondes.

use crate::couleur::{Couleur, Ombre};

/// Une pile de familles de polices, dans l'ordre de repli.
pub type Pile = &'static [&'static str];

/// Le mode de couleur : choisi par le réglage `theme` du cœur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    /// Thème sombre (défaut de Fragment).
    #[default]
    Sombre,
    /// Thème clair.
    Clair,
}

/// Déclare une famille de jetons et la liste de ses noms CSS (sans le préfixe `--`).
macro_rules! jetons {
    ($(#[$meta:meta])* $nom:ident { $($champ:ident : $type:ty,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $nom {
            $(
                #[allow(missing_docs)]
                pub $champ: $type,
            )*
        }

        impl $nom {
            /// Les noms des champs (donc des jetons), dans l'ordre de la déclaration.
            pub const CHAMPS: &'static [&'static str] = &[$(stringify!($champ)),*];
        }
    };
}

jetons! {
    /// Couleurs, ombres et paramètres des avatars : ce qui change d'un thème à l'autre.
    Palette {
        window_close_bg: Couleur,
        text_on_accent: Couleur,
        toggle_thumb_color: Couleur,
        background_modifier_cover: Couleur,
        background_overlay: Couleur,
        text_on_overlay: Couleur,
        background_modifier_error: Couleur,
        background_modifier_warning: Couleur,
        text_on_warning: Couleur,
        color_red: Couleur,
        color_orange: Couleur,
        color_yellow: Couleur,
        color_green: Couleur,
        color_cyan: Couleur,
        color_blue: Couleur,
        color_purple: Couleur,
        color_pink: Couleur,
        link_color: Couleur,
        text_highlight_bg: Couleur,
        code_normal: Couleur,
        code_comment: Couleur,
        code_keyword: Couleur,
        code_function: Couleur,
        code_property: Couleur,
        code_value: Couleur,
        code_tag: Couleur,
        code_operator: Couleur,
        code_string: Couleur,
        code_punctuation: Couleur,
        code_important: Couleur,
        interactive_accent: Couleur,
        interactive_accent_hover: Couleur,
        color_accent_1: Couleur,
        color_accent_2: Couleur,
        text_accent: Couleur,
        background_modifier_border: Couleur,
        background_modifier_border_hover: Couleur,
        background_modifier_hover: Couleur,
        background_modifier_active_hover: Couleur,
        scrollbar_thumb_bg: Couleur,
        scrollbar_active_thumb_bg: Couleur,
        workspace_bg: Couleur,
        color_accent: Couleur,
        tab_bar_bg: Couleur,
        background_secondary: Couleur,
        background_primary: Couleur,
        background_primary_alt: Couleur,
        menu_bg: Couleur,
        background_secondary_alt: Couleur,
        tab_outline_color: Couleur,
        text_normal: Couleur,
        text_muted: Couleur,
        text_faint: Couleur,
        text_error: Couleur,
        text_warning: Couleur,
        text_success: Couleur,
        menu_shadow: Vec<Ombre>,
        shadow_xs: Vec<Ombre>,
        shadow_s: Vec<Ombre>,
        shadow_card: Vec<Ombre>,
        shadow_l: Vec<Ombre>,
        avatar_bg_l: f32,
        avatar_bg_c: f32,
        avatar_fg_l: f32,
        avatar_fg_c: f32,
    }
}

jetons! {
    /// Géométrie et durées : identiques dans les deux thèmes.
    Forme {
        header_height: f32,
        frame_button_width: f32,
        frame_right_space: f32,
        border_width: f32,
        radius_hair: f32,
        radius_xxs: f32,
        radius_xs: f32,
        radius_s: f32,
        radius_sm: f32,
        radius_m: f32,
        radius_ml: f32,
        radius_l: f32,
        radius_xl: f32,
        input_radius: f32,
        button_radius: f32,
        checkbox_radius: f32,
        clickable_icon_radius: f32,
        menu_radius: f32,
        modal_radius: f32,
        tooltip_radius: f32,
        code_radius: f32,
        icon_xs: f32,
        icon_s: f32,
        icon_m: f32,
        icon_l: f32,
        icon_stroke: f32,
        size_2_1: f32,
        size_2_2: f32,
        size_2_3: f32,
        size_2_5: f32,
        size_2_7: f32,
        size_2_9: f32,
        size_2_11: f32,
        size_2_13: f32,
        size_2_15: f32,
        size_2_17: f32,
        size_2_19: f32,
        size_4_1: f32,
        size_4_2: f32,
        size_4_3: f32,
        size_4_4: f32,
        size_4_5: f32,
        size_4_6: f32,
        size_4_7: f32,
        size_4_8: f32,
        size_4_9: f32,
        size_4_10: f32,
        size_4_11: f32,
        size_4_12: f32,
        size_4_14: f32,
        size_4_16: f32,
        size_4_18: f32,
        size_4_20: f32,
        anim_duration_none: u32,
        anim_duration_superfast: u32,
        anim_duration_quick: u32,
        anim_duration_fast: u32,
        anim_duration_moderate: u32,
        anim_duration_slow: u32,
        anim_motion_smooth: [f32; 4],
        anim_motion_in_out: [f32; 4],
    }
}

jetons! {
    /// Polices et échelle typographique : identiques dans les deux thèmes.
    Typo {
        font_interface: Pile,
        font_text: Pile,
        font_ui_small: f32,
        font_monospace: Pile,
        font_serif: Pile,
        font_ui_smallest: f32,
        font_ui_smaller: f32,
        font_ui_medium: f32,
        font_ui_normal: f32,
        font_ui_large: f32,
        font_ui_larger: f32,
        font_ui_largest: f32,
        font_normal: u32,
        font_medium: u32,
        font_semibold: u32,
        font_bold: u32,
        line_height_normal: f32,
    }
}

impl Forme {
    /// La forme commune aux deux thèmes (`:root` de `index.css`).
    #[allow(clippy::too_many_lines)]
    pub fn commune() -> Self {
        let header_height = 36.0; // index.css:63
        let frame_button_width = 46.0; // index.css:73
        let frame_right_space = 3.0 * frame_button_width; // index.css:74
        let border_width = 1.0; // index.css:83
        let radius_hair = 1.0; // index.css:87
        let radius_xxs = 2.0; // index.css:88
        let radius_xs = 3.0; // index.css:89
        let radius_s = 4.0; // index.css:90
        let radius_sm = 6.0; // index.css:91
        let radius_m = 8.0; // index.css:92
        let radius_ml = 10.0; // index.css:93
        let radius_l = 12.0; // index.css:94
        let radius_xl = 16.0; // index.css:95
        let input_radius = 5.0; // index.css:97
        let button_radius = input_radius; // index.css:98
        let checkbox_radius = radius_xs; // index.css:99
        let clickable_icon_radius = radius_sm; // index.css:100
        let menu_radius = radius_m; // index.css:101
        let modal_radius = radius_ml; // index.css:102
        let tooltip_radius = radius_sm; // index.css:103
        let code_radius = radius_xs; // index.css:104
        let icon_xs = 14.0; // index.css:127
        let icon_s = 16.0; // index.css:128
        let icon_m = 18.0; // index.css:129
        let icon_l = 18.0; // index.css:130
        let icon_stroke = 1.5; // index.css:131
        let size_2_1 = 2.0; // index.css:136
        let size_2_2 = 4.0; // index.css:137
        let size_2_3 = 6.0; // index.css:138
        let size_2_5 = 10.0; // index.css:139
        let size_2_7 = 14.0; // index.css:140
        let size_2_9 = 18.0; // index.css:141
        let size_2_11 = 22.0; // index.css:142
        let size_2_13 = 26.0; // index.css:143
        let size_2_15 = 30.0; // index.css:144
        let size_2_17 = 34.0; // index.css:145
        let size_2_19 = 38.0; // index.css:146
        let size_4_1 = 4.0; // index.css:147
        let size_4_2 = 8.0; // index.css:148
        let size_4_3 = 12.0; // index.css:149
        let size_4_4 = 16.0; // index.css:150
        let size_4_5 = 20.0; // index.css:151
        let size_4_6 = 24.0; // index.css:152
        let size_4_7 = 28.0; // index.css:153
        let size_4_8 = 32.0; // index.css:154
        let size_4_9 = 36.0; // index.css:155
        let size_4_10 = 40.0; // index.css:156
        let size_4_11 = 44.0; // index.css:157
        let size_4_12 = 48.0; // index.css:158
        let size_4_14 = 56.0; // index.css:159
        let size_4_16 = 64.0; // index.css:160
        let size_4_18 = 72.0; // index.css:161
        let size_4_20 = 80.0; // index.css:162
        let anim_duration_none = 0; // index.css:166
        let anim_duration_superfast = 80; // index.css:167
        let anim_duration_quick = 90; // index.css:168
        let anim_duration_fast = 120; // index.css:169
        let anim_duration_moderate = 150; // index.css:170
        let anim_duration_slow = 250; // index.css:171
        let anim_motion_smooth = [0.2, 0.0, 0.0, 1.0]; // index.css:172
        let anim_motion_in_out = [0.45, 0.0, 0.55, 1.0]; // index.css:173
        Self {
            header_height,
            frame_button_width,
            frame_right_space,
            border_width,
            radius_hair,
            radius_xxs,
            radius_xs,
            radius_s,
            radius_sm,
            radius_m,
            radius_ml,
            radius_l,
            radius_xl,
            input_radius,
            button_radius,
            checkbox_radius,
            clickable_icon_radius,
            menu_radius,
            modal_radius,
            tooltip_radius,
            code_radius,
            icon_xs,
            icon_s,
            icon_m,
            icon_l,
            icon_stroke,
            size_2_1,
            size_2_2,
            size_2_3,
            size_2_5,
            size_2_7,
            size_2_9,
            size_2_11,
            size_2_13,
            size_2_15,
            size_2_17,
            size_2_19,
            size_4_1,
            size_4_2,
            size_4_3,
            size_4_4,
            size_4_5,
            size_4_6,
            size_4_7,
            size_4_8,
            size_4_9,
            size_4_10,
            size_4_11,
            size_4_12,
            size_4_14,
            size_4_16,
            size_4_18,
            size_4_20,
            anim_duration_none,
            anim_duration_superfast,
            anim_duration_quick,
            anim_duration_fast,
            anim_duration_moderate,
            anim_duration_slow,
            anim_motion_smooth,
            anim_motion_in_out,
        }
    }
}

impl Typo {
    /// La typographie commune aux deux thèmes (`:root` de `index.css`).
    pub fn commune() -> Self {
        let font_interface = &[
            "DM Sans",
            "system-ui",
            "-apple-system",
            "Segoe UI",
            "sans-serif",
        ]; // index.css:52
        let font_text = font_interface; // index.css:53
        let font_ui_small = 12.0; // index.css:67, 113
        let font_monospace = &[
            "JetBrains Mono",
            "ui-monospace",
            "SFMono-Regular",
            "Menlo",
            "monospace",
        ]; // index.css:107
        let font_serif = &["Georgia", "Times New Roman", "serif"]; // index.css:108
        let font_ui_smallest = 10.0; // index.css:111
        let font_ui_smaller = 11.0; // index.css:112
        let font_ui_medium = 13.0; // index.css:114
        let font_ui_normal = 14.0; // index.css:115
        let font_ui_large = 15.0; // index.css:116
        let font_ui_larger = 18.0; // index.css:117
        let font_ui_largest = 20.0; // index.css:118
        let font_normal = 400; // index.css:120
        let font_medium = 500; // index.css:121
        let font_semibold = 600; // index.css:122
        let font_bold = 700; // index.css:123
        let line_height_normal = 1.5; // index.css:124
        Self {
            font_interface,
            font_text,
            font_ui_small,
            font_monospace,
            font_serif,
            font_ui_smallest,
            font_ui_smaller,
            font_ui_medium,
            font_ui_normal,
            font_ui_large,
            font_ui_larger,
            font_ui_largest,
            font_normal,
            font_medium,
            font_semibold,
            font_bold,
            line_height_normal,
        }
    }
}

/// Le thème complet : tout ce qu'un composant lit pour se dessiner.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Sombre ou clair.
    pub mode: Mode,
    /// Couleurs et ombres.
    pub palette: Palette,
    /// Géométrie et durées.
    pub forme: Forme,
    /// Polices et tailles.
    pub typo: Typo,
}

impl Theme {
    /// Le thème sombre, valeurs de `.theme-dark` + `body` + `:root`.
    pub fn sombre() -> Self {
        Self {
            mode: Mode::Sombre,
            palette: crate::sombre::palette(),
            forme: Forme::commune(),
            typo: Typo::commune(),
        }
    }

    /// Le thème clair, valeurs de `.theme-light` + `body` + `:root`.
    pub fn clair() -> Self {
        Self {
            mode: Mode::Clair,
            palette: crate::clair::palette(),
            forme: Forme::commune(),
            typo: Typo::commune(),
        }
    }

    /// Le thème d'un mode donné.
    pub fn pour(mode: Mode) -> Self {
        match mode {
            Mode::Sombre => Self::sombre(),
            Mode::Clair => Self::clair(),
        }
    }

    /// Les noms CSS (`--radius-m`) de tous les jetons, dans l'ordre palette, forme, typo.
    pub fn noms_des_jetons() -> Vec<String> {
        [Palette::CHAMPS, Forme::CHAMPS, Typo::CHAMPS]
            .into_iter()
            .flatten()
            .map(|champ| format!("--{}", champ.replace('_', "-")))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Les noms de jetons de la section « Jetons » de `docs/constantes-rendu.md`.
    fn noms_de_la_table() -> Vec<String> {
        let chemin = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/constantes-rendu.md"
        );
        let texte = std::fs::read_to_string(chemin).expect("table des constantes lisible");
        let section = texte
            .split("\n## Jetons")
            .nth(1)
            .expect("section Jetons")
            .split("\n## Constantes de mise en page")
            .next()
            .expect("fin de la section");
        section
            .lines()
            .filter_map(|ligne| ligne.strip_prefix("| `"))
            .filter_map(|reste| reste.split('`').next())
            .filter(|nom| nom.starts_with("--"))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn chaque_ligne_de_la_table_a_son_jeton_et_inversement() {
        let table = noms_de_la_table();
        let theme = Theme::noms_des_jetons();
        assert!(
            table.len() > 140,
            "la table doit être lue : {}",
            table.len()
        );
        assert_eq!(
            table.len(),
            theme.len(),
            "même nombre de jetons que de lignes"
        );
        let a: BTreeSet<_> = table.iter().collect();
        let b: BTreeSet<_> = theme.iter().collect();
        assert_eq!(a.len(), table.len(), "aucun doublon dans la table");
        assert_eq!(a, b, "jeton oublié ou en trop");
    }

    #[test]
    fn les_valeurs_cles_du_theme_sombre() {
        let t = Theme::sombre();
        assert_eq!(t.mode, Mode::Sombre);
        assert_eq!(t.palette.background_primary, Couleur::hex(0x141517));
        assert_eq!(t.palette.color_accent, Couleur::hex(0x646cff));
        assert_eq!(t.palette.text_normal, Couleur::hex(0xdfded9));
        assert_eq!(t.forme.header_height, 36.0);
        assert_eq!(t.forme.radius_m, 8.0);
        assert_eq!(t.forme.menu_radius, 8.0, "alias résolu");
        assert_eq!(t.forme.frame_right_space, 138.0, "calc résolu");
        assert_eq!(t.typo.font_ui_small, 12.0);
        assert_eq!(t.typo.font_interface[0], "DM Sans");
        assert_eq!(t.typo.font_text, t.typo.font_interface);
    }

    #[test]
    fn les_valeurs_cles_du_theme_clair() {
        let t = Theme::clair();
        assert_eq!(t.mode, Mode::Clair);
        assert_eq!(t.palette.background_primary, Couleur::hex(0xf7f6f2));
        assert_eq!(t.palette.color_accent, Couleur::hex(0x4b52d6));
        assert_eq!(
            t.palette.text_accent, t.palette.color_accent,
            "alias de l'accent"
        );
        assert_eq!(t.palette.menu_bg, Couleur::hex(0xffffff));
        assert_eq!(t.forme, Theme::sombre().forme);
        assert_eq!(t.typo, Theme::sombre().typo);
    }

    #[test]
    fn les_ombres_sont_resolues() {
        let t = Theme::sombre();
        assert_eq!(t.palette.menu_shadow.len(), 1);
        assert_eq!(t.palette.menu_shadow[0].decalage_y, 8.0);
        assert_eq!(t.palette.menu_shadow[0].flou, 24.0);
        assert_eq!(t.palette.shadow_card.len(), 2, "deux couches");
        assert_eq!(t.palette.shadow_xs[0].decalage_y, 0.5);
    }

    #[test]
    fn pour_choisit_le_mode() {
        assert_eq!(Theme::pour(Mode::Clair), Theme::clair());
        assert_eq!(Theme::pour(Mode::Sombre), Theme::sombre());
    }
}
