// Port de app/src/ui/views/agent/trace/orb.ts : la toile de l'orbe, réécrite pour GPUI.
//
// Le canevas 2D et sa boucle `requestAnimationFrame` deviennent une toile GPUI (`canvas`) qui
// peint le plan d'une image et redemande une image tant qu'elle anime ET qu'elle est visible :
// hors du masque de contenu (défilée hors de vue), elle ne peint rien et ne relance rien.

use std::rc::Rc;

use fragment_agent::trace_model::OrbState;
use fragment_theme::Mode;
use gpui_kit::{
    AnyElement, Bounds, IntoElement, PathBuilder, Pixels, Styled as _, canvas, fill, point, px,
    size,
};

use super::{HorlogeOrbes, Marque, T_FIGE, image_etat, plan};
use crate::agent_trace::FournisseurOrbe;
use crate::theme_global::{CouleurGpui, ThemeGlobal};

/// L'orbe est-elle à l'écran ? Ses bornes doivent croiser le masque de contenu de la fenêtre.
#[must_use]
pub fn visible(bornes: Bounds<Pixels>, masque: Bounds<Pixels>) -> bool {
    bornes.intersects(&masque)
}

/// L'encre en octets, sans son opacité (la bibliothèque lit la couleur calculée, opaque).
fn teinte(encre: CouleurGpui) -> [u8; 3] {
    let octet = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    [octet(encre.r), octet(encre.g), octet(encre.b)]
}

fn en_couleur(modele: CouleurGpui, c: [f32; 4]) -> CouleurGpui {
    let mut sortie = modele;
    sortie.r = c[0];
    sortie.g = c[1];
    sortie.b = c[2];
    sortie.a = c[3];
    sortie
}

/// L'élément de l'orbe : `etat` à l'horloge `secondes`, sur un carré de `cote`, à l'encre
/// `encre`. `anime` faux : l'image fixe du mouvement réduit, sans nouvelle image demandée.
#[must_use]
pub fn element_orbe(
    etat: OrbState,
    cote: f32,
    encre: CouleurGpui,
    secondes: f64,
    anime: bool,
) -> AnyElement {
    let t = if anime { secondes } else { T_FIGE };
    let image = image_etat(etat, cote, t);
    let teinte = teinte(encre);
    canvas(
        |_, _, _| (),
        move |bornes, (), fenetre, cx| {
            if !visible(bornes, fenetre.content_mask().bounds) {
                return;
            }
            // Le thème de l'APP (comme `body.theme-dark`), sombre par défaut s'il n'est pas posé.
            let sombre = cx
                .try_global::<ThemeGlobal>()
                .is_none_or(|t| t.0.mode == Mode::Sombre);
            let o = bornes.origin;
            for m in plan(&image, sombre, Some(teinte)) {
                match m {
                    Marque::Trait {
                        x1,
                        y1,
                        x2,
                        y2,
                        epaisseur,
                        couleur,
                    } => {
                        let mut chemin = PathBuilder::stroke(px(epaisseur));
                        chemin.move_to(point(o.x + px(x1), o.y + px(y1)));
                        chemin.line_to(point(o.x + px(x2), o.y + px(y2)));
                        if let Ok(c) = chemin.build() {
                            fenetre.paint_path(c, en_couleur(encre, couleur));
                        }
                    }
                    Marque::Disque { x, y, r, couleur } => {
                        let carre = Bounds::new(
                            point(o.x + px(x - r), o.y + px(y - r)),
                            size(px(2.0 * r), px(2.0 * r)),
                        );
                        fenetre.paint_quad(
                            fill(carre, en_couleur(encre, couleur)).corner_radii(px(r)),
                        );
                    }
                }
            }
            if anime {
                fenetre.request_animation_frame();
            }
        },
    )
    .size(px(cote))
    .into_any_element()
}

/// Le fournisseur d'orbes de la trace : une horloge partagée par toutes les orbes de l'app et
/// le réglage de mouvement réduit.
#[derive(Debug, Clone)]
pub struct Orbes {
    horloge: Rc<HorlogeOrbes>,
    reduit: bool,
}

impl Orbes {
    /// `reduit` : `prefers-reduced-motion` (une image fixe).
    #[must_use]
    pub fn new(reduit: bool) -> Self {
        Self {
            horloge: Rc::new(HorlogeOrbes::new()),
            reduit,
        }
    }

    /// Une orbe de plus sur la même horloge (une autre vue, la page des réglages).
    #[must_use]
    pub fn horloge(&self) -> Rc<HorlogeOrbes> {
        self.horloge.clone()
    }
}

impl FournisseurOrbe for Orbes {
    fn orbe(&self, etat: OrbState, cote: f32, encre: CouleurGpui, maintenant: i64) -> AnyElement {
        element_orbe(
            etat,
            cote,
            encre,
            self.horloge.secondes(maintenant),
            !self.reduit,
        )
    }
}
