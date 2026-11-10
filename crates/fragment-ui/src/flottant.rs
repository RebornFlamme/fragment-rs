//! Ce que les surfaces flottantes (notice, menu, infobulle, barre d'outils) ont en commun, et le
//! décor qui les porte dans les scènes de pixels.
//!
//! ★ Ces surfaces vivent dans des magasins `thread_local` (un par module) et se dessinent par des
//! fonctions `couche(cx)` : une commande de scène (`pixels:menu`…) n'a pas de `cx`, elle ne peut donc
//! ni créer une vue ni la notifier. GPUI est mono-fil pour l'interface : un magasin par fil est un
//! singleton sans verrou, et chaque test (un fil) repart d'un magasin vide. Les écritures qui
//! viennent d'un clic ou d'un survol finissent par `cx.refresh_windows()`.
//!
//! [`Decor`] est la vue racine des scènes `"vue": "flottantes"` : un fond de thème et ces couches,
//! rien d'autre (les onglets sont ceux de `fr-t1-onglets`, avec leurs propres scènes).

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::CommandManager;
use fragment_theme::Ombre;
use gpui_kit::{
    AbsoluteLength, App, BoxShadow, Context, FontWeight, IntoElement, ParentElement as _, Pixels,
    Render, Styled, Window, div, point, relative,
};
use serde_json::Value;

use crate::echelle::u;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};
use crate::{menu, notice, toolbar, tooltip};

/// Les ombres portées d'un jeton (`--menu-shadow`, `--shadow-s`…) en ombres GPUI, mises à l'échelle.
pub fn ombres(cx: &App, liste: &[Ombre]) -> Vec<BoxShadow> {
    liste
        .iter()
        .map(|o| BoxShadow {
            color: couleur(o.couleur).into(),
            offset: point(u(cx, o.decalage_x), u(cx, o.decalage_y)),
            blur_radius: u(cx, o.flou),
            spread_radius: u(cx, o.etendue),
            inset: false,
        })
        .collect()
}

/// Une bordure de `largeur` sur les quatre côtés (GPUI n'a que des largeurs fixes : `border_1`
/// ignorerait le zoom).
pub fn bordure<E: Styled>(mut e: E, largeur: Pixels) -> E {
    let style = e.style();
    style.border_widths.top = Some(AbsoluteLength::from(largeur));
    style.border_widths.right = Some(AbsoluteLength::from(largeur));
    style.border_widths.bottom = Some(AbsoluteLength::from(largeur));
    style.border_widths.left = Some(AbsoluteLength::from(largeur));
    e
}

/// La typographie d'une surface : DM Sans, `taille` px (zoom compris), interligne relatif.
pub fn texte<E: Styled>(e: E, cx: &App, taille: f32, interligne: f32) -> E {
    e.font_family(FAMILLE_INTERFACE)
        .font_weight(FontWeight::NORMAL)
        .text_size(u(cx, taille))
        .line_height(relative(interligne))
}

/// Écoute `pixels:args` : le lecteur y publie les arguments juste avant une commande qui en a.
/// Rend la dernière valeur reçue (`null` tant qu'il n'en est venu aucune).
pub fn ecouter_args(commandes: &CommandManager) -> Rc<RefCell<Value>> {
    let derniere = Rc::new(RefCell::new(Value::Null));
    let cible = derniere.clone();
    // L'abonnement dure autant que le manager : on ne garde pas sa référence.
    let _abonnement = commandes.on("pixels:args", move |donnees| {
        if let Some(v) = donnees.downcast_ref::<Value>() {
            *cible.borrow_mut() = v.clone();
        }
    });
    derniere
}

/// Un nombre lu dans les arguments d'une commande de scène.
pub fn nombre(args: &Value, cle: &str) -> Option<f32> {
    args.get(cle).and_then(Value::as_f64).map(|n| n as f32)
}

/// Un texte lu dans les arguments d'une commande de scène.
pub fn chaine<'a>(args: &'a Value, cle: &str) -> Option<&'a str> {
    args.get(cle).and_then(Value::as_str)
}

/// La vue racine des scènes de surfaces flottantes : un fond de thème et les quatre couches.
pub struct Decor;

impl Render for Decor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fond = couleur(theme_ou_defaut(cx).palette.workspace_bg);
        // Ordre de peinture = ordre des z-index de Fragment : barre (900), notices et menu (1000),
        // infobulle (1100).
        div()
            .size_full()
            .relative()
            .bg(fond)
            .child(toolbar::couche(cx))
            .child(notice::couche(cx))
            .child(menu::couche(cx))
            .child(tooltip::couche(cx))
            // Les modales passent devant tout (z-index 1100, au-dessus du menu).
            .child(crate::modal::couche(window, cx))
    }
}
