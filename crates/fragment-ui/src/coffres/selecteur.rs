// Port de app/src/ui/core/VaultSwitcher.ts (le menu de bascule).
//
//! Le menu du sélecteur de coffre, au pied du dock gauche : un item par coffre connu (le courant
//! coché, un introuvable grisé), un séparateur, puis « Gérer les coffres… ».
//!
//! ★ La liste peut échouer : le menu s'ouvre QUAND MÊME, avec une entrée grisée qui le dit, et
//!   l'issue de secours reste offerte. Un clic qui ne produit rien est le pire des diagnostics.
//!
//! Le pied du dock (`docks::rendre_pied_coffre`) ne connaît pas le registre : son clic appelle
//! [`clic_du_pied`], qui sert l'ouvreur que l'application a [`brancher`]é.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_cli::coffres::{ActionMenu, EntreeMenu, VaultInfo, menu_selecteur};
use fragment_core::Disposer;

use crate::menu::Menu;

/// Ce que déclenchent les items du menu.
#[derive(Clone)]
pub struct ActionsSelecteur {
    /// Basculer sur ce coffre.
    pub basculer: Rc<dyn Fn(String)>,
    /// « Gérer les coffres… ».
    pub gerer: Rc<dyn Fn()>,
}

/// Construit le menu. `coffres = None` : la liste a échoué.
pub fn menu(coffres: Option<&[VaultInfo]>, actions: &ActionsSelecteur) -> Menu {
    let mut menu = Menu::new();
    for entree in menu_selecteur(coffres) {
        match entree {
            EntreeMenu::Separateur => {
                menu.add_separator();
            }
            EntreeMenu::Item(it) => {
                menu.add_item(|m| {
                    m.set_title(&it.titre)
                        .set_checked(it.coche)
                        .set_disabled(it.desactive);
                    if let Some(icone) = it.icone {
                        m.set_icon(icone);
                    }
                    match it.action {
                        Some(ActionMenu::Basculer(chemin)) => {
                            let basculer = actions.basculer.clone();
                            m.on_click(move || basculer(chemin.clone()));
                        }
                        Some(ActionMenu::OuvrirGestionnaire) => {
                            let gerer = actions.gerer.clone();
                            m.on_click(move || gerer());
                        }
                        None => {}
                    }
                });
            }
        }
    }
    menu
}

/// L'ouvreur du menu : reçoit le coin haut-gauche du pied (coordonnées de la fenêtre).
pub type Ouvreur = Rc<dyn Fn(f32, f32)>;

thread_local! {
    static OUVREUR: RefCell<Option<Ouvreur>> = const { RefCell::new(None) };
}

/// Installe l'ouvreur du menu (un seul à la fois) ; le retrait le désinstalle.
pub fn brancher(ouvreur: Ouvreur) -> Disposer {
    OUVREUR.with(|o| *o.borrow_mut() = Some(ouvreur));
    Box::new(|| OUVREUR.with(|o| *o.borrow_mut() = None))
}

/// Le clic sur le sélecteur : ouvre le menu au-dessus du pied, aligné à gauche (le menu se replace
/// vers le haut puisqu'on est en bas de la fenêtre). Rend `false` si aucun ouvreur n'est branché.
pub fn clic_du_pied(x: f32, y: f32) -> bool {
    let ouvreur = OUVREUR.with(|o| o.borrow().clone());
    match ouvreur {
        Some(f) => {
            f(x, y);
            true
        }
        None => false,
    }
}
