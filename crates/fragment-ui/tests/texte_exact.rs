//! Tests headless du texte peint à l'origine fractionnaire (`fr-p4.1.2`, `rendu::texte`).
//!
//! Système de texte réel (DirectWrite, DM Sans), comme `tests/interligne.rs`. Windows seulement.
//! On vérifie : (1) la boîte est CELLE du texte de GPUI (rien ne bouge autour) ; (2) la peinture
//! part de l'abscisse exacte (fraction comprise) ; (3) le curseur et le pointé lisent la même
//! origine que la peinture ; (4) le coût de mise en page, contre le texte de GPUI.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::mem::ManuallyDrop;
use std::time::Instant;

use fragment_theme::Mode;
use fragment_ui::rendu::texte::{Ancrage, EtatTexteExact, texte_exact, unite};
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, IntoElement,
    ParentElement as _, Pixels, Point, Render, Styled as _, TestAppContext, TestDispatcher,
    TextLayout, Window, WindowBounds, WindowOptions, div, point, px, size,
};

const TEXTE: &str = "fermer";

/// Une rangée de 200 px ancrée à droite : le texte de GPUI à gauche (référence de boîte), le
/// texte exact à droite, ancré à la fin.
struct Vue {
    gpui: TextLayout,
    exact: EtatTexteExact,
    /// Nombre de libellés (banc de coût) et lequel des deux.
    banc: Option<(usize, bool)>,
}

impl Render for Vue {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let racine = div()
            .size_full()
            .font_family(polices::FAMILLE_INTERFACE)
            .font_features(polices::caracteristiques())
            .text_size(px(12.0))
            .line_height(gpui_kit::relative(1.4));
        if let Some((n, exact)) = self.banc {
            return racine.flex().flex_col().children((0..n).map(move |i| {
                let t = format!("Libellé {i} de la légende");
                if exact {
                    div().child(texte_exact(t).ancrage_x(Ancrage::Fin))
                } else {
                    div().child(t)
                }
            }));
        }
        let gpui = gpui_kit::StyledText::new(TEXTE);
        self.gpui = gpui.layout().clone();
        racine
            .flex()
            .flex_row()
            .child(div().flex_none().child(gpui))
            .child(
                div().flex_none().ml(px(7.0)).w(px(200.0)).child(
                    texte_exact(TEXTE)
                        .etat(&self.exact)
                        .pleine_largeur()
                        .ancrage_x(Ancrage::Fin),
                ),
            )
    }
}

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    vue: Entity<Vue>,
}

impl Banc {
    fn new(banc: Option<(usize, bool)>) -> Banc {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let (fenetre, vue) = cx.update(|cx| {
            let fenetre = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(800.0), px(2000.0)),
                    })),
                    ..Default::default()
                },
                cx,
                move |_, cx| {
                    cx.new(|_| Vue {
                        gpui: TextLayout::default(),
                        exact: EtatTexteExact::default(),
                        banc,
                    })
                },
            )
            .unwrap();
            let vue = fenetre.root(cx).unwrap();
            (fenetre.into(), vue)
        });
        let mut b = Banc {
            cx: ManuallyDrop::new(cx),
            fenetre,
            vue,
        };
        b.dessiner();
        b
    }

    fn dessiner(&mut self) {
        self.cx.run_until_parked();
        self.cx
            .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
            .unwrap();
        self.cx.run_until_parked();
    }

    fn lire<R>(&mut self, f: impl FnOnce(&Vue, &App) -> R) -> R {
        let vue = self.vue.clone();
        self.cx.update(|cx| f(vue.read(cx), cx))
    }
}

fn f(p: Pixels) -> f32 {
    f32::from(p)
}

/// La boîte du texte exact a la taille de celle du texte de GPUI : largeur arrondie au pixel
/// supérieur, hauteur d'un interligne calé (16,8 -> 17). Les boîtes restent strictes.
#[test]
fn la_boite_est_celle_du_texte_de_gpui() {
    let mut b = Banc::new(None);
    let (gpui, exact, largeur) = b.lire(|v, _| {
        (
            v.gpui.bounds(),
            v.exact.bornes().unwrap(),
            v.exact.largeur_exacte().unwrap(),
        )
    });
    assert_eq!(f(gpui.size.width), largeur.ceil(), "{gpui:?} / {largeur}");
    assert_eq!(f(gpui.size.height), 17.0);
    assert_eq!(f(exact.size.height), f(gpui.size.height));
    assert!(
        largeur.fract() > 0.0,
        "« {TEXTE} » a une largeur fractionnaire : {largeur}"
    );
    // La boîte pleine largeur est calée au pixel (le fond et la bordure de l'appelant aussi).
    assert_eq!(f(exact.origin.x).fract(), 0.0);
}

/// Ancré à droite dans sa boîte, le texte part de `droite − largeur exacte` (au 1/64) : la
/// fraction que Chromium garde et que GPUI perdait. La ligne de base est celle de Chromium (12 px
/// sous le haut pour DM Sans 12 px, interligne 16,8).
#[test]
fn le_texte_ancre_a_droite_part_de_la_fraction() {
    let mut b = Banc::new(None);
    let (bornes, largeur, o) = b.lire(|v, _| {
        (
            v.exact.bornes().unwrap(),
            v.exact.largeur_exacte().unwrap(),
            v.exact.origine_ligne(0).unwrap(),
        )
    });
    let attendu = f(bornes.origin.x) + unite(f(bornes.size.width) - largeur);
    assert!((f(o.x) - attendu).abs() < 1e-3, "{o:?} contre {attendu}");
    assert!(f(o.x).fract() > 0.0, "origine fractionnaire : {o:?}");
    assert_eq!(f(o.y), f(bornes.origin.y) + 12.0);
}

/// Le curseur et le pointé suivent la peinture : la position du caractère 0 est l'origine peinte,
/// celle de la fin est l'origine plus la largeur exacte, et pointer au milieu d'un caractère
/// rend son octet.
#[test]
fn curseur_et_pointe_suivent_l_origine_peinte() {
    let mut b = Banc::new(None);
    let (o, debut, fin, largeur) = b.lire(|v, _| {
        (
            v.exact.origine_ligne(0).unwrap(),
            v.exact.position_pour_indice(0).unwrap(),
            v.exact.position_pour_indice(TEXTE.len()).unwrap(),
            v.exact.largeur_exacte().unwrap(),
        )
    });
    assert_eq!(debut.x, o.x);
    assert!((f(fin.x) - (f(o.x) + largeur)).abs() < 1e-3, "{fin:?}");
    // Le haut du curseur est au-dessus de la ligne de base, dans la ligne.
    assert!(
        debut.y < o.y && f(o.y - debut.y) < 17.0,
        "{debut:?} / {o:?}"
    );
    for i in 0..TEXTE.len() {
        let (p, q) = b.lire(|v, _| {
            (
                v.exact.position_pour_indice(i).unwrap(),
                v.exact.position_pour_indice(i + 1).unwrap(),
            )
        });
        let milieu_gauche = point(p.x + (q.x - p.x) * 0.25, p.y + px(4.0));
        let trouve = b.lire(|v, _| v.exact.indice_pour_position(milieu_gauche));
        assert_eq!(trouve, Some(i), "pointé du caractère {i}");
    }
}

/// Le coût : la mise en page et la peinture de 400 libellés, texte de GPUI contre texte exact
/// (un seul façonnage chacun, aucune mise en page de plus). Affiché pour le compte rendu ; le
/// test n'échoue que si le texte exact coûte plus du double.
#[test]
fn cout_de_mise_en_page() {
    const N: usize = 400;
    let mut mesures = Vec::new();
    for exact in [false, true] {
        let mut b = Banc::new(Some((N, exact)));
        let debut = Instant::now();
        for _ in 0..20 {
            b.cx.update_window(b.fenetre, |_, window, cx| {
                window.refresh();
                window.render_frame(cx)
            })
            .unwrap();
        }
        mesures.push(debut.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
    println!(
        "TEXTE_EXACT|cout|{N} libellés : GPUI {:.2} ms, exact {:.2} ms par image",
        mesures[0], mesures[1]
    );
    assert!(mesures[1] < 2.0 * mesures[0] + 1.0, "{mesures:?}");
}
