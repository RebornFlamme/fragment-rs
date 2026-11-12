//! Test headless de l'interligne exact de l'éditeur du live preview (`fr-t3.5`, ADR-015).
//!
//! GPUI arrondit l'interligne au pixel (`Window::line_height`) : 16 × 1,4 = 22,4 devenait 22, et
//! chaque ligne dérivait de 0,4 px contre Chromium. Le fork lit l'interligne exact.
//!
//! Même banc que `tests/editeur.rs` (système de texte réel, DM Sans), recopié ici pour ne pas
//! toucher le fichier de `fr-t3.4`. Windows seulement.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_text::editor::DocumentSurface as _;
use fragment_theme::Mode;
use fragment_ui::editeur::constantes::{INTERLIGNE_TEXTE, MARGE_CONTENU_VERTICALE};
use fragment_ui::editeur::{EditeurLive, OptionsEditeur};
use fragment_ui::{polices, theme_global};
use gpui_kit::base::input::EditorState;
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Pixels, Point, TestAppContext,
    TestDispatcher, Window, WindowBounds, WindowOptions, px, size,
};

const LARGEUR: f32 = 1000.;

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    editeur: Entity<EditeurLive>,
    etat: Entity<EditorState>,
    fini: Arc<AtomicBool>,
}

const DELAI_TEST: Duration = Duration::from_secs(300);

/// Borne le test : un test figé échoue en 5 min, nommé, au lieu de bloquer la CI.
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
        eprintln!("INTERLIGNE|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

impl Drop for Banc {
    fn drop(&mut self) {
        self.fini.store(true, Ordering::Relaxed);
    }
}

impl Banc {
    fn new(texte: &str, l: f32, h: f32) -> Banc {
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
                move |window, cx| {
                    cx.new(|cx| EditeurLive::new(&texte, OptionsEditeur::default(), window, cx))
                },
            )
            .unwrap()
        });
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let etat = cx.update(|cx| editeur.read(cx).etat().clone());
        let fini = Arc::new(AtomicBool::new(false));
        chien_de_garde(fini.clone());
        let mut b = Banc {
            fini,
            cx: ManuallyDrop::new(cx),
            fenetre,
            editeur,
            etat,
        };
        b.stabiliser();
        b
    }

    fn stabiliser(&mut self) {
        for _ in 0..3 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    fn agir(&mut self, f: impl FnOnce(&mut Window, &mut App, &Entity<EditorState>)) {
        let etat = self.etat.clone();
        self.cx
            .update_window(self.fenetre, |_, window, cx| f(window, cx, &etat))
            .unwrap();
        self.stabiliser();
    }

    fn lire<R>(&mut self, f: impl FnOnce(&EditeurLive, &EditorState, &App) -> R) -> R {
        let (editeur, etat) = (self.editeur.clone(), self.etat.clone());
        self.cx.update(|cx| f(editeur.read(cx), etat.read(cx), cx))
    }

    fn bornes(&mut self, o: usize) -> Bounds<Pixels> {
        self.lire(|_, e, _| e.range_to_bounds(&(o..o + 1)).unwrap())
    }

    fn placer(&mut self, o: usize) {
        self.agir(|window, cx, etat| {
            etat.update(cx, |s, cx| {
                s.focus(window, cx);
                s.set_selected_range(o..o, cx);
            });
        });
    }
}

fn pres(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

/// La ligne 100 commence à 100 × 22,4 = 2 240 px sous la ligne 0 (Chromium), pas à 100 × 22 =
/// 2 200 (l'arrondi de GPUI). Fenêtre haute pour que les deux lignes soient mises en page sans
/// défiler ; puis 22,4 px entre les lignes 99 et 100 après un défilement.
#[test]
fn interligne_fractionnaire_ligne_100() {
    let mut texte = String::new();
    for i in 0..150 {
        texte.push_str(&format!("Ligne {i}\n"));
    }
    let debut = |l: usize| -> usize { texte.split_inclusive('\n').take(l).map(str::len).sum() };
    let (o0, o100) = (debut(0), debut(100));
    let interligne = 16. * INTERLIGNE_TEXTE;

    let mut b = Banc::new(&texte, LARGEUR, 2600.);
    let lh = b.lire(|_, e, _| f32::from(e.line_height().unwrap()));
    assert!(pres(lh, interligne), "interligne de base : {lh}");
    let ecart = f32::from(b.bornes(o100).origin.y - b.bornes(o0).origin.y);
    println!("INTERLIGNE|ligne 100 à {ecart} px de la ligne 0");
    assert!(
        pres(ecart, 100. * interligne),
        "ligne 100 : {ecart} px, attendu {}",
        100. * interligne
    );
    // La surface (repère document) dit la même chose.
    let haut = b.lire(|e, _, cx| e.surface(cx).haut_de_ligne(100));
    assert!(
        pres(haut, MARGE_CONTENU_VERTICALE + 100. * interligne),
        "{haut}"
    );

    // Fenêtre de 600 px : le curseur en ligne 100 la fait défiler à l'écran.
    let mut b = Banc::new(&texte, LARGEUR, 600.);
    b.placer(o100);
    let y99 = b.bornes(debut(99)).origin.y;
    let y100 = b.bornes(o100).origin.y;
    assert!(
        pres(f32::from(y100 - y99), interligne),
        "{y99:?} → {y100:?}"
    );
    let (haut, top) = b.lire(|e, _, cx| {
        let s = e.surface(cx);
        (
            s.haut_de_ligne(100),
            s.coords_at_pos(fragment_text::utf16::byte_to_utf16(e.texte(), o100)),
        )
    });
    assert!(
        top.is_some_and(|r| pres(r.top as f32, haut)),
        "{top:?} contre {haut}"
    );
}
