// Port de app/src/ui/core/layers/types.ts (OverlayHost, LayerContext, LayerSpec)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Ce que la vue offre aux layers pour poser des overlays, SANS exposer son
//  rendu : on monte un élément dans un plan, on récupère le retrait. Le layer
//  POSITIONNE lui-même l'élément (left/top) ; l'hôte garantit seulement
//  l'origine du plan :
//    - `Viewport` : coin du pane, NE scrolle pas (objets fixes à l'écran) ;
//    - `Document` : le repère de `coords_at_pos` lui-même — un élément posé aux
//      coords rendues par l'éditeur y atterrit juste, et suit le texte au scroll
//      comme à la remise en page, sans que personne n'ait à resynchroniser quoi
//      que ce soit.
//  Un layer ne peut ni restructurer les plans ni toucher les widgets d'un autre.
//
//  ★ Transposition : l'hôte (fragment-ui, phase 3) implémente `OverlayHost` et
//    `WidgetElements` ; les `HTMLElement` y sont des `ElementId`.
// ═══════════════════════════════════════════════════════════════════════════

use std::any::Any;
use std::rc::Rc;

use fragment_text::{DocumentSurface, Rect};

use crate::gutter_arbiter::GutterSlot;
use crate::types::{
    ElementId, GutterBand, GutterSide, LayerInstanceRef, LayerPreset, OverlayPlane, Point,
    ViewportBand,
};
use crate::widget_activation::ActivationEntry;

/// Un retrait : rendu par ce qui monte ou s'abonne, appelé UNE fois pour défaire.
pub type Disposer = Box<dyn FnOnce()>;

/// La condition d'un spec sur une vue opaque.
type AppliesTo = dyn Fn(&dyn Any) -> bool;

pub trait OverlayHost {
    /// Monte un élément dans un plan ; rend le retrait.
    fn mount(&self, el: ElementId, plane: OverlayPlane) -> Disposer;

    /// Convertit un point CLIENT en coordonnées du plan viewport, ou `None` si le
    /// point est HORS du pane. Sert de hit-test ET de conversion.
    fn client_to_viewport(&self, x: f64, y: f64) -> Option<Point>;

    /// Le symétrique pour le plan document : un point CLIENT → le repère de
    /// `coords_at_pos`. `None` sans plan document (vue sans éditeur).
    fn client_to_document(&self, x: f64, y: f64) -> Option<Point>;

    /// L'inverse : un point du plan document → CLIENT. `None` sans plan document.
    fn document_to_client(&self, x: f64, y: f64) -> Option<Point>;

    /// La bande de MARGE disponible d'un côté du texte, en coordonnées du plan
    /// document — prête à être posée telle quelle sur un widget. C'est l'espace
    /// entre le bord du pane et le bord de la colonne de lecture ; il dépend de la
    /// largeur du PANE, pas du document. `width` peut être négatif ou nul quand la
    /// colonne remplit le pane — l'appelant décide alors (typiquement : masquer).
    fn gutter_band(&self, side: GutterSide) -> Option<GutterBand>;

    /// Le PARTAGE de la marge entre tous les calques de la vue : un widget de marge
    /// y prend un slot, déclare son haut voulu, et l'arbitre de la vue le place —
    /// au plus près de son ancre, sans recouvrir ceux des AUTRES calques.
    fn gutter_slot(&self, el: ElementId, side: GutterSide) -> GutterSlot;

    /// L'ACTIF des widgets de la vue — UN SEUL, tous calques confondus : le dernier
    /// cliqué (ou focalisé) ; un clic HORS de tout widget le relâche. Inscrire un
    /// élément rend son entrée.
    fn track_activation(&self, el: ElementId) -> ActivationEntry;

    /// Prévient quand la géométrie des plans change (pane ou colonne redimensionnés).
    ///
    /// ⚠️ À n'utiliser QUE pour ce qui dépend vraiment de la taille des plans — les
    /// marges. La position d'un widget ancré au texte n'en dépend pas : le plan
    /// document vit dans le repère de `coords_at_pos`, donc il glisse avec lui.
    fn on_geometry_change(&self, cb: Box<dyn Fn()>) -> Disposer;

    /// Le VIEWPORT des calques — celui de CodeMirror 6, porté (`fragment_text::
    /// viewport`) : la bande du plan document où ce que posent les calques EXISTE —
    /// le visible du pane ± 1000 px écran, penchée vers où l'on défile. Hors
    /// d'elle, rien n'est monté ni affiché. `None` sans plan document.
    fn viewport(&self) -> Option<ViewportBand>;

    /// Prévient quand la bande CHANGE — pas à chaque image de défilement :
    /// seulement quand le visible approche de son bord (l'hystérésis de CM).
    fn on_viewport_change(&self, cb: Box<dyn Fn()>) -> Disposer;

    /// ★ AJOUT au contrat TS : le rectangle CLIENT de la colonne de texte
    /// (`editor.contentEl.getBoundingClientRect()` en TS). `WidgetLayer` en tire le
    /// milieu de la colonne pour sonder le texte à côté d'une marge
    /// (`gutter_anchor_at`) ; le DOM de la surface (`contentEl`) n'est pas dans le
    /// contrat de `DocumentSurface`. `None` sans plan document.
    fn content_client_rect(&self) -> Option<Rect>;
}

/// Ce que `WidgetLayer` lit et écrit sur les éléments de ses widgets (les
/// `el.style.*` et mesures du TS). Les valeurs sont dans le repère du plan de
/// l'élément.
pub trait WidgetElements {
    /// L'élément vient d'être posé : positionné en absolu, captant les clics
    /// (`pointer-events: auto`) — sauf `pass_through` (transparent aux clics : seuls
    /// ses enfants captent).
    fn attach(&self, el: ElementId, pass_through: bool);
    /// Ancre viewport : écrit les quatre côtés (`None` = vidé).
    fn set_viewport_edges(
        &self,
        el: ElementId,
        left: Option<f64>,
        right: Option<f64>,
        top: Option<f64>,
        bottom: Option<f64>,
    );
    /// Une ancre au texte pose `left`/`top` : rien de ce qu'un coin viewport aurait
    /// laissé (`right`/`bottom` vidés).
    fn clear_right_bottom(&self, el: ElementId);
    fn set_left(&self, el: ElementId, left: f64);
    fn set_top(&self, el: ElementId, top: f64);
    /// La bande IMPOSE une largeur à un widget de marge entier. À la première
    /// imposition l'hôte met de côté la largeur propre du widget.
    fn impose_width(&self, el: ElementId, width: f64);
    /// Le widget reprend la largeur qu'il s'était donnée (la largeur imposée est un
    /// effet de l'ancre, pas une propriété du widget).
    fn restore_width(&self, el: ElementId);
    /// Affiche / masque (`display`).
    fn set_displayed(&self, el: ElementId, displayed: bool);
    /// La boîte RENDUE (`offsetLeft/Top/Width/Height`).
    fn offset_box(&self, el: ElementId) -> crate::widget_layer::WidgetBox;
    /// Le `left`/`top` ÉCRITS (0 si absents) — la position d'une ancre mesurée
    /// depuis le coin haut-gauche.
    fn written_origin(&self, el: ElementId) -> (f64, f64);
    /// Le rectangle CLIENT de l'élément (`getBoundingClientRect`).
    fn client_rect(&self, el: ElementId) -> Rect;
    /// Le focus est-il DANS l'élément ? (`el.contains(document.activeElement)`)
    fn contains_focus(&self, el: ElementId) -> bool;
}

/// Ce que la vue tend au layer à sa création — le « contrat exposé par la vue ».
///
/// ★ Seul le pur est modélisé. `view`, `file`, `app`, `scope` (portée clavier) et
///   `paneEl` du TS sont des objets de l'application et de la présentation :
///   fragment-ui (phase 3) les ajoute en enveloppant ce contexte.
pub struct LayerContext {
    /// La surface de document de la vue, ou `None` si elle n'en a pas. Un layer qui
    /// veut ancrer au document doit gérer le cas `None` : sans surface, seuls les
    /// overlays viewport sont possibles.
    ///
    /// ★ C'est un `DocumentSurface`, PAS un `Editor` : un layer n'a jamais eu besoin
    ///   des membres textuels, et toutes les vues n'en ont pas (un PDF scanné n'a
    ///   aucun texte). `fragment_text::has_text` ouvre la moitié textuelle.
    pub editor: Option<Rc<dyn DocumentSurface>>,
    doc: Rc<dyn Fn() -> Option<String>>,
    /// Points de montage d'overlay view-level (viewport toujours ; document si éditeur).
    pub overlays: Rc<dyn OverlayHost>,
    /// L'instance montée : un même spec peut vivre plusieurs fois sur une vue, une
    /// fois par preset. `instance.key` identifie CE montage — c'est la clef que
    /// `close()` retire.
    pub instance: LayerInstanceRef,
    close: Rc<dyn Fn()>,
}

impl LayerContext {
    pub fn new(
        editor: Option<Rc<dyn DocumentSurface>>,
        doc: impl Fn() -> Option<String> + 'static,
        overlays: Rc<dyn OverlayHost>,
        instance: LayerInstanceRef,
        close: impl Fn() + 'static,
    ) -> Self {
        Self {
            editor,
            doc: Rc::new(doc),
            overlays,
            instance,
            close: Rc::new(close),
        }
    }

    /// La CLÉ du document de la vue, LUE À L'USAGE : les calques montent avant que
    /// la vue ait son document. Ce qu'un calque doit lire pour ranger ses marques.
    pub fn doc(&self) -> Option<String> {
        (self.doc)()
    }

    /// Retire CETTE instance de la vue (démontage synchrone).
    pub fn close(&self) {
        (self.close)();
    }
}

/// La déclaration d'un layer (fiche de catalogue), contribuée par un plugin. Un
/// layer n'est PAS un Component : `create` monte la contribution sur un éditeur et
/// rend un disposer ; c'est le plugin (Component) qui possède les ressources
/// partagées entre éditeurs (réseau/CRDT).
pub struct LayerSpec {
    /// Clef unique dans le catalogue — identité de réconciliation.
    pub id: String,
    /// Libellé pour le switcher.
    pub name: String,
    pub icon: Option<String>,
    /// Activé d'office à l'ouverture d'une vue applicable ? (défaut : non).
    pub default_enabled: bool,
    /// `false` : un calque d'INFRASTRUCTURE — monté comme les autres, mais absent du
    /// menu « Calques ». Défaut : affiché.
    pub in_switcher: bool,
    applies_to: Option<Rc<AppliesTo>>,
    presets: Option<Rc<dyn Fn() -> Vec<LayerPreset>>>,
    create: Rc<dyn Fn(&LayerContext) -> Disposer>,
}

impl LayerSpec {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        create: impl Fn(&LayerContext) -> Disposer + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            default_enabled: false,
            in_switcher: true,
            applies_to: None,
            presets: None,
            create: Rc::new(create),
        }
    }

    /// Le layer concerne-t-il cette vue ? `view` est la vue de l'application
    /// (opaque ici). Défaut : oui — « toute vue éditrice » est tranché par le
    /// catalogue, qui connaît les vues.
    pub fn with_applies_to(mut self, f: impl Fn(&dyn Any) -> bool + 'static) -> Self {
        self.applies_to = Some(Rc::new(f));
        self
    }

    /// Les presets du spec : chacun ajoute au switcher une entrée cochable, donc
    /// une instance de plus possible sur la vue (clef `spec:preset`).
    pub fn with_presets(mut self, f: impl Fn() -> Vec<LayerPreset> + 'static) -> Self {
        self.presets = Some(Rc::new(f));
        self
    }

    pub fn applies_to(&self, view: &dyn Any) -> bool {
        self.applies_to.as_ref().is_none_or(|f| f(view))
    }

    /// Les presets (défaut : aucun).
    pub fn presets(&self) -> Vec<LayerPreset> {
        self.presets.as_ref().map(|f| f()).unwrap_or_default()
    }

    /// Monte le layer sur l'éditeur d'une vue ; rend le disposer.
    pub fn create(&self, ctx: &LayerContext) -> Disposer {
        (self.create)(ctx)
    }
}
