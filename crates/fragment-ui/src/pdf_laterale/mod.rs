//! La barre latérale de la vue PDF (tâche `fr-t4-laterale-ui`) : vignettes OU table des matières,
//! poignée de largeur. Le modèle (mode, largeur, virtualisation, arbre) est celui de
//! `fragment_pdf::laterale::BarreLaterale` ; ici, seul le rendu et le fil de peinture des vignettes.
//!
//! - [`vue`] : [`LateralePdf`], l'entité GPUI (état, fil de rendu, événements) ;
//! - [`rendu`] : le dessin (vignettes peintes, arbre en `tree_item`, poignée).
//!
//! La barre ne connaît ni la vue PDF ni la barre d'outils : la vue montée (`fr-t4-vue-pdf`) lui
//! donne le fil de rendu et la table des matières à la construction, la page courante à chaque
//! changement (`set_page_courante`), l'état de la barre d'outils par `synchroniser`, et écoute
//! [`EvenementLaterale`] pour naviguer. Le zone des pages est un voisin flexible : fermer la
//! barre rend sa largeur au document, ce que « Page width » suit tout seul.

pub mod rendu;
pub mod vue;

use fragment_core::CommandManager;

pub use vue::{EtatVignette, EvenementLaterale, LateralePdf};

/// Enregistre les commandes de scène `pixels:pdf_laterale` : aucune ici. Les scènes de la barre
/// latérale (`t4-vignettes`, `t4-toc`, `t4-toc-reveal`, `t4-sans-toc`, `t4-etroite`) passent par
/// `pixels:pdf-page` et l'état éphémère, posés avec la vue montée.
pub fn commandes_de_scene(_commandes: &CommandManager) {}

/// `.pdf-sidebar-resizer { width: 6px }` (pdf-view.css:175) : la poignée de largeur.
pub const LARGEUR_POIGNEE: f32 = 6.0;
/// `.pdf-sidebar-resizer { right: -3px }` (pdf-view.css:174) : elle déborde sur la vue de moitié.
pub const DEBORD_POIGNEE: f32 = -3.0;
/// `.pdf-sidebar-container { max-width: 50% }` (pdf-view.css:141) : le document prime.
pub const PLAFOND_RELATIF: f32 = 0.5;
/// `.pdf-thumbnail-number { right/bottom: 8px }` (pdf-view.css:217-218).
pub const DECALAGE_NUMERO: f32 = 8.0;
/// `.pdf-thumbnail-number { min-width: 14px; line-height: 14px }` (pdf-view.css:219, 225).
pub const COTE_NUMERO: f32 = 14.0;
/// `.pdf-outline-empty { padding: 2rem 1rem }` (pdf-view.css:256), `rem` = 16 px.
pub const MESSAGE_PADDING_V: f32 = 32.0;
pub const MESSAGE_PADDING_H: f32 = 16.0;
/// Une ligne de molette (`deltaMode` 1) vaut 16 px CSS, comme dans les pages.
pub const LIGNE_MOLETTE: f32 = 16.0;
/// Délai entre deux récoltes des rendus du fil (ms) : le fil ne peut pas réveiller l'interface.
pub const RECOLTE_MS: u64 = 4;
