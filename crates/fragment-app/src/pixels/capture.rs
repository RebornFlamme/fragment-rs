//! La capture d'une fenêtre et ses contrôles.
//!
//! Elle passe par `Window::render_to_image` (rendu DirectX dans une texture hors écran puis
//! relecture) : elle ne dépend pas de ce que le compositeur affiche. ★ Sur un runner sans GPU,
//! DirectX tombe sur WARP.

use std::collections::HashSet;
use std::path::Path;

use gpui_kit::{AnyWindowHandle, AsyncApp};
use image::RgbaImage;

use super::actions::{PixelAttendu, Scene};

// ★ Les boîtes de texte et d'icônes (ADR-009) : le fichier est celui de `xtask` (même code des deux
// côtés, voir son en-tête). Le candidat les déclare ; `cargo xtask pixels` les relit.
#[path = "../../../../xtask/src/pixels/boites.rs"]
mod boites;

/// Peint une image fraîche puis la relit : la capture montre l'état APRÈS la dernière entrée.
pub fn capturer(fenetre: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<RgbaImage> {
    cx.update(|cx| {
        fenetre.update(cx, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.render_to_image()
        })
    })?
}

/// Compte les couleurs distinctes (plafonné) : une capture d'une seule couleur trahit un rendu
/// absent (WARP muet), pas une fenêtre vide.
pub fn compter_couleurs(image: &RgbaImage) -> usize {
    let mut vues = HashSet::new();
    for pixel in image.pixels() {
        vues.insert(pixel.0);
        if vues.len() > 4096 {
            break;
        }
    }
    vues.len()
}

/// Les contrôles d'une capture :
/// - la taille est celle de la scène (sinon toute comparaison avec Fragment est caduque : écran
///   du runner trop petit, fenêtre rognée) ;
/// - une mire unie = rasterizeur absent ;
/// - les `pixels_attendus` de la scène (elle s'auto-contrôle).
pub fn verifier(image: &RgbaImage, scene: &Scene, mire: bool) -> anyhow::Result<()> {
    let attendu = scene.taille_capture().physique();
    if image.dimensions() != attendu {
        anyhow::bail!(
            "{} : image {:?}, attendu {:?} (écran trop petit ?)",
            scene.nom,
            image.dimensions(),
            attendu
        );
    }
    if mire && compter_couleurs(image) < 3 {
        anyhow::bail!("{} : mire unie, le rasterizeur ne dessine pas", scene.nom);
    }
    verifier_pixels(image, &scene.nom, &scene.pixels_attendus)
}

/// Chaque pixel attendu doit porter exactement sa couleur.
pub fn verifier_pixels(
    image: &RgbaImage,
    nom: &str,
    attendus: &[PixelAttendu],
) -> anyhow::Result<()> {
    for p in attendus {
        if p.x >= image.width() || p.y >= image.height() {
            anyhow::bail!("{nom} : pixel attendu ({}, {}) hors de l'image", p.x, p.y);
        }
        let [r, g, b, _] = image.get_pixel(p.x, p.y).0;
        if [r, g, b] != p.rgb {
            anyhow::bail!(
                "{nom} : pixel ({}, {}) = {:?}, attendu {:?}",
                p.x,
                p.y,
                [r, g, b],
                p.rgb
            );
        }
    }
    Ok(())
}

/// Écrit le PNG `<dossier>/<scène>-<thème>.png` et, à côté, `<scène>-<thème>.boites.json` : les boîtes
/// de texte et d'icônes de CETTE capture (catégorie et rectangle), dérivées de l'image candidate
/// seule, jamais de la référence (ADR-009).
pub fn ecrire(image: &RgbaImage, dossier: &Path, nom: &str, theme: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(dossier)?;
    let chemin = dossier.join(format!("{nom}-{theme}.png"));
    image.save(&chemin)?;
    let boites = boites::detecter(image);
    std::fs::write(
        dossier.join(format!("{nom}-{theme}.boites.json")),
        boites::serialiser(&boites),
    )?;
    println!(
        "capturée : {} ({}×{}, {} couleurs distinctes, {} boîtes de texte ou d'icônes)",
        chemin.display(),
        image.width(),
        image.height(),
        compter_couleurs(image),
        boites.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn un_pixel_attendu_est_controle() {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        img.put_pixel(2, 1, Rgba([245, 158, 11, 255]));
        let ok = PixelAttendu {
            x: 2,
            y: 1,
            rgb: [245, 158, 11],
        };
        assert!(verifier_pixels(&img, "s", &[ok]).is_ok());
        let faux = PixelAttendu {
            rgb: [0, 0, 0],
            ..ok
        };
        let e = verifier_pixels(&img, "s", &[faux]).unwrap_err().to_string();
        assert!(e.contains("(2, 1)"), "{e}");
        let dehors = PixelAttendu { x: 9, ..ok };
        assert!(verifier_pixels(&img, "s", &[dehors]).is_err());
    }

    #[test]
    fn les_couleurs_distinctes_se_comptent() {
        let mut img = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        assert_eq!(compter_couleurs(&img), 1);
        img.put_pixel(0, 0, Rgba([9, 9, 9, 255]));
        assert_eq!(compter_couleurs(&img), 2);
    }
}
