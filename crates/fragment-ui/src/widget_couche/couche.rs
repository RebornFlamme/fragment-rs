// Port de app/src/ui/core/layers/WidgetLayer.ts et WidgetActivation.ts (la moitié DOM : écouteurs
// pointerdown / pointerup / focusin / focusout, microtâches et images rejouées par l'hôte)
//
//! La couche des widgets d'une vue : tous ses `WidgetLayer` (un par calque, tous sur le même hôte,
//! donc sur le même arbitre de marge et le même actif), et ce que GPUI doit leur traduire.
//!
//! ★ Le TEMPS. Le TS vit de `queueMicrotask`, `requestAnimationFrame` et `ResizeObserver`. Ici,
//!   tout passe par l'IMAGE : [`CoucheWidgets::element`] pose trois enfants au-dessus de la
//!   surface — un pilote AVANT les plans (microtâches et essais en attente des calques, focus
//!   relevé), les plans de l'hôte (mesure, modèles, placement : `crate::calques`), un pilote APRÈS
//!   (ce qui a changé pendant la passe redemande une image). La mise en page de GPUI a lieu avant
//!   les trois `prepaint` : la surface est mesurée quand ils tournent.
//!
//! ★ Le `ResizeObserver` de la marge : l'hôte relance l'arbitre quand un élément de marge change
//!   de taille, DANS la passe de l'image (aucun recouvrement peint). Ici, l'empreinte des éléments
//!   (place, affichage, taille, coin) est comparée d'une image à l'autre : tant qu'elle bouge
//!   (une place annoncée a changé la forme d'un widget, qui se mesure à l'image suivante), une
//!   image suit — elle se tait quand tout est placé (pas d'oscillation).
//!
//! ★ L'ACTIF APRÈS LE GESTE. L'appui (capture, bouton quelconque) est traduit en `pointer_down`
//!   avec la cible relevée par boîte (le widget le plus haut sous le pointeur, `None` dehors —
//!   texte, autre pane, dock) ; le relâché en `pointer_up`, puis UNE image : le tour suivant
//!   (`run_timers`) tourne dans la passe de l'hôte, après le `click` du widget. Un widget qui
//!   bouge à l'activation ne perd donc pas son clic. Un appui alors qu'un menu ou une modale est
//!   ouvert n'est pas « dehors » (on y choisit souvent pour le widget actif).
//!
//! ★ Le FOCUS. Un élément peut confier sa `FocusHandle` à la couche ([`CoucheWidgets::suivre_focus`]) ;
//!   à chaque image, le pilote relève qui l'a : entrée → `focus_in` (actif tout de suite hors
//!   geste), sortie → `focus_out` des calques (un widget relâché hors de la bande est revu, donc
//!   détruit). L'hôte sait qui a le focus (`poser_focus`) : `WidgetLayer` garde construit celui
//!   qui l'a, comme CodeMirror garde la ligne du curseur.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use fragment_layers::overlay_host::{OverlayHost, WidgetElements};
use fragment_layers::types::{ElementId, Point};
use fragment_layers::view_overlays::Racine;
use fragment_layers::widget_activation::PointerDown;
use fragment_layers::widget_layer::WidgetLayer;
use fragment_text::{DocumentSurface, EditorChange};
use gpui_kit::{
    App, DispatchPhase, FocusHandle, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent,
    ParentElement as _, Pixels, Styled as _, Window, canvas, div,
};

use crate::calques::{HoteCalques, SourcePlans, plans_calques};
use crate::echelle::Echelle;

/// Ce qu'un élément montre d'une image à l'autre : s'il change, une image doit suivre.
#[derive(Clone, Debug, PartialEq)]
struct Empreinte {
    el: ElementId,
    racine: Option<Racine>,
    affiche: bool,
    en_grappe: bool,
    place: Option<f64>,
    taille: (f64, f64),
    coin: Option<(f64, f64)>,
    largeur: Option<f64>,
}

/// Un calque de widgets de la vue : son `WidgetLayer`, rendu à la couche par [`CalqueWidgets::fermer`].
pub struct CalqueWidgets {
    id: u64,
    couche: Weak<CoucheWidgets>,
    layer: WidgetLayer,
}

impl CalqueWidgets {
    /// Le `WidgetLayer` du calque (le contrat des plugins).
    pub fn layer(&self) -> &WidgetLayer {
        &self.layer
    }

    /// Le calque est décoché ou fermé : tous ses widgets sont retirés (un déclaré est détruit).
    pub fn fermer(&self) {
        self.layer.destroy();
        if let Some(c) = self.couche.upgrade() {
            c.calques.borrow_mut().retain(|(i, _)| *i != self.id);
        }
    }
}

type Ephemere = Rc<dyn Fn() -> bool>;

/// La couche des widgets d'une vue.
pub struct CoucheWidgets {
    hote: Rc<HoteCalques>,
    surface: Option<Rc<dyn DocumentSurface>>,
    calques: RefCell<Vec<(u64, WidgetLayer)>>,
    prochain: Cell<u64>,
    focus: RefCell<Vec<(ElementId, FocusHandle)>>,
    focalise: Cell<Option<ElementId>>,
    empreintes: RefCell<Vec<Empreinte>>,
    /// Un menu ou une modale est-il ouvert ? (Un appui y est « dedans ».)
    ephemere: RefCell<Ephemere>,
    moi: Weak<CoucheWidgets>,
}

impl CoucheWidgets {
    /// La couche d'une vue sur son hôte ; `surface` : sa surface de texte (`None` : seules les
    /// ancres viewport existent).
    pub fn new(hote: Rc<HoteCalques>, surface: Option<Rc<dyn DocumentSurface>>) -> Rc<Self> {
        Rc::new_cyclic(|moi| Self {
            hote,
            surface,
            calques: RefCell::new(Vec::new()),
            prochain: Cell::new(0),
            focus: RefCell::new(Vec::new()),
            focalise: Cell::new(None),
            empreintes: RefCell::new(Vec::new()),
            // Les modales de fragment-ui n'exposent pas d'état global : le montage le branche.
            ephemere: RefCell::new(Rc::new(crate::menu::menu_ouvert)),
            moi: moi.clone(),
        })
    }

    /// L'hôte des calques de la vue.
    pub fn hote(&self) -> &Rc<HoteCalques> {
        &self.hote
    }

    /// Un nouveau calque de widgets (chaque calque a SON `WidgetLayer` ; la marge et l'actif
    /// restent ceux de la vue).
    pub fn nouveau_calque(&self) -> CalqueWidgets {
        let overlays: Rc<dyn OverlayHost> = self.hote.clone();
        let elements: Rc<dyn WidgetElements> = self.hote.clone();
        let layer = WidgetLayer::new(self.surface.clone(), overlays, elements);
        let id = self.prochain.get() + 1;
        self.prochain.set(id);
        self.calques.borrow_mut().push((id, layer.clone()));
        CalqueWidgets {
            id,
            couche: self.moi.clone(),
            layer,
        }
    }

    fn layers(&self) -> Vec<WidgetLayer> {
        self.calques
            .borrow()
            .iter()
            .map(|(_, l)| l.clone())
            .collect()
    }

    /// Ce qui a bougé dans la surface (l'`onChange` de l'éditeur), relayé à chaque calque.
    pub fn relayer(&self, change: &EditorChange) {
        for l in self.layers() {
            l.on_editor_change(change);
        }
    }

    /// Remplace le test « un menu ou une modale est ouvert » (tests, montage).
    pub fn poser_ephemere(&self, f: impl Fn() -> bool + 'static) {
        *self.ephemere.borrow_mut() = Rc::new(f);
    }

    // ── Le focus ────────────────────────────────────────────────────────────

    /// L'élément `el` suit le focus de `handle` (relevé à chaque image).
    pub fn suivre_focus(&self, el: ElementId, handle: FocusHandle) {
        let mut f = self.focus.borrow_mut();
        f.retain(|(e, _)| *e != el);
        f.push((el, handle));
    }

    /// L'élément ne suit plus de focus (démonté).
    pub fn oublier_focus(&self, el: ElementId) {
        self.focus.borrow_mut().retain(|(e, _)| *e != el);
    }

    /// Le focus clavier est passé à `el` (`None` : hors de tout widget) — `focusin` / `focusout`.
    pub fn signaler_focus(&self, el: Option<ElementId>) {
        let avant = self.focalise.replace(el);
        if avant == el {
            return;
        }
        self.hote.poser_focus(el);
        if let Some(nouveau) = el {
            self.hote.activation().focus_in(nouveau);
        }
        if let Some(ancien) = avant {
            for l in self.layers() {
                l.focus_out(ancien);
            }
        }
    }

    /// L'élément qui a le focus selon la dernière image.
    pub fn focalise(&self) -> Option<ElementId> {
        self.focalise.get()
    }

    fn relever_focus(&self, window: &Window) {
        // Les éléments oubliés (détruits) ne suivent plus rien.
        self.focus
            .borrow_mut()
            .retain(|(el, _)| self.hote.fiche(*el).is_some());
        let el = self
            .focus
            .borrow()
            .iter()
            .find(|(el, h)| {
                h.is_focused(window) && self.hote.fiche(*el).is_some_and(|f| f.racine.is_some())
            })
            .map(|(el, _)| *el);
        self.signaler_focus(el);
    }

    // ── Le pointeur ─────────────────────────────────────────────────────────

    /// Le widget le plus haut sous le point CLIENT (px CSS), ou `None`.
    pub fn cible(&self, x: f64, y: f64) -> Option<ElementId> {
        self.hote
            .a_dessiner()
            .into_iter()
            .rev()
            .map(|(el, ..)| el)
            .find(|el| {
                if !self.hote.fiche(*el).is_some_and(|f| f.dessinee()) {
                    return false;
                }
                let r = self.hote.client_rect(*el);
                x >= r.left && x < r.right && y >= r.top && y < r.bottom
            })
    }

    /// Un appui (capture) au point CLIENT `(x, y)` ; `bouton` : 0 principal, 1 milieu, 2 droit.
    pub fn appui(&self, bouton: u8, x: f64, y: f64) {
        let in_overlay = (self.ephemere.borrow().clone())();
        let target = self.cible(x, y);
        self.hote.activation().pointer_down(PointerDown {
            button: bouton,
            target,
            in_overlay,
        });
    }

    /// Le relâché (ou l'annulation) du geste : il tranche au tour suivant.
    pub fn relache(&self) {
        self.hote.activation().pointer_up();
    }

    // ── L'image ─────────────────────────────────────────────────────────────

    /// Avant les plans : le focus relevé, puis les essais et microtâches en attente des calques.
    fn avant_image(&self, window: &Window) {
        self.relever_focus(window);
        for l in self.layers() {
            if l.needs_frame() {
                l.run_frame();
            }
            l.run_microtasks();
        }
    }

    /// Après les plans : ce qui attend encore (ou a bougé) demande une image.
    fn apres_image(&self, window: &mut Window) {
        let mut encore = self
            .layers()
            .iter()
            .any(|l| l.needs_microtasks() || l.needs_frame())
            || self.hote.arbitre().needs_flush();
        let nouvelles = self.empreintes();
        let anciennes = self.empreintes.replace(nouvelles.clone());
        if nouvelles != anciennes {
            encore = true;
        }
        if encore {
            window.request_animation_frame();
        }
    }

    fn empreintes(&self) -> Vec<Empreinte> {
        self.hote
            .a_dessiner()
            .into_iter()
            .filter_map(|(el, ..)| {
                let f = self.hote.fiche(el)?;
                let coin = self.hote.coin_dans_le_pane(el).map(|p| (p.x, p.y));
                Some(Empreinte {
                    el,
                    racine: f.racine,
                    affiche: f.affiche,
                    en_grappe: f.en_grappe,
                    place: f.place,
                    taille: f.taille,
                    coin,
                    largeur: f.largeur_imposee,
                })
            })
            .collect()
    }

    /// Les calques de la vue, à poser en DERNIER enfant de son pane (au-dessus de la surface), en
    /// absolu, couvrant le pane : pilote, plans de l'hôte, pilote.
    pub fn element(self: &Rc<Self>, source: Rc<dyn SourcePlans>) -> impl IntoElement {
        let (avant, apres) = (Rc::clone(self), Rc::clone(self));
        let ecoute = Rc::clone(self);
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                canvas(
                    move |_, window, _| avant.avant_image(window),
                    move |_, (), window, cx| ecouter(&ecoute, window, cx),
                )
                .absolute()
                .size_full(),
            )
            .child(plans_calques(Rc::clone(&self.hote), source))
            .child(
                canvas(
                    move |_, window, _| apres.apres_image(window),
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
    }

    /// La vue se ferme : tous les calques sont défaits.
    pub fn detruire(&self) {
        let calques = std::mem::take(&mut *self.calques.borrow_mut());
        for (_, l) in calques {
            l.destroy();
        }
        self.focus.borrow_mut().clear();
    }
}

fn bouton(b: MouseButton) -> u8 {
    match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        // Précédent / suivant : ni principal, ni menu.
        _ => 3,
    }
}

fn vers_css(p: Pixels, zoom: f64) -> f64 {
    f64::from(f32::from(p)) / zoom
}

/// Les écouteurs du geste, en CAPTURE (avant tout widget), sur toute la fenêtre.
fn ecouter(couche: &Rc<CoucheWidgets>, window: &mut Window, _cx: &mut App) {
    let c = Rc::downgrade(couche);
    window.on_mouse_event(move |ev: &MouseDownEvent, phase, _window, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let Some(c) = c.upgrade() else { return };
        let zoom = f64::from(Echelle::courante(cx).zoom());
        let p = Point {
            x: vers_css(ev.position.x, zoom),
            y: vers_css(ev.position.y, zoom),
        };
        c.appui(bouton(ev.button), p.x, p.y);
    });
    let c = Rc::downgrade(couche);
    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let Some(c) = c.upgrade() else { return };
        c.relache();
        // Le tour suivant tranche : il lui faut une image.
        window.refresh();
    });
}
