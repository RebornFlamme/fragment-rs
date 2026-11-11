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
    AbsoluteLength, App, Bounds, BoxShadow, Context, FontWeight, IntoElement, ParentElement,
    Pixels, Render, Styled, Window, canvas, div, point, relative,
};
use serde_json::Value;

use crate::echelle::u;
use crate::polices::famille_interface;
use crate::theme_global::{couleur, theme_ou_defaut};
use crate::{menu, notice, toolbar, tooltip};

/// Les ombres portées d'un jeton (`--menu-shadow`, `--shadow-s`…) en ombres GPUI, mises à l'échelle.
///
/// ★ Le « flou » CSS est le DIAMÈTRE de l'étalement : Skia en tire un écart-type de `flou / 2`. Le shader
/// d'ombre de GPUI (`gaussian(y, blur_radius)`) prend `blur_radius` tel quel comme écart-type : sans la
/// division, toute ombre est deux fois trop étalée, donc trop pâle. Décalage et étendue ont le même sens
/// des deux côtés.
pub fn ombres(cx: &App, liste: &[Ombre]) -> Vec<BoxShadow> {
    liste
        .iter()
        .map(|o| BoxShadow {
            color: couleur(o.couleur).into(),
            offset: point(u(cx, o.decalage_x), u(cx, o.decalage_y)),
            blur_radius: u(cx, o.flou / 2.0),
            spread_radius: u(cx, o.etendue),
            inset: false,
        })
        .collect()
}

/// Une couche d'ombre telle que GPUI la peint, en pixels PHYSIQUES (zoom et facteur d'échelle
/// compris) : décalage, écart-type du flou (`blur_radius` de GPUI) et étendue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoucheOmbre {
    pub decalage_x: f32,
    pub decalage_y: f32,
    pub sigma: f32,
    pub etendue: f32,
}

/// Une surface ombrée telle que la mise en page l'a posée, en pixels physiques : sa boîte de
/// bordure, son rayon de coin et ses couches d'ombre (ADR-009, catégorie ombre). C'est la
/// DÉCLARATION du candidat : le halo s'en calcule, jamais de l'image.
#[derive(Debug, Clone, PartialEq)]
pub struct OmbrePeinte {
    pub x: f32,
    pub y: f32,
    pub largeur: f32,
    pub hauteur: f32,
    pub rayon: f32,
    pub couches: Vec<CoucheOmbre>,
}

thread_local! {
    /// Les surfaces ombrées des images peintes depuis le dernier [`vider_ombres`].
    static OMBRES_PEINTES: RefCell<Vec<OmbrePeinte>> = const { RefCell::new(Vec::new()) };
}

/// Oublie les ombres déclarées : à appeler juste avant de peindre l'image à capturer.
pub fn vider_ombres() {
    OMBRES_PEINTES.with(|o| o.borrow_mut().clear());
}

/// Les ombres déclarées depuis le dernier [`vider_ombres`], dans l'ordre de peinture.
pub fn ombres_peintes() -> Vec<OmbrePeinte> {
    OMBRES_PEINTES.with(|o| o.borrow().clone())
}

/// Pose sur `e` les ombres du jeton `liste` ET les déclare (registre lu par `fragment-pixels`) :
/// une seule source pour ce qui est peint et ce qui est déclaré. `rayon` est celui de `.rounded`,
/// `bordure` la largeur de bordure de `e`.
///
/// ★ Un enfant `absolute` + `size_full` reçoit la boîte de REMBOURRAGE du parent (taffy retire la
/// bordure de la zone des enfants positionnés) : la déclaration la regonfle de `bordure`.
pub fn ombrer<E: Styled + ParentElement>(
    e: E,
    cx: &App,
    liste: &[Ombre],
    rayon: Pixels,
    bordure: Pixels,
) -> E {
    let peintes = ombres(cx, liste);
    let declarees = peintes.clone();
    e.shadow(peintes).child(
        canvas(
            move |bounds: Bounds<Pixels>, window: &mut Window, _: &mut App| {
                let k = window.scale_factor();
                let b = f32::from(bordure);
                let ombre = OmbrePeinte {
                    x: (f32::from(bounds.origin.x) - b) * k,
                    y: (f32::from(bounds.origin.y) - b) * k,
                    largeur: (f32::from(bounds.size.width) + 2.0 * b) * k,
                    hauteur: (f32::from(bounds.size.height) + 2.0 * b) * k,
                    rayon: f32::from(rayon) * k,
                    couches: declarees
                        .iter()
                        .map(|o| CoucheOmbre {
                            decalage_x: f32::from(o.offset.x) * k,
                            decalage_y: f32::from(o.offset.y) * k,
                            sigma: f32::from(o.blur_radius) * k,
                            etendue: f32::from(o.spread_radius) * k,
                        })
                        .collect(),
                };
                OMBRES_PEINTES.with(|o| o.borrow_mut().push(ombre));
            },
            |_, (), _, _| {},
        )
        .absolute()
        .size_full(),
    )
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
    famille_interface(e)
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
