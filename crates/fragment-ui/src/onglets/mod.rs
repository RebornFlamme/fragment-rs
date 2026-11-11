// Port de app/src/ui/core/workspace/WorkspaceTabs.ts (la barre et les en-têtes d'onglets),
// WorkspaceLeaf.ts (l'activation au pointeur) et de app/src/ui/workspace.css.
//
//! La barre d'onglets, les en-têtes, et l'activation de la feuille au pointeur.
//!
//! - [`mesures`] : les constantes lues dans workspace.css, et le tracé PUR du contour de l'onglet
//!   actif ;
//! - [`actions`] : ce que fait chaque geste, sans GPUI (ce que testent les cas d'`activeleaf-tracking`) ;
//! - [`pile`] : le rendu d'une pile (barre, contour, feuille visible).
//!
//! L'en-tête de la VUE (← →, titre, actions) est dans [`crate::entete`].
//!
//! Le rendu lit le [`fragment_core::Workspace`] à chaque image et n'y écrit que par les gestes de
//! [`actions`] : l'arbre appartient au cœur. Les scènes de pixels agissent sur ce même Workspace
//! par des événements (`pixels:epingler`, `pixels:monter-vue`) qu'écoute [`brancher_scene`].

pub mod actions;
pub mod mesures;
pub mod pile;
pub mod titre;

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::{Command, CommandManager, EventRef, Keymap, View, ViewState, Workspace};
use gpui_kit::{AnyElement, App};
use serde_json::Value;

use crate::flottant::{ecouter_args, nombre};

pub use pile::{OptionsPile, noter_largeur_exacte, rendre_pile};

/// Le contenu d'une vue qui n'est pas la vue vide : rendu par le module de la vue (T2 et suivants).
pub type ContenuDeVue = Rc<dyn Fn(&dyn View, &App) -> Option<AnyElement>>;

/// Ce que le rendu des piles emprunte à l'application : le Workspace (le modèle), les commandes (les
/// boutons de l'en-tête EXÉCUTENT des commandes, ils ne les réimplémentent pas), le routeur clavier
/// (un menu empile sa portée Échap).
#[derive(Clone)]
pub struct ContexteOnglets {
    /// Le Workspace.
    pub workspace: Rc<RefCell<Workspace>>,
    /// Les commandes.
    pub commandes: Rc<CommandManager>,
    /// Le routeur clavier.
    pub keymap: Keymap,
    /// Le contenu des vues autres que la vue vide ; `None` : un panneau sans contenu.
    pub contenu_de_vue: Option<ContenuDeVue>,
}

/// Le nom de l'événement par lequel la scène épingle un onglet : `{ "onglet": 2 }` (rang dans la pile
/// de la feuille active du centre, 0-based).
pub const EVENEMENT_EPINGLER: &str = "pixels:epingler";
/// Le nom de l'événement par lequel la scène monte la vue vide d'une feuille : `{ "onglet": 1 }`.
pub const EVENEMENT_MONTER_VUE: &str = "pixels:monter-vue";

/// Enregistre les commandes de scène `pixels:epingler` et `pixels:monter-vue`.
///
/// ★ Une commande de scène n'a pas le Workspace : elle publie un événement (`pixels:epingler`,
/// `pixels:monter-vue`) avec ses arguments ; [`brancher_scene`], appelé par l'assemblage, l'écoute.
pub fn commandes_de_scene(commandes: &CommandManager) {
    for (id, nom, evenement) in [
        (
            "pixels:epingler",
            "Pixels : épingler un onglet",
            EVENEMENT_EPINGLER,
        ),
        (
            "pixels:monter-vue",
            "Pixels : monter la vue vide d'un onglet",
            EVENEMENT_MONTER_VUE,
        ),
    ] {
        let args = ecouter_args(commandes);
        let evenements = commandes.events().clone();
        let commande = Command::new(id, nom).with_callback(move || {
            let a: Value = args.borrow().clone();
            evenements.trigger(evenement, &a);
        });
        let _retrait = commandes.add_command(commande, "pixels");
    }
}

/// La pile de la feuille active du centre, ou la première pile du centre.
fn pile_du_centre(w: &Workspace) -> Option<fragment_core::ItemId> {
    let centre = w.root_split();
    w.active_leaf()
        .filter(|&a| w.arbre().get_root(a) == centre)
        .and_then(|a| w.arbre().parent(a))
        .filter(|&p| w.arbre().est_pile(p))
        .or_else(|| w.arbre().piles_sous(centre).first().copied())
}

/// Branche les événements de scène sur le Workspace : à appeler par l'assemblage (`fr-t1-binaire`),
/// avec le Workspace de la fenêtre. Les abonnements vivent autant que les [`EventRef`] rendus.
pub fn brancher_scene(
    commandes: &CommandManager,
    workspace: &Rc<RefCell<Workspace>>,
) -> Vec<EventRef> {
    let ws = workspace.clone();
    let epingler = commandes.on(EVENEMENT_EPINGLER, move |donnees| {
        let Some(a) = donnees.downcast_ref::<Value>() else {
            return;
        };
        let rang = nombre(a, "onglet").unwrap_or(0.0) as usize;
        if let Ok(mut w) = ws.try_borrow_mut()
            && let Some(pile) = pile_du_centre(&w)
        {
            actions::epingler(&mut w, pile, rang, true);
        }
    });
    let ws = workspace.clone();
    let monter = commandes.on(EVENEMENT_MONTER_VUE, move |donnees| {
        let Some(a) = donnees.downcast_ref::<Value>() else {
            return;
        };
        let rang = nombre(a, "onglet").unwrap_or(0.0) as usize;
        if let Ok(mut w) = ws.try_borrow_mut()
            && let Some(pile) = pile_du_centre(&w)
            && let Some(&leaf) = w.arbre().children(pile).get(rang)
        {
            // ★ C'est `set_view_state` qui monte la vue vide (en-tête, « Aucun fichier ouvert »).
            w.set_view_state(
                leaf,
                ViewState::new(fragment_core::workspace::vue::EMPTY_VIEW_TYPE),
                None,
            );
        }
    });
    vec![epingler, monter]
}
