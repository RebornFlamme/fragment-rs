// Port de app/src/ui/core/editor/rangeHighlights.ts (le calque : `layer()`, `RectangleMarker`)
//
//! Le calque des surlignages d'UNE surface : sa table, sa géométrie, son dessin.
//!
//! ★ SOUS LE TEXTE. En TS c'est un `layer()` de CM6 posé sous les lignes ; ici, le calque ne
//!   possède pas d'élément : il offre un CROCHET ([`SousLeTexte`]) que l'élément de la surface
//!   (l'éditeur, une page PDF) appelle AVANT de peindre ses glyphes. Le fork de `gpui-base` n'a
//!   pas à changer : le crochet se pose dans l'élément qui peint déjà, avant le texte.
//! ★ UN calque par surface, long-vivant : on change la table (`poser`) et on « poke » — la surface
//!   vide le poke par [`CalqueSurlignages::livrer`] quand elle peut redessiner, jamais pendant sa
//!   propre mise à jour.
//! ★ Les plages SUIVENT le texte : la surface transmet ses changements à
//!   [`CalqueSurlignages::suivre`].
//! ★ PDF : Fragment fusionne en `multiply` (calque AU-DESSUS d'un canvas opaque). GPUI n'a pas de
//!   mode de fusion : [`ModeCalque::Multiplie`] garde la même teinte, peinte par-dessus la page
//!   sans l'opacifier — le texte noir reste lisible, l'écart au multiply est de l'ordre de
//!   l'opacité de la teinte sur les glyphes. DETTE à régler au montage PDF : produit par canal calculé
//!   côté CPU sur le bitmap de la page (`parite.d/fr-t5-surlignages.toml`).

use std::cell::RefCell;

use fragment_text::{
    EditorChange, HighlightBox, HighlightHandle, HighlightRange, RangeHighlights, TextSurface,
};
use fragment_theme::{Couleur, Theme};
use gpui_kit::{App, Bounds, Corners, Hsla, Pixels, Point, Window, fill, point, px, size};

use super::style::{StyleSurlignage, TRAIT_PASSAGE, style_de};
use crate::echelle::u;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Où se pose le calque par rapport au contenu de la surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ModeCalque {
    /// Sous les glyphes (une note) : la teinte est posée avant le texte.
    #[default]
    SousLeTexte,
    /// Sur une page opaque (PDF) : teinte par-dessus, appelée après le fond de la page.
    Multiplie,
}

/// Une boîte prête à peindre : géométrie du document (px CSS) et style en jetons.
#[derive(Clone, Debug, PartialEq)]
pub struct Rectangle {
    pub boite: HighlightBox,
    pub style: StyleSurlignage,
}

/// Le crochet générique : ce qu'un élément de surface appelle pour peindre ses surlignages.
pub trait SousLeTexte {
    /// `origine` : le coin haut-gauche du repère document de la surface, en pixels de fenêtre ;
    /// `echelle` : l'échelle de la surface sur son plan (1 pour une note).
    fn peindre_sous_le_texte(
        &self,
        surface: &dyn TextSurface,
        origine: Point<Pixels>,
        echelle: f64,
        window: &mut Window,
        cx: &mut App,
    );
}

/// Le calque d'une surface.
#[derive(Default)]
pub struct CalqueSurlignages {
    table: RefCell<RangeHighlights>,
    mode: ModeCalque,
}

impl CalqueSurlignages {
    pub fn new(mode: ModeCalque) -> Self {
        Self {
            table: RefCell::default(),
            mode,
        }
    }

    pub fn mode(&self) -> ModeCalque {
        self.mode
    }

    /// Le calque est-il dessiné au-dessus du contenu ? Seulement en mode `Multiplie`.
    pub fn au_dessus(&self) -> bool {
        self.mode == ModeCalque::Multiplie
    }

    /// Pose (ou remplace) les plages d'une clef ; rend le retrait.
    pub fn poser(&self, clef: &str, plages: Vec<HighlightRange>) -> HighlightHandle {
        self.table.borrow_mut().set(clef, plages)
    }

    pub fn retirer(&self, poignee: &HighlightHandle) {
        self.table.borrow_mut().retire(poignee);
    }

    pub fn supprimer(&self, clef: &str) {
        self.table.borrow_mut().delete(clef);
    }

    /// Les plages courantes d'une clef (remappées).
    pub fn plages(&self, clef: &str) -> Vec<HighlightRange> {
        self.table.borrow().get(clef).to_vec()
    }

    /// Un changement de la surface : les plages le suivent, un poke est armé.
    pub fn suivre(&self, changement: &EditorChange) {
        self.table.borrow_mut().remap(changement);
    }

    /// La surface livre le poke en attente : elle redessine (`request_update`) s'il y en a un.
    /// Rend `true` si un redessin a été demandé.
    pub fn livrer(&self, surface: &dyn TextSurface) -> bool {
        let poke = self.table.borrow_mut().take_redraw();
        if poke {
            surface.request_update();
        }
        poke
    }

    /// Les rectangles à peindre, dans l'ordre de dessin, styles résolus sur `theme`.
    pub fn rectangles(&self, surface: &dyn TextSurface, theme: &Theme) -> Vec<Rectangle> {
        self.table
            .borrow()
            .boxes(surface)
            .into_iter()
            .map(|boite| Rectangle {
                style: style_de(&boite.cls, theme),
                boite,
            })
            .collect()
    }
}

fn teinte(c: Couleur) -> Hsla {
    couleur(c).into()
}

/// Peint un rectangle : fond arrondi, puis le trait du bas d'un passage.
fn peindre_rectangle(
    r: &Rectangle,
    origine: Point<Pixels>,
    echelle: f64,
    window: &mut Window,
    cx: &mut App,
) {
    let zoom = |n: f64| u(cx, (n * echelle) as f32);
    let boite = Bounds::new(
        point(
            origine.x + zoom(r.boite.left),
            origine.y + zoom(r.boite.top),
        ),
        size(zoom(r.boite.width), zoom(r.boite.height)),
    );
    let rayon = u(cx, r.style.rayon);
    window.paint_quad(fill(boite, teinte(r.style.fond)).corner_radii(Corners::all(rayon)));
    if let Some(trait_bas) = r.style.trait_bas {
        let h = u(cx, TRAIT_PASSAGE).min(boite.size.height);
        let bas = Bounds::new(
            point(boite.origin.x, boite.origin.y + boite.size.height - h),
            size(boite.size.width, h),
        );
        let coins = Corners {
            top_left: px(0.0),
            top_right: px(0.0),
            bottom_right: rayon.min(h),
            bottom_left: rayon.min(h),
        };
        window.paint_quad(fill(bas, teinte(trait_bas)).corner_radii(coins));
    }
}

impl SousLeTexte for CalqueSurlignages {
    fn peindre_sous_le_texte(
        &self,
        surface: &dyn TextSurface,
        origine: Point<Pixels>,
        echelle: f64,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = theme_ou_defaut(cx);
        for r in self.rectangles(surface, &theme) {
            peindre_rectangle(&r, origine, echelle, window, cx);
        }
    }
}
