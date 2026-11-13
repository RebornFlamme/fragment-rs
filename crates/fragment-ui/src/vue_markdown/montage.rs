// Port de app/src/ui/views/markdown/MarkdownView.ts (onOpen : l'éditeur monté dans le contenu de
// la vue, ses frappes qui demandent la sauvegarde, le défileur de lecture).
//! Le montage GPUI d'une [`VueMarkdown`] : l'élément éditeur du live preview ([`EditeurLive`]),
//! créé au premier rendu et gardé dans les [`Liens`] de la vue.
//!
//! ★ La couche de rendu ne reçoit qu'un `&dyn View` et un `&App` : pas de quoi créer une entité.
//!   L'élément [`ElementNote`] la crée donc à SON `request_layout`, où GPUI prête la fenêtre et
//!   l'App mutables, et la range dans les liens partagés de la vue.
//! ★ Le document de la vue fait foi. À chaque rendu, une révision nouvelle est recopiée dans
//!   l'éditeur : chargement neuf = éditeur RECRÉÉ (historique et état neuf : rien révélé avant le
//!   premier focus) ; sinon l'écart minimal remplace le texte, le curseur suit.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::View;
use fragment_text::{TextSurface as _, utf16_to_byte};
use gpui_kit::base::input::Paste;
use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, Element, ElementId, Entity, GlobalElementId,
    InspectorElementId, InteractiveElement as _, IntoElement, LayoutId, ParentElement as _, Pixels,
    Styled as _, Subscription, Window, div, point, px,
};

use super::position::CarteNote;
use super::relecture::delta_en_octets;
use super::vue::{Liens, VueMarkdown};
use crate::editeur::{EditeurLive, OptionsEditeur};
use crate::lecture::Defileur;

/// L'éditeur monté d'une vue, et ce qu'il a déjà recopié.
pub struct Monte {
    editeur: Entity<EditeurLive>,
    revision: u64,
    lecture_seule: bool,
    hauteur_boite: Rc<RefCell<f32>>,
    _abonnement: Subscription,
}

/// La cellule où une vue garde son éditeur monté.
pub type CelluleMontage = Rc<RefCell<Option<Monte>>>;

/// La cellule de montage d'une vue.
fn cellule(liens: &Liens) -> CelluleMontage {
    liens.montage.clone()
}

/// Le contenu d'une vue pour [`crate::onglets::ContenuDeVue`] : la note, si la vue en est une.
pub fn contenu(vue: &dyn View, _: &App) -> Option<AnyElement> {
    let md = vue.as_any().downcast_ref::<VueMarkdown>()?;
    Some(ElementNote::new(md.liens().clone()).into_any_element())
}

/// L'élément qui monte (ou remet à jour) l'éditeur de la note.
pub struct ElementNote {
    liens: Liens,
}

impl ElementNote {
    pub fn new(liens: Liens) -> Self {
        Self { liens }
    }
}

impl IntoElement for ElementNote {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

/// Le défileur de l'éditeur, figé pour une image : la reprise lit et pose ici, la pose est
/// appliquée au fork après coup.
struct CarteFork {
    haut: f32,
    gauche: f32,
    a_une_boite: bool,
    hauts: Vec<f32>,
    max: f32,
    pose: Option<f32>,
}

impl Defileur for CarteFork {
    fn position(&self) -> (f32, f32) {
        (self.haut, self.gauche)
    }
    fn poser(&mut self, haut: f32, _gauche: f32) {
        self.haut = haut.clamp(0.0, self.max);
        self.pose = Some(self.haut);
    }
    fn a_une_boite(&self) -> bool {
        self.a_une_boite
    }
}

impl CarteNote for CarteFork {
    fn haut_du_texte(&self) -> f32 {
        self.hauts.first().copied().unwrap_or(0.0)
    }
    fn nombre_de_lignes(&self) -> usize {
        self.hauts.len().max(1)
    }
    fn haut_de_ligne(&self, ligne: usize) -> f32 {
        self.hauts.get(ligne).copied().unwrap_or(0.0) - self.haut_du_texte()
    }
    fn ligne_a_hauteur(&self, h: f32) -> usize {
        let cible = h + self.haut_du_texte();
        self.hauts
            .partition_point(|&y| y <= cible)
            .saturating_sub(1)
    }
    fn defilement_max(&self) -> f32 {
        self.max
    }
}

/// Crée l'éditeur, ou y recopie la révision du document.
fn synchroniser(liens: &Liens, cellule: &CelluleMontage, window: &mut Window, cx: &mut App) {
    // Ce que le menu et les commandes ont posté passe AVANT la recopie : le document change,
    // l'éditeur le suit dans la même image.
    super::interaction::traiter(liens, cx);
    let (revision, neuf, lecture_seule, texte, selection) = {
        let n = liens.note.borrow();
        (
            n.revision(),
            n.neuf(),
            n.lecture_seule(),
            n.texte().to_owned(),
            n.selection(),
        )
    };
    let mut c = cellule.borrow_mut();
    let recreer = match c.as_ref() {
        None => true,
        Some(m) => neuf && m.revision != revision,
    };
    if recreer {
        let options = OptionsEditeur {
            lecture_seule,
            ..OptionsEditeur::default()
        };
        let editeur = cx.new(|cx| EditeurLive::new(&texte, options, window, cx));
        let l = liens.clone();
        let abonnement = cx.subscribe(&editeur, move |e, _: &fragment_text::EditorChange, cx| {
            let e = e.read(cx);
            let sel = e.surface(cx).get_selection();
            l.note.borrow_mut().poser_focus(e.a_le_focus());
            l.frappe(e.texte(), (sel.from, sel.to));
        });
        *c = Some(Monte {
            editeur,
            revision,
            lecture_seule,
            hauteur_boite: Rc::default(),
            _abonnement: abonnement,
        });
        return;
    }
    let Some(m) = c.as_mut() else { return };
    if m.lecture_seule != lecture_seule {
        m.lecture_seule = lecture_seule;
        let etat = m.editeur.read(cx).etat().clone();
        etat.update(cx, |s, cx| s.set_readonly(lecture_seule, cx));
    }
    if m.revision == revision {
        return;
    }
    m.revision = revision;
    let actuel = m.editeur.read(cx).texte().to_owned();
    let etat = m.editeur.read(cx).etat().clone();
    etat.update(cx, |s, cx| {
        if let Some((plage, insere)) = delta_en_octets(&actuel, &texte) {
            s.set_selected_range(plage, cx);
            s.replace(insere, window, cx);
        }
        let de = utf16_to_byte(&texte, selection.0.min(selection.1)).unwrap_or(texte.len());
        let a = utf16_to_byte(&texte, selection.0.max(selection.1)).unwrap_or(texte.len());
        s.set_selected_range(de..a, cx);
    });
}

/// Une image : sauvegarde différée, position de lecture.
fn image(liens: &Liens, cellule: &CelluleMontage, window: &mut Window, cx: &mut App) {
    let maintenant = (liens.horloge)();
    liens.pomper(maintenant);
    let c = cellule.borrow();
    let Some(m) = c.as_ref() else { return };
    let e = m.editeur.read(cx);
    let etat = e.etat().clone();
    let surface = e.surface(cx);
    let n = surface.line_count().max(1);
    let hauts: Vec<f32> = (0..n).map(|l| surface.haut_de_ligne(l)).collect();
    let decalage = etat.read(cx).scroll_offset();
    let boite = *m.hauteur_boite.borrow();
    let bas = hauts.last().copied().unwrap_or(0.0) + e.interligne() * e.taille_texte();
    let mut carte = CarteFork {
        haut: -f32::from(decalage.y),
        gauche: -f32::from(decalage.x),
        a_une_boite: boite > 0.0,
        hauts,
        max: (bas - boite).max(0.0),
        pose: None,
    };
    drop(c);
    liens.image(maintenant, Some(&mut carte));
    if let Some(h) = carte.pose {
        etat.update(cx, |s, cx| s.set_scroll_offset(point(px(0.0), px(-h)), cx));
    }
    if liens.note.borrow().echeance().is_some() || liens.reprise_en_cours() {
        window.request_animation_frame();
    }
}

impl Element for ElementNote {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let cellule = cellule(&self.liens);
        synchroniser(&self.liens, &cellule, window, cx);
        image(&self.liens, &cellule, window, cx);
        let Some(editeur) = cellule.borrow().as_ref().map(|m| m.editeur.clone()) else {
            let mut vide = div().size_full().into_any_element();
            let id = vide.request_layout(window, cx);
            return (id, vide);
        };
        let (l1, l2) = (self.liens.clone(), self.liens.clone());
        let (l3, l4) = (self.liens.clone(), self.liens.clone());
        let mut e = div()
            .id("vue-markdown")
            .size_full()
            .on_scroll(move |_, _, _| {
                l1.geste();
                l1.defilement();
            })
            .on_mouse_down(gpui_kit::MouseButton::Left, move |_, _, _| l2.geste())
            .on_mouse_down(gpui_kit::MouseButton::Right, move |e, _, cx| {
                super::interaction::montrer_menu(
                    &l3,
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                );
                cx.refresh_windows();
            })
            // ★ Une image dans le presse-papiers va au coffre ; le texte reste à l'éditeur (la
            //   capture passe avant lui, et ne consomme que s'il y a une image).
            .capture_action(move |_: &Paste, _, cx| {
                if super::interaction::coller_images(&l4, cx.read_from_clipboard().as_ref()) {
                    cx.stop_propagation();
                    cx.refresh_windows();
                }
            })
            .child(editeur)
            .into_any_element();
        let id = e.request_layout(window, cx);
        (id, e)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        e: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(m) = cellule(&self.liens).borrow().as_ref() {
            *m.hauteur_boite.borrow_mut() = f32::from(bounds.size.height);
        }
        e.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        e: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        e.paint(window, cx);
    }
}
