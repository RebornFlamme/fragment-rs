//! Pièces jointes du champ et parts `attachment` (tâche `fr-t7-pieces-citation`).
//!
//! Un COMPORTEMENT du champ, contribué à `chat:composer@1` sous l'id `attachments` (priorité
//! [`PRIORITE_PIECES`]) : un fournisseur qui n'en veut pas le refuse (`ChatProvider::without`), un
//! plugin peut le remplacer (même id, priorité plus haute).
//!
//! - [`donnees`] : ce qui est pur (mimes, URL `data:`, JSON d'une pièce, parts) ;
//! - [`etat`] : l'état d'un champ ([`EtatPieces`]), sa poignée et l'extension ([`ExtensionPieces`]) ;
//! - [`vue`] : les puces du champ (vignette de 56 px, ✕ au survol), le rendu du part `attachment`,
//!   la lecture du presse-papiers et du dépôt.
//!
//! ★ Écarts au TS (voir `parite.d/fr-t7-pieces-citation.toml`) : pas d'URL d'objet, l'image d'un part
//! est une URL `data:` (celle que la relecture produit déjà) ; les octets voyagent dans `SendOpts` en
//! base64 (format de `fragment-agent::fournisseur`).

pub mod donnees;
pub mod etat;
pub mod vue;

pub use donnees::{
    est_image, mime_de_nom, nom_par_defaut, octets_de_url, part_attachment, part_data,
    piece_depuis_json, piece_depuis_octets, piece_json, pieces_de_chemins, url_donnees,
};
pub use etat::{ATTACHMENTS_EXTENSION, EtatPieces, ExtensionPieces, PRIORITE_PIECES, Piece};
pub use vue::{RenduPieceJointe, pieces_du_presse_papiers, zone_puces};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:chat_pieces` : aucune ici (la scène `t7-pieces` passe
/// par la vue chat montée, `fr-t7-scenes-b`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
