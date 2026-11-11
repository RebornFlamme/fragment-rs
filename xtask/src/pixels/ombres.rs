//! La catégorie OMBRE du sas (ADR-009, amendement du 11/11/2026), EN DETTE : le flou de GPUI et
//! celui de Skia ne tombent pas sur les mêmes octets (mélange alpha, écart-type, 4 échantillons du
//! shader contre le flou analytique), à 1 ou 2 niveaux près.
//!
//! - **Halo** : calculé depuis la DÉCLARATION du candidat (boîte de bordure posée par la mise en
//!   page, rayon, couches d'ombre), jamais depuis l'image. Par couche : la boîte décalée, gonflée de
//!   l'étendue puis de [`SIGMAS`] écarts-types ; le halo est l'union des couches et de la boîte,
//!   moins l'INTÉRIEUR de l'élément (pixels entièrement couverts par la boîte arrondie), qui reste
//!   strict.
//! - **Tolérance** : delta ≤ [`DELTA_OMBRE`] dans le halo seulement. Un delta plus fort dans le
//!   halo échoue, tout pixel divergent hors d'un halo (et hors des boîtes de texte) aussi.
//! - **Garde-fous** (la contre-épreuve : un décalage ou un rayon de flou changé d'1 px doit
//!   échouer, alors que chaque pixel ne bouge que d'un ou deux niveaux) : sur chacun des quatre
//!   côtés droits de l'élément, on mesure dans les deux images la **distance moyenne** de l'ombre
//!   au bord (pondérée par l'encre) et son **encre** par pixel de bord. Ce sont des sommes sur des
//!   milliers de pixels : elles voient un glissement d'un pixel que le delta ne voit pas. Trois
//!   garde-fous en sortent, insensibles à un alpha un peu plus fort ou plus faible (le mélange de
//!   GPUI) :
//!   1. **étalement** : la moyenne, sur les côtés juges, de l'écart de distance (un flou plus large
//!      éloigne l'ombre de TOUS les côtés) ;
//!   2. **glissement** : l'écart d'encre relatif entre côtés opposés (bas moins haut, droite moins
//!      gauche : un décalage nourrit l'un et affame l'autre) ;
//!   3. **encre** : l'écart relatif moyen d'encre (couleur ou opacité fausses).

use image::{Rgba, RgbaImage};

use super::boites::OmbreDeclaree;

/// Delta de canal toléré dans un halo.
pub const DELTA_OMBRE: u8 = 2;
/// Portée d'une ombre, en écarts-types (le shader de GPUI coupe à 3 σ, Skia aussi).
pub const SIGMAS: f64 = 3.0;
/// Écart maximal (px) de l'étalement : moyenne des écarts de distance au bord. Calé entre les vraies
/// captures qui passent (≤ 0,07 : menu sombre, barres d'outils, notice sombre) et un flou CSS de
/// ±1 px sur la référence du menu (−0,10 à +0,15).
pub const ECART_ETALEMENT: f64 = 0.08;
/// Écart maximal du glissement : différence d'écart d'encre relatif entre côtés opposés.
pub const ECART_GLISSEMENT: f64 = 0.10;
/// Écart relatif maximal de l'encre moyenne.
pub const ECART_ENCRE_OMBRE: f64 = 0.20;
/// Encre minimale d'un côté (niveaux × px, par pixel de bord) pour qu'il soit jugé : en dessous,
/// l'ombre y est trop pâle pour peser (décalage qui la cache sous la boîte).
pub const ENCRE_MIN: f64 = 2.0;
/// Nombre minimal de pixels de bord d'un côté jugé.
pub const BORD_MIN: usize = 8;
/// ★ Le seul jeton dont l'ombre porte l'EXCEPTION NOMMÉE (ADR-009, amendement de 11:15, point b) :
/// le mélange alpha (linéaire contre sRGB) y étale l'ombre de +1,5 px en sombre et de −0,6 px en
/// clair. Ses garde-fous et son delta ne bloquent pas : leurs valeurs mesurées font cliquet, par PNG
/// (donc par thème). Toute autre ombre garde tous ses garde-fous.
pub const JETON_EXCEPTION: &str = "--shadow-l";

/// Rectangle réel (x0, y0, x1, y1), bords exclus en haut.
type RectF = (f64, f64, f64, f64);

/// L'étendue d'une surface ombrée : l'union de sa boîte et de ses couches.
pub fn etendue(o: &OmbreDeclaree) -> RectF {
    let mut r = (o.x, o.y, o.x + o.largeur, o.y + o.hauteur);
    for c in &o.couches {
        let marge = c.etendue + SIGMAS * c.sigma;
        r.0 = r.0.min(o.x + c.decalage_x - marge);
        r.1 = r.1.min(o.y + c.decalage_y - marge);
        r.2 = r.2.max(o.x + o.largeur + c.decalage_x + marge);
        r.3 = r.3.max(o.y + o.hauteur + c.decalage_y + marge);
    }
    r
}

/// Le point (x, y) est dans la boîte arrondie de `o`.
fn dans_boite_arrondie(o: &OmbreDeclaree, x: f64, y: f64) -> bool {
    let (x0, y0, x1, y1) = (o.x, o.y, o.x + o.largeur, o.y + o.hauteur);
    if x < x0 || x > x1 || y < y0 || y > y1 {
        return false;
    }
    let r = o.rayon.min(o.largeur / 2.0).min(o.hauteur / 2.0).max(0.0);
    let cx = if x < x0 + r {
        x0 + r
    } else if x > x1 - r {
        x1 - r
    } else {
        return true;
    };
    let cy = if y < y0 + r {
        y0 + r
    } else if y > y1 - r {
        y1 - r
    } else {
        return true;
    };
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// Le pixel (x, y) est ENTIÈREMENT couvert par la boîte arrondie (ses quatre coins y sont : la
/// boîte est convexe). Un pixel de bord ou de coin, en partie couvert, laisse voir l'ombre.
pub fn interieur(o: &OmbreDeclaree, x: u32, y: u32) -> bool {
    let (x, y) = (f64::from(x), f64::from(y));
    [(x, y), (x + 1.0, y), (x, y + 1.0), (x + 1.0, y + 1.0)]
        .iter()
        .all(|&(a, b)| dans_boite_arrondie(o, a, b))
}

/// Les pixels (indices `y * largeur + x`) de l'étendue de `o`, rognée à l'image.
fn pixels_etendue(
    o: &OmbreDeclaree,
    largeur: u32,
    hauteur: u32,
) -> impl Iterator<Item = (u32, u32)> {
    let (x0, y0, x1, y1) = etendue(o);
    let borne = |v: f64, max: u32| (v.max(0.0) as u32).min(max);
    let (a, b) = (borne(x0.floor(), largeur), borne(y0.floor(), hauteur));
    let (c, d) = (borne(x1.ceil(), largeur), borne(y1.ceil(), hauteur));
    (b..d).flat_map(move |y| (a..c).map(move |x| (x, y)))
}

/// Les masques des ombres : `halo` (tolérance ≤ [`DELTA_OMBRE`]) et `interieur` (strict, union
/// des intérieurs de TOUTES les surfaces : une carte opaque cache l'ombre de sa voisine).
pub struct Masques {
    pub halo: Vec<bool>,
    pub interieur: Vec<bool>,
    /// Le halo des seules surfaces de [`JETON_EXCEPTION`] (hors intérieurs) : un delta > 2 y est
    /// mesuré par le cliquet au lieu d'échouer.
    pub exception: Vec<bool>,
}

pub fn masques(ombres: &[OmbreDeclaree], largeur: u32, hauteur: u32) -> Masques {
    let n = (largeur * hauteur) as usize;
    let mut halo = vec![false; n];
    let mut dedans = vec![false; n];
    let mut exception = vec![false; n];
    for o in ombres {
        let nommee = o.jeton == JETON_EXCEPTION;
        for (x, y) in pixels_etendue(o, largeur, hauteur) {
            let i = (y * largeur + x) as usize;
            if interieur(o, x, y) {
                dedans[i] = true;
            } else {
                halo[i] = true;
                exception[i] |= nommee;
            }
        }
    }
    for ((h, e), d) in halo.iter_mut().zip(exception.iter_mut()).zip(&dedans) {
        *h &= !*d;
        *e &= !*d;
    }
    Masques {
        halo,
        interieur: dedans,
        exception,
    }
}

/// Distance L1 sur les canaux de couleur.
fn l1(a: Rgba<u8>, b: Rgba<u8>) -> f64 {
    (0..3).map(|i| f64::from(a.0[i].abs_diff(b.0[i]))).sum()
}

/// Ce qu'un côté de l'élément montre de son ombre.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cote {
    /// Pixels de bord (colonnes pour haut et bas, lignes pour gauche et droite).
    pub bord: usize,
    /// Encre de l'ombre (distance L1 au fond, sommée) par pixel de bord.
    pub encre: f64,
    /// Distance moyenne au bord de l'élément, pondérée par l'encre (px).
    pub distance: f64,
}

pub const NOMS_COTES: [&str; 4] = ["haut", "bas", "gauche", "droite"];

/// La couleur la plus fréquente sur l'anneau d'un pixel autour de l'étendue de `o` (le fond sur
/// lequel l'ombre tombe), hors des pixels `exclus`.
fn fond(img: &RgbaImage, o: &OmbreDeclaree, exclus: &[bool]) -> Rgba<u8> {
    let (largeur, hauteur) = img.dimensions();
    let (x0, y0, x1, y1) = etendue(o);
    let (a, b) = (x0.floor() as i64 - 1, y0.floor() as i64 - 1);
    let (c, d) = (x1.ceil() as i64, y1.ceil() as i64);
    let mut compte: std::collections::HashMap<[u8; 4], u32> = std::collections::HashMap::new();
    for y in b..=d {
        for x in a..=c {
            let anneau = x == a || x == c || y == b || y == d;
            if !anneau || x < 0 || y < 0 || x >= i64::from(largeur) || y >= i64::from(hauteur) {
                continue;
            }
            if exclus[(y as u32 * largeur + x as u32) as usize] {
                continue;
            }
            *compte
                .entry(img.get_pixel(x as u32, y as u32).0)
                .or_default() += 1;
        }
    }
    compte
        .into_iter()
        .max_by_key(|&(c, n)| (n, c))
        .map_or(Rgba([0; 4]), |(c, _)| Rgba(c))
}

/// Les quatre côtés droits (haut, bas, gauche, droite) de l'ombre de `o` dans `img`. Seuls
/// comptent les pixels du halo hors `exclus` (intérieurs, boîtes de texte), en face de la partie
/// DROITE du bord (les coins arrondis sont écartés).
pub fn cotes(img: &RgbaImage, o: &OmbreDeclaree, halo: &[bool], exclus: &[bool]) -> [Cote; 4] {
    let (largeur, hauteur) = img.dimensions();
    let f = fond(img, o, exclus);
    let (x0, y0, x1, y1) = (o.x, o.y, o.x + o.largeur, o.y + o.hauteur);
    let r = o.rayon.min(o.largeur / 2.0).min(o.hauteur / 2.0).max(0.0);
    let mut encre = [0.0f64; 4];
    let mut moment = [0.0f64; 4];
    let mut bords: [std::collections::BTreeSet<u32>; 4] = Default::default();
    for (x, y) in pixels_etendue(o, largeur, hauteur) {
        let i = (y * largeur + x) as usize;
        if !halo[i] || exclus[i] {
            continue;
        }
        let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
        let face_h = cx > x0 + r && cx < x1 - r;
        let face_v = cy > y0 + r && cy < y1 - r;
        let (cote, t, rang) = if face_h && cy < y0 {
            (0, y0 - cy, x)
        } else if face_h && cy > y1 {
            (1, cy - y1, x)
        } else if face_v && cx < x0 {
            (2, x0 - cx, y)
        } else if face_v && cx > x1 {
            (3, cx - x1, y)
        } else {
            continue;
        };
        let d = l1(*img.get_pixel(x, y), f);
        encre[cote] += d;
        moment[cote] += d * t;
        bords[cote].insert(rang);
    }
    std::array::from_fn(|k| {
        let bord = bords[k].len();
        Cote {
            bord,
            encre: if bord > 0 {
                encre[k] / bord as f64
            } else {
                0.0
            },
            distance: if encre[k] > 0.0 {
                moment[k] / encre[k]
            } else {
                0.0
            },
        }
    })
}

/// Les mesures d'une surface ombrée et les garde-fous qu'elle viole.
#[derive(Debug, Clone)]
pub struct ResultatOmbre {
    pub ombre: OmbreDeclaree,
    pub reference: [Cote; 4],
    pub candidat: [Cote; 4],
    pub violations: Vec<String>,
    /// Les valeurs des trois garde-fous (0 pour un garde-fou sans côté jugé).
    pub gardes: Gardes,
    /// L'ombre est celle de [`JETON_EXCEPTION`] : ses violations ne bloquent pas, ses gardes font
    /// cliquet.
    pub exception: bool,
}

/// Les valeurs mesurées des garde-fous d'une ombre : étalement (px), encre et glissement (écarts
/// relatifs ; le glissement est celui de l'axe le plus écarté).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Gardes {
    pub etalement: f64,
    pub encre: f64,
    pub glissement: f64,
}

impl ResultatOmbre {
    /// Les garde-fous tiennent, ou l'ombre est l'exception nommée (jugée par le cliquet).
    pub fn ok(&self) -> bool {
        self.exception || self.violations.is_empty()
    }

    /// Les chiffres des côtés, pour le rapport : écart de distance (px) et d'encre (%).
    pub fn mesures(&self) -> String {
        (0..4)
            .filter(|&k| self.reference[k].bord >= BORD_MIN)
            .map(|k| {
                let (r, c) = (self.reference[k], self.candidat[k]);
                let encre = if r.encre > 0.0 {
                    100.0 * (c.encre - r.encre) / r.encre
                } else {
                    0.0
                };
                format!(
                    "{} {:+.2} px {:+.1} %",
                    NOMS_COTES[k],
                    c.distance - r.distance,
                    encre
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Juge l'ombre de `o` : ses quatre côtés, mesurés sur les mêmes pixels des deux images.
pub fn juger(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    o: &OmbreDeclaree,
    halo: &[bool],
    exclus: &[bool],
) -> ResultatOmbre {
    let r = cotes(reference, o, halo, exclus);
    let c = cotes(candidat, o, halo, exclus);
    let juge = |k: usize| r[k].bord >= BORD_MIN && r[k].encre.max(c[k].encre) >= ENCRE_MIN;
    let encre_rel = |k: usize| {
        if r[k].encre > 0.0 {
            (c[k].encre - r[k].encre) / r[k].encre
        } else {
            f64::INFINITY
        }
    };
    let juges: Vec<usize> = (0..4).filter(|&k| juge(k)).collect();
    let mut violations = Vec::new();
    let mut gardes = Gardes::default();
    if !juges.is_empty() {
        let n = juges.len() as f64;
        let etalement = juges
            .iter()
            .map(|&k| c[k].distance - r[k].distance)
            .sum::<f64>()
            / n;
        gardes.etalement = etalement;
        if etalement.abs() > ECART_ETALEMENT {
            violations.push(format!(
                "ombre : étalement {etalement:+.2} px (borne ±{ECART_ETALEMENT}) : flou différent"
            ));
        }
        let encre = juges.iter().map(|&k| encre_rel(k)).sum::<f64>() / n;
        gardes.encre = encre;
        if encre.abs() > ECART_ENCRE_OMBRE {
            violations.push(format!(
                "ombre : encre {:+.0} % (borne ±{:.0} %) : couleur ou opacité",
                100.0 * encre,
                100.0 * ECART_ENCRE_OMBRE
            ));
        }
    }
    for (a, b, axe) in [(0, 1, "vertical"), (2, 3, "horizontal")] {
        if !(juge(a) && juge(b)) {
            continue;
        }
        let glissement = encre_rel(b) - encre_rel(a);
        if glissement.abs() > gardes.glissement.abs() {
            gardes.glissement = glissement;
        }
        if glissement.abs() > ECART_GLISSEMENT {
            violations.push(format!(
                "ombre : glissement {axe} {:+.0} % d'encre entre {} et {} (borne ±{:.0} %) : décalage différent",
                100.0 * glissement,
                NOMS_COTES[b],
                NOMS_COTES[a],
                100.0 * ECART_GLISSEMENT
            ));
        }
    }
    ResultatOmbre {
        ombre: o.clone(),
        reference: r,
        candidat: c,
        violations,
        gardes,
        exception: o.jeton == JETON_EXCEPTION,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::boites::CoucheDeclaree;
    use super::*;

    /// Fonction d'erreur (Abramowitz et Stegun 7.1.26, erreur < 1,5e-7).
    fn erf(x: f64) -> f64 {
        let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
        let p = t
            * (0.254_829_592
                + t * (-0.284_496_736
                    + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
        let v = 1.0 - p * (-x * x).exp();
        if x < 0.0 { -v } else { v }
    }

    pub(in crate::pixels) fn phi(x: f64) -> f64 {
        0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
    }

    /// Une scène d'ombre : un fond, une boîte rectangulaire opaque, son ombre gaussienne (le
    /// modèle de CSS : boîte floutée d'écart-type σ), quantifiée à l'octet. `biais` imite un
    /// autre rasterizeur : il s'ajoute à l'alpha avant l'arrondi.
    pub(in crate::pixels) struct Modele {
        pub fond: [f64; 3],
        pub boite: [f64; 3],
        pub alpha: f64,
        pub o: OmbreDeclaree,
    }

    impl Modele {
        pub fn menu_sombre() -> Self {
            Modele {
                fond: [20.0, 21.0, 23.0],
                boite: [28.0, 29.0, 33.0],
                alpha: 0.55,
                o: OmbreDeclaree {
                    x: 100.0,
                    y: 80.0,
                    largeur: 200.0,
                    hauteur: 140.0,
                    rayon: 0.0,
                    couches: vec![CoucheDeclaree {
                        decalage_x: 0.0,
                        decalage_y: 8.0,
                        sigma: 12.0,
                        etendue: 0.0,
                    }],
                    jeton: String::new(),
                },
            }
        }

        pub fn peindre(&self, decalage_y: f64, sigma: f64, biais: f64) -> RgbaImage {
            let (l, h) = (400u32, 320u32);
            let o = &self.o;
            let mut img = RgbaImage::new(l, h);
            for (x, y, p) in img.enumerate_pixels_mut() {
                let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let couleur = if interieur(o, x, y) {
                    self.boite
                } else {
                    let ax = phi((cx - o.x) / sigma) - phi((cx - o.x - o.largeur) / sigma);
                    let ay = phi((cy - o.y - decalage_y) / sigma)
                        - phi((cy - o.y - o.hauteur - decalage_y) / sigma);
                    let a = (self.alpha * ax * ay * (1.0 + biais)).clamp(0.0, 1.0);
                    std::array::from_fn(|k| self.fond[k] * (1.0 - a))
                };
                *p = Rgba([
                    couleur[0].round() as u8,
                    couleur[1].round() as u8,
                    couleur[2].round() as u8,
                    255,
                ]);
            }
            img
        }
    }

    fn juger_modele(m: &Modele, candidat: &RgbaImage, reference: &RgbaImage) -> ResultatOmbre {
        let (l, h) = reference.dimensions();
        let ms = masques(std::slice::from_ref(&m.o), l, h);
        juger(reference, candidat, &m.o, &ms.halo, &ms.interieur)
    }

    #[test]
    fn l_interieur_d_une_boite_arrondie_exclut_ses_coins() {
        let o = OmbreDeclaree {
            x: 10.0,
            y: 10.0,
            largeur: 20.0,
            hauteur: 20.0,
            rayon: 6.0,
            couches: vec![],
            jeton: String::new(),
        };
        assert!(interieur(&o, 20, 20));
        assert!(interieur(&o, 10, 20), "bord droit, pixel entier");
        assert!(!interieur(&o, 10, 10), "coin arrondi");
        assert!(!interieur(&o, 9, 20), "hors de la boîte");
        assert!(!interieur(&o, 29, 29));
    }

    #[test]
    fn le_halo_va_jusqu_a_trois_sigmas_apres_le_decalage() {
        let m = Modele::menu_sombre();
        let (x0, y0, x1, y1) = etendue(&m.o);
        assert_eq!((x0, y0, x1, y1), (64.0, 52.0, 336.0, 264.0));
        let ms = masques(std::slice::from_ref(&m.o), 400, 320);
        assert!(ms.halo[(263 * 400 + 200) as usize]);
        assert!(!ms.halo[(264 * 400 + 200) as usize]);
        assert!(ms.interieur[(150 * 400 + 200) as usize]);
        assert!(!ms.halo[(150 * 400 + 200) as usize]);
    }

    #[test]
    fn la_meme_ombre_a_un_niveau_pres_passe() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 12.0, 0.0);
        // Un autre rasterizeur : alpha 4 % plus fort, d'où des pixels à ±1 niveau.
        let c = m.peindre(8.0, 12.0, 0.04);
        let res = juger_modele(&m, &c, &r);
        assert!(res.ok(), "{:?} [{}]", res.violations, res.mesures());
    }

    /// ★ CONTRE-ÉPREUVE (ADR-009, amendement) : un décalage changé d'1 px ne bouge aucun pixel de
    /// plus de 2 niveaux, mais la distance de l'ombre au bord le trahit.
    #[test]
    fn un_decalage_d_un_pixel_fait_echouer() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 12.0, 0.0);
        let c = m.peindre(9.0, 12.0, 0.0);
        let delta = r
            .pixels()
            .zip(c.pixels())
            .map(|(a, b)| (0..3).map(|k| a.0[k].abs_diff(b.0[k])).max().unwrap_or(0))
            .max()
            .unwrap_or(0);
        assert!(
            delta <= DELTA_OMBRE,
            "delta {delta} : le delta seul suffirait"
        );
        let res = juger_modele(&m, &c, &r);
        assert!(!res.ok(), "[{}]", res.mesures());
    }

    /// ★ L'exception nommée (amendement de 11:15, point b) : la même ombre trop floue échoue sous un
    /// autre jeton, passe sous `--shadow-l` avec ses valeurs MESURÉES (pour le cliquet), et un pixel
    /// de delta > 2 de son halo est compté au lieu d'échouer. Rien ne change pour les autres jetons.
    #[test]
    fn seul_shadow_l_porte_l_exception_et_ses_mesures() {
        let mut m = Modele::menu_sombre();
        let r = m.peindre(8.0, 12.0, 0.0);
        let c = m.peindre(8.0, 13.5, 0.3);
        m.o.jeton = "--menu-shadow".to_owned();
        let autre = juger_modele(&m, &c, &r);
        assert!(!autre.ok() && !autre.exception);
        m.o.jeton = JETON_EXCEPTION.to_owned();
        let nommee = juger_modele(&m, &c, &r);
        assert!(nommee.exception && nommee.ok(), "{:?}", nommee.violations);
        assert!(
            !nommee.violations.is_empty(),
            "les violations restent lisibles"
        );
        assert!(
            nommee.gardes.etalement > ECART_ETALEMENT,
            "{:?}",
            nommee.gardes
        );
        let s = crate::pixels::sas::comparer(&r, &c, &[], std::slice::from_ref(&m.o)).unwrap();
        assert_eq!(
            s.differents_hors, 0,
            "{} px hors tolérance",
            s.differents_hors
        );
        assert!(s.forts_exception > 0 && s.delta_max_exception > DELTA_OMBRE);
        assert_eq!(s.verdict, crate::pixels::Verdict::Tolere);
        let g = s.gardes_exception().unwrap_or_default();
        assert!(g.etalement > ECART_ETALEMENT, "{g:?}");
    }

    /// ★ CONTRE-ÉPREUVE : un rayon de flou changé d'1 px (CSS : `flou` 24 → 25, σ 12 → 12,5 ; et
    /// σ 12 → 13) fait échouer.
    #[test]
    fn un_rayon_de_flou_d_un_pixel_fait_echouer() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 12.0, 0.0);
        for sigma in [12.5, 13.0, 11.5] {
            let c = m.peindre(8.0, sigma, 0.0);
            let res = juger_modele(&m, &c, &r);
            assert!(!res.ok(), "σ {sigma} : [{}]", res.mesures());
        }
    }
}

/// ★ CONTRE-ÉPREUVE SUR LA RÉFÉRENCE : la capture de Fragment du menu sombre, où la vraie capture
/// candidate passe le sas (run CI 42997199734 : étalement +0,02 px, glissement −2 %). On y déplace
/// l'ombre d'1 px, ou on élargit son flou, par le modèle gaussien de CSS appliqué au fond réel ; aucun
/// pixel ne bouge de plus de 2 niveaux, et pourtant le sas échoue.
#[cfg(test)]
mod contre_epreuve {
    use super::super::boites::CoucheDeclaree;
    use super::tests::phi;
    use super::*;

    /// Le menu de `t1-menu-dark` tel que le candidat le déclare : boîte 190 × 205 en (420, 260),
    /// rayon 8, `--menu-shadow` sombre (0 8 24, σ = 12).
    fn menu() -> OmbreDeclaree {
        OmbreDeclaree {
            x: 420.0,
            y: 260.0,
            largeur: 190.0,
            hauteur: 205.0,
            rayon: 8.0,
            couches: vec![CoucheDeclaree {
                decalage_x: 0.0,
                decalage_y: 8.0,
                sigma: 12.0,
                etendue: 0.0,
            }],
            jeton: "--menu-shadow".to_owned(),
        }
    }

    /// Alpha du modèle (boîte rectangulaire floutée) au centre du pixel.
    fn alpha(o: &OmbreDeclaree, x: u32, y: u32, dy: f64, sigma: f64) -> f64 {
        let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
        0.55 * (phi((cx - o.x) / sigma) - phi((cx - o.x - o.largeur) / sigma))
            * (phi((cy - o.y - dy) / sigma) - phi((cy - o.y - o.hauteur - dy) / sigma))
    }

    /// La référence dont l'ombre passe du décalage 8 et
    /// du σ 12 à `dy` et `sigma`, dans le halo seulement.
    fn deplacer(reference: &RgbaImage, o: &OmbreDeclaree, dy: f64, sigma: f64) -> RgbaImage {
        let (l, h) = reference.dimensions();
        let ms = masques(std::slice::from_ref(o), l, h);
        let mut c = reference.clone();
        for (x, y, p) in c.enumerate_pixels_mut() {
            if !ms.halo[(y * l + x) as usize] {
                continue;
            }
            // Le pas quantifié du modèle (fond du thème sombre, 0x141517) s'ajoute à la référence :
            // une ombre noire multiplie le fond par 1 − a.
            let (a0, a1) = (alpha(o, x, y, 8.0, 12.0), alpha(o, x, y, dy, sigma));
            for (canal, f) in [20.0f64, 21.0, 23.0].into_iter().enumerate() {
                let pas = (f * (1.0 - a1)).round() - (f * (1.0 - a0)).round();
                p.0[canal] = (f64::from(p.0[canal]) + pas).clamp(0.0, 255.0) as u8;
            }
        }
        c
    }

    fn reference() -> RgbaImage {
        let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pixels/reference/t1-menu-dark.png");
        image::open(chemin)
            .map(|i| i.to_rgba8())
            .unwrap_or_else(|e| panic!("référence illisible : {e}"))
    }

    fn sas(r: &RgbaImage, c: &RgbaImage) -> crate::pixels::sas::Sas {
        crate::pixels::sas::comparer(r, c, &[], &[menu()]).unwrap()
    }

    #[test]
    fn la_reference_contre_elle_meme_est_stricte() {
        let r = reference();
        assert_eq!(sas(&r, &r).verdict, crate::pixels::Verdict::Strict);
    }

    #[test]
    fn une_ombre_deplacee_ou_plus_floue_sur_la_reference_echoue() {
        let r = reference();
        for (dy, sigma, quoi) in [
            (9.0, 12.0, "décalage +1"),
            (7.0, 12.0, "décalage −1"),
            (8.0, 12.5, "flou CSS +1 (σ +0,5)"),
            (8.0, 11.5, "flou CSS −1 (σ −0,5)"),
        ] {
            let c = deplacer(&r, &menu(), dy, sigma);
            let s = sas(&r, &c);
            assert!(s.differents_ombre > 0, "{quoi} : l'ombre doit bouger");
            assert!(
                s.forts_dans_halos == 0 && s.differents_hors == 0,
                "{quoi} : le delta seul (≤ 2) ne le verrait pas"
            );
            assert_eq!(
                s.verdict,
                crate::pixels::Verdict::Echec,
                "{quoi} : [{}]",
                s.ombres[0].mesures()
            );
        }
    }
}
