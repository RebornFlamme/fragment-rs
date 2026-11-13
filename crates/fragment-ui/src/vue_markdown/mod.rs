//! La vue Markdown montée (tâche `fr-t3-vue`) : la vue `markdown` d'une feuille.
//!
//! - [`vue`] : [`VueMarkdown`], la `TextFileView` des notes (lecture et écriture au coffre,
//!   sauvegarde différée, relecture du disque, lecture seule sérialisée, état éphémère) ;
//! - [`note`] : le document partagé et sa façade `Editor` (commandes, menu, collage) ;
//! - [`relecture`] : l'écart minimal d'une relecture, et où le curseur atterrit ;
//! - [`position`] : la position de lecture `{line, dy}` et sa reprise jusqu'à stabilité ;
//! - [`menu`] : le menu contextuel de l'éditeur ;
//! - [`collage`] : le collage d'image ;
//! - [`interaction`] : le menu dessiné, le collage au presse-papiers, `paste-plain`, `follow-link` ;
//! - [`commandes`] : l'enregistrement (vue, extension `md`, commandes `editor:*`) ;
//! - [`montage`] : l'élément éditeur du live preview monté dans le contenu de la vue.

pub mod collage;
pub mod commandes;
pub mod interaction;
pub mod menu;
pub mod montage;
pub mod note;
pub mod position;
pub mod relecture;
pub mod vue;

pub use commandes::enregistrer;
pub use montage::contenu;
pub use note::{EditeurNote, Note, NotePartagee};
pub use position::{CarteNote, PoseNote, PositionNote, lire_position};
pub use vue::{
    CLE_LECTURE_SEULE, ContexteMarkdown, Horloge, Liens, MARKDOWN_VIEW_TYPE, VueMarkdown,
    horloge_systeme,
};

use fragment_core::CommandManager;

/// La taille minimale de la fenêtre (`minWidth` / `minHeight` de la fenêtre principale) : sous
/// 600 × 400, la note n'a plus de colonne de lecture utilisable.
pub const TAILLE_MIN_FENETRE: (f32, f32) = (600.0, 400.0);

/// Enregistre les commandes de scène `pixels:vue_markdown` : aucune, les scènes de la note
/// passent par le coffre monté (la vue s'ouvre comme dans l'app).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
