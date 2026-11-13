//! Marge de lecture 1/4 – 3/4 (tâche `fr-t5-mode-marge`) : une affaire de mise en page de la
//! FEUILLE. Une note ou un PDF : la zone qui défile se réserve un quart à droite (40 % pour un PDF),
//! la colonne garde sa largeur et se recentre dans le reste, la marge droite s'élargit pour les
//! widgets.
//!
//! - [`etat`] : l'état par feuille, non persisté, et la réserve animée (90 ms) qu'il donne aux vues ;
//! - [`geometrie`] : ce que la réserve fait à la zone et à la colonne (pur) ;
//! - [`commandes`] : `workspace:open-margin` / `close-margin` / `toggle-margin` sur la feuille active,
//!   les deux premières liées aux balayages `SwipeRight` / `SwipeLeft` (`crate::clavier::balayage`) ;
//! - la commande de scène `pixels:marge` (`args: {on}`), qui fige l'état final sans transition.
//!
//! Le bouton de l'en-tête et l'application à la note et au PDF sont au montage (`fr-t5-montage-*`).

pub mod commandes;
pub mod etat;
pub mod geometrie;

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::{Command, CommandManager, EventRef, Workspace};
use serde_json::Value;

use crate::flottant::ecouter_args;

pub use commandes::enregistrer_commandes;
pub use etat::{Genre, MargeDeLecture, Reserve};
pub use geometrie::{MiseEnPage, largeur_utile_pdf, mise_en_page_note};

/// L'événement par lequel la scène ouvre ou ferme la marge de la feuille active : `{ "on": bool }`.
pub const EVENEMENT_MARGE: &str = "pixels:marge";

/// Enregistre la commande de scène `pixels:marge`.
///
/// ★ Une commande de scène n'a ni le Workspace ni l'état : elle publie [`EVENEMENT_MARGE`] avec ses
/// arguments ; [`brancher_scene`], appelé par l'assemblage, l'écoute.
pub fn commandes_de_scene(commandes: &CommandManager) {
    let args = ecouter_args(commandes);
    let evenements = commandes.events().clone();
    let commande = Command::new("pixels:marge", "Pixels : marge de lecture 1/4 – 3/4")
        .with_callback(move || {
            let a: Value = args.borrow().clone();
            evenements.trigger(EVENEMENT_MARGE, &a);
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// Branche [`EVENEMENT_MARGE`] sur la feuille active : l'état final est posé SANS transition (les
/// scènes figent l'état, animations coupées). `on` absent : ouverte.
pub fn brancher_scene(
    commandes: &CommandManager,
    workspace: &Rc<RefCell<Workspace>>,
    marge: &Rc<MargeDeLecture>,
) -> EventRef {
    let ws = Rc::downgrade(workspace);
    let m = Rc::clone(marge);
    commandes.on(EVENEMENT_MARGE, move |donnees| {
        let on = donnees
            .downcast_ref::<Value>()
            .and_then(|a| a.get("on"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let leaf = ws
            .upgrade()
            .and_then(|ws| ws.try_borrow().ok().and_then(|w| w.active_leaf()));
        if let Some(leaf) = leaf {
            m.poser_immediat(leaf, on);
        }
    })
}
