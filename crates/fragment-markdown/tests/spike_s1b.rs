//! Spike S1b : fork MINIMAL de `gpui-base::input` (`vendor/gpui-base`, ADR-006) pour le live preview.
//!
//! Mêmes mécaniques que `spike_s1.rs` : tests HEADLESS (plateforme de test de GPUI,
//! `NoopTextSystem` : chasse fixe 0,6 em, aucun pixel). Ils prouvent le COMPORTEMENT du fork
//! (mise en page, clavier, souris, IME simulé, annulation), pas le rendu. Les lignes `S1b|…`
//! imprimées (`--nocapture`) alimentent `docs/spikes/S1b-fork-input.md`.
//!
//! Les six critères :
//! (a) `#` masqué SANS TROU ; (b) H1 à la hauteur DICTÉE ; (c) bloc de 120 px atomique au clavier ;
//! (d) IME ; (e) annuler / rétablir ; (f) soft wrap et 10 000 lignes — (d) (e) (f) avec le modèle
//! de rangées ACTIF, pour prouver que le fork ne casse rien quand on s'en sert.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ops::Range;
use std::time::Instant;

use gpui_kit::{
    App, AppContext, Bounds, Context, ElementInputHandler, Entity, InputHandler, Pixels, Point,
    Render, ScrollDelta, Size, TestAppContext, Window, WindowBounds, WindowHandle, WindowOptions,
    base::Root,
    component::input::{Editor, EditorState},
    div, point,
    prelude::*,
    px, size,
    test::{TestInput, TestWindowExt},
};

/// Vue minimale : l'éditeur seul (le modèle de rangées est posé directement par les tests).
struct Host {
    state: Entity<EditorState>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            Editor::new(&self.state)
                .appearance(false)
                .bordered(false)
                .size_full(),
        )
    }
}

fn open_window<V: Render>(
    cx: &mut TestAppContext,
    size: Size<Pixels>,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> (WindowHandle<Root>, Entity<V>) {
    cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size,
        };
        let (window, content) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            build,
        )
        .expect("open test window");
        (window.downcast::<Root>().expect("Base Root"), content)
    })
}

fn open(
    cx: &mut TestAppContext,
    text: &str,
    width: f32,
    height: f32,
) -> (WindowHandle<Root>, Entity<EditorState>) {
    open_with(cx, text, width, height, false)
}

fn open_with(
    cx: &mut TestAppContext,
    text: &str,
    width: f32,
    height: f32,
    line_numbers: bool,
) -> (WindowHandle<Root>, Entity<EditorState>) {
    cx.update(gpui_kit::init);
    let value = text.to_string();
    let (handle, host) = open_window(cx, size(px(width), px(height)), move |window, cx| {
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(line_numbers)
                .auto_close(false)
                .smart_indent(false)
                .default_value(value)
        });
        cx.new(|_| Host { state })
    });
    let state = cx.update(|cx| host.read(cx).state.clone());
    (handle, state)
}

/// Plusieurs images pour que la géométrie (retard d'un cadre) se stabilise.
fn settle(window: &mut Window, cx: &mut App) {
    for _ in 0..3 {
        window.render_frame(cx);
    }
}

fn line_start(text: &str, n: usize) -> usize {
    text.split('\n').take(n).map(|l| l.len() + 1).sum()
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.
}

/// À 0,05 px près (arrondis de la mise en page).
fn near(a: Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() < 0.05
}

/// Ce que ferait l'hôte (règle `curseurActif`) : `# ` masqué hors de la ligne du curseur, et la
/// hauteur de chaque titre dictée par le modèle.
fn host_model(text: &str, cursor: usize, h1: f32) -> (Vec<Range<usize>>, Vec<(usize, Pixels)>) {
    let (mut hidden, mut heights) = (Vec::new(), Vec::new());
    let mut start = 0;
    for (n, line) in text.split('\n').enumerate() {
        let end = start + line.len();
        if line.starts_with("# ") {
            heights.push((n, px(h1)));
            if !(start..=end).contains(&cursor) {
                hidden.push(start..start + 2);
            }
        }
        start = end + 1;
    }
    (hidden, heights)
}

// ---------------------------------------------------------------------------------------------

/// (a) `#` masqué SANS TROU : le texte suivant se décale ; révélé, il revient.
#[gpui_kit::test]
fn diese_masque_sans_trou(cx: &mut TestAppContext) {
    let text = "# Titre\n\ncorps\n";
    let (handle, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, cx);
        let x_titre_revele = state.read(cx).range_to_bounds(&(2..3)).unwrap().origin.x;
        let x_debut = state.read(cx).range_to_bounds(&(0..1)).unwrap().origin.x;
        let largeur_diese = state.read(cx).range_to_bounds(&(0..2)).unwrap().size.width;

        state.update(cx, |s, cx| s.set_row_model(vec![0..2], vec![], vec![], cx));
        settle(window, cx);
        let x_titre_cache = state.read(cx).range_to_bounds(&(2..3)).unwrap().origin.x;
        let y_corps = state.read(cx).range_to_bounds(&(9..10)).unwrap().origin.y;
        println!(
            "S1b|masque|x_revele={x_titre_revele:?} x_cache={x_titre_cache:?} x_debut={x_debut:?} \
             largeur_diese={largeur_diese:?}"
        );
        // Sans trou : le « T » prend la place du « # » (x du début de ligne), alors que l'éditeur de
        // série le laissait à 2 × 7,8 px (spike S1).
        assert_eq!(x_titre_cache, x_debut, "le texte suivant se décale");
        assert!(x_titre_revele > x_titre_cache, "révélé, le texte revient");
        assert!(
            f32::from(largeur_diese) > 10.,
            "témoin : « # » occupe 2 cellules"
        );

        // La source reste entière et éditable : le document n'a pas bougé.
        assert_eq!(state.read(cx).value(), text);

        // Révélation sous le curseur : modèle vidé, le texte revient à sa place d'origine.
        state.update(cx, |s, cx| s.set_row_model(vec![], vec![], vec![], cx));
        settle(window, cx);
        let x_retour = state.read(cx).range_to_bounds(&(2..3)).unwrap().origin.x;
        assert_eq!(x_retour, x_titre_revele);

        // Les lignes d'en dessous n'ont pas bougé verticalement (masquer ne change pas la hauteur).
        let y_corps_apres = state.read(cx).range_to_bounds(&(9..10)).unwrap().origin.y;
        assert_eq!(y_corps, y_corps_apres);
    })
    .unwrap();
}

/// (b) Un titre H1 prend la hauteur DICTÉE par le modèle ; la suite est décalée d'autant, la souris
/// et le curseur suivent.
#[gpui_kit::test]
fn h1_a_la_hauteur_dictee(cx: &mut TestAppContext) {
    let text = "# Titre\ncorps\nfin\n";
    let (handle, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, cx);
        let lh = f32::from(state.read(cx).line_height().unwrap());
        let (l1, l2) = (line_start(text, 1), line_start(text, 2));
        let y0 = state.read(cx).range_to_bounds(&(0..1)).unwrap().origin.y;

        let (hidden, heights) = host_model(text, text.len(), 36.);
        state.update(cx, |s, cx| s.set_row_model(hidden, heights, vec![], cx));
        settle(window, cx);
        let titre = state.read(cx).range_to_bounds(&(2..3)).unwrap();
        let corps = state.read(cx).range_to_bounds(&(l1..l1 + 1)).unwrap();
        let fin = state.read(cx).range_to_bounds(&(l2..l2 + 1)).unwrap();
        println!(
            "S1b|h1|lh={lh} hauteur_titre={:?} dy_corps={:?} dy_fin={:?}",
            titre.size.height,
            corps.origin.y - titre.origin.y,
            fin.origin.y - corps.origin.y
        );
        assert!(
            near(titre.size.height, 36.),
            "bounds du titre = hauteur dictée"
        );
        assert!(
            near(corps.origin.y - y0, 36.),
            "le corps est décalé de 36 px"
        );
        assert!(
            near(fin.origin.y - corps.origin.y, lh),
            "les lignes normales gardent lh"
        );

        // Souris : un clic à 30 px sous le haut du titre est encore dans le titre ; à 38 px, dans le corps.
        let editor = window.find(("input", state.entity_id())).bounds();
        let top = titre.origin.y;
        window.click_at(
            ("input", state.entity_id()),
            point(titre.origin.x + px(20.), top + px(30.)) - editor.origin,
            cx,
        );
        let c_titre = state.read(cx).cursor();
        window.click_at(
            ("input", state.entity_id()),
            point(titre.origin.x + px(3.), top + px(38.)) - editor.origin,
            cx,
        );
        let c_corps = state.read(cx).cursor();
        println!("S1b|h1|clic_30px={c_titre} clic_38px={c_corps} debut_corps={l1}");
        assert!(c_titre < l1, "clic à 30 px : dans la ligne du titre");
        assert!((l1..l2).contains(&c_corps), "clic à 38 px : dans le corps");

        // Clavier : Bas depuis le titre atterrit sur le corps.
        state.update(cx, |s, cx| s.set_selected_range(0..0, cx));
        state.update(cx, |s, cx| s.focus(window, cx));
        window.press("down", cx);
        assert!((l1..l2).contains(&state.read(cx).cursor()));
    })
    .unwrap();
}

/// (c) Bloc de 120 px : traversé par Haut/Bas comme un ATOME (le curseur ne peut ni y entrer ni
/// s'y arrêter). Variante « à la place d'une ligne » : ligne source masquée, 120 px de haut.
#[gpui_kit::test]
fn bloc_de_120_px_atomique(cx: &mut TestAppContext) {
    let text = "a\nb\nc\nd\n";
    let (handle, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, cx);
        let lh = f32::from(state.read(cx).line_height().unwrap());
        let y = |s: &EditorState, o: usize| s.range_to_bounds(&(o..o + 1)).unwrap().origin.y;

        // 1. Bloc POSÉ SOUS la ligne « b » (offset 2) : « c » (offset 4) est repoussé de 120 px.
        state.update(cx, |s, cx| {
            s.set_row_model(vec![], vec![], vec![(1, px(120.))], cx)
        });
        settle(window, cx);
        let (yb, yc, yd) = {
            let s = state.read(cx);
            (y(s, 2), y(s, 4), y(s, 6))
        };
        println!("S1b|bloc|dy_b_c={:?} dy_c_d={:?} lh={lh}", yc - yb, yd - yc);
        assert!(near(yc - yb, lh + 120.), "bloc de 120 px sous « b »");
        assert!(near(yd - yc, lh), "après le bloc, retour à lh");

        state.update(cx, |s, cx| s.set_selected_range(0..0, cx));
        state.update(cx, |s, cx| s.focus(window, cx));
        window.press("down", cx);
        assert_eq!(state.read(cx).cursor(), 2, "Bas : a -> b");
        window.press("down", cx);
        assert_eq!(
            state.read(cx).cursor(),
            4,
            "Bas : b -> c, d'un coup (le bloc n'est pas une rangée)"
        );
        window.press("up", cx);
        assert_eq!(state.read(cx).cursor(), 2, "Haut : c -> b, d'un coup");
        window.press("up", cx);
        assert_eq!(state.read(cx).cursor(), 0, "Haut : b -> a");

        // Un clic DANS le bloc (sous « b ») tombe sur la ligne suivante, jamais sur une source cachée.
        let editor = window.find(("input", state.entity_id())).bounds();
        let bounds_b = state.read(cx).range_to_bounds(&(2..3)).unwrap();
        window.click_at(
            ("input", state.entity_id()),
            point(bounds_b.origin.x + px(5.), bounds_b.origin.y + px(lh + 60.)) - editor.origin,
            cx,
        );
        let c = state.read(cx).cursor();
        assert!(
            (4..=5).contains(&c),
            "clic dans le bloc -> ligne « c » ({c})"
        );

        // 2. Bloc À LA PLACE de la ligne « b » : source masquée, rangée de 120 px. « c » est à
        // 120 px de « b » et le curseur la traverse en un pas.
        state.update(cx, |s, cx| {
            s.set_row_model(vec![2..3], vec![(1, px(120.))], vec![], cx)
        });
        settle(window, cx);
        let (yb, yc) = {
            let s = state.read(cx);
            (y(s, 2), y(s, 4))
        };
        println!("S1b|bloc_remplace|dy_b_c={:?}", yc - yb);
        assert!(near(yc - yb, 120.));
        assert_eq!(state.read(cx).value(), text, "la source est intacte");
        state.update(cx, |s, cx| s.set_selected_range(0..0, cx));
        window.press("down", cx);
        window.press("down", cx);
        assert_eq!(
            state.read(cx).cursor(),
            4,
            "a -> (bloc) -> c en deux pas, jamais dans la source"
        );
    })
    .unwrap();
}

/// (d) IME (protocole de composition) toujours vert, modèle de rangées actif (`# ` masqué, titre H1).
#[gpui_kit::test]
fn ime_toujours_vert_avec_le_fork(cx: &mut TestAppContext) {
    let (handle, state) = open(cx, "", 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("input", state.entity_id()), cx);
        window.input("# ", cx);
        // Le `# ` est masqué et la ligne est un H1 PENDANT la composition.
        state.update(cx, |s, cx| {
            s.set_row_model(vec![0..2], vec![(0, px(36.))], vec![], cx)
        });
        settle(window, cx);
        let bounds = window.find(("input", state.entity_id())).bounds();
        let mut ime = TestInput::new(ElementInputHandler::new(bounds, state.clone()), window);
        ime.compose_text(None, "に", Some(1..1), cx);
        let m1 = ime.marked_text_range(cx);
        ime.compose_text(None, "日本", Some(2..2), cx);
        let m2 = ime.marked_text_range(cx);
        ime.commit_text("日本語", cx);
        assert_eq!(ime.marked_text_range(cx), None);
        let utf16 = ime.selected_text_range(cx).unwrap().range;
        drop(ime);
        settle(window, cx);
        assert_eq!(state.read(cx).value(), "# 日本語");
        // Mêmes valeurs que le stock (spike S1) : 5 en UTF-16 côté système, 11 en octets côté éditeur.
        assert_eq!(utf16, 5..5);
        assert_eq!(state.read(cx).selected_range(), 11..11);
        // Le masquage a suivi l'édition (la plage `0..2` n'a pas bougé) : « 日 » est au x du début.
        let x_debut = state.read(cx).range_to_bounds(&(0..1)).unwrap().origin.x;
        let x_texte = state.read(cx).range_to_bounds(&(2..5)).unwrap().origin.x;
        assert_eq!(x_debut, x_texte);
        println!("S1b|ime|marque1={m1:?} marque2={m2:?} utf16={utf16:?} octets=11..11");
    })
    .unwrap();
}

/// (e) Annuler / rétablir toujours verts, modèle de rangées actif ; les plages masquées suivent les
/// éditions sans paniquer.
#[gpui_kit::test]
fn annuler_et_retablir_avec_le_fork(cx: &mut TestAppContext) {
    let (handle, state) = open(cx, "", 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("input", state.entity_id()), cx);
        window.input("# ab", cx);
        window.press("enter", cx);
        window.input("cd", cx);
        state.update(cx, |s, cx| {
            s.set_row_model(vec![0..2], vec![(0, px(36.))], vec![(1, px(40.))], cx)
        });
        settle(window, cx);
        let plein = state.read(cx).value();
        assert_eq!(plein, "# ab\ncd");
        window.press("secondary-z", cx);
        let apres_annulation = state.read(cx).value();
        settle(window, cx);
        window.press("ctrl-y", cx);
        let apres_retablissement = state.read(cx).value();
        settle(window, cx);
        println!("S1b|undo|apres={apres_annulation:?} refait={apres_retablissement:?}");
        assert_ne!(apres_annulation, plein);
        assert_eq!(apres_retablissement, plein);
        // Frappe dans la ligne masquée : la plage masquée se décale avec l'insertion en tête.
        state.update(cx, |s, cx| s.set_selected_range(0..0, cx));
        window.input("x", cx);
        settle(window, cx);
        assert_eq!(state.read(cx).value(), "x# ab\ncd");
    })
    .unwrap();
}

/// (f) Soft wrap et 10 000 lignes toujours verts, modèle de rangées actif.
#[gpui_kit::test]
fn soft_wrap_avec_le_fork(cx: &mut TestAppContext) {
    let long = "mot ".repeat(120);
    let text = format!("{long}\nfin\n");
    let (handle, state) = open(cx, &text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        state.update(cx, |s, cx| s.set_soft_wrap(true, window, cx));
        // « mot » du début masqué, et la ligne « fin » plus haute.
        state.update(cx, |s, cx| {
            s.set_row_model(vec![0..4], vec![(1, px(30.))], vec![], cx)
        });
        settle(window, cx);
        let rows = state.read(cx).display_map().wrap_row_count();
        let first = state.read(cx).range_to_bounds(&(4..5)).unwrap();
        let last_ix = long.len() - 2;
        let last = state
            .read(cx)
            .range_to_bounds(&(last_ix..last_ix + 1))
            .unwrap();
        let fin_ix = long.len() + 1;
        let fin = state
            .read(cx)
            .range_to_bounds(&(fin_ix..fin_ix + 1))
            .unwrap();
        assert!(rows >= 4, "{rows} rangées visuelles");
        assert!(last.origin.y > first.origin.y);
        assert!(near(fin.size.height, 30.));
        window.click(("input", state.entity_id()), cx);
        window.press("ctrl-home", cx);
        window.press("down", cx);
        let c = state.read(cx).cursor();
        assert!(c > 0 && c < long.len(), "Bas suit la rangée visuelle ({c})");
        println!(
            "S1b|wrap|rangees={rows} y_debut={:?} y_fin={:?} curseur_apres_bas={c}",
            first.origin.y, last.origin.y
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn dix_mille_lignes_avec_le_fork(cx: &mut TestAppContext) {
    let mut text = String::new();
    for i in 0..10_000 {
        match i % 10 {
            0 => text.push_str(&format!("# Titre {i}\n")),
            3 => text.push_str(&format!("- [ ] tâche {i} avec **gras** et du texte\n")),
            _ => text.push_str(&format!(
                "Ligne {i} de texte lorem ipsum dolor sit amet, consectetur adipiscing\n"
            )),
        }
    }
    let (handle, state) = open(cx, &text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // 1 000 titres : `# ` masqué et hauteur 30 px ; un bloc de 40 px sous chaque tâche (1 000).
        let (hidden, heights) = host_model(&text, usize::MAX, 30.);
        let blocks: Vec<(usize, Pixels)> = (0..10_000)
            .filter(|i| i % 10 == 3)
            .map(|i| (i, px(40.)))
            .collect();
        let t = Instant::now();
        state.update(cx, |s, cx| {
            s.set_row_model(hidden.clone(), heights, blocks, cx)
        });
        window.render_frame(cx);
        let model_ms = ms(t);

        let lh = state.read(cx).line_height().unwrap();
        let mut worst = 0f64;
        let t_all = Instant::now();
        for _ in 0..100 {
            let t = Instant::now();
            window.scroll(
                ("input", state.entity_id()),
                ScrollDelta::Pixels(point(px(0.), -lh * 3.)),
                cx,
            );
            worst = worst.max(ms(t));
        }
        let scroll_mean = ms(t_all) / 100.;
        let visible = state.read(cx).visible_row_range();
        assert!(
            visible.as_ref().is_some_and(|r| r.start > 100),
            "{visible:?}"
        );

        // Un titre visible : sa géométrie reflète la hauteur dictée ET le masquage.
        let r = visible.clone().unwrap();
        let ligne_titre = (r.start..r.end).find(|l| l % 10 == 0).unwrap();
        let o = line_start(&text, ligne_titre);
        let titre = state.read(cx).range_to_bounds(&(o + 2..o + 3));
        let debut = state.read(cx).range_to_bounds(&(o..o + 1));
        if let (Some(t), Some(d)) = (titre, debut) {
            assert_eq!(t.origin.x, d.origin.x, "masqué sans trou à 10 000 lignes");
            assert!(near(d.size.height, 30.));
        }

        // Frappe au milieu du document.
        let mid = text.floor_char_boundary(text.len() / 2);
        state.update(cx, |s, cx| s.set_selected_range(mid..mid, cx));
        state.update(cx, |s, cx| s.focus(window, cx));
        let t_type = Instant::now();
        for _ in 0..20 {
            window.input("x", cx);
        }
        let type_mean = ms(t_type) / 20.;
        println!(
            "S1b|10k|octets={} masquees={} pose_modele_ms={model_ms:.1} \
             defilement_moy_ms={scroll_mean:.2} defilement_max_ms={worst:.2} \
             frappe_moy_ms={type_mean:.2} lignes_visibles={visible:?}",
            text.len(),
            hidden.len(),
        );
        assert!(scroll_mean < 2000., "défilement absurdement lent");
    })
    .unwrap();
}

// ---------------------------------------------------------------------------------------------
// Dettes soldées (tâche fr-p3-fork-input, ADR-006) : police par rangée, gouttière et IME à hauteurs
// variables, plage masquée atomique à Gauche/Droite.

/// (g) La taille de police dictée change la LARGEUR des glyphes (un H1 plus GROS, pas seulement plus
/// haut). La chasse du `NoopTextSystem` est proportionnelle au corps : largeur(40) = 2 × largeur(20).
#[gpui_kit::test]
fn police_dictee_par_rangee(cx: &mut TestAppContext) {
    let text = "# Titre
corps
";
    let (handle, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, cx);
        let l1 = line_start(text, 1);
        let largeur =
            |cx: &App, r: Range<usize>| state.read(cx).range_to_bounds(&r).unwrap().size.width;
        let base_titre = largeur(cx, 2..3);
        let base_corps = largeur(cx, l1..l1 + 1);
        assert!(
            near(base_titre, f32::from(base_corps)),
            "même corps au départ"
        );

        state.update(cx, |s, cx| s.set_line_font_sizes(vec![(0, px(40.))], cx));
        settle(window, cx);
        let w40 = largeur(cx, 2..3);
        state.update(cx, |s, cx| s.set_line_font_sizes(vec![(0, px(20.))], cx));
        settle(window, cx);
        let w20 = largeur(cx, 2..3);
        let corps = largeur(cx, l1..l1 + 1);
        println!("S1b|police|base={base_titre:?} w20={w20:?} w40={w40:?} corps={corps:?}");
        assert!(
            near(w40, 2. * f32::from(w20)),
            "la largeur suit la police dictée"
        );
        assert!(w40 > base_titre, "le titre est plus gros que le corps");
        assert!(
            near(corps, f32::from(base_corps)),
            "les autres lignes gardent leur corps"
        );

        // Avec `# ` masqué et une hauteur dictée en plus : tout coexiste.
        let (hidden, heights) = host_model(text, text.len(), 36.);
        state.update(cx, |s, cx| {
            s.set_row_model(hidden, heights, vec![], cx);
            s.set_line_font_sizes(vec![(0, px(40.))], cx);
        });
        settle(window, cx);
        assert!(near(largeur(cx, 2..3), f32::from(w40)));
        // Vider le modèle de polices : retour au corps de base.
        state.update(cx, |s, cx| s.set_line_font_sizes(vec![], cx));
        settle(window, cx);
        assert!(near(largeur(cx, 2..3), f32::from(base_titre)));
    })
    .unwrap();
}

/// (h) Gouttière allumée et IME : le panneau IME a la hauteur DICTÉE de la rangée de la plage, et
/// la ligne suivante est décalée d'autant. (Le dessin des numéros n'est pas observable en headless :
/// il emprunte la même arithmétique, `LineLayout::row_height` + `block_below`, que le texte.)
#[gpui_kit::test]
fn gouttiere_et_ime_suivent_les_hauteurs(cx: &mut TestAppContext) {
    let text = "# Titre
corps
fin
";
    let (handle, state) = open_with(cx, text, 800., 480., true);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, cx);
        let lh = f32::from(state.read(cx).line_height().unwrap());
        let (l1, l2) = (line_start(text, 1), line_start(text, 2));
        let (hidden, heights) = host_model(text, text.len(), 36.);
        state.update(cx, |s, cx| {
            s.set_row_model(hidden, heights, vec![(1, px(50.))], cx)
        });
        settle(window, cx);
        let bounds = window.find(("input", state.entity_id())).bounds();
        let mut ime = ElementInputHandler::new(bounds, state.clone());
        let titre = ime.bounds_for_range(2..3, window, cx).unwrap();
        let corps = ime.bounds_for_range(l1..l1 + 1, window, cx).unwrap();
        let fin = ime.bounds_for_range(l2..l2 + 1, window, cx).unwrap();
        println!(
            "S1b|ime_hauteurs|titre={:?} corps={:?} dy_corps={:?} dy_fin={:?}",
            titre.size.height,
            corps.size.height,
            corps.origin.y - titre.origin.y,
            fin.origin.y - corps.origin.y
        );
        assert!(
            near(titre.size.height, 36.),
            "panneau IME = rangée du titre"
        );
        assert!(
            near(corps.size.height, lh),
            "ligne normale : hauteur de base"
        );
        assert!(near(corps.origin.y - titre.origin.y, 36.));
        assert!(
            near(fin.origin.y - corps.origin.y, lh + 50.),
            "le bloc décale la suite"
        );
    })
    .unwrap();
}

/// (i) Gauche/Droite franchissent une plage masquée d'un seul pas (atome), dans les deux sens ;
/// la sélection au clavier aussi. Sans plage masquée : un pas d'un caractère comme avant.
#[gpui_kit::test]
fn gauche_droite_franchissent_la_plage_masquee(cx: &mut TestAppContext) {
    let text = "# Titre
corps
";
    let (handle, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, cx);
        state.update(cx, |s, cx| s.focus(window, cx));
        // Curseur sur la ligne 1 : `# ` (0..2) est masqué.
        let (hidden, heights) = host_model(text, text.len(), 36.);
        assert_eq!(hidden, vec![0..2]);
        state.update(cx, |s, cx| s.set_row_model(hidden, heights, vec![], cx));
        settle(window, cx);

        state.update(cx, |s, cx| s.set_selected_range(2..2, cx));
        window.press("left", cx);
        assert_eq!(state.read(cx).cursor(), 0, "Gauche : de 2 à 0 en un pas");
        window.press("right", cx);
        assert_eq!(state.read(cx).cursor(), 2, "Droite : de 0 à 2 en un pas");
        window.press("right", cx);
        assert_eq!(
            state.read(cx).cursor(),
            3,
            "hors de la plage : un caractère"
        );

        state.update(cx, |s, cx| s.set_selected_range(2..2, cx));
        window.press("shift-left", cx);
        assert_eq!(
            state.read(cx).selected_range(),
            0..2,
            "Maj+Gauche : la plage entière"
        );

        // Plus de plage masquée : retour au pas d'un caractère.
        state.update(cx, |s, cx| s.set_row_model(vec![], vec![], vec![], cx));
        settle(window, cx);
        state.update(cx, |s, cx| s.set_selected_range(2..2, cx));
        window.press("left", cx);
        assert_eq!(state.read(cx).cursor(), 1);
    })
    .unwrap();
}
