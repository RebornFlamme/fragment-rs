//! Le clavier des listes et des tâches : des transformations PURES (texte, arbre, sélection) → transaction.
//!
//! Trois morceaux :
//!   - [`clavier`] : Entrée (continue une puce, une tâche, une numérotée ; sort d'un item vide, sans
//!     jamais insérer de ligne vide), Retour arrière (après `- [ ] ` rend `- `), Tab et Maj+Tab ;
//!   - [`case`] : la bascule d'une case à cocher (un seul changement, texte immobile) et le relevé
//!     des cases d'un document (y compris `- [ ]` sans texte, que l'arbre ne voit pas comme tâche) ;
//!   - `contexte` : le « contexte de marqueurs » d'une position (liste, citation), lu dans l'arbre.
//!
//! ★ Aucune de ces fonctions ne touche un éditeur : elles rendent un `EditorTransactionSpec` que la
//!   vue applique en UNE transaction (donc un seul « annuler »). Une touche qui ne s'applique pas rend
//!   `None` : la vue passe alors au comportement par défaut (nouvelle ligne, effacement).
//! ★ Positions : unités de code UTF-16 (ADR-004), comme partout dans `fragment-markdown`.

pub mod case;
pub mod clavier;
mod contexte;

pub use case::{CaseTache, basculer_case, cases_de_taches};
pub use clavier::{
    continuer_liste, desindenter, entree, indenter, retirer_case, retour_arriere,
    sortir_de_tache_vide, supprimer_marqueur_arriere,
};
