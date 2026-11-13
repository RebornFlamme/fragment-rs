//! La barre de l'agent (tâche `fr-t7-barre-agent`) : un LANCEUR en bas de chaque document.
//!
//! - [`modele`] : l'état (repliée / ouverte), la conversation « modèle », l'écran partagé réutilisé et
//!   Entrée → une conversation neuve (pur, sans GPUI) ;
//! - [`hote`] : les branchements réels (le workspace du cœur, le fournisseur agent) ;
//! - [`registre`] : les barres des vues, le raccourci Mod+J, la ligne du menu « Calques » ;
//! - [`rendu`] : le widget GPUI posé par l'API des widgets (bande viewport depuis le bas, pastille,
//!   boutons du champ).
//!
//! Le montage dans l'app (calque « Agent », vue chat hébergée, réglages persistés) est
//! `fr-t7-montage`.

pub mod hote;
pub mod modele;
pub mod registre;
pub mod rendu;

pub use hote::HoteWorkspace;
pub use modele::{
    BarreAgent, CALQUE_AGENT, Cible, ContexteBarre, Crochets, EtatBarre, FournisseurBarre,
    GOUTTIERE_CHAMP, HoteBarre, LARGEUR_CHAMP, Reglages,
};
pub use registre::{
    Barres, COMMANDE_BASCULER, ID_LIGNE_MENU, PLUGIN_AGENT, RemettreCalque, TITRE_LIGNE,
};
pub use rendu::{
    BarrePosee, ChampBarre, ESPACE_BAS, ID_PASTILLE, LIBELLE_PASTILLE, SLOT_CIBLE, SLOT_REPLIER,
    largeur_du_champ, poser_barre, slots_barre,
};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:agent_barre` : vide jusqu'à la tâche propriétaire.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
