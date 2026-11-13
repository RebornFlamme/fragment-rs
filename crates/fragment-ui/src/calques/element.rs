// Port de app/src/ui/core/layers/ViewOverlays.ts (les racines DOM : `ensureViewport`, `ensureDocument`,
// `ensureGutter`, `makeRoot`, et la mesure au ResizeObserver / au défilement)
//
//! Le dessin des plans d'une vue : UN élément posé en dernier enfant du pane, qui le couvre.
//!
//! ★ LIRE PUIS ÉCRIRE DANS LA MÊME IMAGE. GPUI offre le point voulu entre la mise en page et la
//!   peinture : le `prepaint` d'un `canvas`. Là, la surface de la vue (enfant précédent) est déjà
//!   mise en page, donc `coords_at_pos` dit vrai. La passe :
//!     1. mesurer les plans (le pane = nos bornes ; l'origine, la colonne et l'échelle = la vue) ;
//!     2. mettre en page CHAQUE élément seul (`layout_as_root`) et noter sa boîte ;
//!     3. faire tourner les modèles (`HoteCalques::passe` : géométrie, viewport, actif,
//!        arbitre) — ils lisent les boîtes et écrivent les côtés ;
//!     4. refaire 2–3 pour ce qui a changé de largeur imposée ou de taille (au plus trois tours) ;
//!     5. poser chaque élément à son coin (`prepaint_at`), puis le peindre.
//!   Rien n'attend l'image suivante, sauf ce qu'un rappel planifie lui-même.
//!
//! ★ Pas de `transform` dans GPUI : le plan document n'est pas mis à l'échelle comme sous-arbre.
//!   L'hôte place chaque élément par `position_dans_le_pane` (unités du plan × échelle pour le plan
//!   document, px écran pour la marge) ; un élément du plan document dessine donc sa taille écran.
//!
//! ★ `pass_through` : sans lui, l'élément BLOQUE la souris (`occlude`) — un clic sur un widget ne
//!   traverse pas jusqu'au texte. Avec lui, l'enveloppe n'a aucune zone de clic : seuls ses
//!   enfants qui captent eux-mêmes reçoivent le pointeur.
//!
//! Unités : les modèles parlent en px CSS (le repère de Fragment) ; GPUI en px logiques, zoom de
//! l'application compris. La conversion passe par [`crate::echelle`].

use std::rc::Rc;

use fragment_layers::types::{ElementId, Point};
use fragment_layers::view_overlays::MesuresPlans;
use fragment_text::Rect;
use gpui_kit::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Styled as _, Window, canvas, div, point, size,
};

use super::hote::HoteCalques;
use crate::echelle::{Echelle, u};

/// Ce que la VUE dit de ses plans, relu à chaque image (jamais copié : le zoom d'un PDF
/// change sans prévenir). Coordonnées CLIENT en px CSS (la fenêtre, zoom de l'app retiré).
pub trait SourcePlans {
    /// L'origine du repère `coords_at_pos` (`None` : pas de plan document).
    fn origine(&self) -> Option<Point>;
    /// La colonne de lecture, dont les marges sont les marges (`None` : pas de colonne).
    fn colonne(&self) -> Option<Rect>;
    /// L'échelle de la vue sur son plan document (défaut : 1).
    fn echelle(&self) -> f64 {
        1.0
    }
}

/// Le nombre de tours de mesure dans une image (au-delà, l'image suivante reprend).
const TOURS: usize = 3;

/// Un élément construit pour cette image, avec la largeur imposée qui l'a dimensionné.
struct Construit {
    el: ElementId,
    largeur: Option<f64>,
    element: AnyElement,
}

fn vers_css(p: Pixels, zoom: f64) -> f64 {
    f64::from(f32::from(p)) / zoom
}

/// Les plans des calques d'une vue : à poser en DERNIER enfant de son pane (au-dessus de la
/// surface), en absolu, couvrant le pane.
pub fn plans_calques(hote: Rc<HoteCalques>, source: Rc<dyn SourcePlans>) -> impl IntoElement {
    canvas(
        move |bornes, window, cx| preparer(&hote, &*source, bornes, window, cx),
        move |bornes, mut construits: Vec<AnyElement>, window, cx| {
            let masque = ContentMask { bounds: bornes };
            window.with_content_mask(Some(masque), |window| {
                for e in &mut construits {
                    e.paint(window, cx);
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// L'enveloppe d'un élément : sa largeur imposée, et la souris bloquée sauf `pass_through`.
fn envelopper(
    contenu: AnyElement,
    pass_through: bool,
    largeur: Option<f64>,
    cx: &App,
) -> AnyElement {
    let mut d = div().flex_none().child(contenu);
    if let Some(w) = largeur {
        d = d.w(u(cx, w as f32));
    }
    if pass_through {
        d.into_any_element()
    } else {
        d.occlude().into_any_element()
    }
}

fn preparer(
    hote: &HoteCalques,
    source: &dyn SourcePlans,
    bornes: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    let zoom = f64::from(Echelle::courante(cx).zoom());
    let css = |p: Pixels| vers_css(p, zoom);
    let pane = Rect {
        left: css(bornes.left()),
        top: css(bornes.top()),
        right: css(bornes.right()),
        bottom: css(bornes.bottom()),
    };
    let mesures = MesuresPlans {
        pane,
        origine: source.origine(),
        colonne: source.colonne(),
        echelle: source.echelle(),
    };

    let masque = ContentMask { bounds: bornes };
    window.with_content_mask(Some(masque), |window| {
        let mut construits: Vec<Construit> = Vec::new();
        for tour in 0..TOURS {
            let mut change = false;
            for (el, contenu, pass, largeur) in hote.a_dessiner() {
                let deja = construits.iter().position(|c| c.el == el);
                if deja.is_some_and(|i| construits[i].largeur == largeur) {
                    continue;
                }
                let mut element = envelopper(contenu(window, cx), pass, largeur, cx);
                let espace = size(AvailableSpace::MaxContent, AvailableSpace::MaxContent);
                let t = element.layout_as_root(espace, window, cx);
                hote.poser_taille(el, (css(t.width), css(t.height)));
                let c = Construit {
                    el,
                    largeur,
                    element,
                };
                match deja {
                    Some(i) => construits[i] = c,
                    None => construits.push(c),
                }
                change = true;
            }
            // Le premier tour passe toujours : la géométrie et le viewport se mesurent même
            // sans élément.
            if tour > 0 && !change {
                break;
            }
            hote.passe(mesures);
        }
        // Une passe a encore changé une largeur ou monté un élément : l'image suivante reprend.
        let en_retard = hote.a_dessiner().iter().any(|(el, _, _, largeur)| {
            !construits
                .iter()
                .any(|c| c.el == *el && c.largeur == *largeur)
        });
        if en_retard {
            window.request_animation_frame();
        }

        // Ce qui n'est plus à dessiner (masqué, démonté pendant la passe) est jeté ; le reste,
        // dans l'ordre des plans.
        let ordre: Vec<ElementId> = hote.a_dessiner().into_iter().map(|(el, ..)| el).collect();
        let mut finaux = Vec::new();
        for el in ordre {
            let Some(i) = construits.iter().position(|c| c.el == el) else {
                continue;
            };
            let mut c = construits.swap_remove(i);
            let dessinee = hote.fiche(el).is_some_and(|f| f.dessinee());
            let Some(coin) = hote.coin_dans_le_pane(el).filter(|_| dessinee) else {
                continue;
            };
            let origine = bornes.origin + point(u(cx, coin.x as f32), u(cx, coin.y as f32));
            c.element.prepaint_at(origine, window, cx);
            finaux.push(c.element);
        }
        hote.fin_de_passe();
        finaux
    })
}
