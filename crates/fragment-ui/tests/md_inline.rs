//! Tests headless des widgets en ligne (`fr-t3-widgets-inline`) : la largeur dictée d'une plage
//! remplacée par un widget (patch `set_widget_ranges` du fork, ADR-006), la case cliquable, la puce,
//! les barres d'indentation et les maths en ligne.
//!
//! Même banc que `editeur.rs` : plateforme de test de GPUI avec le système de texte RÉEL
//! (DirectWrite) et DM Sans ; le contexte de test n'est JAMAIS détruit (sa destruction ne rend pas
//! la main sous Windows) et un chien de garde borne chaque test à 5 minutes.
//!
//! Windows seulement.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_markdown::decorations::{GenreWidget, WidgetEnLigne};
use fragment_maths::moteur::{Cle, Moteur};
use fragment_theme::Mode;
use fragment_ui::editeur::{EditeurLive, OptionsEditeur};
use fragment_ui::md_inline::{MdInline, installer};
use fragment_ui::{polices, theme_global};
use gpui_kit::base::input::EditorState;
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Point, TestAppContext, TestDispatcher,
    Window, WindowBounds, WindowOptions, px, size,
};

const LARGEUR: f32 = 1000.;
const HAUTEUR: f32 = 600.;
/// ★ L'identifiant d'élément que l'éditeur du fork enregistre (`EDITOR_ELEMENT_ID` de son `state.rs`),
/// pas `("input", id d'entité)` : un identifiant absent fait paniquer `find` / `click_at`.
const ID_EDITEUR: &str = "input-state";

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur le système de texte réel
    /// (DirectWrite) ne rend pas la main sous Windows. Le test se figeait APRÈS sa dernière assertion
    /// (ou pendant le déroulement d'une panique), et `test-windows` atteignait les 6 h de la CI.
    /// La fuite est bornée à un contexte par test, dans un processus de test.
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    editeur: Entity<EditeurLive>,
    etat: Entity<EditorState>,
    moteur: Rc<Moteur>,
    inline: Rc<MdInline>,
    /// Levé à la fin du test (succès ou panique) : arrête le chien de garde.
    fini: Arc<AtomicBool>,
}

/// Délai d'un test, au-delà duquel le chien de garde arrête le processus (`dix_mille_lignes` prend
/// ~50 s en debug sur le portable).
const DELAI_TEST: Duration = Duration::from_secs(300);

/// ★ Borne chaque test : un test qui se fige (boucle d'images, destruction du contexte) faisait
/// tourner `test-windows` jusqu'aux 6 h de la CI sans un mot. Ici il échoue en 5 min, nommé.
fn chien_de_garde(fini: Arc<AtomicBool>) {
    let nom = std::thread::current().name().unwrap_or("?").to_owned();
    std::thread::spawn(move || {
        let debut = Instant::now();
        while debut.elapsed() < DELAI_TEST {
            if fini.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        eprintln!("SURFACE|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

impl Drop for Banc {
    fn drop(&mut self) {
        self.fini.store(true, Ordering::Relaxed);
    }
}

impl Banc {
    fn new(texte: &str, options: OptionsEditeur) -> Banc {
        Banc::avec_taille(texte, options, LARGEUR, HAUTEUR)
    }

    /// Sans les widgets de `md_inline` : les largeurs se posent à la main (tests du fork).
    fn nu(texte: &str, options: OptionsEditeur) -> Banc {
        Banc::construire(texte, options, LARGEUR, HAUTEUR, false)
    }

    fn avec_taille(texte: &str, options: OptionsEditeur, l: f32, h: f32) -> Banc {
        Banc::construire(texte, options, l, h, true)
    }

    fn construire(texte: &str, options: OptionsEditeur, l: f32, h: f32, widgets: bool) -> Banc {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let texte = texte.to_owned();
        let (fenetre, editeur) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(l), px(h)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| cx.new(|cx| EditeurLive::new(&texte, options, window, cx)),
            )
            .unwrap()
        });
        // ★ Une fenêtre de test n'est PAS active : GPUI n'y distribue aucun événement de focus
        //   (chemin de focus vide), l'éditeur ne recevait jamais `InputEvent::Focus` et rien ne se
        //   révélait. On l'active comme le ferait le système.
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let etat = cx.update(|cx| editeur.read(cx).etat().clone());
        let fini = Arc::new(AtomicBool::new(false));
        chien_de_garde(fini.clone());
        let moteur = Rc::new(Moteur::new(64));
        let inline = cx.update(|cx| {
            if widgets {
                installer(&editeur, moteur.clone(), None, cx)
            } else {
                // Sans installation : un état inerte (aucun calque, aucune largeur).
                Rc::new(MdInline::inerte(moteur.clone()))
            }
        });
        let mut b = Banc {
            moteur,
            inline,
            fini,
            cx: ManuallyDrop::new(cx),
            fenetre,
            editeur,
            etat,
        };
        b.stabiliser();
        b
    }

    /// Plusieurs images (la colonne se centre sur la largeur connue après la première) et les
    /// effets en attente (observations, événements).
    fn stabiliser(&mut self) {
        for _ in 0..3 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    /// Une action dans la fenêtre, puis stabilisation.
    fn agir<R>(&mut self, f: impl FnOnce(&mut Window, &mut App, &Entity<EditorState>) -> R) -> R {
        let etat = self.etat.clone();
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| f(window, cx, &etat))
            .unwrap();
        self.stabiliser();
        r
    }

    fn lire<R>(&mut self, f: impl FnOnce(&EditeurLive, &EditorState, &App) -> R) -> R {
        let (editeur, etat) = (self.editeur.clone(), self.etat.clone());
        self.cx.update(|cx| f(editeur.read(cx), etat.read(cx), cx))
    }

    fn bornes(&mut self, o: usize) -> Bounds<gpui_kit::Pixels> {
        self.lire(|_, e, _| e.range_to_bounds(&(o..o + 1)).unwrap())
    }

    /// Un clic à `p` (coordonnées FENÊTRE).
    fn cliquer(&mut self, p: Point<gpui_kit::Pixels>) {
        self.agir(|window, cx, _| {
            let origine = window.find(ID_EDITEUR).bounds().origin;
            window.click_at(ID_EDITEUR, p - origine, cx);
        });
    }

    fn placer(&mut self, o: usize) {
        self.agir(|window, cx, etat| {
            etat.update(cx, |s, cx| {
                s.focus(window, cx);
                s.set_selected_range(o..o, cx);
            });
        });
    }

    fn touche(&mut self, k: &str) {
        self.agir(|window, cx, _| window.press(k, cx));
    }

    fn curseur(&mut self) -> usize {
        self.lire(|_, e, _| e.cursor())
    }
}

fn pres(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.
}

// ─── Largeur dictée ───

impl Banc {
    /// Pose les largeurs des widgets en ligne (octets, px), puis stabilise.
    fn largeurs(&mut self, l: Vec<(std::ops::Range<usize>, f32)>) {
        let e = self.editeur.clone();
        self.cx
            .update(|cx| e.update(cx, |ed, cx| ed.poser_largeurs_widgets(l, cx)));
        self.stabiliser();
    }
}

/// Une plage remplacée par un widget prend SA largeur : le texte suivant se décale d'autant, la
/// source n'est pas modifiée, et le curseur la franchit d'un pas.
#[test]
fn la_largeur_du_widget_egale_sa_plage() {
    let texte = "ab $x$ cd";
    let mut b = Banc::nu(texte, OptionsEditeur::default());
    let sans = b.bornes(6).origin.x;
    b.largeurs(vec![(3..6, 40.0)]);
    let debut = b.bornes(2).right();
    let apres = b.bornes(6).origin.x;
    assert!(
        pres(f32::from(apres - debut), 40.0),
        "le texte suivant démarre à 40 px du début du widget : {:?}",
        apres - debut
    );
    assert!(apres != sans, "le décalage est bien posé");
    assert_eq!(b.lire(|_, e, _| e.value().to_string()), texte);

    b.placer(2);
    b.touche("right");
    assert_eq!(b.curseur(), 3);
    b.touche("right");
    assert_eq!(b.curseur(), 6, "un pas franchit le widget");
    b.touche("left");
    assert_eq!(b.curseur(), 3, "et de même à rebours");

    // Retirer les largeurs rend l'éditeur de série.
    b.largeurs(Vec::new());
    assert!(pres(f32::from(b.bornes(6).origin.x), f32::from(sans)));
}

/// Un widget qui chevauche un masque est ignoré : le masque garde sa largeur nulle.
#[test]
fn un_widget_sur_un_masque_est_ignore() {
    let texte = "# Titre
suite";
    let mut b = Banc::nu(texte, OptionsEditeur::default());
    let x = b.bornes(2).origin.x;
    b.largeurs(vec![(0..2, 30.0)]);
    assert_eq!(b.bornes(2).origin.x, x);
}

// ─── Les widgets du live preview ───

impl Banc {
    fn widgets(&mut self) -> Vec<WidgetEnLigne> {
        self.lire(|e, _, _| e.projection().decorations.widgets.clone())
    }

    /// La boîte du widget qui commence à l'octet `debut`.
    fn boite_du_widget(&mut self, debut: usize) -> Bounds<gpui_kit::Pixels> {
        let w = self
            .widgets()
            .into_iter()
            .find(|w| w.source.octets.from == debut)
            .unwrap_or_else(|| panic!("pas de widget en {debut}"));
        let r = w.source.octets.from..w.source.octets.to;
        self.lire(|_, e, _| e.range_to_bounds(&r).unwrap())
    }

    fn texte(&mut self) -> String {
        self.lire(|_, e, _| e.value().to_string())
    }

    /// Un clic au centre d'une boîte.
    fn cliquer_au_centre(&mut self, b: Bounds<gpui_kit::Pixels>) {
        self.cliquer(b.center());
    }

    /// Rend la formule AVANT l'éditeur : le cache du moteur est chaud, les largeurs se posent à la
    /// notification suivante (le moteur travaille sur son fil, hors de l'horloge de test).
    fn rendre_formule(&mut self, tex: &str, affichage: bool) -> f32 {
        let c = Mode::Sombre;
        let _ = c;
        let couleur = self.lire(|_, _, cx| {
            let t = theme_global::theme_ou_defaut(cx);
            [
                t.palette.text_normal.r,
                t.palette.text_normal.g,
                t.palette.text_normal.b,
            ]
        });
        let cle = Cle::new(tex, affichage, 16.0, couleur);
        let r = self.moteur.rendre(&cle).unwrap();
        let e = self.editeur.clone();
        self.cx.update(|cx| e.update(cx, |_, cx| cx.notify()));
        self.stabiliser();
        r.largeur as f32
    }
}

/// `- [ ] tâche` : cocher puis décocher au clic, la source change d'UN caractère, le curseur ne
/// bouge pas, et chaque clic est une transaction annulable.
#[test]
fn cocher_decocher_au_clic_sans_deplacer_le_texte_undo() {
    let mut b = Banc::new(
        "- [ ] tâche
fin",
        OptionsEditeur::default(),
    );
    let fin = b.texte().len();
    b.placer(fin);
    let case = b.boite_du_widget(2);
    b.cliquer_au_centre(case);
    assert_eq!(
        b.texte(),
        "- [x] tâche
fin"
    );
    assert_eq!(b.curseur(), fin, "le curseur ne bouge pas");
    let case = b.boite_du_widget(2);
    b.cliquer_au_centre(case);
    assert_eq!(
        b.texte(),
        "- [ ] tâche
fin"
    );
    // Annuler : une seule transaction par clic.
    b.touche("ctrl-z");
    assert_eq!(
        b.texte(),
        "- [x] tâche
fin"
    );
}

/// Une case en lecture seule se voit mais ne bascule pas.
#[test]
fn la_case_ne_bascule_pas_en_lecture_seule() {
    let options = OptionsEditeur {
        lecture_seule: true,
        ..OptionsEditeur::default()
    };
    let mut b = Banc::new(
        "- [ ] tâche
fin",
        options,
    );
    let case = b.boite_du_widget(2);
    b.cliquer_au_centre(case);
    assert_eq!(
        b.texte(),
        "- [ ] tâche
fin"
    );
}

/// `- [ ]` en fin de ligne (sans texte) se rend en case, pas en `[ ]` brut.
#[test]
fn tache_en_fin_de_ligne_se_rend_en_case() {
    let mut b = Banc::new(
        "- [ ]
- [x]

autre",
        OptionsEditeur::default(),
    );
    b.placer(15);
    let cases: Vec<_> = b
        .widgets()
        .into_iter()
        .filter(|w| matches!(w.genre, GenreWidget::Case { .. }))
        .collect();
    assert_eq!(cases.len(), 2);
    assert!(matches!(
        cases[1].genre,
        GenreWidget::Case { cochee: true, .. }
    ));
    // Chaque case occupe sa largeur : un niveau d'indentation, 1,6 em.
    for w in &cases {
        let r = w.source.octets.from..w.source.octets.to;
        let l = b.lire(|_, e, _| e.range_to_bounds(&r).unwrap().size.width);
        assert!(pres(f32::from(l), 1.6 * 16.0), "largeur de la case : {l:?}");
    }
}

/// Sous-puce décalée d'un vrai niveau, case alignée sur les puces, texte aligné.
#[test]
fn alignement_sous_puce_case_texte() {
    let texte = "- parent
  - enfant
- [ ] tache

autre";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    b.placer(texte.len());
    let puces: Vec<_> = b
        .widgets()
        .into_iter()
        .filter(|w| matches!(w.genre, GenreWidget::Puce { .. }))
        .collect();
    assert_eq!(puces.len(), 2);
    let p0 = b.boite_du_widget(puces[0].source.octets.from);
    let p1 = b.boite_du_widget(puces[1].source.octets.from);
    assert!(
        f32::from(p1.origin.x - p0.origin.x) >= 14.0,
        "décalage de la sous-puce : {:?}",
        p1.origin.x - p0.origin.x
    );
    let debut_case = texte.find("[ ]").unwrap();
    let c = b.boite_du_widget(debut_case);
    assert!(
        (f32::from(c.center().x) - f32::from(p0.center().x)).abs() <= 4.0,
        "case {:?} contre puce {:?}",
        c.center().x,
        p0.center().x
    );
    // Le texte : « parent » et « tache » commencent au même x (à 2 px).
    let x_parent = b.bornes(texte.find("parent").unwrap()).origin.x;
    let x_tache = b.bornes(texte.find("tache").unwrap()).origin.x;
    assert!(
        (f32::from(x_parent) - f32::from(x_tache)).abs() <= 2.0,
        "parent {x_parent:?} contre tache {x_tache:?}"
    );
}

/// Maths en ligne `$x^2$` et `$$y$$` dans la phrase : rendues hors curseur (largeur de la formule,
/// source remplacée), source au curseur.
#[test]
fn math_inline_dans_la_phrase_rendus_hors_curseur() {
    let texte = "avant
a $x^2$ b $$y$$ c
après";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    let l1 = b.rendre_formule("x^2", false);
    let l2 = b.rendre_formule("y", true);
    b.placer(0);
    let ws: Vec<_> = b
        .widgets()
        .into_iter()
        .filter(|w| matches!(w.genre, GenreWidget::Maths { .. }))
        .collect();
    assert_eq!(ws.len(), 2);
    assert!(matches!(
        ws[1].genre,
        GenreWidget::Maths {
            affichage: true,
            ..
        }
    ));
    for (w, attendu) in ws.iter().zip([l1, l2]) {
        let r = w.source.octets.from..w.source.octets.to;
        let largeur = b.lire(|_, e, _| e.range_to_bounds(&r).unwrap().size.width);
        assert!(
            pres(f32::from(largeur), attendu),
            "largeur de la formule {largeur:?} contre rendu {attendu}"
        );
    }
    assert!(b.inline.images() >= 2, "les formules sont peintes");
    assert_eq!(b.texte(), texte);
    // Le curseur dans la première : sa source revient, plus de largeur dictée.
    b.placer(
        "avant
"
        .len()
            + 4,
    );
    let restantes: Vec<_> = b.lire(|e, _, _| e.largeurs_widgets().to_vec());
    assert_eq!(
        restantes.len(),
        1,
        "seule la formule hors curseur reste rendue"
    );
}

/// Un clic sur une formule sélectionne son LaTeX, délimiteurs exclus.
#[test]
fn clic_sur_une_formule_selectionne_le_latex() {
    let texte = "avant
a $x^2$ b
après";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    b.rendre_formule("x^2", false);
    b.placer(0);
    let debut = texte.find('$').unwrap();
    let boite = b.boite_du_widget(debut);
    b.cliquer_au_centre(boite);
    let sel = b.lire(|_, e, _| e.selected_range());
    assert_eq!(sel, debut + 1..debut + 4);
}
