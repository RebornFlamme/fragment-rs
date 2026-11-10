// Port de app/src/ui/core/Toolbar.ts et toolbar.css (le rendu)
//! La barre d'outils flottante : le pendant persistant du menu contextuel.
//!
//! Le MODÈLE (items, sections, groupes exclusifs, orientation, bornage) est
//! `fragment_annotation::toolbar::Toolbar`, déjà porté ; ce module y ajoute les ACTIONS des items
//! ([`BarreOutils`]) et le RENDU ([`couche`]). La barre ouverte vit dans un magasin du fil.
//!
//! ★ Pas porté ici : le glisser de la poignée (le modèle sait suivre le geste, l'entrée pointeur
//! arrive avec l'assemblage), la mise en colonnes du mode vertical, la rangée d'options
//! (`setOptions`) et l'Échap par `porteePersistante` (le propriétaire de la barre le branche).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub use fragment_annotation::toolbar::{
    Enfant, Forme, ItemMut, Toolbar, ToolbarItem, ToolbarOrientation,
};
use fragment_core::{Command, CommandManager};
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, anchored, deferred, div, point, svg,
};

use crate::echelle::u;
use crate::flottant::{bordure, chaine, ecouter_args, nombre, ombres, texte};
use crate::icones;
use crate::theme_global::{couleur, theme_ou_defaut};

type Action = Rc<dyn Fn()>;

/// Le modèle de la barre et les actions de ses items.
#[derive(Default)]
pub struct BarreOutils {
    modele: Toolbar,
    actions: HashMap<usize, Action>,
}

impl BarreOutils {
    /// Une barre vide, horizontale.
    pub fn new() -> Self {
        Self::default()
    }

    /// Le modèle (sections, items, orientation).
    pub fn modele(&self) -> &Toolbar {
        &self.modele
    }

    /// Le modèle, modifiable.
    pub fn modele_mut(&mut self) -> &mut Toolbar {
        &mut self.modele
    }

    /// Ajoute un item ; `action` part au clic (hors item désactivé).
    pub fn add_item(
        &mut self,
        cb: impl FnOnce(&mut ItemMut<'_>),
        action: Option<impl Fn() + 'static>,
    ) -> &mut Self {
        let index = self.modele.items().len();
        self.modele.add_item(cb);
        if let Some(a) = action {
            self.actions.insert(index, Rc::new(a));
        }
        self
    }

    /// Déclare l'ordre des sections (réservé à celui qui construit la barre).
    pub fn add_sections(&mut self, sections: &[&str]) -> &mut Self {
        self.modele.add_sections(sections);
        self
    }

    /// Un séparateur manuel.
    pub fn add_separator(&mut self) -> &mut Self {
        self.modele.add_separator();
        self
    }

    /// Vide la barre (items, sections, actions) sans la fermer.
    pub fn clear(&mut self) -> &mut Self {
        self.modele.clear();
        self.actions.clear();
        self
    }

    /// Montre la barre à `(x, y)` (coordonnées de la fenêtre).
    pub fn show_at_position(mut self, x: f32, y: f32) {
        if self.modele.show() {
            COURANTE.with(|c| *c.borrow_mut() = Some(Ouverte { barre: self, x, y }));
        }
    }
}

struct Ouverte {
    barre: BarreOutils,
    x: f32,
    y: f32,
}

thread_local! {
    static COURANTE: RefCell<Option<Ouverte>> = const { RefCell::new(None) };
}

/// Ferme la barre ouverte (ses `on_hide` partent une fois).
pub fn fermer_barre() {
    if let Some(mut o) = COURANTE.with(|c| c.borrow_mut().take()) {
        o.barre.modele.hide();
    }
}

/// Une barre est-elle ouverte ?
pub fn barre_ouverte() -> bool {
    COURANTE.with(|c| c.borrow().is_some())
}

/// Clic sur l'item `index` : arme le groupe, puis appelle l'action. La barre RESTE ouverte.
/// Rend `true` si l'affichage a pu changer.
pub fn cliquer(index: usize) -> bool {
    let action = COURANTE.with(|c| {
        let mut garde = c.borrow_mut();
        let o = garde.as_mut()?;
        o.barre
            .modele
            .click(index)
            .then(|| o.barre.actions.get(&index).cloned())
    });
    match action {
        None => false,
        Some(a) => {
            if let Some(a) = a {
                a();
            }
            true
        }
    }
}

/// Enregistre la commande de scène `pixels:toolbar`
/// (`{ "x", "y", "variante": "horizontale" | "sections" }`).
pub fn commandes_de_scene(commandes: &CommandManager) {
    // Chaque scène (et chaque thème) repart d'un magasin vide : le processus rejoue toutes les scènes.
    fermer_barre();
    let args = ecouter_args(commandes);
    let commande = Command::new("pixels:toolbar", "Pixels : ouvrir la barre d'outils")
        .with_callback(move || {
            let a = args.borrow();
            let x = nombre(&a, "x").unwrap_or(300.0);
            let y = nombre(&a, "y").unwrap_or(250.0);
            let mut barre = BarreOutils::new();
            if chaine(&a, "variante") == Some("sections") {
                barre.add_sections(&["outil", "style", "agent"]);
                barre.add_item(
                    |i| {
                        i.set_icon("palette")
                            .set_tooltip("Couleur")
                            .set_section("style");
                    },
                    None::<fn()>,
                );
                barre.add_item(
                    |i| {
                        i.set_icon("sparkles")
                            .set_label("Demander")
                            .set_tooltip("Demander")
                            .set_section("agent");
                    },
                    None::<fn()>,
                );
                barre.add_item(
                    |i| {
                        i.set_icon("pencil")
                            .set_tooltip("Crayon")
                            .set_section("outil")
                            .set_group("outil")
                            .set_active(true);
                    },
                    None::<fn()>,
                );
                barre.add_item(
                    |i| {
                        i.set_icon("eraser")
                            .set_tooltip("Gomme")
                            .set_section("outil")
                            .set_group("outil");
                    },
                    None::<fn()>,
                );
            } else {
                for (icone, nom, actif) in [
                    ("pencil", "Crayon", true),
                    ("eraser", "Gomme", false),
                    ("highlighter", "Surligneur", false),
                ] {
                    barre.add_item(
                        |i| {
                            i.set_icon(icone)
                                .set_tooltip(nom)
                                .set_group("outil")
                                .set_active(actif);
                        },
                        None::<fn()>,
                    );
                }
                barre.add_separator();
                barre.add_item(
                    |i| {
                        i.set_icon("undo-2").set_tooltip("Annuler");
                    },
                    None::<fn()>,
                );
                barre.add_item(
                    |i| {
                        i.set_icon("redo-2")
                            .set_tooltip("Rétablir")
                            .set_disabled(true);
                    },
                    None::<fn()>,
                );
            }
            barre.show_at_position(x, y);
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// La couche de la barre : rien si aucune barre n'est ouverte.
pub fn couche(cx: &App) -> AnyElement {
    let rendu = COURANTE.with(|c| {
        let garde = c.borrow();
        let o = garde.as_ref()?;
        Some((rendre(&o.barre.modele, cx), o.x, o.y))
    });
    let Some((barre, x, y)) = rendu else {
        return div().into_any_element();
    };
    deferred(
        anchored()
            .position(point(u(cx, x), u(cx, y)))
            .snap_to_window_with_margin(u(cx, 8.0))
            .child(barre),
    )
    .into_any_element()
}

fn rendre_item(index: usize, item: &ToolbarItem, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let couleur_texte = if item.active {
        c.text_on_accent
    } else {
        c.text_muted
    };
    let mut el = div()
        .id(("toolbar-item", index))
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_3))
        .px(u(cx, 7.0))
        .py(u(cx, 5.0))
        .rounded(u(cx, f.radius_sm))
        .cursor_default()
        .text_color(couleur(couleur_texte));
    if item.active {
        el = el.bg(couleur(c.color_accent));
    } else if item.disabled {
        el = el.opacity(0.35);
    } else {
        let (fond, texte_survol) = (
            couleur(c.background_modifier_active_hover),
            couleur(c.text_normal),
        );
        el = el.hover(move |s| s.bg(fond).text_color(texte_survol));
    }
    if item.disabled && item.active {
        el = el.opacity(0.35);
    }
    if !item.icon.is_empty() {
        el = el.child(
            div()
                .w(u(cx, f.icon_m))
                .h(u(cx, f.icon_m))
                .flex()
                .items_center()
                .justify_center()
                .opacity(if item.active { 1.0 } else { 0.8 })
                .child(
                    svg()
                        .path(icones::chemin(&item.icon))
                        .size(u(cx, f.icon_s))
                        .flex_none()
                        .text_color(couleur(couleur_texte)),
                ),
        );
    }
    if !item.label.is_empty() {
        el = el.child(item.label.clone());
    }
    el.on_click(move |_, _: &mut Window, app: &mut App| {
        if cliquer(index) {
            app.refresh_windows();
        }
    })
    .into_any_element()
}

fn rendre(modele: &Toolbar, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let vertical = modele.orientation() == ToolbarOrientation::Vertical;
    let mut enfants: Vec<AnyElement> = Vec::new();
    for enfant in modele.enfants() {
        match enfant {
            Enfant::Section { items, .. } if !items.is_empty() => {
                let rangee: Vec<AnyElement> = items
                    .iter()
                    .filter_map(|&i| modele.items().get(i).map(|it| rendre_item(i, it, cx)))
                    .collect();
                let section = div().flex().items_center().gap(u(cx, f.size_2_1));
                let section = if vertical {
                    section.flex_col()
                } else {
                    section
                };
                enfants.push(section.children(rangee).into_any_element());
            }
            Enfant::Section { .. } => {}
            Enfant::Separateur => {
                let sep = div().bg(couleur(c.tab_outline_color));
                let sep = if vertical {
                    sep.h(u(cx, 1.0))
                        .self_stretch()
                        .mx(u(cx, f.size_2_1))
                        .my(u(cx, f.size_4_1))
                } else {
                    sep.w(u(cx, 1.0))
                        .self_stretch()
                        .mx(u(cx, f.size_4_1))
                        .my(u(cx, f.size_2_1))
                };
                enfants.push(sep.into_any_element());
            }
            Enfant::Poignee => {
                let poignee = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .self_stretch()
                    .min_w(u(cx, 16.0))
                    .px(u(cx, 1.0))
                    .rounded(u(cx, f.radius_sm))
                    .text_color(couleur(c.text_muted))
                    .opacity(0.55);
                let poignee = if vertical {
                    poignee.mt(u(cx, f.size_2_1))
                } else {
                    poignee.ml(u(cx, f.size_2_1))
                };
                enfants.push(
                    poignee
                        .child(
                            svg()
                                .path(icones::chemin(modele.icone_de_poignee()))
                                .size(u(cx, f.icon_xs))
                                .flex_none()
                                .text_color(couleur(c.text_muted)),
                        )
                        .into_any_element(),
                );
            }
        }
    }
    let racine = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_1))
        .p(u(cx, f.size_4_1))
        .rounded(u(cx, f.radius_ml))
        .bg(couleur(c.background_secondary))
        .text_color(couleur(c.text_normal))
        .shadow(ombres(cx, &c.menu_shadow));
    let racine = if vertical { racine.flex_col() } else { racine };
    let racine = bordure(racine, u(cx, f.border_width))
        .border_color(couleur(c.tab_outline_color))
        .children(enfants);
    texte(racine, cx, t.typo.font_ui_small, 1.5).into_any_element()
}
