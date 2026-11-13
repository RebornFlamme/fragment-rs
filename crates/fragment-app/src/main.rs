//! Point d'entrée de Fragment.
//!
//! Usage : `fragment [dossier de travail]`. Le dossier porte `.fragment/` (workspace.json,
//! appearance.json, hotkeys.json) ; sans argument, le dossier courant. Ce dossier est le coffre.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::rc::Rc;

use fragment_app::boot::Options;
use fragment_app::stockage::Stockages;

fn main() {
    let dossier = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut options = Options::avec(Stockages::dossier(&dossier), env!("CARGO_PKG_VERSION"));
    options.coffre = Some(fragment_app::coffre::SourceCoffre::Dossier(dossier.clone()));
    options.schema_au_demarrage = true;
    options.nom = dossier.file_name().map_or_else(
        || "Fragment".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );

    gpui_kit::application()
        .with_assets(fragment_ui::icones::Ressources)
        .run(move |cx| {
            gpui_kit::init(cx);
            if let Err(erreur) = fragment_ui::polices::enregistrer(cx.text_system()) {
                eprintln!("fragment : polices : {erreur:#}");
            }
            fragment_app::identite::signaler_replis(cx.text_system());
            let application = match fragment_app::monter(
                cx,
                &options,
                fragment_app::LARGEUR,
                fragment_app::HAUTEUR,
            ) {
                Ok(a) => a,
                Err(erreur) => {
                    eprintln!("fragment : impossible d'ouvrir la fenêtre : {erreur:#}");
                    cx.quit();
                    return;
                }
            };
            // Fermer la fenêtre principale (ou la dernière) écrit ce qui attend et quitte.
            let a = Rc::clone(&application);
            cx.on_window_closed(move |cx, fermee| {
                if fermee == a.principale.window_id() || cx.windows().is_empty() {
                    a.sauver();
                    cx.quit();
                }
            })
            .detach();
        });
}
