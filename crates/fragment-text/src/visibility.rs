// Port de app/src/ui/core/layers/visibility.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Une seule règle pour répondre à « cette position est-elle dessinée ? ».
//
//  ★ Elle vit ici car ses trois consommateurs (markers, widgets, surface de saisie)
//    doivent donner la même réponse : avec trois copies, un correctif finirait tôt ou
//    tard par en oublier une.
//
//  La subtilité tient en une phrase : « pas visible » a deux sens opposés.
//  Hors écran, l'objet existe et il faut le laisser où il est ; masqué, il faut
//  l'effacer. Confondre les deux donne soit des objets qui disparaissent au
//  défilement, soit des objets qui s'empilent sur le titre d'un bloc replié.
//
//  ★ Le TS range ce fichier sous `layers/` ; il ne dépend que de `DocumentSurface`
//    et `range_highlights` (de cette crate) l'appelle : il vit donc dans
//    fragment-text, fragment-layers le réutilise (le cœur ne dépend pas de ses
//    calques).
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::DocumentSurface;

/// L'état d'une position vis-à-vis du rendu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PosVisibility {
    /// Dessinée. `coords_at_pos` dit vrai, on positionne.
    Rendered,
    /// Dans le viewport mais NON dessinée (repliée). `coords_at_pos` répond quand
    /// même, mais en rendant le point dessiné le plus proche — c'est-à-dire autre
    /// chose. On MASQUE.
    Hidden,
    /// Hors du viewport DE L'ÉDITEUR (virtualisation). Aucune coordonnée fiable.
    /// Un marker n'est pas dessiné ; un widget de calque est détruit s'il est
    /// déclaré, masqué sinon — son ancre, elle, suit les éditions.
    Offscreen,
}

pub fn pos_visibility(editor: &dyn DocumentSurface, pos: usize) -> PosVisibility {
    let viewport = editor.viewport_range();
    if pos < viewport.from || pos > viewport.to {
        return PosVisibility::Offscreen;
    }
    for r in editor.rendered_ranges() {
        // Bornes INCLUSES des deux côtés, et c'est voulu : un repli commence à la
        // fin de sa ligne-titre, donc une ancre posée pile à cette frontière
        // appartient à la ligne restée visible. L'exclure ferait clignoter les
        // annotations du titre quand on replie la section qu'il porte.
        if pos >= r.from && pos <= r.to {
            return PosVisibility::Rendered;
        }
    }
    PosVisibility::Hidden
}
