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
    Cadre, DragData, DragHost, DragManager, DropZone, FileMove, GHOST_OFFSET, ItemId, Workspace,
    drop_index_at, zone_at,
};
use gpui_kit::{
    AnyElement, App, FontWeight, Global, IntoElement, ParentElement as _, Pixels, Point,
    Styled as _, div,
};

use crate::echelle::u;
use crate::flottant::texte;
use crate::onglets::mesures::{LARGEUR_ONGLET, largeur_d_un_onglet};
use crate::polices::{largeur_ajustee, largeur_texte};
use crate::rendu::ombre::{ombrer_au_calque, voile};
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
    if cx.has_global::<AppuiFichierEnAttente>() {
        cx.set_global(AppuiFichierEnAttente(None));
    }
}

// ── Le glisser d'un FICHIER de l'explorateur (fr-t2-explorateur-actions) ─────────────────────
//
// Le routage du lâcher est celui du cœur (`DragManager`) : sur l'explorateur, un déplacement ; sur
// une pile du workspace, un onglet (le bord découpe, le centre rejoint) ; ailleurs, un widget. Ce
// module n'ajoute que les crochets par où l'explorateur répond aux trois questions que l'hôte
// pose : « quel élément de l'arbre est sous ce point ? », « ouvre ce fichier dans cette feuille »,
// « un fichier vient d'être lâché sur ce dossier ».

/// Un appui sur une ligne de fichier, en attente du seuil de [`SEUIL_GLISSER`].
#[derive(Debug, Clone, PartialEq)]
pub struct AppuiFichier {
    /// Le chemin du fichier dans le coffre.
    pub chemin: String,
    /// Le texte de la pilule fantôme (le nom sans extension).
    pub libelle: String,
    /// La position de l'appui (px logiques).
    pub x: f64,
    pub y: f64,
}

#[derive(Default)]
struct AppuiFichierEnAttente(Option<AppuiFichier>);

impl Global for AppuiFichierEnAttente {}

/// Note l'appui sur la ligne du fichier `chemin`, à `position` (px de la fenêtre). Le glisser
/// démarre quand le pointeur a parcouru le seuil bouton enfoncé.
pub fn noter_appui_fichier(chemin: &str, libelle: &str, position: Point<Pixels>, cx: &mut App) {
    let zoom = f64::from(crate::echelle::Echelle::courante(cx).zoom());
    cx.set_global(AppuiFichierEnAttente(Some(AppuiFichier {
        chemin: chemin.to_owned(),
        libelle: libelle.to_owned(),
        x: f64::from(f32::from(position.x)) / zoom,
        y: f64::from(f32::from(position.y)) / zoom,
    })));
}

/// L'appui de fichier en attente, s'il y en a un.
pub fn appui_fichier_en_attente(cx: &App) -> Option<AppuiFichier> {
    cx.try_global::<AppuiFichierEnAttente>()
        .and_then(|a| a.0.clone())
}

/// Le chemin de l'arbre sous un point.
pub type RappelPoint = Rc<dyn Fn(f64, f64) -> Option<String>>;
/// Ouvre un fichier dans une feuille.
pub type RappelOuvrir = Rc<dyn Fn(Option<ItemId>, &str)>;
/// Le dossier survolé.
pub type RappelSurvol = Rc<dyn Fn(Option<&str>)>;
/// Déplace un fichier vers une cible.
pub type RappelDeplacer = Rc<dyn Fn(&str, &str)>;

/// Ce que l'explorateur branche sur le glisser de fichier.
#[derive(Clone, Default)]
pub struct CrochetsFichier {
    /// Le chemin de l'élément de l'arbre sous le point (`explorerPathAt`) ; `""` = le vide de
    /// l'arbre (la racine) ; `None` hors de l'explorateur.
    pub chemin_sous_le_point: Option<RappelPoint>,
    /// Ouvrir le fichier (son chemin) dans la feuille que le lâcher vient de créer.
    pub ouvrir: Option<RappelOuvrir>,
    /// Le dossier survolé change (`file-drag-over`) ; `None` = plus aucun.
    pub survol: Option<RappelSurvol>,
    /// Lâcher sur l'explorateur : déplacer `fichier` vers l'élément `cible` (`file-move`).
    pub deplacer: Option<RappelDeplacer>,
    /// Le glisser démarre (`drag-start`, fichier) ou s'achève (`drag-end`).
    pub etat: Option<Rc<dyn Fn(bool)>>,
}

thread_local! {
    static CROCHETS: std::cell::RefCell<CrochetsFichier> = std::cell::RefCell::new(CrochetsFichier::default());
}

/// Pose les crochets du glisser de fichier (un explorateur à la fois par fil).
pub fn poser_crochets(crochets: CrochetsFichier) {
    CROCHETS.with(|c| *c.borrow_mut() = crochets);
}

/// Retire les crochets (l'explorateur disparaît).
pub fn retirer_crochets() {
    poser_crochets(CrochetsFichier::default());
}

fn crochets() -> CrochetsFichier {
    CROCHETS.with(|c| c.borrow().clone())
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

/// Relaie les évènements du glisser de FICHIER du cœur vers les crochets de l'explorateur.
fn relayer_vers_explorateur(gestionnaire: &DragManager<HoteGlisser>) {
    let ev = gestionnaire.events();
    // Les abonnements vivent autant que le gestionnaire : on ne garde pas leurs poignées.
    let _ = ev.on("file-drag-over", |d| {
        if let (Some(cible), Some(f)) = (d.downcast_ref::<Option<String>>(), crochets().survol) {
            f(cible.as_deref());
        }
    });
    let _ = ev.on("file-move", |d| {
        if let (Some(m), Some(f)) = (d.downcast_ref::<FileMove<String>>(), crochets().deplacer) {
            f(&m.file, &m.target);
        }
    });
    let _ = ev.on("drag-start", |d| {
        let fichier = matches!(
            d.downcast_ref::<DragData<Option<ItemId>, String>>(),
            Some(DragData::File(_))
        );
        if let (true, Some(f)) = (fichier, crochets().etat) {
            f(true);
        }
    });
    let _ = ev.on("drag-end", |_| {
        if let Some(f) = crochets().etat {
            f(false);
        }
    });
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
    /// Le chemin d'un fichier du coffre (le glisser vient de l'explorateur).
    type File = String;
    type Tabs = ItemId;

    fn explorer_path_at(&self, x: f64, y: f64) -> Option<String> {
        let f = crochets().chemin_sous_le_point?;
        f(x, y)
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

    fn open_file_and_activate(&self, leaf: &Option<ItemId>, file: &String) {
        if let Some(f) = crochets().ouvrir {
            f(*leaf, file);
        }
    }

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
        let gestionnaire = DragManager::new(hote);
        relayer_vers_explorateur(&gestionnaire);
        Self {
            gestionnaire,
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
                return self.deplacer_fichier(x, y, appuye, cx);
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

    /// Le seuil d'un appui de FICHIER : même mécanique que l'onglet, la source est un chemin.
    fn deplacer_fichier(&mut self, x: f64, y: f64, appuye: bool, cx: &mut App) -> bool {
        let Some(a) = appui_fichier_en_attente(cx) else {
            return false;
        };
        if !appuye {
            oublier_appui(cx);
            return false;
        }
        if (x - a.x).hypot(y - a.y) < SEUIL_GLISSER {
            return false;
        }
        oublier_appui(cx);
        self.gestionnaire
            .start_drag(DragData::File(a.chemin), &a.libelle);
        self.pointeur = Some((x, y));
        self.gestionnaire.on_move(x, y, false);
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
        .bg(couleur(c.tab_bar_bg))
        .text_color(couleur(c.text_normal))
        .whitespace_nowrap();
    // ★ L'ombre ET l'arrondi se DÉCLARENT (`ombrer`, ADR-009) : posés à la main (`.shadow`,
    //   `.rounded`), le halo de `--shadow-s` et ses coins sortaient en divergences hors boîte au
    //   delta 1 (t1-glisser-zone, run 43042397780 : bord haut du halo au-dessus de la pilule).
    // ★ La pilule est `position: fixed` : Chromium la rastérise dans un calque composé, comme la
    //   modale. Son ombre est donc l'image calculée sur le CPU du calque émulé (fr-p4.1, ADR-013 §2),
    //   peinte sous la pilule par un « voile » transparent (décision du chef, fr-t1.5 [D]) ; le
    //   `BoxShadow` de GPUI laissait un étalement de −0,29 px en clair (`t1-glisser-zone-light`).
    let transparent = fragment_theme::Couleur::octets(0, 0, 0, 0);
    let pilule = ombrer_au_calque(
        pilule,
        cx,
        &c.shadow_s,
        "--shadow-s",
        u(cx, f.radius_sm),
        u(cx, 0.0),
        transparent,
    )
    .child(label.to_owned());
    div()
        .absolute()
        .size_full()
        .child(voile(transparent))
        .child(texte(pilule, cx, taille, 1.5))
        .into_any_element()
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
