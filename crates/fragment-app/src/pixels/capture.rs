//! La capture d'une fenêtre et ses contrôles.
//!
//! Elle passe par `Window::render_to_image` (rendu DirectX dans une texture hors écran puis
//! relecture) : elle ne dépend pas de ce que le compositeur affiche. ★ Sur un runner sans GPU,
//! DirectX tombe sur WARP.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;

use fragment_ui::flottant;
use gpui_kit::{AnyWindowHandle, AsyncApp};
use image::RgbaImage;

use super::actions::{PixelAttendu, Scene};

// ★ Les boîtes de texte et d'icônes (ADR-009) : le fichier est celui de `xtask` (même code des deux
// côtés, voir son en-tête). Le candidat les déclare ; `cargo xtask pixels` les relit.
#[path = "../../../../xtask/src/pixels/boites.rs"]
mod boites;

thread_local! {
    /// Les surfaces ombrées de la dernière capture, déclarées par la mise en page pendant SA peinture.
    static OMBRES: RefCell<Vec<boites::OmbreDeclaree>> = const { RefCell::new(Vec::new()) };
    /// Les boîtes arrondies de la dernière capture, déclarées de même.
    static ARRONDIS: RefCell<Vec<boites::ArrondiDeclare>> = const { RefCell::new(Vec::new()) };
    /// Les sprites de glyphes et d'icônes de la dernière capture, lus dans SA scène peinte
    /// (`None` : scène illisible, sans la feature `pixels` ; les boîtes sont alors déduites).
    static SPRITES: RefCell<Option<Vec<boites::BoiteEncre>>> = const { RefCell::new(None) };
}

/// Peint une image fraîche puis la relit : la capture montre l'état APRÈS la dernière entrée.
///
/// ★ Le registre des ombres et des arrondis de `fragment-ui` est vidé juste avant CETTE peinture et relu juste
/// après, dans le même `update` : les autres fenêtres (jamais fermées) peuvent repeindre ensuite
/// sans y ajouter leurs surfaces.
pub fn capturer(fenetre: AnyWindowHandle, cx: &mut AsyncApp) -> anyhow::Result<RgbaImage> {
    cx.update(|cx| {
        fenetre.update(cx, |_, window, cx| {
            flottant::vider_ombres();
            fragment_ui::boites::vider_icones();
            window.draw(cx).clear(cx);
            let ombres = flottant::ombres_peintes().iter().map(declarer).collect();
            OMBRES.with(|o| *o.borrow_mut() = ombres);
            let arrondis = flottant::arrondis_peints()
                .iter()
                .map(declarer_arrondi)
                .collect();
            ARRONDIS.with(|a| *a.borrow_mut() = arrondis);
            let image = window.render_to_image()?;
            let sprites = sprites_peints(window, image.width(), image.height()).map(|mut s| {
                s.extend(icones_declarees(image.width(), image.height()));
                s
            });
            SPRITES.with(|s| *s.borrow_mut() = sprites);
            Ok(image)
        })
    })?
}

/// Les glyphes et les icônes de la scène que `render_to_image` vient de rastériser (fr-p3.1).
///
/// ★ Pourquoi la scène et pas un crochet dans `fragment-ui` : le texte passe par des dizaines de
/// `div().child(chaîne)` et l'éditeur peint ses lignes lui-même ; aucun point de `fragment-ui` ne les
/// voit tous. La scène, si : chaque glyphe y est un sprite posé au pixel près par `paint_glyph`, chaque
/// icône un sprite de `paint_svg`. C'est la vérité de la peinture, pas une estimation de mise en page.
#[cfg(feature = "pixels")]
fn sprites_peints(
    window: &gpui_kit::Window,
    largeur: u32,
    hauteur: u32,
) -> Option<Vec<boites::BoiteEncre>> {
    let scene = window.rendered_scene();
    let mut sprites = Vec::new();
    let glyphes = scene.subpixel_sprites.iter().map(|s| {
        (
            s.order,
            s.tile.bounds.size,
            s.bounds,
            s.content_mask.bounds,
            s.color.a,
        )
    });
    let monochromes = scene.monochrome_sprites.iter().map(|s| {
        (
            s.order,
            s.tile.bounds.size,
            s.bounds,
            s.content_mask.bounds,
            s.color.a,
        )
    });
    for (order, tuile, bornes, masque, alpha) in glyphes.chain(monochromes) {
        if alpha <= 0.0 {
            continue;
        }
        // ★ Glyphe ou icône : `paint_glyph` pose la tuile à sa taille (bornes = tuile), `paint_svg`
        // la rastérise à `SMOOTH_SVG_SCALE_FACTOR` (2) fois la taille affichée. Une tuile plus large
        // que le sprite est donc un SVG.
        let categorie = if tuile.width.0 as f32 > bornes.size.width.0 + 0.5 {
            boites::Categorie::Icone
        } else {
            boites::Categorie::Texte
        };
        let x0 = bornes.origin.x.0.max(masque.origin.x.0).max(0.0).floor();
        let y0 = bornes.origin.y.0.max(masque.origin.y.0).max(0.0).floor();
        let x1 = (bornes.origin.x.0 + bornes.size.width.0)
            .min(masque.origin.x.0 + masque.size.width.0)
            .min(largeur as f32)
            .ceil();
        let y1 = (bornes.origin.y.0 + bornes.size.height.0)
            .min(masque.origin.y.0 + masque.size.height.0)
            .min(hauteur as f32)
            .ceil();
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let rect = (x0, y0, x1, y1);
        // Un sprite entièrement recouvert par un aplat opaque peint APRÈS lui (ordre de dessin plus
        // grand) n'est pas visible : sa boîte tolérerait l'aplat. Il n'est pas déclaré.
        let couvert = scene.quads.iter().any(|q| {
            q.order > order
                && q.background.as_solid().is_some_and(|c| c.a >= 1.0)
                && q.corner_radii.top_left.0 == 0.0
                && q.corner_radii.top_right.0 == 0.0
                && q.corner_radii.bottom_left.0 == 0.0
                && q.corner_radii.bottom_right.0 == 0.0
                && {
                    let b = q.bounds.intersect(&q.content_mask.bounds);
                    b.origin.x.0 <= rect.0
                        && b.origin.y.0 <= rect.1
                        && b.origin.x.0 + b.size.width.0 >= rect.2
                        && b.origin.y.0 + b.size.height.0 >= rect.3
                }
        });
        if couvert {
            continue;
        }
        sprites.push(boites::BoiteEncre {
            categorie,
            x: x0 as u32,
            y: y0 as u32,
            largeur: (x1 - x0) as u32,
            hauteur: (y1 - y0) as u32,
        });
    }
    Some(sprites)
}

/// Les icônes peintes à la main (quads), déclarées par leur dessin pendant la peinture : la scène
/// ne les montre pas comme des sprites (`fragment_ui::boites`, le « − » des boutons de fenêtre).
fn icones_declarees(largeur: u32, hauteur: u32) -> Vec<boites::BoiteEncre> {
    fragment_ui::boites::icones_peintes()
        .into_iter()
        .filter_map(|i| {
            let (x0, y0) = (i.x.max(0.0).floor(), i.y.max(0.0).floor());
            let x1 = (i.x + i.largeur).min(largeur as f32).ceil();
            let y1 = (i.y + i.hauteur).min(hauteur as f32).ceil();
            (x1 > x0 && y1 > y0).then_some(boites::BoiteEncre {
                categorie: boites::Categorie::Icone,
                x: x0 as u32,
                y: y0 as u32,
                largeur: (x1 - x0) as u32,
                hauteur: (y1 - y0) as u32,
            })
        })
        .collect()
}

/// Sans la feature `pixels` (cible de test du lecteur), la scène peinte n'est pas lisible.
#[cfg(not(feature = "pixels"))]
fn sprites_peints(
    _window: &gpui_kit::Window,
    _largeur: u32,
    _hauteur: u32,
) -> Option<Vec<boites::BoiteEncre>> {
    None
}

/// Une surface ombrée de `fragment-ui` dans le format du fichier de déclaration.
fn declarer(o: &flottant::OmbrePeinte) -> boites::OmbreDeclaree {
    boites::OmbreDeclaree {
        x: f64::from(o.x),
        y: f64::from(o.y),
        largeur: f64::from(o.largeur),
        hauteur: f64::from(o.hauteur),
        rayon: f64::from(o.rayon),
        couches: o
            .couches
            .iter()
            .map(|c| boites::CoucheDeclaree {
                decalage_x: f64::from(c.decalage_x),
                decalage_y: f64::from(c.decalage_y),
                sigma: f64::from(c.sigma),
                etendue: f64::from(c.etendue),
            })
            .collect(),
        jeton: o.jeton.to_owned(),
    }
}

/// Une boîte arrondie de `fragment-ui` dans le format du fichier de déclaration.
fn declarer_arrondi(a: &flottant::ArrondiPeint) -> boites::ArrondiDeclare {
    boites::ArrondiDeclare {
        x: f64::from(a.x),
        y: f64::from(a.y),
        largeur: f64::from(a.largeur),
        hauteur: f64::from(a.hauteur),
        rayon: f64::from(a.rayon),
        bordure: f64::from(a.bordure),
        coins: a.coins,
        element: a.element.to_owned(),
    }
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
/// de texte et d'icônes de CETTE capture (catégorie et rectangle), lues dans sa scène peinte
/// (fr-p3.1) ; jamais dans la référence (ADR-009). Sans scène lisible, elles sont déduites de
/// l'image candidate seule (repli).
pub fn ecrire(image: &RgbaImage, dossier: &Path, nom: &str, theme: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(dossier)?;
    let chemin = dossier.join(format!("{nom}-{theme}.png"));
    image.save(&chemin)?;
    let sprites = SPRITES.with(|s| s.borrow().clone());
    let (boites, glyphes, source) = match sprites {
        Some(sprites) => {
            let glyphes: Vec<_> = sprites
                .iter()
                .filter(|s| s.categorie == boites::Categorie::Texte)
                .copied()
                .collect();
            (boites::regrouper(&sprites), glyphes, "scène peinte")
        }
        None => (boites::detecter(image), Vec::new(), "déduites de l'image"),
    };
    let ombres = OMBRES.with(|o| o.borrow().clone());
    let arrondis = ARRONDIS.with(|a| a.borrow().clone());
    std::fs::write(
        dossier.join(format!("{nom}-{theme}.boites.json")),
        boites::serialiser_tout(&boites, &glyphes, &ombres, &arrondis),
    )?;
    println!(
        "capturée : {} ({}×{}, {} couleurs distinctes, {} boîtes de texte ou d'icônes ({source}, {} glyphes), {} surfaces ombrées, {} boîtes arrondies)",
        chemin.display(),
        image.width(),
        image.height(),
        compter_couleurs(image),
        boites.len(),
        glyphes.len(),
        ombres.len(),
        arrondis.len()
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
