//! Les scènes de pixels de `fragment-ui` : chaque module de rendu expose sa fonction
//! `commandes_de_scene`, appelée d'ici. Une tâche de rendu n'ajoute donc aucune ligne à ce fichier :
//! elle remplit la fonction de SON module (docs/tranches/T1.md, choix 6).

use fragment_core::CommandManager;

use crate::{
    clavier, docks, entete, glisser, menu, modal, notice, onglets, palette, reglages, shell,
    splits, suggest, toolbar, tooltip,
};

/// Enregistre dans `commandes` toutes les commandes `pixels:*` des modules de rendu.
pub fn commandes_de_scene(commandes: &CommandManager) {
    notice::commandes_de_scene(commandes);
    menu::commandes_de_scene(commandes);
    tooltip::commandes_de_scene(commandes);
    toolbar::commandes_de_scene(commandes);
    modal::commandes_de_scene(commandes);
    suggest::commandes_de_scene(commandes);
    palette::commandes_de_scene(commandes);
    onglets::commandes_de_scene(commandes);
    entete::commandes_de_scene(commandes);
    splits::commandes_de_scene(commandes);
    docks::commandes_de_scene(commandes);
    glisser::commandes_de_scene(commandes);
    reglages::commandes_de_scene(commandes);
    clavier::commandes_de_scene(commandes);
    shell::commandes_de_scene(commandes);
}
