//! L'app GPUI : shell du workspace sur gpui-kit, vues, calques, barres (phase 3).
//!
//! Le squelette (tâche `fr-t1-squelette`) pose le socle que tout le rendu de T1 lit :
//! - [`echelle`] : le zoom de l'application et `u(n)`, point d'entrée de TOUTE dimension de rendu ;
//! - [`theme_global`] : le [`fragment_theme::Theme`] courant, global GPUI ;
//! - [`polices`] et [`icones`] : DM Sans embarquée et registre d'icônes Lucide ;
//! - [`scene`] : l'appel des `commandes_de_scene` de chaque module de rendu.
//!
//! Les autres modules sont des coquilles vides, une par tâche aval : elles les remplissent
//! sans toucher à ce fichier.

pub mod boites;
pub mod echelle;
pub mod flottant;
pub mod icones;
pub mod polices;
pub mod rendu;
pub mod scene;
pub mod theme_global;

pub mod menu;
pub mod modal;
pub mod notice;
pub mod palette;
pub mod suggest;
pub mod toolbar;
pub mod tooltip;

pub mod clavier;
pub mod docks;
pub mod entete;
pub mod glisser;
pub mod onglets;
pub mod reglages;
pub mod shell;
pub mod splits;

// Socle T2-T4-T11 (fr-p3-socle) : coquilles vides, une par tâche aval.
pub mod coffres;
pub mod editeur;
pub mod explorateur;
pub mod lecture;
pub mod md_blocs;
pub mod md_html;
pub mod md_inline;
pub mod md_proprietes;
pub mod md_repli;
pub mod md_tableau;
pub mod pdf_barre;
pub mod pdf_laterale;
pub mod pdf_pages;
pub mod pdf_recherche;
pub mod pdf_texte;
pub mod themes;
pub mod tree_item;
pub mod vue_fichier;
pub mod vue_markdown;
pub mod vue_pdf;

// Socle T5-T7 (fr-p3-socle-t5t7) : coquilles vides, une par tâche aval.
pub mod agent;
pub mod agent_barre;
pub mod agent_controles;
pub mod agent_reglages;
pub mod agent_trace;
// Socle T6 (fr-t6-socle).
pub mod artefacts;
pub mod calques;
pub mod chat_barre_laterale;
pub mod chat_citation;
pub mod chat_defilement;
pub mod chat_fil;
pub mod chat_pieces;
pub mod chat_post;
pub mod chat_rail;
pub mod chat_surface;
pub mod chat_vue;
pub mod doc_widget;
pub mod grappe;
pub mod lien_ancre;
pub mod menu_calques;
pub mod mode_marge;
pub mod orbe;
pub mod surlignages;
pub mod widget_couche;

/// Les fichiers de `assets/` embarqués (écrit par `build.rs`).
pub(crate) mod embarque {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));
}
