//! Point d'entrée de Fragment.
//!
//! Usage : `fragment [dossier de travail]`. Le dossier porte `.fragment/` (workspace.json,
//! appearance.json, hotkeys.json) ; c'est le coffre. Sans argument : le coffre mémorisé, sinon le
//! gestionnaire de coffres.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::rc::Rc;

use fragment_app::boot::Options;
use fragment_app::coffre;
use fragment_app::stockage::Stockages;

fn main() {
    // Un dossier en argument est un choix explicite ; sans argument, le coffre mémorisé, ou le
    // gestionnaire de coffres seul (premier lancement, coffre disparu : fr-t2-gestionnaire).
    let impose = std::env::args_os()
        .nth(1)
        .map(|d| PathBuf::from(d).to_string_lossy().into_owned());

    gpui_kit::application()
        .with_assets(fragment_ui::icones::Ressources)
        .run(move |cx| {
            gpui_kit::init(cx);
            if let Err(erreur) = fragment_ui::polices::enregistrer(cx.text_system()) {
                eprintln!("fragment : polices : {erreur:#}");
            }
            fragment_app::identite::signaler_replis(cx.text_system());
            // Plus aucune fenêtre (le gestionnaire fermé sans choix) : l'application s'arrête.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let lanceur: coffre::Lanceur = Rc::new(lancer_app);
            let registre = fragment_cli::coffres::RegistreCoffres::new(coffre::dossier_user_data());
            coffre::ChoixCoffre::new(registre, lanceur).demarrer(cx, impose.as_deref());
        });
}

/// Démarre l'application sur le coffre `dossier` ; rend son Keymap.
fn lancer_app(cx: &mut gpui_kit::App, dossier: &str) -> Option<fragment_core::Keymap> {
    let dossier = PathBuf::from(dossier);
    let mut options = Options::avec(Stockages::dossier(&dossier), env!("CARGO_PKG_VERSION"));
    options.coffre = Some(coffre::SourceCoffre::Dossier(dossier.clone()));
    options.schema_au_demarrage = true;
    options.nom = dossier.file_name().map_or_else(
        || "Fragment".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let application =
        match fragment_app::monter(cx, &options, fragment_app::LARGEUR, fragment_app::HAUTEUR) {
            Ok(a) => a,
            Err(erreur) => {
                eprintln!("fragment : impossible d'ouvrir la fenêtre : {erreur:#}");
                cx.quit();
                return None;
            }
        };
    let keymap = fragment_core::App::keymap(&*application.noyau.app).clone();
    // Fermer la fenêtre principale (ou la dernière) écrit ce qui attend et quitte.
    let a = Rc::clone(&application);
    cx.on_window_closed(move |cx, fermee| {
        if fermee == a.principale.window_id() || cx.windows().is_empty() {
            a.sauver();
            cx.quit();
        }
    })
    .detach();
    Some(keymap)
}
