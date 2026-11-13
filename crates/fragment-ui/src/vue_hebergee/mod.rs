//! HostedView : feuille détachée, plugin tardif, detach, is-embedded ; DocWidget passe par elle.
//!
//! Coquille vide posée par `fr-t9-socle` ; la tâche `fr-t9-vue-hebergee` la remplit.

use fragment_core::CommandManager;

/// Enregistre les commandes `pixels:*` du module (aucune pour l'instant).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
