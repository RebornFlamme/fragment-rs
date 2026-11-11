// Port de app/src/ui/views/pdf/PdfView.ts (pages, peindre, liberer, repeindre, recaler, défilement,
// molette et pinch) et de app/src/ui/views/pdf/devicePixels.ts (taille mesurée des canvas)
//
// La colonne des pages d'un PDF, montée sur GPUI. Le modèle (`VueModele`, fragment-pdf) décide
// de tout : géométrie, zoom, bande peinte, fin de geste. Cette vue :
// - mesure la zone et la densité (facteur de la fenêtre × zoom de l'app) et les lui livre ;
// - applique ses plans aux images des pages (`Emplacements`) et au fil de rendu ;
// - peint, pour chaque page, sa boîte blanche ombrée, l'image périmée étirée dessous et
//   l'image courante dessus ;
// - mesure, à chaque image, les pixels physiques où chaque boîte sera peinte (l'accrochage
//   de GPUI à la grille de l'écran) : une demande de rendu attend cette mesure, comme le
//   canvas de Fragment attendait son `ResizeObserver`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fragment_pdf::document::TaillePage;
use fragment_pdf::pdf_zoom::Taille;
use fragment_pdf::vue_modele::{PlanRendu, Point, SENSIBILITE_MOLETTE, VueModele};
use fragment_text::EditorRange;
use gpui_kit::{
    App, Bounds, Context, Corners, InteractiveElement as _, IntoElement, ParentElement as _,
    PinchEvent, Pixels as PixelsGpui, Render, RenderImage, ScrollEvent, Styled as _, Task, Window,
    canvas, div,
};

use super::emplacements::{Emplacements, EtatImages, ImagePage};
use super::fil::{FilDeRendu, Reponse};
use super::{ESPACE_PAGES, LIGNE_MOLETTE, MARGE_HAUTE};
use crate::echelle::{Echelle, u};
use crate::flottant::ombrer;
use crate::theme_global::{couleur, theme_ou_defaut};

pub use fragment_pdf::vue_modele::Pixels;

/// Délai entre deux récoltes des rendus du fil (ms) : le fil ne peut pas réveiller l'interface.
const RECOLTE_MS: u64 = 4;

/// L'arrondi de GPUI pour accrocher un bord à la grille physique : la moitié va vers zéro.
fn arrondi_gpui(v: f32) -> f32 {
    (v.abs() - 0.5).ceil().copysign(v)
}

/// Les pixels physiques où GPUI peindra une boîte (`snap_bounds` de la fenêtre) : bords
/// accrochés, PAS `arrondi(largeur × facteur)`. `None` pour une boîte vide.
pub fn pixels_peints(b: Bounds<PixelsGpui>, facteur: f32) -> Option<Pixels> {
    let g = arrondi_gpui(f32::from(b.left()) * facteur);
    let d = arrondi_gpui(f32::from(b.right()) * facteur).max(g);
    let h = arrondi_gpui(f32::from(b.top()) * facteur);
    let bas = arrondi_gpui(f32::from(b.bottom()) * facteur).max(h);
    let (w, hh) = (d - g, bas - h);
    (w >= 1.0 && hh >= 1.0).then_some(Pixels {
        w: w as u32,
        h: hh as u32,
    })
}

/// La colonne des pages d'un PDF ouvert.
pub struct PagesPdf {
    modele: VueModele,
    fil: Option<FilDeRendu>,
    emplacements: Emplacements<Arc<RenderImage>>,
    /// Les tailles des pages, reçues du fil, en attendant que la zone soit mesurée.
    tailles_en_attente: Option<Vec<TaillePage>>,
    attend_ouverture: bool,
    erreur: Option<String>,
    /// Zone mesurée : origine (px logiques, fenêtre), taille en px CSS.
    zone_connue: bool,
    origine: gpui_kit::Point<PixelsGpui>,
    zone_css: (f64, f64),
    densite: f64,
    /// Les mesures physiques des boîtes, posées au prépeint et relues après l'image.
    mesures_image: Rc<RefCell<Vec<(usize, Pixels)>>>,
    /// La dernière mesure livrée au modèle, par page.
    livrees: HashMap<usize, Pixels>,
    /// Les images jetées, à retirer de l'atlas de GPUI à la prochaine peinture.
    a_jeter: Rc<RefCell<Vec<Arc<RenderImage>>>>,
    horloge: Option<Instant>,
    echeance: Option<(u64, Task<()>)>,
    recolte: Option<Task<()>>,
    recolte_active: bool,
}

impl PagesPdf {
    /// Une vue sur le document que rend `fil` (ouvert par lui, de façon asynchrone).
    pub fn new(fil: FilDeRendu, cx: &mut Context<Self>) -> Self {
        let mut v = Self {
            modele: VueModele::new(),
            fil: None,
            emplacements: Emplacements::new(),
            tailles_en_attente: None,
            attend_ouverture: false,
            erreur: None,
            zone_connue: false,
            origine: gpui_kit::Point::default(),
            zone_css: (0.0, 0.0),
            densite: 1.0,
            mesures_image: Rc::new(RefCell::new(Vec::new())),
            livrees: HashMap::new(),
            a_jeter: Rc::new(RefCell::new(Vec::new())),
            horloge: None,
            echeance: None,
            recolte: None,
            recolte_active: false,
        };
        v.remplacer(fil, cx);
        v
    }

    /// Ferme le document courant et ouvre celui que rend `fil`. Le mode et l'échelle restent.
    pub fn remplacer(&mut self, fil: FilDeRendu, cx: &mut Context<Self>) {
        self.modele.fermer();
        let travail = self.emplacements.reinitialiser(0);
        self.a_jeter.borrow_mut().extend(travail.jetees);
        self.livrees.clear();
        self.erreur = None;
        self.tailles_en_attente = None;
        self.fil = Some(fil);
        self.attend_ouverture = true;
        self.lancer_recolte(cx);
        cx.notify();
    }

    // ── Lecture ──────────────────────────────────────────────────────────

    /// Le modèle de la vue (échelle, mode, page courante, champ…), pour la barre d'outils.
    pub fn modele(&self) -> &VueModele {
        &self.modele
    }

    /// L'erreur d'ouverture du document, s'il y en a une.
    pub fn erreur(&self) -> Option<&str> {
        self.erreur.as_deref()
    }

    /// Les plages de positions dessinées (`renderedRanges` de l'éditeur du PDF).
    pub fn plages_dessinees(&self) -> Vec<EditorRange> {
        self.emplacements.plages_dessinees()
    }

    /// Ce qui est affiché pour `page` (à partir de 1).
    pub fn etat_images(&self, page: usize) -> EtatImages {
        self.emplacements.etat(page)
    }

    /// Les pages qui ont une image ou un rendu, à partir de 1.
    pub fn pages_occupees(&self) -> Vec<usize> {
        self.emplacements.occupees()
    }

    /// La dernière taille physique mesurée de la boîte de `page`.
    pub fn mesure(&self, page: usize) -> Option<Pixels> {
        self.livrees.get(&page).copied()
    }

    /// La densité livrée au modèle : facteur de la fenêtre × zoom de l'app.
    pub fn densite(&self) -> f64 {
        self.densite
    }

    /// Un rendu au moins est demandé ou en vol.
    pub fn rendus_en_cours(&self) -> bool {
        self.emplacements.en_cours()
    }

    // ── Action ───────────────────────────────────────────────────────────

    fn maintenant(&mut self, cx: &App) -> u64 {
        let now = cx.background_executor().now();
        let origine = *self.horloge.get_or_insert(now);
        u64::try_from(now.saturating_duration_since(origine).as_millis()).unwrap_or(u64::MAX)
    }

    /// LA porte d'entrée des événements : l'horloge du modèle avance, `f` agit sur lui, et
    /// son plan est appliqué. La barre d'outils et la vue montée passent par ici.
    pub fn agir(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut VueModele) -> PlanRendu) {
        let t = self.maintenant(cx);
        let mut plan = self.modele.avancer(t);
        plan.fusionner(f(&mut self.modele));
        self.executer(plan, cx);
        cx.notify();
    }

    fn executer(&mut self, plan: PlanRendu, cx: &mut Context<Self>) {
        let travail = self.emplacements.appliquer(&plan);
        if let Some(fil) = self.fil.as_mut() {
            for j in travail.annulees {
                fil.annuler(j);
            }
        }
        self.a_jeter.borrow_mut().extend(travail.jetees);
        self.programmer(cx);
        self.lancer_recolte(cx);
    }

    /// (Re)programme l'appel de `avancer` à la prochaine échéance du modèle (fin de geste,
    /// recalage des pixels).
    fn programmer(&mut self, cx: &mut Context<Self>) {
        let Some(t) = self.modele.prochaine_echeance() else {
            self.echeance = None;
            return;
        };
        if self.echeance.as_ref().is_some_and(|(e, _)| *e == t) {
            return;
        }
        let attente = t.saturating_sub(self.maintenant(cx));
        let tache = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(attente))
                .await;
            let _ = this.update(cx, |v, cx| {
                v.echeance = None;
                v.agir(cx, |_| PlanRendu::vide());
            });
        });
        self.echeance = Some((t, tache));
    }

    fn lancer_recolte(&mut self, cx: &mut Context<Self>) {
        let utile = self.attend_ouverture || self.fil.as_ref().is_some_and(FilDeRendu::occupe);
        if !utile || self.recolte_active {
            return;
        }
        self.recolte_active = true;
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

    /// Prend les réponses du fil. Rend vrai s'il faut continuer à récolter.
    fn recolter(&mut self, cx: &mut Context<Self>) -> bool {
        let reponses = self
            .fil
            .as_mut()
            .map(FilDeRendu::recolter)
            .unwrap_or_default();
        let mut change = false;
        for r in reponses {
            match r {
                Reponse::Ouvert(Ok(tailles)) => {
                    self.attend_ouverture = false;
                    self.tailles_en_attente = Some(tailles);
                    self.ouvrir_si_pret(cx);
                    change = true;
                }
                Reponse::Ouvert(Err(e)) => {
                    self.attend_ouverture = false;
                    self.erreur = Some(e);
                    change = true;
                }
                Reponse::Rendu {
                    page,
                    jeton,
                    resultat,
                } => {
                    let image = resultat
                        .ok()
                        .map(|(image, pixels)| ImagePage { image, pixels });
                    if let Ok((pixels, perimee)) = self.emplacements.recevoir(page, jeton, image) {
                        self.a_jeter.borrow_mut().extend(perimee);
                        self.modele.page_prete(page, pixels);
                        change = true;
                    }
                }
            }
        }
        if change {
            self.programmer(cx);
            cx.notify();
        }
        let encore = self.attend_ouverture || self.fil.as_ref().is_some_and(FilDeRendu::occupe);
        self.recolte_active = encore;
        encore
    }

    /// Ouvre le document dans le modèle dès que ses tailles ET la zone sont connues.
    fn ouvrir_si_pret(&mut self, cx: &mut Context<Self>) {
        if !self.zone_connue {
            return;
        }
        let Some(tailles) = self.tailles_en_attente.take() else {
            return;
        };
        let Some(premiere) = tailles.first().copied() else {
            return;
        };
        let travail = self.emplacements.reinitialiser(tailles.len());
        self.a_jeter.borrow_mut().extend(travail.jetees);
        let base = Taille {
            largeur: premiere.largeur,
            hauteur: premiere.hauteur,
        };
        let t = self.maintenant(cx);
        let mut plan = self.modele.avancer(t);
        plan.fusionner(self.modele.ouvrir(
            tailles.len(),
            base,
            ESPACE_PAGES,
            MARGE_HAUTE,
            self.densite,
        ));
        // Toutes les tailles sont lues à l'ouverture (pdfium les donne sans rendre) : les
        // pages inégales sont à leur taille dès le départ.
        for (i, p) in tailles.iter().enumerate().skip(1) {
            if p.largeur != base.largeur || p.hauteur != base.hauteur {
                plan.fusionner(self.modele.taille_page_connue(i + 1, p.largeur, p.hauteur));
            }
        }
        self.executer(plan, cx);
        cx.notify();
    }

    /// Après chaque image : la zone, la densité, puis les mesures des boîtes.
    fn apres_image(&mut self, zone: Bounds<PixelsGpui>, facteur: f32, cx: &mut Context<Self>) {
        let zoom = f64::from(Echelle::courante(cx).zoom());
        let densite = f64::from(facteur) * zoom;
        let largeur = f64::from(f32::from(zone.size.width)) / zoom;
        let hauteur = f64::from(f32::from(zone.size.height)) / zoom;
        self.origine = zone.origin;
        let t = self.maintenant(cx);
        let mut plan = self.modele.avancer(t);
        let mut change = false;
        if !self.zone_connue {
            if largeur > 0.0 {
                self.densite = densite;
                self.modele.definir_zone(largeur, 0.0, hauteur);
                self.zone_css = (largeur, hauteur);
                self.zone_connue = true;
                self.ouvrir_si_pret(cx);
                change = true;
            }
        } else {
            if (largeur, hauteur) != self.zone_css {
                self.zone_css = (largeur, hauteur);
                plan.fusionner(self.modele.zone_redimensionnee(largeur, hauteur));
                change = true;
            }
            if densite != self.densite {
                self.densite = densite;
                plan.fusionner(self.modele.set_densite(densite));
                change = true;
            }
        }
        change |= !plan.est_vide();
        self.executer(plan, cx);

        // Les mesures de cette image : au modèle (recalage d'une page qui a glissé) et aux
        // demandes qui les attendaient.
        let mesures: Vec<(usize, Pixels)> = self.mesures_image.borrow_mut().drain(..).collect();
        let mut soumis = false;
        for (page, p) in mesures {
            if self.livrees.get(&page) != Some(&p) {
                self.livrees.insert(page, p);
                self.modele.pixels_livres(page, Some(p));
            }
            if let Some(d) = self.emplacements.fournir_mesure(page, p)
                && let Some(fil) = self.fil.as_mut()
            {
                fil.soumettre(d);
                soumis = true;
            }
        }
        if soumis {
            self.lancer_recolte(cx);
        }
        self.programmer(cx);
        if change {
            cx.notify();
        }
    }

    fn point_css(&self, position: gpui_kit::Point<PixelsGpui>, zoom: f64) -> Point {
        Point {
            x: f64::from(f32::from(position.x - self.origine.x)) / zoom,
            y: f64::from(f32::from(position.y - self.origine.y)) / zoom,
        }
    }

    /// Molette : défilement, ou zoom continu ancré sous le curseur avec Ctrl/Cmd (le pinch d'un
    /// trackpad arrive aussi ainsi sous Windows).
    fn molette(&mut self, ev: &ScrollEvent, cx: &mut Context<Self>) {
        let zoom = f64::from(Echelle::courante(cx).zoom());
        let d = ev.delta.pixel_delta(u(cx, LIGNE_MOLETTE));
        let (dx, dy) = (
            f64::from(f32::from(d.x)) / zoom,
            f64::from(f32::from(d.y)) / zoom,
        );
        if ev.modifiers.control || ev.modifiers.platform {
            // GPUI : y positif = vers le haut ; le navigateur : deltaY positif = vers le bas.
            let point = self.point_css(ev.position, zoom);
            self.agir(cx, |m| m.zoomer_molette(-dy, 0, true, point).rendu);
        } else {
            let (haut, gauche) = (
                self.modele.scroll_top() - dy,
                self.modele.scroll_left() - dx,
            );
            self.agir(cx, |m| m.defiler(haut, gauche));
        }
        cx.stop_propagation();
    }

    /// Pinch natif (macOS) : `delta` = variation relative (0,1 = +10 %), ramenée au delta de
    /// molette qui donnerait la même échelle.
    fn pincer(&mut self, ev: &PinchEvent, cx: &mut Context<Self>) {
        let facteur = 1.0 + f64::from(ev.delta);
        if facteur <= 0.0 {
            return;
        }
        let zoom = f64::from(Echelle::courante(cx).zoom());
        let point = self.point_css(ev.position, zoom);
        let delta = -facteur.ln() / SENSIBILITE_MOLETTE;
        self.agir(cx, |m| m.zoomer_molette(delta, 0, true, point).rendu);
        cx.stop_propagation();
    }

    /// Les pages à poser : celles qui ont une image ou un rendu, et celles qui croisent la
    /// zone visible (leur boîte blanche donne la silhouette du document au défilement rapide).
    fn pages_a_poser(&self) -> Vec<usize> {
        let mut pages = self.emplacements.occupees();
        let g = self.modele.geometrie();
        let n = self.modele.nombre_pages();
        let haut = self.modele.scroll_top();
        let bas = haut + self.modele.zone().hauteur;
        if n > 0 {
            let mut i = g.page_sous_px(haut);
            while i < n && g.top_px(i) <= bas {
                pages.push(i + 1);
                i += 1;
            }
        }
        pages.sort_unstable();
        pages.dedup();
        pages
    }
}

impl Render for PagesPdf {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let entite = cx.entity().downgrade();
        let a_jeter = Rc::clone(&self.a_jeter);
        let zero = u(cx, 0.0);

        // La zone : mesurée au prépeint, livrée après l'image ; la peinture retire de l'atlas
        // les images jetées.
        let zone = canvas(
            move |bounds, window: &mut Window, cx: &mut App| {
                let facteur = window.scale_factor();
                cx.defer(move |cx| {
                    let _ = entite.update(cx, |v, cx| v.apres_image(bounds, facteur, cx));
                });
            },
            move |_, (), window: &mut Window, _| {
                for image in a_jeter.borrow_mut().drain(..) {
                    let _ = window.drop_image(image);
                }
            },
        )
        .absolute()
        .top(zero)
        .left(zero)
        .size_full();

        let g = self.modele.geometrie();
        let (haut, gauche) = (self.modele.scroll_top(), self.modele.scroll_left());
        let mut pages = Vec::new();
        for page in self.pages_a_poser() {
            let i = page - 1;
            let x = g.left_px(i) - gauche;
            let y = g.top_px(i) - haut;
            let (perimee, courante) = self.emplacements.images(page);
            let perimee = perimee.map(|p| Arc::clone(&p.image));
            let courante = courante.map(|p| Arc::clone(&p.image));
            let mesures = Rc::clone(&self.mesures_image);
            let images = canvas(
                move |bounds, window: &mut Window, _: &mut App| {
                    if let Some(p) = pixels_peints(bounds, window.scale_factor()) {
                        mesures.borrow_mut().push((page, p));
                    }
                },
                move |bounds, (), window: &mut Window, _| {
                    // L'image périmée DESSOUS, étirée à la boîte ; la courante par-dessus.
                    for image in [perimee, courante].into_iter().flatten() {
                        let _ =
                            window.paint_image(bounds, bounds, Corners::default(), image, 0, false);
                    }
                },
            )
            .absolute()
            .top(zero)
            .left(zero)
            .size_full();
            let boite = div()
                .absolute()
                .left(u(cx, x as f32))
                .top(u(cx, y as f32))
                .w(u(cx, g.largeur_px(i) as f32))
                .h(u(cx, g.hauteur_px(i) as f32))
                .bg(couleur(theme.palette.pdf_page_bg))
                .child(images);
            pages.push(ombrer(
                boite,
                cx,
                &theme.palette.shadow_s,
                "shadow-s",
                zero,
                zero,
            ));
        }

        div()
            .id("pdf-pages")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(couleur(theme.palette.background_secondary))
            .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| this.molette(ev, cx)))
            .on_pinch(cx.listener(|this, ev: &PinchEvent, _, cx| this.pincer(ev, cx)))
            .child(zone)
            .children(pages)
    }
}
