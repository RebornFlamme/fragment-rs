//! Le lecteur des déclarations CSS EN LIGNE (`style="…"`) du HTML du Markdown, réduit au sous-ensemble
//! d'ADR-012 : couleur, fond, bordure, padding, rayon, police (graisse, style, taille), alignement,
//! soulignement / barré, `display: flex` avec `gap` et `flex`. Le reste est ignoré.
//!
//! Les variables du thème (`var(--color-accent)`, `var(--text-muted)`…) sont résolues par les jetons
//! de `fragment-theme` : un bloc écrit par l'agent prend les couleurs de la note.

use fragment_theme::valeurs_css;
use fragment_theme::{Aplat, Couleur, Theme};

/// Ce que le sous-ensemble retient d'un attribut `style`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleCss {
    pub couleur: Option<Couleur>,
    pub fond: Option<Couleur>,
    /// Épaisseur et couleur de la bordure (`None` pour la couleur : celle du texte, `currentColor`).
    pub bordure: Option<(f32, Option<Couleur>)>,
    /// `[haut, droite, bas, gauche]`, en px.
    pub padding: Option<[f32; 4]>,
    pub rayon: Option<f32>,
    pub gras: Option<bool>,
    pub italique: bool,
    pub taille: Option<f32>,
    pub alignement: Option<Alignement>,
    pub souligne: bool,
    pub barre: bool,
    pub flex: bool,
    /// `flex-direction: column`.
    pub colonne: bool,
    pub gap: Option<f32>,
    pub flex_grow: Option<f32>,
    pub cache: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignement {
    Gauche,
    Centre,
    Droite,
}

/// Les déclarations `nom: valeur` d'un attribut `style`, noms en minuscules.
#[must_use]
pub fn declarations(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|d| {
            let (n, v) = d.split_once(':')?;
            let v = v.trim().trim_end_matches("!important").trim();
            (!n.trim().is_empty() && !v.is_empty())
                .then(|| (n.trim().to_ascii_lowercase(), v.to_owned()))
        })
        .collect()
}

/// La couleur d'une variable du thème (`--color-accent` → `color_accent`).
#[must_use]
pub fn couleur_de_variable(theme: &Theme, nom: &str) -> Option<Couleur> {
    let p = &theme.palette;
    let fond = p.background_primary;
    let champ = nom.trim().trim_start_matches("--").replace('-', "_");
    Some(match champ.as_str() {
        "color_accent" => p.color_accent,
        "color_accent_1" => p.color_accent_1,
        "color_accent_2" => p.color_accent_2,
        "interactive_accent" => p.interactive_accent,
        "background_primary" => p.background_primary,
        "background_primary_alt" => p.background_primary_alt,
        "background_secondary" => p.background_secondary,
        "background_secondary_alt" => p.background_secondary_alt,
        "color_red" => p.color_red,
        "color_orange" => p.color_orange,
        "color_yellow" => p.color_yellow,
        "color_green" => p.color_green,
        "color_cyan" => p.color_cyan,
        "color_blue" => p.color_blue,
        "color_purple" => p.color_purple,
        "color_pink" => p.color_pink,
        "text_error" => p.text_error,
        "text_warning" => p.text_warning,
        "text_success" => p.text_success,
        "text_on_accent" => p.text_on_accent,
        "background_modifier_active_hover" => p.aplat(Aplat::SurvolActif, fond),
        _ => return crate::editeur::jetons::couleur_du_jeton(theme, &champ),
    })
}

/// Une couleur CSS : littérale, `var(--…)` (avec repli `var(--x, rouge)`), `currentColor` (= `None`
/// à l'appel : le texte), ou un mot-clé courant.
#[must_use]
pub fn couleur(theme: &Theme, v: &str) -> Option<Couleur> {
    let v = v.trim();
    if let Some(corps) = v.strip_prefix("var(").and_then(|r| r.strip_suffix(')')) {
        let (nom, repli) = match corps.split_once(',') {
            Some((n, r)) => (n, Some(r)),
            None => (corps, None),
        };
        return couleur_de_variable(theme, nom).or_else(|| repli.and_then(|r| couleur(theme, r)));
    }
    if v.eq_ignore_ascii_case("currentcolor") {
        return Some(theme.palette.text_normal);
    }
    valeurs_css::couleur(v).or_else(|| mot_cle(v))
}

/// Les mots-clés de couleur que les réponses de l'agent écrivent le plus (CSS Color 4).
fn mot_cle(v: &str) -> Option<Couleur> {
    let hex = match v.to_ascii_lowercase().as_str() {
        "red" => 0xff0000,
        "green" => 0x008000,
        "blue" => 0x0000ff,
        "yellow" => 0xffff00,
        "orange" => 0xffa500,
        "purple" => 0x800080,
        "gray" | "grey" => 0x808080,
        "silver" => 0xc0c0c0,
        "navy" => 0x000080,
        "teal" => 0x008080,
        "maroon" => 0x800000,
        "lime" => 0x00ff00,
        "aqua" | "cyan" => 0x00ffff,
        "fuchsia" | "magenta" => 0xff00ff,
        "olive" => 0x808000,
        _ => return None,
    };
    Some(Couleur::hex(hex))
}

/// Une longueur en px (`px`, `em` sur `taille`, nombre nu, `var(--size-…)` du thème).
fn longueur(theme: &Theme, v: &str, taille: f32) -> Option<f32> {
    let v = v.trim();
    if let Some(n) = v.strip_suffix("em").and_then(valeurs_css::nombre)
        && !v.ends_with("rem")
    {
        return Some(n * taille);
    }
    if let Some(n) = v.strip_suffix("rem").and_then(valeurs_css::nombre) {
        return Some(n * taille);
    }
    if let Some(corps) = v.strip_prefix("var(").and_then(|r| r.strip_suffix(')')) {
        return variable_de_taille(theme, corps.split(',').next().unwrap_or(""));
    }
    valeurs_css::longueur_px(v).or_else(|| valeurs_css::nombre(v))
}

fn variable_de_taille(theme: &Theme, nom: &str) -> Option<f32> {
    let f = &theme.forme;
    Some(match nom.trim().trim_start_matches("--") {
        "size-2-1" => f.size_2_1,
        "size-2-3" => f.size_2_3,
        "size-4-1" => f.size_4_1,
        "radius-s" => f.radius_sm,
        "border-width" => f.border_width,
        _ => return None,
    })
}

/// Lit `style` dans le sous-ensemble. `taille` = le corps du texte (pour les `em`).
#[must_use]
pub fn lire(theme: &Theme, style: &str, taille: f32) -> StyleCss {
    let mut s = StyleCss::default();
    for (n, v) in declarations(style) {
        let bas = v.to_ascii_lowercase();
        match n.as_str() {
            "color" => s.couleur = couleur(theme, &v),
            "background" | "background-color" => s.fond = couleur(theme, &v),
            "border" => s.bordure = bordure(theme, &v, taille),
            "padding" => {
                let l: Vec<f32> = v
                    .split_whitespace()
                    .filter_map(|x| longueur(theme, x, taille))
                    .collect();
                s.padding = match l.as_slice() {
                    [a] => Some([*a; 4]),
                    [a, b] => Some([*a, *b, *a, *b]),
                    [a, b, c] => Some([*a, *b, *c, *b]),
                    [a, b, c, d] => Some([*a, *b, *c, *d]),
                    _ => None,
                };
            }
            "border-radius" => s.rayon = longueur(theme, &v, taille),
            "font-weight" => {
                s.gras = Some(
                    bas == "bold" || bas == "bolder" || bas.parse::<u32>().is_ok_and(|w| w >= 600),
                );
            }
            "font-style" => s.italique = bas == "italic" || bas == "oblique",
            "font-size" => s.taille = longueur(theme, &v, taille),
            "text-align" => {
                s.alignement = match bas.as_str() {
                    "center" => Some(Alignement::Centre),
                    "right" | "end" => Some(Alignement::Droite),
                    "left" | "start" => Some(Alignement::Gauche),
                    _ => None,
                };
            }
            "text-decoration" | "text-decoration-line" => {
                s.souligne = bas.contains("underline");
                s.barre = bas.contains("line-through");
            }
            "display" => {
                s.flex = bas == "flex" || bas == "inline-flex";
                s.cache = bas == "none";
            }
            "flex-direction" => s.colonne = bas.starts_with("column"),
            "gap" => {
                s.gap = v
                    .split_whitespace()
                    .next()
                    .and_then(|x| longueur(theme, x, taille))
            }
            "flex" | "flex-grow" => {
                s.flex_grow = v.split_whitespace().next().and_then(valeurs_css::nombre);
            }
            _ => {}
        }
    }
    s
}

/// `border: 1px solid <couleur>` (l'ordre des trois parties est libre).
fn bordure(theme: &Theme, v: &str, taille: f32) -> Option<(f32, Option<Couleur>)> {
    let mut largeur = None;
    let mut c = None;
    let mut style = false;
    for x in v.split_whitespace() {
        let b = x.to_ascii_lowercase();
        if matches!(b.as_str(), "solid" | "dashed" | "dotted" | "double") {
            style = true;
        } else if b == "none" || b == "0" {
            return None;
        } else if let Some(l) = longueur(theme, x, taille) {
            largeur = Some(l);
        } else if let Some(k) = couleur(theme, x) {
            c = Some(k);
        }
    }
    // Sans style, CSS ne dessine pas de bordure (`border-style: none`).
    // ★ La largeur par défaut d'une bordure est `medium` = 3 px.
    style.then(|| (largeur.unwrap_or(3.0), c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn couleur_rgb_et_variable_du_theme() {
        let t = Theme::sombre();
        let s = lire(&t, "color: rgb(200, 0, 0); border: 1px solid", 16.0);
        assert_eq!(s.couleur, Some(Couleur::octets(200, 0, 0, 255)));
        assert_eq!(s.bordure, Some((1.0, None)));
        assert_eq!(
            couleur(&t, "var(--color-accent)"),
            Some(t.palette.color_accent)
        );
        assert_eq!(couleur(&t, "var(--text-muted)"), Some(t.palette.text_muted));
    }

    #[test]
    fn flex_gap_padding() {
        let t = Theme::sombre();
        let s = lire(
            &t,
            "display:flex; gap: 8px; padding: 4px 6px; flex: 1",
            16.0,
        );
        assert!(s.flex);
        assert_eq!(s.gap, Some(8.0));
        assert_eq!(s.padding, Some([4.0, 6.0, 4.0, 6.0]));
        assert_eq!(s.flex_grow, Some(1.0));
    }
}
