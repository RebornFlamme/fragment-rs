//! Les commandes `editor:*`, le modèle des menus de l'éditeur et l'insertion de texte.
//!
//! Trois morceaux, tous PURS (aucun moteur de rendu, aucune vue) :
//!   - [`actions`] : la table `editor:*` → transformation, et son enregistrement ;
//!   - [`menu`] : le modèle du menu contextuel (sections, entrées, sous-menus, grisées) ;
//!   - [`insertion`] : insérer du texte à la sélection sans casser un tableau GFM.

pub mod actions;
pub mod insertion;
pub mod menu;
