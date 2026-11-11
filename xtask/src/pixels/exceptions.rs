//! ★ Les exceptions NOMMÉES TEMPORAIRES de T1 (ADR-009, amendement du 11/11/2026 à 12:00). **À
//! RETIRER PAR `fr-p4.1`** (mélange alpha de GPUI contre Chromium), dont le critère de fin est cette
//! liste vide et ses cliquets ramenés à 0.
//!
//! Résidus sous le pixel d'une seule cause, le mélange alpha (même cause que `--shadow-l`) :
//! étalement des ombres de l'infobulle, du menu clair et des notices claires ; position des arcs
//! clairs ; arcs de la palette et de la modale claires.
//!
//! - Chaque exception nomme son PNG (scène ET thème) et son élément : la surface ombrée de jeton
//!   `jeton` (déclarée par le candidat) et, pour [`Nature::Arcs`], la boîte arrondie de même boîte.
//!   Aucune tolérance générique, ni par thème, ni par catégorie.
//! - Ce qu'elle lève : [`Nature::Ombre`], les trois garde-fous de l'ombre (le delta ≤ 2 du halo
//!   reste exigé) ; [`Nature::Arcs`], le garde-fou de position de ses arcs et le delta ≤ 6 de ses
//!   bandes.
//! - Ce qu'elle mesure fait cliquet (clé `exceptions_t1` de `pixels/cliquet.json`) : ombre,
//!   `etalement`, `encre`, `glissement` en valeur absolue ; arcs, `deplacement` (pire déplacement
//!   radial moyen, px), `delta_max` et `forts` (pixels de delta > 6 de ses bandes).

use super::boites::{ArrondiDeclare, OmbreDeclaree};

/// Ce qu'une exception lève.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nature {
    /// Les garde-fous (étalement, encre, glissement) de la surface ombrée.
    Ombre,
    /// Le garde-fou de position et le delta ≤ 6 des arcs de la boîte arrondie.
    Arcs,
}

/// Une exception nommée : un élément d'un PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exception {
    /// Le PNG (scène et thème).
    pub png: &'static str,
    /// Le nom de l'élément : clé du cliquet et du rapport.
    pub element: &'static str,
    /// Le jeton d'ombre de la surface qui porte l'élément.
    pub jeton: &'static str,
    pub nature: Nature,
}

const fn ombre(png: &'static str, element: &'static str, jeton: &'static str) -> Exception {
    Exception {
        png,
        element,
        jeton,
        nature: Nature::Ombre,
    }
}

const fn arcs(png: &'static str, element: &'static str, jeton: &'static str) -> Exception {
    Exception {
        png,
        element,
        jeton,
        nature: Nature::Arcs,
    }
}

/// La liste, À RETIRER PAR `fr-p4.1`. Valeurs mesurées : `pixels/cliquet.json`, `exceptions_t1`.
pub const EXCEPTIONS: &[Exception] = &[
    // Ombres : étalement de +0,22 à +0,24 px (infobulle, petite boîte : 4 échantillons du shader),
    // −0,19 (menu clair), −0,15 à −0,16 (notices claires) ; borne 0,08.
    ombre(
        "t1-infobulle-dark.png",
        "ombre de l'infobulle",
        "--menu-shadow",
    ),
    ombre(
        "t1-infobulle-light.png",
        "ombre de l'infobulle",
        "--menu-shadow",
    ),
    ombre("t1-menu-light.png", "ombre du menu", "--menu-shadow"),
    ombre("t1-notice-light.png", "ombre de la notice", "--shadow-s"),
    ombre(
        "t1-notice-empilees-light.png",
        "ombre des notices",
        "--shadow-s",
    ),
    // Arcs clairs : déplacement de 0,16 à 0,25 px (contraste de bordure de 7 niveaux) ; borne 0,15.
    arcs(
        "t1-infobulle-light.png",
        "arcs de l'infobulle",
        "--menu-shadow",
    ),
    arcs("t1-menu-light.png", "arcs du menu", "--menu-shadow"),
    arcs("t1-notice-light.png", "arcs de la notice", "--shadow-s"),
    arcs(
        "t1-notice-empilees-light.png",
        "arcs des notices",
        "--shadow-s",
    ),
    // Palette et modale claires : arc de GPUI 0,2 px plus à l'extérieur, 60 px de delta 7 à 33 sur
    // une bordure de 92 niveaux.
    arcs("t1-modale-light.png", "arcs de la modale", "--shadow-l"),
    arcs("t1-palette-light.png", "arcs de la palette", "--shadow-l"),
    arcs(
        "t1-palette-aucun-resultat-light.png",
        "arcs de la palette",
        "--shadow-l",
    ),
    arcs(
        "t1-palette-selection-light.png",
        "arcs de la palette",
        "--shadow-l",
    ),
];

/// Les exceptions du PNG `png`.
pub fn du_png(png: &str) -> Vec<&'static Exception> {
    EXCEPTIONS.iter().filter(|e| e.png == png).collect()
}

/// L'élément nommé d'ombre qui couvre `o`, parmi `liste`.
pub fn pour_ombre(liste: &[&'static Exception], o: &OmbreDeclaree) -> Option<&'static str> {
    liste
        .iter()
        .find(|e| e.nature == Nature::Ombre && e.jeton == o.jeton)
        .map(|e| e.element)
}

/// Même boîte, au demi-pixel près.
fn meme_boite(a: &ArrondiDeclare, o: &OmbreDeclaree) -> bool {
    [
        (a.x, o.x),
        (a.y, o.y),
        (a.largeur, o.largeur),
        (a.hauteur, o.hauteur),
    ]
    .iter()
    .all(|(u, v)| (u - v).abs() < 0.5)
}

/// L'élément nommé d'arcs qui couvre `a`, parmi `liste` : la boîte arrondie d'une surface ombrée
/// de son jeton.
pub fn pour_arrondi(
    liste: &[&'static Exception],
    a: &ArrondiDeclare,
    ombres: &[OmbreDeclaree],
) -> Option<&'static str> {
    liste
        .iter()
        .filter(|e| e.nature == Nature::Arcs)
        .find(|e| {
            ombres
                .iter()
                .any(|o| o.jeton == e.jeton && meme_boite(a, o))
        })
        .map(|e| e.element)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_exception_nomme_un_png_de_t1_et_un_element() {
        for e in EXCEPTIONS {
            assert!(e.png.starts_with("t1-") && e.png.ends_with(".png"), "{e:?}");
            assert!(!e.element.is_empty() && e.jeton.starts_with("--"), "{e:?}");
        }
        // Pas de doublon (png, nature) : un élément par nature et par PNG.
        for (i, a) in EXCEPTIONS.iter().enumerate() {
            for b in &EXCEPTIONS[i + 1..] {
                assert!(!(a.png == b.png && a.nature == b.nature), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn une_exception_ne_vaut_que_pour_son_png_et_son_element() {
        let o = OmbreDeclaree {
            x: 10.0,
            y: 10.0,
            largeur: 50.0,
            hauteur: 20.0,
            rayon: 6.0,
            couches: Vec::new(),
            jeton: "--menu-shadow".to_owned(),
        };
        let a = ArrondiDeclare {
            x: 10.0,
            y: 10.0,
            largeur: 50.0,
            hauteur: 20.0,
            rayon: 6.0,
            bordure: 1.0,
        };
        let autre = ArrondiDeclare {
            x: 30.0,
            ..a.clone()
        };
        let liste = du_png("t1-infobulle-light.png");
        assert_eq!(pour_ombre(&liste, &o), Some("ombre de l'infobulle"));
        assert_eq!(
            pour_arrondi(&liste, &a, std::slice::from_ref(&o)),
            Some("arcs de l'infobulle")
        );
        assert_eq!(pour_arrondi(&liste, &autre, std::slice::from_ref(&o)), None);
        // Le thème sombre de l'infobulle n'excepte que l'ombre, pas les arcs.
        let sombre = du_png("t1-infobulle-dark.png");
        assert_eq!(pour_arrondi(&sombre, &a, std::slice::from_ref(&o)), None);
        // Une scène sans exception n'en a aucune.
        assert!(du_png("t1-menu-dark.png").is_empty());
        let autre_jeton = OmbreDeclaree {
            jeton: "--shadow-s".to_owned(),
            ..o
        };
        assert_eq!(pour_ombre(&liste, &autre_jeton), None);
    }
}
