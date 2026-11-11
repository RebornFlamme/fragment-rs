//! La catégorie BORD D'ARRONDI du sas (ADR-009, amendement du 11/11/2026 à 11:15, point a), EN DETTE
//! de phase 4 : l'anticrénelage d'une courbe (champ de distance de GPUI contre couverture analytique
//! de Skia) est le même phénomène que pour les glyphes, pas de la géométrie.
//!
//! - **Arc théorique** : calculé depuis la DÉCLARATION du candidat (boîte de bordure posée par la
//!   mise en page, rayon, largeur de bordure), jamais depuis l'image. Une boîte déclare quatre arcs
//!   extérieurs (rayon `r`) et, si elle a une bordure, quatre arcs intérieurs (rayon `r - bordure`,
//!   la règle de CSS), de mêmes centres.
//! - **Bande** : les pixels dont le CENTRE est à ≤ [`BANDE`] px de l'arc, dans le quart de disque du
//!   coin. Tout le reste du bord (côtés droits) et l'intérieur restent stricts.
//! - **Tolérance** : delta ≤ [`DELTA_ARRONDI`] dans la bande, en cliquet.
//! - **Garde-fou de position** (la contre-épreuve : un rayon changé d'1 px ne bouge l'arc que de
//!   0,41 px au plus, soit un delta de 0,41 × le contraste, sous 6 niveaux sur une bordure peu
//!   contrastée) : par arc, l'écart d'aire couverte, Σ (candidat − référence) projeté sur la couleur
//!   « dedans moins dehors » de l'arc, divisé par la longueur de l'arc (π/2 · ρ) : le DÉPLACEMENT
//!   RADIAL MOYEN de l'arc, en px. Un rayon de `r + 1` retire (2 − π/2) · r ≈ 0,43 r px² au coin,
//!   soit 0,27 px de déplacement moyen quel que soit `r` ; deux anticrénelages d'un même arc restent
//!   à quelques centièmes.

use image::RgbaImage;

use super::boites::ArrondiDeclare;

/// Delta de canal toléré dans la bande d'un arc.
pub const DELTA_ARRONDI: u8 = 6;
/// Demi-largeur (px) de la bande, de part et d'autre de l'arc, mesurée au centre du pixel.
pub const BANDE: f64 = 1.0;
/// Déplacement radial moyen maximal (px) d'un arc. Calé entre les vraies captures (voir ADR-009) et
/// un rayon changé d'1 px (0,27 px).
pub const ECART_POSITION: f64 = 0.15;
/// Contraste minimal (norme RGB) entre « dedans » et « dehors » pour juger l'aire : en dessous, un
/// déplacement de l'arc ne déplace aucun pixel de plus d'un niveau ou deux, l'aire n'a pas de sens.
pub const CONTRASTE_MIN: f64 = 6.0;

/// Un arc de coin : centre, rayon, et direction du coin depuis le centre (`sx`, `sy` valent −1 ou
/// +1 : (−1, −1) est le coin haut gauche).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub cx: f64,
    pub cy: f64,
    pub rayon: f64,
    pub sx: f64,
    pub sy: f64,
    /// Arc intérieur de la bordure (sinon : arc extérieur de la boîte).
    pub interieur: bool,
}

/// Le rayon effectif d'une boîte (CSS le borne à la moitié du plus petit côté).
fn rayon_effectif(a: &ArrondiDeclare) -> f64 {
    a.rayon.min(a.largeur / 2.0).min(a.hauteur / 2.0).max(0.0)
}

/// Les arcs d'une boîte arrondie : quatre extérieurs, et quatre intérieurs si elle a une bordure.
pub fn arcs(a: &ArrondiDeclare) -> Vec<Arc> {
    let r = rayon_effectif(a);
    if r <= 0.0 {
        return Vec::new();
    }
    let (x0, y0, x1, y1) = (a.x, a.y, a.x + a.largeur, a.y + a.hauteur);
    let coins = [
        (x0 + r, y0 + r, -1.0, -1.0),
        (x1 - r, y0 + r, 1.0, -1.0),
        (x0 + r, y1 - r, -1.0, 1.0),
        (x1 - r, y1 - r, 1.0, 1.0),
    ];
    let mut liste = Vec::new();
    for ((cx, cy, sx, sy), declare) in coins.into_iter().zip(a.coins) {
        // Un coin non déclaré (côté droit, ou arc d'un autre élément) reste strict.
        if !declare {
            continue;
        }
        liste.push(Arc {
            cx,
            cy,
            rayon: r,
            sx,
            sy,
            interieur: false,
        });
        if a.bordure > 0.0 && r - a.bordure > 0.0 {
            liste.push(Arc {
                cx,
                cy,
                rayon: r - a.bordure,
                sx,
                sy,
                interieur: true,
            });
        }
    }
    liste
}

/// Le centre du pixel (x, y) est dans la bande de l'arc.
///
/// ★ Un pixel dont la CELLULE chevauche le quart de disque (centre à moins d'un
/// demi-pixel hors du quadrant, quand le centre de l'arc est fractionnaire) en
/// fait partie : Skia l'anticrénèle comme le reste de l'arc (décision du chef,
/// fr-t1.5 [C]). Sa distance se prend au prolongement du côté (coordonnée
/// ramenée au bord du quadrant). Un pixel entièrement hors du quadrant reste strict.
pub fn dans_bande(arc: &Arc, x: u32, y: u32) -> bool {
    let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
    let (mut dx, mut dy) = (px - arc.cx, py - arc.cy);
    if dx * arc.sx <= -0.5 || dy * arc.sy <= -0.5 {
        return false;
    }
    if dx * arc.sx < 0.0 {
        dx = 0.0;
    }
    if dy * arc.sy < 0.0 {
        dy = 0.0;
    }
    ((dx * dx + dy * dy).sqrt() - arc.rayon).abs() <= BANDE
}

/// Les pixels de la bande de `arc`, rognée à l'image.
pub fn pixels_bande(arc: &Arc, largeur: u32, hauteur: u32) -> Vec<(u32, u32)> {
    let portee = arc.rayon + BANDE + 1.0;
    let (xa, xb) = if arc.sx < 0.0 {
        (arc.cx - portee, arc.cx)
    } else {
        (arc.cx, arc.cx + portee)
    };
    let (ya, yb) = if arc.sy < 0.0 {
        (arc.cy - portee, arc.cy)
    } else {
        (arc.cy, arc.cy + portee)
    };
    let borne = |v: f64, max: u32| (v.max(0.0) as u32).min(max);
    let (a, b) = (borne(xa.floor(), largeur), borne(ya.floor(), hauteur));
    let (c, d) = (
        borne(xb.ceil() + 1.0, largeur),
        borne(yb.ceil() + 1.0, hauteur),
    );
    (b..d)
        .flat_map(|y| (a..c).map(move |x| (x, y)))
        .filter(|&(x, y)| dans_bande(arc, x, y))
        .collect()
}

/// Le masque des bandes de toutes les boîtes déclarées.
pub fn masque(arrondis: &[ArrondiDeclare], largeur: u32, hauteur: u32) -> Vec<bool> {
    let mut m = vec![false; (largeur * hauteur) as usize];
    for a in arrondis {
        for arc in arcs(a) {
            for (x, y) in pixels_bande(&arc, largeur, hauteur) {
                m[(y * largeur + x) as usize] = true;
            }
        }
    }
    m
}

fn rgb(img: &RgbaImage, x: f64, y: f64) -> Option<[f64; 3]> {
    let (l, h) = img.dimensions();
    if x < 0.0 || y < 0.0 || x >= f64::from(l) || y >= f64::from(h) {
        return None;
    }
    let p = img.get_pixel(x as u32, y as u32).0;
    Some([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])
}

/// Les couleurs « dedans » et « dehors » de l'arc, lues sur la RÉFÉRENCE au milieu d'un côté droit
/// voisin (strict des deux côtés, donc identiques dans les deux images) : la colonne (ou la ligne)
/// juste à l'intérieur de l'arc et celle juste à l'extérieur, 1,5 px après le point de tangence.
/// `None` si la boîte n'a pas de côté droit assez long.
fn couleurs(reference: &RgbaImage, a: &ArrondiDeclare, arc: &Arc) -> Option<([f64; 3], [f64; 3])> {
    let r = rayon_effectif(a);
    if a.hauteur - 2.0 * r >= 3.0 {
        let y = arc.cy - arc.sy * 1.5;
        let dedans = rgb(reference, arc.cx + arc.sx * (arc.rayon - 0.5), y)?;
        let dehors = rgb(reference, arc.cx + arc.sx * (arc.rayon + 0.5), y)?;
        return Some((dedans, dehors));
    }
    if a.largeur - 2.0 * r >= 3.0 {
        let x = arc.cx - arc.sx * 1.5;
        let dedans = rgb(reference, x, arc.cy + arc.sy * (arc.rayon - 0.5))?;
        let dehors = rgb(reference, x, arc.cy + arc.sy * (arc.rayon + 0.5))?;
        return Some((dedans, dehors));
    }
    None
}

/// Ce que le sas lit d'un arc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MesureArc {
    pub arc: Arc,
    /// Contraste « dedans − dehors » (norme RGB), 0 sans côté droit pour le lire.
    pub contraste: f64,
    /// Déplacement radial moyen (px, positif vers le dehors), candidat moins référence : l'écart
    /// d'aire couverte divisé par la longueur de l'arc ; `None` sous [`CONTRASTE_MIN`].
    pub aire: Option<f64>,
}

/// Les mesures d'une boîte arrondie et les garde-fous qu'elle viole.
#[derive(Debug, Clone)]
pub struct ResultatArrondi {
    pub arrondi: ArrondiDeclare,
    pub arcs: Vec<MesureArc>,
    pub violations: Vec<String>,
    /// L'élément d'une exception nommée temporaire de T1 (`exceptions`, à retirer par `fr-p4.1`) :
    /// ses violations ne bloquent pas, ses mesures font cliquet.
    pub nommee: Option<&'static str>,
}

pub const NOMS_COINS: [&str; 4] = ["haut gauche", "haut droit", "bas gauche", "bas droit"];

fn nom_coin(arc: &Arc) -> &'static str {
    match (arc.sx < 0.0, arc.sy < 0.0) {
        (true, true) => NOMS_COINS[0],
        (false, true) => NOMS_COINS[1],
        (true, false) => NOMS_COINS[2],
        (false, false) => NOMS_COINS[3],
    }
}

impl ResultatArrondi {
    pub fn ok(&self) -> bool {
        self.nommee.is_some() || self.violations.is_empty()
    }

    /// Le plus fort déplacement radial moyen, en valeur absolue (0 sans arc jugé).
    pub fn pire_aire(&self) -> f64 {
        self.arcs
            .iter()
            .filter_map(|m| m.aire)
            .fold(0.0, |p, a| if a.abs() > p { a.abs() } else { p })
    }

    /// Les écarts d'aire des arcs, pour le rapport.
    pub fn mesures(&self) -> String {
        self.arcs
            .iter()
            .filter_map(|m| {
                m.aire.map(|a| {
                    format!(
                        "{}{} {a:+.3} px",
                        nom_coin(&m.arc),
                        if m.arc.interieur { " (bordure)" } else { "" }
                    )
                })
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Juge les arcs de `a` : la position moyenne de chacun (son aire couverte), mesurée sur les pixels de sa bande hors
/// `exclus` (boîtes de texte), dans les deux images.
pub fn juger(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    a: &ArrondiDeclare,
    exclus: &[bool],
) -> ResultatArrondi {
    let (largeur, hauteur) = reference.dimensions();
    let mut mesures = Vec::new();
    let mut violations = Vec::new();
    for arc in arcs(a) {
        let Some((dedans, dehors)) = couleurs(reference, a, &arc) else {
            mesures.push(MesureArc {
                arc,
                contraste: 0.0,
                aire: None,
            });
            continue;
        };
        let v: [f64; 3] = std::array::from_fn(|k| dedans[k] - dehors[k]);
        let k2: f64 = v.iter().map(|c| c * c).sum();
        let contraste = k2.sqrt();
        let aire = (contraste >= CONTRASTE_MIN).then(|| {
            pixels_bande(&arc, largeur, hauteur)
                .into_iter()
                .filter(|&(x, y)| !exclus[(y * largeur + x) as usize])
                .map(|(x, y)| {
                    let (r, c) = (reference.get_pixel(x, y).0, candidat.get_pixel(x, y).0);
                    (0..3)
                        .map(|k| (f64::from(c[k]) - f64::from(r[k])) * v[k])
                        .sum::<f64>()
                        / k2
                })
                .sum::<f64>()
                / (std::f64::consts::FRAC_PI_2 * arc.rayon)
        });
        if let Some(e) = aire
            && e.abs() > ECART_POSITION
        {
            violations.push(format!(
                "arrondi {}{} : arc déplacé de {e:+.3} px en moyenne (borne ±{ECART_POSITION}) : rayon ou boîte différents",
                nom_coin(&arc),
                if arc.interieur { " (bordure)" } else { "" }
            ));
        }
        mesures.push(MesureArc {
            arc,
            contraste,
            aire,
        });
    }
    ResultatArrondi {
        arrondi: a.clone(),
        arcs: mesures,
        violations,
        nommee: None,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use image::Rgba;

    /// Une boîte arrondie à bordure sur un fond, rendue par couverture exacte (sur-échantillonnage
    /// 16 × 16) : `rayon` et `dx` (décalage de la boîte) sont ceux du DESSIN ; `biais` imite un autre
    /// anticrénelage (la couverture est relevée vers 0 ou 1 de `biais` sans changer l'aire en
    /// moyenne : une courbe en S autour de 0,5).
    pub(in crate::pixels) struct Modele {
        pub fond: [f64; 3],
        pub bordure: [f64; 3],
        pub remplissage: [f64; 3],
        pub a: ArrondiDeclare,
    }

    impl Modele {
        /// Les couleurs du menu sombre (fond ombré, `--background-modifier-border`, fond du menu).
        pub fn menu_sombre() -> Self {
            Modele {
                fond: [17.0, 18.0, 20.0],
                bordure: [51.0, 52.0, 56.0],
                remplissage: [34.0, 35.0, 40.0],
                a: ArrondiDeclare {
                    x: 20.0,
                    y: 20.0,
                    largeur: 60.0,
                    hauteur: 40.0,
                    rayon: 8.0,
                    bordure: 1.0,
                    ..ArrondiDeclare::default()
                },
            }
        }

        fn dedans(x: f64, y: f64, x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> bool {
            if x < x0 || x > x1 || y < y0 || y > y1 {
                return false;
            }
            let cx = x.clamp(x0 + r, x1 - r);
            let cy = y.clamp(y0 + r, y1 - r);
            (x - cx).powi(2) + (y - cy).powi(2) <= r * r
        }

        pub fn peindre(&self, rayon: f64, dx: f64, biais: f64) -> RgbaImage {
            let a = &self.a;
            let (x0, y0) = (a.x + dx, a.y);
            let (x1, y1) = (x0 + a.largeur, y0 + a.hauteur);
            let b = a.bordure;
            let mut img = RgbaImage::new(100, 80);
            const N: u32 = 16;
            for (x, y, p) in img.enumerate_pixels_mut() {
                let (mut ext, mut int) = (0.0, 0.0);
                for i in 0..N {
                    for j in 0..N {
                        let sx = f64::from(x) + (f64::from(i) + 0.5) / f64::from(N);
                        let sy = f64::from(y) + (f64::from(j) + 0.5) / f64::from(N);
                        ext += f64::from(u8::from(Self::dedans(sx, sy, x0, y0, x1, y1, rayon)));
                        int += f64::from(u8::from(Self::dedans(
                            sx,
                            sy,
                            x0 + b,
                            y0 + b,
                            x1 - b,
                            y1 - b,
                            rayon - b,
                        )));
                    }
                }
                let n2 = f64::from(N * N);
                let s = |c: f64| {
                    if c <= 0.0 || c >= 1.0 {
                        c
                    } else {
                        (c + biais * (c - 0.5) * (1.0 - (2.0 * c - 1.0).abs())).clamp(0.0, 1.0)
                    }
                };
                let (ce, ci) = (s(ext / n2), s(int / n2));
                let couleur: [f64; 3] = std::array::from_fn(|k| {
                    let bord = self.fond[k] + ce * (self.bordure[k] - self.fond[k]);
                    bord + ci * (self.remplissage[k] - bord)
                });
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

    fn sas(m: &Modele, r: &RgbaImage, c: &RgbaImage) -> crate::pixels::sas::Sas {
        crate::pixels::sas::comparer_tout(r, c, &[], &[], std::slice::from_ref(&m.a), &[]).unwrap()
    }

    #[test]
    fn une_boite_a_bordure_declare_huit_arcs() {
        let m = Modele::menu_sombre();
        let liste = arcs(&m.a);
        assert_eq!(liste.len(), 8);
        assert_eq!(
            (liste[0].cx, liste[0].cy, liste[0].rayon),
            (28.0, 28.0, 8.0)
        );
        assert_eq!(liste[1].rayon, 7.0);
        let sans = ArrondiDeclare {
            bordure: 0.0,
            ..m.a.clone()
        };
        assert_eq!(arcs(&sans).len(), 4);
        let carre = ArrondiDeclare {
            rayon: 0.0,
            ..m.a.clone()
        };
        assert!(arcs(&carre).is_empty());
    }

    #[test]
    fn la_bande_suit_l_arc_et_s_arrete_aux_cotes_droits() {
        let m = Modele::menu_sombre();
        let arc = arcs(&m.a)[0];
        // Sur la diagonale du coin haut gauche, l'arc passe à 8 (√2 − 1) ≈ 3,3 px du coin.
        assert!(dans_bande(&arc, 22, 22));
        assert!(!dans_bande(&arc, 20, 20), "le coin lui-même est dehors");
        assert!(!dans_bande(&arc, 25, 25), "l'intérieur est strict");
        // Le côté gauche, après le point de tangence (y > 28) : hors bande.
        assert!(!dans_bande(&arc, 20, 30));
        assert!(dans_bande(&arc, 20, 27));
    }

    #[test]
    fn le_pixel_qui_chevauche_le_quadrant_d_un_centre_fractionnaire_est_dans_la_bande() {
        // Coin haut gauche, centre à (863,4 ; 6,4), rayon 6 : le pixel (863, 0) a son
        // centre 0,1 px hors du quadrant mais sa cellule le chevauche (cas (863,4) de
        // t1-pile-active-inactive).
        let arc = Arc {
            cx: 863.4,
            cy: 6.4,
            rayon: 6.0,
            sx: -1.0,
            sy: -1.0,
            interieur: false,
        };
        assert!(dans_bande(&arc, 863, 0));
        // Contre-épreuve : le pixel suivant est entièrement hors du quadrant, strict.
        assert!(!dans_bande(&arc, 864, 0));
        // Chevauchant le quadrant mais loin de l'arc (intérieur) : strict.
        assert!(!dans_bande(&arc, 863, 4));
    }

    #[test]
    fn la_meme_boite_est_stricte() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 0.0, 0.0);
        assert_eq!(
            sas(&m, &r, &r.clone()).verdict,
            crate::pixels::Verdict::Strict
        );
    }

    #[test]
    fn un_autre_anticrenelage_du_meme_arc_est_tolere() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 0.0, 0.0);
        let c = m.peindre(8.0, 0.0, 0.6);
        let s = sas(&m, &r, &c);
        assert!(s.differents_arrondi > 0, "le biais doit bouger des pixels");
        assert_eq!(s.differents_hors, 0, "seule la bande bouge");
        assert_eq!(
            s.verdict,
            crate::pixels::Verdict::Tolere,
            "[{}]",
            s.arrondis[0].mesures()
        );
    }

    /// Une bordure peu contrastée (12 niveaux sur le fond) : l'arc qui bouge de 0,41 px n'y déplace
    /// aucun pixel de plus de 6 niveaux.
    fn faible_contraste() -> Modele {
        Modele {
            fond: [30.0, 30.0, 30.0],
            bordure: [42.0, 42.0, 42.0],
            remplissage: [36.0, 36.0, 36.0],
            ..Modele::menu_sombre()
        }
    }

    /// ★ CONTRE-ÉPREUVE (ADR-009, amendement de 11:15) : un rayon changé d'1 px fait échouer, que
    /// la déclaration suive le dessin ou non, même sur une bordure peu contrastée où le delta seul
    /// (≤ 6) ne le verrait pas : c'est la position moyenne de l'arc qui le voit.
    #[test]
    fn un_rayon_change_d_un_pixel_fait_echouer() {
        for m in [Modele::menu_sombre(), faible_contraste()] {
            let r = m.peindre(8.0, 0.0, 0.0);
            for rayon in [7.0, 9.0] {
                let c = m.peindre(rayon, 0.0, 0.0);
                for declare in [8.0, rayon] {
                    let d = Modele {
                        fond: m.fond,
                        bordure: m.bordure,
                        remplissage: m.remplissage,
                        a: ArrondiDeclare {
                            rayon: declare,
                            ..m.a.clone()
                        },
                    };
                    let s = sas(&d, &r, &c);
                    assert_eq!(
                        s.verdict,
                        crate::pixels::Verdict::Echec,
                        "dessin r = {rayon}, déclaré {declare}"
                    );
                    assert!(
                        !s.arrondis[0].ok(),
                        "dessin r = {rayon}, déclaré {declare} : la position doit le voir [{}]",
                        s.arrondis[0].mesures()
                    );
                }
            }
        }
        // Sur la bordure peu contrastée, le delta reste sous la tolérance.
        let m = faible_contraste();
        let (r, c) = (m.peindre(8.0, 0.0, 0.0), m.peindre(9.0, 0.0, 0.0));
        let delta = r
            .pixels()
            .zip(c.pixels())
            .map(|(a, b)| (0..3).map(|k| a.0[k].abs_diff(b.0[k])).max().unwrap_or(0))
            .max()
            .unwrap_or(0);
        assert!(
            delta <= DELTA_ARRONDI,
            "delta {delta} : le delta seul suffirait"
        );
    }

    /// ★ CONTRE-ÉPREUVE : une boîte décalée d'1 px fait échouer (ses côtés droits sont stricts).
    #[test]
    fn une_boite_decalee_d_un_pixel_fait_echouer() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 0.0, 0.0);
        for dx in [1.0, -1.0] {
            let c = m.peindre(8.0, dx, 0.0);
            let s = sas(&m, &r, &c);
            assert_eq!(s.verdict, crate::pixels::Verdict::Echec, "dx = {dx}");
            assert!(
                s.differents_hors > 0,
                "dx = {dx} : les côtés droits divergent"
            );
        }
    }

    #[test]
    fn un_delta_de_plus_de_six_dans_la_bande_echoue() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 0.0, 0.0);
        let mut c = r.clone();
        // Un pixel de la bande du coin haut gauche, 7 niveaux plus clair.
        let p = c.get_pixel(22, 22).0;
        c.put_pixel(22, 22, Rgba([p[0] + 7, p[1] + 7, p[2] + 7, 255]));
        let s = sas(&m, &r, &c);
        assert_eq!(s.verdict, crate::pixels::Verdict::Echec);
        assert_eq!(s.forts_dans_bandes, 1);
    }

    #[test]
    fn sans_declaration_les_coins_restent_stricts() {
        let m = Modele::menu_sombre();
        let r = m.peindre(8.0, 0.0, 0.0);
        let c = m.peindre(8.0, 0.0, 0.6);
        let s = crate::pixels::sas::comparer(&r, &c, &[], &[]).unwrap();
        assert_eq!(s.verdict, crate::pixels::Verdict::Echec);
    }
}
