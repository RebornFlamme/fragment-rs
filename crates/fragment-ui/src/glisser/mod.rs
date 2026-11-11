// Port de app/src/ui/core/DragManager.ts (la capture du pointeur et le fantôme), de la source de
// glisser de WorkspaceTabs.ts (le seuil, `showDropIndicator`) et de Workspace.showDropIndicatorAt,
// avec les règles `.drag-ghost` et `.is-split-preview-*` de app/src/ui/workspace.css.
//
//! Le glisser d'un onglet : l'appui sur un en-tête, le seuil de 4 px, puis le routage du
//! [`DragManager`] du cœur jusqu'au relâché, qui découpe (bord), rejoint (centre) ou réordonne
//! (barre d'onglets).
//!
//! ★ L'onglet INACTIF doit pouvoir être glissé : c'est le cas qui a cassé Fragment (l'appui activait
//!   l'onglet en reconstruisant la barre, et la capture visait un élément détaché). Ici rien n'est
//!   capturé sur l'élément : l'appui est NOTÉ ([`noter_appui`], appelé par l'en-tête d'onglet après
//!   son activation), et la suite du geste arrive par les écouteurs globaux du workspace
//!   (`crate::splits`), qui vivent indépendamment de la barre redessinée.
//!
//! Le cœur décide (zone, découpe) ; ce module ne fait que traduire des points en appels, et peindre
//! l'aperçu (via `OptionsPile::apercu` de la pile visée) et la pilule fantôme.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{
    Cadre, DragData, DragHost, DragManager, DropZone, GHOST_OFFSET, ItemId, Workspace,
    drop_index_at, zone_at,
};
use gpui_kit::{
    AnyElement, App, FontWeight, Global, IntoElement, ParentElement as _, Pixels, Point,
    Styled as _, div,
};

use crate::echelle::u;
use crate::flottant::{ombres, texte};
use crate::onglets::mesures::{LARGEUR_ONGLET, largeur_d_un_onglet};
use crate::polices::{largeur_ajustee, largeur_texte};
use crate::splits::geometrie::Disposition;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La distance (px) que le pointeur doit parcourir, bouton enfoncé, avant que l'appui sur un onglet
/// devienne un glisser (WorkspaceTabs.ts : en deçà, c'est un clic).
pub const SEUIL_GLISSER: f64 = 4.0;

/// Le rembourrage vertical de la pilule fantôme (`.drag-ghost` : `padding: 3px …`, workspace.css).
const REMBOURRAGE_FANTOME: f32 = 3.0;

/// Ce que la barre d'onglets réserve autour des en-têtes (workspace.css) : le rembourrage de
/// l'intérieur (4 + 4), le « + » (marge −1, case de 24, 2 à droite) et le chevron (2, 24, 4).
const AUTOUR_DES_ONGLETS: f64 = 8.0 + 25.0 + 30.0;
/// Le premier en-tête commence à 4 px du bord gauche de la barre.
const DEBUT_DES_ONGLETS: f64 = 4.0;

/// Un appui sur un en-tête d'onglet, en attente du seuil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appui {
    pub pile: ItemId,
    pub leaf: ItemId,
    /// La position de l'appui (px de la fenêtre).
    pub x: f64,
    pub y: f64,
}

#[derive(Default)]
struct AppuiEnAttente(Option<Appui>);

impl Global for AppuiEnAttente {}

/// Note l'appui sur l'en-tête de l'onglet `leaf` de `pile`, à `position` (px de la fenêtre). Appelé
/// par l'en-tête APRÈS l'activation de l'onglet : le glisser part de l'état activé, sans élément
/// à capturer.
pub fn noter_appui(pile: ItemId, leaf: ItemId, position: Point<Pixels>, cx: &mut App) {
    let zoom = f64::from(crate::echelle::Echelle::courante(cx).zoom());
    cx.set_global(AppuiEnAttente(Some(Appui {
        pile,
        leaf,
        x: f64::from(f32::from(position.x)) / zoom,
        y: f64::from(f32::from(position.y)) / zoom,
    })));
}

/// L'appui en attente, s'il y en a un.
pub fn appui_en_attente(cx: &App) -> Option<Appui> {
    cx.try_global::<AppuiEnAttente>().and_then(|a| a.0)
}

fn oublier_appui(cx: &mut App) {
    if cx.has_global::<AppuiEnAttente>() {
        cx.set_global(AppuiEnAttente(None));
    }
}

/// L'aperçu du dépôt sous le pointeur : la pile visée, la zone, et l'abscisse de la barre
/// d'insertion (relative à la barre) pour un réordonnancement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Apercu {
    pub pile: ItemId,
    pub zone: DropZone,
    pub insertion: Option<f32>,
}

/// Les rectangles des en-têtes d'onglets d'une pile, déduits de la largeur de sa barre : 206 px
/// chacun tant que la place suffit, la part uniforme sinon (`largeur_d_un_onglet`).
///
/// ★ Une DÉDUCTION, pas une mesure : la barre d'onglets (`crate::onglets`) n'exporte pas ses
///   bornes. Exacte tant que les onglets ont leur largeur pleine (le cas des scènes) ; à remplacer
///   par la mesure quand la barre la publiera.
pub fn cadres_des_onglets(barre: Cadre, n: usize) -> Vec<Cadre> {
    let dispo = (barre.largeur - AUTOUR_DES_ONGLETS).max(0.0);
    let w = f64::from(largeur_d_un_onglet(n, dispo as f32)).min(f64::from(LARGEUR_ONGLET));
    (0..n)
        .map(|i| {
            Cadre::new(
                barre.x + DEBUT_DES_ONGLETS + w * i as f64,
                barre.y,
                w,
                barre.hauteur,
            )
        })
        .collect()
}

/// L'hôte du [`DragManager`] : le Workspace et la dernière mise en page.
#[derive(Clone)]
pub struct HoteGlisser {
    workspace: Rc<RefCell<Workspace>>,
    disposition: Rc<RefCell<Disposition>>,
    apercu: Rc<Cell<Option<Apercu>>>,
    entete: f64,
}

impl HoteGlisser {
    /// La zone et l'interstice d'un point dans la pile `pile`, avec l'abscisse de la barre
    /// d'insertion (relative à la barre).
    fn viser(&self, pile: ItemId, x: f64, y: f64) -> Option<(DropZone, usize, f32)> {
        let d = self.disposition.try_borrow().ok()?;
        let barre = d.barre(pile, self.entete)?;
        let corps = d.corps(pile, self.entete)?;
        let zone = zone_at(barre, corps, x, y);
        let n = self
            .workspace
            .try_borrow()
            .ok()
            .map_or(0, |w| w.arbre().children(pile).len());
        let onglets = cadres_des_onglets(barre, n);
        let i = drop_index_at(&onglets, x);
        let bord = match onglets.get(i) {
            Some(o) => o.x - barre.x,
            None => onglets.last().map_or(0.0, |o| o.droite() - barre.x),
        };
        Some((zone, i, bord as f32))
    }
}

impl DragHost for HoteGlisser {
    /// `None` : une création de feuille refusée (pile disparue).
    type Leaf = Option<ItemId>;
    /// Les fichiers arrivent avec le coffre (T2) : rien à tirer d'ici en T1.
    type File = String;
    type Tabs = ItemId;

    fn explorer_path_at(&self, _x: f64, _y: f64) -> Option<String> {
        None
    }

    fn clear_drop_indicator(&self) {
        self.apercu.set(None);
    }

    fn show_drop_indicator_at(&self, x: f64, y: f64) {
        let a = self.tabs_at_point(x, y).and_then(|pile| {
            let (zone, _, insertion) = self.viser(pile, x, y)?;
            Some(Apercu {
                pile,
                zone,
                insertion: Some(insertion),
            })
        });
        self.apercu.set(a);
    }

    fn tabs_at_point(&self, x: f64, y: f64) -> Option<ItemId> {
        let d = self.disposition.try_borrow().ok()?;
        let w = self.workspace.try_borrow().ok()?;
        w.arbre().get_tabs_at_point(&|id| d.cadre(id), x, y)
    }

    fn create_leaf_in_parent(&self, tabs: &ItemId) -> Option<ItemId> {
        let mut w = self.workspace.try_borrow_mut().ok()?;
        w.create_leaf_in_parent(*tabs, None).ok()
    }

    fn in_sidedock(&self, tabs: &ItemId) -> bool {
        self.workspace
            .try_borrow()
            .is_ok_and(|w| w.arbre().get_root(*tabs) != w.root_split())
    }

    fn drop_leaf(&self, tabs: &ItemId, leaf: &Option<ItemId>, x: f64, y: f64) {
        let Some(leaf) = *leaf else {
            return;
        };
        let Some((zone, i, _)) = self.viser(*tabs, x, y) else {
            return;
        };
        if let Ok(mut w) = self.workspace.try_borrow_mut() {
            // Un refus de l'arbre (dépôt d'une feuille sur elle-même…) laisse tout en place.
            let _ = w.drop_leaf(*tabs, leaf, zone, i);
        }
    }

    fn open_file_and_activate(&self, _leaf: &Option<ItemId>, _file: &String) {}

    fn leaf_file(&self, _leaf: &Option<ItemId>) -> Option<String> {
        None
    }
}

/// Le glisser d'onglet d'une fenêtre : le routeur du cœur et la position du pointeur.
pub struct Glisser {
    gestionnaire: DragManager<HoteGlisser>,
    apercu: Rc<Cell<Option<Apercu>>>,
    pointeur: Option<(f64, f64)>,
}

impl Glisser {
    /// `entete` : la hauteur des barres d'onglets (`--header-height`), pour la zone de dépôt.
    pub fn new(
        workspace: Rc<RefCell<Workspace>>,
        disposition: Rc<RefCell<Disposition>>,
        entete: f64,
    ) -> Self {
        let apercu = Rc::new(Cell::new(None));
        let hote = HoteGlisser {
            workspace,
            disposition,
            apercu: apercu.clone(),
            entete,
        };
        Self {
            gestionnaire: DragManager::new(hote),
            apercu,
            pointeur: None,
        }
    }

    /// Un glisser est en cours.
    pub fn en_cours(&self) -> bool {
        self.gestionnaire.is_dragging()
    }

    /// L'aperçu du dépôt sous le pointeur.
    pub fn apercu(&self) -> Option<Apercu> {
        if self.en_cours() {
            self.apercu.get()
        } else {
            None
        }
    }

    /// Le texte de la pilule et la position du pointeur, pendant un glisser.
    pub fn fantome(&self) -> Option<(String, f64, f64)> {
        let (x, y) = self.pointeur?;
        let label = self.gestionnaire.ghost_label.clone()?;
        Some((label, x, y))
    }

    /// Un mouvement du pointeur (px logiques), bouton gauche enfoncé ou non. Démarre le glisser
    /// quand un appui en attente franchit le seuil. Rend vrai si l'image doit être redessinée.
    pub fn deplacer(&mut self, x: f64, y: f64, appuye: bool, alt: bool, cx: &mut App) -> bool {
        if !self.en_cours() {
            let Some(a) = appui_en_attente(cx) else {
                return false;
            };
            if !appuye {
                // Le relâché a eu lieu ailleurs : l'appui est périmé.
                oublier_appui(cx);
                return false;
            }
            if (x - a.x).hypot(y - a.y) < SEUIL_GLISSER {
                return false;
            }
            oublier_appui(cx);
            let label = self
                .gestionnaire
                .host()
                .workspace
                .try_borrow()
                .ok()
                .and_then(|w| w.leaf(a.leaf).map(|l| l.get_display_text()))
                .unwrap_or_default();
            self.gestionnaire
                .start_drag(DragData::Leaf(Some(a.leaf)), &label);
        }
        self.pointeur = Some((x, y));
        self.gestionnaire.on_move(x, y, alt);
        true
    }

    /// Le relâché du bouton : le dépôt, s'il y avait un glisser. Rend vrai si l'arbre a pu changer.
    pub fn relacher(&mut self, x: f64, y: f64, alt: bool, cx: &mut App) -> bool {
        oublier_appui(cx);
        if !self.en_cours() {
            return false;
        }
        self.pointeur = None;
        self.gestionnaire.on_up(x, y, alt);
        self.apercu.set(None);
        true
    }

    /// Le geste est interrompu (fenêtre défocalisée…) : rien n'est déposé.
    pub fn annuler(&mut self, cx: &mut App) {
        oublier_appui(cx);
        self.pointeur = None;
        self.gestionnaire.on_cancel();
        self.apercu.set(None);
    }
}

/// La pilule fantôme (`.drag-ghost`) : le titre de l'onglet, posée à côté du pointeur
/// (`GHOST_OFFSET`), hors de tout hit-test (rien ne l'écoute).
pub fn rendre_fantome(label: &str, x: f64, y: f64, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f, ty) = (&t.palette, &t.forme, &t.typo);
    let taille = ty.font_ui_small;
    let largeur = largeur_texte(cx, label, FontWeight::NORMAL, taille);
    let pilule = div()
        .absolute()
        .left(u(cx, (x + GHOST_OFFSET.0) as f32))
        .top(u(cx, (y + GHOST_OFFSET.1) as f32))
        .w(largeur_ajustee(cx, largeur, 2.0 * f.size_2_5))
        .px(u(cx, f.size_2_5))
        .py(u(cx, REMBOURRAGE_FANTOME))
        .rounded(u(cx, f.radius_sm))
        .bg(couleur(c.tab_bar_bg))
        .text_color(couleur(c.text_normal))
        .shadow(ombres(cx, &c.shadow_s))
        .whitespace_nowrap()
        .child(label.to_owned());
    texte(pilule, cx, taille, 1.5).into_any_element()
}

/// Enregistre les commandes de scène `pixels:glisser` : aucune, les scènes de glisser rejouent le
/// geste à la souris (`drag`).
pub fn commandes_de_scene(_commandes: &fragment_core::CommandManager) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_onglets_de_pleine_largeur_se_suivent_de_206_en_206_depuis_4_px() {
        let barre = Cadre::new(36.0, 0.0, 1244.0, 36.0);
        let o = cadres_des_onglets(barre, 2);
        assert_eq!(o[0], Cadre::new(40.0, 0.0, 206.0, 36.0));
        assert_eq!(o[1].x, 246.0);
    }
}
