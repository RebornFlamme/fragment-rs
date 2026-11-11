//! Le sas par catégorie (ADR-009) : strict hors des boîtes de texte et d'icônes, anticrénelage
//! toléré DANS une boîte sous garde-fous.
//!
//! Hors des boîtes (élargies de [`PAD_TOL`] pixel) : 0 pixel de différence. Dans une boîte, quatre
//! garde-fous, mesurés sur la référence ET le candidat :
//! 1. **encre totale** : la couverture cumulée (distance au fond, normalisée par le contraste de
//!    la référence) reste à [`ECART_ENCRE`] près ;
//! 2. **position** (amendement du 11/11, 12:00) : le BARYCENTRE de l'encre du passage est à
//!    ±[`ECART_BARYCENTRE`] px de celui de la référence, en x ET en y ; un passage d'au moins
//!    [`LONGUEUR_MOITIES`] px d'encre est aussi mesuré par moitié gauche et moitié droite (un côté
//!    décalé ne se compense pas de l'autre). Les quatre bords de la boîte d'encre restent à ±1 px (la
//!    largeur à ±1 px est la dette T1 `fr-t1.3`). ★ Le bord à 10 % de couverture ne sert plus
//!    d'origine : GPUI cale un fût sur la grille (1,0 px net) quand Chromium l'étale (0,27 / 0,87 /
//!    0,27, même centre), la colonne de bord passait sous le seuil d'un seul côté ;
//! 3. **forme** : mot par mot, les pixels de cœur d'un trait (couverture ≥ [`COEUR`]) trouvent un trait
//!    (≥ [`PRESENCE`]) au même endroit de l'autre image, au meilleur décalage de ±[`DECALAGE`] px
//!    (±[`DECALAGE_V`] en hauteur) ; la part d'orphelins reste sous [`ECART_FORME`]. Des traits plus
//!    fins ou décalés d'un pixel ne font pas d'orphelin, une lettre changée ou absente en fait ;
//! 4. **teinte** : la couleur de l'encre garde la direction de celle de la référence
//!    (cosinus ≥ [`TEINTE_MIN`]).
//!
//! Une lettre manquante ou changée, une autre graisse ou une autre couleur les font échouer.

use image::{Rgba, RgbaImage};

use super::arrondis::{self, ResultatArrondi};
use super::boites::{ArrondiDeclare, BoiteEncre, Categorie, OmbreDeclaree};
use super::exceptions::{self, Exception};
use super::ombres::{self, ResultatOmbre};
use super::{Boite, Verdict, zones_connexes};

/// Marge autour d'une boîte d'encre où l'écart est toléré (anticrénelage qui déborde).
pub const PAD_TOL: u32 = 1;
/// Marge autour d'une boîte pour mesurer l'encre.
pub const PAD_MESURE: u32 = 2;
/// Marge autour d'une boîte pour estimer le fond.
pub const PAD_FOND: u32 = 4;
/// Écart relatif maximal d'encre totale.
pub const ECART_ENCRE: f64 = 0.25;
/// Part maximale de traits sans équivalent dans un mot (mesure réelle sur les captures : ≤ 2,3 %).
pub const ECART_FORME: f64 = 0.05;
/// Couverture à partir de laquelle un pixel est du coeur d'un trait.
pub const COEUR: f64 = 0.8;
/// Couverture à partir de laquelle un trait est présent dans le voisinage.
pub const PRESENCE: f64 = 0.2;
/// Rayon du voisinage (px) où chercher le trait de l'autre image.
pub const VOISINAGE: i64 = 0;
/// Décalage horizontal (px) toléré entre deux carreaux comparés.
pub const DECALAGE: i64 = 3;
/// Décalage vertical (px) toléré entre deux carreaux comparés.
pub const DECALAGE_V: i64 = 1;
/// Colonnes vides (au plus) à l'intérieur d'un mot : au-delà, un nouveau mot commence.
pub const ESPACE_MOT: i64 = 2;
/// Cosinus minimal entre les teintes d'encre.
pub const TEINTE_MIN: f64 = 0.995;
/// Couverture à partir de laquelle un pixel fait partie de la boîte d'encre.
pub const SEUIL_ENCRE: f64 = 0.10;
/// Écart maximal (px) d'un bord de la boîte d'encre.
pub const ECART_BORD: u32 = 1;
/// Écart maximal (px) du barycentre de l'encre, en x et en y (ADR-009, amendement de 12:00).
pub const ECART_BARYCENTRE: f64 = 0.5;
/// Largeur d'encre (px) à partir de laquelle un passage est aussi mesuré par moitié. ★ Choisie sur
/// les 20 PNG de T1 : une moitié doit porter au moins deux glyphes de 13 px (≈ 7 px d'avance
/// chacun), sinon la masse d'un glyphe coupé par le milieu fait bouger le barycentre de la moitié
/// sans qu'aucun trait ne se déplace. Mesure : voir `docs/parite-pixels.md`.
pub const LONGUEUR_MOITIES: u32 = 28;
/// Couverture de colonne qui fait le coeur d'un glyphe, pour découper un passage en glyphes : sous
/// ce seuil des deux côtés, la colonne est un creux entre deux lettres (franges d'anticrénelage).
pub const COUPURE: f64 = 0.4;

/// Rectangle de pixels, bords inclus : (x0, y0, x1, y1).
type Rect = (u32, u32, u32, u32);

/// La boîte `b` élargie de `pad` et rognée à l'image.
fn rect(b: &BoiteEncre, pad: u32, largeur: u32, hauteur: u32) -> Rect {
    (
        b.x.saturating_sub(pad),
        b.y.saturating_sub(pad),
        (b.x + b.largeur - 1 + pad).min(largeur - 1),
        (b.y + b.hauteur - 1 + pad).min(hauteur - 1),
    )
}

/// Distance L1 sur les canaux de couleur.
fn l1(a: Rgba<u8>, b: Rgba<u8>) -> u32 {
    (0..3).map(|i| u32::from(a.0[i].abs_diff(b.0[i]))).sum()
}

/// La couleur la plus fréquente de `r`.
fn fond(img: &RgbaImage, r: Rect) -> Rgba<u8> {
    let mut compte: std::collections::HashMap<[u8; 4], u32> = std::collections::HashMap::new();
    for y in r.1..=r.3 {
        for x in r.0..=r.2 {
            *compte.entry(img.get_pixel(x, y).0).or_default() += 1;
        }
    }
    compte
        .into_iter()
        .max_by_key(|&(c, n)| (n, c))
        .map_or(Rgba([0; 4]), |(c, _)| Rgba(c))
}

/// Ce que les garde-fous lisent d'une image dans une boîte.
#[derive(Debug, Clone)]
pub struct Mesure {
    /// Couverture cumulée.
    pub encre: f64,
    /// Boîte d'encre (x0, y0, x1, y1), `None` sans encre.
    pub boite: Option<Rect>,
    /// Couverture (0 à 1) de chaque pixel de la région, ligne par ligne, et sa largeur.
    pub couverture: Vec<f64>,
    pub largeur: u32,
    /// Somme des écarts au fond, par canal.
    pub teinte: [f64; 3],
}

/// Mesure `img` dans `r`, avec le fond et le contraste de référence `k` (somme L1 maximale).
fn mesurer(img: &RgbaImage, r: Rect, fond: Rgba<u8>, k: f64) -> Mesure {
    let (l, h) = (r.2 - r.0 + 1, r.3 - r.1 + 1);
    let mut couvertures = Vec::with_capacity((l * h) as usize);
    let mut encre = 0.0;
    let mut boite: Option<Rect> = None;
    let mut teinte = [0.0; 3];
    for y in r.1..=r.3 {
        for x in r.0..=r.2 {
            let p = *img.get_pixel(x, y);
            let couverture = (f64::from(l1(p, fond)) / k).min(1.0);
            encre += couverture;
            couvertures.push(couverture);
            if couverture >= SEUIL_ENCRE {
                boite = Some(match boite {
                    None => (x, y, x, y),
                    Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                });
            }
            if couverture >= 0.15 {
                for (i, t) in teinte.iter_mut().enumerate() {
                    *t += f64::from(p.0[i]) - f64::from(fond.0[i]);
                }
            }
        }
    }
    Mesure {
        encre,
        boite,
        couverture: couvertures,
        largeur: l,
        teinte,
    }
}

/// Garde-fou de forme, mot par mot. Le candidat est découpé en segments (colonnes d'encre séparées
/// de [`ESPACE_MOT`] colonnes vides au moins). Dans un segment, un pixel de COEUR (couverture ≥
/// [`COEUR`]) est orphelin si l'autre image n'a, dans son voisinage de [`VOISINAGE`] px, aucun pixel
/// d'au moins [`PRESENCE`] de couverture : le trait y manque. Un anticrénelage différent, des traits
/// plus fins ou un glyphe décalé d'un pixel ne font pas d'orphelin ; une lettre changée ou absente
/// en fait. L'autre image est décalée de -[`DECALAGE`] à +[`DECALAGE`] px en x (la dette de largeur
/// décale les mots d'un pixel ou deux le long d'une ligne) et de ±[`DECALAGE_V`] px en y, au meilleur
/// décalage. Rend la part d'orphelins du pire segment (orphelins des deux sens / cœurs du candidat).
fn ecart_forme(rf: &Mesure, cd: &Mesure) -> f64 {
    let l = cd.largeur as i64;
    let h = cd.couverture.len() as i64 / l.max(1);
    let lire = |m: &Mesure, x: i64, y: i64| -> f64 {
        if x < 0 || x >= l || y < 0 || y >= h {
            0.0
        } else {
            m.couverture[(y * l + x) as usize]
        }
    };
    let voisin_max = |m: &Mesure, x: i64, y: i64| -> f64 {
        (-VOISINAGE..=VOISINAGE)
            .flat_map(|u| (-VOISINAGE..=VOISINAGE).map(move |v| (u, v)))
            .map(|(u, v)| lire(m, x + u, y + v))
            .fold(0.0, f64::max)
    };
    let encre_col = |x: i64| (0..h).any(|y| lire(cd, x, y) >= 0.15);
    let mut segments: Vec<(i64, i64)> = Vec::new();
    for x in (0..l).filter(|&x| encre_col(x)) {
        match segments.last_mut() {
            Some(s) if x <= s.1 + ESPACE_MOT => s.1 = x,
            _ => segments.push((x, x)),
        }
    }
    let mut pire = 0.0f64;
    for (sa, sb) in segments {
        let coeurs = (0..h)
            .flat_map(|y| (sa..=sb).map(move |x| (x, y)))
            .filter(|&(x, y)| lire(cd, x, y) >= COEUR)
            .count()
            .max(1) as f64;
        let meilleur = (-DECALAGE..=DECALAGE)
            .flat_map(|dx| (-DECALAGE_V..=DECALAGE_V).map(move |dy| (dx, dy)))
            .map(|(dx, dy)| {
                let mut orphelins = 0u32;
                for y in 0..h {
                    for x in sa..=sb {
                        // Coeur du candidat sans trait de la référence alentour.
                        if lire(cd, x, y) >= COEUR && voisin_max(rf, x + dx, y + dy) < PRESENCE {
                            orphelins += 1;
                        }
                        // Coeur de la référence sans trait du candidat alentour.
                        if lire(rf, x + dx, y + dy) >= COEUR && voisin_max(cd, x, y) < PRESENCE {
                            orphelins += 1;
                        }
                    }
                }
                f64::from(orphelins) / coeurs
            })
            .fold(f64::INFINITY, f64::min);
        pire = pire.max(meilleur);
    }
    pire
}

fn cosinus(a: [f64; 3], b: [f64; 3]) -> f64 {
    let produit: f64 = (0..3).map(|i| a[i] * b[i]).sum();
    let na = a.iter().map(|v| v * v).sum::<f64>().sqrt();
    let nb = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return if na == nb { 1.0 } else { 0.0 };
    }
    produit / (na * nb)
}

/// Les chiffres d'une boîte et les garde-fous qu'elle viole.
#[derive(Debug, Clone)]
pub struct ResultatBoite {
    pub boite: BoiteEncre,
    /// Pixels divergents dans la boîte élargie, et plus fort delta de canal.
    pub differents: u64,
    pub delta_max: u8,
    /// Écart relatif d'encre totale (candidat contre référence).
    pub ecart_encre: f64,
    /// Plus grand écart de couverture sur une cellule.
    pub ecart_forme: f64,
    /// Cosinus des teintes.
    pub teinte: f64,
    /// Écarts (candidat moins référence) des barycentres d'encre : (partie, x, y).
    pub barycentres: Vec<(&'static str, f64, f64)>,
    /// Écart (candidat moins référence) des bords d'encre : x0, y0, x1, y1.
    pub ecart_bords: Option<[i64; 4]>,
    /// La largeur d'encre diffère d'un pixel (dette T1, `fr-t1.3`).
    pub dette_largeur: bool,
    /// Garde-fous violés, en clair.
    pub violations: Vec<String>,
}

impl ResultatBoite {
    /// Les chiffres des garde-fous, pour le rapport.
    pub fn mesures(&self) -> String {
        let bords = self.ecart_bords.map_or_else(
            || "—".to_owned(),
            |e| format!("{:+},{:+},{:+},{:+}", e[0], e[1], e[2], e[3]),
        );
        let barycentres = self
            .barycentres
            .iter()
            .map(|(p, x, y)| format!("{p} {x:+.2},{y:+.2}"))
            .collect::<Vec<_>>()
            .join(" ; ");
        format!(
            "encre {:+.1} %, forme {:.1} %, teinte {:.4}, bords {bords}, barycentres {barycentres}",
            100.0 * self.ecart_encre,
            100.0 * self.ecart_forme,
            self.teinte
        )
    }

    pub fn ok(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Barycentre (x, y) de la couverture de `m` sur les colonnes `[xa, xb]` de sa région, en px depuis
/// son coin haut gauche ; `None` sans encre.
fn barycentre(m: &Mesure, xa: u32, xb: u32) -> Option<(f64, f64)> {
    let l = m.largeur as usize;
    let (mut masse, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for (i, &c) in m.couverture.iter().enumerate() {
        let (x, y) = ((i % l) as u32, (i / l) as u32);
        if x < xa || x > xb {
            continue;
        }
        masse += c;
        sx += c * f64::from(x);
        sy += c * f64::from(y);
    }
    (masse > 0.0).then(|| (sx / masse, sy / masse))
}

/// Les glyphes d'un passage, en colonnes `[a, b]` de la région. Coeurs : suites de colonnes où
/// l'une OU l'autre image atteint [`COUPURE`] de couverture ; chaque creux entre deux coeurs est
/// partagé en son milieu (la colonne du milieu d'un creux impair, frange commune aux deux glyphes,
/// n'est à aucun), et les bords de la région vont au premier et au dernier glyphe. La même découpe
/// sert aux deux images.
fn glyphes(rf: &Mesure, cd: &Mesure) -> Vec<(u32, u32)> {
    let l = rf.largeur as usize;
    let h = rf.couverture.len() / l.max(1);
    let plein = |x: usize| {
        (0..h).any(|y| rf.couverture[y * l + x] >= COUPURE || cd.couverture[y * l + x] >= COUPURE)
    };
    let mut coeurs: Vec<(u32, u32)> = Vec::new();
    for x in (0..l).filter(|&x| plein(x)) {
        let x = x as u32;
        match coeurs.last_mut() {
            Some(s) if x == s.1 + 1 => s.1 = x,
            _ => coeurs.push((x, x)),
        }
    }
    let mut glyphes = coeurs.clone();
    for k in 0..coeurs.len().saturating_sub(1) {
        let (fin, debut) = (coeurs[k].1, coeurs[k + 1].0);
        let creux = debut - fin - 1;
        glyphes[k].1 = fin + creux / 2;
        glyphes[k + 1].0 = debut - creux / 2;
    }
    if let Some(g) = glyphes.first_mut() {
        g.0 = 0;
    }
    if let Some(g) = glyphes.last_mut() {
        g.1 = (l as u32).saturating_sub(1);
    }
    glyphes
}

/// Écarts (candidat moins référence) du barycentre de l'encre : le passage entier puis, s'il est
/// assez long, sa moitié gauche et sa moitié droite. Chaque entrée : (partie, écart en x, écart en y).
///
/// ★ Le barycentre se prend glyphe par glyphe (voir [`glyphes`]) puis se moyenne, pondéré par la
/// largeur du coeur du glyphe. Un barycentre unique sur tout le passage lisait la RÉPARTITION de l'encre, pas
/// la position : sur 413 px, des traits de GPUI plus fins de 10 % à gauche qu'à droite déplaçaient
/// le barycentre de −0,9 px, alors que chaque moitié restait à ±0,2 px (run 43011857108). Un glyphe
/// est symétrique sous le calage de GPUI (fût net contre fût étalé, même centre) : son barycentre ne
/// bouge pas ; un glyphe déplacé d'1 px, si.
fn ecarts_barycentres(rf: &Mesure, cd: &Mesure) -> Vec<(&'static str, f64, f64)> {
    let decalages: Vec<(f64, f64, f64, f64)> = glyphes(rf, cd)
        .into_iter()
        .filter_map(|(a, b)| {
            let (r, c) = (barycentre(rf, a, b)?, barycentre(cd, a, b)?);
            Some((
                f64::from(a + b) / 2.0,
                f64::from(b - a + 1),
                c.0 - r.0,
                c.1 - r.1,
            ))
        })
        .collect();
    let moyenne = |nom: &'static str, garde: &dyn Fn(f64) -> bool| {
        let (mut poids, mut dx, mut dy) = (0.0, 0.0, 0.0);
        for &(_, largeur, x, y) in decalages.iter().filter(|d| garde(d.0)) {
            poids += largeur;
            dx += largeur * x;
            dy += largeur * y;
        }
        (poids > 0.0).then(|| (nom, dx / poids, dy / poids))
    };
    let mut parties: Vec<_> = moyenne("passage", &|_| true).into_iter().collect();
    let longueur = rf.boite.map_or(0, |b| b.2 - b.0 + 1);
    if let (Some(&(premier, ..)), Some(&(dernier, ..))) = (decalages.first(), decalages.last())
        && longueur >= LONGUEUR_MOITIES
        && dernier > premier
    {
        let milieu = (premier + dernier) / 2.0;
        parties.extend(moyenne("moitié gauche", &|c| c <= milieu));
        parties.extend(moyenne("moitié droite", &|c| c > milieu));
    }
    parties
}

fn juger(b: &BoiteEncre, rf: &Mesure, cd: &Mesure) -> ResultatBoite {
    let mut violations = Vec::new();
    let ecart_encre = if rf.encre > 0.0 {
        (cd.encre - rf.encre) / rf.encre
    } else if cd.encre > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };
    if ecart_encre.abs() > ECART_ENCRE {
        violations.push(format!(
            "encre totale {:+.0} % (borne ±{:.0} %)",
            100.0 * ecart_encre,
            100.0 * ECART_ENCRE
        ));
    }
    let ecart_forme = ecart_forme(rf, cd);
    if ecart_forme > ECART_FORME {
        violations.push(format!(
            "forme : {:.0} % des traits sans équivalent dans un mot (borne {:.0} %)",
            100.0 * ecart_forme,
            100.0 * ECART_FORME
        ));
    }
    let teinte = cosinus(rf.teinte, cd.teinte);
    if teinte < TEINTE_MIN {
        violations.push(format!("teinte : cosinus {teinte:.3} (min {TEINTE_MIN})"));
    }
    let (ecart_bords, dette_largeur) = match (rf.boite, cd.boite) {
        (Some(r), Some(c)) => {
            let e = [
                i64::from(c.0) - i64::from(r.0),
                i64::from(c.1) - i64::from(r.1),
                i64::from(c.2) - i64::from(r.2),
                i64::from(c.3) - i64::from(r.3),
            ];
            if e.iter().any(|v| v.unsigned_abs() > u64::from(ECART_BORD)) {
                violations.push(format!(
                    "boîte d'encre : bords gauche {:+}, haut {:+}, droit {:+}, bas {:+} px (borne ±{ECART_BORD})",
                    e[0], e[1], e[2], e[3]
                ));
            }
            (Some(e), e[2] != 0)
        }
        (None, None) => (None, false),
        _ => {
            violations.push("encre présente d'un seul côté".to_owned());
            (None, false)
        }
    };
    let barycentres = ecarts_barycentres(rf, cd);
    for &(partie, dx, dy) in &barycentres {
        if dx.abs() > ECART_BARYCENTRE || dy.abs() > ECART_BARYCENTRE {
            violations.push(format!(
                "origine : barycentre ({partie}) décalé de ({dx:+.2}, {dy:+.2}) px (borne ±{ECART_BARYCENTRE})"
            ));
        }
    }
    ResultatBoite {
        boite: *b,
        barycentres,
        differents: 0,
        delta_max: 0,
        ecart_encre,
        ecart_forme,
        teinte,
        ecart_bords,
        dette_largeur,
        violations,
    }
}

/// Résultat d'une comparaison par catégorie.
#[derive(Debug)]
pub struct Sas {
    /// Divergences hors de toute boîte : géométrie, couleur, position (strict).
    pub differents_hors: u64,
    pub zones_hors: Vec<Boite>,
    /// Plus fort delta hors boîtes, et nombre de pixels hors boîtes de delta ≤ 2 (ombres, mélanges
    /// alpha) : de quoi savoir si l'écart est un arrondi ou une vraie divergence.
    pub delta_max_hors: u8,
    pub differents_hors_faibles: u64,
    /// Divergences dans l'union des boîtes, plus fort delta, aire de l'union.
    pub differents_boites: u64,
    pub delta_max_boites: u8,
    pub aire_boites: u64,
    pub boites: Vec<ResultatBoite>,
    /// Catégorie ombre (amendement du 11/11) : pixels tolérés (delta ≤ 2) dans les halos, plus
    /// fort delta parmi eux, aire des halos ; pixels de delta > 2 dans un halo (comptés aussi
    /// dans `differents_hors`).
    pub differents_ombre: u64,
    pub delta_max_ombre: u8,
    pub aire_halos: u64,
    pub forts_dans_halos: u64,
    pub ombres: Vec<ResultatOmbre>,
    /// Exception nommée `--shadow-l` : pixels de delta > 2 dans son halo (non bloquants, en
    /// cliquet) et leur plus fort delta.
    pub forts_exception: u64,
    pub delta_max_exception: u8,
    /// Catégorie bord d'arrondi (amendement de 11:15) : pixels tolérés (delta ≤ 6) dans les bandes
    /// des arcs déclarés, plus fort delta parmi eux, aire des bandes ; pixels de delta > 6 dans une
    /// bande (comptés aussi dans `differents_hors`).
    pub differents_arrondi: u64,
    pub delta_max_arrondi: u8,
    pub aire_bandes: u64,
    pub forts_dans_bandes: u64,
    pub arrondis: Vec<ResultatArrondi>,
    /// Les exceptions nommées temporaires de T1 (à retirer par `fr-p4.1`) : par élément, ses
    /// mesures en valeur absolue (clés du cliquet).
    pub nommees:
        std::collections::BTreeMap<&'static str, std::collections::BTreeMap<&'static str, f64>>,
    pub verdict: Verdict,
    pub diff: RgbaImage,
}

impl Sas {
    /// Part de pixels divergents dans les boîtes (en %).
    pub fn pourcent_boites(&self) -> f64 {
        if self.aire_boites == 0 {
            0.0
        } else {
            100.0 * self.differents_boites as f64 / self.aire_boites as f64
        }
    }

    /// Part de pixels tolérés dans les halos d'ombre (en %).
    pub fn pourcent_ombre(&self) -> f64 {
        if self.aire_halos == 0 {
            0.0
        } else {
            100.0 * self.differents_ombre as f64 / self.aire_halos as f64
        }
    }

    /// Part de pixels tolérés dans les bandes d'arrondi (en %).
    pub fn pourcent_arrondi(&self) -> f64 {
        if self.aire_bandes == 0 {
            0.0
        } else {
            100.0 * self.differents_arrondi as f64 / self.aire_bandes as f64
        }
    }

    /// Les garde-fous mesurés de l'exception nommée : le pire, en valeur absolue, des surfaces de
    /// `--shadow-l` (`None` sans telle surface).
    pub fn gardes_exception(&self) -> Option<ombres::Gardes> {
        let liste: Vec<&ombres::Gardes> = self
            .ombres
            .iter()
            .filter(|o| o.exception)
            .map(|o| &o.gardes)
            .collect();
        if liste.is_empty() {
            return None;
        }
        let pire = |f: fn(&ombres::Gardes) -> f64| {
            liste
                .iter()
                .map(|g| f(g))
                .fold(0.0f64, |p, v| p.max(v.abs()))
        };
        Some(ombres::Gardes {
            etalement: pire(|g| g.etalement),
            encre: pire(|g| g.encre),
            glissement: pire(|g| g.glissement),
        })
    }

    /// Le pire écart absolu des barycentres d'encre, en x et en y, et où se trouve le pire des deux.
    pub fn pire_barycentre(&self) -> Option<(f64, f64, String)> {
        let mut pire: Option<(f64, f64, String, f64)> = None;
        for b in &self.boites {
            for &(partie, dx, dy) in &b.barycentres {
                let (x, y, ou, m) = pire.take().unwrap_or((0.0, 0.0, String::new(), -1.0));
                let (x, y) = (x.max(dx.abs()), y.max(dy.abs()));
                let ici = dx.abs().max(dy.abs());
                pire = Some(if ici > m {
                    (
                        x,
                        y,
                        format!(
                            "{} ({},{}), {partie}",
                            b.boite.categorie.nom(),
                            b.boite.x,
                            b.boite.y
                        ),
                        ici,
                    )
                } else {
                    (x, y, ou, m)
                });
            }
        }
        pire.map(|(x, y, ou, _)| (x, y, ou))
    }

    pub fn dettes_largeur(&self) -> usize {
        self.boites.iter().filter(|b| b.dette_largeur).count()
    }
}

/// Compare `reference` et `candidat` avec les `boites` et les `ombres` déclarées, sans arrondis.
#[cfg(test)]
pub fn comparer(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    boites: &[BoiteEncre],
    ombres: &[OmbreDeclaree],
) -> anyhow::Result<Sas> {
    comparer_tout(reference, candidat, boites, ombres, &[], &[])
}

/// Compare `reference` et `candidat` avec les `boites`, les `ombres` et les `arrondis` déclarés par
/// le candidat, sous les exceptions nommées `nommees` du PNG (`exceptions::du_png`).
pub fn comparer_tout(
    reference: &RgbaImage,
    candidat: &RgbaImage,
    boites: &[BoiteEncre],
    ombres: &[OmbreDeclaree],
    arrondis: &[ArrondiDeclare],
    nommees: &[&'static Exception],
) -> anyhow::Result<Sas> {
    if reference.dimensions() != candidat.dimensions() {
        anyhow::bail!(
            "tailles différentes : référence {:?}, candidat {:?}",
            reference.dimensions(),
            candidat.dimensions()
        );
    }
    let (largeur, hauteur) = reference.dimensions();
    let mut dans = vec![false; (largeur * hauteur) as usize];
    for b in boites {
        let r = rect(b, PAD_TOL, largeur, hauteur);
        for y in r.1..=r.3 {
            for x in r.0..=r.2 {
                dans[(y * largeur + x) as usize] = true;
            }
        }
    }
    // fr-t1.5 : un trait droit (bordure, séparateur, bord de fond) reste STRICT, même sous la boîte
    // gonflée d'un texte qui le longe. Ses pixels sont ceux du détecteur, sur la référence ET sur le
    // candidat : un trait décalé d'1 px diverge sur des pixels de trait des deux images.
    // ★ Les deux, pas l'un OU l'autre : sous le bas d'un glyphe posé sur une bordure, le fond entre
    // deux jambages est un pixel de trait d'une image et de l'encre dans l'autre ; l'union rendrait
    // strict l'anticrénelage même que la boîte tolère.
    let traits_reference = super::boites::pixels_de_trait(reference);
    let traits_candidat = super::boites::pixels_de_trait(candidat);
    for ((d, &r), &c) in dans.iter_mut().zip(&traits_reference).zip(&traits_candidat) {
        if r && c {
            *d = false;
        }
    }
    let aire_boites = dans.iter().filter(|&&d| d).count() as u64;
    // Les halos d'ombre, hors des boîtes de texte (qui gardent leurs propres garde-fous).
    let masques = ombres::masques(ombres, largeur, hauteur);
    let halo: Vec<bool> = masques
        .halo
        .iter()
        .zip(&dans)
        .map(|(&h, &d)| h && !d)
        .collect();
    let aire_halos = halo.iter().filter(|&&h| h).count() as u64;
    let exception: Vec<bool> = masques
        .exception
        .iter()
        .zip(&dans)
        .map(|(&e, &d)| e && !d)
        .collect();
    // Les bandes des arcs, hors des boîtes de texte.
    let bande: Vec<bool> = arrondis::masque(arrondis, largeur, hauteur)
        .iter()
        .zip(&dans)
        .map(|(&b, &d)| b && !d)
        .collect();
    let aire_bandes = bande.iter().filter(|&&b| b).count() as u64;
    // Les bandes des arcs d'une exception nommée (à retirer par fr-p4.1) : l'élément de chaque pixel.
    let elements_arcs: Vec<Option<&'static str>> = arrondis
        .iter()
        .map(|a| exceptions::pour_arrondi(nommees, a, ombres))
        .collect();
    let mut bande_nommee: Vec<Option<&'static str>> = vec![None; dans.len()];
    for (a, element) in arrondis.iter().zip(&elements_arcs) {
        let Some(element) = element else { continue };
        for arc in arrondis::arcs(a) {
            for (x, y) in arrondis::pixels_bande(&arc, largeur, hauteur) {
                let i = (y * largeur + x) as usize;
                if !dans[i] {
                    bande_nommee[i] = Some(element);
                }
            }
        }
    }
    // Les côtés du contour de l'onglet actif, au delta ≤ 1 seulement (`exceptions::DELTA_COTES`).
    let mut cote_nomme: Vec<Option<&'static str>> = vec![None; dans.len()];
    for (a, element) in arrondis.iter().zip(&elements_arcs) {
        let Some(element) = element else { continue };
        for (x, y) in exceptions::pixels_cotes(a, largeur, hauteur) {
            let i = (y * largeur + x) as usize;
            if !dans[i] {
                cote_nomme[i] = Some(element);
            }
        }
    }
    let mut forts_nommes: std::collections::BTreeMap<&'static str, (u64, u8)> =
        std::collections::BTreeMap::new();
    let (mut differents_arrondi, mut delta_max_arrondi, mut forts_dans_bandes) = (0u64, 0u8, 0u64);
    let (mut forts_exception, mut delta_max_exception) = (0u64, 0u8);
    let (mut differents_ombre, mut delta_max_ombre, mut forts_dans_halos) = (0u64, 0u8, 0u64);
    let mut diff = RgbaImage::new(largeur, hauteur);
    let mut masque_hors = vec![false; dans.len()];
    let (mut differents, mut differents_hors, mut differents_boites) = (0u64, 0u64, 0u64);
    let mut delta_max_boites = 0u8;
    let (mut delta_max_hors, mut differents_hors_faibles) = (0u8, 0u64);
    for (x, y, r) in reference.enumerate_pixels() {
        let c = candidat.get_pixel(x, y);
        let delta = (0..4).map(|i| r.0[i].abs_diff(c.0[i])).max().unwrap_or(0);
        let i = (y * largeur + x) as usize;
        if delta == 0 {
            let g = (u16::from(r.0[0]) + u16::from(r.0[1]) + u16::from(r.0[2])) / 12;
            let g = u8::try_from(g).unwrap_or(u8::MAX);
            diff.put_pixel(x, y, Rgba([g, g, g, 255]));
            continue;
        }
        differents += 1;
        if dans[i] {
            differents_boites += 1;
            delta_max_boites = delta_max_boites.max(delta);
            // Orange : écart toléré dans une boîte ; rouge : hors boîte.
            diff.put_pixel(x, y, Rgba([255, 160, 0, 255]));
        } else if bande[i] && delta <= arrondis::DELTA_ARRONDI {
            differents_arrondi += 1;
            delta_max_arrondi = delta_max_arrondi.max(delta);
            // Violet : écart toléré dans la bande d'un arc.
            diff.put_pixel(x, y, Rgba([190, 0, 255, 255]));
        } else if halo[i] && delta <= ombres::DELTA_OMBRE {
            differents_ombre += 1;
            delta_max_ombre = delta_max_ombre.max(delta);
            // Bleu : écart toléré dans un halo d'ombre.
            diff.put_pixel(x, y, Rgba([0, 140, 255, 255]));
        } else if let Some(element) = bande_nommee[i] {
            let e = forts_nommes.entry(element).or_default();
            e.0 += 1;
            e.1 = e.1.max(delta);
            // Magenta : exception nommée temporaire de T1, mesurée par le cliquet.
            diff.put_pixel(x, y, Rgba([255, 0, 160, 255]));
        } else if let Some(element) = cote_nomme[i].filter(|_| delta <= exceptions::DELTA_COTES) {
            // Côté du contour de l'onglet actif, delta ≤ 1 : même élément, même cliquet.
            let e = forts_nommes.entry(element).or_default();
            e.0 += 1;
            e.1 = e.1.max(delta);
            diff.put_pixel(x, y, Rgba([255, 0, 160, 255]));
        } else if exception[i] && !bande[i] {
            forts_exception += 1;
            delta_max_exception = delta_max_exception.max(delta);
            // Cyan : exception nommée `--shadow-l`, mesurée par le cliquet.
            diff.put_pixel(x, y, Rgba([0, 230, 230, 255]));
        } else {
            forts_dans_bandes += u64::from(bande[i]);
            forts_dans_halos += u64::from(halo[i] && !bande[i]);
            differents_hors += 1;
            delta_max_hors = delta_max_hors.max(delta);
            differents_hors_faibles += u64::from(delta <= 2);
            masque_hors[i] = true;
            diff.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }
    let zones_hors = zones_connexes(&masque_hors, largeur, hauteur);
    let mut resultats = Vec::with_capacity(boites.len());
    for b in boites {
        let mesure = rect(b, PAD_MESURE, largeur, hauteur);
        let region_fond = rect(b, PAD_FOND, largeur, hauteur);
        let (fond_r, fond_c) = (fond(reference, region_fond), fond(candidat, region_fond));
        let k = (mesure.1..=mesure.3)
            .flat_map(|y| (mesure.0..=mesure.2).map(move |x| (x, y)))
            .map(|(x, y)| l1(*reference.get_pixel(x, y), fond_r))
            .max()
            .unwrap_or(0)
            .max(1);
        let k = f64::from(k);
        let mut res = juger(
            b,
            &mesurer(reference, mesure, fond_r, k),
            &mesurer(candidat, mesure, fond_c, k),
        );
        let tol = rect(b, PAD_TOL, largeur, hauteur);
        for y in tol.1..=tol.3 {
            for x in tol.0..=tol.2 {
                let (r, c) = (reference.get_pixel(x, y), candidat.get_pixel(x, y));
                let d = (0..4).map(|i| r.0[i].abs_diff(c.0[i])).max().unwrap_or(0);
                if d > 0 {
                    res.differents += 1;
                    res.delta_max = res.delta_max.max(d);
                }
            }
        }
        resultats.push(res);
    }
    // Les garde-fous d'ombre se mesurent hors des intérieurs et des boîtes de texte.
    let exclus: Vec<bool> = masques
        .interieur
        .iter()
        .zip(&dans)
        .map(|(&i, &d)| i || d)
        .collect();
    let resultats_ombres: Vec<ResultatOmbre> = ombres
        .iter()
        .map(|o| {
            let mut r = ombres::juger(reference, candidat, o, &halo, &exclus);
            r.nommee = exceptions::pour_ombre(nommees, o);
            r
        })
        .collect();
    let resultats_arrondis: Vec<ResultatArrondi> = arrondis
        .iter()
        .zip(&elements_arcs)
        .map(|(a, element)| {
            let mut r = arrondis::juger(reference, candidat, a, &dans);
            r.nommee = *element;
            r
        })
        .collect();
    // Les mesures des exceptions nommées, en valeur absolue : le pire de chaque élément.
    let mut mesures_nommees: std::collections::BTreeMap<
        &'static str,
        std::collections::BTreeMap<&'static str, f64>,
    > = std::collections::BTreeMap::new();
    let mut garder = |element: &'static str, cle: &'static str, v: f64| {
        let e = mesures_nommees
            .entry(element)
            .or_default()
            .entry(cle)
            .or_insert(0.0);
        *e = e.max(v.abs());
    };
    for o in &resultats_ombres {
        if let Some(element) = o.nommee {
            garder(element, "etalement", o.gardes.etalement);
            garder(element, "encre", o.gardes.encre);
            garder(element, "glissement", o.gardes.glissement);
        }
    }
    for a in &resultats_arrondis {
        if let Some(element) = a.nommee {
            let (forts, delta) = forts_nommes.get(element).copied().unwrap_or_default();
            garder(element, "deplacement", a.pire_aire());
            garder(element, "forts", forts as f64);
            garder(element, "delta_max", f64::from(delta));
        }
    }
    let gardes = resultats.iter().all(ResultatBoite::ok)
        && resultats_ombres.iter().all(|o| o.ok())
        && resultats_arrondis.iter().all(ResultatArrondi::ok);
    let verdict = if differents == 0 {
        Verdict::Strict
    } else if differents_hors == 0 && gardes {
        Verdict::Tolere
    } else {
        Verdict::Echec
    };
    // Une boîte qui échoue échoue aussi sans divergence de pixel (encre d'un seul côté).
    let verdict = if verdict == Verdict::Strict && !gardes {
        Verdict::Echec
    } else {
        verdict
    };
    Ok(Sas {
        differents_hors,
        zones_hors,
        delta_max_hors,
        differents_hors_faibles,
        differents_boites,
        delta_max_boites,
        aire_boites,
        boites: resultats,
        differents_ombre,
        delta_max_ombre,
        aire_halos,
        forts_dans_halos,
        ombres: resultats_ombres,
        forts_exception,
        delta_max_exception,
        differents_arrondi,
        delta_max_arrondi,
        aire_bandes,
        forts_dans_bandes,
        arrondis: resultats_arrondis,
        nommees: mesures_nommees,
        verdict,
        diff,
    })
}

/// Le nombre de boîtes par catégorie : (texte, icônes).
pub fn par_categorie(boites: &[BoiteEncre]) -> (usize, usize) {
    let icones = boites
        .iter()
        .filter(|b| b.categorie == Categorie::Icone)
        .count();
    (boites.len() - icones, icones)
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::boites::detecter;
    use super::*;

    const FOND: [u8; 3] = [30, 30, 30];
    pub(in crate::pixels) const ENCRE: [u8; 3] = [220, 220, 220];

    /// Glyphes de 5 × 7 pixels, agrandis ×2 : assez pour qu'une lettre changée se voie.
    fn glyphe(c: char) -> [&'static str; 7] {
        match c {
            'a' => [
                ".###.", "#...#", "....#", ".####", "#...#", "#...#", ".####",
            ],
            'n' => [
                "#.##.", "##..#", "#...#", "#...#", "#...#", "#...#", "#...#",
            ],
            'e' => [
                ".###.", "#...#", "#...#", "#####", "#....", "#...#", ".###.",
            ],
            'o' => [
                ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
            ],
            'p' => [
                "####.", "#...#", "#...#", "####.", "#....", "#....", "#....",
            ],
            'u' => [
                "#...#", "#...#", "#...#", "#...#", "#...#", "#..##", ".##.#",
            ],
            _ => [""; 7],
        }
    }

    /// Dessine `texte` en (x0, y0) ; `couverture` imite un trait plus fin (< 1), `decalage_fin`
    /// pousse la dernière lettre de n pixels (la dette de largeur).
    pub(in crate::pixels) fn ligne(
        texte: &str,
        couverture: f64,
        encre: [u8; 3],
        decalage_fin: u32,
    ) -> RgbaImage {
        let n = texte.chars().count();
        let decalages: Vec<i64> = (0..n)
            .map(|i| {
                if i + 1 == n {
                    i64::from(decalage_fin)
                } else {
                    0
                }
            })
            .collect();
        ligne_decalee(texte, couverture, encre, &decalages)
    }

    /// Comme [`ligne`], chaque lettre poussée de `decalages[i]` px en x.
    fn ligne_decalee(texte: &str, couverture: f64, encre: [u8; 3], decalages: &[i64]) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        for (i, c) in texte.chars().enumerate() {
            let gx = (20 + 14 * i as i64 + decalages[i]) as u32;
            for (r, rangee) in glyphe(c).iter().enumerate() {
                for (k, point) in rangee.chars().enumerate() {
                    if point != '#' {
                        continue;
                    }
                    let mut pixel = [0u8; 4];
                    for canal in 0..3 {
                        let (f, e) = (f64::from(FOND[canal]), f64::from(encre[canal]));
                        pixel[canal] = (f + couverture * (e - f)).round() as u8;
                    }
                    pixel[3] = 255;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        img.put_pixel(gx + 2 * k as u32 + dx, 20 + 2 * r as u32 + dy, Rgba(pixel));
                    }
                }
            }
        }
        img
    }

    fn sas(reference: &RgbaImage, candidat: &RgbaImage) -> Sas {
        comparer(reference, candidat, &detecter(candidat), &[]).unwrap()
    }

    fn motifs(s: &Sas) -> String {
        s.boites
            .iter()
            .flat_map(|b| b.violations.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ; ")
    }

    #[test]
    fn deux_images_identiques_sont_strictes() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let s = sas(&r, &r.clone());
        assert_eq!(s.verdict, Verdict::Strict);
        assert!(!s.boites.is_empty(), "le mot doit avoir sa boîte");
    }

    #[test]
    fn un_trait_plus_fin_est_tolere_dans_la_boite() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 0.85, ENCRE, 0);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
        assert_eq!(s.differents_hors, 0);
        assert!(s.differents_boites > 0 && s.delta_max_boites > 0);
    }

    /// ★ CONTRE-ÉPREUVE (ADR-009) : une lettre changée doit sortir rouge. « panneau » devient
    /// « ponneau » : même encre à peu près, même boîte, mais un trait manque et un autre est de trop.
    #[test]
    fn une_lettre_changee_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("ponneau", 0.85, ENCRE, 0);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert!(motifs(&s).contains("forme"), "{}", motifs(&s));
    }

    #[test]
    fn une_lettre_manquante_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("pa eau", 0.85, ENCRE, 0);
        assert_eq!(sas(&r, &c).verdict, Verdict::Echec);
    }

    #[test]
    fn une_autre_couleur_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 1.0, [220, 60, 60], 0);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert!(motifs(&s).contains("teinte"), "{}", motifs(&s));
    }

    #[test]
    fn un_pixel_hors_boite_reste_strict() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let mut c = r.clone();
        c.put_pixel(150, 55, Rgba([31, 30, 30, 255]));
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert_eq!(s.differents_hors, 1);
    }

    #[test]
    fn un_texte_decale_d_un_pixel_en_hauteur_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let mut c = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        for (x, y, p) in r.enumerate_pixels() {
            if y + 1 < 60 {
                c.put_pixel(x, y + 1, *p);
            }
        }
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        assert!(motifs(&s).contains("origine"), "{}", motifs(&s));
    }

    /// ★ CONTRE-ÉPREUVE (ADR-009, amendement de 12:00) : un passage déplacé d'1 px en x échoue par
    /// son barycentre, alors que ses quatre bords d'encre restent dans le ±1 px.
    #[test]
    fn un_passage_decale_d_un_pixel_en_x_fait_echouer() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne_decalee("panneau", 1.0, ENCRE, &[1; 7]);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        let m = motifs(&s);
        assert!(m.contains("barycentre (passage)"), "{m}");
        assert!(!m.contains("boîte d'encre"), "{m}");
    }

    /// Un côté poussé à gauche, l'autre à droite : le barycentre du passage ne bouge presque pas,
    /// ceux des moitiés bougent d'un pixel.
    #[test]
    fn une_moitie_decalee_ne_se_compense_pas_de_l_autre() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne_decalee("panneau", 1.0, ENCRE, &[-1, -1, -1, 0, 1, 1, 1]);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        let m = motifs(&s);
        assert!(
            m.contains("moitié gauche") && m.contains("moitié droite"),
            "{m}"
        );
        assert!(!m.contains("(passage)"), "{m}");
    }

    /// Le calage des fûts de GPUI : un trait net d'1 px (couverture 1,0) contre le même trait étalé
    /// par Chromium (0,15 / 0,7 / 0,15, même centre). Le bord à 10 % recule d'une colonne d'un seul
    /// côté ; le barycentre, lui, ne bouge pas : toléré.
    #[test]
    fn un_fut_cale_sur_la_grille_est_tolere() {
        let c = ligne("panneau", 1.0, ENCRE, 0);
        let mut r = c.clone();
        let (l, h) = c.dimensions();
        for y in 0..h {
            for x in 1..l - 1 {
                let couv = |x: u32| {
                    let p = c.get_pixel(x, y).0[0];
                    (f64::from(p) - f64::from(FOND[0])) / (f64::from(ENCRE[0]) - f64::from(FOND[0]))
                };
                let v = 0.15 * couv(x - 1) + 0.7 * couv(x) + 0.15 * couv(x + 1);
                let mut pixel = [255u8; 4];
                for canal in 0..3 {
                    let (f, e) = (f64::from(FOND[canal]), f64::from(ENCRE[canal]));
                    pixel[canal] = (f + v * (e - f)).round() as u8;
                }
                r.put_pixel(x, y, Rgba(pixel));
            }
        }
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
        let bords = s.boites[0].ecart_bords.unwrap();
        assert_eq!(
            bords[0], 1,
            "le bord à 10 % recule d'une colonne : {bords:?}"
        );
    }

    /// ★ CONTRE-ÉPREUVE sur une vraie capture : dans la référence de `t1-menu-dark`, chaque passage
    /// de texte déplacé d'1 px (à droite, puis en bas) fait échouer son barycentre ; les autres
    /// passages, intacts, restent stricts.
    #[test]
    fn un_passage_de_la_reference_deplace_d_un_pixel_fait_echouer() {
        let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pixels/reference/t1-menu-dark.png");
        let r = image::open(chemin)
            .map(|i| i.to_rgba8())
            .unwrap_or_else(|e| panic!("référence illisible : {e}"));
        let boites = detecter(&r);
        let textes: Vec<&BoiteEncre> = boites
            .iter()
            .filter(|b| b.categorie == Categorie::Texte)
            .collect();
        assert!(textes.len() >= 5, "{boites:?}");
        for b in textes {
            for (dx, dy) in [(1u32, 0u32), (0, 1)] {
                let mut c = r.clone();
                let (x0, y0, x1, y1) = rect(b, PAD_MESURE, r.width(), r.height());
                for y in (y0..=y1).rev() {
                    for x in (x0..=x1).rev() {
                        let source = *r.get_pixel(x - dx, y - dy);
                        c.put_pixel(x, y, source);
                    }
                }
                let s = comparer(&r, &c, &boites, &[]).unwrap();
                assert_eq!(s.verdict, Verdict::Echec, "{b:?} poussé de ({dx}, {dy})");
                let fautive = s.boites.iter().find(|x| x.boite == *b).unwrap();
                let m = fautive.violations.join(" ; ");
                assert!(
                    m.contains("barycentre (passage)"),
                    "{b:?} ({dx}, {dy}) : {m}"
                );
                assert!(!m.contains("boîte d'encre"), "{b:?} ({dx}, {dy}) : {m}");
            }
        }
    }

    #[test]
    fn un_pixel_de_largeur_en_plus_est_une_dette_toleree() {
        let r = ligne("panneau", 1.0, ENCRE, 0);
        let c = ligne("panneau", 1.0, ENCRE, 1);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Tolere, "{}", motifs(&s));
        assert_eq!(s.dettes_largeur(), 1);
        // Deux pixels : au-delà du ±1 de l'ADR.
        let c2 = ligne("panneau", 1.0, ENCRE, 2);
        assert_eq!(sas(&r, &c2).verdict, Verdict::Echec);
    }

    #[test]
    fn de_l_encre_d_un_seul_cote_fait_echouer() {
        let r = RgbaImage::from_pixel(160, 60, Rgba([FOND[0], FOND[1], FOND[2], 255]));
        let c = ligne("panneau", 1.0, ENCRE, 0);
        // Le candidat déclare ses boîtes : la référence, vide, n'a pas d'encre.
        assert_eq!(sas(&r, &c).verdict, Verdict::Echec);
    }

    /// `img` avec un trait horizontal de 1 px sur la rangée `y`, de x = 10 à 149.
    fn avec_trait_horizontal(mut img: RgbaImage, y: u32) -> RgbaImage {
        for x in 10..150 {
            img.put_pixel(x, y, Rgba([90, 90, 90, 255]));
        }
        img
    }

    /// `img` avec une bordure verticale de 1 px sur la colonne `x`, sur toute la hauteur.
    fn avec_bordure_verticale(mut img: RgbaImage, x: u32) -> RgbaImage {
        for y in 0..img.height() {
            img.put_pixel(x, y, Rgba([90, 90, 90, 255]));
        }
        img
    }

    /// ★ CONTRE-ÉPREUVE (fr-t1.5) : le bas des lettres (rangée 33) touche un trait (rangée 34) ;
    /// le texte garde sa boîte, mais le trait décalé d'1 px reste hors boîte et fait échouer.
    #[test]
    fn un_trait_decale_contre_un_texte_fait_echouer() {
        let r = avec_trait_horizontal(ligne("panneau", 1.0, ENCRE, 0), 34);
        let ok = sas(
            &r,
            &avec_trait_horizontal(ligne("panneau", 0.85, ENCRE, 0), 34),
        );
        assert_eq!(ok.verdict, Verdict::Tolere, "{}", motifs(&ok));
        assert!(
            !ok.boites.is_empty(),
            "le texte posé sur le trait garde sa boîte"
        );
        let c = avec_trait_horizontal(ligne("panneau", 1.0, ENCRE, 0), 35);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        // Les deux rangées du trait divergent sur toute sa longueur, sous les lettres comprises.
        assert!(
            s.differents_hors >= 2 * 140 - 2 * 14,
            "{}",
            s.differents_hors
        );
    }

    /// ★ CONTRE-ÉPREUVE (fr-t1.5, le A de « Aucun » contre le ruban) : la boîte du « a » gonflée
    /// d'1 px couvre la bordure qu'il touche ; la bordure décalée d'1 px sous le « a » échoue.
    #[test]
    fn une_bordure_decalee_sous_le_a_fait_echouer() {
        // Le « a » commence en x = 20 : la bordure en x = 19 touche son flanc gauche.
        let r = avec_bordure_verticale(ligne("anneau", 1.0, ENCRE, 0), 19);
        let ok = sas(
            &r,
            &avec_bordure_verticale(ligne("anneau", 0.85, ENCRE, 0), 19),
        );
        assert_eq!(ok.verdict, Verdict::Tolere, "{}", motifs(&ok));
        assert!(
            ok.boites.iter().any(|b| b.boite.x <= 21),
            "le « a » garde sa boîte : {:?}",
            ok.boites.iter().map(|b| b.boite).collect::<Vec<_>>()
        );
        let c = avec_bordure_verticale(ligne("anneau", 1.0, ENCRE, 0), 18);
        let s = sas(&r, &c);
        assert_eq!(s.verdict, Verdict::Echec);
        // Les deux colonnes divergent sur toute la hauteur, à côté du « a » compris.
        assert_eq!(s.differents_hors, 2 * 60);
    }

    #[test]
    fn des_tailles_differentes_sont_une_erreur() {
        let a = RgbaImage::new(2, 2);
        let b = RgbaImage::new(3, 2);
        assert!(comparer(&a, &b, &[], &[]).is_err());
    }
}
