// Port de app/src/ui/index.css (les formes de valeur que portent les jetons) et de la lecture des
// valeurs d'un thème de coffre (app/src/ui/theme/ThemePlugin.ts, qui les passait telles quelles
// à Chromium : ici il faut les comprendre soi-même).
//
//! Les valeurs CSS simples d'un jeton : couleur, longueur en pixels, nombre, durée, courbe de
//! Bézier, liste d'ombres. Ce sont les FEUILLES du graphe de [`crate::cascade`] ; les fonctions
//! (`var()`, `color-mix`, `calc`) y sont traitées, pas ici.
//!
//! Chaque lecteur rend `None` plutôt qu'une erreur : l'appelant décide de ce qu'est une valeur
//! illisible (un avertissement, jamais un plantage).
//!
//! ★ Le texte est lu avec la même arithmétique que les littéraux du code (`f32` pour l'opacité
//! décimale, arrondi de Chromium pour les canaux) : c'est ce qui rend les thèmes construits depuis
//! le graphe identiques, au bit près, à ceux qu'on écrivait à la main.

use crate::couleur::{Couleur, Ombre};

/// Arrondi de Chromium pour un canal 8 bits : au plus proche, 0,5 vers le haut.
fn octet(x: f64) -> u8 {
    // Borné à 0..=255 : la conversion ne peut ni déborder ni perdre de signe.
    (x + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// Découpe `texte` aux séparateurs qui ne sont pas dans une parenthèse.
fn decouper(texte: &str, separateur: impl Fn(char) -> bool) -> Vec<&str> {
    let mut morceaux = Vec::new();
    let (mut profondeur, mut debut) = (0_i32, 0);
    for (i, c) in texte.char_indices() {
        match c {
            '(' => profondeur += 1,
            ')' => profondeur -= 1,
            _ if profondeur == 0 && separateur(c) => {
                morceaux.push(&texte[debut..i]);
                debut = i + c.len_utf8();
            }
            _ => {}
        }
    }
    morceaux.push(&texte[debut..]);
    morceaux
}

/// Les morceaux séparés par des virgules hors parenthèses, rognés (les vides sont gardés).
pub(crate) fn par_virgules(texte: &str) -> Vec<&str> {
    decouper(texte, |c| c == ',')
        .into_iter()
        .map(str::trim)
        .collect()
}

/// Les morceaux séparés par des blancs hors parenthèses (les vides disparaissent).
pub(crate) fn par_blancs(texte: &str) -> Vec<&str> {
    decouper(texte, char::is_whitespace)
        .into_iter()
        .filter(|m| !m.is_empty())
        .collect()
}

/// `nom(arguments)` : le nom (en minuscules) et ce qu'il y a entre la première parenthèse et la
/// dernière. `None` si le texte n'est pas UN appel de fonction : la première parenthèse doit se
/// refermer à la toute fin (`rgba(…), rgba(…)` est une liste, pas un appel).
pub(crate) fn fonction(texte: &str) -> Option<(String, &str)> {
    let texte = texte.trim();
    let ouvrante = texte.find('(')?;
    let nom = texte[..ouvrante].trim();
    if nom.is_empty() || !nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let mut profondeur = 0_i32;
    for (i, c) in texte[ouvrante..].char_indices() {
        match c {
            '(' => profondeur += 1,
            ')' => {
                profondeur -= 1;
                if profondeur == 0 {
                    // La parenthèse qui ferme la première doit être la dernière du texte.
                    return (ouvrante + i + 1 == texte.len())
                        .then(|| (nom.to_ascii_lowercase(), &texte[ouvrante + 1..ouvrante + i]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Un nombre décimal (`1.5`, `-2`, `.5`) ; ni `inf` ni `NaN`.
pub fn nombre(texte: &str) -> Option<f32> {
    let t = texte.trim();
    let premier = t.chars().next()?;
    if !(premier.is_ascii_digit() || matches!(premier, '-' | '+' | '.')) {
        return None;
    }
    t.parse::<f32>().ok().filter(|n| n.is_finite())
}

/// Une longueur en pixels : `12px`, `0.5px` ou le zéro sans unité. Les autres unités sont
/// refusées (Fragment n'en écrit pas dans ses jetons).
pub fn longueur_px(texte: &str) -> Option<f32> {
    let t = texte.trim().to_ascii_lowercase();
    match t.strip_suffix("px") {
        Some(n) => nombre(n),
        None => nombre(&t).filter(|n| *n == 0.0),
    }
}

/// Une durée en millisecondes : `120ms` ou `0.25s`.
pub fn duree_ms(texte: &str) -> Option<f32> {
    let t = texte.trim().to_ascii_lowercase();
    if let Some(n) = t.strip_suffix("ms") {
        nombre(n)
    } else {
        t.strip_suffix('s').and_then(nombre).map(|s| s * 1000.0)
    }
}

/// `cubic-bezier(x1, y1, x2, y2)`.
pub fn bezier(texte: &str) -> Option<[f32; 4]> {
    let (nom, args) = fonction(texte)?;
    if nom != "cubic-bezier" {
        return None;
    }
    let valeurs: Vec<f32> = par_virgules(args)
        .into_iter()
        .map(nombre)
        .collect::<Option<_>>()?;
    <[f32; 4]>::try_from(valeurs).ok()
}

/// Un chiffre hexadécimal doublé (`f` devient `ff`).
fn double(c: char) -> Option<u8> {
    let v = c.to_digit(16)?;
    u8::try_from(v * 17).ok()
}

/// La notation `#rgb`, `#rgba`, `#rrggbb` ou `#rrggbbaa` (sans le dièse).
fn hexadecimal(chiffres: &str) -> Option<Couleur> {
    if !chiffres.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let octets: Vec<u8> = match chiffres.len() {
        3 | 4 => chiffres.chars().map(double).collect::<Option<_>>()?,
        6 | 8 => chiffres
            .as_bytes()
            .chunks(2)
            .map(|paire| u8::from_str_radix(std::str::from_utf8(paire).ok()?, 16).ok())
            .collect::<Option<_>>()?,
        _ => return None,
    };
    let a = octets.get(3).copied().unwrap_or(255);
    Some(Couleur::octets(octets[0], octets[1], octets[2], a))
}

/// Un canal rouge, vert ou bleu : nombre 0-255 ou pourcentage.
fn canal(texte: &str) -> Option<u8> {
    match texte.strip_suffix('%') {
        Some(p) => Some(octet(f64::from(nombre(p)?) * 255.0 / 100.0)),
        None => Some(octet(f64::from(nombre(texte)?))),
    }
}

/// L'opacité : nombre 0-1 ou pourcentage, bornée.
fn opacite(texte: &str) -> Option<f32> {
    let valeur = match texte.strip_suffix('%') {
        Some(p) => nombre(p)? / 100.0,
        None => nombre(texte)?,
    };
    Some(valeur.clamp(0.0, 1.0))
}

/// Les arguments d'`rgb()`/`hsl()` : trois canaux et une opacité éventuelle, séparés par des
/// virgules (forme ancienne) ou des blancs avec `/` (forme moderne).
fn arguments_de_couleur(args: &str) -> Option<([String; 3], Option<String>)> {
    let (canaux, alpha) = match args.split_once('/') {
        Some((c, a)) => (c, Some(a.trim().to_owned())),
        None => (args, None),
    };
    let mut morceaux: Vec<String> = canaux
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .collect();
    let alpha = match (morceaux.len(), alpha) {
        (3, alpha) => alpha,
        (4, None) => morceaux.pop(),
        _ => return None,
    };
    let [a, b, c] = <[String; 3]>::try_from(morceaux).ok()?;
    Some(([a, b, c], alpha))
}

/// `hsl(teinte, saturation%, luminosité%)` vers trois canaux 8 bits (algorithme de la spec).
fn hsl_vers_rvb(teinte: f64, saturation: f64, luminosite: f64) -> [u8; 3] {
    let h = teinte.rem_euclid(360.0);
    let (s, l) = (saturation.clamp(0.0, 1.0), luminosite.clamp(0.0, 1.0));
    let a = s * l.min(1.0 - l);
    let f = |n: f64| {
        let k = (n + h / 30.0).rem_euclid(12.0);
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    [
        octet(f(0.0) * 255.0),
        octet(f(8.0) * 255.0),
        octet(f(4.0) * 255.0),
    ]
}

/// Une couleur littérale : `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`,
/// `hsla()` et les mots `white`, `black`, `transparent`.
pub fn couleur(texte: &str) -> Option<Couleur> {
    let t = texte.trim().to_ascii_lowercase();
    match t.as_str() {
        "white" => return Some(Couleur::BLANC),
        "black" => return Some(Couleur::NOIR),
        "transparent" => return Some(Couleur::TRANSPARENT),
        _ => {}
    }
    if let Some(chiffres) = t.strip_prefix('#') {
        return hexadecimal(chiffres);
    }
    let (nom, args) = fonction(&t)?;
    let (trois, alpha) = arguments_de_couleur(args)?;
    let [r, v, b] = match nom.as_str() {
        "rgb" | "rgba" => [canal(&trois[0])?, canal(&trois[1])?, canal(&trois[2])?],
        "hsl" | "hsla" => {
            let teinte = trois[0].strip_suffix("deg").unwrap_or(&trois[0]);
            let pourcent = |s: &str| nombre(s.strip_suffix('%')?).map(|p| f64::from(p) / 100.0);
            hsl_vers_rvb(
                f64::from(nombre(teinte)?),
                pourcent(&trois[1])?,
                pourcent(&trois[2])?,
            )
        }
        _ => return None,
    };
    match alpha {
        None => Some(Couleur::octets(r, v, b, 255)),
        Some(a) => Some(Couleur::css(r, v, b, opacite(&a)?)),
    }
}

/// Une liste d'ombres portées : `x y flou [étendue] couleur`, séparées par des virgules.
/// `inset` n'est pas géré (aucune ombre intérieure dans les jetons).
pub fn ombres(texte: &str) -> Option<Vec<Ombre>> {
    par_virgules(texte)
        .into_iter()
        .map(une_ombre)
        .collect::<Option<Vec<_>>>()
        .filter(|liste| !liste.is_empty())
}

fn une_ombre(texte: &str) -> Option<Ombre> {
    let mut teinte = None;
    let mut mesures = Vec::new();
    for morceau in par_blancs(texte) {
        if let Some(c) = couleur(morceau) {
            if teinte.replace(c).is_some() {
                return None; // deux couleurs dans une même ombre
            }
        } else {
            mesures.push(longueur_px(morceau)?);
        }
    }
    let (x, y, flou, etendue) = match mesures[..] {
        [x, y] => (x, y, 0.0, 0.0),
        [x, y, flou] => (x, y, flou, 0.0),
        [x, y, flou, etendue] => (x, y, flou, etendue),
        _ => return None,
    };
    (flou >= 0.0).then_some(Ombre::new(x, y, flou, etendue, teinte?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexadecimal_de_trois_a_huit_chiffres() {
        assert_eq!(couleur("#f80"), Some(Couleur::octets(255, 136, 0, 255)));
        assert_eq!(couleur("#f808"), Some(Couleur::octets(255, 136, 0, 136)));
        assert_eq!(couleur("#646CFF"), Some(Couleur::hex(0x646cff)));
        assert_eq!(
            couleur("#646cff80"),
            Some(Couleur::octets(0x64, 0x6c, 0xff, 0x80))
        );
        assert_eq!(couleur("#12345"), None, "cinq chiffres : illisible");
        assert_eq!(couleur("#ggg"), None);
    }

    #[test]
    fn rgb_et_rgba_deux_syntaxes() {
        assert_eq!(
            couleur("rgb(0, 170, 85)"),
            Some(Couleur::octets(0, 170, 85, 255))
        );
        assert_eq!(
            couleur("rgb(0 170 85)"),
            Some(Couleur::octets(0, 170, 85, 255))
        );
        assert_eq!(
            couleur("rgba(255, 238, 0, 0.22)"),
            Some(Couleur::css(255, 238, 0, 0.22)),
            "même opacité que le littéral du code"
        );
        assert_eq!(
            couleur("rgb(0 0 0 / 50%)"),
            Some(Couleur::css(0, 0, 0, 0.5)),
            "forme moderne, opacité en pourcents"
        );
        assert_eq!(
            couleur("rgb(100%, 0%, 0%)"),
            Some(Couleur::octets(255, 0, 0, 255))
        );
        assert_eq!(
            couleur("rgba(0, 0, 0)"),
            Some(Couleur::NOIR),
            "rgba sans opacité"
        );
        assert_eq!(couleur("rgb(1, 2)"), None);
    }

    #[test]
    fn hsl_et_hsla() {
        assert_eq!(
            couleur("hsl(0, 100%, 50%)"),
            Some(Couleur::octets(255, 0, 0, 255))
        );
        assert_eq!(
            couleur("hsl(120deg 100% 25%)"),
            Some(Couleur::octets(0, 128, 0, 255))
        );
        assert_eq!(
            couleur("hsla(240, 100%, 50%, 0.5)"),
            Some(Couleur::css(0, 0, 255, 0.5))
        );
        assert_eq!(
            couleur("hsl(0, 100, 50)"),
            None,
            "pourcentages obligatoires"
        );
    }

    #[test]
    fn mots_couleurs() {
        assert_eq!(couleur("White"), Some(Couleur::BLANC));
        assert_eq!(couleur("black"), Some(Couleur::NOIR));
        assert_eq!(couleur(" transparent "), Some(Couleur::TRANSPARENT));
        assert_eq!(couleur("rebeccapurple"), None);
    }

    #[test]
    fn longueurs_nombres_durees() {
        assert_eq!(longueur_px("36px"), Some(36.0));
        assert_eq!(longueur_px("0.5px"), Some(0.5));
        assert_eq!(longueur_px("0"), Some(0.0));
        assert_eq!(
            longueur_px("1.5"),
            None,
            "un nombre non nul sans unité n'est pas une longueur"
        );
        assert_eq!(longueur_px("2em"), None);
        assert_eq!(nombre("1.5"), Some(1.5));
        assert_eq!(nombre("inf"), None);
        assert_eq!(duree_ms("120ms"), Some(120.0));
        assert_eq!(duree_ms("0.25s"), Some(250.0));
        assert_eq!(duree_ms("0ms"), Some(0.0));
    }

    #[test]
    fn courbe_de_bezier() {
        assert_eq!(
            bezier("cubic-bezier(0.2, 0, 0, 1)"),
            Some([0.2, 0.0, 0.0, 1.0])
        );
        assert_eq!(bezier("cubic-bezier(0.2, 0, 0)"), None);
        assert_eq!(bezier("ease"), None);
    }

    #[test]
    fn ombres_une_ou_deux_couches() {
        let une = ombres("0 8px 24px rgba(0, 0, 0, 0.55)").expect("une couche");
        assert_eq!(
            une,
            vec![Ombre::new(0.0, 8.0, 24.0, 0.0, Couleur::css(0, 0, 0, 0.55))]
        );
        let deux = ombres("0 1px 2px rgba(0, 0, 0, 0.06), 0 6px 20px rgba(0, 0, 0, 0.08)")
            .expect("deux couches");
        assert_eq!(deux.len(), 2);
        assert_eq!(deux[1].flou, 20.0);
        assert_eq!(
            ombres("2px 3px 4px 5px #fff").map(|o| o[0].etendue),
            Some(5.0)
        );
        assert_eq!(
            ombres("inset 0 1px #000"),
            None,
            "ombre intérieure non gérée"
        );
        assert_eq!(ombres("0 8px 24px"), None, "sans couleur");
    }
}
