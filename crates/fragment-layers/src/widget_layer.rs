// Port de app/src/ui/core/layers/WidgetLayer.ts (le MODÈLE : ancres, remappage,
// bande de marge, cycle de vie des widgets déclarés ; pas de DOM)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les widgets d'overlay — la moitié "interactive" de l'API layers. À côté des
//  markers (décoratifs) : un widget est un élément PERSISTANT, monté une fois,
//  déplaçable.
//
//  Cette structure est le COMPORTEMENT (positionnement, ancre, bascule). Elle ne
//  touche aucun rendu de la vue : elle passe par `OverlayHost` (plans, marges,
//  actif) et `WidgetElements` (styles et mesures des éléments).
//
//  Deux ancres :
//    - VIEWPORT : (x, y) fixes à l'écran. Marche SANS éditeur (vue PDF, image…).
//    - DOCUMENT : coords de `coords_at_pos(pos)` → suit le texte NATIVEMENT (le
//      plan document vit dans le scroller), recalé à l'édition. Exige un éditeur.
//    - GUTTER   : le niveau vient du texte, la colonne vient du pane.
//
//  ★ C'EST ICI que vivent les conversions de repère, parce que c'est le seul objet
//    qui possède les DEUX côtés (l'éditeur et les plans). Un widget appelant ne
//    manipule jamais un système de coordonnées : il donne un point client, il
//    reçoit une ancre.
//
//  ★ L'ANCRE VIT ICI, en un seul exemplaire. Elle est remappée à chaque édition
//    (un offset brut désigne un autre caractère dès qu'on tape au-dessus) ; toute
//    copie gardée ailleurs serait périmée au premier changement. D'où
//    `WidgetHandle::get_anchor` : on lit, on ne duplique pas.
//
//  ★ Transposition du temps et des événements (pas de DOM, pas de boucle ici) :
//      editor.onChange                → `on_editor_change` (l'hôte l'appelle)
//      onGeometryChange / onViewport… → abonnements pris sur l'`OverlayHost`
//      queueMicrotask (`revoir`)      → `run_microtasks`
//      requestAnimationFrame (essais) → `run_frame`
//      focusout de l'élément          → `focus_out`
//      toDOM asynchrone               → `DomBuild::Pending` puis `deliver`
//    Les `console.error` autour des rappels des plugins ne sont pas portés : une
//    panique de rappel remonte à l'hôte.
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use fragment_text::{
    Assoc, DocumentSurface, EditorChange, MappedPos, PosVisibility, Rect, TextAnchored,
    pos_visibility,
};

pub use fragment_text::{MappedAnchor, map_anchor};

use crate::gutter_arbiter::GutterSlot;
use crate::overlay_host::{Disposer, OverlayHost, WidgetElements};
use crate::types::{
    CompactInfo, ElementId, GutterBand, GutterMember, GutterSide, LeafId, OverlayPlane, Point,
};
use crate::widget_activation::ActivationEntry;

/// Le COIN du pane depuis lequel se mesure une ancre `Viewport` : `x` / `y` sont
/// les distances à ce coin (le cœur écrit `left` ou `right`, `top` ou `bottom`).
/// Défaut `TopLeft`. ★ Un widget collé en BAS (ou à droite) du pane le reste quand
/// le pane change de taille, sans une ligne de code : c'est la mise en page qui le
/// tient.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ViewportCorner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ViewportCorner {
    fn droite(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    fn bas(self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }
}

/// ★ Une ancre au texte est une PLAGE de positions de la SURFACE — `pos` (le
///   début) et `to` (la fin, absente = un point) —, comme une décoration de CM6.
///   Les positions sont OPAQUES : c'est la surface qui leur donne un sens (un
///   offset de note — unités UTF-16, ADR-004 —, une page de PDF, une part de
///   message) ; le cœur les TIENT et les REMAPPE (`map_range`, une seule règle),
///   le widget les LIT — il ne calcule ni n'écrit jamais la sienne.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WidgetAnchor {
    Viewport {
        x: f64,
        y: f64,
        /// Défaut : `TopLeft`.
        from: Option<ViewportCorner>,
    },
    Document {
        pos: usize,
        to: Option<usize>,
        dx: f64,
        dy: f64,
    },
    Gutter {
        side: GutterSide,
        pos: usize,
        to: Option<usize>,
        dy: f64,
    },
}

impl WidgetAnchor {
    /// Une ancre viewport depuis le coin haut-gauche.
    pub fn viewport(x: f64, y: f64) -> Self {
        Self::Viewport { x, y, from: None }
    }

    /// Cette ancre suit-elle le TEXTE ? (`suitLeTexte` : celles dont le `pos` doit
    /// être remappé.)
    pub fn suit_le_texte(&self) -> bool {
        !matches!(self, Self::Viewport { .. })
    }

    /// La position (début) d'une ancre au texte.
    pub fn pos(&self) -> Option<usize> {
        match *self {
            Self::Viewport { .. } => None,
            Self::Document { pos, .. } | Self::Gutter { pos, .. } => Some(pos),
        }
    }

    /// Le décalage vertical d'une ancre au texte.
    fn dy(&self) -> f64 {
        match *self {
            Self::Viewport { .. } => 0.0,
            Self::Document { dy, .. } | Self::Gutter { dy, .. } => dy,
        }
    }

    /// Une ancre de marge vit dans le PLAN DOCUMENT — c'est ce qui lui donne le
    /// suivi du texte et du scroll gratuitement. Elle ne s'en distingue que par la
    /// façon de calculer le `x` et la largeur.
    pub fn plane(&self) -> OverlayPlane {
        match self {
            Self::Viewport { .. } => OverlayPlane::Viewport,
            _ => OverlayPlane::Document,
        }
    }

    fn est_marge(&self) -> bool {
        matches!(self, Self::Gutter { .. })
    }
}

impl TextAnchored for WidgetAnchor {
    fn span(&self) -> Option<(usize, Option<usize>)> {
        match *self {
            Self::Viewport { .. } => None,
            Self::Document { pos, to, .. } | Self::Gutter { pos, to, .. } => Some((pos, to)),
        }
    }

    fn with_span(self, pos: usize, to: Option<usize>) -> Self {
        match self {
            Self::Viewport { .. } => self,
            Self::Document { dx, dy, .. } => Self::Document { pos, to, dx, dy },
            Self::Gutter { side, dy, .. } => Self::Gutter { side, pos, to, dy },
        }
    }
}

/// Les options d'un widget posé (`add_widget`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WidgetOptions {
    /// TRANSPARENT aux clics : l'élément ne capte rien lui-même, seuls ses enfants
    /// captent — une BANDE qui porte un contrôle au milieu laisse passer les clics
    /// à côté. L'activation suit toujours. Défaut : il capte.
    pub pass_through: bool,
}

/// En dessous, la marge n'est plus utilisable : le widget se masque.
pub const MIN_GUTTER_W: f64 = 60.0;
/// …sauf réduit à ses icônes (`set_compact`) : il tient jusque-là.
pub const MIN_COMPACT_W: f64 = 28.0;

/// Les essais de placement d'un widget de marge NEUF dont le texte n'est pas
/// mesurable (une image de plus, quelques fois).
const ESSAIS_MAX: u32 = 10;

/// La boîte d'un widget telle qu'elle est PLACÉE, dans le repère de son plan.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct WidgetBox {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

pub trait WidgetHandle {
    /// L'ancre COURANTE — remappée par les éditions. La source de vérité.
    fn get_anchor(&self) -> WidgetAnchor;
    fn set_anchor(&self, anchor: WidgetAnchor);
    /// Où le widget est RÉELLEMENT affiché (repère de son plan), ou `None` s'il est
    /// masqué. Peut différer de l'ancre : en marge, l'empilement le décale.
    fn get_box(&self) -> Option<WidgetBox>;
    /// Prévenu quand le widget est DÉPLACÉ par l'arbitre de la marge — y compris à
    /// cause d'un widget d'un AUTRE calque. Rend le désabonnement.
    fn on_box_change(&self, cb: Box<dyn Fn()>) -> Disposer;
    /// Marge seulement : la PLACE dont le widget dispose avant de gêner le suivant
    /// (px du plan ; `INFINITY` = illimitée ; `None` = pas en marge, ou masqué).
    fn get_room(&self) -> Option<f64>;
    /// Prévenu quand la place change, PENDANT l'arbitrage et avant la mesure.
    fn on_room_change(&self, cb: Box<dyn Fn(Option<f64>)>) -> Disposer;
    /// L'ACTIF de la vue : un seul widget actif, tous calques confondus.
    fn is_active(&self) -> bool;
    fn on_active_change(&self, cb: Box<dyn Fn(bool)>) -> Disposer;
    /// Active / désactive explicitement (seule voie en mode manuel).
    fn set_active(&self, active: bool);
    /// Manuel : un geste DANS le widget ne l'active pas (un drag de bouclier).
    fn set_activation_manual(&self, manual: bool);
    /// La feuille (détachée) que le widget héberge : quand il est actif, elle
    /// devient la feuille active HÉBERGÉE.
    fn set_activation_leaf(&self, leaf: Option<LeafId>);
    /// Marge : le widget se RÉDUIT à ses icônes (`info`) ou redevient entier
    /// (`None`). Réduit, il garde sa largeur propre, se colle au texte, et tient
    /// dans une marge trop étroite pour le reste. L'identité de `info` compte
    /// (`Rc::ptr_eq`) : la même, répétée, ne fait rien.
    fn set_compact(&self, info: Option<Rc<CompactInfo>>);
    /// Marge : la bande a-t-elle la largeur d'un widget ENTIER ? (Faux : seule la
    /// forme réduite y tient.) Hors marge : vrai.
    fn fits_gutter(&self) -> bool;
    fn on_fit_change(&self, cb: Box<dyn Fn(bool)>) -> Disposer;
    fn remove(&self);
}

/// Le résultat de `LazyWidget::to_dom`.
pub enum DomBuild {
    /// L'élément est là.
    Ready(ElementId),
    /// Asynchrone (le `Promise` du TS) : l'hôte rendra l'élément par
    /// `WidgetLayer::deliver` avec le ticket reçu.
    Pending,
    /// `toDOM` a levé : le widget n'existe pas (il sera retenté à la prochaine
    /// passe qui le voit dans le viewport).
    Failed,
}

/// Le jeton d'une construction : `deliver` le rend, ce qui permet d'annuler une
/// construction dépassée (le widget est sorti du viewport, retiré, reconstruit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildTicket(pub u64);

/// Un widget DÉCLARÉ — le `WidgetType` de CodeMirror 6 (`toDOM` / `destroy`).
///
/// ★ Le plugin ne monte ni ne démonte rien : il donne ce widget et son ancre, et
///   c'est `WidgetLayer` (le système) qui le CONSTRUIT quand son ancre entre dans
///   le viewport des calques et le DÉTRUIT quand elle en sort. Le HANDLE, lui, vit
///   toujours : l'ancre suit les éditions, qu'il y ait un élément ou non.
pub trait LazyWidget {
    fn to_dom(&self, handle: Rc<dyn WidgetHandle>, ticket: BuildTicket) -> DomBuild;
    /// Juste après que l'élément est posé dans le plan (placé, inscrit à l'actif).
    fn mounted(&self, _el: ElementId, _handle: Rc<dyn WidgetHandle>) {}
    fn destroy(&self, _el: ElementId) {}
}

/// Ce qu'on pose : un ÉLÉMENT (posé tel quel, seulement masqué hors du viewport)
/// ou un widget DÉCLARÉ.
pub enum WidgetSource {
    Element(ElementId),
    Lazy(Rc<dyn LazyWidget>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct WidgetId(u64);

type RappelBool = Rc<dyn Fn(bool)>;
type RappelUnit = Rc<dyn Fn()>;
type RappelRoom = Rc<dyn Fn(Option<f64>)>;

struct Widget {
    id: WidgetId,
    /// L'élément — `None` pour un widget déclaré qui n'existe pas (hors viewport).
    el: Option<ElementId>,
    anchor: WidgetAnchor,
    /// Le numéro de l'ancre : change à CHAQUE affectation (l'identité de l'objet
    /// ancre du TS, que `positionGutter` compare après ses rappels).
    ancre_gen: u64,
    /// La position courante de l'ancre, lue par le membre de grappe (jamais copiée).
    pos_courante: Rc<std::cell::Cell<usize>>,
    /// Retrait rendu par `OverlayHost::mount`.
    off: Option<Disposer>,
    /// Déclaré (construit / détruit par le viewport) ; `None` : un élément donné tel quel.
    lazy: Option<Rc<dyn LazyWidget>>,
    /// Le handle (rendu par `add_widget`) : passé à `to_dom` / `mounted`.
    handle: Option<Rc<dyn WidgetHandle>>,
    /// Une construction en cours (`to_dom` asynchrone) : son jeton, pour l'annuler.
    jeton: u64,
    construction: bool,
    /// Le système le DÉTRUIT (sortie du viewport) : un `remove` venu de son
    /// `destroy` est ignoré.
    en_destruction: bool,
    /// Ce qu'on a demandé à l'actif avant que l'élément existe : rejoué à sa naissance.
    manuel: bool,
    feuille: Option<LeafId>,
    actif_cbs: Vec<(u64, RappelBool)>,
    off_actif: Option<Disposer>,
    /// Une largeur imposée par la bande est en place (à rendre en quittant la marge).
    largeur_imposee: bool,
    /// Marge seulement : la place de ce widget chez l'arbitre de la vue.
    slot: Option<GutterSlot>,
    /// Les abonnés de `on_box_change` — survivent aux changements de slot.
    box_cbs: Vec<(u64, RappelUnit)>,
    /// Les abonnés de `on_room_change` — survivent aussi aux changements de slot.
    room_cbs: Vec<(u64, RappelRoom)>,
    off_slot: Option<Disposer>,
    /// Son inscription à l'actif de la vue (`None` tant qu'il n'existe pas).
    activation: Option<ActivationEntry>,
    /// Marge : tentatives de placement d'un widget neuf dont le texte n'était pas mesurable.
    essais: u32,
    /// Sa forme réduite, s'il s'y est réduit — telle que la marge la voit (+ son ancre).
    compact: Option<Rc<GutterMember>>,
    /// La forme réduite telle que le widget l'a déclarée (identité).
    info: Option<Rc<CompactInfo>>,
    /// Marge : la bande accueille-t-elle le widget entier ? (`None` : jamais placé en marge)
    fits: Option<bool>,
    fit_cbs: Vec<(u64, RappelBool)>,
    /// Transparent aux clics (`WidgetOptions::pass_through`).
    pass_through: bool,
    /// Masqué (`display: none`) par ce calque.
    masque: bool,
}

/// La PASSE de repositionnement (le cycle de mesure de CM6 : lire, puis écrire) :
/// pendant `reposition_texte`, les coordonnées des ancres et les bandes de marge
/// sont LUES une fois, en tête, puis chaque widget écrit sa position.
#[derive(Default)]
struct Passe {
    coords: HashMap<usize, Option<Rect>>,
    bandes: HashMap<usize, Option<GutterBand>>,
}

fn cote(side: GutterSide) -> usize {
    match side {
        GutterSide::Left => 0,
        GutterSide::Right => 1,
    }
}

#[derive(Default)]
struct State {
    widgets: Vec<Widget>,
    prochain: u64,
    off_geometry: Option<Disposer>,
    off_viewport: Option<Disposer>,
    passe: Option<Passe>,
    /// Les `revoir` en attente (microtâche) : le widget et l'élément qu'il avait.
    revoir: Vec<(WidgetId, ElementId)>,
    /// Les essais en attente de la prochaine image : le widget et le numéro d'ancre.
    images: Vec<(WidgetId, u64)>,
}

impl State {
    fn numero(&mut self) -> u64 {
        self.prochain += 1;
        self.prochain
    }

    fn widget(&mut self, id: WidgetId) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|w| w.id == id)
    }
}

struct Layer {
    editor: Option<Rc<dyn DocumentSurface>>,
    overlays: Rc<dyn OverlayHost>,
    elements: Rc<dyn WidgetElements>,
    st: RefCell<State>,
    moi: Weak<Layer>,
}

/// Le calque de widgets d'une vue. Clonable : une poignée sur un même calque.
#[derive(Clone)]
pub struct WidgetLayer {
    layer: Rc<Layer>,
}

/// La poignée rendue par `add_widget`.
struct Handle {
    layer: Weak<Layer>,
    id: WidgetId,
}

impl WidgetLayer {
    /// Un calque sur `editor` (`None` : une vue sans surface — PDF scanné, image —
    /// où seules les ancres viewport existent) et les overlays de la vue.
    pub fn new(
        editor: Option<Rc<dyn DocumentSurface>>,
        overlays: Rc<dyn OverlayHost>,
        elements: Rc<dyn WidgetElements>,
    ) -> Self {
        let layer = Rc::new_cyclic(|moi| Layer {
            editor,
            overlays,
            elements,
            st: RefCell::new(State::default()),
            moi: moi.clone(),
        });
        // Les widgets ancrés au texte se recalent quand le texte change (édition
        // au-dessus → coords_at_pos bouge) ou reflue (`on_editor_change`). Sans
        // éditeur (PDF), aucun ancrage au texte → pas d'abonnement.
        if layer.editor.is_some() {
            // Les MARGES dépendent de la largeur du pane — que l'éditeur n'a aucune
            // raison de signaler. C'est le seul abonnement géométrique légitime.
            let (m1, m2) = (layer.moi.clone(), layer.moi.clone());
            let off_geometry = layer.overlays.on_geometry_change(Box::new(move || {
                if let Some(l) = m1.upgrade() {
                    l.reposition_texte();
                }
            }));
            // Le VIEWPORT des calques a changé : ce qui y entre s'affiche, ce qui en
            // sort se cache.
            let off_viewport = layer.overlays.on_viewport_change(Box::new(move || {
                if let Some(l) = m2.upgrade() {
                    l.reposition_texte();
                }
            }));
            let mut st = layer.st.borrow_mut();
            st.off_geometry = Some(off_geometry);
            st.off_viewport = Some(off_viewport);
        }
        Self { layer }
    }

    /// L'hôte des overlays de la vue, celui que ce calque a reçu — en LECTURE.
    pub fn overlays(&self) -> &Rc<dyn OverlayHost> {
        &self.layer.overlays
    }

    /// Pose un widget : un ÉLÉMENT (posé tel quel, seulement masqué hors du
    /// viewport) ou un widget DÉCLARÉ (construit / détruit par le viewport, comme
    /// une décoration de CM6).
    pub fn add_widget(
        &self,
        source: WidgetSource,
        anchor: WidgetAnchor,
        options: WidgetOptions,
    ) -> Rc<dyn WidgetHandle> {
        self.layer.add_widget(source, anchor, options)
    }

    /// Ce qui a bougé dans l'éditeur (l'abonnement `editor.onChange` du TS, que
    /// l'hôte relaie).
    pub fn on_editor_change(&self, c: &EditorChange) {
        if self.layer.editor.is_none() {
            return;
        }
        if c.doc_changed {
            self.layer.remap_anchors(&*c.map_pos);
        }
        if c.doc_changed || c.viewport_changed {
            self.layer.reposition_texte();
        }
    }

    /// `focusout` de l'élément d'un widget : s'il est hors du viewport, il n'a plus
    /// de raison d'exister (CM retire le viewport du curseur à la mise à jour
    /// suivante) — revu à la prochaine microtâche.
    pub fn focus_out(&self, el: ElementId) {
        let mut st = self.layer.st.borrow_mut();
        if let Some(id) = st.widgets.iter().find(|w| w.el == Some(el)).map(|w| w.id) {
            st.revoir.push((id, el));
        }
    }

    /// Des microtâches sont-elles en attente ? (`revoir`)
    pub fn needs_microtasks(&self) -> bool {
        !self.layer.st.borrow().revoir.is_empty()
    }

    /// Le point de contrôle de microtâches : rejoue les `revoir` en attente.
    pub fn run_microtasks(&self) {
        loop {
            let revoir = std::mem::take(&mut self.layer.st.borrow_mut().revoir);
            if revoir.is_empty() {
                break;
            }
            for (id, el) in revoir {
                let ok = self
                    .layer
                    .st
                    .borrow_mut()
                    .widget(id)
                    .is_some_and(|w| w.el == Some(el));
                if ok {
                    self.layer.position(id);
                }
            }
        }
    }

    /// Des essais attendent-ils la prochaine image ?
    pub fn needs_frame(&self) -> bool {
        !self.layer.st.borrow().images.is_empty()
    }

    /// L'image suivante (`requestAnimationFrame`) : retente le placement d'un
    /// widget de marge neuf dont le texte n'était pas mesurable.
    pub fn run_frame(&self) {
        let images = std::mem::take(&mut self.layer.st.borrow_mut().images);
        for (id, gen_ancre) in images {
            let ok = self
                .layer
                .st
                .borrow_mut()
                .widget(id)
                .is_some_and(|w| w.ancre_gen == gen_ancre);
            if ok {
                self.layer.position(id);
            }
        }
    }

    /// Un widget déclaré a fini sa construction asynchrone (`DomBuild::Pending`).
    /// Rend `false` si la construction est dépassée (widget sorti du viewport,
    /// retiré, reconstruit) : l'hôte doit alors lui-même défaire l'élément.
    pub fn deliver(
        &self,
        widget: &Rc<dyn WidgetHandle>,
        ticket: BuildTicket,
        el: ElementId,
    ) -> bool {
        // La poignée est la nôtre : on retrouve son identifiant par son adresse.
        let id = {
            let st = self.layer.st.borrow();
            st.widgets
                .iter()
                .find(|w| {
                    w.handle
                        .as_ref()
                        .is_some_and(|h| std::ptr::addr_eq(Rc::as_ptr(h), Rc::as_ptr(widget)))
                })
                .map(|w| w.id)
        };
        match id {
            Some(id) => self.layer.poser(id, ticket.0, el),
            None => false,
        }
    }

    /// Point client → coords viewport (ou `None` hors du pane) — pour le drop-to-spawn.
    pub fn client_to_viewport(&self, x: f64, y: f64) -> Option<Point> {
        self.layer.overlays.client_to_viewport(x, y)
    }

    /// L'ancre DOCUMENT qui laisse l'objet EXACTEMENT où il est à l'écran.
    ///
    /// Le piège que ça referme : `pos_at_coords` rend l'offset du caractère le plus
    /// PROCHE, et `coords_at_pos` le coin de CE glyphe — s'ancrer avec dx/dy = 0
    /// aimante donc sur le glyphe et fait sauter l'objet d'une hauteur de ligne. On
    /// garde l'écart, mesuré dans le repère document des deux côtés.
    pub fn document_anchor_at(&self, client_x: f64, client_y: f64) -> Option<WidgetAnchor> {
        let editor = self.layer.editor.as_ref()?;
        let pos = editor.pos_at_coords(client_x, client_y)?;
        let glyph = editor.coords_at_pos(pos)?;
        let point = self.layer.overlays.client_to_document(client_x, client_y)?;
        Some(WidgetAnchor::Document {
            pos,
            to: None,
            dx: point.x - glyph.left,
            dy: point.y - glyph.top,
        })
    }

    /// Le symétrique : l'ancre VIEWPORT à la même place à l'écran.
    pub fn viewport_anchor_at(&self, client_x: f64, client_y: f64) -> Option<WidgetAnchor> {
        self.layer.viewport_anchor_at(client_x, client_y)
    }

    /// L'ancre de MARGE qui garde l'objet au même NIVEAU vertical qu'à l'écran.
    ///
    /// Une ancre de marge ne retient qu'un `y` : le `x` et la largeur sont imposés
    /// par la bande à chaque placement, puisqu'ils dépendent du pane. On sonde donc
    /// le texte au MILIEU de la colonne — pas sous le widget, qui est justement à
    /// côté du texte, voire hors de la zone rendue.
    pub fn gutter_anchor_at(&self, client_y: f64, side: GutterSide) -> Option<WidgetAnchor> {
        let editor = self.layer.editor.as_ref()?;
        let contenu = self.layer.overlays.content_client_rect()?;
        let x = contenu.left + (contenu.right - contenu.left) / 2.0;
        let pos = editor.pos_at_coords(x, client_y)?;
        let glyph = editor.coords_at_pos(pos)?;
        let point = self.layer.overlays.client_to_document(x, client_y)?;
        Some(WidgetAnchor::Gutter {
            side,
            pos,
            to: None,
            dy: point.y - glyph.top,
        })
    }

    /// Y a-t-il la place d'afficher un widget dans cette marge ?
    pub fn gutter_fits(&self, side: GutterSide) -> bool {
        self.layer
            .overlays
            .gutter_band(side)
            .is_some_and(|b| b.width >= MIN_GUTTER_W)
    }

    /// L'ancrage document est-il possible ici ? (Faux sans éditeur : PDF, image…)
    pub fn can_anchor_to_document(&self) -> bool {
        self.layer.editor.is_some()
    }

    pub fn destroy(&self) {
        let (offs, ids) = {
            let mut st = self.layer.st.borrow_mut();
            let offs = [st.off_geometry.take(), st.off_viewport.take()];
            (offs, st.widgets.iter().map(|w| w.id).collect::<Vec<_>>())
        };
        for off in offs.into_iter().flatten() {
            off();
        }
        for id in ids {
            self.layer.detacher(id);
        }
        self.layer.st.borrow_mut().widgets.clear();
    }
}

impl Layer {
    fn add_widget(
        &self,
        source: WidgetSource,
        anchor: WidgetAnchor,
        options: WidgetOptions,
    ) -> Rc<dyn WidgetHandle> {
        let (lazy, element) = match source {
            WidgetSource::Element(el) => (None, Some(el)),
            WidgetSource::Lazy(l) => (Some(l), None),
        };
        let id = {
            let mut st = self.st.borrow_mut();
            let id = WidgetId(st.numero());
            let ancre_gen = st.numero();
            st.widgets.push(Widget {
                id,
                el: None,
                anchor,
                ancre_gen,
                pos_courante: Rc::new(std::cell::Cell::new(anchor.pos().unwrap_or(0))),
                off: None,
                lazy,
                handle: None,
                jeton: 0,
                construction: false,
                en_destruction: false,
                manuel: false,
                feuille: None,
                actif_cbs: Vec::new(),
                off_actif: None,
                largeur_imposee: false,
                slot: None,
                box_cbs: Vec::new(),
                room_cbs: Vec::new(),
                off_slot: None,
                activation: None,
                essais: 0,
                compact: None,
                info: None,
                fits: None,
                fit_cbs: Vec::new(),
                pass_through: options.pass_through,
                masque: false,
            });
            id
        };
        let handle: Rc<dyn WidgetHandle> = Rc::new(Handle {
            layer: self.moi.clone(),
            id,
        });
        if let Some(w) = self.st.borrow_mut().widget(id) {
            w.handle = Some(Rc::clone(&handle));
        }
        if let Some(el) = element {
            self.attacher(id, el);
        }
        self.position(id);
        handle
    }

    /// Lit un champ d'un widget (ou `None` s'il n'existe plus).
    fn lire<R>(&self, id: WidgetId, f: impl FnOnce(&Widget) -> R) -> Option<R> {
        self.st.borrow().widgets.iter().find(|w| w.id == id).map(f)
    }

    fn ecrire<R>(&self, id: WidgetId, f: impl FnOnce(&mut Widget) -> R) -> Option<R> {
        self.st.borrow_mut().widget(id).map(f)
    }

    /// Pose l'élément dans son plan et l'inscrit à l'actif (un élément donné, ou né de `to_dom`).
    fn attacher(&self, id: WidgetId, el: ElementId) {
        let Some((pass_through, plane, manuel, feuille)) = self.lire(id, |w| {
            (w.pass_through, w.anchor.plane(), w.manuel, w.feuille)
        }) else {
            return;
        };
        // Les plans sont sans clic ; le widget, lui, capte — sauf transparent.
        self.elements.attach(el, pass_through);
        let off = self.overlays.mount(el, plane);
        let activation = self.overlays.track_activation(el);
        if manuel {
            activation.set_manual(true);
        }
        if feuille.is_some() {
            activation.set_leaf(feuille);
        }
        // Relâché (plus actif, focus sorti) : s'il est hors du viewport, il n'a plus
        // de raison d'exister (CM retire le viewport du curseur à la mise à jour suivante).
        let moi = self.moi.clone();
        let off_actif = activation.on_change(move |a| {
            let Some(layer) = moi.upgrade() else { return };
            let cbs: Vec<RappelBool> = layer
                .lire(id, |w| {
                    w.actif_cbs.iter().map(|(_, c)| Rc::clone(c)).collect()
                })
                .unwrap_or_default();
            for cb in cbs {
                cb(a);
            }
            if !a {
                layer.st.borrow_mut().revoir.push((id, el));
            }
        });
        self.ecrire(id, |w| {
            w.el = Some(el);
            w.off = Some(off);
            w.activation = Some(activation);
            w.off_actif = Some(off_actif);
        });
    }

    /// Retire l'élément du plan, de la marge et de l'actif ; un widget déclaré est DÉTRUIT.
    fn detacher(&self, id: WidgetId) {
        let Some((el, off, off_actif, activation, lazy)) = self
            .ecrire(id, |w| {
                w.jeton += 1;
                w.construction = false;
                let el = w.el?;
                let off = w.off.take();
                let off_actif = w.off_actif.take();
                let activation = w.activation.take();
                w.largeur_imposee = false;
                w.fits = None;
                w.el = None;
                Some((el, off, off_actif, activation, w.lazy.clone()))
            })
            .flatten()
        else {
            return;
        };
        if let Some(off) = off {
            off();
        }
        self.liberer_slot(id);
        if let Some(off) = off_actif {
            off();
        }
        if let Some(a) = activation {
            a.release();
        }
        if let Some(lazy) = lazy {
            self.detruire(id, &*lazy, el);
        }
    }

    /// `destroy` d'un widget déclaré : un `remove` venu de lui est ignoré pendant.
    fn detruire(&self, id: WidgetId, lazy: &dyn LazyWidget, el: ElementId) {
        self.ecrire(id, |w| w.en_destruction = true);
        lazy.destroy(el);
        self.ecrire(id, |w| w.en_destruction = false);
    }

    /// Construit un widget déclaré entré dans le viewport (CM : `WidgetView.of` →
    /// `toDOM`). Asynchrone possible : s'il est sorti ou retiré entre-temps, il est
    /// rendu aussitôt (`destroy`).
    fn construire(&self, id: WidgetId) {
        let Some((lazy, handle, ticket)) = self
            .ecrire(id, |w| {
                let (Some(lazy), Some(handle)) = (w.lazy.clone(), w.handle.clone()) else {
                    return None;
                };
                if w.el.is_some() || w.construction {
                    return None;
                }
                w.construction = true;
                w.jeton += 1;
                Some((lazy, handle, w.jeton))
            })
            .flatten()
        else {
            return;
        };
        match lazy.to_dom(handle, BuildTicket(ticket)) {
            DomBuild::Ready(el) => {
                self.poser(id, ticket, el);
            }
            DomBuild::Pending => {}
            DomBuild::Failed => {
                self.ecrire(id, |w| {
                    if w.jeton == ticket {
                        w.construction = false;
                    }
                });
            }
        }
    }

    /// L'élément d'une construction est arrivé : pose-le, ou défais-le si elle est dépassée.
    fn poser(&self, id: WidgetId, ticket: u64, el: ElementId) -> bool {
        let Some((courant, lazy, handle)) =
            self.lire(id, |w| (w.jeton, w.lazy.clone(), w.handle.clone()))
        else {
            return false;
        };
        let (Some(lazy), Some(handle)) = (lazy, handle) else {
            return false;
        };
        if ticket != courant {
            self.detruire(id, &*lazy, el);
            return false;
        }
        self.ecrire(id, |w| w.construction = false);
        self.attacher(id, el);
        lazy.mounted(el, handle);
        self.position(id);
        true
    }

    fn viewport_anchor_at(&self, client_x: f64, client_y: f64) -> Option<WidgetAnchor> {
        let pt = self.overlays.client_to_viewport(client_x, client_y)?;
        Some(WidgetAnchor::viewport(pt.x, pt.y))
    }

    /// Change l'ancre d'un widget monté ; bascule de plan si le plan change.
    fn reanchor(&self, id: WidgetId, a: WidgetAnchor) {
        let Some((el, ancienne)) = self.lire(id, |w| (w.el, w.anchor)) else {
            return;
        };
        if let Some(el) = el
            && a.plane() != ancienne.plane()
        {
            let off = self.ecrire(id, |w| w.off.take()).flatten();
            if let Some(off) = off {
                off();
            }
            let nouveau = self.overlays.mount(el, a.plane());
            self.ecrire(id, |w| w.off = Some(nouveau));
        }
        // On quitte la marge : le widget récupère la largeur qu'il s'était donnée.
        if ancienne.est_marge() && !a.est_marge() {
            self.rendre_largeur(id);
            self.annoncer_fit(id, true);
        }
        // Changement de marge (ou sortie) : on rend la place à l'arbitre.
        let change_de_cote = match (ancienne, a) {
            (
                WidgetAnchor::Gutter { side: avant, .. },
                WidgetAnchor::Gutter { side: apres, .. },
            ) => avant != apres,
            (WidgetAnchor::Gutter { .. }, _) => true,
            _ => false,
        };
        if change_de_cote {
            self.liberer_slot(id);
        }
        self.affecter_ancre(id, a);
        self.position(id);
    }

    fn affecter_ancre(&self, id: WidgetId, a: WidgetAnchor) {
        let mut st = self.st.borrow_mut();
        let gen_ancre = st.numero();
        if let Some(w) = st.widget(id) {
            w.anchor = a;
            w.ancre_gen = gen_ancre;
            if let Some(p) = a.pos() {
                w.pos_courante.set(p);
            }
        }
    }

    /// Rejoue les ancres document à travers un changement de document. Sans ça,
    /// l'offset stocké désigne un autre caractère dès la première frappe en amont
    /// et le widget dérive.
    ///
    /// Texte ancré supprimé : on DÉTACHE en viewport, à la place qu'occupe le widget
    /// à l'écran. Il ne disparaît pas et ne se téléporte pas — l'utilisateur voit ce
    /// qu'il a détruit. (Fermer le widget serait l'autre lecture défendable ; c'est
    /// une décision produit, isolée ici.) Si le widget est hors du pane, on ne peut
    /// pas le détacher « sur place » : il garde une ancre document, au point
    /// d'effondrement — toujours valide, jamais hors bornes.
    fn remap_anchors(&self, map_pos: &dyn Fn(usize, Assoc) -> MappedPos) {
        let ids: Vec<WidgetId> = self.st.borrow().widgets.iter().map(|w| w.id).collect();
        for id in ids {
            let Some((anchor, el)) = self.lire(id, |w| (w.anchor, w.el)) else {
                continue;
            };
            if !anchor.suit_le_texte() {
                continue;
            }
            let m = map_anchor(anchor, map_pos);
            self.affecter_ancre(id, m.anchor);
            // Un widget déclaré qui n'existe pas garde son ancre au point d'effondrement.
            let Some(el) = el.filter(|_| m.deleted) else {
                continue;
            };
            let r = self.elements.client_rect(el);
            if let Some(fallback) = self.viewport_anchor_at(r.left, r.top) {
                self.reanchor(id, fallback);
            }
        }
    }

    fn position(&self, id: WidgetId) {
        let Some((anchor, el, lazy)) = self.lire(id, |w| (w.anchor, w.el, w.lazy.is_some())) else {
            return;
        };
        // ★ Un widget DÉCLARÉ naît dans le viewport et y meurt en sortant (CM6) —
        //   sauf l'actif ou celui qui a le focus (CM garde la ligne du curseur).
        //   Sans éditeur, ancré à l'écran : toujours construit.
        if lazy {
            let dehors = match (&self.editor, anchor.pos()) {
                (Some(editor), Some(pos)) => {
                    pos_visibility(&**editor, pos) == PosVisibility::Hidden
                        || self.hors_viewport(id, &anchor)
                }
                _ => false,
            };
            if dehors {
                if el.is_some() {
                    self.detacher(id);
                }
                return;
            }
            if el.is_none() {
                self.construire(id);
                return;
            }
        }
        let Some(el) = el else { return };
        if let WidgetAnchor::Viewport { x, y, from } = anchor {
            let coin = from.unwrap_or_default();
            let (droite, bas) = (coin.droite(), coin.bas());
            self.elements.set_viewport_edges(
                el,
                (!droite).then_some(x),
                droite.then_some(x),
                (!bas).then_some(y),
                bas.then_some(y),
            );
            return;
        }
        // Une ancre au texte pose left / top : rien de ce qu'un coin viewport aurait laissé.
        self.elements.clear_right_bottom(el);
        let (Some(editor), Some(pos)) = (&self.editor, anchor.pos()) else {
            return;
        };

        // ★ LE MASQUAGE PASSE EN PREMIER, et c'est la seule règle de priorité de
        //   cette méthode. Deux raisons peuvent cacher un widget — son texte est
        //   replié, ou sa marge est trop étroite. Si chacune décidait dans son coin,
        //   la dernière règle exécutée gagnerait, au hasard de l'ordre des signaux.
        //   Ici l'ordre est explicite : contenu caché ⇒ widget caché, quoi qu'en
        //   dise la marge.
        if pos_visibility(&**editor, pos) == PosVisibility::Hidden
            || self.hors_viewport(id, &anchor)
        {
            self.masquer(id, el);
            return;
        }

        if let WidgetAnchor::Gutter { side, dy, .. } = anchor {
            self.position_gutter(id, side, pos, dy);
            return;
        }

        // Le texte est ressorti d'un repli : on rend le widget à la vue.
        self.afficher(id, el);

        // Ancre hors du viewport RENDU (CM virtualise) → pas de coords, on ne touche
        // à rien. C'est volontaire : les coords du plan sont absolues dans le
        // scroller, donc la position gardée reste juste tant que le texte ne bouge
        // pas, et une édition survenue pendant ce temps est rattrapée au retour à
        // l'écran, avant que le widget soit visible. Mesuré : recalculer le Y depuis
        // la carte des hauteurs est PIRE — son estimation hors écran s'écarte plus
        // (≈29px) que la dérive à corriger.
        let Some(r) = self.coords(pos) else { return };
        if let WidgetAnchor::Document { dx, dy, .. } = anchor {
            self.elements.set_left(el, r.left + dx);
            self.elements.set_top(el, r.top + dy);
        }
    }

    fn masquer(&self, id: WidgetId, el: ElementId) {
        self.elements.set_displayed(el, false);
        let slot = self
            .ecrire(id, |w| {
                w.masque = true;
                w.slot.clone()
            })
            .flatten();
        if let Some(slot) = slot {
            slot.set_wanted(None);
        }
    }

    fn afficher(&self, id: WidgetId, el: ElementId) {
        self.elements.set_displayed(el, true);
        self.ecrire(id, |w| w.masque = false);
    }

    /// Un widget de marge : le NIVEAU vient du texte, la COLONNE vient du pane.
    ///
    /// C'est tout le contrat de ce mode — et c'est pour ça que la largeur est
    /// réécrite ici à chaque placement plutôt que laissée au widget : elle est une
    /// conséquence de l'ancre, pas un réglage. Marge trop étroite (fenêtre
    /// rétrécie, colonne qui remplit le pane) → on masque, plutôt que de laisser
    /// un widget déborder sur le texte ou sortir du pane.
    ///
    /// ★ RÉDUIT à ses icônes (`set_compact`), le widget garde sa largeur et se colle
    ///   au texte : il tient dans une bande trop étroite pour le reste. La bande
    ///   étroite est ANNONCÉE (`on_fit_change`) avant le masquage : le widget peut
    ///   s'y réduire — l'appel le replace aussitôt, on s'arrête donc là s'il l'a fait.
    fn position_gutter(&self, id: WidgetId, side: GutterSide, pos: usize, dy: f64) {
        let Some((el, gen_avant, compact_avant)) =
            self.lire(id, |w| (w.el, w.ancre_gen, w.compact.clone()))
        else {
            return;
        };
        let Some(el) = el else { return };
        let band = self.bande(side);
        self.annoncer_fit(id, band.is_some_and(|b| b.width >= MIN_GUTTER_W));
        let Some((gen_apres, compact_apres)) = self.lire(id, |w| (w.ancre_gen, w.compact.clone()))
        else {
            return;
        };
        if !memes_membres(&compact_apres, &compact_avant) || gen_apres != gen_avant {
            return;
        }
        let compact = compact_apres.is_some();
        let min = if compact { MIN_COMPACT_W } else { MIN_GUTTER_W };
        let Some(band) = band.filter(|b| b.width >= min) else {
            self.masquer(id, el);
            return;
        };

        let Some(r) = self.coords(pos) else {
            // Hors du viewport rendu : on garde la dernière position. ★ Mais un widget
            // NEUF n'en a pas — il n'entrerait dans l'arbitrage qu'au prochain
            // changement de l'éditeur, et pourrait d'ici là chevaucher les autres
            // (coords_at_pos rend None pendant une mesure) : on réessaie à l'image
            // suivante, quelques fois.
            let mut st = self.st.borrow_mut();
            let reessai = st.widget(id).and_then(|w| {
                if w.slot.is_none() && w.essais < ESSAIS_MAX {
                    w.essais += 1;
                    Some(w.ancre_gen)
                } else {
                    None
                }
            });
            if let Some(ancre) = reessai {
                st.images.push((id, ancre));
            }
            return;
        };

        self.afficher(id, el);
        if compact {
            // Réduit : sa largeur à lui, au MÊME bord gauche que déplié — ses icônes
            // (coin haut-gauche) ne bougent pas d'un pixel à la bascule.
            self.rendre_largeur(id);
            self.elements.set_left(el, band.left);
        } else {
            self.ecrire(id, |w| w.largeur_imposee = true);
            self.elements.set_left(el, band.left);
            self.elements.impose_width(el, band.width);
        }
        // Le HAUT n'est pas écrit ici : c'est l'arbitre de la vue qui le pose, en
        // tenant compte des widgets de TOUS les calques (`GutterArbiter`).
        self.slot_de(id, el, side).set_wanted(Some(r.top + dy));
    }

    /// ★ Hors du VIEWPORT des calques (`OverlayHost::viewport`, celui de CM6) : le
    /// widget n'existe pas (déclaré : détruit) ou n'est pas affiché (un élément
    /// donné tel quel : il n'a pas de `to_dom` pour renaître) — comme CM ne dessine
    /// pas une ligne hors du sien. Il ne compte plus dans la marge ni dans la mise
    /// en page. Une ancre sans coordonnée HORS du viewport de l'éditeur (CM ne
    /// mesure que le sien) est dehors. L'ACTIF et celui qui a le FOCUS restent (CM
    /// garde la ligne du curseur dessinée).
    fn hors_viewport(&self, id: WidgetId, a: &WidgetAnchor) -> bool {
        let (Some(editor), Some(pos)) = (&self.editor, a.pos()) else {
            return false;
        };
        let Some((actif, el)) = self.lire(id, |w| {
            (
                w.activation
                    .as_ref()
                    .is_some_and(ActivationEntry::is_active),
                w.el,
            )
        }) else {
            return false;
        };
        if actif {
            return false;
        }
        if el.is_some_and(|el| self.elements.contains_focus(el)) {
            return false;
        }
        let Some(v) = self.overlays.viewport() else {
            return false;
        };
        let Some(r) = self.coords(pos) else {
            return pos_visibility(&**editor, pos) == PosVisibility::Offscreen;
        };
        let y = r.top + a.dy();
        y < v.top || y > v.bottom
    }

    /// La largeur imposée par la bande est rendue : le widget reprend la sienne.
    fn rendre_largeur(&self, id: WidgetId) {
        let Some(el) = self
            .ecrire(id, |w| {
                if w.largeur_imposee {
                    w.largeur_imposee = false;
                    w.el
                } else {
                    None
                }
            })
            .flatten()
        else {
            return;
        };
        self.elements.restore_width(el);
    }

    fn annoncer_fit(&self, id: WidgetId, fits: bool) {
        let cbs: Vec<RappelBool> = {
            let mut st = self.st.borrow_mut();
            let Some(w) = st.widget(id) else { return };
            if w.fits == Some(fits) {
                return;
            }
            w.fits = Some(fits);
            w.fit_cbs.iter().map(|(_, c)| Rc::clone(c)).collect()
        };
        for cb in cbs {
            cb(fits);
        }
    }

    fn slot_de(&self, id: WidgetId, el: ElementId, side: GutterSide) -> GutterSlot {
        if let Some(Some(slot)) = self.lire(id, |w| w.slot.clone()) {
            return slot;
        }
        let slot = self.overlays.gutter_slot(el, side);
        let compact = self.lire(id, |w| w.compact.clone()).flatten();
        slot.set_compact(compact);
        let (m1, m2) = (self.moi.clone(), self.moi.clone());
        let off_move = slot.on_move(move || {
            let Some(layer) = m1.upgrade() else { return };
            let cbs: Vec<RappelUnit> = layer
                .lire(id, |w| {
                    w.box_cbs.iter().map(|(_, c)| Rc::clone(c)).collect()
                })
                .unwrap_or_default();
            for cb in cbs {
                cb();
            }
        });
        let off_room = slot.on_room(move |room| {
            let Some(layer) = m2.upgrade() else { return };
            let cbs: Vec<RappelRoom> = layer
                .lire(id, |w| {
                    w.room_cbs.iter().map(|(_, c)| Rc::clone(c)).collect()
                })
                .unwrap_or_default();
            for cb in cbs {
                cb(room);
            }
        });
        self.ecrire(id, |w| {
            w.slot = Some(slot.clone());
            w.off_slot = Some(Box::new(move || {
                off_move();
                off_room();
            }));
        });
        slot
    }

    fn liberer_slot(&self, id: WidgetId) {
        let Some((slot, off)) = self.ecrire(id, |w| (w.slot.take(), w.off_slot.take())) else {
            return;
        };
        let avait = slot.is_some();
        if let Some(off) = off {
            off();
        }
        if let Some(slot) = slot {
            slot.remove();
        }
        // Hors de la marge : plus de place (le widget le sait, il peut se rétablir).
        if avait {
            let cbs: Vec<RappelRoom> = self
                .lire(id, |w| {
                    w.room_cbs.iter().map(|(_, c)| Rc::clone(c)).collect()
                })
                .unwrap_or_default();
            for cb in cbs {
                cb(None);
            }
        }
    }

    fn reposition_texte(&self) {
        // Réentrée (un rappel de la passe en relance une) : la passe en cours suffit.
        if self.st.borrow().passe.is_some() {
            for id in self.ids_au_texte() {
                self.position(id);
            }
            return;
        }
        self.st.borrow_mut().passe = Some(Passe::default());
        // 1. LIRE : toutes les ancres et les bandes, avant toute écriture.
        let ancres: Vec<WidgetAnchor> = {
            let st = self.st.borrow();
            st.widgets
                .iter()
                .filter(|w| w.anchor.suit_le_texte())
                .map(|w| w.anchor)
                .collect()
        };
        for a in ancres {
            if let Some(pos) = a.pos() {
                self.coords(pos);
            }
            if let WidgetAnchor::Gutter { side, .. } = a {
                self.bande(side);
            }
        }
        // 2. ÉCRIRE.
        for id in self.ids_au_texte() {
            self.position(id);
        }
        self.st.borrow_mut().passe = None;
    }

    fn ids_au_texte(&self) -> Vec<WidgetId> {
        self.st
            .borrow()
            .widgets
            .iter()
            .filter(|w| w.anchor.suit_le_texte())
            .map(|w| w.id)
            .collect()
    }

    /// `coords_at_pos`, lu une fois par passe (cf. `Passe`).
    fn coords(&self, pos: usize) -> Option<Rect> {
        let editor = self.editor.as_ref()?;
        if let Some(passe) = &self.st.borrow().passe
            && let Some(r) = passe.coords.get(&pos)
        {
            return *r;
        }
        let r = editor.coords_at_pos(pos);
        if let Some(passe) = &mut self.st.borrow_mut().passe {
            passe.coords.insert(pos, r);
        }
        r
    }

    /// `gutter_band`, lue une fois par passe.
    fn bande(&self, side: GutterSide) -> Option<GutterBand> {
        if let Some(passe) = &self.st.borrow().passe
            && let Some(b) = passe.bandes.get(&cote(side))
        {
            return *b;
        }
        let b = self.overlays.gutter_band(side);
        if let Some(passe) = &mut self.st.borrow_mut().passe {
            passe.bandes.insert(cote(side), b);
        }
        b
    }
}

/// Même membre de grappe (identité, `Rc::ptr_eq`), ou tous deux absents.
fn memes_membres(a: &Option<Rc<GutterMember>>, b: &Option<Rc<GutterMember>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

impl Handle {
    fn layer(&self) -> Option<Rc<Layer>> {
        self.layer.upgrade()
    }
}

/// Ajoute un abonné à une liste d'abonnés d'un widget ; rend le retrait.
fn abonner<T: 'static>(
    layer: &Rc<Layer>,
    id: WidgetId,
    cb: T,
    liste: fn(&mut Widget) -> &mut Vec<(u64, T)>,
) -> Disposer {
    let n = layer.st.borrow_mut().numero();
    layer.ecrire(id, |w| liste(w).push((n, cb)));
    let moi = Rc::downgrade(layer);
    Box::new(move || {
        if let Some(layer) = moi.upgrade() {
            layer.ecrire(id, |w| liste(w).retain(|(i, _)| *i != n));
        }
    })
}

impl WidgetHandle for Handle {
    fn get_anchor(&self) -> WidgetAnchor {
        self.layer()
            .and_then(|l| l.lire(self.id, |w| w.anchor))
            .unwrap_or(WidgetAnchor::viewport(0.0, 0.0))
    }

    fn set_anchor(&self, anchor: WidgetAnchor) {
        if let Some(l) = self.layer() {
            l.reanchor(self.id, anchor);
        }
    }

    fn get_box(&self) -> Option<WidgetBox> {
        let l = self.layer()?;
        let (el, masque, slot, anchor) =
            l.lire(self.id, |w| (w.el, w.masque, w.slot.clone(), w.anchor))?;
        let el = el.filter(|_| !masque)?;
        if let Some(slot) = slot {
            slot.flush();
        }
        // Une ancre viewport mesurée depuis la droite / le bas : la position RENDUE.
        let rendu = matches!(
            anchor,
            WidgetAnchor::Viewport { from: Some(c), .. } if c != ViewportCorner::TopLeft
        );
        let offset = l.elements.offset_box(el);
        let (left, top) = if rendu {
            (offset.left, offset.top)
        } else {
            l.elements.written_origin(el)
        };
        Some(WidgetBox {
            left,
            top,
            width: offset.width,
            height: offset.height,
        })
    }

    fn on_box_change(&self, cb: Box<dyn Fn()>) -> Disposer {
        match self.layer() {
            Some(l) => abonner(&l, self.id, Rc::from(cb), |w| &mut w.box_cbs),
            None => Box::new(|| {}),
        }
    }

    fn get_room(&self) -> Option<f64> {
        let l = self.layer()?;
        let slot = l.lire(self.id, |w| w.slot.clone())??;
        slot.get_room()
    }

    fn on_room_change(&self, cb: Box<dyn Fn(Option<f64>)>) -> Disposer {
        match self.layer() {
            Some(l) => abonner(&l, self.id, Rc::from(cb), |w| &mut w.room_cbs),
            None => Box::new(|| {}),
        }
    }

    fn is_active(&self) -> bool {
        self.layer()
            .and_then(|l| {
                l.lire(self.id, |w| {
                    w.activation
                        .as_ref()
                        .is_some_and(ActivationEntry::is_active)
                })
            })
            .unwrap_or(false)
    }

    fn on_active_change(&self, cb: Box<dyn Fn(bool)>) -> Disposer {
        match self.layer() {
            Some(l) => abonner(&l, self.id, Rc::from(cb), |w| &mut w.actif_cbs),
            None => Box::new(|| {}),
        }
    }

    fn set_active(&self, active: bool) {
        let Some(l) = self.layer() else { return };
        if let Some(Some(a)) = l.lire(self.id, |w| w.activation.clone()) {
            a.set_active(active);
        }
    }

    fn set_activation_manual(&self, manual: bool) {
        let Some(l) = self.layer() else { return };
        if let Some(Some(a)) = l.ecrire(self.id, |w| {
            w.manuel = manual;
            w.activation.clone()
        }) {
            a.set_manual(manual);
        }
    }

    fn set_activation_leaf(&self, leaf: Option<LeafId>) {
        let Some(l) = self.layer() else { return };
        if let Some(Some(a)) = l.ecrire(self.id, |w| {
            w.feuille = leaf;
            w.activation.clone()
        }) {
            a.set_leaf(leaf);
        }
    }

    fn set_compact(&self, info: Option<Rc<CompactInfo>>) {
        let Some(l) = self.layer() else { return };
        let Some((compact, courante, pos)) = l.lire(self.id, |w| {
            (
                w.compact.is_some(),
                w.info.clone(),
                Rc::clone(&w.pos_courante),
            )
        }) else {
            return;
        };
        let identique = match &info {
            None => !compact,
            Some(i) => compact && courante.is_some_and(|c| Rc::ptr_eq(&c, i)),
        };
        if identique {
            return;
        }
        // Pour la marge : la forme réduite + l'ancre COURANTE (lue, jamais copiée).
        let membre = info
            .as_ref()
            .map(|i| Rc::new(GutterMember::new(Rc::clone(i), move || pos.get())));
        let slot = l
            .ecrire(self.id, |w| {
                w.info = info;
                w.compact = membre.clone();
                w.slot.clone()
            })
            .flatten();
        if let Some(slot) = slot {
            slot.set_compact(membre);
        }
        l.position(self.id);
    }

    fn fits_gutter(&self) -> bool {
        self.layer()
            .and_then(|l| l.lire(self.id, |w| !w.anchor.est_marge() || w.fits != Some(false)))
            .unwrap_or(true)
    }

    fn on_fit_change(&self, cb: Box<dyn Fn(bool)>) -> Disposer {
        match self.layer() {
            Some(l) => abonner(&l, self.id, Rc::from(cb), |w| &mut w.fit_cbs),
            None => Box::new(|| {}),
        }
    }

    fn remove(&self) {
        let Some(l) = self.layer() else { return };
        // ★ Pendant que le SYSTÈME le détruit (sorti du viewport), l'objet se défait
        //   (`dispose` → il retire souvent son handle) : ce n'est pas une demande de
        //   retrait — sa déclaration reste (CM : `destroy` d'un WidgetType ne retire
        //   pas la décoration).
        match l.lire(self.id, |w| w.en_destruction) {
            None | Some(true) => return,
            Some(false) => {}
        }
        l.detacher(self.id);
        let mut st = l.st.borrow_mut();
        st.widgets.retain(|w| w.id != self.id);
    }
}
