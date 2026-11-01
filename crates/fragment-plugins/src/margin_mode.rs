// Port de app/src/ui/coreCommands/marginMode.ts (le MODÈLE)
//
// ═══════════════════════════════════════════════════════════════════════════
//  La MARGE DE LECTURE 1/4 – 3/4 — une affaire de mise en page de la feuille,
//  donc du plugin interne « Espace de travail ». Une note ou un PDF en mise en
//  page annotée : la zone de lecture se réserve 1/4 à droite, la colonne GARDE sa
//  largeur (centrée dans les 3/4), la marge s'élargit pour les widgets.
//
//  Trois COMMANDES, une seule action (la classe de la feuille) :
//    - `workspace:open-margin`  — raccourci : le BALAYAGE vers la droite (SwipeRight) ;
//    - `workspace:close-margin` — raccourci : le balayage vers la gauche ;
//    - `workspace:toggle-margin` — palette, et le BOUTON de l'en-tête de vue.
//
//  Par feuille, non persisté. Toute vue qui montre un DOCUMENT : aucune vue n'est
//  nommée ici — chacune dit en CSS (côté fragment-ui) ce que `is-margin-mode`
//  change chez elle.
// ═══════════════════════════════════════════════════════════════════════════

use fragment_core::swipe::{SWIPE_LEFT, SWIPE_RIGHT};

/// La classe posée sur la feuille (conservée : le CSS de fragment-ui la reprend sous ce nom).
pub const CLASSE: &str = "is-margin-mode";

/// Comment une commande décide de l'état voulu depuis l'état actuel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etat {
    /// `(on) => !on`
    Bascule,
    /// `() => true`
    Ouvre,
    /// `() => false`
    Ferme,
}

impl Etat {
    /// L'état voulu, sachant l'actuel.
    pub fn voulu(self, actuel: bool) -> bool {
        match self {
            Etat::Bascule => !actuel,
            Etat::Ouvre => true,
            Etat::Ferme => false,
        }
    }
}

/// Une commande de marge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandeMarge {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    /// La touche (sans modificateur) : un balayage, `SwipeRight` / `SwipeLeft`.
    pub hotkey: Option<&'static str>,
    pub etat: Etat,
}

/// Les trois commandes, dans l'ordre d'inscription.
pub const COMMANDES_MARGE: [CommandeMarge; 3] = [
    CommandeMarge {
        id: "toggle-margin",
        name: "Basculer la marge de lecture 1/4 – 3/4",
        icon: "panel-right-dashed",
        hotkey: None,
        etat: Etat::Bascule,
    },
    CommandeMarge {
        id: "open-margin",
        name: "Ouvrir la marge de lecture 1/4 – 3/4",
        icon: "panel-right-open",
        hotkey: Some(SWIPE_RIGHT),
        etat: Etat::Ouvre,
    },
    CommandeMarge {
        id: "close-margin",
        name: "Fermer la marge de lecture",
        icon: "panel-right-close",
        hotkey: Some(SWIPE_LEFT),
        etat: Etat::Ferme,
    },
];

/// Une vue qui montre un document, dans une feuille du workspace (pas un widget).
pub fn est_note(a_un_document: bool, a_un_parent: bool) -> bool {
    a_un_document && a_un_parent
}

/// La commande est-elle disponible, et quel état poser ? `None` : indisponible
/// (la `checkCallback` rend faux). `view` = `(a un document, feuille dans un parent)`
/// de la vue active.
pub fn appliquer(
    commande: &CommandeMarge,
    view: Option<(bool, bool)>,
    actuel: bool,
) -> Option<bool> {
    let (doc, parent) = view?;
    est_note(doc, parent).then(|| commande.etat.voulu(actuel))
}

#[cfg(test)]
mod tests {
    use super::*;

    // marginMode.ts n'a pas de test unitaire : ces cas sont des ajouts du port.

    #[test]
    fn les_trois_commandes_et_leurs_balayages() {
        let ids: Vec<_> = COMMANDES_MARGE.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec!["toggle-margin", "open-margin", "close-margin"]);
        assert_eq!(COMMANDES_MARGE[1].hotkey, Some("SwipeRight"));
        assert_eq!(COMMANDES_MARGE[2].hotkey, Some("SwipeLeft"));
    }

    #[test]
    fn bascule_ouvre_ferme() {
        assert!(Etat::Bascule.voulu(false));
        assert!(!Etat::Bascule.voulu(true));
        assert!(Etat::Ouvre.voulu(true));
        assert!(!Etat::Ferme.voulu(true));
    }

    #[test]
    fn indisponible_hors_d_une_note() {
        let c = &COMMANDES_MARGE[0];
        assert_eq!(appliquer(c, None, false), None);
        assert_eq!(appliquer(c, Some((false, true)), false), None);
        assert_eq!(appliquer(c, Some((true, false)), false), None);
        assert_eq!(appliquer(c, Some((true, true)), false), Some(true));
    }
}
