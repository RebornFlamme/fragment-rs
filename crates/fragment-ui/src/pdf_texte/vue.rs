// Port de app/src/ui/views/pdf/PdfView.ts (batirTexte, batirLiens, le clic des liens externes,
// getEditor) et de app/src/ui/views/pdf/pdf-view.css (.pdf-text-layer ::selection,
// .pdf-annotation-layer .linkAnnotation)
//
// La couche texte d'un PDF, montée sur GPUI par-dessus la colonne des pages (`PagesPdf`) :
// - elle tient l'instantané de sa `SurfacePdf` (géométrie, échelle, défilement, origine) à jour
//   à chaque image ;
// - elle demande le texte d'une page quand son image est là (le texte APRÈS l'image : c'est du
//   confort, l'image prime) et l'oublie quand la page est libérée ;
// - elle fait la sélection au glisser et la peint, et rend les liens cliquables (internes :
//   vers la page ; externes : le navigateur du système) ;
// - un outil armé (contrat pour T5) prend le pointeur : ni sélection ni liens, les gestes sont
//   émis en `PointeurPdf`.
//
// ★ Le pointeur arrive au parent de la colonne en phase de bouillonnement : la molette et le
//   pinch restent à `PagesPdf`, les boutons sont à cette couche. Les rects de lien et de
//   sélection n'occultent rien.

use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use fragment_pdf::document::CibleLien;
use fragment_text::{DocumentSurface as _, Rect, TextSurface as _};
use fragment_theme::Couleur;
use gpui_kit::{
    App, Bounds, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels as PixelsGpui, Render,
    Styled as _, Subscription, Task, Window, canvas, div,
};

use super::modele::TextePage;
use super::source::FilDeTexte;
use super::surface::{PlacePage, SurfacePdf};
use crate::echelle::{Echelle, u};
use crate::pdf_pages::PagesPdf;
use crate::theme_global::{couleur, theme_ou_defaut};

/// `.pdf-text-layer ::selection` : l'accent à 35 %, sur transparent.
const SELECTION_POURCENT: f32 = 35.0;
/// `.linkAnnotation > a:hover` : l'accent à 18 %, sur transparent.
const LIEN_SURVOL_POURCENT: f32 = 18.0;
/// Délai entre deux récoltes du fil du texte (ms).
const RECOLTE_MS: u64 = 4;

/// Le moment d'un geste de pointeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhasePointeur {
    Bas,
    Deplace,
    Haut,
}

/// Un geste de pointeur pris par l'outil armé (contrat de T5) : la position du document sous le
/// pointeur (`pos_at_coords`) et le point de la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointeurPdf {
    pub phase: PhasePointeur,
    pub pos: Option<usize>,
    pub x: f64,
    pub y: f64,
}

/// Le geste en cours.
#[derive(Debug, Clone, PartialEq)]
enum Geste {
    Selection,
    /// Un bouton enfoncé sur un lien : (page, rang du lien).
    Lien(usize, usize),
    Outil,
}

/// La couche texte d'un PDF ouvert, au-dessus de sa colonne de pages.
pub struct TextePdf {
    pages: Entity<PagesPdf>,
    fil: FilDeTexte,
    surface: SurfacePdf,
    /// Les pages dont le texte est demandé et pas encore arrivé.
    demandees: HashSet<usize>,
    outil_arme: bool,
    geste: Option<Geste>,
    recolte: Option<Task<()>>,
    _observation: Subscription,
}

impl EventEmitter<PointeurPdf> for TextePdf {}

fn point(x: PixelsGpui, y: PixelsGpui) -> (f64, f64) {
    (f64::from(f32::from(x)), f64::from(f32::from(y)))
}

impl TextePdf {
    /// La couche texte de `pages`, nourrie par `fil` (le même document).
    pub fn new(pages: Entity<PagesPdf>, fil: FilDeTexte, cx: &mut Context<Self>) -> Self {
        let observation = cx.observe(&pages, |_, _, cx| cx.notify());
        Self {
            pages,
            fil,
            surface: SurfacePdf::new(),
            demandees: HashSet::new(),
            outil_arme: false,
            geste: None,
            recolte: None,
            _observation: observation,
        }
    }

    /// Le document change (même vue) : le texte et la géométrie mémorisés ne valent plus rien.
    pub fn remplacer(&mut self, fil: FilDeTexte, cx: &mut Context<Self>) {
        self.fil = fil;
        self.demandees.clear();
        self.geste = None;
        {
            let mut e = self.surface.etat.borrow_mut();
            e.textes.clear();
            e.memoire.clear();
            e.selection = None;
        }
        cx.notify();
    }

    /// La colonne des pages.
    pub fn pages(&self) -> &Entity<PagesPdf> {
        &self.pages
    }

    /// La surface de texte (`getEditor` de la vue PDF).
    pub fn surface(&self) -> SurfacePdf {
        self.surface.clone()
    }

    /// Arme ou désarme un outil d'annotation.
    pub fn armer_outil(&mut self, arme: bool, cx: &mut Context<Self>) {
        self.outil_arme = arme;
        if arme {
            self.surface.etat.borrow_mut().selection = None;
        }
        self.geste = None;
        cx.notify();
    }

    /// La couche prend-elle le pointeur pour un outil ? (TS : `pointer-events: auto` sur la
    /// surface d'annotation seulement quand un outil est armé.)
    pub fn capte_pointeur(&self) -> bool {
        self.outil_arme
    }

    /// Le texte de la page `page` est-il lu ?
    pub fn texte_pret(&self, page: usize) -> bool {
        self.surface.etat.borrow().textes.contains_key(&page)
    }

    /// Des lectures de texte sont attendues.
    pub fn lectures_en_cours(&self) -> bool {
        !self.demandees.is_empty()
    }

    // ── L'instantané et le texte ─────────────────────────────────────────

    /// Relit la vue des pages : géométrie, échelle, défilement, bandes ; demande le texte des
    /// pages peintes, oublie celui des pages libérées.
    fn synchroniser(&mut self, cx: &mut Context<Self>) {
        let zoom = f64::from(Echelle::courante(cx).zoom());
        let pages = self.pages.read(cx);
        let m = pages.modele();
        let g = m.geometrie();
        let places: Vec<PlacePage> = (0..m.nombre_pages())
            .map(|i| {
                let c = g.coin(i);
                PlacePage {
                    top_px: g.top_px(i),
                    hauteur_px: g.hauteur_px(i),
                    coin: Rect {
                        left: c.left,
                        top: c.top,
                        right: c.right,
                        bottom: c.bottom,
                    },
                }
            })
            .collect();
        let peintes: HashSet<usize> = pages
            .pages_occupees()
            .into_iter()
            .filter(|&p| {
                let e = pages.etat_images(p);
                e.courante.is_some() || e.perimee.is_some()
            })
            .collect();
        let (echelle, defilement, visibles, dessinees) = (
            m.echelle(),
            (m.scroll_top(), m.scroll_left()),
            m.pages_visibles(),
            pages.plages_dessinees(),
        );
        let a_demander: Vec<usize> = {
            let mut e = self.surface.etat.borrow_mut();
            e.places = places;
            e.echelle = echelle;
            e.zoom = zoom;
            e.defilement = defilement;
            e.visibles = visibles;
            e.dessinees = dessinees;
            e.textes.retain(|p, _| peintes.contains(p));
            let mut v: Vec<usize> = peintes
                .iter()
                .copied()
                .filter(|p| !e.textes.contains_key(p) && !self.demandees.contains(p))
                .collect();
            v.sort_unstable();
            v
        };
        self.demandees.retain(|p| peintes.contains(p));
        for p in a_demander {
            self.demandees.insert(p);
            self.fil.demander(p);
        }
        if self.fil.occupe() && self.recolte.is_none() {
            self.recolte = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(RECOLTE_MS))
                        .await;
                    let encore = this.update(cx, |v, cx| v.recolter(cx)).unwrap_or(false);
                    if !encore {
                        break;
                    }
                }
            }));
        }
    }

    /// Prend les lectures du fil. Rend vrai s'il faut continuer à récolter.
    fn recolter(&mut self, cx: &mut Context<Self>) -> bool {
        let mut change = false;
        for (page, lu) in self.fil.recolter() {
            // Une page libérée entre-temps : sa lecture est jetée.
            if !self.demandees.remove(&page) {
                continue;
            }
            // Un scan, une police illisible : l'absence de texte est un état NORMAL.
            let texte = Rc::new(lu.unwrap_or_else(|_| TextePage::default()));
            let mut e = self.surface.etat.borrow_mut();
            e.memoire.insert(page, Rc::clone(&texte));
            e.textes.insert(page, texte);
            change = true;
        }
        if change {
            // Ce qui ne déplace rien : une seule passe des calques par lot.
            self.surface.request_update();
            cx.notify();
        }
        let encore = self.fil.occupe();
        if !encore {
            self.recolte = None;
        }
        encore
    }

    // ── Le pointeur ──────────────────────────────────────────────────────

    fn emettre(&self, phase: PhasePointeur, x: f64, y: f64, cx: &mut Context<Self>) {
        let pos = self.surface.pos_at_coords(x, y);
        cx.emit(PointeurPdf { phase, pos, x, y });
    }

    fn bas(&mut self, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        let (x, y) = point(ev.position.x, ev.position.y);
        if self.outil_arme {
            self.geste = Some(Geste::Outil);
            self.emettre(PhasePointeur::Bas, x, y, cx);
            cx.stop_propagation();
            return;
        }
        if let Some((page, rang, _)) = self.surface.lien_sous(x, y) {
            self.geste = Some(Geste::Lien(page, rang));
            return;
        }
        let caret = self.surface.caret_sous(x, y);
        self.surface.etat.borrow_mut().selection = caret.map(|p| (p, p));
        self.geste = caret.map(|_| Geste::Selection);
        cx.notify();
    }

    fn deplace(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        if ev.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let (x, y) = point(ev.position.x, ev.position.y);
        match self.geste {
            Some(Geste::Outil) => self.emettre(PhasePointeur::Deplace, x, y, cx),
            Some(Geste::Selection) => {
                if let Some(tete) = self.surface.caret_proche(x, y) {
                    let mut e = self.surface.etat.borrow_mut();
                    if let Some((_, t)) = e.selection.as_mut() {
                        *t = tete;
                    }
                    drop(e);
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    fn haut(&mut self, ev: &MouseUpEvent, cx: &mut Context<Self>) {
        let (x, y) = point(ev.position.x, ev.position.y);
        match self.geste.take() {
            Some(Geste::Outil) => self.emettre(PhasePointeur::Haut, x, y, cx),
            Some(Geste::Lien(page, rang)) => {
                if let Some((p, r, cible)) = self.surface.lien_sous(x, y)
                    && (p, r) == (page, rang)
                {
                    self.suivre(cible, cx);
                }
            }
            _ => {}
        }
    }

    /// Suit un lien : une page du document, ou le navigateur du système (jamais la fenêtre).
    pub fn suivre(&mut self, cible: CibleLien, cx: &mut Context<Self>) {
        match cible {
            CibleLien::Interne { page, y } => {
                let page = page as usize;
                self.pages
                    .update(cx, |p, cx| p.agir(cx, |m| m.aller_a_la_page(page, y)));
            }
            CibleLien::Externe { uri } => cx.open_url(&uri),
        }
    }
}

impl Render for TextePdf {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.synchroniser(cx);
        let theme = theme_ou_defaut(cx);
        let accent = theme.palette.color_accent;
        let selection = couleur(accent.melange(SELECTION_POURCENT, Couleur::TRANSPARENT));
        let survol = couleur(accent.melange(LIEN_SURVOL_POURCENT, Couleur::TRANSPARENT));
        let rayon = u(cx, theme.forme.radius_xxs);
        let zero = u(cx, 0.0);

        // L'origine de la zone dans la fenêtre, mesurée au prépeint (celle de la colonne).
        let etat = Rc::clone(&self.surface.etat);
        let origine = canvas(
            move |b: Bounds<PixelsGpui>, _: &mut Window, _: &mut App| {
                etat.borrow_mut().origine = point(b.origin.x, b.origin.y);
            },
            |_, (), _: &mut Window, _: &mut App| {},
        )
        .absolute()
        .top(zero)
        .left(zero)
        .size_full();

        // Du plan (unités non zoomées) vers la zone (px CSS, avant le zoom de l'app).
        let (echelle, (haut, gauche)) = {
            let e = self.surface.etat.borrow();
            (e.echelle, e.defilement)
        };
        let boite = |r: Rect, cx: &App| {
            div()
                .absolute()
                .left(u(cx, (r.left * echelle - gauche) as f32))
                .top(u(cx, (r.top * echelle - haut) as f32))
                .w(u(cx, ((r.right - r.left) * echelle) as f32))
                .h(u(cx, ((r.bottom - r.top) * echelle) as f32))
        };

        let plage = self.surface.get_selection();
        let mut calque = Vec::new();
        if plage.from != plage.to {
            for r in self.surface.coords_for_range(plage.from, plage.to) {
                calque.push(boite(r, cx).bg(selection).into_any_element());
            }
        }
        // Les liens : sous le plan des calques, donc absents quand un outil est armé.
        if !self.outil_arme {
            let e = self.surface.etat.borrow();
            let mut pages: Vec<_> = e.textes.iter().collect();
            pages.sort_unstable_by_key(|(p, _)| **p);
            for (page, t) in pages {
                let Some(place) = e.places.get(page - 1) else {
                    continue;
                };
                for (rang, l) in t.liens.iter().enumerate() {
                    let r = Rect {
                        left: place.coin.left + l.boite.left,
                        top: place.coin.top + l.boite.top,
                        right: place.coin.left + l.boite.right,
                        bottom: place.coin.top + l.boite.bottom,
                    };
                    calque.push(
                        boite(r, cx)
                            .id(("pdf-lien", page * 10_000 + rang))
                            .rounded(rayon)
                            .cursor_pointer()
                            .hover(move |s| s.bg(survol))
                            .into_any_element(),
                    );
                }
            }
        }

        div()
            .id("pdf-texte")
            .relative()
            .size_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| this.bas(ev, cx)),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| this.deplace(ev, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseUpEvent, _, cx| this.haut(ev, cx)),
            )
            .child(self.pages.clone())
            .child(
                div()
                    .absolute()
                    .top(zero)
                    .left(zero)
                    .size_full()
                    .overflow_hidden()
                    .child(origine)
                    .children(calque),
            )
    }
}
