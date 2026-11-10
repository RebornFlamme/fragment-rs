// Port de app/src/ui/core/workspace/WorkspaceTabs.ts (la part modèle) et de
// Workspace.createLeafBySplit.
//
// Une pile d'onglets sans DOM : ses feuilles, l'onglet visible, la sélection, la découpe d'un
// pane (le cœur du glisser d'onglet ET des commandes de split) et la traduction d'un point en
// zone de dépôt. La barre d'onglets, son contour et ses menus sont du rendu (fragment-ui) ;
// ce qu'ils déclenchent passe par les méthodes d'ici.
//
// Ce que `selectTab` faisait de plus dans Fragment — matérialiser une feuille différée, tenir
// l'invariant « la feuille active est l'onglet visible » — appartient au Workspace, qui le
// rétablit après chaque opération sur l'arbre.

use super::SplitDirection;
use super::arbre::{Arbre, ErreurArbre, ItemId, Rect};

/// Ce qu'un dépôt fait selon l'endroit du point dans la pile visée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropZone {
    /// Bas de la barre d'onglets : réordonner, à l'interstice sous le pointeur.
    Reorder,
    SplitTop,
    SplitBottom,
    SplitLeft,
    SplitRight,
    /// Milieu du corps : rejoindre la pile, en dernier.
    Center,
}

/// La part HAUTE de la barre d'onglets (en fraction de sa hauteur) qui découpe vers le haut
/// au lieu de réordonner. ★ Réglée « au ressenti » dans Fragment, et déterminante pour que le
/// geste paraisse juste : ne pas la retoucher sans essayer à la main.
pub const PART_HAUTE_DE_LA_BARRE: f64 = 0.2;

/// Distance relative au bord du corps (un quart) sous laquelle un dépôt découpe ; au-delà,
/// c'est le centre.
pub const QUART_DU_BORD: f64 = 0.25;

/// Traduit un point en zone de dépôt, d'après la géométrie PROPRE de la pile : le rectangle
/// de sa barre d'onglets et celui de son corps (le conteneur des feuilles).
///
/// Sur la barre : la part haute découpe vers le haut, le reste réordonne. Dans le corps : le
/// bord le plus proche parmi gauche, droite et bas, s'il est à moins d'un quart ; sinon le
/// centre. Pas de « haut » dans le corps : le haut, c'est la barre.
pub fn zone_at(barre: Rect, corps: Rect, x: f64, y: f64) -> DropZone {
    if y <= barre.bas() {
        let py = (y - barre.y) / barre.hauteur;
        return if py < PART_HAUTE_DE_LA_BARRE {
            DropZone::SplitTop
        } else {
            DropZone::Reorder
        };
    }
    let px = (x - corps.x) / corps.largeur;
    let py = (y - corps.y) / corps.hauteur;
    let (gauche, droite, bas) = (px, 1.0 - px, 1.0 - py);
    let min = gauche.min(droite).min(bas);
    if min > QUART_DU_BORD {
        DropZone::Center
    } else if min == gauche {
        DropZone::SplitLeft
    } else if min == droite {
        DropZone::SplitRight
    } else {
        DropZone::SplitBottom
    }
}

/// L'interstice (0…N) sous l'abscisse `x`, d'après les rectangles des onglets dans l'ordre :
/// avant le premier onglet dont le milieu est à droite de `x`, sinon après le dernier.
pub fn drop_index_at(onglets: &[Rect], x: f64) -> usize {
    onglets
        .iter()
        .position(|r| x < r.x + r.largeur / 2.0)
        .unwrap_or(onglets.len())
}

impl<F> Arbre<F> {
    /// L'onglet VISIBLE d'une pile (`getActiveLeaf`) ; `None` pour une pile vide ou un nœud
    /// qui n'est pas une pile.
    pub fn active_leaf(&self, pile: ItemId) -> Option<ItemId> {
        let t = self.pile(pile)?;
        t.children.get(t.active_index).copied()
    }

    /// Le rang de l'onglet visible (`currentTab` du layout).
    pub fn active_index(&self, pile: ItemId) -> Option<usize> {
        self.pile(pile).map(|t| t.active_index)
    }

    /// Montre l'onglet `index` (une seule feuille visible, les autres cachées, pas détruites).
    /// `false` hors bornes : rien ne change.
    pub fn select_tab(&mut self, pile: ItemId, index: usize) -> bool {
        let Some(t) = self.pile_mut(pile) else {
            return false;
        };
        if index >= t.children.len() {
            return false;
        }
        if t.active_index != index {
            t.active_index = index;
            self.demander_sauvegarde();
        }
        true
    }

    /// Montre la feuille `feuille` dans sa pile ; `false` si elle n'est pas dans une pile.
    pub fn select_leaf(&mut self, feuille: ItemId) -> bool {
        let Some(pile) = self.parent(feuille) else {
            return false;
        };
        match self.children(pile).iter().position(|&c| c == feuille) {
            Some(i) => self.select_tab(pile, i),
            None => false,
        }
    }

    /// Le commit d'un dépôt de feuille sur une pile, selon la zone : réordonne (à
    /// l'interstice `interstice`, voir `drop_index_at`), rejoint la pile (centre) ou découpe.
    pub fn drop_leaf(
        &mut self,
        pile: ItemId,
        feuille: ItemId,
        zone: DropZone,
        interstice: usize,
    ) -> Result<(), ErreurArbre> {
        match zone {
            DropZone::Reorder => self.reorder_leaf(pile, feuille, interstice),
            DropZone::Center => self.add_child(pile, feuille, None),
            DropZone::SplitLeft
            | DropZone::SplitRight
            | DropZone::SplitTop
            | DropZone::SplitBottom => self.split_leaf(pile, feuille, zone).map(|_| ()),
        }
    }

    /// Réordonne à l'interstice `vers` (0…N) de cette pile. ★ Le décalage d'un ne vaut que
    /// DANS la pile : la feuille quitte sa place avant d'être réinsérée, les interstices
    /// situés après elle reculent d'un rang.
    pub fn reorder_leaf(
        &mut self,
        pile: ItemId,
        feuille: ItemId,
        vers: usize,
    ) -> Result<(), ErreurArbre> {
        let de = self.children(pile).iter().position(|&c| c == feuille);
        let mut vers = vers;
        if let Some(de) = de {
            if de < vers {
                vers -= 1;
            }
            if de == vers {
                return Ok(());
            }
        }
        self.add_child(pile, feuille, Some(vers))
    }

    /// Découpe : une NOUVELLE pile accueille `feuille`, du côté que dit la zone, à côté de
    /// `pile`. Rend la nouvelle pile. Une seule règle :
    /// - même axe que le split parent → pile sœur, avant ou après ce pane ;
    /// - axe perpendiculaire → un nouveau split de l'axe voulu prend la place du pane et
    ///   contient [pane, nouvelle pile] dans le bon ordre.
    ///
    /// ★ Une seule implémentation de la découpe, trois portes d'entrée : le glisser d'onglet,
    ///   `create_leaf_by_split` (donc les commandes `workspace:split-*`) et le menu du pane.
    ///   Si la feuille vide sa pile d'origine, celle-ci part dans `prendre_piles_videes`.
    ///   Le Workspace rend ensuite la feuille active.
    pub fn split_leaf(
        &mut self,
        pile: ItemId,
        feuille: ItemId,
        zone: DropZone,
    ) -> Result<ItemId, ErreurArbre> {
        let parent = self.parent(pile).ok_or(ErreurArbre::Racine(pile))?;
        let direction_parent = self
            .split(parent)
            .map(|s| s.direction)
            .ok_or(ErreurArbre::PasUnParent(parent))?;
        if !self.est_feuille(feuille) {
            return Err(ErreurArbre::FormeRefusee {
                parent: pile,
                enfant: feuille,
            });
        }
        let (axe, avant) = match zone {
            DropZone::SplitLeft => (SplitDirection::Vertical, true),
            DropZone::SplitRight => (SplitDirection::Vertical, false),
            DropZone::SplitTop => (SplitDirection::Horizontal, true),
            DropZone::SplitBottom => (SplitDirection::Horizontal, false),
            DropZone::Reorder | DropZone::Center => {
                return Err(ErreurArbre::FormeRefusee {
                    parent: pile,
                    enfant: feuille,
                });
            }
        };

        let nouvelle = self.creer_pile();
        let i = self
            .children(parent)
            .iter()
            .position(|&c| c == pile)
            .unwrap_or(0);
        if direction_parent == axe {
            self.add_child(parent, nouvelle, Some(if avant { i } else { i + 1 }))?;
        } else {
            let enveloppe = self.creer_split(axe);
            // L'enveloppe prend la place du pane (le pane glisse d'un rang, puis la quitte).
            self.add_child(parent, enveloppe, Some(i))?;
            if avant {
                self.add_child(enveloppe, nouvelle, None)?;
                self.add_child(enveloppe, pile, None)?;
            } else {
                self.add_child(enveloppe, pile, None)?;
                self.add_child(enveloppe, nouvelle, None)?;
            }
        }
        self.add_child(nouvelle, feuille, None)?;
        Ok(nouvelle)
    }

    /// Une nouvelle feuille (portant `charge`) À CÔTÉ de `feuille`, dans un split :
    /// `Vertical` = à droite (une colonne de plus), `Horizontal` = en dessous. `None` si
    /// `feuille` n'est pas dans une pile d'un split (la charge est alors abandonnée).
    ///
    /// Ne fait que déléguer à `split_leaf` : rien n'est réimplémenté ici.
    pub fn create_leaf_by_split(
        &mut self,
        feuille: ItemId,
        direction: SplitDirection,
        charge: F,
    ) -> Option<ItemId> {
        let pile = self.parent(feuille).filter(|&p| self.est_pile(p))?;
        self.parent(pile)?;
        let nouvelle = self.creer_feuille(charge);
        let zone = match direction {
            SplitDirection::Vertical => DropZone::SplitRight,
            SplitDirection::Horizontal => DropZone::SplitBottom,
        };
        match self.split_leaf(pile, nouvelle, zone) {
            Ok(_) => Some(nouvelle),
            Err(_) => {
                // Détachée et jamais insérée : on la rend à l'arène.
                let _ = self.liberer(nouvelle);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn barre() -> Rect {
        Rect::new(0.0, 0.0, 400.0, 40.0)
    }

    fn corps() -> Rect {
        Rect::new(0.0, 40.0, 400.0, 400.0)
    }

    // ─── Ajoutés : zoneAt / dropIndexAt n'ont pas de test TS (géométrie DOM) ───

    #[test]
    fn zone_haut_de_barre_decoupe_le_reste_reordonne() {
        assert_eq!(zone_at(barre(), corps(), 100.0, 5.0), DropZone::SplitTop);
        assert_eq!(zone_at(barre(), corps(), 100.0, 30.0), DropZone::Reorder);
        assert_eq!(zone_at(barre(), corps(), 100.0, 40.0), DropZone::Reorder);
    }

    #[test]
    fn zone_bords_du_corps_et_centre() {
        assert_eq!(zone_at(barre(), corps(), 20.0, 240.0), DropZone::SplitLeft);
        assert_eq!(
            zone_at(barre(), corps(), 390.0, 240.0),
            DropZone::SplitRight
        );
        assert_eq!(
            zone_at(barre(), corps(), 200.0, 430.0),
            DropZone::SplitBottom
        );
        assert_eq!(zone_at(barre(), corps(), 200.0, 200.0), DropZone::Center);
    }

    #[test]
    fn interstice_au_milieu_des_onglets() {
        let onglets = [
            Rect::new(0.0, 0.0, 100.0, 30.0),
            Rect::new(100.0, 0.0, 100.0, 30.0),
        ];
        assert_eq!(drop_index_at(&onglets, 10.0), 0);
        assert_eq!(drop_index_at(&onglets, 60.0), 1);
        assert_eq!(drop_index_at(&onglets, 160.0), 2);
        assert_eq!(drop_index_at(&[], 160.0), 0);
    }
}
