//! Palette du thème sombre : une ligne par jeton, valeurs de la table de `docs/constantes-rendu.md`
//! (relevé de `index.css`, classe `.theme-dark` et `body`). Alias, `calc()` et `color-mix` sont résolus ici.

use crate::couleur::{Couleur, Ombre};
use crate::theme::{Opacites, Palette};

/// La palette du thème sombre.
#[allow(clippy::too_many_lines)]
pub fn palette() -> Palette {
    let window_close_bg = Couleur::hex(0xe81123); // index.css:184
    let text_on_accent = Couleur::hex(0xffffff); // index.css:186
    let toggle_thumb_color = Couleur::hex(0xffffff); // index.css:187
    let background_modifier_cover = Couleur::css(0, 0, 0, 0.5); // index.css:189
    let background_overlay = Couleur::css(0, 0, 0, 0.75); // index.css:190
    let text_on_overlay = Couleur::hex(0xffffff); // index.css:191
    let background_modifier_error = Couleur::hex(0xa52828); // index.css:193
    let background_modifier_warning = Couleur::hex(0xd29922); // index.css:195
    let text_on_warning = Couleur::hex(0x1a1a1a); // index.css:196
    let color_red = Couleur::hex(0xe93147); // index.css:199
    let color_orange = Couleur::hex(0xec7500); // index.css:200
    let color_yellow = Couleur::hex(0xe0ac00); // index.css:201
    let color_green = Couleur::hex(0x4caf82); // index.css:202
    let color_cyan = Couleur::hex(0x56b6c2); // index.css:203
    let color_blue = Couleur::hex(0x5b9dff); // index.css:204
    let color_purple = Couleur::hex(0x7852ee); // index.css:205
    let color_pink = Couleur::hex(0xd53984); // index.css:206
    let link_color = color_blue; // index.css:209
    let text_highlight_bg = Couleur::css(255, 238, 0, 0.22); // index.css:211
    let code_normal = Couleur::hex(0xabb2bf); // index.css:214
    let code_comment = Couleur::hex(0x7f848e); // index.css:215
    let code_keyword = Couleur::hex(0xc678dd); // index.css:216
    let code_function = Couleur::hex(0x61afef); // index.css:217
    let code_property = Couleur::hex(0xe06c75); // index.css:218
    let code_value = Couleur::hex(0xd19a66); // index.css:219
    let code_tag = Couleur::hex(0xe5c07b); // index.css:220
    let code_operator = Couleur::hex(0x56b6c2); // index.css:221
    let code_string = Couleur::hex(0x98c379); // index.css:222
    let code_punctuation = Couleur::hex(0xabb2bf); // index.css:223
    let code_important = Couleur::hex(0xffffff); // index.css:224
    let color_accent = Couleur::hex(0x646cff); // index.css:265
    let interactive_accent = color_accent; // index.css:228
    let text_normal = Couleur::hex(0xdfded9); // index.css:276
    let interactive_accent_hover = color_accent.melange(85.0, text_normal); // index.css:229
    let color_accent_1 = color_accent.melange(70.0, Couleur::BLANC); // index.css:231
    let color_accent_2 = color_accent.melange(75.0, Couleur::NOIR); // index.css:232
    let text_accent = color_accent.melange(75.0, Couleur::BLANC); // index.css:235
    let background_modifier_border = text_normal.melange(9.0, Couleur::TRANSPARENT); // index.css:239
    let background_modifier_border_hover = text_normal.melange(20.0, Couleur::TRANSPARENT); // index.css:240
    let background_modifier_hover = text_normal.melange(6.0, Couleur::TRANSPARENT); // index.css:241
    let background_modifier_active_hover = text_normal.melange(10.0, Couleur::TRANSPARENT); // index.css:242
    let scrollbar_thumb_bg = text_normal.melange(20.0, Couleur::TRANSPARENT); // index.css:243
    let scrollbar_active_thumb_bg = text_normal.melange(30.0, Couleur::TRANSPARENT); // index.css:244
    let background_primary = Couleur::hex(0x141517); // index.css:269
    let workspace_bg = background_primary; // index.css:249
    let tab_bar_bg = Couleur::hex(0x0e0f11); // index.css:267
    let background_secondary = Couleur::hex(0x111214); // index.css:268
    let background_primary_alt = Couleur::hex(0x191a1e); // index.css:270
    let menu_bg = Couleur::hex(0x1c1d21); // index.css:271
    let background_secondary_alt = Couleur::hex(0x222328); // index.css:272
    let tab_outline_color = Couleur::hex(0x34363c); // index.css:273
    let text_muted = Couleur::hex(0xa3a29d); // index.css:277
    let text_faint = Couleur::hex(0x6f6e6a); // index.css:278
    let text_error = Couleur::hex(0xe5534b); // index.css:279
    let text_warning = Couleur::hex(0xd29922); // index.css:280
    let text_success = Couleur::hex(0x4caf82); // index.css:281
    let menu_shadow = vec![Ombre::new(0.0, 8.0, 24.0, 0.0, Couleur::css(0, 0, 0, 0.55))]; // index.css:283
    let shadow_xs = vec![Ombre::new(0.0, 0.5, 1.5, 0.0, Couleur::css(0, 0, 0, 0.22))]; // index.css:286
    let shadow_s = vec![Ombre::new(0.0, 2.0, 8.0, 0.0, Couleur::css(0, 0, 0, 0.4))]; // index.css:287
    let shadow_card = vec![
        Ombre::new(0.0, 1.0, 2.0, 0.0, Couleur::css(0, 0, 0, 0.06)),
        Ombre::new(0.0, 6.0, 20.0, 0.0, Couleur::css(0, 0, 0, 0.08)),
    ]; // index.css:288
    let shadow_l = vec![Ombre::new(0.0, 16.0, 48.0, 0.0, Couleur::css(0, 0, 0, 0.6))]; // index.css:289
    let avatar_bg_l = 0.37; // index.css:292
    let avatar_bg_c = 0.07; // index.css:293
    let avatar_fg_l = 0.9; // index.css:294
    let avatar_fg_c = 0.05; // index.css:295
    Palette {
        window_close_bg,
        text_on_accent,
        toggle_thumb_color,
        background_modifier_cover,
        background_overlay,
        text_on_overlay,
        background_modifier_error,
        background_modifier_warning,
        text_on_warning,
        color_red,
        color_orange,
        color_yellow,
        color_green,
        color_cyan,
        color_blue,
        color_purple,
        color_pink,
        link_color,
        text_highlight_bg,
        code_normal,
        code_comment,
        code_keyword,
        code_function,
        code_property,
        code_value,
        code_tag,
        code_operator,
        code_string,
        code_punctuation,
        code_important,
        interactive_accent,
        interactive_accent_hover,
        color_accent_1,
        color_accent_2,
        text_accent,
        background_modifier_border,
        background_modifier_border_hover,
        background_modifier_hover,
        background_modifier_active_hover,
        scrollbar_thumb_bg,
        scrollbar_active_thumb_bg,
        workspace_bg,
        color_accent,
        tab_bar_bg,
        background_secondary,
        background_primary,
        background_primary_alt,
        menu_bg,
        background_secondary_alt,
        tab_outline_color,
        text_normal,
        text_muted,
        text_faint,
        text_error,
        text_warning,
        text_success,
        menu_shadow,
        shadow_xs,
        shadow_s,
        shadow_card,
        shadow_l,
        avatar_bg_l,
        avatar_bg_c,
        avatar_fg_l,
        avatar_fg_c,
        opacites: Opacites {
            bordure: 9.0,
            bordure_survol: 20.0,
            survol: 6.0,
            survol_actif: 10.0,
        },
    }
}
