// Port de app/src/ui/core/workspace/ItemView.ts (retenirLaLecture, getEphemeralState,
// setEphemeralState : les étages 2 et 3 de la position de lecture).
//! La position de lecture d'une vue, sans GPUI : la rétention (étage 2) et l'état éphémère
//! (étage 3, ce que le cœur écrit dans workspace.json).
//!
//! Les trois étages : (1) la VUE sait lire et reprendre sa position (`Defileur`, `Reprise`) ;
//! (2) une vue CACHÉE ou RÉINSÉRÉE perd son défilement, alors elle RETIENT sa position à chaque
//! défilement et la REND quand sa boîte revient ; (3) l'état éphémère entier voyage dans
//! `LeafLayout.e_state`, et chaque position retenue demande la sauvegarde du layout.
//!
//! ★ Pas de sauvegarde forcée à la fermeture : essayée dans Fragment, jetée (une écriture
//!   coupée par la fermeture tronquait workspace.json). Les 2 dernières secondes de défilement
//!   avant une fermeture brutale peuvent se perdre, c'est voulu.
//! ★ Une vue ne connaît pas le Workspace : `image` RENVOIE « une position a été retenue », et
//!   l'hôte demande alors `request_save_layout`.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Map, Value};

use super::position::{CLE_SCROLL, Defileur, Position};
use super::reprise::{IssueReprise, Reprise};

/// L'état de lecture d'une vue : ce qu'elle a retenu, la reprise en cours.
#[derive(Default)]
pub struct Lecture {
    retenue: Option<Value>,
    defilement_signale: bool,
    visible: bool,
    reprise: Option<Reprise>,
}

impl Lecture {
    /// Un état neuf : rien de retenu, vue invisible.
    pub fn new() -> Self {
        Self::default()
    }

    /// La position retenue (opaque : le défaut y met `{top,left}`).
    pub fn retenue(&self) -> Option<&Value> {
        self.retenue.as_ref()
    }

    /// Un événement de défilement : la lecture en sera retenue à la prochaine image (une fois
    /// par image, quel que soit le nombre d'événements).
    pub fn signaler_defilement(&mut self) {
        self.defilement_signale = true;
    }

    /// Une image. Retient la position si un défilement a eu lieu depuis la dernière et que la
    /// vue a une boîte (cachée, la remise à 0 de la mise en page n'est pas une lecture). Fait
    /// aussi avancer la reprise en cours. Renvoie vrai quand une position a été retenue :
    /// l'hôte demande alors la sauvegarde du layout.
    pub fn image(
        &mut self,
        maintenant_ms: u64,
        a_une_boite: bool,
        lire: impl FnOnce() -> Option<Value>,
    ) -> bool {
        if let Some(r) = self.reprise.as_mut()
            && r.image(maintenant_ms).est_finie()
        {
            self.reprise = None;
        }
        if !std::mem::take(&mut self.defilement_signale) || !a_une_boite {
            return false;
        }
        self.retenue = lire();
        true
    }

    /// La boîte de la vue a changé (l'observateur de taille). `a_une_boite` : elle a une taille
    /// non nulle ; `defileur_a_zero` : son défileur est en (0, 0). Renvoie la position À
    /// RENDRE : la vue revient (de 0 × 0 à une taille) OU a été réinsérée dans la même tâche
    /// (onglet déplacé dans une autre pile : jamais vue à 0 × 0, mais son défileur est remis à
    /// zéro). Une position retenue en haut rend ce même haut : sans effet.
    pub fn boite_changee(&mut self, a_une_boite: bool, defileur_a_zero: bool) -> Option<Value> {
        let revenue = a_une_boite && (!self.visible || defileur_a_zero);
        self.visible = a_une_boite;
        if revenue { self.retenue.clone() } else { None }
    }

    /// Le document de la vue a changé : la position retenue était celle de l'ancien.
    pub fn document_change(&mut self) {
        self.retenue = None;
    }

    /// Un geste (molette, pointeur, clavier dans la vue) : la reprise en cours est abandonnée.
    pub fn geste(&mut self) {
        if let Some(r) = self.reprise.as_mut() {
            r.geste();
        }
    }

    /// Y a-t-il une reprise qui court ?
    pub fn reprise_en_cours(&self) -> bool {
        self.reprise
            .as_ref()
            .is_some_and(|r| r.issue() == IssueReprise::EnCours)
    }

    /// Lance une reprise (la précédente est annulée) : `essai` est rejoué à chaque image
    /// jusqu'au succès, 10 s au plus, ou jusqu'au premier geste.
    pub fn reprendre(&mut self, maintenant_ms: u64, essai: Box<dyn FnMut() -> bool>) {
        let r = Reprise::demarrer(maintenant_ms, essai);
        self.reprise = (!r.issue().est_finie()).then_some(r);
    }

    /// Le défaut de `allerAPosition` : poser `{top,left}` sur le défileur, en réessayant tant
    /// que le contenu n'est pas assez grand. Une position d'une autre forme est ignorée.
    pub fn aller_a_position(
        &mut self,
        maintenant_ms: u64,
        position: &Value,
        defileur: Rc<RefCell<dyn Defileur>>,
    ) {
        let Some(p) = Position::depuis_json(position) else {
            return;
        };
        self.reprendre(
            maintenant_ms,
            Box::new(move || p.poser_sur(&mut *defileur.borrow_mut())),
        );
    }

    /// `getEphemeralState` : complète `base` avec la position. Visible, la vue lit son
    /// défilement ; cachée, elle n'en a plus à lire et rend la position retenue.
    pub fn etat_ephemere(
        &self,
        mut base: Map<String, Value>,
        a_une_boite: bool,
        lue: impl FnOnce() -> Option<Value>,
    ) -> Map<String, Value> {
        let position = if a_une_boite {
            lue()
        } else {
            self.retenue.clone()
        };
        if let Some(p) = position {
            base.insert(CLE_SCROLL.to_owned(), p);
        }
        base
    }

    /// `setEphemeralState` : retient la position de l'état (elle sera rendue au retour de la
    /// boîte) et la renvoie pour que la vue y aille. `None` si l'état n'en porte pas.
    pub fn appliquer_etat(&mut self, etat: &Value) -> Option<Value> {
        let scroll = etat.as_object()?.get(CLE_SCROLL)?;
        self.retenue = (!scroll.is_null()).then(|| scroll.clone());
        self.retenue.clone()
    }
}
