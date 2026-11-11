//! Modèle PUR de la vue PDF : aucune vue, aucun moteur, aucune horloge système.
//!
//! La vue montée (fr-t4-pages, fr-t4-barre, fr-t4-vue-pdf) pilote ce modèle avec des
//! événements (défilement, molette, taille de la zone, touches, temps écoulé) et lit
//! en retour l'état (échelle, défilement, page courante, champ de page) et un
//! [`PlanRendu`] : les pages à peindre et à libérer. Rien ici ne touche au rendu.
//!
//! Le temps vient de l'appelant : il appelle `VueModele::avancer(maintenant_ms)` avant
//! chaque événement (l'horloge du modèle est celle du dernier appel) et à l'échéance
//! donnée par `prochaine_echeance`, qui déclenche la fin de geste et le recalage.
//!
//! Découpage :
//! - [`vue`] : [`VueModele`], le cœur (géométrie, zoom, défilement, modes, gestes) ;
//! - [`rendu`] : le plan de rendu et l'état de chaque page ;
//! - [`pixels`] : la règle d'un pixel de raster par pixel physique ;
//! - [`champ`] : le champ « page N of M » ;
//! - [`etat`] : position de lecture et état éphémère.

pub mod champ;
pub mod etat;
pub mod pixels;
pub mod rendu;
pub mod vue;

pub use champ::{ChampPage, analyser_entier};
pub use etat::{EtatEphemere, PositionLecture, mode_depuis_texte, mode_vers_texte};
pub use pixels::{Pixels, taille_physique};
pub use rendu::{EtatPage, Peinture, PlanRendu};
pub use vue::{
    DELAI_FIN_DE_GESTE, MARGE_RENDU, MODES_ZOOM, Point, Raccourci, Reprise, ResultatMolette,
    SENSIBILITE_MOLETTE, VueModele, Zone, raccourci_pour,
};
