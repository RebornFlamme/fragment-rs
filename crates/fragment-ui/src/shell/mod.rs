// Port de l'assemblage du DOM de app/src/ui/bootApp.tsx (le conteneur du workspace, la barre d'état,
// les boutons de fenêtre) et des calques flottants qu'y posent Notice, Menu, Tooltip et Modal.
//
//! La coquille : la vue racine de la fenêtre principale.
//!
//! Ordre de peinture = ordre des `z-index` de Fragment : le workspace (ruban, docks, piles, poignées,
//! boutons de fenêtre, fantôme du glisser), la barre d'état (10), les notices et le menu (1000),
//! l'infobulle (1100), et les modales devant tout (la palette en est une).
//!
//! ★ `DockArea` de gpui-kit n'est plus là (T1.md, choix 1) : l'arbre est celui du cœur, rendu par
//!   [`crate::splits::rendre_workspace`].

pub mod barre_etat;

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::CommandManager;
use gpui_kit::{Context, IntoElement, Render, SharedString, Window};

use crate::rendu::mode_texte::pile;
use crate::splits::{ContexteSplits, rendre_workspace};
use crate::{menu, modal, notice, tooltip};

/// Les éléments de la barre d'état (`app.statusBarItems`), posés par les plugins.
pub type ElementsBarreEtat = Rc<RefCell<Vec<SharedString>>>;

/// La vue racine de la fenêtre principale.
pub struct Coquille {
    /// Le workspace rendu (et son état de geste).
    pub splits: ContexteSplits,
    /// Les éléments de la barre d'état.
    pub barre_etat: ElementsBarreEtat,
}

impl Coquille {
    pub fn new(splits: ContexteSplits, barre_etat: ElementsBarreEtat) -> Self {
        Self { splits, barre_etat }
    }
}

impl Render for Coquille {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let elements = self.barre_etat.borrow().clone();
        // Le workspace est la surface opaque (ClearType) ; la barre d'état (`position: fixed`,
        // z-index 10) et les couches au-dessus sont des calques (gris) : ADR-017.
        pile(
            rendre_workspace(&self.splits, window.viewport_size(), cx),
            [
                barre_etat::rendre(&elements, cx),
                notice::couche(cx),
                menu::couche(cx),
                tooltip::couche(cx),
                modal::couche(window, cx).into_any_element(),
            ],
        )
    }
}

/// Enregistre les commandes de scène `pixels:shell` : aucune, les scènes de la coquille passent
/// par `layout` et par les commandes réelles de l'application.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
