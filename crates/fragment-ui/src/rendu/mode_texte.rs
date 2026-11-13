//! Le mode de rastérisation du texte, choisi par PROPRIÉTÉ de la surface, comme Chromium
//! (`fr-p4.1.4`, ADR-017, mesure `docs/mesures/rasterisation-glyphes.md`).
//!
//! Chromium peint en ClearType le texte posé dans le calque opaque de la page, et en gris celui
//! d'un calque composé à part (surface `position: fixed` superposée : menu, modale, palette,
//! notice, infobulle, barre d'état) ou d'un effet d'opacité < 1 (surface intermédiaire
//! transparente). Ici, trois règles, toutes sans liste de noms :
//! 1. **Surface opaque** : ce que peint [`surface_opaque`] (le contenu en flux d'une fenêtre) prend
//!    le mode opaque installé par [`installer`] (ClearType en capture, celui du système dans l'app).
//! 2. **Calque** : tout le reste est gris. Le mode de base de l'App est le gris, donc :
//!    - les calques d'une [`pile`] (tout enfant posé au-dessus du fond) sont gris ;
//!    - les dessins différés de GPUI (`deferred`, menus, infobulles, listes déroulantes) sont
//!      peints APRÈS l'arbre, hors de toute surface opaque : gris sans une ligne de code.
//! 3. **Opacité < 1** : gris, décidé par GPUI lui-même au glyphe près (fork n°2, ADR-017,
//!    `vendor/gpui-pre.patch`), survol compris.
//!
//! ★ GPUI lit le mode glyphe par glyphe au moment de peindre (`Window::paint_glyph`) dans un
//! `Rc<Cell<_>>` partagé par l'App et ses fenêtres : poser le mode avant de peindre un sous-arbre et
//! le remettre après suffit. Gris et ClearType ont des clés d'atlas distinctes.

use gpui_kit::{
    AnyElement, App, Element, ElementId, Global, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, ParentElement as _, Pixels, Styled as _, TextRenderingMode, Window, div,
};

/// Le mode du texte des surfaces opaques, installé une fois par l'application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeOpaque(pub TextRenderingMode);

impl Global for ModeOpaque {}

/// Installe la règle : `opaque` pour les surfaces opaques, le gris pour tout le reste (mode de base).
pub fn installer(cx: &mut App, opaque: TextRenderingMode) {
    cx.set_global(ModeOpaque(opaque));
    cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
}

/// Le mode des surfaces opaques (celui du système tant que rien n'est installé).
pub fn mode_opaque(cx: &App) -> TextRenderingMode {
    cx.try_global::<ModeOpaque>()
        .map_or(TextRenderingMode::PlatformDefault, |m| m.0)
}

/// Le genre d'une surface, d'où découle son mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Contenu en flux sur le fond opaque de la fenêtre.
    Opaque,
    /// Calque composé à part : toujours gris.
    Calque,
}

/// Le mode du texte d'une surface de `genre`, quand le mode opaque est `opaque`.
pub fn mode_pour(genre: Genre, opaque: TextRenderingMode) -> TextRenderingMode {
    match genre {
        Genre::Opaque => opaque,
        Genre::Calque => TextRenderingMode::Grayscale,
    }
}

/// Peint `enfant` dans le mode de son genre, puis remet le mode d'avant. Transparent pour la mise
/// en page : la boîte est celle de l'enfant.
pub struct Surface {
    genre: Genre,
    enfant: AnyElement,
}

/// Le contenu en flux d'une fenêtre : son texte prend le mode opaque.
pub fn surface_opaque(enfant: impl IntoElement) -> Surface {
    Surface {
        genre: Genre::Opaque,
        enfant: enfant.into_any_element(),
    }
}

/// Un calque superposé : son texte est gris.
pub fn calque(enfant: impl IntoElement) -> Surface {
    Surface {
        genre: Genre::Calque,
        enfant: enfant.into_any_element(),
    }
}

/// La pile d'une fenêtre : `fond` (en flux, surface opaque) puis `calques`, peints dans l'ordre des
/// `z-index` de Fragment, chacun en calque. Un calque nouveau y est gris sans autre code.
pub fn pile(fond: impl IntoElement, calques: impl IntoIterator<Item = AnyElement>) -> Surface {
    surface_opaque(
        div()
            .size_full()
            .relative()
            .child(fond)
            .children(calques.into_iter().map(calque)),
    )
}

impl IntoElement for Surface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Surface {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.enfant.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.enfant.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui_kit::Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let avant = cx.text_rendering_mode();
        cx.set_text_rendering_mode(mode_pour(self.genre, mode_opaque(cx)));
        self.enfant.paint(window, cx);
        cx.set_text_rendering_mode(avant);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_surface_opaque_prend_le_mode_opaque() {
        assert_eq!(
            mode_pour(Genre::Opaque, TextRenderingMode::Subpixel),
            TextRenderingMode::Subpixel
        );
        assert_eq!(
            mode_pour(Genre::Opaque, TextRenderingMode::PlatformDefault),
            TextRenderingMode::PlatformDefault
        );
    }

    #[test]
    fn un_calque_est_toujours_gris() {
        for opaque in [
            TextRenderingMode::Subpixel,
            TextRenderingMode::PlatformDefault,
            TextRenderingMode::Grayscale,
        ] {
            assert_eq!(
                mode_pour(Genre::Calque, opaque),
                TextRenderingMode::Grayscale
            );
        }
    }
}
