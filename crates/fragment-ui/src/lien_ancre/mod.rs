//! Lien pointillé d'ancre (soulignement vers widget) (tâche `fr-t5-lien-ancre`).
//!
//! Au survol d'un widget ancré à un passage, le lien CONTINUE le soulignement du passage, file à
//! l'horizontale, descend en oblique et entre à l'horizontale dans le widget, au milieu de sa rangée
//! d'icônes ; un petit cercle marque son bout.
//!
//! - [`modele`] : bouts du lien (depuis une [`SurfaceLien`]), règle « un lien ou non », registre
//!   [`Liens`] par surface (un lien par membre d'une grappe survolée, suivi quand le widget bouge) ;
//! - [`dessin`] : pointillés calculés et peinture GPUI en `color_accent_2` ;
//! - la commande de scène `pixels:lien-ancre` (`{from:{x,y}, to:{x,y}}`, en pixels logiques).
//!
//! Le branchement au survol réel est au montage (`fr-t5-montage-note`).

pub mod dessin;
pub mod modele;

use std::cell::RefCell;

use fragment_annotation::anchor_link::{LinkEnds, Point, trace};
use fragment_core::{Command, CommandManager};
use gpui_kit::{
    AnyElement, App, Bounds, IntoElement, ParentElement as _, Pixels, Styled as _, Window, canvas,
    div, px,
};
use serde_json::Value;

use crate::flottant::ecouter_args;
use crate::theme_global::theme_ou_defaut;

pub use modele::{
    Ancre, CibleLien, EtatLien, Lien, Liens, Marqueur, Rect, SurfaceLien, bouts_du_lien, lien_voulu,
};

thread_local! {
    static SCENE: RefCell<Option<LinkEnds>> = const { RefCell::new(None) };
}

fn point_de(a: &Value, cle: &str) -> Option<Point> {
    let p = a.get(cle)?;
    Some(Point {
        x: p.get("x")?.as_f64()?,
        y: p.get("y")?.as_f64()?,
    })
}

/// Les bouts d'une scène : le coude et l'entrée sont ceux de l'app (40 et 12 px avant le widget).
pub fn bouts_de_scene(a: &Value) -> Option<LinkEnds> {
    let from = point_de(a, "from")?;
    let to = point_de(a, "to")?;
    let sens = if to.x >= from.x { -1.0 } else { 1.0 };
    Some(LinkEnds {
        from,
        to,
        elbow_x: to.x + sens * modele::COUDE,
        entry_x: to.x + sens * modele::ENTREE,
    })
}

/// Enregistre `pixels:lien-ancre` (`{from:{x,y}, to:{x,y}}`).
pub fn commandes_de_scene(commandes: &CommandManager) {
    SCENE.with(|s| *s.borrow_mut() = None);
    let args = ecouter_args(commandes);
    let commande = Command::new("pixels:lien-ancre", "Pixels : poser un lien d'ancre")
        .with_callback(move || {
            let bouts = bouts_de_scene(&args.borrow());
            SCENE.with(|s| *s.borrow_mut() = bouts);
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// La scène courante, ou rien tant qu'aucune commande n'est venue : un calque qui couvre son parent.
pub fn couche(cx: &App) -> Option<AnyElement> {
    let bouts = SCENE.with(|s| *s.borrow())?;
    let teinte = theme_ou_defaut(cx).palette.color_accent_2;
    let pts = trace(&bouts);
    Some(
        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full()
            .child(
                canvas(
                    |_, _, _| {},
                    move |bornes: Bounds<Pixels>, (), window: &mut Window, _cx: &mut App| {
                        dessin::peindre(window, bornes.origin, &pts, teinte, 1.0);
                    },
                )
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full(),
            )
            .into_any_element(),
    )
}
