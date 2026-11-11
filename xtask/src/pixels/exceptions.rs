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

/// L'élément des arcs du contour de l'onglet actif (haut convexe, pieds concaves, raccords
/// compris), déclarés sans ombre par `fragment_ui::onglets` : son nom EST la clé du cliquet.
pub const CONTOUR_ONGLET: &str = "contour d'onglet actif";

const fn contour(png: &'static str) -> Exception {
    Exception {
        png,
        element: CONTOUR_ONGLET,
        jeton: "--tab-outline-color",
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
    // Palette et modale claires (bordure opaque, bord unique de fr-p4.1) : 20 à 22 px de delta 7 à 8
    // à la jonction de l'arc et du côté droit, l'anticrénelage propre à Skia (mire de S8 : 8 au pire,
    // ADR-013). Les arcs du menu, des notices et de l'infobulle ne sont plus exceptés.
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
    // Contour de l'onglet actif (décision du chef, fr-t1.5) : le chemin de GPUI est rastérisé en
    // MSAA 4x (couverture quantifiée, asymétrique d'un côté à l'autre) et Chromium met environ 40 %
    // d'encre de plus aux raccords ligne/arc : delta jusqu'à 11 en sombre. Les côtés droits et la
    // ligne de base, pré-composés, restent stricts ; seule la bande des arcs déclarés est levée.
    contour("t1-activation-fermeture-dark.png"),
    contour("t1-activation-fermeture-light.png"),
    contour("t1-boutons-fenetre-dark.png"),
    contour("t1-boutons-fenetre-light.png"),
    contour("t1-dock-replie-dark.png"),
    contour("t1-dock-replie-light.png"),
    contour("t1-docks-ouverts-dark.png"),
    contour("t1-docks-ouverts-light.png"),
    contour("t1-entete-vue-dark.png"),
    contour("t1-entete-vue-light.png"),
    contour("t1-fenetre-vide-dark.png"),
    contour("t1-fenetre-vide-light.png"),
    contour("t1-glisser-depot-dark.png"),
    contour("t1-glisser-depot-light.png"),
    contour("t1-glisser-zone-dark.png"),
    contour("t1-glisser-zone-light.png"),
    contour("t1-menu-onglet-dark.png"),
    contour("t1-menu-onglet-light.png"),
    contour("t1-onglet-epingle-dark.png"),
    contour("t1-onglet-epingle-light.png"),
    contour("t1-onglet-survol-dark.png"),
    contour("t1-onglet-survol-light.png"),
    contour("t1-onglets-dark.png"),
    contour("t1-onglets-light.png"),
    contour("t1-pile-active-inactive-dark.png"),
    contour("t1-pile-active-inactive-light.png"),
    contour("t1-poignee-survol-dark.png"),
    contour("t1-poignee-survol-light.png"),
    contour("t1-split-horizontal-dark.png"),
    contour("t1-split-horizontal-light.png"),
    contour("t1-split-proportions-dark.png"),
    contour("t1-split-proportions-light.png"),
    contour("t1-split-vertical-dark.png"),
    contour("t1-split-vertical-light.png"),
    contour("t1-zoom-150-dark.png"),
    contour("t1-zoom-150-light.png"),
    contour("t1-zoom-80-dark.png"),
    contour("t1-zoom-80-light.png"),
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
            // Un arc sans ombre (contour d'onglet) se nomme lui-même ; les autres sont la boîte
            // arrondie d'une surface ombrée de leur jeton.
            (!a.element.is_empty() && a.element == e.element)
                || ombres
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
            ..ArrondiDeclare::default()
        };
        let autre = ArrondiDeclare {
            x: 30.0,
            ..a.clone()
        };
        let liste = du_png("t1-infobulle-light.png");
        assert_eq!(pour_ombre(&liste, &o), Some("ombre de l'infobulle"));
        // Ses arcs ne sont plus exceptés (fr-p4.1, bord unique).
        assert_eq!(pour_arrondi(&liste, &a, std::slice::from_ref(&o)), None);
        let modale = OmbreDeclaree {
            jeton: "--shadow-l".to_owned(),
            ..o.clone()
        };
        let claire = du_png("t1-modale-light.png");
        assert_eq!(
            pour_arrondi(&claire, &a, std::slice::from_ref(&modale)),
            Some("arcs de la modale")
        );
        assert_eq!(
            pour_arrondi(&claire, &autre, std::slice::from_ref(&modale)),
            None
        );
        // Le thème sombre de la modale n'excepte rien.
        let sombre = du_png("t1-modale-dark.png");
        assert_eq!(
            pour_arrondi(&sombre, &a, std::slice::from_ref(&modale)),
            None
        );
        // Une scène sans exception n'en a aucune.
        assert!(du_png("t1-menu-dark.png").is_empty());
        let autre_jeton = OmbreDeclaree {
            jeton: "--shadow-s".to_owned(),
            ..o
        };
        assert_eq!(pour_ombre(&liste, &autre_jeton), None);
    }

    /// Le coin haut gauche du contour de l'onglet : un trait de 1 px centré sur un quart de cercle
    /// de centre (`cx`, `cy`) et de rayon `r`, prolongé par le côté (vers le bas) et le haut (vers
    /// la droite), sur-échantillonné 16 × 16 ; `biais` relève la couverture vers 0 ou 1 (un autre
    /// anticrénelage, même aire en moyenne).
    fn coin_de_contour(cx: f64, cy: f64, r: f64, biais: f64) -> image::RgbaImage {
        let trait_ = |x: f64, y: f64| {
            if x <= cx && y <= cy {
                ((x - cx).hypot(y - cy) - r).abs() <= 0.5
            } else if y > cy {
                (x - (cx - r)).abs() <= 0.5
            } else {
                (y - (cy - r)).abs() <= 0.5
            }
        };
        let mut img = image::RgbaImage::new(40, 40);
        const N: u32 = 16;
        for (x, y, p) in img.enumerate_pixels_mut() {
            let mut n = 0u32;
            for i in 0..N {
                for j in 0..N {
                    let sx = f64::from(x) + (f64::from(i) + 0.5) / f64::from(N);
                    let sy = f64::from(y) + (f64::from(j) + 0.5) / f64::from(N);
                    n += u32::from(trait_(sx, sy));
                }
            }
            let c = f64::from(n) / f64::from(N * N);
            let c = if c > 0.0 && c < 1.0 {
                (c + biais * (c - 0.5) * (1.0 - (2.0 * c - 1.0).abs())).clamp(0.0, 1.0)
            } else {
                c
            };
            let v = (14.0 + c * 38.0).round() as u8;
            *p = image::Rgba([v, v, v, 255]);
        }
        img
    }

    /// Contre-épreuve de l'exception du contour : un autre anticrénelage du même arc est levé
    /// (mesuré par le cliquet), un rayon ou un centre décalé d'1 px sort de la bande et échoue.
    #[test]
    fn le_contour_d_onglet_decale_d_un_pixel_echoue() {
        let (cx, cy, r) = (20.0, 15.5, 5.0);
        // Le trait de 1 px : boîte « à bordure » de rayon extérieur r + 0,5, au demi-pixel.
        let declare = ArrondiDeclare {
            x: cx - r - 0.5,
            y: cy - r - 0.5,
            largeur: 30.0,
            hauteur: 30.0,
            rayon: r + 0.5,
            bordure: 1.0,
            coins: [true, false, false, false],
            element: CONTOUR_ONGLET.to_owned(),
        };
        let nommees = du_png("t1-onglets-dark.png");
        let reference = coin_de_contour(cx, cy, r, 0.0);
        let juger = |candidat: &image::RgbaImage| {
            crate::pixels::sas::comparer_tout(
                &reference,
                candidat,
                &[],
                &[],
                std::slice::from_ref(&declare),
                &nommees,
            )
            .unwrap()
            .differents_hors
        };
        assert_eq!(juger(&coin_de_contour(cx, cy, r, 0.8)), 0);
        assert!(
            juger(&coin_de_contour(cx, cy, r + 1.0, 0.0)) > 0,
            "rayon + 1"
        );
        assert!(
            juger(&coin_de_contour(cx + 1.0, cy, r, 0.0)) > 0,
            "centre + 1 en x"
        );
        assert!(
            juger(&coin_de_contour(cx, cy + 1.0, r, 0.0)) > 0,
            "centre + 1 en y"
        );
    }

    #[test]
    fn le_contour_d_onglet_ne_vaut_que_pour_ses_png_et_son_element() {
        let pied = ArrondiDeclare {
            x: 30.0,
            y: 26.0,
            largeur: 10.0,
            hauteur: 10.0,
            rayon: 4.5,
            bordure: 1.0,
            coins: [false, false, false, true],
            element: CONTOUR_ONGLET.to_owned(),
        };
        let onglets = du_png("t1-onglets-dark.png");
        assert_eq!(pour_arrondi(&onglets, &pied, &[]), Some(CONTOUR_ONGLET));
        // Sans nom, la même boîte reste stricte ; la fenêtre de réglages n'a pas de contour.
        let anonyme = ArrondiDeclare {
            element: String::new(),
            ..pied.clone()
        };
        assert_eq!(pour_arrondi(&onglets, &anonyme, &[]), None);
        assert_eq!(
            pour_arrondi(&du_png("t1-reglages-zoom-dark.png"), &pied, &[]),
            None
        );
    }
}
