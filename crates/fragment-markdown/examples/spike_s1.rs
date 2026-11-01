//! Spike S1 : un éditeur Markdown live preview minimal, à lancer à la main :
//!
//! `cargo run -p fragment-markdown --example spike_s1`   (Windows)
//!
//! À essayer : placer le curseur sur le titre (le `# ` apparaît), dans le `**gras**`
//! (les `**` apparaissent), cliquer la case `[ ]`, cliquer sous le « tableau » (le curseur
//! tombe sur la bonne ligne), écrire une longue ligne (soft wrap), Ctrl+Z / Ctrl+Y, taper en
//! IME (japonais…). Constat à regarder : `# ` caché laisse un TROU (glyphes transparents),
//! les titres ne sont pas plus grands, le « tableau » est une plaque de la hauteur de ses
//! lignes sources — voir `docs/spikes/S1-editeur.md`.
#[cfg(windows)]
#[path = "../tests/spike_s1/live.rs"]
mod live;

#[cfg(windows)]
const SAMPLE: &str = "# Spike S1\n\nUn paragraphe avec du **gras** et une très longue phrase qui doit passer à la ligne quand la fenêtre est étroite, pour juger le soft wrap et le curseur.\n\n- [ ] une case à cocher inline\n- [x] une autre\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nTexte sous le tableau.\n";

#[cfg(windows)]
fn run() {
    use gpui_kit::{AppContext, component::input::EditorState};
    use live::LiveView;

    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let opened = gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let state = cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("markdown")
                    .line_number(false)
                    .auto_close(false)
                    .smart_indent(false)
                    .default_value(SAMPLE)
            });
            let view = cx.new(|cx| LiveView::new(state, cx));
            let start = SAMPLE.find("| a |").unwrap_or(0);
            let end = SAMPLE
                .find("| 1 | 2 |")
                .map_or(start, |i| i + "| 1 | 2 |".len());
            view.update(cx, |v, _| v.block = Some(start..end));
            view
        });
        if let Err(error) = opened {
            eprintln!("spike S1 : impossible d'ouvrir la fenêtre : {error:#}");
            cx.quit();
        }
    });
}

fn main() {
    #[cfg(windows)]
    run();
    #[cfg(not(windows))]
    eprintln!("spike S1 : exemple prévu pour Windows (GPUI).");
}
