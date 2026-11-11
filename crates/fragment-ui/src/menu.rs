// Port de app/src/ui/core/Menu.ts et menu.css
//! Le menu contextuel : un builder impératif, à durée de vie courte (construit, montré, détruit).
//!
//! Le MODÈLE ([`Menu`], [`MenuItem`]) ne connaît pas GPUI : sections, séparateurs régénérés,
//! sous-menus. Le menu OUVERT vit dans un magasin du fil (voir `flottant`) et se dessine par
//! [`couche`]. Échap passe par la pile de portées du `Keymap` (`Scope::echap`) : tant que le menu
//! est ouvert, sa portée masque les raccourcis globaux.
//!
//! ★ Le contrat des sections est celui d'Obsidian 0.15.3 : `set_section` est public, `add_sections`
//! est réservé à l'ouvreur du menu ; sans aucune section, le menu garde son ordre d'insertion et ses
//! séparateurs manuels ; avec des sections, les séparateurs manuels sont retirés et ce sont les
//! groupes qui séparent.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::{Command, CommandManager, Keymap, Scope};
use gpui_kit::{
    AnyElement, App, Bounds, InteractiveElement as _, IntoElement, MouseDownEvent,
    ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _, Window, anchored,
    canvas, deferred, div, point, relative, svg,
};
use serde_json::Value;

use crate::echelle::{calees, moitie, u};
use crate::flottant::{bordure, bordure_translucide, ecouter_args, nombre, ombrer, texte};
use crate::icones;
use crate::theme_global::{couleur, theme_ou_defaut};
use fragment_theme::Aplat;

/// Largeur minimale du contenu d'un menu (`min-width`, boîte de contenu).
const LARGEUR_MIN: f32 = 180.0;
/// Côté de l'icône d'une entrée.
const ICONE: f32 = 15.0;

/// Une action d'entrée (clic).
type Action = Rc<dyn Fn()>;

/// Une entrée de menu. Construite par [`Menu::add_item`].
#[derive(Default)]
pub struct MenuItem {
    title: String,
    icon: Option<String>,
    checked: bool,
    disabled: bool,
    warning: bool,
    section: String,
    submenu: Option<Menu>,
    on_click: Option<Action>,
}

impl MenuItem {
    /// Le libellé.
    pub fn set_title(&mut self, title: &str) -> &mut Self {
        self.title = title.to_owned();
        self
    }

    /// Le nom d'une icône du registre (`icones`).
    pub fn set_icon(&mut self, icon: &str) -> &mut Self {
        self.icon = Some(icon.to_owned());
        self
    }

    /// Coche l'entrée (un ✓ après le libellé).
    pub fn set_checked(&mut self, checked: bool) -> &mut Self {
        self.checked = checked;
        self
    }

    /// Désactive l'entrée : atténuée, sans survol, sans réaction au clic.
    pub fn set_disabled(&mut self, disabled: bool) -> &mut Self {
        self.disabled = disabled;
        self
    }

    /// Marque une entrée destructive (en rouge).
    pub fn set_warning(&mut self, warning: bool) -> &mut Self {
        self.warning = warning;
        self
    }

    /// Place l'entrée dans une section ('' = sans section).
    pub fn set_section(&mut self, section: &str) -> &mut Self {
        self.section = section.to_owned();
        self
    }

    /// L'action du clic.
    pub fn on_click(&mut self, action: impl Fn() + 'static) -> &mut Self {
        self.on_click = Some(Rc::new(action));
        self
    }

    /// Attache un sous-menu à cette entrée et le rend, pour le peupler.
    pub fn set_submenu(&mut self) -> &mut Menu {
        self.submenu.insert(Menu::new())
    }

    /// Le libellé.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// La section ('' = aucune).
    pub fn section(&self) -> &str {
        &self.section
    }

    /// L'entrée est-elle désactivée ?
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// L'entrée est-elle cochée ?
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// L'entrée est-elle destructive ?
    pub fn is_warning(&self) -> bool {
        self.warning
    }

    /// Le sous-menu, s'il y en a un.
    pub fn submenu(&self) -> Option<&Menu> {
        self.submenu.as_ref()
    }
}

enum Entree {
    Item(MenuItem),
    Separateur,
}

/// Une ligne du menu affiché.
pub enum Ligne<'a> {
    /// Une entrée.
    Item(&'a MenuItem),
    /// Un séparateur.
    Separateur,
}

/// Le menu (le modèle, sans rendu).
#[derive(Default)]
pub struct Menu {
    entrees: Vec<Entree>,
    sections: Vec<String>,
    on_hide: Vec<Action>,
}

impl Menu {
    /// Un menu vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Le builder central.
    pub fn add_item(&mut self, cb: impl FnOnce(&mut MenuItem)) -> &mut Self {
        let mut item = MenuItem::default();
        cb(&mut item);
        self.entrees.push(Entree::Item(item));
        self
    }

    /// Un séparateur manuel (retiré si le menu a des sections).
    pub fn add_separator(&mut self) -> &mut Self {
        self.entrees.push(Entree::Separateur);
        self
    }

    /// Déclare l'ordre des sections : réservé à celui qui OUVRE le menu.
    pub fn add_sections(&mut self, sections: &[&str]) -> &mut Self {
        self.sections
            .extend(sections.iter().map(|s| (*s).to_owned()));
        self
    }

    /// Une fonction appelée à la fermeture.
    pub fn on_hide(&mut self, action: impl Fn() + 'static) -> &mut Self {
        self.on_hide.push(Rc::new(action));
        self
    }

    /// L'ordre d'affichage : `Some(i)` = l'entrée `i` de `entrees`, `None` = un séparateur.
    fn ordre(&self) -> Vec<Option<usize>> {
        let items = || {
            self.entrees
                .iter()
                .enumerate()
                .filter_map(|(i, e)| match e {
                    Entree::Item(it) => Some((i, it)),
                    Entree::Separateur => None,
                })
        };
        // OPT-IN : sans section, l'ordre d'insertion et les séparateurs manuels restent.
        if self.sections.is_empty() && items().all(|(_, it)| it.section.is_empty()) {
            return self
                .entrees
                .iter()
                .enumerate()
                .map(|(i, e)| matches!(e, Entree::Item(_)).then_some(i))
                .collect();
        }
        let mut ordre_sections = self.sections.clone();
        for (_, it) in items() {
            if !ordre_sections.contains(&it.section) {
                ordre_sections.push(it.section.clone());
            }
        }
        let mut sortie = Vec::new();
        let mut premier = true;
        for section in &ordre_sections {
            let groupe: Vec<usize> = items()
                .filter(|(_, it)| &it.section == section)
                .map(|(i, _)| i)
                .collect();
            if groupe.is_empty() {
                continue;
            }
            if !premier {
                sortie.push(None);
            }
            sortie.extend(groupe.into_iter().map(Some));
            premier = false;
        }
        sortie
    }

    /// Les lignes dans l'ordre où le menu les affiche.
    pub fn lignes(&self) -> Vec<Ligne<'_>> {
        self.ordre()
            .into_iter()
            .map(|o| match o.and_then(|i| self.entrees.get(i)) {
                Some(Entree::Item(it)) => Ligne::Item(it),
                _ => Ligne::Separateur,
            })
            .collect()
    }

    /// Les libellés affichés, `"|"` pour un séparateur (pratique pour comparer un ordre).
    pub fn forme(&self) -> Vec<String> {
        self.lignes()
            .into_iter()
            .map(|l| match l {
                Ligne::Item(it) => it.title.clone(),
                Ligne::Separateur => "|".to_owned(),
            })
            .collect()
    }

    /// Montre le menu à `(x, y)` (coordonnées de la fenêtre) et empile sa portée Échap sur `keymap`.
    /// Un menu déjà ouvert est fermé d'abord.
    pub fn show_at_position(self, x: f32, y: f32, keymap: &Keymap) {
        fermer_menu();
        let portee = Scope::echap(fermer_menu);
        keymap.push_scope(&portee);
        COURANT.with(|c| {
            *c.borrow_mut() = Some(Ouvert {
                menu: self,
                x,
                y,
                chemin: Vec::new(),
                niveaux: Vec::new(),
                portee: Some((keymap.clone(), portee)),
            });
        });
    }

    fn item_a(&self, i: usize) -> Option<&MenuItem> {
        match self.entrees.get(i) {
            Some(Entree::Item(it)) => Some(it),
            _ => None,
        }
    }
}

/// Le menu ouvert et son état de navigation.
struct Ouvert {
    menu: Menu,
    x: f32,
    y: f32,
    /// Les entrées dont le sous-menu est déployé, de la racine vers le fond.
    chemin: Vec<usize>,
    /// Le rectangle peint de chaque niveau (pour reconnaître un clic « dedans »).
    niveaux: Vec<Bounds<Pixels>>,
    portee: Option<(Keymap, Scope)>,
}

impl Ouvert {
    /// Le menu du `niveau` (0 = racine) selon le chemin déployé.
    fn menu_au_niveau(&self, niveau: usize) -> Option<&Menu> {
        let mut m = &self.menu;
        for i in 0..niveau {
            m = m.item_a(*self.chemin.get(i)?)?.submenu.as_ref()?;
        }
        Some(m)
    }
}

thread_local! {
    static COURANT: RefCell<Option<Ouvert>> = const { RefCell::new(None) };
}

/// Ferme le menu ouvert (et toute sa chaîne de sous-menus) : dépile sa portée, puis appelle les
/// `on_hide`. Sans effet s'il n'y en a pas.
pub fn fermer_menu() {
    let ouvert = COURANT.with(|c| c.borrow_mut().take());
    if let Some(o) = ouvert {
        if let Some((keymap, portee)) = &o.portee {
            keymap.pop_scope(portee);
        }
        for action in &o.menu.on_hide {
            action();
        }
    }
}

/// Un menu est-il ouvert ?
pub fn menu_ouvert() -> bool {
    COURANT.with(|c| c.borrow().is_some())
}

/// Les sous-menus déployés (le nombre d'entrées du chemin).
pub fn profondeur_ouverte() -> usize {
    COURANT.with(|c| c.borrow().as_ref().map_or(0, |o| o.chemin.len()))
}

/// Survol de l'entrée `idx` du `niveau` : déploie son sous-menu et replie les autres.
/// Rend `true` si l'affichage a changé.
pub fn survoler(niveau: usize, idx: usize) -> bool {
    COURANT.with(|c| {
        let mut garde = c.borrow_mut();
        let Some(o) = garde.as_mut() else {
            return false;
        };
        let a_un_sous_menu = o
            .menu_au_niveau(niveau)
            .and_then(|m| m.item_a(idx))
            .is_some_and(|it| it.submenu.is_some());
        let avant = o.chemin.clone();
        o.chemin.truncate(niveau);
        if a_un_sous_menu && o.chemin.len() == niveau {
            o.chemin.push(idx);
        }
        o.niveaux.truncate(o.chemin.len() + 1);
        o.chemin != avant
    })
}

/// Clic sur l'entrée `idx` du `niveau`. Rend `true` si l'affichage a changé.
///
/// Une entrée désactivée ne réagit pas ; une entrée à sous-menu le déploie sans rien fermer ;
/// sinon la chaîne ENTIÈRE se ferme AVANT l'action, pour qu'elle puisse ouvrir autre chose.
pub fn cliquer(niveau: usize, idx: usize) -> bool {
    enum Suite {
        Rien,
        Deployer,
        Agir(Option<Action>),
    }
    let suite = COURANT.with(|c| {
        let garde = c.borrow();
        let Some(it) = garde
            .as_ref()
            .and_then(|o| o.menu_au_niveau(niveau))
            .and_then(|m| m.item_a(idx))
        else {
            return Suite::Rien;
        };
        if it.disabled {
            Suite::Rien
        } else if it.submenu.is_some() {
            Suite::Deployer
        } else {
            Suite::Agir(it.on_click.clone())
        }
    });
    match suite {
        Suite::Rien => false,
        Suite::Deployer => survoler(niveau, idx),
        Suite::Agir(action) => {
            fermer_menu();
            if let Some(a) = action {
                a();
            }
            true
        }
    }
}

fn enregistrer_niveau(niveau: usize, bounds: Bounds<Pixels>) {
    COURANT.with(|c| {
        if let Some(o) = c.borrow_mut().as_mut() {
            if o.niveaux.len() <= niveau {
                o.niveaux.resize(niveau + 1, bounds);
            }
            o.niveaux[niveau] = bounds;
        }
    });
}

/// Un appui hors de TOUS les niveaux ferme le menu (les sous-menus vivent hors du rectangle de
/// leur parent : sans ce contrôle, cliquer leur entrée fermerait le menu avant le clic).
fn appui_exterieur(evt: &MouseDownEvent, _fenetre: &mut Window, app: &mut App) {
    let dedans = COURANT.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|o| o.niveaux.iter().any(|b| b.contains(&evt.position)))
    });
    if !dedans {
        fermer_menu();
        app.refresh_windows();
    }
}

/// Enregistre la commande de scène `pixels:menu` (`{ "x": …, "y": … }`) : le menu de la scène
/// `t1-menu` (sections, entrée désactivée, sous-menu, entrée destructive).
pub fn commandes_de_scene(commandes: &CommandManager) {
    // Chaque scène (et chaque thème) repart d'un magasin vide : le processus rejoue toutes les scènes.
    fermer_menu();
    let args = ecouter_args(commandes);
    let commande =
        Command::new("pixels:menu", "Pixels : ouvrir le menu").with_callback(move || {
            let a: Value = args.borrow().clone();
            let x = nombre(&a, "x").unwrap_or(420.0);
            let y = nombre(&a, "y").unwrap_or(260.0);
            let mut menu = Menu::new();
            menu.add_sections(&["edition", "fichier", "danger"]);
            menu.add_item(|i| {
                i.set_title("Copier")
                    .set_icon("copy")
                    .set_section("edition");
            });
            menu.add_item(|i| {
                i.set_title("Coller").set_section("edition");
            });
            menu.add_item(|i| {
                i.set_title("Renommer")
                    .set_section("fichier")
                    .set_disabled(true);
            });
            menu.add_item(|i| {
                i.set_title("Trier par").set_section("fichier");
                i.set_submenu().add_item(|s| {
                    s.set_title("Nom");
                });
            });
            menu.add_item(|i| {
                i.set_title("Supprimer")
                    .set_icon("trash-2")
                    .set_section("danger")
                    .set_warning(true);
            });
            // Ajoutée APRÈS : elle atterrit tout de même dans le groupe « edition ».
            menu.add_item(|i| {
                i.set_title("Dupliquer").set_section("edition");
            });
            menu.show_at_position(x, y, &Keymap::new());
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// La couche du menu : rien si aucun menu n'est ouvert.
pub fn couche(cx: &App) -> AnyElement {
    let rendu = COURANT.with(|c| {
        let garde = c.borrow();
        let o = garde.as_ref()?;
        Some((rendre_niveau(&o.menu, 0, &o.chemin, o.y, cx), o.x, o.y))
    });
    let Some((menu, x, y)) = rendu else {
        return div().into_any_element();
    };
    deferred(
        anchored()
            .position(point(u(cx, x), u(cx, y)))
            .snap_to_window_with_margin(u(cx, 8.0))
            .child(menu),
    )
    .into_any_element()
}

/// `origine_y` : position exacte (logique) du bord haut de la boîte du niveau, pour caler les lignes.
fn rendre_niveau(
    menu: &Menu,
    niveau: usize,
    chemin: &[usize],
    origine_y: f32,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    // ★ Fragment compte 29,5 px par ligne (2 × 5 px + interligne 1,5 × 13 px) : on cale les arêtes
    // comme Chromium (voir `echelle::calees`) au lieu d'écrire 29,5, que GPUI arrondirait à 29.
    let ordre = menu.ordre();
    let ligne_exacte = 2.0 * 5.0 + 1.5 * t.typo.font_ui_medium;
    let separateur_exact = 1.0 + 2.0 * f.size_4_1;
    let hauteurs_exactes: Vec<f32> = ordre
        .iter()
        .map(|o| {
            if o.is_some() {
                ligne_exacte
            } else {
                separateur_exact
            }
        })
        .collect();
    // Le contenu commence après la bordure et le rembourrage de la boîte.
    let debut = origine_y + f.border_width + f.size_4_1;
    let hauteurs = calees(cx, debut, &hauteurs_exactes);
    let mut haut_exact = debut;
    let lignes: Vec<AnyElement> = ordre
        .into_iter()
        .enumerate()
        .map(|(k, o)| {
            let haut_ligne = haut_exact;
            haut_exact += hauteurs_exactes[k];
            let hauteur = hauteurs[k];
            (haut_ligne, hauteur, o)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(
            |(haut_ligne, hauteur, o)| match o.and_then(|i| menu.item_a(i).map(|it| (i, it))) {
                None => div()
                    .h(u(cx, 1.0))
                    .mx(u(cx, f.size_2_3))
                    .my(u(cx, f.size_4_1))
                    .bg(couleur(c.aplat(Aplat::SurvolActif, c.menu_bg)))
                    .into_any_element(),
                Some((idx, it)) => {
                    let couleur_texte = if it.warning {
                        c.text_error
                    } else {
                        c.text_normal
                    };
                    let icone = div()
                        .w(u(cx, f.icon_m))
                        .flex()
                        .pl(moitie(cx, f.icon_m - ICONE))
                        .opacity(if it.warning { 1.0 } else { 0.7 })
                        .children(it.icon.as_ref().map(|nom| {
                            svg()
                                .path(icones::chemin(nom))
                                .size(u(cx, ICONE))
                                .flex_none()
                                .text_color(couleur(couleur_texte))
                        }));
                    let mut titre = div().flex_1().child(it.title.clone());
                    if it.checked {
                        titre = titre.child(div().opacity(0.7).child(" \u{2713}"));
                    }
                    let mut ligne = div()
                        .id(("menu-entree", niveau * 1000 + idx))
                        .relative()
                        .flex()
                        .items_center()
                        .gap(u(cx, f.size_4_2))
                        .px(u(cx, f.size_4_2))
                        // Hauteur calée (voir plus haut) : 30 et 29 px en alternance pour 29,5 exact.
                        .h(hauteur)
                        .rounded(u(cx, f.radius_s))
                        .cursor_default()
                        .text_color(couleur(couleur_texte))
                        .child(icone)
                        .child(titre);
                    if it.disabled {
                        ligne = ligne.opacity(0.4);
                    } else {
                        let survol = couleur(c.aplat(Aplat::SurvolActif, c.menu_bg));
                        ligne = ligne.hover(move |s| s.bg(survol));
                    }
                    if it.submenu.is_some() {
                        ligne = ligne.child(
                            div().flex().opacity(0.5).child(
                                svg()
                                    .path(icones::chemin("chevron-right"))
                                    .size(u(cx, f.icon_xs))
                                    .flex_none()
                                    .text_color(couleur(couleur_texte)),
                            ),
                        );
                        if let (Some(sous), true) = (&it.submenu, chemin.get(niveau) == Some(&idx))
                        {
                            ligne = ligne.child(deferred(
                                div()
                                    .absolute()
                                    .left(relative(1.0))
                                    .ml(u(cx, 2.0))
                                    .top(u(cx, -5.0))
                                    .child(rendre_niveau(
                                        sous,
                                        niveau + 1,
                                        chemin,
                                        haut_ligne - 5.0,
                                        cx,
                                    )),
                            ));
                        }
                    }
                    ligne
                        .on_hover(move |survole: &bool, _: &mut Window, app: &mut App| {
                            if *survole && survoler(niveau, idx) {
                                app.refresh_windows();
                            }
                        })
                        .on_click(move |_, _: &mut Window, app: &mut App| {
                            if cliquer(niveau, idx) {
                                app.refresh_windows();
                            }
                        })
                        .into_any_element()
                }
            },
        )
        .collect();

    // `min-width` et le rembourrage de `.menu` : boîte de contenu en CSS, boîte de bordure ici.
    let largeur_min = LARGEUR_MIN + 2.0 * f.size_4_1 + 2.0 * f.border_width;
    let boite = div()
        .id(("menu", niveau))
        .relative()
        .flex()
        .flex_col()
        .min_w(u(cx, largeur_min))
        .p(u(cx, f.size_4_1))
        .rounded(u(cx, f.radius_m))
        .bg(couleur(c.menu_bg))
        .text_color(couleur(c.text_normal))
        .on_mouse_down_out(appui_exterieur)
        .child(
            canvas(
                move |bounds, _, _| enregistrer_niveau(niveau, bounds),
                |_, (), _, _| {},
            )
            .absolute()
            .top(u(cx, 0.0))
            .left(u(cx, 0.0))
            .size_full(),
        )
        .children(lignes);
    // Bordure translucide calée sur le mélange de Chromium (voir `bordure_translucide`).
    let boite = bordure(boite, u(cx, f.border_width))
        .border_color(couleur(bordure_translucide(c, c.menu_bg)));
    let boite = ombrer(
        boite,
        cx,
        &c.menu_shadow,
        "--menu-shadow",
        u(cx, f.radius_m),
        u(cx, f.border_width),
    );
    texte(boite, cx, t.typo.font_ui_medium, 1.5).into_any_element()
}
