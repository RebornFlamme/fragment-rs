// Port de app/src/ui/views/pdf/PdfView.ts (onOpen, construireBarreOutils, onLoadFile : la barre
// d'outils sous l'en-tête, la barre latérale et la zone des pages côte à côte, le relais des
// menus, des vignettes et des raccourcis) et de app/src/ui/core/workspace/ItemView.ts
// (`overlayPane` : le conteneur où les calques posent leurs éléments, ici la zone des pages).
//! [`MontagePdf`] : l'assemblage GPUI d'une [`super::VuePdf`].
//!
//! ```text
//! colonne ─ barre d'outils (36 px)
//!         └ rangée ─ barre latérale (si ouverte) ─ zone des pages (flex 1) = overlayPane
//!                                                   └ recherche ─ couche texte ─ pages
//! ```
//!
//! ★ La zone des pages est un voisin FLEXIBLE de la barre latérale : ouvrir la barre rétrécit la
//!   zone, `PagesPdf` mesure sa nouvelle largeur et « Page width » suit (le modèle ignore une
//!   largeur nulle : vue cachée, rien n'est perdu au retour).
//! ★ Toute action sur le zoom ou la page passe par `PagesPdf::agir`, qui possède le modèle.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::View;
use fragment_pdf::laterale::ModeBarre as ModeLaterale;
use fragment_pdf::vue_modele::{PlanRendu, PositionLecture, Reprise, VueModele};
use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, Context, Element, ElementId, Entity, FocusHandle,
    GlobalElementId, InspectorElementId, InteractiveElement as _, IntoElement, LayoutId,
    ParentElement as _, Pixels, Render, Styled as _, Subscription, Task, Window, div,
};
use serde_json::{Map, Value, json};

use super::document::DocumentPartage;
use super::vue::{LiensPdf, Touche, VuePdf};
use crate::lecture::CLE_SCROLL;
use crate::pdf_barre::rendu::SurBouton;
use crate::pdf_barre::{BarrePdf, Bouton, ModeBarre, rendre_barre};
use crate::pdf_laterale::{EvenementLaterale, LateralePdf};
use crate::pdf_pages::PagesPdf;
use crate::pdf_recherche::RecherchePdf;
use crate::pdf_texte::TextePdf;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La cellule où une vue garde son montage.
pub type CelluleMontage = Rc<RefCell<Option<Entity<MontagePdf>>>>;

/// Le contenu d'une vue pour [`crate::onglets::ContenuDeVue`] : le PDF, si la vue en est un.
pub fn contenu(vue: &dyn View, _: &App) -> Option<AnyElement> {
    let pdf = vue.as_any().downcast_ref::<VuePdf>()?;
    Some(ElementMontage::new(pdf.liens().clone()).into_any_element())
}

/// Les clés de l'état éphémère (celles de Fragment, une API : workspace, plugins).
pub mod cles {
    pub const SIDEBAR: &str = "sidebar";
    pub const FIND: &str = "find";
}

/// L'assemblage monté d'une vue PDF.
pub struct MontagePdf {
    liens: LiensPdf,
    revision: u64,
    erreur: Option<String>,
    document: Option<DocumentPartage>,
    pages: Option<Entity<PagesPdf>>,
    recherche: Option<Entity<RecherchePdf>>,
    laterale: Option<Entity<LateralePdf>>,
    barre: Rc<RefCell<BarrePdf>>,
    /// Mode et largeur de barre latérale restaurés avant qu'elle n'existe.
    laterale_en_attente: Option<(Option<String>, Option<f64>)>,
    reprise: Option<Reprise>,
    focus: FocusHandle,
    _toc: Option<Task<()>>,
    _abonnements: Vec<Subscription>,
}

impl MontagePdf {
    pub fn new(liens: LiensPdf, cx: &mut Context<Self>) -> Self {
        Self {
            liens,
            revision: 0,
            erreur: None,
            document: None,
            pages: None,
            recherche: None,
            laterale: None,
            barre: Rc::new(RefCell::new(BarrePdf::new())),
            laterale_en_attente: None,
            reprise: None,
            focus: cx.focus_handle(),
            _toc: None,
            _abonnements: Vec::new(),
        }
    }

    // ── Accès (tests, calques) ───────────────────────────────────────────

    pub fn pages(&self) -> Option<&Entity<PagesPdf>> {
        self.pages.as_ref()
    }
    pub fn recherche(&self) -> Option<&Entity<RecherchePdf>> {
        self.recherche.as_ref()
    }
    pub fn laterale(&self) -> Option<&Entity<LateralePdf>> {
        self.laterale.as_ref()
    }
    pub fn barre(&self) -> std::cell::Ref<'_, BarrePdf> {
        self.barre.borrow()
    }
    pub fn erreur(&self) -> Option<&str> {
        self.erreur.as_deref()
    }

    /// Agit sur le modèle de la vue (zoom, page) : `None` sans document.
    pub fn agir(&self, cx: &mut App, f: impl FnOnce(&mut VueModele) -> PlanRendu) {
        if let Some(p) = &self.pages {
            p.update(cx, |p, cx| p.agir(cx, f));
        }
    }

    /// Un appui sur un bouton de la barre d'outils : son menu s'ouvre sous le pointeur.
    pub fn cliquer(&mut self, b: Bouton, x: f32, y: f32, cx: &mut Context<Self>) {
        let barre = self.barre.clone();
        barre.borrow_mut().appui_sur(b);
        let mut menu = None;
        self.agir(cx, |m| {
            let (mn, plan) = barre.borrow_mut().cliquer(b, m);
            menu = mn;
            plan
        });
        if self.pages.is_none() && b == Bouton::BasculerBarre {
            barre.borrow_mut().basculer_barre();
        }
        if let Some(menu) = menu {
            menu.show_at_position(x, y, &self.liens.keymap);
        }
        cx.notify();
    }

    // ── Le document ──────────────────────────────────────────────────────

    /// Ouvre (ou remplace) le document quand la vue a chargé un autre fichier.
    fn synchroniser_document(&mut self, cx: &mut Context<Self>) {
        let source = self.liens.source.borrow().clone();
        if source.revision == self.revision {
            return;
        }
        self.revision = source.revision;
        let octets = match source.octets {
            Some(Ok(o)) => o,
            Some(Err(e)) => {
                self.erreur = Some(e);
                return;
            }
            None => return,
        };
        self.erreur = None;
        let doc = DocumentPartage::ouvrir(octets.as_ref().clone());
        if let (Some(p), Some(r)) = (&self.pages, &self.recherche) {
            p.update(cx, |p, cx| p.remplacer(doc.fil_de_rendu(), cx));
            let texte = r.read(cx).texte().clone();
            texte.update(cx, |t, cx| t.remplacer(doc.fil_de_texte(), cx));
            r.update(cx, |r, cx| r.remplacer(doc.fil_de_texte(), cx));
        } else {
            let pages = cx.new(|cx| PagesPdf::new(doc.fil_de_rendu(), cx));
            let fil = doc.fil_de_texte();
            let texte = cx.new(|cx| TextePdf::new(pages.clone(), fil, cx));
            let keymap = self.liens.keymap.clone();
            let fil = doc.fil_de_texte();
            let recherche = cx.new(|cx| RecherchePdf::new(texte, fil, keymap, cx));
            self._abonnements
                .push(cx.observe(&pages, |_, _, cx| cx.notify()));
            self._abonnements
                .push(cx.observe(&recherche, |_, _, cx| cx.notify()));
            self.pages = Some(pages);
            self.recherche = Some(recherche);
        }
        // La table des matières arrive du fil du document ; la barre latérale naît avec elle.
        let attente = doc.signets();
        self._toc = Some(cx.spawn(async move |this, cx| {
            let toc = cx
                .background_executor()
                .spawn(async move { attente.recv().ok().and_then(Result::ok) })
                .await
                .unwrap_or_default();
            let _ = this.update(cx, |m, cx| m.toc_arrivee(toc, cx));
        }));
        self.document = Some(doc);
        cx.notify();
    }

    fn toc_arrivee(
        &mut self,
        toc: Vec<fragment_pdf::pdf_outline::EntreeToc>,
        cx: &mut Context<Self>,
    ) {
        let Some(doc) = self.document.clone() else {
            return;
        };
        if let Some(l) = &self.laterale {
            l.update(cx, |l, cx| l.remplacer(doc.fil_de_rendu(), toc, cx));
        } else {
            let laterale = cx.new(|cx| LateralePdf::new(doc.fil_de_rendu(), toc, cx));
            self._abonnements
                .push(cx.subscribe(&laterale, |m, _, ev: &EvenementLaterale, cx| {
                    m.sur_laterale(*ev, cx);
                }));
            self._abonnements
                .push(cx.observe(&laterale, |_, _, cx| cx.notify()));
            self.laterale = Some(laterale);
        }
        if let (Some((mode, largeur)), Some(l)) = (self.laterale_en_attente.take(), &self.laterale)
        {
            l.update(cx, |l, cx| l.restaurer(mode.as_deref(), largeur, cx));
        }
        cx.notify();
    }

    fn sur_laterale(&mut self, ev: EvenementLaterale, cx: &mut Context<Self>) {
        if let EvenementLaterale::Aller { page, y } = ev {
            let page = usize::try_from(page).unwrap_or(usize::MAX);
            self.agir(cx, |m| m.aller_a_la_page(page, y));
        }
        cx.notify();
    }

    // ── L'état éphémère ──────────────────────────────────────────────────

    /// Applique l'état reçu par la vue : zoom et page (le modèle), barre latérale, recherche,
    /// position de lecture (qui prime sur la seule page).
    fn appliquer_etat(&mut self, cx: &mut Context<Self>) {
        if self.pages.is_none() {
            return;
        }
        let Some(etat) = self.liens.a_poser.borrow_mut().take() else {
            return;
        };
        self.agir(cx, |m| m.restaurer_etat_ephemere(&etat));
        if let Some(find) = etat.get(cles::FIND)
            && let Some(r) = &self.recherche
        {
            let mut o = r.read(cx).barre().options();
            let lire = |cle: &str, v: &mut bool| {
                if let Some(b) = find.get(cle).and_then(Value::as_bool) {
                    *v = b;
                }
            };
            lire("caseSensitive", &mut o.casse);
            lire("entireWord", &mut o.mot_entier);
            lire("matchDiacritics", &mut o.diacritiques);
            lire("highlightAll", &mut o.tout_surligner);
            r.update(cx, |r, cx| r.definir_options(o, cx));
        }
        if let Some(s) = etat.get(cles::SIDEBAR) {
            let mode = s.get("mode").and_then(Value::as_str).map(str::to_owned);
            let largeur = s.get("width").and_then(Value::as_f64);
            {
                let mut b = self.barre.borrow_mut();
                match mode.as_deref() {
                    Some("thumbnails") => b.definir_mode_barre(ModeBarre::Vignettes),
                    Some("outline") => b.definir_mode_barre(ModeBarre::Plan),
                    _ => {}
                }
                if let Some(o) = s.get("open").and_then(Value::as_bool) {
                    b.ouvrir_barre(o);
                }
            }
            match &self.laterale {
                Some(l) => l.update(cx, |l, cx| l.restaurer(mode.as_deref(), largeur, cx)),
                None => self.laterale_en_attente = Some((mode, largeur)),
            }
        }
        if let Some(p) = etat.get(CLE_SCROLL).and_then(PositionLecture::depuis_json) {
            let mut reprise = None;
            self.agir(cx, |m| {
                let (r, plan) = m.aller_a_position(p);
                reprise = r;
                plan
            });
            self.reprise = reprise;
        }
    }

    /// L'état éphémère du moment, déposé pour la vue.
    fn instantane(&self, cx: &App) -> Option<Map<String, Value>> {
        let pages = self.pages.as_ref()?.read(cx);
        let m = pages.modele();
        let mut etat = match m.etat_ephemere().vers_json() {
            Value::Object(o) => o,
            _ => Map::new(),
        };
        let b = self.barre.borrow();
        let largeur = self
            .laterale
            .as_ref()
            .map_or(0.0, |l| l.read(cx).modele().largeur());
        let mode = match b.mode_barre() {
            ModeBarre::Vignettes => ModeLaterale::Vignettes,
            ModeBarre::Plan => ModeLaterale::TableDesMatieres,
        };
        etat.insert(
            cles::SIDEBAR.into(),
            json!({ "open": b.barre_ouverte(), "mode": mode.vers_texte(), "width": largeur }),
        );
        if let Some(r) = &self.recherche {
            let o = r.read(cx).barre().options();
            etat.insert(
                cles::FIND.into(),
                json!({
                    "caseSensitive": o.casse,
                    "entireWord": o.mot_entier,
                    "matchDiacritics": o.diacritiques,
                    "highlightAll": o.tout_surligner,
                }),
            );
        }
        if let Some(p) = m.lire_position() {
            etat.insert(CLE_SCROLL.into(), p.vers_json());
        }
        Some(etat)
    }

    // ── Une image ────────────────────────────────────────────────────────

    fn image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.synchroniser_document(cx);
        self.appliquer_etat(cx);
        // Les raccourcis déposés par la portée de la vue.
        let touches: Vec<Touche> = self.liens.touches.borrow_mut().drain(..).collect();
        for t in touches {
            match t {
                Touche::Chercher => {
                    if let Some(r) = &self.recherche {
                        r.update(cx, |r, cx| r.ouvrir(window, cx));
                    }
                }
                Touche::Zoom(z) => self.agir(cx, |m| m.executer_raccourci(z)),
            }
        }
        // Les menus de la barre d'outils, puis la barre latérale qui la suit.
        let barre = self.barre.clone();
        self.agir(cx, |m| barre.borrow_mut().traiter(m));
        barre
            .borrow_mut()
            .definir_portee(self.focus.contains_focused(window, cx));
        if let (Some(l), Some(p)) = (&self.laterale, &self.pages) {
            let page = p.read(cx).modele().page_vue();
            l.update(cx, |l, cx| {
                l.synchroniser(&mut barre.borrow_mut(), cx);
                l.set_page_courante(page, cx);
            });
        }
        // La reprise de lecture, une passe par image tant que la page bouge.
        if let Some(mut r) = self.reprise.take() {
            let mut fini = true;
            self.agir(cx, |m| {
                let (f, plan) = r.pas(m);
                fini = f;
                plan
            });
            if !fini {
                self.reprise = Some(r);
                window.request_animation_frame();
            }
        }
        if self.liens.a_poser.borrow().is_none() {
            *self.liens.instantane.borrow_mut() = self.instantane(cx);
        }
    }
}

impl Render for MontagePdf {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.image(window, cx);
        let t = theme_ou_defaut(cx);
        let entite = cx.entity().downgrade();
        let sur_bouton: SurBouton = Rc::new(move |b, x, y, cx: &mut App| {
            let _ = entite.update(cx, |m, cx| m.cliquer(b, x, y, cx));
        });
        let barre = match &self.pages {
            Some(p) => rendre_barre(&self.barre.borrow(), p.read(cx).modele(), cx, &sur_bouton),
            None => rendre_barre(&self.barre.borrow(), &VueModele::new(), cx, &sur_bouton),
        };
        let mut rangee = div().flex().flex_row().flex_1().min_h_0().w_full();
        if let Some(l) = &self.laterale
            && self.barre.borrow().barre_ouverte()
        {
            rangee = rangee.child(l.clone());
        }
        // overlayPane : la zone des pages, conteneur des calques (T5).
        let mut zone = div()
            .id("pdf-overlay-pane")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full();
        if let Some(r) = &self.recherche {
            zone = zone.child(r.clone());
        }
        div()
            .id("vue-pdf")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(couleur(t.palette.background_primary))
            .child(div().flex_none().child(barre))
            .child(rangee.child(zone))
    }
}

/// L'élément qui crée le montage à son premier `request_layout` (la couche de rendu ne prête
/// qu'un `&App` ; GPUI prête ici la fenêtre et l'App mutables), puis le dessine.
struct ElementMontage {
    liens: LiensPdf,
}

impl ElementMontage {
    fn new(liens: LiensPdf) -> Self {
        Self { liens }
    }
}

impl IntoElement for ElementMontage {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for ElementMontage {
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
        let existant = self.liens.montage.borrow().clone();
        let m = existant.unwrap_or_else(|| {
            let liens = self.liens.clone();
            let m = cx.new(|cx| MontagePdf::new(liens, cx));
            *self.liens.montage.borrow_mut() = Some(m.clone());
            m
        });
        let mut e = div().size_full().child(m).into_any_element();
        let id = e.request_layout(window, cx);
        (id, e)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        e: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
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
