// Port de app/src/ui/core/editor/rangeHighlights.test.ts (le calque : géométrie, suivi, redessin)
// et de app/e2e/doc-widget-views.spec.ts (le passage actif, plus marqué).
//
//! Tests headless du calque des surlignages, sur une FAUSSE surface de texte : un caractère = 10 px,
//! 10 caractères par ligne visuelle, une ligne = 20 px. Les styles sont ceux des jetons de thème.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::Cell;
use std::rc::Rc;

use fragment_text::{
    Assoc, DocumentSurface, EditorChange, EditorPosition, EditorRange, HighlightRange, MappedPos,
    Rect, TextSurface,
};
use fragment_theme::Theme;
use fragment_ui::surlignages::{CalqueSurlignages, ModeCalque, SousLeTexte};
use gpui_kit::{
    Context, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext, Window, canvas,
    div, point, px, size,
};

const PAR_LIGNE: usize = 10;

#[derive(Default)]
struct Surface {
    pokes: Cell<usize>,
}

impl DocumentSurface for Surface {
    fn coords_at_pos(&self, _offset: usize) -> Option<Rect> {
        None
    }
    fn pos_at_coords(&self, _x: f64, _y: f64) -> Option<usize> {
        None
    }
    fn viewport_range(&self) -> EditorRange {
        EditorRange { from: 0, to: 1000 }
    }
    fn rendered_ranges(&self) -> Vec<EditorRange> {
        vec![EditorRange { from: 0, to: 1000 }]
    }
    fn request_update(&self) {
        self.pokes.set(self.pokes.get() + 1);
    }
    fn as_text(&self) -> Option<&dyn TextSurface> {
        Some(self)
    }
}

impl TextSurface for Surface {
    fn get_line(&self, _n: usize) -> String {
        String::new()
    }
    fn line_count(&self) -> usize {
        100
    }
    fn offset_to_pos(&self, offset: usize) -> EditorPosition {
        EditorPosition {
            line: offset / PAR_LIGNE,
            ch: offset % PAR_LIGNE,
        }
    }
    fn pos_to_offset(&self, pos: EditorPosition) -> usize {
        pos.line * PAR_LIGNE + pos.ch
    }
    fn get_selection(&self) -> EditorRange {
        EditorRange::default()
    }
    /// La forme d'une sélection : première ligne partielle, lignes pleines, dernière partielle.
    fn coords_for_range(&self, from: usize, to: usize) -> Vec<Rect> {
        let (l0, l1) = (from / PAR_LIGNE, (to - 1) / PAR_LIGNE);
        (l0..=l1)
            .map(|l| {
                let debut = if l == l0 { from % PAR_LIGNE } else { 0 };
                let fin = if l == l1 {
                    (to - 1) % PAR_LIGNE + 1
                } else {
                    PAR_LIGNE
                };
                Rect {
                    left: debut as f64 * 10.0,
                    right: fin as f64 * 10.0,
                    top: l as f64 * 20.0,
                    bottom: l as f64 * 20.0 + 20.0,
                }
            })
            .collect()
    }
}

fn inserer(at: usize, n: usize) -> EditorChange {
    EditorChange::doc(move |pos, assoc| MappedPos {
        pos: if pos > at || (pos == at && assoc == Assoc::After) {
            pos + n
        } else {
            pos
        },
        deleted: false,
    })
}

fn supprimer(from: usize, to: usize) -> EditorChange {
    EditorChange::doc(move |pos, _| MappedPos {
        pos: if pos <= from {
            pos
        } else if pos >= to {
            pos - (to - from)
        } else {
            from
        },
        deleted: pos > from && pos < to,
    })
}

fn geometrie(c: &CalqueSurlignages, s: &Surface) -> Vec<(f64, f64, f64, f64)> {
    c.rectangles(s, &Theme::sombre())
        .iter()
        .map(|r| (r.boite.left, r.boite.top, r.boite.width, r.boite.height))
        .collect()
}

#[test]
fn les_rectangles_d_une_plage_sur_deux_lignes_ont_la_forme_d_une_selection() {
    let (s, c) = (Surface::default(), CalqueSurlignages::default());
    c.poser("k", vec![HighlightRange::new(7, 14)]);
    assert_eq!(
        geometrie(&c, &s),
        vec![(70.0, 0.0, 30.0, 20.0), (0.0, 20.0, 40.0, 20.0)]
    );
}

#[test]
fn une_plage_videe_par_une_suppression_est_retiree() {
    let (s, c) = (Surface::default(), CalqueSurlignages::default());
    c.poser("k", vec![HighlightRange::new(12, 16)]);
    c.suivre(&supprimer(10, 20));
    assert!(c.plages("k").is_empty());
    assert!(geometrie(&c, &s).is_empty());
}

#[test]
fn les_rectangles_suivent_une_edition() {
    let (s, c) = (Surface::default(), CalqueSurlignages::default());
    c.poser("k", vec![HighlightRange::new(12, 16)]);
    assert_eq!(geometrie(&c, &s), vec![(20.0, 20.0, 40.0, 20.0)]);
    c.suivre(&inserer(0, 10)); // une ligne de plus au-dessus
    assert_eq!(geometrie(&c, &s), vec![(20.0, 40.0, 40.0, 20.0)]);
    c.suivre(&inserer(24, 2)); // une frappe dedans : la plage s'allonge
    assert_eq!(geometrie(&c, &s), vec![(20.0, 40.0, 60.0, 20.0)]);
}

#[test]
fn le_redessin_est_demande_a_la_surface_apres_un_remappage() {
    let (s, c) = (Surface::default(), CalqueSurlignages::default());
    c.poser("k", vec![HighlightRange::new(2, 5)]);
    assert!(c.livrer(&s), "la pose arme le poke");
    assert!(!c.livrer(&s), "un seul poke par lot");
    c.suivre(&inserer(0, 1));
    assert!(c.livrer(&s), "★ le remappage repoke");
    assert_eq!(s.pokes.get(), 2);
}

#[test]
fn le_passage_actif_est_plus_marque_que_le_passage() {
    let (s, c) = (Surface::default(), CalqueSurlignages::default());
    c.poser("a", vec![HighlightRange::with_cls(0, 4, "is-passage")]);
    c.poser(
        "b",
        vec![HighlightRange::with_cls(20, 24, "is-passage is-active")],
    );
    let t = Theme::sombre();
    let rs = c.rectangles(&s, &t);
    assert_eq!(rs.len(), 2);
    assert!(rs[1].style.fond.a > rs[0].style.fond.a);
    // Les jetons de thème, pas des littéraux : le trait du passage est l'accent 2.
    assert_eq!(rs[0].style.trait_bas, Some(t.palette.color_accent_2));
    assert_eq!(rs[0].style.rayon, t.forme.radius_xxs);
}

#[test]
fn le_survol_a_le_style_d_un_passage_et_le_nu_celui_du_surlignage() {
    let (s, c) = (Surface::default(), CalqueSurlignages::default());
    c.poser("h", vec![HighlightRange::with_cls(0, 4, "is-hover")]);
    c.poser("n", vec![HighlightRange::new(20, 24)]);
    let t = Theme::clair();
    let rs = c.rectangles(&s, &t);
    assert!(rs[0].style.trait_bas.is_some());
    assert!(rs[1].style.trait_bas.is_none());
    assert_eq!(rs[1].style.rayon, t.forme.radius_xs);
}

#[test]
fn retirer_une_pose_remplacee_est_sans_effet() {
    let (s, c) = (Surface::default(), CalqueSurlignages::default());
    let ancienne = c.poser("k", vec![HighlightRange::new(0, 4)]);
    let _nouvelle = c.poser("k", vec![HighlightRange::new(10, 14)]);
    c.retirer(&ancienne);
    assert_eq!(geometrie(&c, &s).len(), 1);
}

#[test]
fn le_calque_est_sous_le_texte_sauf_en_pdf() {
    assert!(!CalqueSurlignages::default().au_dessus());
    assert!(CalqueSurlignages::new(ModeCalque::Multiplie).au_dessus());
}

// ─── Le dessin, dans une fenêtre de test ───

struct Vue {
    surface: Rc<Surface>,
    calque: Rc<CalqueSurlignages>,
    peints: Rc<Cell<usize>>,
}

impl Render for Vue {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (s, c, n) = (
            Rc::clone(&self.surface),
            Rc::clone(&self.calque),
            Rc::clone(&self.peints),
        );
        div().size_full().child(
            canvas(
                |_, _, _| (),
                move |bornes, _, window, cx| {
                    c.peindre_sous_le_texte(
                        &*s,
                        bornes.origin + point(px(0.0), px(0.0)),
                        1.0,
                        window,
                        cx,
                    );
                    n.set(n.get() + 1);
                },
            )
            .size_full(),
        )
    }
}

#[test]
fn le_crochet_peint_sans_erreur() {
    let mut cx = TestAppContext::single();
    let calque = Rc::new(CalqueSurlignages::default());
    calque.poser(
        "k",
        vec![HighlightRange::with_cls(7, 14, "is-passage is-active")],
    );
    let peints = Rc::new(Cell::new(0));
    let (s, c, n) = (
        Rc::new(Surface::default()),
        Rc::clone(&calque),
        Rc::clone(&peints),
    );
    let (_vue, vcx) = cx.add_window_view(move |_, _| Vue {
        surface: s,
        calque: c,
        peints: n,
    });
    vcx.simulate_resize(size(px(400.0), px(300.0)));
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    assert!(peints.get() >= 1);
}
