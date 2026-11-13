//! Vue chat (tâche `fr-t7-vue`) : modes (page, panneau, ancrée, embarquée, barre), cowork, en-tête,
//! menu ⋯, carte d'origine, colonne de la note, plugin chat.
//!
//! - [`geometrie`] : la colonne de la note, la grille du cowork, les transitions (pur) ;
//! - [`modele`] : l'état de la vue et ses gestes, sans GPUI ;
//! - [`vue`] : [`VueChat`], la vue du cœur (type, titre, adresse, état, ⋯) ;
//! - [`contenu`] : [`ContenuChat`], le contenu GPUI qui assemble les briques de T7 ;
//! - [`plugin`] : la vue `chat`, le schéma `chat:` et les commandes `chat:open`, `chat:new`,
//!   `chat:new-tab` ;
//! - [`fournisseur_test`] : le fournisseur de test (conversations en mémoire, réponse scriptée) ;
//! - la commande de scène `pixels:chat` (`args: {mode, conversation}`).
//!
//! Le montage dans l'app (fournisseur agent, chat comme document, surface de calques) est
//! `fr-t7-montage`.

pub mod contenu;
pub mod fournisseur_test;
pub mod geometrie;
pub mod modele;
pub mod plugin;
pub mod vue;

pub use contenu::{ContenuChat, PART_ORIGINE};
pub use fournisseur_test::{FournisseurDeTest, ID_FOURNISSEUR_TEST};
pub use geometrie::{
    BARRE_DEFILEMENT, Colonne, DEBUT_TEXTE, Disposition, Entrees, FIN_TEXTE, LARGEUR_BARRE_OUVERTE,
    SEUIL_COWORK, Transition, colonne, disposition,
};
pub use modele::{
    ActionChat, CarteOrigine, ContexteChat, EtatChat, Presentation, TYPE_VUE_CHAT,
    nouvelle_conversation, ouvrir_conversation, ouvrir_origine, supprimer_conversation,
};
pub use plugin::{
    COMMANDE_NOUVELLE, COMMANDE_NOUVELLE_ONGLET, COMMANDE_OUVRIR, DocumentsChat, PLUGIN_CHAT,
    enregistrer, registre_fournisseurs,
};
pub use vue::{LiensChat, ToucheChat, VueChat, fabrique};

use fragment_core::{Command, CommandManager};
use serde_json::Value;

use crate::flottant::ecouter_args;

/// L'événement par lequel la scène ouvre la vue chat : `{ "mode": "page" | "panel", "conversation": id }`.
pub const EVENEMENT_CHAT: &str = "pixels:chat";

/// Enregistre la commande de scène `pixels:chat`.
///
/// Une commande de scène n'a pas le Workspace : elle publie [`EVENEMENT_CHAT`] avec ses arguments ;
/// l'assemblage (`fr-t7-montage`) l'écoute et ouvre la conversation dans le mode demandé.
pub fn commandes_de_scene(commandes: &CommandManager) {
    let args = ecouter_args(commandes);
    let evenements = commandes.events().clone();
    let commande =
        Command::new("pixels:chat", "Pixels : ouvrir la vue chat").with_callback(move || {
            let a: Value = args.borrow().clone();
            evenements.trigger(EVENEMENT_CHAT, &a);
        });
    let _retrait = commandes.add_command(commande, "pixels");
}
