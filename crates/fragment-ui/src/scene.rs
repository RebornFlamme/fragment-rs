//! Les scènes de pixels de `fragment-ui` : chaque module de rendu expose sa fonction
//! `commandes_de_scene`, appelée d'ici. Une tâche de rendu n'ajoute donc aucune ligne à ce fichier :
//! elle remplit la fonction de SON module (docs/tranches/T1.md, choix 6).

use fragment_core::CommandManager;

use crate::{
    clavier, docks, entete, glisser, menu, modal, notice, onglets, palette, reglages, shell,
    splits, suggest, toolbar, tooltip,
};
// Socle T2-T4-T11 (fr-p3-socle).
use crate::{
    coffres, editeur, explorateur, lecture, md_blocs, md_html, md_inline, md_proprietes, md_repli,
    md_tableau, pdf_barre, pdf_laterale, pdf_pages, pdf_recherche, pdf_texte, themes, tree_item,
    vue_fichier, vue_markdown, vue_pdf,
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
    tree_item::commandes_de_scene(commandes);
    explorateur::commandes_de_scene(commandes);
    coffres::commandes_de_scene(commandes);
    lecture::commandes_de_scene(commandes);
    vue_fichier::commandes_de_scene(commandes);
    editeur::commandes_de_scene(commandes);
    md_inline::commandes_de_scene(commandes);
    md_blocs::commandes_de_scene(commandes);
    md_repli::commandes_de_scene(commandes);
    md_tableau::commandes_de_scene(commandes);
    md_proprietes::commandes_de_scene(commandes);
    md_html::commandes_de_scene(commandes);
    vue_markdown::commandes_de_scene(commandes);
    pdf_pages::commandes_de_scene(commandes);
    pdf_barre::commandes_de_scene(commandes);
    pdf_laterale::commandes_de_scene(commandes);
    pdf_texte::commandes_de_scene(commandes);
    pdf_recherche::commandes_de_scene(commandes);
    vue_pdf::commandes_de_scene(commandes);
    themes::commandes_de_scene(commandes);
}
