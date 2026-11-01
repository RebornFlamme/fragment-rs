//! Coquille de l'application : la fenêtre « Fragment » et son dock.
//!
//! Phase 0 : un dock VIDE (aucune vue). Les vues, le workspace et les coffres
//! arrivent avec la tranche T1 (voir `program.md` §5, phase 3).

use gpui_kit::component::dock::{DockArea, DockSkin};
use gpui_kit::*;

/// Titre de la fenêtre principale.
pub const TITRE: &str = "Fragment";
/// Taille par défaut de la fenêtre : c'est AUSSI la taille des scènes de pixels
/// (1280 × 800, DPR 1, `program.md` « Parité au PIXEL »).
pub const LARGEUR: f32 = 1280.0;
pub const HAUTEUR: f32 = 800.0;

/// La vue racine : un `DockArea` de gpui-kit habillé par `DockSkin`.
pub struct Coquille {
    dock: Entity<DockArea>,
}

impl Coquille {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock = cx.new(|cx| {
            DockArea::new("fragment-main", Some(1), window, cx).with_renderer(DockSkin::new(cx))
        });
        Self { dock }
    }
}

impl Render for Coquille {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.dock.clone())
    }
}

/// Options de la fenêtre principale (centrée, 1280 × 800, titrée « Fragment »).
pub fn options_fenetre(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(LARGEUR), px(HAUTEUR)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(TITRE.into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Ouvre la fenêtre principale. `gpui_kit::init` doit avoir été appelé.
pub fn ouvrir_fenetre(cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    let (fenetre, _) = gpui_kit::open_window(options_fenetre(cx), cx, |window, cx| {
        cx.new(|cx| Coquille::new(window, cx))
    })?;
    Ok(fenetre)
}
