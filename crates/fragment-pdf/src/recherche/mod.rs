//! Modèle PUR de la recherche dans un PDF (sans moteur, sans vue).
//!
//! Trois couches, du bas vers le haut :
//! - [`normalisation`] : le texte d'une page tel que la recherche le voit (ligatures,
//!   diacritiques, blancs, coupures de fin de ligne) et la table qui ramène ses offsets
//!   dans le texte brut ;
//! - [`correspondance`] : une requête contre ce texte (casse, mots entiers, diacritiques) ;
//! - [`controleur`] : l'état de la barre — correspondance courante, navigation cyclique
//!   Entrée / Maj+Entrée, compte « n of m », effacement.
//!
//! Tous les offsets publics sont en unités UTF-16 (ADR-004). Pour passer du texte BRUT
//! de la page au texte NORMALISÉ de notre couche, voir
//! [`crate::pdf_find_offsets`].

pub mod controleur;
pub mod correspondance;
pub mod normalisation;

pub use controleur::{
    Correspondance, DELAI_FRAPPE_MS, EtatRecherche, EtatTrouve, ItemTexte, PageRecherche,
    Recherche, ResultatRecherche, Surlignage, TypeRecherche, texte_compteur,
};
pub use correspondance::OptionsRecherche;
