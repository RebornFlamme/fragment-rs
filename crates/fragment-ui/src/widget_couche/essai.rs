//! Les widgets de TEST de la couche (docs/tranches/T5.md §3.4) : ils remplacent, côté fragment-rs,
//! les plugins de test des specs de Fragment (exécutés là-bas, jamais traduits ici — ADR-007).
//! Écrits pour ce dépôt d'après le contrat qu'ils éprouvent :
//!
//! - RIGIDE : ignore la place annoncée (le « globe » d'un autre calque) — il est seulement décalé et
//!   réduit la place du suivant ; c'est une ELLIPSE à dégradé radial ([`super::globe`]) ;
//! - FLEX : se plie à sa place (sa hauteur ne la dépasse pas) ;
//! - SEUIL : SES seuils — sous [`SEUIL`] px de place, il passe en compact ([`COMPACT`] px) ;
//! - VUE : une vue nue (une « carte » où l'on tape) ; active, elle porte l'anneau.
//!
//! ★ Couleurs et formes sont celles des fixtures de Fragment (`e2e-room`, `e2e-globe`, la vue nue
//! de `doc-widget`) : la palette de FIXTURE de `fragment_theme::fixture`, pas les jetons du thème.
//!
//! Tous sont DÉCLARÉS (`WidgetDeclare`) : construits dans la bande, détruits hors d'elle. Tous
//! prennent le focus à l'appui et comptent leurs clics. Chacun compte les annonces de place qu'il
//! reçoit (stabilité) et relève la boîte où GPUI l'a posé.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use fragment_layers::types::{ElementId, GutterSide};
use fragment_layers::widget_layer::{WidgetAnchor, WidgetHandle, WidgetOptions, WidgetSource};
use fragment_theme::fixture::{FIXTURE_BLEU_VUE, FIXTURE_ROUGE, FIXTURE_ROUGE_FOND};
use gpui_kit::{
    AnyElement, App, Bounds, Corners, FocusHandle, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Pixels, RenderImage, StatefulInteractiveElement as _,
    Styled as _, Window, canvas, div,
};

use super::couche::{CalqueWidgets, CoucheWidgets};
use super::globe::globe_bgra;
use super::paresseux::{Fabrique, WidgetDeclare};
use crate::calques::{Contenu, HoteCalques};
use crate::echelle::u;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Sous cette place (px), le widget « seuil » passe en compact.
pub const SEUIL: f64 = 80.0;
/// La hauteur du « seuil » compact (px).
pub const COMPACT: f64 = 40.0;
/// La largeur d'un widget de test hors de la marge (la marge lui impose la sienne).
pub const LARGEUR_LIBRE: f64 = 240.0;
/// Le décalage vertical du rigide sous son ancre (le globe se pose un peu sous sa ligne).
pub const DY_RIGIDE: f64 = 6.0;

/// Le globe mis en image : sa taille en pixels de l'écran et l'image.
type ImageGlobe = ((u32, u32), Arc<RenderImage>);

/// Le rayon des coins de la vue nue (px).
pub const RAYON_VUE: f64 = 8.0;

/// La sorte d'un widget de test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SorteEssai {
    Rigide,
    Flex,
    Seuil,
    Vue,
}

impl SorteEssai {
    /// Depuis le nom du contrat `pixels:widget-test` (`rigide | flex | seuil | vue`).
    pub fn depuis_nom(nom: &str) -> Option<Self> {
        match nom {
            "rigide" => Some(Self::Rigide),
            "flex" => Some(Self::Flex),
            "seuil" => Some(Self::Seuil),
            "vue" => Some(Self::Vue),
            _ => None,
        }
    }
}

/// Ce qu'on pose : la sorte, l'ancre, la hauteur naturelle (px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpecEssai {
    pub sorte: SorteEssai,
    pub ancre: WidgetAnchor,
    pub hauteur: f64,
}

impl SpecEssai {
    /// Un widget de marge ancré à `from` (`to` : la fin du passage).
    pub fn marge(
        sorte: SorteEssai,
        from: usize,
        to: Option<usize>,
        side: GutterSide,
        hauteur: f64,
    ) -> Self {
        let dy = if sorte == SorteEssai::Rigide {
            DY_RIGIDE
        } else {
            0.0
        };
        Self {
            sorte,
            ancre: WidgetAnchor::Gutter {
                side,
                pos: from,
                to,
                dy,
            },
            hauteur,
        }
    }
}

/// L'état d'un widget de test, partagé par ses constructions successives (le handle survit,
/// l'élément change).
#[derive(Default)]
struct Etat {
    /// Les annonces de place reçues.
    annonces: Cell<u32>,
    /// Le « seuil » est compact.
    compact: Cell<bool>,
    /// La hauteur naturelle courante (un widget qui grandit : le flux d'une réponse).
    hauteur: Cell<f64>,
    /// Les clics reçus (un clic perdu ne compte pas).
    clics: Cell<u32>,
    /// La poignée de focus (créée à la première image).
    focus: RefCell<Option<FocusHandle>>,
    /// La dernière boîte où GPUI l'a posé.
    pose: RefCell<Option<Bounds<Pixels>>>,
    /// L'image du globe du rigide, avec sa taille en pixels de l'écran (refaite si elle change).
    globe: RefCell<Option<ImageGlobe>>,
}

/// Un widget de test posé : son handle (vivant toujours) et ce qu'il a relevé.
pub struct WidgetEssai {
    pub spec: SpecEssai,
    pub handle: Rc<dyn WidgetHandle>,
    pub declare: Rc<WidgetDeclare>,
    etat: Rc<Etat>,
}

impl WidgetEssai {
    /// L'élément construit, ou `None` hors de la bande.
    pub fn element(&self) -> Option<ElementId> {
        self.declare.element()
    }

    /// La dernière boîte où GPUI l'a posé (px logiques de la fenêtre).
    pub fn pose(&self) -> Option<Bounds<Pixels>> {
        *self.etat.pose.borrow()
    }

    /// Oublie la boîte relevée (avant une image dont on veut savoir s'il est peint).
    pub fn oublier_pose(&self) {
        *self.etat.pose.borrow_mut() = None;
    }

    /// Les annonces de place reçues.
    pub fn annonces(&self) -> u32 {
        self.etat.annonces.get()
    }

    /// Le « seuil » est-il compact ?
    pub fn compact(&self) -> bool {
        self.etat.compact.get()
    }

    /// Les clics reçus.
    pub fn clics(&self) -> u32 {
        self.etat.clics.get()
    }

    /// Change la hauteur naturelle (le contenu grandit).
    pub fn poser_hauteur(&self, h: f64) {
        self.etat.hauteur.set(h);
    }

    /// La hauteur naturelle courante.
    pub fn hauteur(&self) -> f64 {
        self.etat.hauteur.get()
    }

    /// La poignée de focus, si le widget a déjà été dessiné.
    pub fn poignee_focus(&self) -> Option<FocusHandle> {
        self.etat.focus.borrow().clone()
    }
}

/// Pose un widget de test dans `calque` (couche `couche`).
pub fn poser_essai(
    couche: &Rc<CoucheWidgets>,
    calque: &CalqueWidgets,
    spec: SpecEssai,
) -> WidgetEssai {
    let etat = Rc::new(Etat::default());
    etat.hauteur.set(spec.hauteur);
    let hote = Rc::clone(couche.hote());
    let fabrique: Fabrique = {
        let (hote, couche, etat) = (Rc::downgrade(&hote), Rc::downgrade(couche), etat.clone());
        Rc::new(move |_: &Rc<dyn WidgetHandle>, el: ElementId| -> Contenu {
            let dessin = Rc::new(Dessin {
                sorte: spec.sorte,
                el,
                hote: hote.clone(),
                couche: couche.clone(),
                etat: etat.clone(),
            });
            Rc::new(move |window, cx| dessin.dessiner(window, cx))
        })
    };
    let declare = WidgetDeclare::new(&hote, fabrique);
    let handle = calque.layer().add_widget(
        WidgetSource::Lazy(declare.clone()),
        spec.ancre,
        WidgetOptions::default(),
    );
    let e = etat.clone();
    let seuil = spec.sorte == SorteEssai::Seuil;
    // ★ Le rappel est SYNCHRONE, pendant la passe de l'arbitre : le seuil change de forme avant
    //   d'être mesuré (l'image suivante le mesure et l'arbitre converge).
    let _off = handle.on_room_change(Box::new(move |room| {
        e.annonces.set(e.annonces.get() + 1);
        if seuil {
            e.compact.set(room.is_some_and(|r| r < SEUIL));
        }
    }));
    WidgetEssai {
        spec,
        handle,
        declare,
        etat,
    }
}

/// Ce que dessine un widget de test construit (une instance par construction).
struct Dessin {
    sorte: SorteEssai,
    el: ElementId,
    hote: Weak<HoteCalques>,
    couche: Weak<CoucheWidgets>,
    etat: Rc<Etat>,
}

impl Dessin {
    fn dessiner(&self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let t = theme_ou_defaut(cx);
        let p = &t.palette;
        let hote = self.hote.upgrade();
        let fiche = hote.as_ref().and_then(|h| h.fiche(self.el));
        let imposee = fiche.as_ref().and_then(|f| f.largeur_imposee).is_some();
        let place = fiche.as_ref().and_then(|f| f.place);
        let actif = hote
            .as_ref()
            .is_some_and(|h| h.activation().active_el() == Some(self.el));
        let n = self.etat.hauteur.get();
        let hauteur = match self.sorte {
            SorteEssai::Rigide | SorteEssai::Vue => n,
            // Plié à sa place, jamais au-delà de sa hauteur naturelle.
            SorteEssai::Flex => match place {
                Some(r) if r.is_finite() => n.min(r.max(0.0)),
                _ => n,
            },
            SorteEssai::Seuil if self.etat.compact.get() => COMPACT,
            SorteEssai::Seuil => n,
        };
        let etat = self.etat.clone();
        let releve = canvas(
            move |b, _, _| *etat.pose.borrow_mut() = Some(b),
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let focus = self.poignee(cx);
        let (f, e) = (focus.clone(), self.etat.clone());
        // Un clic (appui puis relâché sur le MÊME widget) compte ; l'appui donne le focus.
        let mut boite = div()
            .id(("widget-essai", self.el.0 as usize))
            .relative()
            .h(u(cx, hauteur as f32))
            .track_focus(&focus)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| window.focus(&f, cx))
            .on_click(move |_, _, _| e.clics.set(e.clics.get() + 1))
            .child(releve);
        boite = if imposee {
            boite.w_full()
        } else {
            boite.w(u(cx, LARGEUR_LIBRE as f32))
        };
        boite = match self.sorte {
            SorteEssai::Rigide => boite.child(self.globe(cx)),
            SorteEssai::Flex | SorteEssai::Seuil => {
                // Le fond rouge à 15 % et un contour d'un pixel posé DEHORS (il ne prend pas de
                // place, comme l'`outline` de la fixture).
                let contour = div()
                    .absolute()
                    .top(u(cx, -1.0))
                    .left(u(cx, -1.0))
                    .right(u(cx, -1.0))
                    .bottom(u(cx, -1.0))
                    .border_1()
                    .border_color(couleur(FIXTURE_ROUGE));
                boite.bg(couleur(FIXTURE_ROUGE_FOND)).child(contour)
            }
            // La vue nue complète : un bleu à 15 %, coins de 8 px.
            SorteEssai::Vue => boite
                .rounded(u(cx, RAYON_VUE as f32))
                .bg(couleur(FIXTURE_BLEU_VUE)),
        };
        if actif && self.sorte == SorteEssai::Vue {
            // L'anneau de l'actif : par-dessus le contenu, il ne prend pas de place.
            boite = boite.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded(u(cx, RAYON_VUE as f32))
                    .border_2()
                    .border_color(couleur(p.interactive_accent)),
            );
        }
        boite.into_any_element()
    }

    /// Le globe du rigide : une image calculée à la taille exacte de la boîte, posée 1:1.
    fn globe(&self, _cx: &mut App) -> AnyElement {
        let etat = self.etat.clone();
        canvas(
            |_, _, _| {},
            move |b, _, window, cx| {
                let echelle = window.scale_factor();
                let l = (f32::from(b.size.width) * echelle).round().max(1.0) as u32;
                let h = (f32::from(b.size.height) * echelle).round().max(1.0) as u32;
                let image = {
                    let mut g = etat.globe.borrow_mut();
                    match g.as_ref() {
                        Some((t, i)) if *t == (l, h) => i.clone(),
                        ancien => {
                            if let Some((_, vieille)) = ancien {
                                cx.drop_image(vieille.clone(), Some(window));
                            }
                            let Some(tampon) = image::RgbaImage::from_raw(l, h, globe_bgra(l, h))
                            else {
                                return;
                            };
                            let i = Arc::new(RenderImage::new(vec![image::Frame::new(tampon)]));
                            *g = Some(((l, h), i.clone()));
                            i
                        }
                    }
                };
                let _ = window.paint_image(b, b, Corners::default(), image, 0, false);
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }

    /// La poignée de focus du widget (une par widget de test, gardée d'une construction à
    /// l'autre), confiée à la couche pour l'élément de CETTE construction.
    fn poignee(&self, cx: &mut App) -> FocusHandle {
        let h = self
            .etat
            .focus
            .borrow_mut()
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        if let Some(c) = self.couche.upgrade() {
            c.suivre_focus(self.el, h.clone());
        }
        h
    }
}
