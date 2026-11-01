//! Spike S1 : modèle minimal du live preview et vue qui l'habille sur l'éditeur de gpui-kit.
//!
//! Partagé par `tests/spike_s1.rs` et `examples/spike_s1.rs` (d'où `#[path]`).
//! Port d'esprit (pas de ligne) de `app/src/ui/views/markdown/decorationPlugin.ts` :
//! la règle `touches(from, to)` = `cursorFrom <= to && from <= cursorTo`, appliquée
//! au nœud entier (la ligne pour un titre, l'étendue `**…**` pour le gras).
#![allow(dead_code)]

use std::ops::Range;

use gpui_kit::{
    Context, Entity, FontWeight, HighlightStyle, Subscription, Window,
    component::input::{Editor, EditorState, TextDecoration, TextDecorationCollection},
    div, hsla,
    prelude::*,
    px, rgb,
    test::TestSupportExt,
};

/// Nature d'un morceau de source reconnu par le modèle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `# ` en tête de titre.
    HeadingMarker,
    /// Le texte du titre.
    HeadingText,
    /// Un `**`.
    StrongMarker,
    /// Le texte entre `**` et `**`.
    StrongText,
    /// Le `[ ]` / `[x]` d'une tâche (remplacé par un widget).
    TaskBox { checked: bool },
}

/// Un morceau de source. `range` est en OCTETS UTF-8 (l'unité de l'éditeur de gpui-kit ;
/// la frontière de contrat est en UTF-16, ADR-004).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub kind: Kind,
    /// Vrai si la source doit être cachée (curseur hors du nœud).
    pub concealed: bool,
}

/// Calcule les morceaux du document pour un curseur donné (règle `curseurActif`).
pub fn compute(text: &str, cursor: Range<usize>) -> Vec<Span> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for line in text.split('\n') {
        let end = start + line.len();
        let touched = cursor.start <= end && start <= cursor.end;
        heading(line, start, end, touched, &mut out);
        task(line, start, &mut out);
        strong(line, start, &cursor, &mut out);
        start = end + 1;
    }
    out
}

fn heading(line: &str, start: usize, end: usize, touched: bool, out: &mut Vec<Span>) {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ') {
        let text_start = start + hashes + 1;
        out.push(Span {
            range: start..text_start,
            kind: Kind::HeadingMarker,
            concealed: !touched,
        });
        if end > text_start {
            out.push(Span {
                range: text_start..end,
                kind: Kind::HeadingText,
                concealed: false,
            });
        }
    }
}

fn task(line: &str, start: usize, out: &mut Vec<Span>) {
    let b = line.as_bytes();
    if b.len() >= 6
        && &b[..3] == b"- ["
        && (b[3] == b' ' || b[3] == b'x')
        && b[4] == b']'
        && b[5] == b' '
    {
        out.push(Span {
            range: start + 2..start + 5,
            kind: Kind::TaskBox {
                checked: b[3] == b'x',
            },
            concealed: true,
        });
    }
}

fn strong(line: &str, start: usize, cursor: &Range<usize>, out: &mut Vec<Span>) {
    let mut from = 0usize;
    while let Some(a) = line[from..].find("**") {
        let open = from + a;
        let Some(b) = line[open + 2..].find("**") else {
            break;
        };
        let close = open + 2 + b;
        if close > open + 2 {
            let (os, cs) = (start + open, start + close);
            let touched = cursor.start <= cs + 2 && os <= cursor.end;
            out.push(Span {
                range: os..os + 2,
                kind: Kind::StrongMarker,
                concealed: !touched,
            });
            out.push(Span {
                range: os + 2..cs,
                kind: Kind::StrongText,
                concealed: false,
            });
            out.push(Span {
                range: cs..cs + 2,
                kind: Kind::StrongMarker,
                concealed: !touched,
            });
        }
        from = close + 2;
    }
}

/// Décorations « glyphes transparents » : la seule façon de « cacher » dans l'éditeur stock.
/// ★ Une décoration est un `HighlightStyle` (couleur, graisse, fond…) : elle ne change JAMAIS
/// la mise en page, le texte caché garde sa largeur.
pub fn conceal_decorations(spans: &[Span]) -> Vec<TextDecoration> {
    spans
        .iter()
        .filter(|s| s.concealed)
        .map(|s| {
            TextDecoration::new(
                s.range.clone(),
                HighlightStyle {
                    color: Some(hsla(0., 0., 0., 0.)),
                    ..Default::default()
                },
            )
        })
        .collect()
}

/// Décorations de style (gras des titres et du `**gras**`).
pub fn style_decorations(spans: &[Span]) -> Vec<TextDecoration> {
    spans
        .iter()
        .filter(|s| matches!(s.kind, Kind::HeadingText | Kind::StrongText))
        .map(|s| {
            TextDecoration::new(
                s.range.clone(),
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
            )
        })
        .collect()
}

/// L'éditeur de gpui-kit habillé par le modèle, plus deux calques posés PAR-DESSUS :
/// la case à cocher inline et un widget de bloc (tableau).
pub struct LiveView {
    pub state: Entity<EditorState>,
    /// Marqueurs cachés (glyphes transparents).
    pub conceal: TextDecorationCollection,
    style: TextDecorationCollection,
    last: Vec<Span>,
    /// `[ ]` / `[x]` de la première tâche du document.
    pub task_box: Option<Range<usize>>,
    /// Octets couverts par un widget de bloc (lignes d'un tableau), s'il y en a un.
    pub block: Option<Range<usize>>,
    /// Journal des clics reçus par les widgets.
    pub log: Vec<&'static str>,
    _observe: Subscription,
}

impl LiveView {
    pub fn new(state: Entity<EditorState>, cx: &mut Context<Self>) -> Self {
        let conceal = state.update(cx, |s, cx| s.create_decorations_collection(Vec::new(), cx));
        let style = state.update(cx, |s, cx| s.create_decorations_collection(Vec::new(), cx));
        // ★ Pas d'événement « sélection changée » dans InputEvent : on observe l'entité entière
        // (chaque déplacement du curseur la notifie).
        let observe = cx.observe(&state, |this, _, cx| {
            this.refresh(cx);
            cx.notify();
        });
        let mut this = Self {
            state,
            conceal,
            style,
            last: Vec::new(),
            task_box: None,
            block: None,
            log: Vec::new(),
            _observe: observe,
        };
        this.refresh(cx);
        this
    }

    /// Oublie le dernier calcul (force la réécriture complète au prochain `refresh`).
    pub fn invalidate(&mut self) {
        self.last.clear();
    }

    /// Recalcule le modèle et réécrit les décorations SI elles ont changé.
    /// ★ `TextDecorationCollection::set` notifie TOUJOURS l'éditeur : sans cette garde,
    /// l'observation ci-dessus boucle à l'infini.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let (text, selection) = {
            let s = self.state.read(cx);
            (s.value(), s.selected_range())
        };
        let spans = compute(&text, selection);
        if spans == self.last {
            return;
        }
        self.task_box = spans
            .iter()
            .find(|s| matches!(s.kind, Kind::TaskBox { .. }))
            .map(|s| s.range.clone());
        self.conceal.set(conceal_decorations(&spans), cx);
        self.style.set(style_decorations(&spans), cx);
        self.last = spans;
    }

    /// Bascule `[ ]` ↔ `[x]` : UNE édition du document, donc un pas d'annulation.
    pub fn toggle_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(range) = self.task_box.clone() else {
            return;
        };
        self.log.push("task");
        let checked = self.state.read(cx).value().get(range.clone()) == Some("[x]");
        self.state.update(cx, |s, cx| {
            s.set_selected_range(range, cx);
            s.replace(if checked { "[ ]" } else { "[x]" }, window, cx);
        });
    }
}

impl Render for LiveView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Géométrie de la DERNIÈRE image (un cadre de retard : voir le rapport).
        let (task, checked, block, text_x) = {
            let s = self.state.read(cx);
            let task = self.task_box.as_ref().and_then(|r| s.range_to_bounds(r));
            let checked = self
                .task_box
                .as_ref()
                .is_some_and(|r| s.value().get(r.clone()) == Some("[x]"));
            let block = self.block.as_ref().and_then(|r| s.range_to_bounds(r));
            let text_x = s.text_bounds().map(|b| b.origin.x);
            (task, checked, block, text_x)
        };

        let mut root = div().relative().size_full().child(
            Editor::new(&self.state)
                .appearance(false)
                .bordered(false)
                .size_full(),
        );

        if let Some(b) = task {
            root = root.child(
                div()
                    .id("task-box")
                    .absolute()
                    .left(b.origin.x)
                    .top(b.origin.y)
                    .w(b.size.width)
                    .h(b.size.height)
                    .border_1()
                    .border_color(rgb(0x7c8cff))
                    .rounded_sm()
                    .bg(if checked {
                        rgb(0x7c8cff)
                    } else {
                        rgb(0x14161c)
                    })
                    .occlude()
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_task(window, cx)))
                    .test_support(),
            );
        }

        if let (Some(b), Some(x)) = (block, text_x) {
            root = root.child(
                div()
                    .id("block-table")
                    .absolute()
                    .left(x)
                    .top(b.origin.y)
                    .w(px(320.))
                    .h(b.size.height)
                    .border_1()
                    .border_color(rgb(0x3a3f4b))
                    .bg(rgb(0x1c1f26))
                    .occlude()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.log.push("table");
                        cx.notify();
                    }))
                    .test_support(),
            );
        }

        root
    }
}
