//! Point d'entrée de Fragment.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        // Fermer la dernière fenêtre quitte l'application.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(erreur) = fragment_app::ouvrir_fenetre(cx) {
            eprintln!("fragment : impossible d'ouvrir la fenêtre : {erreur:#}");
            cx.quit();
        }
    });
}
