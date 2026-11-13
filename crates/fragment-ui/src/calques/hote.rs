// Port de app/src/ui/core/layers/ViewOverlays.ts (l'hôte : plans, racine de marge, arbitre, actif)
//
//! L'hôte des calques d'une vue : il implémente le contrat `OverlayHost` de `fragment-layers`
//! (et les traits d'éléments que ses modèles lisent et écrivent) pour TOUTE vue à surface, sans
//! une ligne de code de la vue. Il tient un REGISTRE d'éléments opaques (`ElementId`) : leur plan,
//! les côtés écrits par `WidgetLayer`, la largeur imposée par la bande, l'affichage, la place de
//! l'arbitre et la boîte MESURÉE à la dernière image. Le dessin vit dans [`super::element`].
//!
//! - plan VIEWPORT : couvre le pane, ne défile pas (ancres par coin) ;
//! - plan DOCUMENT : son origine est celle de `coords_at_pos`, il suit le texte ; unités de la
//!   vue, mises à l'échelle de son zoom ;
//! - racine de MARGE : posée dans le plan document (elle défile avec lui) mais en px ÉCRAN, le
//!   zoom annulé. `gutter_slot` y range l'élément ; le retrait du slot le rend à son plan.
//!
//! ★ L'arbitre de la marge et l'actif des widgets appartiennent à l'HÔTE : la vue est le seul
//!   objet commun à tous ses calques. L'hôte ne fait que les brancher (`with_wanted_scale` :
//!   le niveau voulu arrive en unités du plan) ; leur logique est dans `fragment-layers`.
//!
//! ★ Aucun emprunt du registre n'est tenu pendant un rappel : les rappels des modèles
//!   reviennent écrire ici (poser un côté, lire une bande) au milieu de la passe.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};

use fragment_layers::gutter_arbiter::{GutterArbiter, GutterArbiterHost, GutterSlot, SlotElements};
use fragment_layers::overlay_host::{Disposer, OverlayHost, WidgetElements};
use fragment_layers::types::{
    ElementId, GutterBand, GutterCluster, GutterSide, LeafId, OverlayPlane, Point, ViewportBand,
};
use fragment_layers::view_overlays::{
    Changements, Ecart, GeometrieVue, MesuresPlans, Racine, coin_viewport,
};
use fragment_layers::widget_activation::{ActivationEntry, ElementTree, WidgetActivation};
use fragment_layers::widget_layer::WidgetBox;
use fragment_text::Rect;
use gpui_kit::{AnyElement, App, Window};

/// Ce qu'un élément DESSINE : rappelé à chaque image (et à chaque re-mesure dans l'image).
pub type Contenu = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// La fiche d'un élément du registre (lecture seule hors de l'hôte).
#[derive(Clone)]
pub struct Fiche {
    /// La racine où il est monté (`None` : pas monté).
    pub racine: Option<Racine>,
    /// Sa racine avant d'entrer dans la marge (rendue au retrait du slot).
    avant_marge: Option<Racine>,
    /// Transparent aux clics : seuls ses enfants captent.
    pub pass_through: bool,
    pub left: Option<f64>,
    pub right: Option<f64>,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    /// La largeur imposée par la bande de marge (`None` : sa largeur propre).
    pub largeur_imposee: Option<f64>,
    /// Affiché (`display`).
    pub affiche: bool,
    /// La place annoncée par l'arbitre (`--gutter-room`).
    pub place: Option<f64>,
    /// Membre d'une grappe : mesuré, pas dessiné.
    pub en_grappe: bool,
    /// La boîte MESURÉE à la dernière mise en page (px CSS).
    pub taille: (f64, f64),
    contenu: Option<Contenu>,
}

impl Default for Fiche {
    fn default() -> Self {
        Self {
            racine: None,
            avant_marge: None,
            pass_through: false,
            left: None,
            right: None,
            top: None,
            bottom: None,
            largeur_imposee: None,
            affiche: true,
            place: None,
            en_grappe: false,
            taille: (0.0, 0.0),
            contenu: None,
        }
    }
}

impl Fiche {
    /// Dessinée à cette image ? (montée, affichée, hors grappe)
    pub fn dessinee(&self) -> bool {
        self.racine.is_some() && self.affiche && !self.en_grappe
    }
}

#[derive(Default)]
struct Registre {
    fiches: BTreeMap<ElementId, Fiche>,
    parents: HashMap<ElementId, ElementId>,
    focus: Option<ElementId>,
    /// Les inscriptions à l'actif prises par `gutter_slot`, rendues avec le slot.
    entrees_marge: HashMap<ElementId, Vec<ActivationEntry>>,
    prochain: u64,
}

impl Registre {
    fn fiche(&mut self, el: ElementId) -> &mut Fiche {
        self.fiches.entry(el).or_default()
    }

    fn contient(&self, ancetre: ElementId, mut noeud: ElementId) -> bool {
        // Borné : un cycle de parents (erreur de l'appelant) ne bloque pas la boucle.
        for _ in 0..=self.parents.len() {
            if noeud == ancetre {
                return true;
            }
            match self.parents.get(&noeud) {
                Some(p) => noeud = *p,
                None => return false,
            }
        }
        false
    }
}

/// Ce que l'hôte partage avec les modèles qu'il branche (arbitre, actif).
struct Partage {
    registre: RefCell<Registre>,
    geometrie: RefCell<GeometrieVue>,
}

impl Partage {
    fn pane(&self) -> Option<Rect> {
        self.geometrie.borrow().mesures().map(|m| m.pane)
    }

    /// La boîte d'un élément dans le repère de SON plan (`offsetLeft/Top/Width/Height`).
    fn boite(&self, el: ElementId) -> WidgetBox {
        let f = self
            .registre
            .borrow()
            .fiches
            .get(&el)
            .cloned()
            .unwrap_or_default();
        let (left, top) = match (f.racine, self.pane()) {
            (Some(Racine::Viewport), Some(pane)) => {
                let c = coin_viewport(pane, f.taille, f.left, f.right, f.top, f.bottom);
                (c.x, c.y)
            }
            _ => (f.left.unwrap_or(0.0), f.top.unwrap_or(0.0)),
        };
        WidgetBox {
            left,
            top,
            width: f.taille.0,
            height: f.taille.1,
        }
    }

    /// Le coin d'un élément relatif au coin du pane (px CSS), ou `None` s'il n'est pas placé.
    fn coin_dans_le_pane(&self, el: ElementId) -> Option<Point> {
        let racine = self.registre.borrow().fiches.get(&el)?.racine?;
        let b = self.boite(el);
        self.geometrie
            .borrow()
            .position_dans_le_pane(racine, b.left, b.top)
    }
}

/// Les éléments vus des modèles (`SlotElements` pour l'arbitre, `ElementTree` pour l'actif).
struct Elements(Rc<Partage>);

impl SlotElements for Elements {
    fn is_displayed(&self, el: ElementId) -> bool {
        self.0
            .registre
            .borrow()
            .fiches
            .get(&el)
            .is_some_and(|f| f.racine.is_some() && f.affiche)
    }
    fn height(&self, el: ElementId) -> f64 {
        self.0
            .registre
            .borrow()
            .fiches
            .get(&el)
            .map_or(0.0, |f| f.taille.1)
    }
    fn set_top(&self, el: ElementId, top: f64) {
        self.0.registre.borrow_mut().fiche(el).top = Some(top);
    }
    fn set_room(&self, el: ElementId, room: Option<f64>) {
        self.0.registre.borrow_mut().fiche(el).place = room;
    }
    fn set_clustered(&self, el: ElementId, clustered: bool) {
        self.0.registre.borrow_mut().fiche(el).en_grappe = clustered;
    }
}

impl ElementTree for Elements {
    fn contains(&self, ancestor: ElementId, node: ElementId) -> bool {
        self.0.registre.borrow().contient(ancestor, node)
    }
}

type RappelGrappes = Rc<dyn Fn(GutterSide, &[GutterCluster])>;

/// Les présentateurs de grappes (la vue les relaie au Workspace).
#[derive(Default)]
struct Grappes {
    cbs: RefCell<Vec<(u64, RappelGrappes)>>,
}

impl GutterArbiterHost for Grappes {
    fn clusters_change(&self, side: GutterSide, clusters: Vec<GutterCluster>) {
        let cbs: Vec<_> = self
            .cbs
            .borrow()
            .iter()
            .map(|(_, c)| Rc::clone(c))
            .collect();
        for cb in cbs {
            cb(side, &clusters);
        }
    }
}

type Rappel = Rc<dyn Fn()>;

#[derive(Default)]
struct Rappels {
    geometrie: Vec<(u64, Rappel)>,
    viewport: Vec<(u64, Rappel)>,
    prochain: u64,
}

#[derive(Clone, Copy)]
enum Liste {
    Geometrie,
    Viewport,
}

impl Rappels {
    fn liste(&mut self, l: Liste) -> &mut Vec<(u64, Rappel)> {
        match l {
            Liste::Geometrie => &mut self.geometrie,
            Liste::Viewport => &mut self.viewport,
        }
    }
}

/// L'hôte des calques d'UNE vue. Partagé en `Rc` : les calques le tiennent par leur
/// `LayerContext`, l'élément de dessin par sa poignée.
pub struct HoteCalques {
    partage: Rc<Partage>,
    plan_document: bool,
    activation: WidgetActivation,
    arbitre: GutterArbiter,
    grappes: Rc<Grappes>,
    rappels: Rc<RefCell<Rappels>>,
    off_activation: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl HoteCalques {
    /// Un hôte. `plan_document` : la vue a un repère `coords_at_pos` (une vue sans éditeur
    /// n'héberge que des widgets viewport). `ecart` : l'écart colonne → widget de la vue
    /// (défaut `gutter_gap` ; PDF : 12 px).
    pub fn new(plan_document: bool, ecart: Ecart) -> Rc<Self> {
        let partage = Rc::new(Partage {
            registre: RefCell::new(Registre::default()),
            geometrie: RefCell::new(GeometrieVue::new(ecart)),
        });
        let elements = Rc::new(Elements(Rc::clone(&partage)));
        let activation = WidgetActivation::new(elements.clone());
        let grappes = Rc::new(Grappes::default());

        let p_haut = Rc::downgrade(&partage);
        let p_echelle = Rc::downgrade(&partage);
        let p_rendu = Rc::downgrade(&partage);
        let actif = activation.clone();
        let arbitre = GutterArbiter::new(elements)
            // Plafond de la place de l'actif : la hauteur visible du pane (px écran).
            .with_visible_height(move || {
                p_haut
                    .upgrade()
                    .and_then(|p| p.pane())
                    .map_or(f64::INFINITY, |r| r.bottom - r.top)
            })
            .with_active_el(move || actif.active_el())
            .with_host(grappes.clone())
            .with_wanted_scale(move || {
                p_echelle
                    .upgrade()
                    .map_or(1.0, |p| p.geometrie.borrow().echelle())
            })
            .with_on_slot_removed(move |el| {
                if let Some(p) = p_rendu.upgrade() {
                    rendre_slot(&p, el);
                }
            });

        // Un changement d'actif re-place la marge (l'actif y est épinglé).
        let arb = arbitre.clone();
        let off = activation.on_active_change(move |_| arb.invalidate());

        Rc::new(Self {
            partage,
            plan_document,
            activation,
            arbitre,
            grappes,
            rappels: Rc::new(RefCell::new(Rappels::default())),
            off_activation: RefCell::new(Some(off)),
        })
    }

    /// Un hôte de vue à éditeur, écart par défaut.
    pub fn pour_editeur() -> Rc<Self> {
        Self::new(true, Box::new(fragment_layers::gutter_stack::gutter_gap))
    }

    /// La vue a-t-elle un plan document ?
    pub fn a_un_plan_document(&self) -> bool {
        self.plan_document
    }

    // ── Le registre, côté présentation ──────────────────────────────────────

    /// Un nouvel élément (vide, non monté).
    pub fn nouvel_element(&self) -> ElementId {
        let mut r = self.partage.registre.borrow_mut();
        r.prochain += 1;
        let el = ElementId(r.prochain);
        r.fiches.insert(el, Fiche::default());
        el
    }

    /// Ce que l'élément dessine.
    pub fn poser_contenu(&self, el: ElementId, contenu: Contenu) {
        self.partage.registre.borrow_mut().fiche(el).contenu = Some(contenu);
    }

    /// Oublie l'élément (après son démontage).
    pub fn oublier_element(&self, el: ElementId) {
        let mut r = self.partage.registre.borrow_mut();
        r.fiches.remove(&el);
        r.parents.remove(&el);
        r.parents.retain(|_, p| *p != el);
    }

    /// Déclare `enfant` contenu dans `parent` (pour « contient » de l'actif et du focus).
    pub fn poser_parent(&self, enfant: ElementId, parent: ElementId) {
        self.partage
            .registre
            .borrow_mut()
            .parents
            .insert(enfant, parent);
    }

    /// L'élément qui a le focus clavier (`None` : aucun élément des calques).
    pub fn poser_focus(&self, el: Option<ElementId>) {
        self.partage.registre.borrow_mut().focus = el;
    }

    /// La fiche d'un élément (copie).
    pub fn fiche(&self, el: ElementId) -> Option<Fiche> {
        self.partage.registre.borrow().fiches.get(&el).cloned()
    }

    /// La largeur du pane que les plans couvrent (px CSS), une fois mesuré : une bande « de bord à
    /// bord » (la barre de l'agent) en a besoin, un `Viewport` n'ayant que la taille de son contenu.
    pub fn largeur_pane(&self) -> Option<f64> {
        self.partage.pane().map(|p| p.right - p.left)
    }

    /// Le coin d'un élément relatif au coin du pane (px CSS), s'il est placé.
    pub fn coin_dans_le_pane(&self, el: ElementId) -> Option<Point> {
        self.partage.coin_dans_le_pane(el)
    }

    /// Les éléments à dessiner, dans l'ordre : plan document (marge comprise), puis viewport.
    pub fn a_dessiner(&self) -> Vec<(ElementId, Contenu, bool, Option<f64>)> {
        let r = self.partage.registre.borrow();
        let mut doc = Vec::new();
        let mut vue = Vec::new();
        for (el, f) in &r.fiches {
            let (Some(racine), Some(c)) = (f.racine, &f.contenu) else {
                continue;
            };
            if !f.affiche {
                continue;
            }
            let e = (*el, Rc::clone(c), f.pass_through, f.largeur_imposee);
            if racine == Racine::Viewport {
                vue.push(e);
            } else {
                doc.push(e);
            }
        }
        doc.extend(vue);
        doc
    }

    /// Pose la boîte MESURÉE d'un élément ; vrai si elle a changé.
    pub fn poser_taille(&self, el: ElementId, taille: (f64, f64)) -> bool {
        let marge = {
            let mut r = self.partage.registre.borrow_mut();
            let f = r.fiche(el);
            if f.taille == taille {
                return false;
            }
            f.taille = taille;
            f.racine == Some(Racine::Marge)
        };
        // ★ Le ResizeObserver du TS (ajout de fr-t5-widgets) : un élément de marge qui change de
        //   taille replace la marge DANS la passe de cette image — sinon le recouvrement est peint.
        if marge {
            self.arbitre.invalidate();
        }
        true
    }

    // ── Les modèles branchés ────────────────────────────────────────────────

    /// L'actif des widgets de la vue (pointeur, focus : branchés par la présentation).
    pub fn activation(&self) -> &WidgetActivation {
        &self.activation
    }

    /// L'arbitre des marges de la vue.
    pub fn arbitre(&self) -> &GutterArbiter {
        &self.arbitre
    }

    /// La feuille hébergée par le widget actif (second étage de l'actif, côté vue).
    pub fn active_hosted_leaf(&self) -> Option<LeafId> {
        self.activation.active_leaf()
    }

    /// Prévenu à chaque changement d'actif.
    pub fn on_activation_change(&self, cb: impl Fn() + 'static) -> Disposer {
        self.activation.on_active_change(move |_| cb())
    }

    /// Les grappes d'un côté de la marge ont changé (la vue les relaie au Workspace).
    pub fn on_clusters_change(
        &self,
        cb: impl Fn(GutterSide, &[GutterCluster]) + 'static,
    ) -> Disposer {
        let mut cbs = self.grappes.cbs.borrow_mut();
        let id = cbs.iter().map(|(i, _)| *i).max().unwrap_or(0) + 1;
        cbs.push((id, Rc::new(cb)));
        let g = Rc::downgrade(&self.grappes);
        Box::new(move || {
            if let Some(g) = g.upgrade() {
                g.cbs.borrow_mut().retain(|(i, _)| *i != id);
            }
        })
    }

    // ── La passe d'une image ────────────────────────────────────────────────

    /// Une mesure des plans (après la mise en page, avant de placer) : prévient la géométrie
    /// et le viewport s'ils ont changé, puis fait tourner l'actif (le tour qui suit un
    /// relâché) et l'arbitre. Rend ce qui a changé.
    pub fn passe(&self, m: MesuresPlans) -> Changements {
        let mut m = m;
        if !self.plan_document {
            m.origine = None;
            m.colonne = None;
        }
        let ch = self.partage.geometrie.borrow_mut().poser_mesures(m);
        if ch.echelle_marge {
            self.arbitre.invalidate();
        }
        if ch.geometrie {
            self.prevenir(Liste::Geometrie);
        }
        if ch.viewport {
            self.prevenir(Liste::Viewport);
        }
        self.activation.run_timers();
        self.arbitre.run_microtasks();
        ch
    }

    /// Fin de la passe : les bandes de marge mémorisées ne valent plus.
    pub fn fin_de_passe(&self) {
        self.partage.geometrie.borrow_mut().oublier_bandes();
    }

    fn prevenir(&self, l: Liste) {
        let cbs: Vec<Rappel> = self
            .rappels
            .borrow_mut()
            .liste(l)
            .iter()
            .map(|(_, c)| Rc::clone(c))
            .collect();
        for cb in cbs {
            cb();
        }
    }

    fn abonner(&self, l: Liste, cb: Box<dyn Fn()>) -> Disposer {
        let mut r = self.rappels.borrow_mut();
        r.prochain += 1;
        let id = r.prochain;
        r.liste(l).push((id, Rc::from(cb)));
        let w: Weak<RefCell<Rappels>> = Rc::downgrade(&self.rappels);
        Box::new(move || {
            if let Some(r) = w.upgrade() {
                r.borrow_mut().liste(l).retain(|(i, _)| *i != id);
            }
        })
    }

    /// Défait les branchements (la vue se ferme).
    pub fn detruire(&self) {
        if let Some(off) = self.off_activation.borrow_mut().take() {
            off();
        }
        self.activation.destroy();
        self.arbitre.destroy();
        let mut r = self.rappels.borrow_mut();
        r.geometrie.clear();
        r.viewport.clear();
        drop(r);
        self.grappes.cbs.borrow_mut().clear();
        let entrees: Vec<ActivationEntry> = {
            let mut reg = self.partage.registre.borrow_mut();
            reg.fiches.clear();
            reg.entrees_marge.drain().flat_map(|(_, v)| v).collect()
        };
        for e in entrees {
            e.release();
        }
    }
}

/// Un slot de marge est rendu : l'inscription à l'actif est relâchée, l'élément revient à son
/// plan d'avant.
fn rendre_slot(p: &Partage, el: ElementId) {
    let entree = {
        let mut r = p.registre.borrow_mut();
        if let Some(f) = r.fiches.get_mut(&el)
            && f.racine == Some(Racine::Marge)
        {
            f.racine = Some(f.avant_marge.take().unwrap_or(Racine::Document));
        }
        r.entrees_marge.get_mut(&el).and_then(|v| v.pop())
    };
    if let Some(e) = entree {
        e.release();
    }
}

impl OverlayHost for HoteCalques {
    fn mount(&self, el: ElementId, plane: OverlayPlane) -> Disposer {
        {
            let mut r = self.partage.registre.borrow_mut();
            let f = r.fiche(el);
            let racine = Racine::from(plane);
            if f.racine == Some(Racine::Marge) {
                f.avant_marge = Some(racine);
            } else {
                f.racine = Some(racine);
            }
        }
        let p = Rc::downgrade(&self.partage);
        Box::new(move || {
            if let Some(p) = p.upgrade()
                && let Some(f) = p.registre.borrow_mut().fiches.get_mut(&el)
            {
                f.racine = None;
                f.avant_marge = None;
            }
        })
    }

    fn client_to_viewport(&self, x: f64, y: f64) -> Option<Point> {
        self.partage.geometrie.borrow().client_to_viewport(x, y)
    }

    fn client_to_document(&self, x: f64, y: f64) -> Option<Point> {
        self.partage.geometrie.borrow().client_to_document(x, y)
    }

    fn document_to_client(&self, x: f64, y: f64) -> Option<Point> {
        self.partage.geometrie.borrow().document_to_client(x, y)
    }

    fn gutter_band(&self, side: GutterSide) -> Option<GutterBand> {
        self.partage.geometrie.borrow_mut().gutter_band(side)
    }

    fn gutter_slot(&self, el: ElementId, side: GutterSide) -> GutterSlot {
        if self.plan_document {
            let mut r = self.partage.registre.borrow_mut();
            let f = r.fiche(el);
            if f.racine != Some(Racine::Marge) {
                f.avant_marge = f.racine;
                f.racine = Some(Racine::Marge);
            }
        }
        // Un élément de marge posé à la main (sans WidgetLayer) peut aussi être l'actif.
        let entree = self.activation.track(el);
        self.partage
            .registre
            .borrow_mut()
            .entrees_marge
            .entry(el)
            .or_default()
            .push(entree);
        self.arbitre.slot(el, side)
    }

    fn track_activation(&self, el: ElementId) -> ActivationEntry {
        self.activation.track(el)
    }

    fn on_geometry_change(&self, cb: Box<dyn Fn()>) -> Disposer {
        self.abonner(Liste::Geometrie, cb)
    }

    fn viewport(&self) -> Option<ViewportBand> {
        self.partage.geometrie.borrow().viewport()
    }

    fn on_viewport_change(&self, cb: Box<dyn Fn()>) -> Disposer {
        self.abonner(Liste::Viewport, cb)
    }

    fn content_client_rect(&self) -> Option<Rect> {
        self.partage.geometrie.borrow().mesures()?.colonne
    }
}

impl WidgetElements for HoteCalques {
    fn attach(&self, el: ElementId, pass_through: bool) {
        self.partage.registre.borrow_mut().fiche(el).pass_through = pass_through;
    }

    fn set_viewport_edges(
        &self,
        el: ElementId,
        left: Option<f64>,
        right: Option<f64>,
        top: Option<f64>,
        bottom: Option<f64>,
    ) {
        let mut r = self.partage.registre.borrow_mut();
        let f = r.fiche(el);
        (f.left, f.right, f.top, f.bottom) = (left, right, top, bottom);
    }

    fn clear_right_bottom(&self, el: ElementId) {
        let mut r = self.partage.registre.borrow_mut();
        let f = r.fiche(el);
        f.right = None;
        f.bottom = None;
    }

    fn set_left(&self, el: ElementId, left: f64) {
        self.partage.registre.borrow_mut().fiche(el).left = Some(left);
    }

    fn set_top(&self, el: ElementId, top: f64) {
        self.partage.registre.borrow_mut().fiche(el).top = Some(top);
    }

    fn impose_width(&self, el: ElementId, width: f64) {
        self.partage.registre.borrow_mut().fiche(el).largeur_imposee = Some(width);
    }

    fn restore_width(&self, el: ElementId) {
        self.partage.registre.borrow_mut().fiche(el).largeur_imposee = None;
    }

    fn set_displayed(&self, el: ElementId, displayed: bool) {
        self.partage.registre.borrow_mut().fiche(el).affiche = displayed;
    }

    fn offset_box(&self, el: ElementId) -> WidgetBox {
        self.partage.boite(el)
    }

    fn written_origin(&self, el: ElementId) -> (f64, f64) {
        self.partage
            .registre
            .borrow()
            .fiches
            .get(&el)
            .map_or((0.0, 0.0), |f| {
                (f.left.unwrap_or(0.0), f.top.unwrap_or(0.0))
            })
    }

    fn client_rect(&self, el: ElementId) -> Rect {
        let (Some(pane), Some(c)) = (self.partage.pane(), self.partage.coin_dans_le_pane(el))
        else {
            return Rect::default();
        };
        let b = self.partage.boite(el);
        let (left, top) = (pane.left + c.x, pane.top + c.y);
        Rect {
            left,
            top,
            right: left + b.width,
            bottom: top + b.height,
        }
    }

    fn contains_focus(&self, el: ElementId) -> bool {
        let r = self.partage.registre.borrow();
        r.focus.is_some_and(|f| r.contient(el, f))
    }
}
