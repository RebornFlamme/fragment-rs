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

use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _, Window, div,
    px, rgb,
};

/// Délai laissé à la fenêtre pour poser sa première image stable.
const ATTENTE_STABLE: Duration = Duration::from_millis(1500);

struct Scene {
    nom: String,
    themes: Vec<String>,
    /// Vue à capturer : l'application (défaut) ou la `mire` de contrôle du rendu.
    mire: bool,
    /// Taille attendue de l'image, en pixels physiques (fenêtre × DPR).
    attendu: (u32, u32),
}

/// Mire de contrôle : aplats, trait de 1 px, rayon, texte. Elle prouve que le rasterizeur
/// (WARP sur le runner) dessine vraiment quads et glyphes — une fenêtre vide n'a qu'une couleur.
/// ★ Couleurs littérales assumées : c'est une mire de test, hors thème par nature.
struct Mire;

impl Render for Mire {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .p(px(40.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .w(px(200.0))
                    .h(px(80.0))
                    .bg(rgb(0x7c3aed))
                    .rounded(px(12.0)),
            )
            .child(div().w(px(300.0)).h(px(1.0)).bg(rgb(0xffffff)))
            .child(
                div()
                    .text_color(rgb(0xdcddde))
                    .text_size(px(24.0))
                    .child("Fragment — mire 0123456789"),
            )
    }
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
        let mire = json["vue"].as_str() == Some("mire");
        let dpr = json["window"]["dpr"].as_f64().unwrap_or(1.0);
        let dim =
            |cle: &str, defaut: f64| (json["window"][cle].as_f64().unwrap_or(defaut) * dpr) as u32;
        let attendu = (
            dim("width", f64::from(fragment_app::LARGEUR)),
            dim("height", f64::from(fragment_app::HAUTEUR)),
        );
        scenes.push(Scene {
            nom,
            themes,
            mire,
            attendu,
        });
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
            let fenetre = if scene.mire {
                cx.update(|cx| {
                    gpui_kit::open_window(fragment_app::options_fenetre(cx), cx, |_, cx| {
                        cx.new(|_| Mire)
                    })
                    .map(|(fenetre, _)| fenetre)
                })?
            } else {
                cx.update(fragment_app::ouvrir_fenetre)?
            };
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
            // ★ On NE ferme PAS la fenêtre : fermer la dernière fenêtre fait quitter l'application
            // (code 0) et les scènes suivantes n'étaient jamais capturées (constaté en CI, 01/11).
            // `render_to_image` rend la scène de CETTE fenêtre : le recouvrement n'y change rien.
            // ★ Contrôles : une taille différente rend toute comparaison avec Fragment caduque
            // (écran du runner trop petit → fenêtre rognée) ; une mire unie = rendu absent.
            if image.dimensions() != scene.attendu {
                anyhow::bail!(
                    "{} : image {:?}, attendu {:?} (écran trop petit ?)",
                    scene.nom,
                    image.dimensions(),
                    scene.attendu
                );
            }
            if scene.mire && compter_couleurs(&image) < 3 {
                anyhow::bail!("{} : mire unie, le rasterizeur ne dessine pas", scene.nom);
            }
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
