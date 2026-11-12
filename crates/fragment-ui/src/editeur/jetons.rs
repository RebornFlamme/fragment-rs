// Port de la résolution des variables CSS d'app/src/ui/views/markdown/editor.css (marques et lignes
// du live preview) et de `codeHighlightStyle` (language.ts) vers des styles de texte GPUI.
//! Les NOMS de jetons que rend `fragment_markdown::decorations` (et `JetonCode::nom`) résolus dans
//! le thème courant, puis traduits en `HighlightStyle` pour les décorations de texte du fork.
//!
//! Aucun littéral de couleur : un nom inconnu ne colore rien (et un test le refuse).
//!
//! ★ Le fork ne sait poser par plage que couleur, fond, graisse, italique, soulignement, barré et
//!   fondu (`HighlightStyle`). La POLICE et la TAILLE d'une plage (le code en ligne : monospace à
//!   0,9 em), les coins et la marge intérieure d'un fond n'ont pas d'équivalent : dette notée dans
//!   la PR et dans `parite.d/fr-t3-surface.toml`.

use fragment_markdown::decorations::{StyleLigne, StyleMarque};
use fragment_markdown::syntaxe::imbrique::JetonCode;
use fragment_theme::{Aplat, Couleur, Theme};
use gpui_kit::{
    FontStyle, FontWeight, HighlightStyle, Hsla, StrikethroughStyle, UnderlineStyle, px,
};

use crate::theme_global::couleur;

/// La couleur du jeton `nom` (un champ de `Palette`). Les jetons translucides de fond sont
/// pré-composés sur le fond de la note (`background_primary`), comme Chromium (ADR-009 c).
#[must_use]
pub fn couleur_du_jeton(theme: &Theme, nom: &str) -> Option<Couleur> {
    let p = &theme.palette;
    let fond = p.background_primary;
    Some(match nom {
        "link_color" => p.link_color,
        "text_highlight_bg" => p.text_highlight_bg,
        "text_normal" => p.text_normal,
        "text_muted" => p.text_muted,
        "text_faint" => p.text_faint,
        "text_accent" => p.text_accent,
        "background_modifier_hover" => p.aplat(Aplat::Survol, fond),
        "background_modifier_border" => p.aplat(Aplat::Bordure, fond),
        "background_modifier_border_hover" => p.aplat(Aplat::BordureSurvol, fond),
        "code_normal" => p.code_normal,
        "code_comment" => p.code_comment,
        "code_keyword" => p.code_keyword,
        "code_function" => p.code_function,
        "code_property" => p.code_property,
        "code_value" => p.code_value,
        "code_tag" => p.code_tag,
        "code_operator" => p.code_operator,
        "code_string" => p.code_string,
        "code_punctuation" => p.code_punctuation,
        "code_important" => p.code_important,
        _ => return None,
    })
}

/// La graisse du jeton `nom` (un champ de `Typo`).
#[must_use]
pub fn graisse_du_jeton(theme: &Theme, nom: &str) -> Option<FontWeight> {
    let t = &theme.typo;
    let g = match nom {
        "font_normal" => t.font_normal,
        "font_medium" => t.font_medium,
        "font_semibold" => t.font_semibold,
        "font_bold" => t.font_bold,
        _ => return None,
    };
    // Les graisses CSS sont des entiers de 100 à 900 : exactes en f32.
    #[allow(clippy::cast_precision_loss)]
    Some(FontWeight(g as f32))
}

fn couleur_gpui(theme: &Theme, nom: Option<&str>) -> Option<Hsla> {
    nom.and_then(|n| couleur_du_jeton(theme, n))
        .map(|c| couleur(c).into())
}

/// Le style d'une marque en ligne (`.cm-ob-strong`, `.cm-hl`, `.cm-ob-code`…).
#[must_use]
pub fn style_marque(theme: &Theme, s: &StyleMarque) -> HighlightStyle {
    HighlightStyle {
        color: couleur_gpui(theme, s.couleur),
        background_color: couleur_gpui(theme, s.fond),
        font_weight: s.graisse.and_then(|g| graisse_du_jeton(theme, g)),
        font_style: s.italique.then_some(FontStyle::Italic),
        underline: s.souligne.then_some(UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        }),
        strikethrough: s.barre.then_some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        }),
        // `opacity: 0.7` = un fondu de 0,3.
        fade_out: s.opacite.map(|o| 1.0 - o),
    }
}

/// Le style d'une ligne entière, réduit à ce qui se pose sur son TEXTE (couleur, graisse,
/// italique, opacité). Le fond, la bordure, les coins et les traits d'une ligne sont dessinés par
/// l'hôte des blocs (`md_blocs`, tâche `fr-t3-widgets-blocs`), pas par une décoration de texte.
#[must_use]
pub fn style_ligne(theme: &Theme, s: &StyleLigne) -> HighlightStyle {
    HighlightStyle {
        color: couleur_gpui(theme, s.couleur),
        font_weight: s.graisse.and_then(|g| graisse_du_jeton(theme, g)),
        font_style: s.italique.then_some(FontStyle::Italic),
        fade_out: s.opacite.map(|o| 1.0 - o),
        ..HighlightStyle::default()
    }
}

/// Le style d'un jeton de coloration du code (`--code-*`, commentaires en italique).
#[must_use]
pub fn style_code(theme: &Theme, j: JetonCode) -> HighlightStyle {
    HighlightStyle {
        color: couleur_gpui(theme, Some(j.nom())),
        font_style: j.italique().then_some(FontStyle::Italic),
        ..HighlightStyle::default()
    }
}

/// Le jeton translucide de la sélection (`::selection`, accent à 35 %).
#[must_use]
pub fn couleur_selection(theme: &Theme) -> Hsla {
    let mut c: Hsla = couleur(theme.palette.color_accent).into();
    c.a *= super::constantes::OPACITE_SELECTION;
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::decorations::{GenreLigne, GenreMarque};

    /// Chaque nom de jeton que le modèle du live preview peut rendre se résout dans les deux thèmes.
    #[test]
    fn chaque_jeton_du_live_preview_se_resout() {
        let marques = [
            GenreMarque::Gras,
            GenreMarque::Italique,
            GenreMarque::Barre,
            GenreMarque::Surligne,
            GenreMarque::Code,
            GenreMarque::Lien,
        ];
        let lignes = [
            GenreLigne::Titre(1),
            GenreLigne::Titre(6),
            GenreLigne::Citation,
            GenreLigne::Code,
            GenreLigne::CodePremiere,
            GenreLigne::CodeDerniere,
            GenreLigne::Indentation { niveaux: 2 },
        ];
        for theme in [Theme::sombre(), Theme::clair()] {
            for m in marques {
                let s = m.style();
                for c in [s.couleur, s.fond].into_iter().flatten() {
                    assert!(couleur_du_jeton(&theme, c).is_some(), "{c}");
                }
                if let Some(g) = s.graisse {
                    assert!(graisse_du_jeton(&theme, g).is_some(), "{g}");
                }
            }
            for l in lignes {
                let s = l.style();
                let traits = s.traits.map(|t| t.1);
                let bordure = s.bordure_gauche.map(|b| b.1);
                for c in [s.couleur, s.fond, traits, bordure].into_iter().flatten() {
                    assert!(couleur_du_jeton(&theme, c).is_some(), "{c}");
                }
            }
            for j in JetonCode::TOUS {
                assert!(couleur_du_jeton(&theme, j.nom()).is_some(), "{}", j.nom());
            }
        }
    }

    #[test]
    fn gras_et_lien_et_barre() {
        let t = Theme::sombre();
        let gras = style_marque(&t, &GenreMarque::Gras.style());
        assert_eq!(gras.font_weight, Some(FontWeight(700.0)));
        let lien = style_marque(&t, &GenreMarque::Lien.style());
        assert_eq!(lien.color, Some(couleur(t.palette.link_color).into()));
        assert!(lien.underline.is_some());
        let barre = style_marque(&t, &GenreMarque::Barre.style());
        assert!(barre.strikethrough.is_some());
        assert!((barre.fade_out.unwrap_or_default() - 0.3).abs() < 1e-6);
    }
}
