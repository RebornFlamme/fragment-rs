//! `fragment-pixels` : rejoue les scènes `pixels/scenes/*.json` et capture le framebuffer.
//!
//! Usage : `fragment-pixels <dossier des scènes> [dossier de sortie = pixels/candidate]`.
//!
//! Phase 0 : seules les scènes SANS action sont rejouées (preuve « fenêtre vide ») ;
//! les autres sont listées comme ignorées. Les actions (commandes, clics, frappes)
//! arrivent avec la tranche T1. La capture passe par `Window::render_to_image`
//! (rendu DirectX dans une texture hors écran puis relecture) : elle ne dépend pas
//! de ce que le compositeur affiche. ★ Sur un runner sans GPU, DirectX tombe sur WARP.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Délai laissé à la fenêtre pour poser sa première image stable.
const ATTENTE_STABLE: Duration = Duration::from_millis(1500);

struct Scene {
    nom: String,
    themes: Vec<String>,
}

fn lire_scenes(dossier: &Path) -> anyhow::Result<Vec<Scene>> {
    let mut scenes = Vec::new();
    let mut chemins: Vec<PathBuf> = std::fs::read_dir(dossier)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    chemins.sort();
    for chemin in chemins {
        let texte = std::fs::read_to_string(&chemin)?;
        let json: serde_json::Value = serde_json::from_str(&texte)?;
        let nom = json["scene"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| chemin.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let sans_action = json["actions"].as_array().is_none_or(|a| a.is_empty());
        if !sans_action {
            println!("ignorée (actions pas encore rejouées) : {nom}");
            continue;
        }
        let themes = json["themes"]
            .as_array()
            .map(|t| {
                t.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_else(|| vec!["dark".to_owned()]);
        scenes.push(Scene { nom, themes });
    }
    Ok(scenes)
}

async fn capturer(
    scenes: Vec<Scene>,
    sortie: PathBuf,
    cx: &mut gpui_kit::AsyncApp,
) -> anyhow::Result<usize> {
    std::fs::create_dir_all(&sortie)?;
    let mut ecrites = 0;
    for scene in scenes {
        for theme in &scene.themes {
            // Phase 0 : un seul thème existe (le défaut de gpui-kit) ;
            // le clair arrive avec fragment-theme (S4/T11).
            if theme != "dark" {
                println!(
                    "ignoré (thème pas encore disponible) : {}-{theme}",
                    scene.nom
                );
                continue;
            }
            let fenetre = cx.update(fragment_app::ouvrir_fenetre)?;
            cx.background_executor().timer(ATTENTE_STABLE).await;
            let image =
                cx.update(|cx| fenetre.update(cx, |_, window, _| window.render_to_image()))??;
            let chemin = sortie.join(format!("{}-{theme}.png", scene.nom));
            image.save(&chemin)?;
            println!(
                "capturée : {} ({}×{}, {} couleurs distinctes)",
                chemin.display(),
                image.width(),
                image.height(),
                compter_couleurs(&image)
            );
            cx.update(|cx| fenetre.update(cx, |_, window, _| window.remove_window()))?;
            ecrites += 1;
        }
    }
    Ok(ecrites)
}

/// Compte les couleurs distinctes (plafonné) : une capture d'une seule couleur
/// trahit un rendu absent (WARP muet), pas une fenêtre vide.
fn compter_couleurs(image: &image::RgbaImage) -> usize {
    let mut vues = std::collections::HashSet::new();
    for pixel in image.pixels() {
        vues.insert(pixel.0);
        if vues.len() > 4096 {
            break;
        }
    }
    vues.len()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scenes_dir = PathBuf::from(args.next().unwrap_or_else(|| "pixels/scenes".into()));
    let sortie = PathBuf::from(args.next().unwrap_or_else(|| "pixels/candidate".into()));
    let scenes = match lire_scenes(&scenes_dir) {
        Ok(s) => s,
        Err(erreur) => {
            eprintln!("fragment-pixels : lecture des scènes : {erreur:#}");
            std::process::exit(2);
        }
    };
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
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
