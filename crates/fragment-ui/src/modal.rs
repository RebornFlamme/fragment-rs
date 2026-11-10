//! Les modales : une couche au-dessus de tout, avec sa portée clavier.
//!
//! Une modale est une boîte (la vue `Modale`, ou une modale de suggestion : voir `suggest`)
//! posée sur un voile. Ce module fournit les trois pièces communes :
//! - [`Fermeture`] : le drapeau partagé « fermer la modale », levé par Échap, le voile, le bouton ;
//! - [`PorteeModale`] : la portée du `Keymap` empilée tant que la modale est ouverte (Échap ferme et
//!   est consommé, les raccourcis globaux non reconnus remontent à la racine) ;
//! - [`CoucheModale`] : la vue que la vue racine pose en dernier enfant ; elle ouvre et ferme.
//!
//! ★ POURQUOI UNE FILE DE DEMANDES. Une commande du cœur est un `Fn()` sans contexte GPUI : elle ne
//! peut ni créer une vue ni donner le focus. Elle dépose donc une [`Demande`] dans une file propre au
//! fil principal ([`ouvrir`]) ; la couche la vide à chaque image et construit la vue avec le `Window`
//! qu'elle a en main. L'appelant redessine ensuite (`refresh_windows`, ce que fait déjà le lecteur
//! de scènes après une commande).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{CommandManager, Keymap, Scope};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, FocusHandle, Global, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Pixels, Render, SharedString,
    Styled as _, Window, div, relative, svg,
};

use crate::echelle::u;
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

// ── Le drapeau de fermeture ───────────────────────────────────────────────────

/// « Cette modale doit se fermer » : un drapeau partagé entre la couche, la boîte et la portée.
#[derive(Clone, Default)]
pub struct Fermeture(Rc<Cell<bool>>);

impl Fermeture {
    /// Demande la fermeture.
    pub fn demander(&self) {
        self.0.set(true);
    }

    /// Une fermeture a-t-elle été demandée ?
    pub fn demandee(&self) -> bool {
        self.0.get()
    }
}

// ── La portée clavier ─────────────────────────────────────────────────────────

/// La portée d'une modale, empilée dans le `Keymap` tant qu'elle est ouverte.
///
/// Chaînée à la racine (`Scope::pour_vue`) : Échap est consommé ici et ferme ; tout ce que la
/// modale ne reconnaît pas remonte aux raccourcis globaux, qui ne sont donc pas gelés.
pub struct PorteeModale {
    keymap: Keymap,
    scope: Scope,
}

impl PorteeModale {
    /// Crée la portée, y lie Échap à `fermeture` et l'empile.
    pub fn ouvrir(keymap: &Keymap, fermeture: &Fermeture) -> Self {
        let scope = Scope::pour_vue(keymap.root_scope());
        let f = fermeture.clone();
        scope.register(None, Some("Escape"), move |_, _| {
            f.demander();
            // Consommé : Échap ne doit pas atteindre l'éditeur dessous.
            Some(false)
        });
        keymap.push_scope(&scope);
        Self {
            keymap: keymap.clone(),
            scope,
        }
    }

    /// La portée elle-même (pour les tests et l'introspection).
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Dépile la portée : le démontage est symétrique de l'ouverture.
    pub fn fermer(self) {
        self.keymap.pop_scope(&self.scope);
    }
}

/// Le `Keymap` de l'application, installé par l'assemblage ; sans lui, pas de portée empilée.
struct ClavierModal(Keymap);

impl Global for ClavierModal {}

/// Branche le `Keymap` de l'application : les modales ouvertes ensuite empilent leur portée.
pub fn brancher_clavier(cx: &mut App, keymap: Keymap) {
    cx.set_global(ClavierModal(keymap));
}

// ── Les demandes d'ouverture ──────────────────────────────────────────────────

/// Ce que construit une demande : la vue de la boîte et l'élément qui prend le focus.
pub struct Ouverture {
    /// La boîte (`.modal` ou `.prompt`).
    pub vue: AnyView,
    /// Le focus donné à l'ouverture (le champ, ou la boîte).
    pub focus: FocusHandle,
}

type Constructeur = Box<dyn FnOnce(&mut Window, &mut App, Fermeture) -> Ouverture>;

/// Une demande d'ouverture de modale.
pub struct Demande {
    /// Identité de la modale : une demande de même clé que la modale ouverte est ignorée
    /// (rouvrir la palette quand elle est ouverte ne fait rien).
    pub cle: Option<&'static str>,
    construire: Constructeur,
}

impl Demande {
    /// Une demande ; `construire` reçoit le `Window` de la couche et le drapeau de fermeture.
    pub fn new(
        cle: Option<&'static str>,
        construire: impl FnOnce(&mut Window, &mut App, Fermeture) -> Ouverture + 'static,
    ) -> Self {
        Self {
            cle,
            construire: Box::new(construire),
        }
    }
}

thread_local! {
    static EN_ATTENTE: RefCell<Vec<Demande>> = const { RefCell::new(Vec::new()) };
}

/// Dépose une demande ; la couche l'ouvre à sa prochaine image.
pub fn ouvrir(demande: Demande) {
    EN_ATTENTE.with(|f| f.borrow_mut().push(demande));
}

fn prendre_demandes() -> Vec<Demande> {
    EN_ATTENTE.with(|f| std::mem::take(&mut *f.borrow_mut()))
}

/// Une demande de clé `demandee` doit-elle ouvrir quelque chose quand `ouverte` l'est déjà ?
pub fn doit_ouvrir(ouverte: Option<&'static str>, demandee: Option<&'static str>) -> bool {
    match (ouverte, demandee) {
        (Some(a), Some(b)) => a != b,
        _ => true,
    }
}

// ── La couche ─────────────────────────────────────────────────────────────────

struct Ouverte {
    vue: AnyView,
    cle: Option<&'static str>,
    fermeture: Fermeture,
    portee: Option<PorteeModale>,
}

/// La vue que la vue racine pose en DERNIER enfant : voile et boîte quand une modale est ouverte,
/// rien (un `div` hors flux) sinon.
#[derive(Default)]
pub struct CoucheModale {
    ouverte: Option<Ouverte>,
}

impl CoucheModale {
    /// Une couche vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Une modale est-elle ouverte ?
    pub fn est_ouverte(&self) -> bool {
        self.ouverte.is_some()
    }

    fn fermer(&mut self) {
        if let Some(o) = self.ouverte.take()
            && let Some(p) = o.portee
        {
            p.fermer();
        }
    }

    /// Applique la fermeture demandée, puis les demandes en attente.
    fn synchroniser(&mut self, window: &mut Window, cx: &mut App) {
        if self
            .ouverte
            .as_ref()
            .is_some_and(|o| o.fermeture.demandee())
        {
            self.fermer();
        }
        for demande in prendre_demandes() {
            let cle_ouverte = self.ouverte.as_ref().and_then(|o| o.cle);
            if self.ouverte.is_some() && !doit_ouvrir(cle_ouverte, demande.cle) {
                continue;
            }
            self.fermer();
            let fermeture = Fermeture::default();
            let ouverture = (demande.construire)(window, cx, fermeture.clone());
            window.focus(&ouverture.focus, cx);
            let portee = cx
                .try_global::<ClavierModal>()
                .map(|k| PorteeModale::ouvrir(&k.0, &fermeture));
            self.ouverte = Some(Ouverte {
                vue: ouverture.vue,
                cle: demande.cle,
                fermeture,
                portee,
            });
        }
    }
}

impl Render for CoucheModale {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.synchroniser(window, cx);
        let Some(ouverte) = &self.ouverte else {
            return div().absolute().into_any_element();
        };
        let theme = theme_ou_defaut(cx);
        let fermeture = ouverte.fermeture.clone();
        // padding-top: 12vh : la palette s'ouvre dans le tiers haut.
        let haut = window.viewport_size().height * 0.12;
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .pt(haut)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(couleur(theme.palette.background_modifier_cover))
                    .on_mouse_down(MouseButton::Left, move |_, window, _| {
                        fermeture.demander();
                        window.refresh();
                    }),
            )
            .child(ouverte.vue.clone())
            .into_any_element()
    }
}

// ── Mesures communes aux boîtes ───────────────────────────────────────────────

/// `min(max_css, 100vw - 32px)`, en pixels de l'écran (le zoom multiplie les longueurs CSS).
pub(crate) fn largeur_boite(window: &Window, cx: &App, max_css: f32) -> Pixels {
    let plafond = window.viewport_size().width - u(cx, 32.0);
    let voulue = u(cx, max_css);
    if voulue < plafond { voulue } else { plafond }
}

// ── La modale générique ───────────────────────────────────────────────────────

/// `.modal` : titre, bouton de fermeture, contenu.
pub struct Modale {
    titre: SharedString,
    contenu: SharedString,
    focus: FocusHandle,
    fermeture: Fermeture,
}

impl Modale {
    /// Demande l'ouverture d'une modale générique.
    pub fn ouvrir(titre: &str, contenu: &str) {
        let titre = SharedString::from(titre.to_owned());
        let contenu = SharedString::from(contenu.to_owned());
        ouvrir(Demande::new(None, move |_, cx, fermeture| {
            let vue = cx.new(|cx| Self {
                titre,
                contenu,
                focus: cx.focus_handle(),
                fermeture,
            });
            let focus = vue.read(cx).focus.clone();
            Ouverture {
                vue: vue.into(),
                focus,
            }
        }));
    }
}

impl Render for Modale {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme_ou_defaut(cx);
        let fermer = self.fermeture.clone();
        let echap = self.fermeture.clone();
        let mut boite = div()
            .id("modal")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(move |_, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    echap.demander();
                    cx.notify();
                }
            }))
            .relative()
            .flex()
            .flex_col()
            .w(largeur_boite(window, cx, 560.0))
            .max_h(window.viewport_size().height * 0.70)
            .py(u(cx, t.forme.size_4_5))
            .px(u(cx, t.forme.size_4_6))
            .bg(couleur(t.palette.workspace_bg))
            .text_color(couleur(t.palette.text_normal))
            .border(u(cx, t.forme.border_width))
            .border_color(couleur(t.palette.tab_outline_color))
            .rounded(u(cx, t.forme.radius_l))
            .shadow(crate::modal::ombres(&t.palette.shadow_l))
            .font_family(FAMILLE_INTERFACE)
            .text_size(u(cx, t.typo.font_ui_normal))
            .line_height(relative(t.typo.line_height_normal))
            .overflow_hidden()
            // La boîte avale les clics : le voile derrière est le seul à fermer.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        // Le bouton de fermeture : 24 × 24 à 10 px du coin, icône de 16 px.
        boite = boite.child(
            div()
                .id("modal-fermer")
                .absolute()
                .top(u(cx, 10.0))
                .right(u(cx, 10.0))
                .size(u(cx, 24.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(u(cx, t.forme.radius_sm))
                .text_color(couleur(t.palette.text_muted))
                .cursor_pointer()
                .hover(|s| {
                    s.bg(couleur(t.palette.background_modifier_active_hover))
                        .text_color(couleur(t.palette.text_normal))
                })
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    fermer.demander();
                    cx.stop_propagation();
                    window.refresh();
                })
                .child(
                    svg()
                        .path(icones::chemin("x"))
                        .size(u(cx, t.forme.icon_s))
                        .text_color(couleur(t.palette.text_muted)),
                ),
        );
        if !self.titre.is_empty() {
            boite = boite.child(
                div()
                    .text_size(u(cx, t.typo.font_ui_larger))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .mb(u(cx, t.forme.size_4_3))
                    .child(self.titre.clone()),
            );
        }
        boite.child(div().child(self.contenu.clone()))
    }
}

/// Les ombres d'un jeton, en ombres GPUI.
pub(crate) fn ombres(jeton: &[fragment_theme::Ombre]) -> Vec<gpui_kit::BoxShadow> {
    jeton
        .iter()
        .map(|o| gpui_kit::BoxShadow {
            color: gpui_kit::Hsla::from(couleur(o.couleur)),
            offset: gpui_kit::point(gpui_kit::px(o.decalage_x), gpui_kit::px(o.decalage_y)),
            blur_radius: gpui_kit::px(o.flou),
            spread_radius: gpui_kit::px(o.etendue),
            inset: false,
        })
        .collect()
}

// ── Les scènes ────────────────────────────────────────────────────────────────

/// Le titre de la modale des scènes (`t1-modale`).
pub const SCENE_TITRE: &str = "Titre de la modale";
/// Le contenu de la modale des scènes.
pub const SCENE_CONTENU: &str = "Un paragraphe de contenu, pour mesurer le cadre de la modale.";

/// Enregistre la commande de scène `pixels:modal` : ouvre la modale générique de `t1-modale`.
pub fn commandes_de_scene(commandes: &CommandManager) {
    let commande = fragment_core::Command::new("pixels:modal", "Pixels: modale")
        .with_callback(|| Modale::ouvrir(SCENE_TITRE, SCENE_CONTENU));
    // Le retrait n'est jamais appelé : la commande vit autant que le gestionnaire de la scène.
    let _ = commandes.add_command(commande, "pixels");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_demande_de_meme_cle_est_ignoree() {
        assert!(!doit_ouvrir(Some("palette"), Some("palette")));
        assert!(doit_ouvrir(Some("palette"), Some("autre")));
        assert!(doit_ouvrir(None, Some("palette")));
        assert!(doit_ouvrir(Some("palette"), None));
    }
}
