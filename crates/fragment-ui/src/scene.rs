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
// Socle T5-T7 (fr-p3-socle-t5t7).
use crate::{
    agent, agent_barre, agent_controles, agent_reglages, agent_trace, calques, chat_barre_laterale,
    chat_citation, chat_defilement, chat_fil, chat_pieces, chat_post, chat_rail, chat_surface,
    chat_vue, doc_widget, grappe, lien_ancre, menu_calques, mode_marge, orbe, surlignages,
    widget_couche,
};
// Socle T9 (fr-t9-socle).
use crate::{chat_origine, embed, html_surface, md_embed, vue_hebergee, vue_html, vue_image};

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
    calques::commandes_de_scene(commandes);
    widget_couche::commandes_de_scene(commandes);
    surlignages::commandes_de_scene(commandes);
    doc_widget::commandes_de_scene(commandes);
    grappe::commandes_de_scene(commandes);
    lien_ancre::commandes_de_scene(commandes);
    mode_marge::commandes_de_scene(commandes);
    menu_calques::commandes_de_scene(commandes);
    chat_fil::commandes_de_scene(commandes);
    chat_surface::commandes_de_scene(commandes);
    chat_defilement::commandes_de_scene(commandes);
    chat_rail::commandes_de_scene(commandes);
    chat_barre_laterale::commandes_de_scene(commandes);
    chat_pieces::commandes_de_scene(commandes);
    chat_citation::commandes_de_scene(commandes);
    chat_post::commandes_de_scene(commandes);
    chat_vue::commandes_de_scene(commandes);
    agent_trace::commandes_de_scene(commandes);
    orbe::commandes_de_scene(commandes);
    agent_controles::commandes_de_scene(commandes);
    agent_barre::commandes_de_scene(commandes);
    agent_reglages::commandes_de_scene(commandes);
    agent::commandes_de_scene(commandes);
    vue_hebergee::commandes_de_scene(commandes);
    embed::commandes_de_scene(commandes);
    md_embed::commandes_de_scene(commandes);
    vue_image::commandes_de_scene(commandes);
    vue_html::commandes_de_scene(commandes);
    html_surface::commandes_de_scene(commandes);
    chat_origine::commandes_de_scene(commandes);
}
