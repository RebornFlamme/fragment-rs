//! Rastérisation d'un SVG de formule par resvg (tiny-skia), sur le fond et dans le rognage de
//! la capture de référence, pour la comparer pixel à pixel.

use resvg::{tiny_skia, usvg};

use crate::Erreur;
use crate::reference::{Mesure, couleur};

/// Une formule rendue par une voie : un SVG dont `width`/`height` sont en px CSS, et sa ligne
/// de base mesurée depuis le haut (px).
#[derive(Clone, Debug)]
pub struct RenduSvg {
    pub svg: String,
    pub largeur: f64,
    pub hauteur: f64,
    pub ligne_de_base: f64,
}

/// Arrondi de Chromium : une longueur de mise en page est TRONQUÉE à 1/64 px (LayoutUnit).
/// ★ Mesuré sur la capture : 2,282 ex × 8,0639 px = 18,4018 → `getBoundingClientRect` rend
/// 18,390625 (1177/64), pas 18,40625.
pub fn unite_de_mise_en_page(v: f64) -> f64 {
    (v * 64.0).floor() / 64.0
}

/// Ligne de base de la référence (depuis le haut du `<svg>`), d'après son `vertical-align`.
pub fn ligne_de_base_reference(m: &Mesure) -> f64 {
    let profondeur_ex = m
        .svg
        .split("vertical-align:")
        .nth(1)
        .and_then(|s| s.trim_start().split("ex").next())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|v| -v)
        .unwrap_or(0.0);
    m.hauteur - profondeur_ex * m.ex_px
}

/// Où poser le rendu dans le rognage : la boîte EXACTE de la référence (`meme_boite`, voie A)
/// ou la règle de l'application (ligne de base alignée ; à gauche en ligne, centré en bloc).
///
/// ★ Mesuré (exploration au 1/8 de px, `explorer_calage_voie_a`) : Chromium peint le `<svg>`
/// à son origine ARRONDIE au pixel, mais à sa taille de mise en page (non accrochée). L'origine
/// fractionnaire double les pixels au-delà de 16 (12,3 % contre 5,7 %).
pub fn placer(rendu: &RenduSvg, m: &Mesure, meme_boite: bool) -> (f64, f64) {
    let (x, y) = if meme_boite {
        (m.x, m.y)
    } else {
        let x = if m.bloc {
            m.x + (m.largeur - rendu.largeur) / 2.0
        } else {
            m.x
        };
        (x, m.y + ligne_de_base_reference(m) - rendu.ligne_de_base)
    };
    (x.round() - m.clip.x, y.round() - m.clip.y)
}

/// Peint `rendu` en (x, y) sur le fond de la référence, dans une image de la taille du rognage.
pub fn peindre(rendu: &RenduSvg, m: &Mesure, pos: (f64, f64)) -> Result<image::RgbImage, Erreur> {
    peindre_sur_echantillonne(rendu, m, pos, SURECHANTILLONNAGE)
}

/// Facteur de suréchantillonnage retenu : 1, le chemin de production (GPUI peint ses SVG par resvg).
/// Mesuré : 2 divise par deux les pixels au-delà de 16 sans annuler l'écart (S5).
pub const SURECHANTILLONNAGE: u32 = 1;

/// Peint à `k` fois la résolution puis moyenne chaque bloc k × k (filtre boîte, en sRGB comme
/// le mélange de Skia). ★ tiny-skia anticrénèle par suréchantillonnage 4 × (quantifié),
/// Skia dans Chromium par couverture analytique : le suréchantillonnage rapproche les deux.
pub fn peindre_sur_echantillonne(
    rendu: &RenduSvg,
    m: &Mesure,
    (x, y): (f64, f64),
    k: u32,
) -> Result<image::RgbImage, Erreur> {
    let k = k.max(1);
    let (lf, hf) = (m.clip.width as u32, m.clip.height as u32);
    let largeur = lf * k;
    let hauteur = hf * k;
    let fond = couleur(&m.fond)?;
    let mut pixmap = tiny_skia::Pixmap::new(largeur, hauteur)
        .ok_or_else(|| Erreur::Format("pixmap vide".into()))?;
    pixmap.fill(tiny_skia::Color::from_rgba8(fond[0], fond[1], fond[2], 255));
    let options = usvg::Options::default();
    let arbre = usvg::Tree::from_str(&rendu.svg, &options)
        .map_err(|e| Erreur::Format(format!("usvg : {e}")))?;
    // L'arbre a la taille déclarée par width/height ; on l'étire à la boîte voulue (identité
    // quand le SVG porte déjà ses px).
    let taille = arbre.size();
    let sx = rendu.largeur as f32 / taille.width();
    let sy = rendu.hauteur as f32 / taille.height();
    let kf = k as f32;
    let t = tiny_skia::Transform::from_scale(kf, kf)
        .pre_translate(x as f32, y as f32)
        .pre_scale(sx, sy);
    resvg::render(&arbre, t, &mut pixmap.as_mut());
    let mut sortie = image::RgbImage::new(lf, hf);
    let px = pixmap.pixels();
    for yy in 0..hf {
        for xx in 0..lf {
            let mut somme = [0u32; 3];
            for j in 0..k {
                for i in 0..k {
                    // Fond opaque : la couleur prémultipliée est la couleur.
                    let c = px[((yy * k + j) * largeur + xx * k + i) as usize].demultiply();
                    somme[0] += u32::from(c.red());
                    somme[1] += u32::from(c.green());
                    somme[2] += u32::from(c.blue());
                }
            }
            let n = k * k;
            let moy = |v: u32| ((v + n / 2) / n) as u8;
            sortie.put_pixel(
                xx,
                yy,
                image::Rgb([moy(somme[0]), moy(somme[1]), moy(somme[2])]),
            );
        }
    }
    Ok(sortie)
}
