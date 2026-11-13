//! Le SVG du HTML du Markdown, peint par le rastériseur de GPUI (`SvgRenderer`, ADR-012 §3).
//!
//! L'analyseur SVG ne connaît pas `var(--…)` : les variables du thème et `currentColor` sont
//! substituées dans l'arbre par les couleurs des jetons AVANT la rastérisation. Un SVG animé est figé
//! à l'instant demandé ([`super::smil`]).

use std::sync::Arc;

use fragment_theme::{Couleur, Theme};
use gpui_kit::{RenderImage, SvgRenderer};

use super::arbre::{Element, Noeud, serialiser};
use super::css;
use super::smil;

/// ★ `SvgRenderer` rastérise au DOUBLE de l'échelle demandée (suréchantillonnage de GPUI, mesuré par
/// S6) : l'échelle 0,5 rend la taille déclarée au pixel près, à densité 1.
const ECHELLE_UNITE: f32 = 0.5;

fn texte_couleur(c: Couleur) -> String {
    if c.a == 255 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else {
        format!(
            "rgba({}, {}, {}, {:.3})",
            c.r,
            c.g,
            c.b,
            f32::from(c.a) / 255.0
        )
    }
}

/// Remplace dans `v` chaque `var(--x)` (et `var(--x, repli)`) de couleur et `currentColor`.
#[must_use]
pub fn substituer(theme: &Theme, v: &str) -> String {
    let mut sortie = String::with_capacity(v.len());
    let mut reste = v;
    while let Some(i) = reste.find("var(") {
        sortie.push_str(&reste[..i]);
        let apres = &reste[i..];
        // La parenthèse fermante qui équilibre `var(` (un repli peut contenir `rgb(…)`).
        let mut prof = 0usize;
        let mut fin = None;
        for (k, c) in apres.char_indices() {
            match c {
                '(' => prof += 1,
                ')' => {
                    prof -= 1;
                    if prof == 0 {
                        fin = Some(k + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(fin) = fin else {
            sortie.push_str(apres);
            reste = "";
            break;
        };
        let expr = &apres[..fin];
        match css::couleur(theme, expr) {
            Some(c) => sortie.push_str(&texte_couleur(c)),
            None => sortie.push_str(expr),
        }
        reste = &apres[fin..];
    }
    sortie.push_str(reste);
    let courant = texte_couleur(theme.palette.text_normal);
    sortie
        .replace("currentColor", &courant)
        .replace("currentcolor", &courant)
}

/// Le SVG habillé aux couleurs du thème (attributs et `style`).
#[must_use]
pub fn habiller(theme: &Theme, e: &Element) -> Element {
    let mut s = e.clone();
    habiller_en_place(theme, &mut s);
    s
}

fn habiller_en_place(theme: &Theme, e: &mut Element) {
    for (_, v) in &mut e.attributs {
        if v.contains("var(") || v.to_ascii_lowercase().contains("currentcolor") {
            *v = substituer(theme, v);
        }
    }
    for n in &mut e.enfants {
        if let Noeud::Element(x) = n {
            habiller_en_place(theme, x);
        }
    }
}

fn nombre(v: Option<&str>) -> Option<f32> {
    v.and_then(|v| v.trim().trim_end_matches("px").trim().parse().ok())
        .filter(|x: &f32| *x > 0.0)
}

/// La taille logique du SVG : `width` / `height`, sinon la boîte `viewBox`, sinon 300 × 150 (le
/// défaut d'un élément remplacé).
#[must_use]
pub fn taille(e: &Element) -> (f32, f32) {
    let vb: Vec<f32> = e
        .attribut("viewBox")
        .map(|v| {
            v.split(|c: char| c.is_whitespace() || c == ',')
                .filter_map(|x| x.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let (vl, vh) = match vb.as_slice() {
        [_, _, l, h] if *l > 0.0 && *h > 0.0 => (Some(*l), Some(*h)),
        _ => (None, None),
    };
    match (nombre(e.attribut("width")), nombre(e.attribut("height"))) {
        (Some(l), Some(h)) => (l, h),
        (Some(l), None) => (l, vh.zip(vl).map_or(150.0, |(h, w)| l * h / w)),
        (None, Some(h)) => (vl.zip(vh).map_or(300.0, |(w, hh)| h * w / hh), h),
        (None, None) => (vl.unwrap_or(300.0), vh.unwrap_or(150.0)),
    }
}

/// Le texte du SVG prêt à rastériser à l'instant `t` (habillé, figé).
#[must_use]
pub fn source_a(theme: &Theme, e: &Element, t: f32) -> String {
    let habille = habiller(theme, e);
    let fige = if smil::anime(&habille) {
        smil::figer(&habille, t)
    } else {
        habille
    };
    serialiser(&fige)
}

/// Rastérise le texte SVG pour la densité `densite` (pixels physiques par pixel logique).
#[must_use]
pub fn rasteriser(source: &str, densite: f32) -> Option<Arc<RenderImage>> {
    SvgRenderer::new(Arc::new(()))
        .render_single_frame(source.as_bytes(), ECHELLE_UNITE * densite)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::super::arbre::analyser;
    use super::*;

    #[test]
    fn la_couleur_du_theme_remplace_la_variable() {
        let t = Theme::sombre();
        let a = t.palette.color_accent;
        assert_eq!(
            substituer(&t, "fill: var(--color-accent)"),
            format!("fill: rgb({}, {}, {})", a.r, a.g, a.b)
        );
    }

    #[test]
    fn taille_par_attributs_ou_viewbox() {
        let Some(Noeud::Element(e)) = analyser("<svg width=\"100\" height=\"40\"></svg>").pop()
        else {
            panic!()
        };
        assert_eq!(taille(&e), (100.0, 40.0));
        let Some(Noeud::Element(e)) = analyser("<svg viewBox=\"0 0 200 60\"></svg>").pop() else {
            panic!()
        };
        assert_eq!(taille(&e), (200.0, 60.0));
    }
}
