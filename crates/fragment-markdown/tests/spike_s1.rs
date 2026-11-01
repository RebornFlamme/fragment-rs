//! Spike S1 : l'éditeur Markdown live preview sur l'éditeur de gpui-kit (`EditorState`).
//!
//! Tests HEADLESS (plateforme de test de GPUI : `NoopTextSystem`, police à chasse fixe
//! de 0,6 em, aucun pixel). Ils prouvent le COMPORTEMENT (frappe, modèle, clics, géométrie
//! logique), pas le rendu. Les lignes `S1|…` imprimées (`--nocapture`) alimentent
//! `docs/spikes/S1-editeur.md`.
//!
//! Windows seulement : les crates GPUI ne se construisent pas sous Linux sans bibliothèques
//! système (le job `test-linux` n'en installe pas).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "spike_s1/live.rs"]
mod live;

use std::time::Instant;

use gpui_kit::{
    App, AppContext, Bounds, ElementInputHandler, Entity, Pixels, Point, Render, ScrollDelta, Size,
    TestAppContext, Window, WindowBounds, WindowHandle, WindowOptions,
    base::Root,
    component::input::EditorState,
    point, px, size,
    test::{TestInput, TestWindowExt},
};
use live::{Kind, LiveView, compute};

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

/// Ouvre une fenêtre avec l'éditeur Markdown habillé par `LiveView`.
fn open(
    cx: &mut TestAppContext,
    text: &str,
    width: f32,
    height: f32,
) -> (WindowHandle<Root>, Entity<LiveView>, Entity<EditorState>) {
    cx.update(gpui_kit::init);
    let value = text.to_string();
    let (handle, view) = open_window(cx, size(px(width), px(height)), move |window, cx| {
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(false)
                .auto_close(false)
                .smart_indent(false)
                .default_value(value)
        });
        cx.new(|cx| LiveView::new(state, cx))
    });
    let state = cx.update(|cx| view.read(cx).state.clone());
    (handle, view, state)
}

/// Plusieurs images pour que la géométrie (retard d'un cadre) et les décorations se stabilisent.
fn settle(window: &mut Window, view: &Entity<LiveView>, cx: &mut App) {
    for _ in 0..3 {
        view.update(cx, |view, cx| {
            view.refresh(cx);
            cx.notify();
        });
        window.render_frame(cx);
    }
}

fn sorted(mut v: Vec<std::ops::Range<usize>>) -> Vec<std::ops::Range<usize>> {
    v.sort_by_key(|r| r.start);
    v
}

fn line_start(text: &str, n: usize) -> usize {
    text.split('\n').take(n).map(|l| l.len() + 1).sum()
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.
}

// ---------------------------------------------------------------------------------------------

/// Point 1 : frappe au clavier (texte, Entrée) dans l'éditeur Markdown.
#[gpui_kit::test]
fn frappe_et_retour_chariot(cx: &mut TestAppContext) {
    let (handle, view, state) = open(cx, "", 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("input", state.entity_id()), cx);
        window.input("# Titre", cx);
        window.press("enter", cx);
        window.input("corps **gras**", cx);
        settle(window, &view, cx);
        assert_eq!(state.read(cx).value(), "# Titre\ncorps **gras**");
        println!(
            "S1|frappe|langage={} valeur_ok=true",
            state.read(cx).language_name()
        );
    })
    .unwrap();
}

/// Point 1 bis : composition IME SIMULÉE (protocole `replace_and_mark_text_in_range`).
/// L'IME du système (fenêtre de candidats, DirectWrite/TSF) n'est PAS testable en headless.
#[gpui_kit::test]
fn composition_ime_simulee_et_unites_utf16(cx: &mut TestAppContext) {
    let (handle, view, state) = open(cx, "", 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("input", state.entity_id()), cx);
        window.input("# ", cx);
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
        settle(window, &view, cx);
        assert_eq!(state.read(cx).value(), "# 日本語");
        // ★ ADR-004 : l'éditeur parle UTF-16 vers le système (IME) et OCTETS UTF-8 vers
        // l'application (sélection, décorations) : la conversion est à nous, à la frontière.
        assert_eq!(utf16, 5..5);
        assert_eq!(state.read(cx).selected_range(), 11..11);
        println!("S1|ime|marque1={m1:?} marque2={m2:?} utf16={utf16:?} octets=11..11");
    })
    .unwrap();
}

/// Point 2 : `#` masqué hors curseur, révélé quand le curseur est sur la ligne (règle `curseurActif`).
#[gpui_kit::test]
fn diese_masque_hors_curseur_et_revele_dessous(cx: &mut TestAppContext) {
    let text = "# Titre\n\ncorps **gras** fin\n";
    let (handle, view, state) = open(cx, text, 800., 480.);

    let mut x_cache = None;
    let mut heights = None;
    // Curseur à la fin (ligne vide finale) : tout est caché.
    cx.update_window(handle.into(), |_, window, cx| {
        let end = text.len();
        state.update(cx, |s, cx| s.set_selected_range(end..end, cx));
        settle(window, &view, cx);
        let conceal = sorted(view.read(cx).conceal.get_ranges(cx));
        assert_eq!(conceal, vec![0..2, 15..17, 21..23]);
        let s = state.read(cx);
        x_cache = Some(s.range_to_bounds(&(2..3)).unwrap().origin.x);
        heights = Some((
            s.range_to_bounds(&(0..1)).unwrap().size.height,
            s.range_to_bounds(&(15..16)).unwrap().size.height,
        ));
    })
    .unwrap();

    // Curseur dans le titre : `# ` révélé, le gras reste caché.
    let mut x_reveal = None;
    cx.update_window(handle.into(), |_, window, cx| {
        state.update(cx, |s, cx| s.set_selected_range(4..4, cx));
        settle(window, &view, cx);
        let conceal = sorted(view.read(cx).conceal.get_ranges(cx));
        assert_eq!(conceal, vec![15..17, 21..23]);
        x_reveal = Some(state.read(cx).range_to_bounds(&(2..3)).unwrap().origin.x);
    })
    .unwrap();

    // Curseur dans le gras : `**` révélés, `# ` caché.
    cx.update_window(handle.into(), |_, window, cx| {
        state.update(cx, |s, cx| s.set_selected_range(18..18, cx));
        settle(window, &view, cx);
        let conceal = sorted(view.read(cx).conceal.get_ranges(cx));
        assert_eq!(conceal, vec![0..2]);
    })
    .unwrap();

    // ★ Constat structurant : cacher ne change PAS la mise en page (glyphes transparents,
    // largeur conservée) ; Obsidian/Fragment font disparaître `# ` et le texte se décale.
    let (x_cache, x_reveal) = (x_cache.unwrap(), x_reveal.unwrap());
    assert_eq!(
        x_cache, x_reveal,
        "une décoration ne reflue jamais le texte"
    );
    let (h_titre, h_corps) = heights.unwrap();
    assert_eq!(h_titre, h_corps, "toutes les lignes ont la même hauteur");
    println!(
        "S1|conceal|x_cache={x_cache:?} x_revele={x_reveal:?} h_titre={h_titre:?} h_corps={h_corps:?}"
    );
}

/// L'observation de l'entité suffit à rejouer la règle quand le curseur bouge (sans appel explicite).
#[gpui_kit::test]
fn le_curseur_declenche_la_regle_par_observation(cx: &mut TestAppContext) {
    let text = "# Titre\n\ncorps\n";
    let (handle, view, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("input", state.entity_id()), cx);
        state.update(cx, |s, cx| s.set_selected_range(text.len()..text.len(), cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, _, cx| {
        assert_eq!(sorted(view.read(cx).conceal.get_ranges(cx)), vec![0..2]);
        state.update(cx, |s, cx| s.set_selected_range(3..3, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, _, cx| {
        assert!(view.read(cx).conceal.get_ranges(cx).is_empty());
    })
    .unwrap();
}

/// Point 3 : case à cocher INLINE cliquable (widget posé sur `[ ]`, la source reste le texte).
#[gpui_kit::test]
fn case_a_cocher_inline_cliquable(cx: &mut TestAppContext) {
    let text = "- [ ] tâche\nautre\n";
    let (handle, view, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, &view, cx);
        assert_eq!(view.read(cx).task_box, Some(2..5));
        let b = window.find("task-box").bounds();
        let src = state.read(cx).range_to_bounds(&(2..5)).unwrap();
        println!(
            "S1|case|widget={:?} source={:?}",
            (b.origin, b.size),
            (src.origin, src.size)
        );
        window.click("task-box", cx);
        settle(window, &view, cx);
        assert_eq!(state.read(cx).value(), "- [x] tâche\nautre\n");
        assert_eq!(view.read(cx).log, vec!["task"]);
        // Un pas d'annulation, comme dans Fragment (une transaction).
        state.update(cx, |s, cx| s.focus(window, cx));
        window.press("secondary-z", cx);
        settle(window, &view, cx);
        assert_eq!(state.read(cx).value(), "- [ ] tâche\nautre\n");
        window.press("ctrl-y", cx);
        assert_eq!(state.read(cx).value(), "- [x] tâche\nautre\n");
    })
    .unwrap();
}

/// Point 4 : widget de BLOC (tableau) posé sur ses lignes sources ; clic sous le bloc sur la bonne ligne.
#[gpui_kit::test]
fn widget_de_bloc_et_clic_dessous(cx: &mut TestAppContext) {
    let text = "intro\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\naprès\n";
    let (l1, l2, l5, l6) = (
        line_start(text, 1),
        line_start(text, 2),
        line_start(text, 5),
        line_start(text, 6),
    );
    let block = l2..l5 - 1;
    let (handle, view, state) = open(cx, text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |v, _| v.block = Some(block.clone()));
        settle(window, &view, cx);
        let editor = window.find(("input", state.entity_id())).bounds();
        let widget = window.find("block-table").bounds();
        let lh = state.read(cx).line_height().unwrap();
        let row2 = state.read(cx).range_to_bounds(&(l2..l2 + 1)).unwrap();
        println!(
            "S1|bloc|widget_y={:?} ligne2_y={:?} hauteur_widget={:?} 3_lignes={:?}",
            widget.origin.y,
            row2.origin.y,
            widget.size.height,
            lh * 3.
        );

        // Clic SUR le widget : le journal le reçoit, le curseur ne bouge pas.
        state.update(cx, |s, cx| s.set_selected_range(0..0, cx));
        window.click("block-table", cx);
        assert_eq!(view.read(cx).log, vec!["table"]);
        assert_eq!(state.read(cx).cursor(), 0);

        // Clic SOUS le widget : la ligne 6 ("après") reçoit le curseur.
        let below = state
            .read(cx)
            .range_to_bounds(&(l6..l6 + 1))
            .unwrap()
            .center();
        window.click_at(("input", state.entity_id()), below - editor.origin, cx);
        let c = state.read(cx).cursor();
        assert!(
            (l6..=l6 + 1).contains(&c),
            "clic sous le bloc : curseur {c}, ligne attendue {l6}"
        );

        // Clic sur la ligne vide juste sous le tableau : la ligne 5.
        let blank = state.read(cx).range_to_bounds(&(l5..l5)).unwrap();
        let p = point(blank.origin.x + px(20.), blank.origin.y + lh / 2.);
        window.click_at(("input", state.entity_id()), p - editor.origin, cx);
        assert_eq!(state.read(cx).cursor(), l5);

        // ★ Non atomique au CLAVIER : Bas depuis la ligne 1 entre dans les lignes cachées.
        state.update(cx, |s, cx| s.set_selected_range(l1..l1, cx));
        state.update(cx, |s, cx| s.focus(window, cx));
        window.press("down", cx);
        let k = state.read(cx).cursor();
        println!("S1|bloc|curseur_apres_bas={k} bloc={block:?}");
        assert!(block.contains(&k), "le caret entre dans la source cachée");
    })
    .unwrap();
}

/// Point 5 : soft wrap (le curseur suit les lignes VISUELLES, géométrie cohérente).
#[gpui_kit::test]
fn soft_wrap(cx: &mut TestAppContext) {
    let long = "mot ".repeat(120);
    let text = format!("{long}\nfin\n");
    let (handle, view, state) = open(cx, &text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        state.update(cx, |s, cx| s.set_soft_wrap(true, window, cx));
        settle(window, &view, cx);
        let rows = state.read(cx).display_map().wrap_row_count();
        let first = state.read(cx).range_to_bounds(&(0..1)).unwrap();
        let last_ix = long.len() - 2;
        let last = state
            .read(cx)
            .range_to_bounds(&(last_ix..last_ix + 1))
            .unwrap();
        assert!(rows >= 4, "{rows} rangées visuelles");
        assert!(last.origin.y > first.origin.y);
        window.click(("input", state.entity_id()), cx);
        window.press("ctrl-home", cx);
        window.press("down", cx);
        let c = state.read(cx).cursor();
        assert!(c > 0 && c < long.len(), "Bas suit la rangée visuelle ({c})");
        println!(
            "S1|wrap|rangees={rows} y_debut={:?} y_fin={:?} curseur_apres_bas={c}",
            first.origin.y, last.origin.y
        );
        state.update(cx, |s, cx| s.set_soft_wrap(false, window, cx));
        settle(window, &view, cx);
        println!(
            "S1|wrap|rangees_sans_wrap={}",
            state.read(cx).display_map().wrap_row_count()
        );
    })
    .unwrap();
}

/// Point 6 : annulation / rétablissement de la frappe (groupée) ; décorations qui suivent.
#[gpui_kit::test]
fn annuler_et_retablir(cx: &mut TestAppContext) {
    let (handle, view, state) = open(cx, "", 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("input", state.entity_id()), cx);
        window.input("# ab", cx);
        window.press("enter", cx);
        window.input("cd", cx);
        settle(window, &view, cx);
        assert_eq!(state.read(cx).value(), "# ab\ncd");
        window.press("secondary-z", cx);
        let after_undo = state.read(cx).value();
        window.press("secondary-z", cx);
        let after_undo2 = state.read(cx).value();
        window.press("ctrl-y", cx);
        let after_redo = state.read(cx).value();
        println!("S1|undo|apres1={after_undo:?} apres2={after_undo2:?} refait={after_redo:?}");
        assert_ne!(after_undo, "# ab\ncd");
        assert_ne!(after_redo, after_undo2);
        // Le titre reste reconnu par le modèle après annulation.
        settle(window, &view, cx);
        let spans = compute(&state.read(cx).value(), 0..0);
        assert!(spans.iter().any(|s| s.kind == Kind::HeadingMarker) || after_undo2.is_empty());
    })
    .unwrap();
}

/// Point 7 : `coords_at_pos` / `pos_at_coords` (contrat `TextSurface`). Il n'existe pas d'API
/// publique `pos_at_coords` (`index_for_mouse_position` est `pub(crate)`) : on la reconstruit
/// par dichotomie sur `range_to_bounds` (publique) et on la compare au clic réel.
#[gpui_kit::test]
fn coordonnees_et_positions(cx: &mut TestAppContext) {
    let text: String = (0..12)
        .map(|i| format!("ligne {i} abcdefghij"))
        .collect::<Vec<_>>()
        .join("\n");
    let (handle, view, state) = open(cx, &text, 800., 480.);
    cx.update_window(handle.into(), |_, window, cx| {
        settle(window, &view, cx);
        let lh = state.read(cx).line_height().unwrap();
        let editor = window.find(("input", state.entity_id())).bounds();
        let mut checked = 0;
        for (line, col) in [(1usize, 3usize), (5, 10), (9, 0), (11, 12)] {
            let o = line_start(&text, line) + col;
            let cell = state.read(cx).range_to_bounds(&(o..o + 1)).unwrap();
            // 25 % dans la cellule : la frontière la plus proche est `o`.
            let p = point(
                cell.origin.x + cell.size.width / 4.,
                cell.origin.y + lh / 2.,
            );
            let bisect = pos_at_coords(state.read(cx), text.len(), p, lh);
            window.click_at(("input", state.entity_id()), p - editor.origin, cx);
            let clicked = state.read(cx).cursor();
            assert_eq!(bisect, Some(o), "dichotomie ligne {line} col {col}");
            assert_eq!(clicked, o, "clic ligne {line} col {col}");
            checked += 1;
        }
        let cell = state.read(cx).range_to_bounds(&(0..1)).unwrap();
        println!(
            "S1|coords|points_ok={checked} chasse={:?} hauteur_ligne={lh:?}",
            cell.size.width
        );
    })
    .unwrap();
}

/// `pos_at_coords` par dichotomie : dernier offset dont le caret précède `p` (ordre rangée puis x).
fn pos_at_coords(state: &EditorState, len: usize, p: Point<Pixels>, lh: Pixels) -> Option<usize> {
    let before = |k: usize| -> Option<bool> {
        let c = state.range_to_bounds(&(k..k))?.origin;
        Some(c.y + lh <= p.y || (c.y <= p.y && c.x <= p.x))
    };
    if !before(0)? {
        return None;
    }
    let (mut lo, mut hi) = (0usize, len);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if before(mid)? {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(lo)
}

/// Point 8 : 10 000 lignes. Les durées sont IMPRIMÉES (profil du test : voir le rapport) ;
/// les seuils d'assertion ne sont qu'un garde-fou grossier.
#[gpui_kit::test]
fn dix_mille_lignes(cx: &mut TestAppContext) {
    let mut text = String::new();
    for i in 0..10_000 {
        match i % 10 {
            0 => text.push_str(&format!("# Titre {i}\n")),
            3 => text.push_str(&format!("- [ ] tâche {i} avec **gras** et du texte\n")),
            6 => text.push_str(&format!("| a{i} | b | c |\n")),
            _ => text.push_str(&format!(
                "Ligne {i} de texte lorem ipsum dolor sit amet, consectetur adipiscing\n"
            )),
        }
    }
    let t_model = Instant::now();
    let spans = compute(&text, 0..0);
    let model_ms = ms(t_model);

    let t_open = Instant::now();
    let (handle, view, state) = open(cx, &text, 800., 480.);
    let open_ms = ms(t_open);

    cx.update_window(handle.into(), |_, window, cx| {
        let t = Instant::now();
        window.render_frame(cx);
        let first_frame_ms = ms(t);

        // Décorations de tout le document (c'est ce que fait `refresh`).
        let t = Instant::now();
        view.update(cx, |v, cx| {
            v.invalidate();
            v.refresh(cx);
        });
        window.render_frame(cx);
        let deco_ms = ms(t);

        // Défilement : 100 pas de 3 lignes.
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
        assert!(visible.as_ref().is_some_and(|r| r.start > 100));

        // Frappe au milieu du document : 100 caractères.
        let mid = text.floor_char_boundary(text.len() / 2);
        state.update(cx, |s, cx| s.set_selected_range(mid..mid, cx));
        state.update(cx, |s, cx| s.focus(window, cx));
        let t_type = Instant::now();
        for _ in 0..100 {
            window.input("x", cx);
        }
        let type_mean = ms(t_type) / 100.;

        println!(
            "S1|10k|octets={} spans={} modele_ms={model_ms:.1} ouverture_ms={open_ms:.1} \
             1re_image_ms={first_frame_ms:.1} decorations_ms={deco_ms:.1} \
             defilement_moy_ms={scroll_mean:.2} defilement_max_ms={worst:.2} \
             frappe_moy_ms={type_mean:.2} lignes_visibles={visible:?}",
            text.len(),
            spans.len(),
        );
        assert!(scroll_mean < 2000., "défilement absurdement lent");
    })
    .unwrap();
}
