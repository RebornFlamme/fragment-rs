//! Les boîtes de texte et d'icônes d'une capture candidate (ADR-009).
//!
//! ★ Ce fichier est COMPILÉ DEUX FOIS : par `xtask` (`mod boites;` de `pixels.rs`) et par
//! `fragment-pixels` (`#[path]` dans `fragment-app/src/pixels/capture.rs`, même procédé que
//! `mod.rs`). Il ne dépend donc que de `image` et `serde_json`, et d'aucun autre module.
//!
//! Le candidat déclare ses boîtes dans un fichier voisin du PNG (`<scène>-<thème>.boites.json`).
//! GPUI ne laisse pas lire les sprites de glyphes ni d'icônes d'une scène peinte (la scène de
//! `Window` est privée) : la déclaration est donc DÉRIVÉE DE L'IMAGE CANDIDATE SEULE, sans jamais
//! regarder la référence. C'est ce qui rend le sas non circulaire : une boîte ne dépend pas de
//! ce que Fragment dessine, et tout écart hors d'une boîte reste strict.
//!
//! Méthode : un pixel est « de bord » s'il diffère d'un voisin de plus de [`SEUIL_BORD`] ;
//! les traits droits de plus de [`RUN_UNIFORME`] pixels DE COULEUR UNIFORME (bordures, séparateurs,
//! bords de fonds : les deux pixels de part et d'autre du bord identiques tout le long) et
//! toute composante qui les touche (arcs des coins arrondis) sont écartés : c'est de la
//! géométrie, donc strict ; les composantes restantes (traits de glyphes, d'icônes) sont
//! regroupées quand leurs boîtes sont proches ([`ECART_H`], [`ECART_V`]) : un mot, une ligne,
//! une icône.

// ★ Deux crates compilent ce fichier et chacune n'en utilise qu'une partie (xtask lit et détecte,
// fragment-pixels détecte et écrit) : `dead_code` n'a pas de sens ici.
#![allow(dead_code)]

use image::RgbaImage;
use serde_json::{Value, json};

/// Différence minimale (canal le plus écarté) entre deux voisins pour parler de bord.
pub const SEUIL_BORD: u8 = 14;
/// Longueur à partir de laquelle une suite de bords de couleur UNIFORME est un trait droit.
/// ★ Plus courte que les 24 px de l'ancienne règle (sans uniformité) : le côté du bouton actif d'une barre d'outils (32 px, coins
/// arrondis) n'a que 20 px uniformes, les pixels anticrénelés de l'arc changeant de couleur. À 20,
/// les boîtes de toutes les captures de T1 restent celles de l'ancienne règle, plus le texte de la
/// barre d'état ; à 16, des icônes de `t1-zoom-150` deviennent de la géométrie.
pub const RUN_UNIFORME: u32 = 20;
/// Écart horizontal maximal entre deux composantes d'un même passage de texte.
pub const ECART_H: u32 = 5;
/// Écart vertical maximal entre deux composantes d'un même passage de texte.
pub const ECART_V: u32 = 2;
/// Nombre minimal de pixels de bord d'une boîte (en dessous : poussière, laissée au strict).
pub const MIN_PIXELS: usize = 8;
/// Côtés (inclus) d'une boîte d'icône : de 10 à 28 px, presque carrée.
pub const ICONE_COTE: std::ops::RangeInclusive<u32> = 10..=28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Categorie {
    Texte,
    Icone,
}

impl Categorie {
    pub fn nom(self) -> &'static str {
        match self {
            Categorie::Texte => "texte",
            Categorie::Icone => "icone",
        }
    }

    fn depuis_nom(nom: &str) -> Option<Self> {
        match nom {
            "texte" => Some(Categorie::Texte),
            "icone" => Some(Categorie::Icone),
            _ => None,
        }
    }
}

/// Une boîte d'encre déclarée par le candidat : rectangle de pixels (bords compris).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoiteEncre {
    pub categorie: Categorie,
    pub x: u32,
    pub y: u32,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Écart de canal maximal entre deux pixels (alpha compris).
fn delta(a: image::Rgba<u8>, b: image::Rgba<u8>) -> u8 {
    (0..4).map(|i| a.0[i].abs_diff(b.0[i])).max().unwrap_or(0)
}

/// Les boîtes d'encre de `image`, triées par (y, x).
pub fn detecter(image: &RgbaImage) -> Vec<BoiteEncre> {
    let (largeur, hauteur) = image.dimensions();
    let (l, h) = (largeur as usize, hauteur as usize);
    let n = l * h;
    // Différences avec le voisin de droite (traits verticaux) et du dessous (traits horizontaux).
    let mut droite = vec![false; n];
    let mut dessous = vec![false; n];
    for y in 0..hauteur {
        for x in 0..largeur {
            let i = (y * largeur + x) as usize;
            let p = *image.get_pixel(x, y);
            if x + 1 < largeur && delta(p, *image.get_pixel(x + 1, y)) >= SEUIL_BORD {
                droite[i] = true;
            }
            if y + 1 < hauteur && delta(p, *image.get_pixel(x, y + 1)) >= SEUIL_BORD {
                dessous[i] = true;
            }
        }
    }
    let mut bord = vec![false; n];
    for y in 0..h {
        for x in 0..l {
            let i = y * l + x;
            if droite[i] {
                bord[i] = true;
                bord[i + 1] = true;
            }
            if dessous[i] {
                bord[i] = true;
                bord[i + l] = true;
            }
        }
    }
    // Traits droits : suites de `dessous` en ligne, de `droite` en colonne, de couleur UNIFORME.
    // ★ Le bas des lettres d'un passage (« Fragment 0.0.0 ») aligne des bords sur 24 px et plus :
    // sans l'exigence d'uniformité, tout le texte touchait un « trait droit » et devenait strict.
    // La clé d'un bord est la paire de pixels qu'il sépare : une bordure, un séparateur, le bord
    // d'un fond la gardent constante ; l'encre anticrénelée d'un glyphe la change à chaque colonne.
    let mut droit = vec![false; n];
    let pixel = |i: usize| *image.get_pixel((i % l) as u32, (i / l) as u32);
    for y in 0..h {
        marquer_suites(
            (0..l).map(|x| y * l + x),
            &dessous,
            |i| (pixel(i), pixel(i + l)),
            &mut droit,
            RUN_UNIFORME as usize,
        );
    }
    for x in 0..l {
        marquer_suites(
            (0..h).map(|y| y * l + x),
            &droite,
            |i| (pixel(i), pixel(i + 1)),
            &mut droit,
            RUN_UNIFORME as usize,
        );
    }
    // Composantes 8-connexes du masque de bord ; une composante qui touche un trait droit est
    // de la géométrie.
    let mut vu = vec![false; n];
    let mut groupes: Vec<Groupe> = Vec::new();
    let mut pile = Vec::new();
    for depart in 0..n {
        if !bord[depart] || vu[depart] {
            continue;
        }
        let mut g = Groupe::vide();
        let mut geometrie = false;
        vu[depart] = true;
        pile.push(depart);
        while let Some(i) = pile.pop() {
            let (x, y) = ((i % l) as u32, (i / l) as u32);
            g.ajouter(x, y);
            geometrie |= droit[i];
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
                    if nx < 0 || ny < 0 || nx >= i64::from(largeur) || ny >= i64::from(hauteur) {
                        continue;
                    }
                    let j = ny as usize * l + nx as usize;
                    if bord[j] && !vu[j] {
                        vu[j] = true;
                        pile.push(j);
                    }
                }
            }
        }
        if !geometrie {
            groupes.push(g);
        }
    }
    // Fusion des groupes proches (point fixe, les groupes sont peu nombreux).
    loop {
        let mut fusion = None;
        'cherche: for a in 0..groupes.len() {
            for b in a + 1..groupes.len() {
                if groupes[a].proche(&groupes[b]) {
                    fusion = Some((a, b));
                    break 'cherche;
                }
            }
        }
        let Some((a, b)) = fusion else { break };
        let g = groupes.swap_remove(b);
        groupes[a].absorber(&g);
    }
    let mut boites: Vec<BoiteEncre> = groupes
        .into_iter()
        .filter(|g| g.pixels >= MIN_PIXELS)
        .map(|g| {
            let (largeur, hauteur) = (g.x1 - g.x0 + 1, g.y1 - g.y0 + 1);
            let icone = ICONE_COTE.contains(&largeur)
                && ICONE_COTE.contains(&hauteur)
                && largeur * 10 <= hauteur * 16
                && hauteur * 10 <= largeur * 16;
            BoiteEncre {
                categorie: if icone {
                    Categorie::Icone
                } else {
                    Categorie::Texte
                },
                x: g.x0,
                y: g.y0,
                largeur,
                hauteur,
            }
        })
        .collect();
    boites.sort_by_key(|b| (b.y, b.x));
    boites
}

/// Marque dans `dehors` les pixels des suites d'au moins `min` positions vraies de `masque` dont
/// la `cle` (la paire de pixels de part et d'autre du bord) ne change pas : un trait UNIFORME.
fn marquer_suites<K: PartialEq>(
    indices: impl Iterator<Item = usize>,
    masque: &[bool],
    cle: impl Fn(usize) -> K,
    dehors: &mut [bool],
    min: usize,
) {
    let mut courante: Vec<usize> = Vec::new();
    let mut cle_courante: Option<K> = None;
    let mut vider = |courante: &mut Vec<usize>| {
        if courante.len() >= min {
            for &i in courante.iter() {
                dehors[i] = true;
            }
        }
        courante.clear();
    };
    for i in indices {
        if !masque[i] {
            vider(&mut courante);
            cle_courante = None;
            continue;
        }
        let k = cle(i);
        if cle_courante.as_ref() != Some(&k) {
            vider(&mut courante);
            cle_courante = Some(k);
        }
        courante.push(i);
    }
    vider(&mut courante);
}

#[derive(Debug, Clone, Copy)]
struct Groupe {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    pixels: usize,
}

impl Groupe {
    fn vide() -> Self {
        Self {
            x0: u32::MAX,
            y0: u32::MAX,
            x1: 0,
            y1: 0,
            pixels: 0,
        }
    }

    fn ajouter(&mut self, x: u32, y: u32) {
        self.x0 = self.x0.min(x);
        self.y0 = self.y0.min(y);
        self.x1 = self.x1.max(x);
        self.y1 = self.y1.max(y);
        self.pixels += 1;
    }

    fn absorber(&mut self, autre: &Groupe) {
        self.x0 = self.x0.min(autre.x0);
        self.y0 = self.y0.min(autre.y0);
        self.x1 = self.x1.max(autre.x1);
        self.y1 = self.y1.max(autre.y1);
        self.pixels += autre.pixels;
    }

    /// Les boîtes, élargies de l'écart de regroupement, se recouvrent.
    fn proche(&self, autre: &Groupe) -> bool {
        self.x0 <= autre.x1 + ECART_H
            && autre.x0 <= self.x1 + ECART_H
            && self.y0 <= autre.y1 + ECART_V
            && autre.y0 <= self.y1 + ECART_V
    }
}

/// Une couche d'ombre déclarée, en pixels physiques : décalage, écart-type du flou, étendue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoucheDeclaree {
    pub decalage_x: f64,
    pub decalage_y: f64,
    pub sigma: f64,
    pub etendue: f64,
}

/// Une surface ombrée déclarée par le candidat (ADR-009, amendement du 11/11) : sa boîte de bordure
/// telle que la MISE EN PAGE l'a posée (pas lue dans l'image), son rayon de coin, ses couches et le
/// nom du jeton d'ombre (`--shadow-l`…), qui seul peut porter une exception nommée (2e amendement).
#[derive(Debug, Clone, PartialEq)]
pub struct OmbreDeclaree {
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
    pub rayon: f64,
    pub couches: Vec<CoucheDeclaree>,
    /// Le jeton CSS de l'ombre (`--menu-shadow`, `--shadow-l`…) ; vide s'il n'est pas déclaré.
    pub jeton: String,
}

/// Une boîte arrondie déclarée par le candidat (ADR-009, amendement de 11:15, point a) : sa boîte de
/// bordure posée par la mise en page, son rayon de coin et la largeur de sa bordure (l'arc intérieur
/// de la bordure a pour rayon `rayon - bordure`), en pixels physiques.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrondiDeclare {
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
    pub rayon: f64,
    pub bordure: f64,
    /// Les coins dont l'arc est déclaré, dans l'ordre haut gauche, haut droit, bas gauche, bas
    /// droit ([`TOUS_LES_COINS`] par défaut) : un arc isolé (pied concave de l'onglet actif, posé
    /// comme le coin d'une boîte voisine) ne déclare que le sien, les autres coins restent stricts.
    pub coins: [bool; 4],
    /// Le nom de l'élément, pour une exception nommée qui ne porte pas d'ombre (« contour
    /// d'onglet actif ») ; vide sinon.
    pub element: String,
}

/// Les quatre coins d'une boîte arrondie.
pub const TOUS_LES_COINS: [bool; 4] = [true; 4];

impl Default for ArrondiDeclare {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            largeur: 0.0,
            hauteur: 0.0,
            rayon: 0.0,
            bordure: 0.0,
            coins: TOUS_LES_COINS,
            element: String::new(),
        }
    }
}

/// Le fichier de déclaration voisin du PNG candidat, sans ombres.
pub fn serialiser(boites: &[BoiteEncre]) -> String {
    serialiser_tout(boites, &[], &[])
}

/// Le fichier de déclaration voisin du PNG candidat : boîtes de texte et d'icônes, surfaces
/// ombrées et boîtes arrondies (version 3).
pub fn serialiser_tout(
    boites: &[BoiteEncre],
    ombres: &[OmbreDeclaree],
    arrondis: &[ArrondiDeclare],
) -> String {
    let liste: Vec<Value> = boites
        .iter()
        .map(|b| {
            json!({
                "categorie": b.categorie.nom(),
                "x": b.x, "y": b.y, "largeur": b.largeur, "hauteur": b.hauteur,
            })
        })
        .collect();
    let ombres: Vec<Value> = ombres
        .iter()
        .map(|o| {
            let couches: Vec<Value> = o
                .couches
                .iter()
                .map(|c| {
                    json!({
                        "decalage_x": c.decalage_x, "decalage_y": c.decalage_y,
                        "sigma": c.sigma, "etendue": c.etendue,
                    })
                })
                .collect();
            json!({
                "x": o.x, "y": o.y, "largeur": o.largeur, "hauteur": o.hauteur,
                "rayon": o.rayon, "couches": couches, "jeton": o.jeton,
            })
        })
        .collect();
    let arrondis: Vec<Value> = arrondis
        .iter()
        .map(|a| {
            let mut v = json!({
                "x": a.x, "y": a.y, "largeur": a.largeur, "hauteur": a.hauteur,
                "rayon": a.rayon, "bordure": a.bordure,
            });
            // Clés facultatives : un fichier sans elles déclare les quatre coins, sans élément.
            if a.coins != TOUS_LES_COINS {
                v["coins"] = json!(a.coins);
            }
            if !a.element.is_empty() {
                v["element"] = json!(a.element);
            }
            v
        })
        .collect();
    let json = json!({ "version": 3, "boites": liste, "ombres": ombres, "arrondis": arrondis });
    serde_json::to_string_pretty(&json).unwrap_or_default() + "\n"
}

/// Relit les surfaces ombrées d'un fichier de déclaration (aucune pour un fichier de version 1).
pub fn deserialiser_ombres(texte: &str) -> Option<Vec<OmbreDeclaree>> {
    let json: Value = serde_json::from_str(texte).ok()?;
    let Some(liste) = json.get("ombres") else {
        return Some(Vec::new());
    };
    liste
        .as_array()?
        .iter()
        .map(|o| {
            let couches = o["couches"]
                .as_array()?
                .iter()
                .map(|c| {
                    Some(CoucheDeclaree {
                        decalage_x: c["decalage_x"].as_f64()?,
                        decalage_y: c["decalage_y"].as_f64()?,
                        sigma: c["sigma"].as_f64()?,
                        etendue: c["etendue"].as_f64()?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(OmbreDeclaree {
                x: o["x"].as_f64()?,
                y: o["y"].as_f64()?,
                largeur: o["largeur"].as_f64()?,
                hauteur: o["hauteur"].as_f64()?,
                rayon: o["rayon"].as_f64()?,
                couches,
                jeton: o["jeton"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// Relit les boîtes arrondies d'un fichier de déclaration (aucune avant la version 3 : leurs
/// coins restent alors stricts, l'arc ne se déduit jamais de l'image).
pub fn deserialiser_arrondis(texte: &str) -> Option<Vec<ArrondiDeclare>> {
    let json: Value = serde_json::from_str(texte).ok()?;
    let Some(liste) = json.get("arrondis") else {
        return Some(Vec::new());
    };
    liste
        .as_array()?
        .iter()
        .map(|a| {
            Some(ArrondiDeclare {
                x: a["x"].as_f64()?,
                y: a["y"].as_f64()?,
                largeur: a["largeur"].as_f64()?,
                hauteur: a["hauteur"].as_f64()?,
                rayon: a["rayon"].as_f64()?,
                bordure: a["bordure"].as_f64()?,
                coins: match a.get("coins") {
                    None => TOUS_LES_COINS,
                    Some(c) => {
                        let c = c.as_array()?;
                        let coin = |i: usize| c.get(i).and_then(Value::as_bool);
                        [coin(0)?, coin(1)?, coin(2)?, coin(3)?]
                    }
                },
                element: a["element"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// Relit un fichier de déclaration.
pub fn deserialiser(texte: &str) -> Option<Vec<BoiteEncre>> {
    let json: Value = serde_json::from_str(texte).ok()?;
    json["boites"]
        .as_array()?
        .iter()
        .map(|b| {
            let champ = |nom: &str| b[nom].as_u64().and_then(|v| u32::try_from(v).ok());
            Some(BoiteEncre {
                categorie: Categorie::depuis_nom(b["categorie"].as_str()?)?,
                x: champ("x")?,
                y: champ("y")?,
                largeur: champ("largeur")?,
                hauteur: champ("hauteur")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    pub(crate) fn fond(l: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(l, h, Rgba([30, 30, 30, 255]))
    }

    #[test]
    fn un_fond_uni_n_a_pas_de_boite() {
        assert!(detecter(&fond(80, 40)).is_empty());
    }

    #[test]
    fn une_bordure_et_ses_coins_sont_de_la_geometrie() {
        let mut img = fond(120, 80);
        for x in 10..110 {
            img.put_pixel(x, 10, Rgba([90, 90, 90, 255]));
            img.put_pixel(x, 69, Rgba([90, 90, 90, 255]));
        }
        for y in 10..70 {
            img.put_pixel(10, y, Rgba([90, 90, 90, 255]));
            img.put_pixel(109, y, Rgba([90, 90, 90, 255]));
        }
        assert!(detecter(&img).is_empty());
    }

    #[test]
    fn un_mot_donne_une_boite_de_texte() {
        let mut img = fond(120, 40);
        // Trois « lettres » de 5 × 9 px séparées de 3 px.
        for lettre in 0..3 {
            let x0 = 20 + lettre * 8;
            for y in 10..19 {
                img.put_pixel(x0, y, Rgba([220, 220, 220, 255]));
                img.put_pixel(x0 + 4, y, Rgba([220, 220, 220, 255]));
            }
            for x in x0..x0 + 5 {
                img.put_pixel(x, 14, Rgba([220, 220, 220, 255]));
            }
        }
        let boites = detecter(&img);
        assert_eq!(boites.len(), 1, "{boites:?}");
        assert_eq!(boites[0].categorie, Categorie::Texte);
        // Les voisins des pixels d'encre sont des bords : un pixel de plus de chaque côté.
        assert_eq!((boites[0].x, boites[0].y), (19, 9));
        assert_eq!((boites[0].largeur, boites[0].hauteur), (23, 11));
    }

    #[test]
    fn un_pictogramme_carre_est_une_icone() {
        let mut img = fond(60, 60);
        for i in 0..14 {
            img.put_pixel(20 + i, 20 + i, Rgba([220, 220, 220, 255]));
            img.put_pixel(33 - i, 20 + i, Rgba([220, 220, 220, 255]));
        }
        let boites = detecter(&img);
        assert_eq!(boites.len(), 1, "{boites:?}");
        assert_eq!(boites[0].categorie, Categorie::Icone);
    }

    /// Un passage dont le bas des lettres aligne des bords sur plus de 24 px, d'une
    /// encre qui varie d'une colonne à l'autre (anticrénelage) : c'est du texte, pas un trait.
    fn passage_a_ligne_d_encre(img: &mut RgbaImage, x0: u32, y0: u32) {
        for x in x0..x0 + 60 {
            let encre = 120 + ((x * 37) % 100) as u8;
            img.put_pixel(x, y0 + 8, Rgba([encre, encre, encre, 255]));
            if x % 4 == 0 {
                for y in y0..y0 + 8 {
                    img.put_pixel(x, y, Rgba([220, 220, 220, 255]));
                }
            }
        }
    }

    #[test]
    fn une_ligne_d_encre_non_uniforme_reste_du_texte() {
        let mut img = fond(120, 40);
        passage_a_ligne_d_encre(&mut img, 20, 10);
        let boites = detecter(&img);
        assert_eq!(boites.len(), 1, "{boites:?}");
        assert_eq!(boites[0].categorie, Categorie::Texte);
        assert!(boites[0].largeur >= 60, "{boites:?}");
    }

    /// Contre-épreuve : un vrai trait de 1 px, uniforme, reste de la géométrie (strict), même à
    /// côté d'un passage ; le décaler d'1 px fait donc échouer la comparaison hors boîte.
    #[test]
    fn un_trait_uniforme_de_1_px_reste_strict_et_son_decalage_echoue() {
        let trait_en = |y_trait: u32| {
            let mut img = fond(160, 60);
            passage_a_ligne_d_encre(&mut img, 20, 10);
            for x in 10..150 {
                img.put_pixel(x, y_trait, Rgba([90, 90, 90, 255]));
            }
            img
        };
        let (reference, candidat) = (trait_en(40), trait_en(41));
        let boites = detecter(&candidat);
        assert_eq!(boites.len(), 1, "{boites:?}");
        let b = boites[0];
        let dans =
            |x: u32, y: u32| x >= b.x && x < b.x + b.largeur && y >= b.y && y < b.y + b.hauteur;
        // Les pixels qui diffèrent entre les deux traits sont tous hors des boîtes.
        let hors: Vec<(u32, u32)> = (0..60)
            .flat_map(|y| (0..160).map(move |x| (x, y)))
            .filter(|&(x, y)| reference.get_pixel(x, y) != candidat.get_pixel(x, y))
            .filter(|&(x, y)| !dans(x, y))
            .collect();
        assert_eq!(
            hors.len(),
            2 * 140,
            "le trait décalé doit diverger hors boîte"
        );
    }

    #[test]
    fn la_declaration_fait_l_aller_retour() {
        let boites = vec![
            BoiteEncre {
                categorie: Categorie::Texte,
                x: 3,
                y: 4,
                largeur: 50,
                hauteur: 12,
            },
            BoiteEncre {
                categorie: Categorie::Icone,
                x: 70,
                y: 4,
                largeur: 16,
                hauteur: 16,
            },
        ];
        assert_eq!(deserialiser(&serialiser(&boites)), Some(boites));
        assert_eq!(deserialiser("pas du json"), None);
    }

    #[test]
    fn les_ombres_declarees_font_l_aller_retour() {
        let ombres = vec![OmbreDeclaree {
            x: 420.0,
            y: 260.5,
            largeur: 180.0,
            hauteur: 96.0,
            rayon: 8.0,
            couches: vec![CoucheDeclaree {
                decalage_x: 0.0,
                decalage_y: 8.0,
                sigma: 12.0,
                etendue: 0.0,
            }],
            jeton: "--menu-shadow".to_owned(),
        }];
        let arrondis = vec![
            ArrondiDeclare {
                x: 420.0,
                y: 260.5,
                largeur: 180.0,
                hauteur: 96.0,
                rayon: 8.0,
                bordure: 1.0,
                ..ArrondiDeclare::default()
            },
            ArrondiDeclare {
                x: 30.0,
                y: 4.0,
                largeur: 10.0,
                hauteur: 10.0,
                rayon: 4.5,
                bordure: 1.0,
                coins: [false, false, false, true],
                element: "contour d'onglet actif".to_owned(),
            },
        ];
        let texte = serialiser_tout(&[], &ombres, &arrondis);
        assert_eq!(deserialiser_ombres(&texte), Some(ombres));
        assert_eq!(deserialiser_arrondis(&texte), Some(arrondis));
        assert_eq!(
            deserialiser_arrondis(r#"{"version": 2, "boites": [], "ombres": []}"#),
            Some(Vec::new())
        );
        assert_eq!(deserialiser(&texte), Some(Vec::new()));
        // Un fichier de version 1 n'a pas d'ombres.
        assert_eq!(
            deserialiser_ombres(r#"{"version": 1, "boites": []}"#),
            Some(Vec::new())
        );
    }
}
