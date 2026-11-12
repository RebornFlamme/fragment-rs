// Port de app/src/ui/views/pdf/PdfFindBar.ts (ouvrir, fermer, relancer, suivant, portée empilée au
// focus) et de PdfView.ts (le relais `chercher` / `fermee` vers le contrôleur de recherche)
//
//! La barre de recherche montée : [`RecherchePdf`] enveloppe la couche texte d'un PDF ([`TextePdf`])
//! et lui ajoute, au-dessus, la barre de recherche (Ctrl+F) et les surlignages des résultats.
//!
//! - elle lit le texte de TOUTES les pages sur son propre fil (la couche texte ne lit que les
//!   pages peintes), construit la recherche une fois le document lu, et relance dès que le texte
//!   arrive ;
//! - la frappe est temporisée ([`DELAI_FRAPPE_MS`]) ; Entrée, Maj+Entrée, les boutons et les
//!   réglages relancent aussitôt ;
//! - un résultat amène sa page (et sa ligne) dans la zone ;
//! - les surlignages se posent par les coordonnées de la couche texte ([`SurfacePdf`]) : justes à
//!   tout zoom, comme les glyphes de la sélection.
//!
//! ★ LE CLAVIER du champ passe par une PORTÉE empilée au focus (comme une modale), pas par la
//!   portée de la vue : Entrée, Maj+Entrée et Échap ne valent que quand on tape dans le champ.
//!   Posées dans la portée de la vue, ces liaisons explicites arrêteraient la chaîne même sans
//!   rien consommer. Les liaisons n'ont pas accès au contexte GPUI : elles déposent une commande
//!   dans une file que le rendu suivant exécute (le pont clavier rafraîchit la fenêtre après toute
//!   frappe consommée).
//! ★ Les caractères tapés arrivent, eux, par l'écouteur de frappes du champ (la portée ne les
//!   reconnaît pas, la frappe poursuit son chemin jusqu'à lui).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use fragment_core::{KeyEvent, Keymap, KeymapContext, Modifier, Scope};
use fragment_pdf::recherche::{DELAI_FRAPPE_MS, PageRecherche, TypeRecherche};
use fragment_text::TextSurface as _;
use gpui_kit::{
    App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, ParentElement as _, Render, Styled as _, Subscription, Task,
    Window, div,
};

use super::modele::BarreRecherche;
use super::rendu::{Bouton, RectSurlignage, SurBouton, rendre_barre, rendre_surlignages};
use crate::pdf_texte::{FilDeTexte, SurfacePdf, TextePage, TextePdf, pos_de_page};

/// Délai entre deux récoltes du fil du texte (ms).
const RECOLTE_MS: u64 = 4;
/// L'écart gardé au-dessus d'une occurrence quand on la montre, en points de la page.
const MARGE_HAUT: f64 = 80.0;

/// Ce que la barre demande, déposé par les liaisons clavier et les boutons.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Commande {
    Ouvrir,
    Fermer,
    Suivant,
    Precedent,
    SelectionnerTout,
    Bouton(Bouton, f32, f32),
}

type File = Rc<RefCell<VecDeque<Commande>>>;

/// Les lignes d'une page : (début de la rangée dans le texte, haut de la rangée en points).
type Lignes = Vec<(usize, f64)>;

/// Une page lue : son texte cherchable et ses lignes.
type PageLue = (PageRecherche, Lignes);

/// La couche texte d'un PDF, sa barre de recherche et ses surlignages.
pub struct RecherchePdf {
    texte: Entity<TextePdf>,
    barre: BarreRecherche,
    fil: FilDeTexte,
    /// Les pages lues, dans l'ordre ; `None` tant que le texte n'a pas été demandé.
    lues: Option<Vec<Option<PageLue>>>,
    /// Les lignes de chaque page (haut de rangée), pour amener une occurrence dans la zone.
    lignes: Vec<Lignes>,
    /// Une recherche à lancer dès que le document sera lu.
    en_attente: Option<(TypeRecherche, bool)>,
    recolte: Option<Task<()>>,
    frappe: Option<Task<()>>,
    focus: FocusHandle,
    keymap: Keymap,
    portee_champ: Scope,
    portee_vue: Scope,
    portee_empilee: bool,
    commandes: File,
    _observation: Subscription,
}

impl RecherchePdf {
    /// La recherche de `texte` ; `fil` lit le même document (un fil à part : la couche texte ne
    /// lit que les pages peintes, la recherche les lit toutes). `keymap` : celui de la fenêtre.
    pub fn new(
        texte: Entity<TextePdf>,
        fil: FilDeTexte,
        keymap: Keymap,
        cx: &mut Context<Self>,
    ) -> Self {
        let commandes: File = Rc::new(RefCell::new(VecDeque::new()));
        let racine = keymap.root_scope().clone();

        // La portée du champ, empilée au focus : chaînée à la racine pour que Mod+P et les globaux
        // restent joignables pendant qu'on tape.
        let portee_champ = Scope::pour_vue(&racine);
        let deposer = |c: Commande| {
            let file = commandes.clone();
            move |_: &KeyEvent, _: &KeymapContext| {
                file.borrow_mut().push_back(c);
                Some(false)
            }
        };
        portee_champ.register(
            Some(&[Modifier::Shift]),
            Some("Enter"),
            deposer(Commande::Precedent),
        );
        portee_champ.register(Some(&[]), Some("Enter"), deposer(Commande::Suivant));
        portee_champ.register(
            Some(&[Modifier::Mod]),
            Some("f"),
            deposer(Commande::SelectionnerTout),
        );
        portee_champ.register(None, Some("Escape"), deposer(Commande::Fermer));

        // La portée de la vue : Ctrl+F ouvre la barre. Le shell l'empile quand la vue PDF est
        // active (comme le `view.scope` de Fragment).
        let portee_vue = Scope::pour_vue(&racine);
        portee_vue.register(Some(&[Modifier::Mod]), Some("f"), deposer(Commande::Ouvrir));

        let observation = cx.observe(&texte, |_, _, cx| cx.notify());
        Self {
            texte,
            barre: BarreRecherche::new(),
            fil,
            lues: None,
            lignes: Vec::new(),
            en_attente: None,
            recolte: None,
            frappe: None,
            focus: cx.focus_handle(),
            keymap,
            portee_champ,
            portee_vue,
            portee_empilee: false,
            commandes,
            _observation: observation,
        }
    }

    // ── Accès ────────────────────────────────────────────────────────────

    /// La portée de la vue (Ctrl+F) : à empiler quand la vue PDF est active.
    pub fn portee_vue(&self) -> Scope {
        self.portee_vue.clone()
    }

    /// La couche texte enveloppée.
    pub fn texte(&self) -> &Entity<TextePdf> {
        &self.texte
    }

    /// Le modèle de la barre.
    pub fn barre(&self) -> &BarreRecherche {
        &self.barre
    }

    /// Le texte de toutes les pages est-il lu, la recherche prête ?
    pub fn prete(&self) -> bool {
        self.barre.a_un_document()
    }

    /// Le focus du champ.
    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// Le document change : le texte lu ne vaut plus rien ; la barre ouverte relance dans le
    /// nouveau (`relancer`).
    pub fn remplacer(&mut self, fil: FilDeTexte, cx: &mut Context<Self>) {
        self.fil = fil;
        self.lues = None;
        self.lignes.clear();
        self.recolte = None;
        self.barre.decharger();
        if self.barre.ouverte() && self.barre.a_une_requete() {
            self.en_attente = Some((TypeRecherche::Nouvelle, false));
        }
        cx.notify();
    }

    // ── Gestes ───────────────────────────────────────────────────────────

    /// Ctrl+F : ouvre la barre (ou re-sélectionne le champ) et lui donne le focus.
    pub fn ouvrir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let relancer = self.barre.ouvrir();
        window.focus(&self.focus, cx);
        if relancer {
            self.lancer(TypeRecherche::Nouvelle, false, cx);
        }
        cx.notify();
    }

    /// Échap / ✕ : ferme la barre ; les surlignages disparaissent.
    pub fn fermer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.frappe = None;
        self.en_attente = None;
        self.barre.fermer();
        window.blur(cx);
        cx.notify();
    }

    /// ↑ / ↓ (Maj+Entrée / Entrée) : l'occurrence précédente (`-1`) ou suivante (`1`).
    pub fn suivant(&mut self, sens: i8, cx: &mut Context<Self>) {
        self.lancer(TypeRecherche::Encore, sens < 0, cx);
    }

    /// Aa : bascule la casse et relance.
    pub fn basculer_casse(&mut self, cx: &mut Context<Self>) {
        let type_ = self.barre.basculer_casse();
        self.lancer(type_, false, cx);
    }

    /// Les surlignages visibles, en unités du plan (celles de la couche texte) : un rect par
    /// rangée de chaque occurrence des pages peintes.
    pub fn rects_surlignage(&self, cx: &App) -> Vec<RectSurlignage> {
        if !self.barre.ouverte() {
            return Vec::new();
        }
        let surface: SurfacePdf = self.texte.read(cx).surface();
        let mut pages: Vec<usize> = surface.etat.borrow().textes.keys().copied().collect();
        pages.sort_unstable();
        let courante = self.barre.occurrence_courante();
        let mut rects = Vec::new();
        for page in pages {
            let base = pos_de_page(page) + 1;
            for s in self.barre.surlignages_page(page) {
                for rect in surface.coords_for_range(base + s.index, base + s.index + s.longueur) {
                    rects.push(RectSurlignage {
                        rect,
                        courant: s.selectionne && courante.is_some_and(|(p, _)| p == page),
                    });
                }
            }
        }
        rects
    }

    // ── La recherche ─────────────────────────────────────────────────────

    /// Lance ou relance la recherche, et amène le résultat dans la zone.
    fn lancer(&mut self, type_: TypeRecherche, arriere: bool, cx: &mut Context<Self>) {
        if !self.barre.ouverte() {
            return;
        }
        if !self.barre.a_un_document() {
            // Le texte n'est pas lu : la recherche partira dès qu'il le sera.
            self.en_attente = Some((type_, arriere));
            cx.notify();
            return;
        }
        let (courante, visibles) = {
            let pages = self.texte.read(cx).pages().read(cx);
            let m = pages.modele();
            (m.page_vue().max(1), m.pages_visibles())
        };
        let visible = move |n: usize| visibles.is_some_and(|(a, b)| n >= a && n <= b);
        let resultat = if type_ == TypeRecherche::Encore {
            self.barre
                .suivant(if arriere { -1 } else { 1 }, courante, &visible)
        } else {
            self.barre.chercher(type_, arriere, courante, &visible)
        };
        // Seul le surlignage change : on ne déplace rien.
        if resultat.is_some() && type_ != TypeRecherche::ChangementToutSurligner {
            self.montrer(cx);
        }
        cx.notify();
    }

    /// Amène l'occurrence courante : sa page, et sa ligne si on la connaît.
    fn montrer(&mut self, cx: &mut Context<Self>) {
        let Some((page, index)) = self.barre.occurrence_courante() else {
            return;
        };
        let haut = self.lignes.get(page - 1).and_then(|l| {
            let i = l.partition_point(|(debut, _)| *debut <= index);
            i.checked_sub(1).map(|i| l[i].1)
        });
        let pages = self.texte.read(cx).pages().clone();
        pages.update(cx, |p, cx| {
            p.agir(cx, |m| {
                // Une destination en coordonnées PDF (origine en bas) : la ligne, un peu sous le
                // haut de la zone.
                let y = haut.map(|h| {
                    let hauteur =
                        m.geometrie().hauteur_px(page - 1) / m.echelle().max(f64::EPSILON);
                    hauteur - (h - MARGE_HAUT).max(0.0)
                });
                m.aller_a_la_page(page, y)
            });
        });
    }

    /// La frappe change le champ : temporisée, sauf un champ vide (le compteur repart aussitôt).
    fn frappe_changee(&mut self, cx: &mut Context<Self>) {
        self.frappe = None;
        if !self.barre.a_une_requete() {
            self.lancer(TypeRecherche::Nouvelle, false, cx);
            return;
        }
        self.frappe = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(DELAI_FRAPPE_MS))
                .await;
            let _ = this.update(cx, |v, cx| {
                v.frappe = None;
                v.lancer(TypeRecherche::Nouvelle, false, cx);
            });
        }));
        cx.notify();
    }

    // ── Le texte du document ─────────────────────────────────────────────

    /// Demande le texte de toutes les pages (une fois par document).
    fn demarrer_lecture(&mut self, cx: &mut Context<Self>) {
        if self.lues.is_some() {
            return;
        }
        let n = self.texte.read(cx).pages().read(cx).modele().nombre_pages();
        if n == 0 {
            return;
        }
        self.lues = Some((0..n).map(|_| None).collect());
        for page in 1..=n {
            self.fil.demander(page);
        }
        if self.recolte.is_none() {
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
        for (page, lu) in self.fil.recolter() {
            // Une page sans texte lisible (un scan) est un état NORMAL : elle ne contient rien.
            let lue = match lu {
                Ok(t) => (PageRecherche::depuis_texte_brut(&t.texte), lignes_de(&t)),
                Err(_) => (PageRecherche::vide(), Vec::new()),
            };
            if let Some(case) = self
                .lues
                .as_mut()
                .and_then(|l| l.get_mut(page.saturating_sub(1)))
            {
                *case = Some(lue);
            }
        }
        let toutes = self
            .lues
            .as_ref()
            .is_some_and(|l| !l.is_empty() && l.iter().all(Option::is_some));
        if toutes && let Some(lues) = self.lues.take() {
            let (pages, lignes): (Vec<_>, Vec<_>) = lues.into_iter().flatten().unzip();
            self.lignes = lignes;
            self.barre.charger(pages);
            // `lues` reste `Some(vide)` : le document est lu, on n'y revient pas.
            self.lues = Some(Vec::new());
            if self.barre.ouverte() && self.barre.a_une_requete() {
                let (type_, arriere) = self
                    .en_attente
                    .take()
                    .unwrap_or((TypeRecherche::Nouvelle, false));
                self.lancer(type_, arriere, cx);
            }
            cx.notify();
        }
        let encore = self.fil.occupe();
        if !encore {
            self.recolte = None;
        }
        encore
    }

    // ── Commandes, focus, portée ─────────────────────────────────────────

    /// Exécute ce que les liaisons clavier et les boutons ont déposé.
    fn traiter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let commandes: Vec<Commande> = self.commandes.borrow_mut().drain(..).collect();
        for c in commandes {
            match c {
                Commande::Ouvrir => self.ouvrir(window, cx),
                Commande::Fermer | Commande::Bouton(Bouton::Fermer, ..) => self.fermer(window, cx),
                Commande::Suivant | Commande::Bouton(Bouton::Suivant, ..) => self.suivant(1, cx),
                Commande::Precedent | Commande::Bouton(Bouton::Precedent, ..) => {
                    self.suivant(-1, cx);
                }
                Commande::SelectionnerTout => {
                    self.barre.selectionner_tout();
                    cx.notify();
                }
                Commande::Bouton(Bouton::Casse, ..) => {
                    self.basculer_casse(cx);
                    window.focus(&self.focus, cx);
                }
                Commande::Bouton(Bouton::Options, x, y) => {
                    self.barre
                        .menu_options()
                        .show_at_position(x, y, &self.keymap);
                }
            }
        }
        // Les choix du menu des réglages : rejoués sur le modèle, chacun relance.
        for type_ in self.barre.traiter() {
            self.lancer(type_, false, cx);
            window.focus(&self.focus, cx);
        }
    }

    /// La portée du champ est empilée tant que le champ a le focus (et seulement alors).
    fn synchroniser_portee(&mut self, window: &Window) {
        let veut = self.barre.ouverte() && self.focus.is_focused(window);
        if veut && !self.portee_empilee {
            self.keymap.push_scope(&self.portee_champ);
        } else if !veut && self.portee_empilee {
            self.keymap.pop_scope(&self.portee_champ);
        }
        self.portee_empilee = veut;
    }

    /// Une frappe dans le champ : du texte ou un retour arrière (le reste est dans la portée).
    fn sur_touche(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if !self.barre.ouverte() {
            return;
        }
        let k = &ev.keystroke;
        let change = if k.key == "backspace" {
            self.barre.effacer()
        } else {
            let raccourci = k.modifiers.control || k.modifiers.alt || k.modifiers.platform;
            match &k.key_char {
                Some(c) if !raccourci => self.barre.saisir(c),
                _ => return,
            }
        };
        cx.stop_propagation();
        if change {
            self.frappe_changee(cx);
        }
        cx.notify();
    }
}

impl Drop for RecherchePdf {
    fn drop(&mut self) {
        if self.portee_empilee {
            self.keymap.pop_scope(&self.portee_champ);
        }
    }
}

/// Les rangées d'une page : une entrée à chaque changement de ligne.
fn lignes_de(t: &TextePage) -> Lignes {
    let mut lignes: Lignes = Vec::new();
    let mut courante: Option<(f64, f64)> = None;
    for g in &t.glyphes {
        if !g.a_une_boite() {
            continue;
        }
        let (haut, bas) = (g.boite.top, g.boite.bottom);
        let meme = courante.is_some_and(|(h, b)| (haut - h).abs() < (b - h) / 2.0);
        if !meme {
            lignes.push((g.debut, haut));
            courante = Some((haut, bas));
        }
    }
    lignes
}

impl Render for RecherchePdf {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.traiter(window, cx);
        self.synchroniser_portee(window);
        if self.barre.ouverte() {
            self.demarrer_lecture(cx);
        }

        let (echelle, defilement) = {
            let surface = self.texte.read(cx).surface();
            let e = surface.etat.borrow();
            (e.echelle, e.defilement)
        };
        let rects = self.rects_surlignage(cx);
        let surlignages = rendre_surlignages(&rects, echelle, defilement, cx);

        let zone = div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(self.texte.clone())
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .overflow_hidden()
                    .children(surlignages),
            );

        let mut colonne = div().id("pdf-recherche").flex().flex_col().size_full();
        if self.barre.ouverte() {
            let file = self.commandes.clone();
            let sur_bouton: SurBouton = Rc::new(move |b, x, y, _| {
                file.borrow_mut().push_back(Commande::Bouton(b, x, y));
            });
            let focus = self.focus.is_focused(window);
            colonne = colonne.child(
                div()
                    .id("pdf-recherche-champ")
                    .flex_none()
                    .track_focus(&self.focus)
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                        this.sur_touche(ev, cx);
                    }))
                    // Cliquer dans la barre rend le focus au champ.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus, cx);
                        }),
                    )
                    .child(rendre_barre(&self.barre, focus, cx, &sur_bouton)),
            );
        }
        colonne.child(zone)
    }
}
