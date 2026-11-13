// Port de app/src/ui/views/markdown/MarkdownView.ts (le menu contextuel de l'éditeur : entrées
// grisées selon la sélection et la lecture seule, exécutées sur l'éditeur visé).
//! Le menu contextuel d'une note : le modèle de `fragment_markdown::commandes::menu`, rempli par
//! l'état de la vue, et son exécution sur le document.

use fragment_markdown::commandes::menu::{
    ActionMenu, ContexteMenu, DemandeHote, ItemMenu, executer_action, modele_menu_editeur,
};
use fragment_text::TextSurface as _;

use super::vue::VueMarkdown;

/// Ce qui grise les entrées : la sélection, la lecture seule. L'éditeur principal n'est jamais
/// imbriqué (une cellule de tableau l'est, c'est son propre menu).
pub fn contexte_menu(vue: &VueMarkdown) -> ContexteMenu {
    let sel = vue.editeur_interne().get_selection();
    ContexteMenu {
        selection_vide: sel.from == sel.to,
        lecture_seule: vue.lecture_seule(),
        imbrique: false,
    }
}

/// Les entrées du menu contextuel de la note, dans l'ordre et par section.
pub fn menu_contextuel(vue: &VueMarkdown) -> Vec<ItemMenu> {
    modele_menu_editeur(contexte_menu(vue))
}

/// Une action ne modifie-t-elle pas le document ? (Seules celles-là passent en lecture seule.)
pub fn action_sans_ecriture(action: ActionMenu) -> bool {
    matches!(
        action,
        ActionMenu::Copier | ActionMenu::ToutSelectionner | ActionMenu::Aucune
    )
}

/// Exécute une entrée sur la note ; rend ce que l'hôte doit faire lui-même (presse-papiers).
/// ★ En lecture seule, une action qui écrirait est ignorée, même si le menu l'a laissée passer.
pub fn executer_entree(
    vue: &VueMarkdown,
    action: ActionMenu,
    texte_brut: &str,
) -> Option<DemandeHote> {
    if vue.lecture_seule() && !action_sans_ecriture(action) {
        return None;
    }
    executer_action(&vue.editeur_interne(), action, texte_brut)
}
