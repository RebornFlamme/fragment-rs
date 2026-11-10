//! `fragment-pixels` : rejoue les scènes `pixels/scenes/*.json` et capture le framebuffer.
//!
//! Usage : `fragment-pixels <dossier des scènes> [dossier de sortie = pixels/candidate]`.
//!
//! Le lecteur (schéma, entrées simulées, captures) vit dans `src/pixels/` ; ce binaire lit les
//! scènes, saute celles d'une tranche postérieure (`"depuis": "T3"`, voir `TRANCHE_COURANTE`) et
//! écrit un PNG par scène et par thème. La capture passe par `Window::render_to_image` : elle ne
//! dépend pas de ce que le compositeur affiche.

// ★ Même fichier que la cible de test `pixels` (Cargo.toml) : voir l'en-tête de `pixels/mod.rs`.
#[path = "../pixels/mod.rs"]
mod pixels;

use std::path::PathBuf;

use gpui_kit::AsyncApp;

use pixels::actions::{self, Scene, TRANCHE_COURANTE};

async fn capturer(scenes: Vec<Scene>, sortie: PathBuf, cx: &mut AsyncApp) -> anyhow::Result<usize> {
    let mut ecrites = 0;
    for scene in scenes {
        if scene.en_attente_de_tranche(TRANCHE_COURANTE) {
            println!(
                "en attente de sa tranche ({}) : {}",
                scene.depuis.map_or(0, |t| t.0),
                scene.nom
            );
            continue;
        }
        for &theme in &scene.themes {
            let image = pixels::jouer(&scene, theme, cx).await?;
            pixels::capture::ecrire(&image, &sortie, &scene.nom, theme.suffixe())?;
            ecrites += 1;
        }
    }
    Ok(ecrites)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scenes_dir = PathBuf::from(args.next().unwrap_or_else(|| "pixels/scenes".into()));
    let sortie = PathBuf::from(args.next().unwrap_or_else(|| "pixels/candidate".into()));
    let scenes = match actions::lire_scenes(&scenes_dir) {
        Ok(s) => s,
        Err(erreur) => {
            eprintln!("fragment-pixels : lecture des scènes : {erreur}");
            std::process::exit(2);
        }
    };
    // Les icônes (SVG de Lucide) sont des ressources embarquées de fragment-ui.
    gpui_kit::application()
        .with_assets(fragment_ui::icones::Ressources)
        .run(move |cx| {
            gpui_kit::init(cx);
            if let Err(erreur) = fragment_ui::polices::enregistrer(&cx.text_system()) {
                eprintln!("fragment-pixels : polices : {erreur:#}");
                std::process::exit(1);
            }
            cx.spawn(async move |cx| {
                // ★ On sort par `process::exit` depuis la tâche : selon la plateforme,
                // `run` ne rend pas toujours la main après `quit`.
                match capturer(scenes, sortie, cx).await {
                    Ok(n) if n > 0 => {
                        println!("fragment-pixels : {n} capture(s)");
                        std::process::exit(0);
                    }
                    Ok(_) => {
                        eprintln!("fragment-pixels : aucune capture");
                        std::process::exit(1);
                    }
                    Err(erreur) => {
                        eprintln!("fragment-pixels : {erreur:#}");
                        std::process::exit(1);
                    }
                }
            })
            .detach();
        });
}
