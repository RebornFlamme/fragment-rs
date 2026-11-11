//! Le modèle PUR des blocs du live preview et du repli (fr-t3-blocs-modele).
//!
//! Fragment les confie à des `StateField` CodeMirror (`mathBlockField`, `embedField`, `tableField`,
//! `propertiesField`, `htmlBlockField`) et au `foldService` ; ici le même calcul rend des DONNÉES,
//! testées sans GPUI, que la surface d'édition traduit pour le fork (ADR-006, `set_row_model`).
//!
//! - [`calculer`] : (texte, arbre, curseur, contexte) → les [`BlocCalcule`] : maths `$$`, embed seul
//!   sur sa ligne, tableau, front matter valide, bloc HTML, commentaire, bloc de code (et sa langue),
//!   séparateur ; chacun avec sa plage source et son [`ModeBloc`] (remplacé / source + rendu dessous /
//!   source seule) ;
//! - [`integrer`] : les blocs et le repli → le `ModeleRangees` (lignes masquées, hauteurs dictées,
//!   blocs) que `ModeleRangees::vers_fork` traduit pour `set_row_model` ;
//! - [`repli`] : plages repliables (titres imbriqués, listes, indentation, Properties), état de repli
//!   remappé aux éditions, filtre qui chasse le curseur hors d'une source cachée.
//!
//! ★ Le code et le séparateur sont dans la liste (un hôte de blocs veut leur plage et leur langue)
//!   mais ne changent PAS le modèle de rangées : leur rendu est celui de `decorations` (badge, trait).

mod calcul;
mod modele;
mod rangees;
pub mod repli;

pub use calcul::{calculer, plage_proprietes};
pub use modele::{BlocCalcule, ContenuBloc, ModeBloc};
pub use rangees::integrer;
pub use repli::{AnalyseRepli, Edition, EtatRepli, NatureRepli, PlageRepli, chasser_curseur};
