// Port de app/src/ui/views/chat/scroll/ (ScrollController.ts, jump.ts, announcer.ts ; le modèle
// followModel.ts est dans fragment-chat).
//! Défilement du fil : ne JAMAIS déplacer le lecteur contre son intention (tâche
//! `fr-t7-defilement`).
//!
//! - [`controleur`] : le contrôleur PUR (`free` / `turn` / `live`, réserve, ancre de lecture,
//!   position de la vue, tour voisin) sur des mesures ;
//! - [`defileur`] : le défileur GPUI ([`Defileur`]) qui mesure, écoute les gestes et écrit ;
//! - [`pastille`] : la pastille « ↓ » (Réponse en cours, N nouveaux messages, Aller au plus récent)
//!   et le suivi des lus qui la nourrit ;
//! - [`annonceur`] : l'annonceur des lecteurs d'écran (au plus toutes les 2 s) et la règle « une
//!   mise à jour du fil par image » ;
//! - la commande de scène `pixels:chat-defiler` (`args: {to: "fin" | <id de message>}`).
//!
//! Le montage dans la vue chat (envoi, fin de tour, Ctrl+F, rail, lien `#m5`) est `fr-t7-vue`.

pub mod annonceur;
pub mod controleur;
pub mod defileur;
pub mod pastille;

pub use annonceur::{Annonces, Annonceur, UneParImage};
pub use controleur::{ControleurDefilement, Hote, Mesures, Position, contexte_px};
pub use defileur::{Defileur, MesureColonne};
pub use pastille::{EtatPastille, SuiviLus, etat_pastille, libelle_pastille, pastille};

use fragment_core::{Command, CommandManager};
use gpui_kit::Context;
use serde_json::Value;

use crate::flottant::{chaine, ecouter_args};

/// L'événement par lequel la scène fait défiler la vue chat active : `{ "to": "fin" | <id> }`.
pub const EVENEMENT_DEFILER: &str = "pixels:chat-defiler";

/// Enregistre la commande de scène `pixels:chat-defiler`.
///
/// Une commande de scène n'a pas la vue : elle publie [`EVENEMENT_DEFILER`] avec ses arguments ;
/// la vue chat (`fr-t7-vue`) l'écoute et appelle [`appliquer_scene`] sur son défileur.
pub fn commandes_de_scene(commandes: &CommandManager) {
    let args = ecouter_args(commandes);
    let evenements = commandes.events().clone();
    let commande = Command::new("pixels:chat-defiler", "Pixels : faire défiler le chat")
        .with_callback(move || {
            let a: Value = args.borrow().clone();
            evenements.trigger(EVENEMENT_DEFILER, &a);
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// Applique les arguments de la scène : `to` absent ou `"fin"` → le plus récent ; sinon l'id d'un
/// message, placé en haut.
pub fn appliquer_scene(d: &mut Defileur, args: &Value, cx: &mut Context<Defileur>) {
    match chaine(args, "to") {
        None | Some("fin") => d.aller_au_plus_recent(cx),
        Some(id) => d.reveler(id, cx),
    }
}
