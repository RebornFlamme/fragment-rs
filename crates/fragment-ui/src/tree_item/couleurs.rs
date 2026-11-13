// Port de app/src/ui/views/treeItem.css (couleurs des états) et explorer.css (cible de dépôt)
//! Les couleurs des états d'une ligne d'arbre, lues dans les JETONS de thème, sans GPUI.
//!
//! Variables d'Obsidian reprises par Fragment : `--nav-item-color` (texte atténué), `-hover` et
//! `-active` (texte normal), `--nav-item-background-hover` et `-active` (les deux fonds
//! translucides, ici pré-composés sur le fond du panneau comme le fait Chromium).
//!
//! ★ Le survol l'emporte sur l'état actif : `.tree-item-self.is-clickable:hover` pèse plus lourd que
//! `.tree-item-self.is-active` (une pseudo-classe de plus). Une ligne active survolée prend donc le
//! fond de SURVOL, plus léger, et non celui de l'actif.

use fragment_theme::{Aplat, Couleur, Palette};

use crate::rendu::melange::aplat_exact;
use crate::tree_item::geometrie::POURCENT_CIBLE_DEPOT;

/// L'état d'une ligne, hors survol (que GPUI suit lui-même).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Etat {
    /// L'élément actif (fichier ouvert) : `is-active`.
    pub actif: bool,
    /// Cible d'un dépôt de fichier en cours : `is-drop-target`.
    pub cible_depot: bool,
}

/// Fond et texte d'une ligne dans un état donné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CouleursLigne {
    /// Le fond ; `None` = transparent.
    pub fond: Option<Couleur>,
    /// La couleur du texte (et du chevron, qui en hérite).
    pub texte: Couleur,
}

/// Les couleurs d'une ligne ; `survol` dit si le pointeur est dessus, `panneau` est le fond opaque
/// du panneau sur lequel les fonds translucides se composent.
pub fn couleurs_ligne(c: &Palette, etat: Etat, survol: bool, panneau: Couleur) -> CouleursLigne {
    let texte = if survol || etat.actif {
        c.text_normal
    } else {
        c.text_muted
    };
    // ★ La cible de dépôt passe devant : l'accent à 32 % reste visible sous le pointeur, qui
    // survole la ligne pendant tout le dépôt.
    let fond = if etat.cible_depot {
        Some(aplat_exact(c.text_accent, POURCENT_CIBLE_DEPOT, panneau))
    } else if survol {
        Some(c.aplat(Aplat::Survol, panneau))
    } else if etat.actif {
        Some(c.aplat(Aplat::SurvolActif, panneau))
    } else {
        None
    };
    CouleursLigne { fond, texte }
}

/// La couleur de la barre de guide : `--nav-indentation-guide-color`, soit la bordure de survol.
///
/// ★ Composée comme les REMPLISSAGES de Chromium (`Palette::aplat`, prémultiplié), pas comme une
/// bordure (`aplat_exact`) : la capture de Fragment donne (59,58,59) en sombre et (196,195,192) en
/// clair, que seul `sur_chromium` retrouve (scène t2-explorateur-ouvert, run 43351537908).
pub fn couleur_guide(c: &Palette, panneau: Couleur) -> Couleur {
    c.aplat(Aplat::BordureSurvol, panneau)
}
