// Port de app/e2e/pdf-toolbar.spec.ts (tests 7 à 12 : barre latérale, vignettes, table des
// matières, Reveal, toggle et 600 px, absence de table des matières) et de pdf-nettete.spec.ts
// (test 3 : vignettes pixel pour pixel)
//
//! Tests de la barre latérale PDF (tâche `fr-t4-laterale-ui`), dans une fenêtre de test GPUI
//! (sans GPU). Chaque test porte le nom du cas e2e, en snake_case et sans accents ; les cas
//! AJOUTÉS (poignée, virtualisation, netteté pdfium) suivent.
//!
//! Le banc monte, côte à côte comme la vue PDF, la barre latérale et la colonne des pages, avec la
//! barre d'outils (`BarrePdf`) pour les menus. Les rastériseurs sont des FAUX en ligne
//! (déterministes, horloge simulée) qui journalisent chaque rendu ; un test passe par pdfium, sur
//! un PDF écrit ici octet par octet. Données réécrites : 40 pages de 300 x 420 pt, une table de
//! trois parties dont une à deux chapitres.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use fragment_core::Keymap;
use fragment_pdf::document::{Raster, TaillePage};
use fragment_pdf::laterale::{LARGEUR_DEFAUT, LARGEUR_MAX, LARGEUR_MIN, ModeBarre};
use fragment_pdf::pdf_outline::EntreeToc;
use fragment_pdf::vue_modele::Pixels;
use fragment_ui::echelle::Echelle;
use fragment_ui::menu;
use fragment_ui::pdf_barre::{BarrePdf, Bouton};
use fragment_ui::pdf_laterale::{EvenementLaterale, LateralePdf};
use fragment_ui::pdf_pages::{FilDeRendu, PagesPdf, Rasteriseur};
use gpui_kit::{
    Bounds, Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement as _,
    Pixels as PixelsGpui, Render, Styled as _, TestAppContext, VisualTestContext, Window, div,
    point, px, size,
};

const N: usize = 40;

// ─── Un rastériseur factice ───

type Journal = Arc<Mutex<Vec<(usize, Pixels)>>>;

struct Faux {
    tailles: Vec<TaillePage>,
    journal: Journal,
}

impl Rasteriseur for Faux {
    fn tailles(&mut self) -> Result<Vec<TaillePage>, String> {
        Ok(self.tailles.clone())
    }

    fn rendre(&mut self, page: usize, pixels: Pixels) -> Result<Raster, String> {
        self.journal.lock().unwrap().push((page, pixels));
        Ok(Raster {
            largeur: pixels.w,
            hauteur: pixels.h,
            octets: vec![255; (pixels.w * pixels.h * 4) as usize],
        })
    }
}

fn faux() -> (FilDeRendu, Journal) {
    let journal = Journal::default();
    let r = Faux {
        tailles: vec![
            TaillePage {
                largeur: 300.0,
                hauteur: 420.0
            };
            N
        ],
        journal: Arc::clone(&journal),
    };
    (FilDeRendu::en_ligne(Box::new(r)), journal)
}

/// Une table de trois parties ; la deuxième a deux chapitres (pages 8 et 12).
fn table() -> Vec<EntreeToc> {
    let feuille = |titre: &str, page: u32| EntreeToc {
        titre: titre.into(),
        page: Some(page),
        y: None,
        enfants: Vec::new(),
    };
    vec![
        feuille("Préface", 1),
        EntreeToc {
            enfants: vec![feuille("Chapitre A1", 8), feuille("Chapitre A2", 12)],
            ..feuille("Partie A", 5)
        },
        feuille("Partie B", 20),
    ]
}

// ─── Le banc : la barre latérale et les pages côte à côte, la barre d'outils à part ───

struct Racine {
    pages: Entity<PagesPdf>,
    laterale: Entity<LateralePdf>,
    barre: BarrePdf,
    /// Les largeurs annoncées au relâchement de la poignée.
    largeurs: Vec<f64>,
}

impl Racine {
    fn new(
        pages: FilDeRendu,
        laterale: FilDeRendu,
        toc: Vec<EntreeToc>,
        cx: &mut Context<Self>,
    ) -> Self {
        let pages = cx.new(|cx| PagesPdf::new(pages, cx));
        let lat = cx.new(|cx| LateralePdf::new(laterale, toc, cx));
        // Ce que fera la vue montée : la barre demande, les pages obéissent.
        cx.subscribe(&lat, |this, _, ev: &EvenementLaterale, cx| match *ev {
            EvenementLaterale::Aller { page, y } => {
                this.pages.update(cx, |p, cx| {
                    p.agir(cx, |m| m.aller_a_la_page(page as usize, y));
                });
                this.relayer_page(cx);
            }
            EvenementLaterale::LargeurChangee(l) => this.largeurs.push(l),
        })
        .detach();
        Self {
            pages,
            laterale: lat,
            barre: BarrePdf::new(),
            largeurs: Vec::new(),
        }
    }

    /// La page de la barre d'outils passe à la barre latérale.
    fn relayer_page(&mut self, cx: &mut Context<Self>) {
        let page = self.pages.read(cx).modele().page_vue().max(1);
        self.laterale
            .update(cx, |l, cx| l.set_page_courante(page, cx));
    }

    /// L'état de la barre d'outils passe à la barre latérale.
    fn synchroniser(&mut self, cx: &mut Context<Self>) {
        let laterale = self.laterale.clone();
        laterale.update(cx, |l, cx| l.synchroniser(&mut self.barre, cx));
    }
}

impl Render for Racine {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().flex().size_full().child(self.laterale.clone()).child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .h_full()
                .debug_selector(|| "zone-pages".to_owned())
                .child(self.pages.clone()),
        )
    }
}

struct Banc {
    racine: Entity<Racine>,
    pages: Entity<PagesPdf>,
    laterale: Entity<LateralePdf>,
    journal_pages: Journal,
    journal_laterale: Journal,
}

fn monter(
    cx: &mut TestAppContext,
    largeur: f32,
    toc: Vec<EntreeToc>,
) -> (Banc, &mut VisualTestContext) {
    let (fil_pages, journal_pages) = faux();
    let (fil_laterale, journal_laterale) = faux();
    let mut fils = Some((fil_pages, fil_laterale));
    let (racine, vcx) = cx.add_window_view(move |_, cx| {
        let (p, l) = fils.take().unwrap();
        Racine::new(p, l, toc, cx)
    });
    vcx.simulate_resize(size(px(largeur), px(800.0)));
    let (pages, laterale) = racine.read_with(vcx, |r, _| (r.pages.clone(), r.laterale.clone()));
    let banc = Banc {
        racine,
        pages,
        laterale,
        journal_pages,
        journal_laterale,
    };
    stabiliser(vcx);
    (banc, vcx)
}

/// Une image : dessin, puis ce qu'il a déclenché (mesures, demandes).
fn image(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vcx.run_until_parked();
}

/// Le temps passe de `ms`, une image est dessinée.
fn attendre(vcx: &mut VisualTestContext, ms: u64) {
    vcx.executor().advance_clock(Duration::from_millis(ms));
    vcx.run_until_parked();
    image(vcx);
}

/// Laisse tout se poser : rendus récoltés, fins de geste et recalages passés.
fn stabiliser(vcx: &mut VisualTestContext) {
    for _ in 0..60 {
        attendre(vcx, 5);
    }
}

// ─── Gestes ───

/// Ouvre un bouton de menu de la barre d'outils, clique l'entrée `titre`, rejoue la file : le
/// geste d'un utilisateur. La barre latérale suit.
fn choisir(b: &Banc, vcx: &mut VisualTestContext, bouton: Bouton, titre: &str) {
    b.racine.update(vcx, |r, cx| {
        let pages = r.pages.clone();
        pages.update(cx, |p, cx| {
            p.agir(cx, |m| {
                r.barre.appui_sur(bouton);
                let (menu_ouvert, mut plan) = r.barre.cliquer(bouton, m);
                let menu_ouvert = menu_ouvert.expect("le menu s'ouvre");
                let idx = menu_ouvert
                    .forme()
                    .iter()
                    .position(|t| t == titre)
                    .unwrap_or_else(|| panic!("pas d'entrée « {titre} »"));
                menu_ouvert.show_at_position(10.0, 10.0, &Keymap::new());
                assert!(menu::cliquer(0, idx));
                plan.fusionner(r.barre.traiter(m));
                plan
            });
        });
        r.synchroniser(cx);
        r.relayer_page(cx);
    });
    stabiliser(vcx);
}

/// Le bouton « Toggle sidebar ».
fn basculer(b: &Banc, vcx: &mut VisualTestContext) {
    b.racine.update(vcx, |r, cx| {
        r.barre.basculer_barre();
        r.synchroniser(cx);
    });
    stabiliser(vcx);
}

/// Saisit un numéro dans le champ de page et valide (comme la barre d'outils).
fn aller_champ(b: &Banc, vcx: &mut VisualTestContext, numero: &str) {
    b.pages.update(vcx, |p, cx| {
        p.agir(cx, |m| {
            m.champ_focaliser();
            m.champ_saisir(numero);
            m.champ_valider()
        });
    });
    b.racine.update(vcx, |r, cx| r.relayer_page(cx));
    stabiliser(vcx);
}

fn bornes(vcx: &mut VisualTestContext, sonde: &str) -> Option<Bounds<PixelsGpui>> {
    vcx.debug_bounds(sonde)
}

fn centre(vcx: &mut VisualTestContext, sonde: &str) -> gpui_kit::Point<PixelsGpui> {
    let b = bornes(vcx, sonde).unwrap_or_else(|| panic!("sonde {sonde} introuvable"));
    point(
        b.origin.x + b.size.width / 2.0,
        b.origin.y + b.size.height / 2.0,
    )
}

fn cliquer(vcx: &mut VisualTestContext, sonde: &str) {
    let p = centre(vcx, sonde);
    vcx.simulate_click(p, Modifiers::default());
    stabiliser(vcx);
}

fn largeur_page(b: &Banc, vcx: &mut VisualTestContext) -> f64 {
    b.pages
        .read_with(vcx, |p, _| p.modele().geometrie().largeur_px(0))
}

fn page_vue(b: &Banc, vcx: &mut VisualTestContext) -> usize {
    b.pages.read_with(vcx, |p, _| p.modele().page_vue())
}

fn droite(vcx: &mut VisualTestContext, sonde: &str) -> f32 {
    let b = bornes(vcx, sonde).unwrap_or_else(|| panic!("sonde {sonde} introuvable"));
    f32::from(b.right())
}

/// Toutes les vignettes peintes ont EXACTEMENT la taille physique de leur boîte. Rend leur nombre.
fn verifier_net(b: &Banc, vcx: &mut VisualTestContext, quoi: &str) -> usize {
    b.laterale.read_with(vcx, |l, _| {
        let occupees = l.vignettes_occupees();
        assert!(!occupees.is_empty(), "{quoi} : aucune vignette");
        for page in &occupees {
            let e = l.etat_vignette(*page).unwrap();
            assert!(!e.en_cours, "{quoi} : vignette {page} encore en rendu");
            assert!(e.mesure.is_some(), "{quoi} : vignette {page} non mesurée");
            assert_eq!(
                e.image, e.mesure,
                "{quoi} : vignette {page} étirée (image ≠ boîte physique)"
            );
        }
        occupees.len()
    })
}

// ─── Tests 7 à 12 de pdf-toolbar.spec.ts ───

#[test]
fn page_width_suit_la_largeur_de_la_zone_barre_laterale_ouverte() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, table());
    choisir(&b, vcx, Bouton::OptionsZoom, "Page width");
    let fermee = largeur_page(&b, vcx);
    assert!(
        !b.racine
            .read_with(vcx, |r, _| r.barre.bouton_actif(Bouton::BasculerBarre))
    );
    assert!(bornes(vcx, "pdf-sidebar-container").is_none());

    basculer(&b, vcx);
    assert!(
        b.racine
            .read_with(vcx, |r, _| r.barre.bouton_actif(Bouton::BasculerBarre))
    );
    let barre = bornes(vcx, "pdf-sidebar-container").expect("la barre est visible");
    assert_eq!(f32::from(barre.size.width), LARGEUR_DEFAUT as f32);
    assert!(largeur_page(&b, vcx) < fermee - 50.0);
}

#[test]
fn vignettes_peintes_cliquables_la_page_courante_est_active() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, table());
    choisir(&b, vcx, Bouton::OptionsBarre, "Thumbnails");
    assert_eq!(
        b.laterale
            .read_with(vcx, |l, _| l.modele().nombre_vignettes()),
        N
    );
    assert_eq!(verifier_net(&b, vcx, "ouverture") > 0, true);
    // La première vignette a un canvas peint.
    assert!(b.laterale.read_with(vcx, |l, _| {
        l.etat_vignette(1).and_then(|e| e.image).is_some()
    }));

    cliquer(vcx, "vignette-3");
    assert_eq!(page_vue(&b, vcx), 3);
    assert_eq!(
        b.laterale.read_with(vcx, |l, _| l.modele().page_active()),
        3
    );

    // Aller loin : la vignette active défile en vue.
    aller_champ(&b, vcx, "35");
    assert_eq!(page_vue(&b, vcx), 35);
    assert_eq!(
        b.laterale.read_with(vcx, |l, _| l.modele().page_active()),
        35
    );
    let barre = bornes(vcx, "pdf-sidebar-container").unwrap();
    let v = bornes(vcx, "vignette-35").expect("la vignette 35 est peinte");
    assert!(
        v.top() >= barre.top() && v.bottom() <= barre.bottom(),
        "{v:?} hors de {barre:?}"
    );
    verifier_net(&b, vcx, "après défilement");
}

#[test]
fn table_des_matieres_arbre_repli_clic_page_a_droite() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, table());
    choisir(&b, vcx, Bouton::OptionsBarre, "Table of contents");
    let lignes = b.laterale.read_with(vcx, |l, _| l.modele().toc().lignes());
    let titres: Vec<&str> = lignes.iter().map(|l| l.titre.as_str()).collect();
    assert_eq!(titres, ["Préface", "Partie A", "Partie B"]);
    assert_eq!(lignes[0].page, Some(1));
    assert!(
        bornes(vcx, "toc-0/flair").is_some(),
        "la page est affichée à droite"
    );
    assert!(lignes[1].replie && lignes[1].repliable);

    // Les enfants sont repliés ; le chevron les déplie.
    assert!(bornes(vcx, "toc-1-1/titre").is_none());
    cliquer(vcx, "toc-1/chevron");
    assert!(bornes(vcx, "toc-1-1/titre").is_some());
    assert_eq!(page_vue(&b, vcx), 1, "le chevron ne navigue pas");

    cliquer(vcx, "toc-1-1/titre");
    assert_eq!(page_vue(&b, vcx), 12);
    let active = b
        .laterale
        .read_with(vcx, |l, _| l.modele().toc().active().cloned());
    assert_eq!(active, Some(vec![1, 1]));
    assert!(bornes(vcx, "toc-1-1/flair").is_some());
}

#[test]
fn reveal_page_in_table_of_contents_deplie_et_surligne_l_entree_de_la_page() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, table());
    // Déplier puis replier la partie A, puis aller page 9 (dans le chapitre A1).
    choisir(&b, vcx, Bouton::OptionsBarre, "Table of contents");
    cliquer(vcx, "toc-1/chevron");
    cliquer(vcx, "toc-1/chevron");
    assert!(bornes(vcx, "toc-1-0/titre").is_none());
    choisir(&b, vcx, Bouton::OptionsBarre, "Thumbnails");
    aller_champ(&b, vcx, "9");
    assert_eq!(page_vue(&b, vcx), 9);

    choisir(
        &b,
        vcx,
        Bouton::OptionsBarre,
        "Reveal page in table of contents",
    );
    assert_eq!(
        b.laterale.read_with(vcx, |l, _| l.modele().mode()),
        ModeBarre::TableDesMatieres
    );
    let active = b
        .laterale
        .read_with(vcx, |l, _| l.modele().toc().active().cloned());
    assert_eq!(active, Some(vec![1, 0]));
    let ligne = b
        .laterale
        .read_with(vcx, |l, _| l.modele().toc().lignes())
        .into_iter()
        .find(|l| l.chemin == [1, 0])
        .expect("l'entrée est visible");
    assert!(ligne.active && !ligne.replie);
    let barre = bornes(vcx, "pdf-sidebar-container").unwrap();
    let titre = bornes(vcx, "toc-1-0/titre").expect("l'entrée est peinte");
    assert!(titre.top() >= barre.top() && titre.bottom() <= barre.bottom());
}

#[test]
fn le_toggle_referme_la_barre_pas_de_debordement_a_600_px() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, table());
    basculer(&b, vcx);
    assert!(bornes(vcx, "pdf-sidebar-container").is_some());
    basculer(&b, vcx);
    assert!(bornes(vcx, "pdf-sidebar-container").is_none());
    basculer(&b, vcx);

    // La barre la plus large possible, puis une fenêtre de 600 px : elle plafonne à la moitié.
    tirer_poignee(vcx, 1000.0);
    assert_eq!(
        b.laterale.read_with(vcx, |l, _| l.modele().largeur()),
        LARGEUR_MAX
    );
    vcx.simulate_resize(size(px(600.0), px(500.0)));
    stabiliser(vcx);
    let barre = bornes(vcx, "pdf-sidebar-container").unwrap();
    assert!(f32::from(barre.size.width) <= 300.0 + 0.5, "{barre:?}");
    let zone = bornes(vcx, "zone-pages").unwrap();
    assert!(
        f32::from(zone.right()) <= 600.0 + 1.0,
        "les pages débordent : {zone:?}"
    );
    assert!(f32::from(barre.right()) <= f32::from(zone.left()) + 1.0);

    // Aucune déformation : l'image d'une vignette garde le format de la page (300 x 420).
    choisir(&b, vcx, Bouton::OptionsBarre, "Thumbnails");
    let v = bornes(vcx, "vignette-image-1").expect("vignette peinte");
    let rapport = f32::from(v.size.height) / f32::from(v.size.width);
    assert!((rapport - 1.4).abs() < 0.02, "rapport {rapport}");
    verifier_net(&b, vcx, "600 px");
}

#[test]
fn sans_table_des_matieres_un_message_et_les_vignettes_restent_possibles() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, Vec::new());
    choisir(&b, vcx, Bouton::OptionsBarre, "Table of contents");
    assert_eq!(
        b.laterale.read_with(vcx, |l, _| l.modele().message_vide()),
        Some("No table of contents")
    );
    assert!(bornes(vcx, "pdf-outline-empty").is_some());

    choisir(&b, vcx, Bouton::OptionsBarre, "Thumbnails");
    assert_eq!(
        b.laterale.read_with(vcx, |l, _| l.modele().message_vide()),
        None
    );
    assert!(bornes(vcx, "pdf-outline-empty").is_none());
    assert!(bornes(vcx, "vignette-1").is_some());
    verifier_net(&b, vcx, "vignettes sans table");
}

// ─── Cas AJOUTÉS ───

/// La poignée, saisie à la bordure droite de la barre, tirée jusqu'à l'abscisse `vers`.
fn tirer_poignee(vcx: &mut VisualTestContext, vers: f32) {
    let barre = bornes(vcx, "pdf-sidebar-container").unwrap();
    let (x, y) = (f32::from(barre.right()) - 1.0, 300.0);
    vcx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::default());
    vcx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::default());
    image(vcx);
    vcx.simulate_mouse_move(
        point(px(vers), px(y)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    image(vcx);
    vcx.simulate_mouse_up(
        point(px(vers), px(y)),
        MouseButton::Left,
        Modifiers::default(),
    );
    stabiliser(vcx);
}

#[test]
fn la_poignee_borne_la_largeur_et_les_vignettes_sont_repeintes_au_relachement() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, table());
    basculer(&b, vcx);
    verifier_net(&b, vcx, "avant");
    let largeur = |b: &Banc, vcx: &mut VisualTestContext| {
        b.laterale.read_with(vcx, |l, _| l.modele().largeur())
    };
    assert_eq!(largeur(&b, vcx), LARGEUR_DEFAUT);

    // Pendant le geste, rien n'est repeint ; au relâchement, tout l'est à la nouvelle taille.
    let barre = bornes(vcx, "pdf-sidebar-container").unwrap();
    let (x, y) = (f32::from(barre.right()) - 1.0, 300.0);
    let avant = b.journal_laterale.lock().unwrap().len();
    vcx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::default());
    vcx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::default());
    image(vcx);
    vcx.simulate_mouse_move(
        point(px(x + 60.0), px(y)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    stabiliser(vcx);
    assert_eq!(largeur(&b, vcx), LARGEUR_DEFAUT + 60.0);
    assert_eq!(
        b.journal_laterale.lock().unwrap().len(),
        avant,
        "repeint pendant le geste"
    );
    vcx.simulate_mouse_up(
        point(px(x + 60.0), px(y)),
        MouseButton::Left,
        Modifiers::default(),
    );
    stabiliser(vcx);
    assert!(
        b.journal_laterale.lock().unwrap().len() > avant,
        "pas de repeint au relâchement"
    );
    verifier_net(&b, vcx, "après");
    assert_eq!(
        b.racine.read_with(vcx, |r, _| r.largeurs.clone()),
        [LARGEUR_DEFAUT + 60.0]
    );

    // Les bornes : 120 et 480 px.
    tirer_poignee(vcx, 0.0);
    assert_eq!(largeur(&b, vcx), LARGEUR_MIN);
    tirer_poignee(vcx, 1000.0);
    assert_eq!(largeur(&b, vcx), LARGEUR_MAX);
}

#[test]
fn rien_n_est_peint_barre_fermee_et_les_vignettes_hors_zone_sont_liberees() {
    let mut cx = TestAppContext::single();
    let (b, vcx) = monter(&mut cx, 1000.0, table());
    assert!(
        b.journal_laterale.lock().unwrap().is_empty(),
        "peint barre fermée"
    );
    assert!(
        b.laterale
            .read_with(vcx, |l, _| l.vignettes_occupees().is_empty())
    );

    basculer(&b, vcx);
    let ouvertes = b.laterale.read_with(vcx, |l, _| l.vignettes_occupees());
    assert!(
        ouvertes.contains(&1) && ouvertes.len() < N / 2,
        "{ouvertes:?}"
    );

    aller_champ(&b, vcx, "35");
    let apres = b.laterale.read_with(vcx, |l, _| l.vignettes_occupees());
    assert!(
        apres.contains(&35) && !apres.contains(&1) && apres.len() < N / 2,
        "{apres:?}"
    );
    verifier_net(&b, vcx, "après le saut");

    // Fermée de nouveau : tout est jeté.
    basculer(&b, vcx);
    assert!(
        b.laterale
            .read_with(vcx, |l, _| l.vignettes_occupees().is_empty())
    );
}

#[test]
fn une_page_au_format_different_corrige_sa_vignette() {
    let mut cx = TestAppContext::single();
    let journal = Journal::default();
    let mut tailles = vec![
        TaillePage {
            largeur: 300.0,
            hauteur: 420.0
        };
        3
    ];
    tailles[1] = TaillePage {
        largeur: 420.0,
        hauteur: 300.0,
    };
    let fil = FilDeRendu::en_ligne(Box::new(Faux {
        tailles,
        journal: Arc::clone(&journal),
    }));
    let (fil_pages, _) = faux();
    let mut fils = Some((fil_pages, fil));
    let (racine, vcx) = cx.add_window_view(move |_, cx| {
        let (p, l) = fils.take().unwrap();
        Racine::new(p, l, Vec::new(), cx)
    });
    vcx.simulate_resize(size(px(1000.0), px(800.0)));
    let (pages, laterale) = racine.read_with(vcx, |r, _| (r.pages.clone(), r.laterale.clone()));
    let b = Banc {
        racine,
        pages,
        laterale,
        journal_pages: Journal::default(),
        journal_laterale: journal,
    };
    basculer(&b, vcx);
    let p1 = bornes(vcx, "vignette-image-1").unwrap();
    let p2 = bornes(vcx, "vignette-image-2").unwrap();
    assert!(f32::from(p1.size.height) > f32::from(p1.size.width));
    assert!(
        f32::from(p2.size.height) < f32::from(p2.size.width),
        "paysage : {p2:?}"
    );
    verifier_net(&b, vcx, "formats mixtes");
}

// ─── pdfium ───

/// Un PDF minimal écrit ici : des pages nues de tailles données, un rectangle noir chacune.
fn pdf(tailles: &[(f64, f64)]) -> Vec<u8> {
    let n = tailles.len();
    let mut objets: Vec<String> = Vec::new();
    objets.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    let enfants: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    objets.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {n} >>",
        enfants.join(" ")
    ));
    for (i, (l, h)) in tailles.iter().enumerate() {
        objets.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {l} {h}] /Contents {} 0 R >>",
            4 + 2 * i
        ));
        let flux = "0 0 0 rg 20 20 60 40 re f";
        objets.push(format!(
            "<< /Length {} >>\nstream\n{flux}\nendstream",
            flux.len()
        ));
    }
    let mut sortie = String::from("%PDF-1.4\n");
    let mut positions = Vec::new();
    for (i, o) in objets.iter().enumerate() {
        positions.push(sortie.len());
        sortie += &format!("{} 0 obj\n{o}\nendobj\n", i + 1);
    }
    let xref = sortie.len();
    sortie += &format!("xref\n0 {}\n0000000000 65535 f \n", objets.len() + 1);
    for p in positions {
        sortie += &format!("{p:010} 00000 n \n");
    }
    sortie += &format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objets.len() + 1
    );
    sortie.into_bytes()
}

#[test]
fn vignettes_de_la_barre_laterale_pixel_pour_pixel_avec_pdfium() {
    // Le binaire épinglé est installé en CI (scripts/pdfium.ps1) ; en local il peut manquer.
    if let Err(e) = fragment_pdf::document::initialiser(None) {
        assert!(
            std::env::var_os("CI").is_none(),
            "pdfium absent en CI : {e}"
        );
        eprintln!("pdfium absent, test sauté : {e}");
        return;
    }
    let octets = pdf(&vec![(300.0, 400.0); 12]);
    let (fil_pages, _) = faux();
    let mut fils = Some((fil_pages, FilDeRendu::pdfium(octets)));
    let mut cx = TestAppContext::single();
    let (racine, vcx) = cx.add_window_view(move |_, cx| {
        let (p, l) = fils.take().unwrap();
        Racine::new(p, l, Vec::new(), cx)
    });
    vcx.simulate_resize(size(px(1000.0), px(800.0)));
    vcx.simulate_scale_factor_change(1.5);
    vcx.update(|_, cx| Echelle::installer(cx, 0.85));
    let (pages, laterale) = racine.read_with(vcx, |r, _| (r.pages.clone(), r.laterale.clone()));
    let b = Banc {
        racine,
        pages,
        laterale,
        journal_pages: Journal::default(),
        journal_laterale: Journal::default(),
    };
    basculer(&b, vcx);
    // Le fil de rendu n'obéit pas à l'horloge simulée : on attend en temps RÉEL.
    for _ in 0..2000 {
        attendre(vcx, 5);
        let pret = b.laterale.read_with(vcx, |l, _| {
            l.erreur().is_none()
                && !l.rendus_en_cours()
                && !l.vignettes_occupees().is_empty()
                && l.vignettes_occupees()
                    .iter()
                    .all(|&p| l.etat_vignette(p).is_some_and(|e| e.image.is_some()))
        });
        if pret {
            break;
        }
        assert_eq!(
            b.laterale
                .read_with(vcx, |l, _| l.erreur().map(str::to_owned)),
            None
        );
        thread::sleep(Duration::from_millis(2));
    }
    stabiliser(vcx);
    let n = verifier_net(&b, vcx, "pdfium");
    assert!(n >= 3, "{n} vignettes");
}
