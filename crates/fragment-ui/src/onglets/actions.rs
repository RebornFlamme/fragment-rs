// Port de app/src/ui/core/workspace/WorkspaceTabs.ts (les gestionnaires de la barre :
// `rebuildTabHeaders`, `showTabListMenu`, `closeAll`) et de WorkspaceLeaf.ts (l'activation au
// pointeur de la feuille).
//
//! Ce que font les gestes sur la pile d'onglets, SANS GPUI.
//!
//! Le rendu ne contient aucune règle : chaque gestionnaire de souris appelle une de ces
//! fonctions, qui ne parlent qu'au [`Workspace`]. C'est ce qui se teste en headless (les quatre
//! cas de `activeleaf-tracking.spec.ts`), et c'est aussi ce que la scène de pixels rejoue.

use fragment_core::{ItemId, Workspace};

/// L'APPUI sur un en-tête d'onglet : montrer ET activer l'onglet `index` de `pile`, dès l'appui.
///
/// ★ Pas au `click` : un clic humain bouge souvent de quelques pixels entre l'appui et le
/// relâchement, franchit le seuil du glisser et avalait le `click` : l'onglet ne devenait ni
/// visible ni actif. L'appui, lui, est fiable.
pub fn presser_entete(ws: &mut Workspace, pile: ItemId, index: usize) {
    ws.activer_onglet(pile, index);
}

/// Un appui (ou un focus) N'IMPORTE OÙ dans une feuille la rend active, pas seulement son
/// onglet : revenir écrire dans la note après le chat du dock lui rend ses raccourcis.
pub fn toucher_feuille(ws: &mut Workspace, leaf: ItemId) {
    ws.activer_par_contact(leaf);
}

/// La croix d'un onglet (ou « Fermer » du menu contextuel) : détruit la feuille. La fermeture de
/// l'ACTIVE passe le relais à l'onglet redevenu visible (jamais `None` tant qu'il en reste un).
pub fn fermer_onglet(ws: &mut Workspace, leaf: ItemId) {
    ws.detach(leaf);
}

/// Le « + » : une feuille fraîche à la fin de la pile, rendue ACTIVE.
///
/// ★ `set_active_leaf` est indispensable : sans lui, « ouvrir un fichier » continuerait de viser
/// l'ANCIENNE feuille active.
pub fn nouvel_onglet(ws: &mut Workspace, pile: ItemId) -> Option<ItemId> {
    let leaf = ws.create_leaf_in_parent(pile, None).ok()?;
    ws.set_active_leaf(Some(leaf));
    Some(leaf)
}

/// « Fermer tout » du menu de la pile : ferme chaque onglet. Le dernier déclenche la règle de la
/// pile vidée (le pane se ferme s'il est dans un split, sinon un onglet vide s'ouvre).
pub fn fermer_tout(ws: &mut Workspace, pile: ItemId) {
    let feuilles: Vec<ItemId> = ws.arbre().children(pile).to_vec();
    for f in feuilles {
        ws.detach(f);
    }
}

/// Le bouton qui bascule le dock DROIT, dans la barre du coin haut-droit.
///
/// ★ « Ouvert » = déplié ET non vide. Le dock par défaut est déplié mais VIDE (largeur 0 par le
/// CSS) : on le traite comme fermé, donc le premier clic lui donne une pile puis le déplie.
pub fn basculer_dock_droit(ws: &mut Workspace) {
    let droit = ws.right_split();
    let ouvert = ws.arbre().dock_ouvert(droit) && !ws.arbre().children(droit).is_empty();
    if ouvert {
        ws.collapse(droit);
    } else {
        // Le dock droit existe toujours ; seule sa pile peut manquer.
        let _ = ws.ensure_right_pile();
        ws.expand(droit);
    }
}

/// Une ligne du menu de la pile (le chevron) : un onglet, dans l'ordre de la barre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneOnglet {
    /// La feuille.
    pub leaf: ItemId,
    /// Le rang dans la pile.
    pub index: usize,
    /// Le titre de l'onglet.
    pub titre: String,
    /// L'icône de la vue.
    pub icone: String,
    /// L'onglet visible de la pile (coche).
    pub visible: bool,
}

/// Les onglets de `pile`, avec titre et icône : ce qu'affichent la barre et le menu du chevron.
pub fn lignes_de_la_pile(ws: &Workspace, pile: ItemId) -> Vec<LigneOnglet> {
    let visible = ws.arbre().active_index(pile);
    ws.arbre()
        .children(pile)
        .iter()
        .enumerate()
        .filter_map(|(index, &leaf)| {
            let f = ws.leaf(leaf)?;
            Some(LigneOnglet {
                leaf,
                index,
                titre: f.get_display_text(),
                icone: f.get_icon(),
                visible: visible == Some(index),
            })
        })
        .collect()
}

/// Montre l'onglet `index` depuis le menu du chevron : `select_tab`, qui tient l'invariant
/// « la feuille active d'une pile est son onglet visible ».
pub fn montrer_onglet(ws: &mut Workspace, pile: ItemId, index: usize) {
    ws.select_tab(pile, index);
}

/// Épingle (ou désépingle) l'onglet `index` de `pile` : ★ Fragment n'a AUCUNE marque visuelle
/// d'épinglage dans la barre (ni classe ni icône) ; l'état ne sert qu'à `get_leaf` et au layout.
pub fn epingler(ws: &mut Workspace, pile: ItemId, index: usize, epingle: bool) {
    if let Some(&leaf) = ws.arbre().children(pile).get(index) {
        ws.set_pinned(leaf, epingle);
    }
}
