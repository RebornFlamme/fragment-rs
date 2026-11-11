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
use fragment_theme::{Couleur, Ombre, Palette};
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
    /// Le jeton CSS de l'ombre (`--menu-shadow`, `--shadow-l`…) : seul `--shadow-l` porte une
    /// exception nommée (ADR-009, amendement de 11:15, point b).
    pub jeton: &'static str,
}

/// Une boîte arrondie telle que la mise en page l'a posée, en pixels physiques : sa boîte de
/// bordure, son rayon de coin et sa largeur de bordure (ADR-009, catégorie bord d'arrondi). C'est la
/// DÉCLARATION du candidat : l'arc théorique s'en calcule, jamais de l'image.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrondiPeint {
    pub x: f32,
    pub y: f32,
    pub largeur: f32,
    pub hauteur: f32,
    pub rayon: f32,
    pub bordure: f32,
}

thread_local! {
    /// Les surfaces ombrées des images peintes depuis le dernier [`vider_ombres`].
    static OMBRES_PEINTES: RefCell<Vec<OmbrePeinte>> = const { RefCell::new(Vec::new()) };
    /// Les boîtes arrondies des images peintes depuis le dernier [`vider_ombres`].
    static ARRONDIS_PEINTS: RefCell<Vec<ArrondiPeint>> = const { RefCell::new(Vec::new()) };
}

/// Oublie les ombres et les arrondis déclarés : à appeler juste avant de peindre l'image à capturer.
pub fn vider_ombres() {
    OMBRES_PEINTES.with(|o| o.borrow_mut().clear());
    ARRONDIS_PEINTS.with(|a| a.borrow_mut().clear());
}

/// Les ombres déclarées depuis le dernier [`vider_ombres`], dans l'ordre de peinture.
pub fn ombres_peintes() -> Vec<OmbrePeinte> {
    OMBRES_PEINTES.with(|o| o.borrow().clone())
}

/// Les boîtes arrondies déclarées depuis le dernier [`vider_ombres`], dans l'ordre de peinture.
pub fn arrondis_peints() -> Vec<ArrondiPeint> {
    ARRONDIS_PEINTS.with(|a| a.borrow().clone())
}

/// Un enfant invisible qui, au prépeint, lit la boîte de bordure de son parent (`bounds` est la
/// boîte de rembourrage : on la regonfle de `bordure`) et la passe à `declarer`, en pixels physiques
/// (x, y, largeur, hauteur) avec le facteur d'échelle.
fn declaration(
    cx: &App,
    bordure: Pixels,
    declarer: impl Fn([f32; 4], f32) + 'static,
) -> impl IntoElement {
    canvas(
        move |bounds: Bounds<Pixels>, window: &mut Window, _: &mut App| {
            let k = window.scale_factor();
            let b = f32::from(bordure);
            declarer(
                [
                    (f32::from(bounds.origin.x) - b) * k,
                    (f32::from(bounds.origin.y) - b) * k,
                    (f32::from(bounds.size.width) + 2.0 * b) * k,
                    (f32::from(bounds.size.height) + 2.0 * b) * k,
                ],
                k,
            );
        },
        |_, (), _, _| {},
    )
    .absolute()
    .top(u(cx, 0.0))
    .left(u(cx, 0.0))
    .size_full()
}

/// Arrondit `e` de `rayon` ET déclare la boîte arrondie (registre lu par `fragment-pixels`, catégorie
/// bord d'arrondi d'ADR-009) : une seule source pour ce qui est peint et ce qui est déclaré.
/// `bordure` est la largeur de bordure de `e` (0 sans bordure).
pub fn arrondir<E: Styled + ParentElement>(e: E, cx: &App, rayon: Pixels, bordure: Pixels) -> E {
    e.rounded(rayon)
        .child(declaration(cx, bordure, move |[x, y, l, h], k| {
            let arrondi = ArrondiPeint {
                x,
                y,
                largeur: l,
                hauteur: h,
                rayon: f32::from(rayon) * k,
                bordure: f32::from(bordure) * k,
            };
            ARRONDIS_PEINTS.with(|a| a.borrow_mut().push(arrondi));
        }))
}

/// Pose sur `e` les ombres du jeton `liste` (de nom CSS `jeton`), l'arrondi `rayon`, ET les déclare
/// (registre lu par `fragment-pixels`) : une seule source pour ce qui est peint et ce qui est
/// déclaré. `bordure` est la largeur de bordure de `e`.
///
/// ★ Un enfant `absolute` + `size_full` a la TAILLE de la boîte de rembourrage du parent (taffy retire
/// la bordure de la zone des enfants positionnés), mais sans `top`/`left` il garde sa position
/// STATIQUE, au coin de la boîte de contenu (constaté en CI : menu déclaré 4 px trop bas et trop à
/// droite, son rembourrage). D'où `top(0)`/`left(0)` ; la déclaration regonfle ensuite de `bordure`.
pub fn ombrer<E: Styled + ParentElement>(
    e: E,
    cx: &App,
    liste: &[Ombre],
    jeton: &'static str,
    rayon: Pixels,
    bordure: Pixels,
) -> E {
    let peintes = ombres(cx, liste);
    let declarees = peintes.clone();
    let e = arrondir(e.shadow(peintes), cx, rayon, bordure);
    e.child(declaration(cx, bordure, move |[x, y, l, h], k| {
        let ombre = OmbrePeinte {
            x,
            y,
            largeur: l,
            hauteur: h,
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
            jeton,
        };
        OMBRES_PEINTES.with(|o| o.borrow_mut().push(ombre));
    }))
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

/// La couleur de `--background-modifier-border` d'une surface de fond opaque `fond`, à passer à
/// `border_color`.
///
/// ★ Elle reste TRANSLUCIDE : pré-composée en opaque (`Palette::aplat`), l'anticrénelage des arcs
/// extérieurs s'éloignait de Chromium de 2 à 4 niveaux (notice sombre, run 43018474495), car
/// Chromium y pose un trait translucide sur un fond déjà anticrénelé. Sur les côtés droits,
/// Chromium mélange la bordure à l'opacité EXACTE du jeton (`fond + p × (teinte − fond)`,
/// arrondi au plus proche : 232,5 → 233 sur le blanc du menu clair) ; GPUI la mélange à l'octet
/// d'opacité (26/255 pour 10 %) et tombe un niveau à côté (232). On garde donc l'octet d'opacité
/// et on déplace, canal par canal et seulement si besoin, la teinte vers celle dont le mélange de
/// GPUI tombe au plus près de la valeur de Chromium. À retirer par `fr-p4.1` (mélange alpha).
pub fn bordure_translucide(c: &Palette, fond: Couleur) -> Couleur {
    let teinte = c.background_modifier_border;
    let p = f64::from(c.opacites.bordure) / 100.0;
    let a = f64::from(teinte.a) / 255.0;
    if a <= 0.0 {
        return teinte;
    }
    let canal = |t: u8, f: u8| -> u8 {
        let f = f64::from(f);
        let cible = (f + p * (f64::from(t) - f) + 0.5).floor();
        let gpui = |x: u8| f + a * (f64::from(x) - f);
        if (gpui(t) + 0.5).floor() == cible {
            return t;
        }
        // La teinte la plus proche de `t` parmi celles dont le mélange tombe au plus près.
        (0..=255u8)
            .min_by(|x, y| {
                let ex = (gpui(*x) - cible).abs();
                let ey = (gpui(*y) - cible).abs();
                ex.total_cmp(&ey).then(
                    (i16::from(*x) - i16::from(t))
                        .abs()
                        .cmp(&(i16::from(*y) - i16::from(t)).abs()),
                )
            })
            .unwrap_or(t)
    };
    Couleur::octets(
        canal(teinte.r, fond.r),
        canal(teinte.g, fond.g),
        canal(teinte.b, fond.b),
        teinte.a,
    )
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

#[cfg(test)]
mod tests_bordure {
    use super::*;
    use fragment_theme::Theme;

    /// Le mélange de GPUI : la teinte à son octet d'opacité sur le fond, arrondi au plus proche.
    fn gpui(c: Couleur, fond: Couleur) -> [u8; 3] {
        let a = f64::from(c.a) / 255.0;
        let m = |t: u8, f: u8| {
            let f = f64::from(f);
            (f + a * (f64::from(t) - f) + 0.5).floor() as u8
        };
        [m(c.r, fond.r), m(c.g, fond.g), m(c.b, fond.b)]
    }

    /// Valeurs relevées sur les captures de Fragment (run 43018474495) : côtés droits des bordures
    /// du menu (clair, sombre) et de la notice sombre.
    #[test]
    fn bordure_translucide_tombe_sur_les_cotes_de_chromium() {
        let clair = Theme::clair().palette;
        let sombre = Theme::sombre().palette;
        let menu_clair = bordure_translucide(&clair, clair.menu_bg);
        assert_eq!(gpui(menu_clair, clair.menu_bg), [232, 232, 233]);
        let menu_sombre = bordure_translucide(&sombre, sombre.menu_bg);
        assert_eq!(gpui(menu_sombre, sombre.menu_bg), [46, 46, 50]);
        let notice = bordure_translucide(&sombre, sombre.background_secondary_alt);
        assert_eq!(gpui(notice, sombre.background_secondary_alt), [51, 52, 56]);
        // La teinte ne bouge que là où il le faut, et l'opacité jamais.
        assert_eq!(menu_sombre, sombre.background_modifier_border);
        assert_eq!(menu_clair.a, clair.background_modifier_border.a);
    }
}
