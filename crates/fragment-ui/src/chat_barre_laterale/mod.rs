//! Barre des conversations du cowork et accueil (tâche `fr-t7-barre-laterale`).
//!
//! - [`modele`] : le modèle pur (liste filtrée sans accents, groupée par jour, signature) ;
//! - [`horloge`] : l'horloge injectable (murale, ou figée par `pixels:horloge`) ;
//! - [`barre`] : la barre ([`BarreLaterale`]) : recherche, « Nouvelle conversation », liste ;
//! - [`accueil`] : l'icône du fournisseur et la salutation selon l'heure et le nom.
//!
//! Le cowork (grille, cellule de 0 à 260 px, repli, seuil de 720 px), le centrage du composer et
//! `data-empty` sont à la vue chat (`fr-t7-vue`).

pub mod accueil;
pub mod barre;
pub mod horloge;
pub mod modele;

pub use accueil::{FAMILLE_SERIF, accueil, corps_salutation, texte_accueil};
pub use barre::{BarreLaterale, EvenementBarre};
pub use horloge::{Horloge, horloge_depuis_args, instant_local};
pub use modele::{EtatBarre, LARGEUR_BARRE, Ligne, message_vide, signature, titre_affiche};

use fragment_core::{Command, CommandManager};
use serde_json::Value;

use crate::flottant::ecouter_args;

/// L'événement par lequel la scène pose l'horloge du chat : les arguments de `pixels:horloge`
/// (voir [`horloge_depuis_args`]).
pub const EVENEMENT_HORLOGE: &str = "pixels:horloge";

/// Enregistre la commande de scène `pixels:horloge`.
///
/// Une commande de scène n'a pas la vue : elle publie [`EVENEMENT_HORLOGE`] avec ses arguments ;
/// la vue chat (`fr-t7-vue`) l'écoute et appelle [`horloge_depuis_args`] puis
/// [`BarreLaterale::definir_horloge`].
pub fn commandes_de_scene(commandes: &CommandManager) {
    let args = ecouter_args(commandes);
    let evenements = commandes.events().clone();
    let commande = Command::new("pixels:horloge", "Pixels : figer l'horloge du chat")
        .with_callback(move || {
            let a: Value = args.borrow().clone();
            evenements.trigger(EVENEMENT_HORLOGE, &a);
        });
    let _retrait = commandes.add_command(commande, "pixels");
}
