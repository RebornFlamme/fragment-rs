// Port de app/src/ui/workspace.css (le titre d'un en-tête d'onglet : une ligne, `overflow: hidden`,
// `text-overflow: ellipsis`), tel que Chromium le tronque et le peint.
//
//! Le titre d'un en-tête d'onglet.
//!
//! GPUI sait ellipser (`text_ellipsis`), mais pas comme Chromium (décisions du chef, fr-t1.5) :
//! - [B] GPUI retire l'espace et la ponctuation qui précèdent « … » ; Chromium les garde
//!   (« New … ») ;
//! - [A] quand « … » ne tient pas lui-même dans la boîte, Chromium n'ellipse pas : le titre est
//!   COUPÉ au bord (débordement masqué) ; GPUI met « … » quand même.
//!
//! Ici la troncature est faite à la main : la ligne est mesurée par le système de texte de GPUI à
//! la peinture (la largeur de la boîte n'est connue qu'alors), coupée par [`couper`], et la chaîne
//! déjà tronquée est peinte. La boîte garde sa hauteur de ligne par une copie invisible du titre.
//!
//! ★ Le titre est peint au bord gauche EXACT de la pile (sous-pixel) : Chromium ne cale au pixel que
//! les fonds et le contour, pas le texte. GPUI cale la mise en page (un `left(−0,5)` n'y changeait
//! rien, CI d4a5542) ; le décalage est donc appliqué à l'origine de la peinture.

use gpui_kit::{
    App, Bounds, ContentMask, IntoElement, ParentElement as _, Pixels, SharedString, Styled as _,
    TextAlign, Visibility, Window, canvas, div, point, px,
};

use crate::echelle::u;

/// L'ellipse de `text-overflow: ellipsis`.
pub const POINTS: &str = "\u{2026}";

/// La marge de comparaison des largeurs : l'unité de mise en page de Chromium (1/64 px).
const MARGE: f32 = 1.0 / 64.0;

/// Ce que devient un titre dans une boîte trop étroite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coupure {
    /// Il tient : peint tel quel.
    Entier,
    /// Tronqué à l'octet donné (frontière de caractère), suivi de « … ».
    Tronque(usize),
    /// « … » ne tient pas : le titre entier, coupé au bord par le masque.
    Coupe,
}

/// La règle de Chromium.
///
/// - `largeur` : la largeur du titre entier ;
/// - `dispo` : celle de la boîte ;
/// - `points` : celle de « … » ;
/// - `bornes` : les frontières de caractère (octet, abscisse de la frontière), croissantes, du
///   début (0) à la fin exclue.
///
/// On garde le plus long préfixe qui laisse la place de « … », espaces compris : Chromium ne les
/// retire pas.
pub fn couper(largeur: f32, dispo: f32, points: f32, bornes: &[(usize, f32)]) -> Coupure {
    if largeur <= dispo + MARGE {
        return Coupure::Entier;
    }
    if points > dispo + MARGE {
        return Coupure::Coupe;
    }
    let place = dispo - points + MARGE;
    bornes
        .iter()
        .rev()
        .find(|(_, x)| *x <= place)
        .map_or(Coupure::Tronque(0), |(i, _)| Coupure::Tronque(*i))
}

/// Le titre d'un onglet : prend la place qui reste (`flex: 1`), tronqué par [`couper`], peint
/// décalé de `decalage` px logiques (le bord gauche exact de la pile moins son bord calé).
pub fn titre(texte: SharedString, decalage: f32, cx: &App) -> impl IntoElement {
    let mut gabarit = div().child(texte.clone());
    // Invisible mais mesurée : la boîte garde la hauteur de ligne du titre.
    gabarit.style().visibility = Some(Visibility::Hidden);
    div()
        .relative()
        .flex_1()
        .min_w(u(cx, 0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .child(gabarit)
        .child(
            canvas(
                |_, _, _| {},
                move |bornes, (), window, cx| peindre(&texte, decalage, bornes, window, cx),
            )
            .absolute()
            .top(u(cx, 0.0))
            .left(u(cx, 0.0))
            .right(u(cx, 0.0))
            .bottom(u(cx, 0.0)),
        )
}

/// Mesure, coupe et peint `texte` dans `bornes`.
fn peindre(
    texte: &SharedString,
    decalage: f32,
    bornes: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let style = window.text_style();
    let taille = style.font_size.to_pixels(window.rem_size());
    let interligne = window.pixel_snap(
        style
            .line_height
            .to_pixels(taille.into(), window.rem_size()),
    );
    let forme = |s: SharedString, window: &mut Window| {
        let run = style.to_run(s.len());
        window.text_system().shape_line(s, taille, &[run], None)
    };
    let entier = forme(texte.clone(), window);
    let points = forme(POINTS.into(), window);
    let bornes_car: Vec<(usize, f32)> = texte
        .char_indices()
        .map(|(i, _)| (i, f32::from(entier.x_for_index(i))))
        .collect();
    let coupure = couper(
        f32::from(entier.width()),
        f32::from(bornes.size.width),
        f32::from(points.width()),
        &bornes_car,
    );
    let ligne = match coupure {
        Coupure::Entier | Coupure::Coupe => entier,
        Coupure::Tronque(i) => forme(format!("{}{POINTS}", &texte[..i]).into(), window),
    };
    let zoom = f32::from(u(cx, 1.0));
    let origine = point(bornes.origin.x + px(decalage * zoom), bornes.origin.y);
    window.with_content_mask(Some(ContentMask { bounds: bornes }), |window| {
        // Une erreur de peinture d'un titre ne doit pas faire tomber l'image : rien n'est peint.
        let _ = ligne.paint(origine, interligne, TextAlign::Left, None, window, cx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// « New Tab » à 7 px par caractère.
    fn bornes() -> Vec<(usize, f32)> {
        (0..7).map(|i| (i, i as f32 * 7.0)).collect()
    }

    #[test]
    fn un_titre_qui_tient_est_entier() {
        assert_eq!(couper(49.0, 49.0, 10.0, &bornes()), Coupure::Entier);
        assert_eq!(couper(49.0, 60.0, 10.0, &bornes()), Coupure::Entier);
    }

    #[test]
    fn l_espace_avant_les_points_est_garde() {
        // 31 px : « New » (21) + espace (28) + « … » (10) = 38 > 31 ; « New » + « … » = 31 tient.
        assert_eq!(couper(49.0, 31.0, 10.0, &bornes()), Coupure::Tronque(3));
        // 38 px : « New » ET l'espace tiennent avant « … » (28 + 10) : l'espace reste.
        assert_eq!(couper(49.0, 38.0, 10.0, &bornes()), Coupure::Tronque(4));
    }

    #[test]
    fn quand_les_points_ne_tiennent_pas_le_titre_est_coupe() {
        assert_eq!(couper(49.0, 9.0, 10.0, &bornes()), Coupure::Coupe);
        // Les points seuls tiennent : rien du titre, « … » seul.
        assert_eq!(couper(49.0, 12.0, 10.0, &bornes()), Coupure::Tronque(0));
    }
}
