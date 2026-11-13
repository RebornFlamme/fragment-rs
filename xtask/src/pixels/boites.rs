//! Les boîtes de texte et d'icônes d'une capture candidate (ADR-009).
//!
//! ★ Ce fichier est COMPILÉ DEUX FOIS : par `xtask` (`mod boites;` de `pixels.rs`) et par
//! `fragment-pixels` (`#[path]` dans `fragment-app/src/pixels/capture.rs`, même procédé que
//! `mod.rs`). Il ne dépend donc que de `image` et `serde_json`, et d'aucun autre module.
//!
//! Le candidat déclare ses boîtes dans un fichier voisin du PNG (`<scène>-<thème>.boites.json`).
//!
//! **Boîtes exactes (fr-p3.1, version 4)** : `fragment-pixels` lit la scène PEINTE de GPUI
//! (`Window::rendered_scene`, feature `bench-support`) et y prend le rectangle de chaque sprite de
//! glyphe et d'icône (voir `fragment-app/src/pixels/capture.rs`). Chaque glyphe est déclaré
//! (`glyphes`) ; [`regrouper`] en fait les passages (`boites`). Une boîte ne dépend toujours que du
//! candidat, jamais de ce que Fragment dessine : le sas reste non circulaire.
//!
//! **Repli** ([`detecter`]) : sans fichier de déclaration, `xtask` déduit les boîtes de l'IMAGE
//! candidate seule (ancienne méthode, environ 2 lettres changées sur 3 vues sur du texte de 13 px).
//!
//! Méthode du repli : un pixel est « de bord » s'il diffère d'un voisin de plus de [`SEUIL_BORD`] ;
//! les traits droits de plus de [`RUN_UNIFORME`] pixels DE COULEUR UNIFORME (bordures, séparateurs,
//! bords de fonds : les deux pixels de part et d'autre du bord identiques tout le long) sont
//! écartés : c'est de la géométrie, donc strict (le sas les retire aussi des boîtes gonflées, voir
//! [`pixels_de_trait`]). Une composante qui touchait un trait en est privée et son reste n'est
//! gardé que s'il n'est pas lui-même géométrie : ni au bout d'un trait (arc d'un coin arrondi), ni
//! de 1 px d'épaisseur, ni porteur d'une suite uniforme de [`SEGMENT_MIN`] px (fr-t1.5). Les
//! composantes restantes (traits de glyphes, d'icônes) sont regroupées quand leurs boîtes sont proches ([`ECART_H`], [`ECART_V`]) : un mot, une ligne,
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
/// Longueur maximale d'une coupure de trait par un glyphe (voir [`marquer_traits`]).
pub const COUPURE_MAX: usize = 16;
/// Longueur d'une suite uniforme qui fait d'un reste de composante un morceau de géométrie : le
/// contour du fond arrondi d'un bouton de 28 px (côtés uniformes de 18 px, trop courts pour être
/// des traits) ; plus longue que les fûts des lettres qui touchent une bordure (14 px dans les tests
/// du sas, à 12 px les lettres de ces tests devenaient de la géométrie).
pub const SEGMENT_MIN: usize = 16;
/// Épaisseur (en pixels de bord) d'un trait de 1 px : la ligne et ses deux voisins.
pub const EPAISSEUR_TRAIT: u32 = 3;
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

/// Ce que la détection lit d'une image : pixels de bord, pixels de trait et bouts de trait.
struct Analyse {
    /// Pixels qui diffèrent d'un voisin de plus de [`SEUIL_BORD`].
    bord: Vec<bool>,
    /// Les deux pixels de chaque paire de bord d'un trait droit uniforme (voir [`marquer_traits`]).
    traits: Vec<bool>,
    /// Pixels juste au-delà des extrémités d'un trait : ce qui y touche le prolonge (arc d'un coin).
    bouts: Vec<bool>,
    /// Pixels des suites uniformes d'au moins [`SEGMENT_MIN`] positions (traits compris).
    segments: Vec<bool>,
}

/// Les pixels de trait de `image` : géométrie, que la boîte d'un texte voisin ne tolère pas.
///
/// Le sas (`sas.rs`) les retire de ses boîtes de texte, mesurés sur la référence ET le candidat.
pub fn pixels_de_trait(image: &RgbaImage) -> Vec<bool> {
    analyser(image).traits
}

fn analyser(image: &RgbaImage) -> Analyse {
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
    let mut traits = vec![false; n];
    let mut bouts = vec![false; n];
    let mut segments = vec![false; n];
    let pixel = |i: usize| *image.get_pixel((i % l) as u32, (i / l) as u32);
    for y in 0..h.saturating_sub(1) {
        let ligne: Vec<usize> = (0..l).map(|x| y * l + x).collect();
        marquer_traits(
            &ligne,
            &dessous,
            &bord,
            &pixel,
            (l, n),
            &mut traits,
            &mut bouts,
        );
        marquer_segments(&ligne, &dessous, &pixel, l, &mut segments);
    }
    for x in 0..l.saturating_sub(1) {
        let colonne: Vec<usize> = (0..h).map(|y| y * l + x).collect();
        marquer_traits(
            &colonne,
            &droite,
            &bord,
            &pixel,
            (1, n),
            &mut traits,
            &mut bouts,
        );
        marquer_segments(&colonne, &droite, &pixel, 1, &mut segments);
    }
    Analyse {
        bord,
        traits,
        bouts,
        segments,
    }
}

/// Les boîtes d'encre de `image`, triées par (y, x).
pub fn detecter(image: &RgbaImage) -> Vec<BoiteEncre> {
    let (largeur, hauteur) = image.dimensions();
    let l = largeur as usize;
    let n = l * hauteur as usize;
    let Analyse {
        bord,
        traits,
        bouts,
        segments,
    } = analyser(image);
    // Composantes 8-connexes du masque de bord, PRIVÉ DES TRAITS (décision du chef, fr-t1.5) :
    // un glyphe qui longe une bordure (le A de « Aucun » contre le ruban, le haut de « New Tab »
    // sous le bord du menu) reste du texte ; le trait, lui, n'entre dans aucune composante.
    // Un reste qui touchait un trait n'est gardé que s'il n'est pas lui-même un trait :
    // ★ ce qui touche le BOUT d'un trait le prolonge (l'arc d'un coin arrondi) ; un reste de 1 px
    // d'épaisseur est une ligne (le séparateur dégradé d'un onglet, posé sur le bord du menu) ; un
    // reste qui porte une suite uniforme de [`SEGMENT_MIN`] px est un contour (le fond arrondi d'un
    // bouton, aux côtés de 18 px, collé à la bordure du ruban). Tout cela reste géométrie.
    let mut vu = vec![false; n];
    let mut groupes: Vec<Groupe> = Vec::new();
    let mut pile = Vec::new();
    for depart in 0..n {
        if !bord[depart] || traits[depart] || vu[depart] {
            continue;
        }
        let mut g = Groupe::vide();
        let (mut geometrie, mut touche, mut segment) = (false, false, false);
        vu[depart] = true;
        pile.push(depart);
        while let Some(i) = pile.pop() {
            let (x, y) = ((i % l) as u32, (i / l) as u32);
            g.ajouter(x, y);
            geometrie |= bouts[i];
            segment |= segments[i];
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
                    if nx < 0 || ny < 0 || nx >= i64::from(largeur) || ny >= i64::from(hauteur) {
                        continue;
                    }
                    let j = ny as usize * l + nx as usize;
                    touche |= traits[j];
                    if bord[j] && !traits[j] && !vu[j] {
                        vu[j] = true;
                        pile.push(j);
                    }
                }
            }
        }
        let fin = (g.x1 - g.x0 + 1).min(g.y1 - g.y0 + 1) <= EPAISSEUR_TRAIT;
        if !geometrie && !(touche && (segment || fin)) {
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

/// Marque les traits d'une `ligne` de pixels (une rangée ou une colonne, dans l'ordre).
///
/// Un trait est une suite d'au moins [`RUN_UNIFORME`] positions vraies de `masque` dont la `cle`
/// (la paire de pixels de part et d'autre du bord, le second à `voisin` du premier) ne change pas.
/// ★ Un glyphe qui touche le trait le coupe : la clé change sous l'encre et laisse des bouts de
/// suite trop courts contre lui. Les positions de même clé reprises après une coupure d'au plus
/// [`COUPURE_MAX`] positions, toutes de bord (l'encre et la bordure le sont ; le fond qui sépare
/// deux boîtes ne l'est pas), appartiennent au même trait. Les deux pixels de chaque paire du
/// trait vont dans `traits`, et sous une coupure le pixel qui garde la couleur du trait ; les pixels juste au-delà de ses deux extrémités (et leurs voisins
/// perpendiculaires) vont dans `bouts`.
fn marquer_traits(
    ligne: &[usize],
    masque: &[bool],
    bord: &[bool],
    pixel: &impl Fn(usize) -> image::Rgba<u8>,
    (voisin, n): (usize, usize),
    traits: &mut [bool],
    bouts: &mut [bool],
) {
    type Cle = (image::Rgba<u8>, image::Rgba<u8>);
    let cle = |i: usize| (pixel(i), pixel(i + voisin));
    let cles: Vec<Option<Cle>> = ligne.iter().map(|&i| masque[i].then(|| cle(i))).collect();
    // Les suites uniformes assez longues : leurs positions, et leurs clés.
    let mut longue = vec![false; cles.len()];
    let mut cles_longues: Vec<Cle> = Vec::new();
    let mut debut = 0;
    for p in 1..=cles.len() {
        if p < cles.len() && cles[p].is_some() && cles[p] == cles[debut] {
            continue;
        }
        if let Some(k) = cles[debut]
            && p - debut >= RUN_UNIFORME as usize
        {
            longue[debut..p].iter_mut().for_each(|v| *v = true);
            if !cles_longues.contains(&k) {
                cles_longues.push(k);
            }
        }
        debut = p;
    }
    let mut fermer = |(a, d, avec_longue): (usize, usize, bool), k: Cle| {
        if !avec_longue {
            return;
        }
        for &i in &ligne[a..=d] {
            // Sous une coupure, le côté que l'encre ne couvre pas garde sa couleur : il reste du
            // trait (la bordure sous le pied d'une lettre).
            let j = i + voisin;
            traits[i] |= pixel(i) == k.0;
            traits[j] |= pixel(j) == k.1;
        }
        let au_dela = [a.checked_sub(1), Some(d + 1).filter(|&e| e < ligne.len())];
        for e in au_dela.into_iter().flatten() {
            let i = ligne[e];
            let perpendiculaires = [
                i.checked_sub(voisin),
                Some(i),
                Some(i + voisin),
                Some(i + 2 * voisin),
            ];
            for j in perpendiculaires.into_iter().flatten().filter(|&j| j < n) {
                bouts[j] = true;
            }
        }
    };
    for &k in &cles_longues {
        // L'amas courant : première et dernière position de clé `k`, et s'il contient une suite
        // longue (sinon ce n'est qu'une coïncidence de couleurs, pas un trait).
        let mut amas: Option<(usize, usize, bool)> = None;
        let mut dernier_sans_bord: Option<usize> = None;
        for (p, &i) in ligne.iter().enumerate() {
            if !bord[i] && !bord[i + voisin] {
                dernier_sans_bord = Some(p);
            }
            if cles[p] != Some(k) {
                continue;
            }
            amas = match amas {
                Some((a, d, lg))
                    if p - d - 1 <= COUPURE_MAX && dernier_sans_bord.is_none_or(|s| s < d) =>
                {
                    Some((a, p, lg || longue[p]))
                }
                precedent => {
                    if let Some(precedent) = precedent {
                        fermer(precedent, k);
                    }
                    Some((p, p, longue[p]))
                }
            };
        }
        if let Some(amas) = amas {
            fermer(amas, k);
        }
    }
}

/// Marque dans `segments` les pixels des suites d'au moins [`SEGMENT_MIN`] positions vraies de
/// `masque` le long de `ligne` dont la paire de pixels (le second à `voisin`) ne change pas.
fn marquer_segments(
    ligne: &[usize],
    masque: &[bool],
    pixel: &impl Fn(usize) -> image::Rgba<u8>,
    voisin: usize,
    segments: &mut [bool],
) {
    let cle = |p: usize| masque[ligne[p]].then(|| (pixel(ligne[p]), pixel(ligne[p] + voisin)));
    let mut debut = 0;
    for p in 1..=ligne.len() {
        if p < ligne.len() && cle(p).is_some() && cle(p) == cle(debut) {
            continue;
        }
        if cle(debut).is_some() && p - debut >= SEGMENT_MIN {
            for &i in &ligne[debut..p] {
                segments[i] = true;
            }
        }
        debut = p;
    }
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

/// Les passages de texte et les icônes d'une liste de sprites peints (un rectangle par glyphe ou par
/// icône, en pixels physiques), triés par (y, x).
///
/// Une icône reste seule. Deux glyphes sont du même passage quand leurs lignes se recouvrent (au moins
/// une rangée commune) et qu'au plus [`ECART_H`] colonnes les séparent : un mot, une ligne. ★ Pas
/// d'écart vertical comme pour le repli : deux lignes de texte ne se touchent jamais, alors qu'un
/// jambage descendant et la hampe de la ligne suivante peuvent venir à 2 px l'un de l'autre.
pub fn regrouper(sprites: &[BoiteEncre]) -> Vec<BoiteEncre> {
    let glyphes: Vec<&BoiteEncre> = sprites
        .iter()
        .filter(|s| s.categorie == Categorie::Texte && s.largeur > 0 && s.hauteur > 0)
        .collect();
    // Union-find sur les glyphes, par paires (quelques milliers de glyphes au plus par capture).
    let mut parent: Vec<usize> = (0..glyphes.len()).collect();
    fn racine(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let fin = |b: &BoiteEncre| (b.x + b.largeur - 1, b.y + b.hauteur - 1);
    for i in 0..glyphes.len() {
        let (a, (ax1, ay1)) = (glyphes[i], fin(glyphes[i]));
        for (j, &b) in glyphes.iter().enumerate().skip(i + 1) {
            let (bx1, by1) = fin(b);
            let lignes = a.y <= by1 && b.y <= ay1;
            let colonnes = a.x <= bx1 + 1 + ECART_H && b.x <= ax1 + 1 + ECART_H;
            if lignes && colonnes {
                let (ra, rb) = (racine(&mut parent, i), racine(&mut parent, j));
                parent[ra] = rb;
            }
        }
    }
    let mut passages: std::collections::BTreeMap<usize, Groupe> = std::collections::BTreeMap::new();
    for (i, g) in glyphes.iter().enumerate() {
        let r = racine(&mut parent, i);
        let groupe = passages.entry(r).or_insert_with(Groupe::vide);
        let (x1, y1) = fin(g);
        groupe.ajouter(g.x, g.y);
        groupe.ajouter(x1, y1);
    }
    let mut boites: Vec<BoiteEncre> = passages
        .values()
        .map(|g| BoiteEncre {
            categorie: Categorie::Texte,
            x: g.x0,
            y: g.y0,
            largeur: g.x1 - g.x0 + 1,
            hauteur: g.y1 - g.y0 + 1,
        })
        .chain(
            sprites
                .iter()
                .filter(|s| s.categorie == Categorie::Icone && s.largeur > 0 && s.hauteur > 0)
                .copied(),
        )
        .collect();
    boites.sort_by_key(|b| (b.y, b.x, b.largeur, b.hauteur));
    boites.dedup();
    boites
}

/// Le fichier de déclaration voisin du PNG candidat, sans ombres.
pub fn serialiser(boites: &[BoiteEncre]) -> String {
    serialiser_tout(boites, &[], &[], &[], &[])
}

/// Le fichier de déclaration voisin du PNG candidat : boîtes de texte et d'icônes, glyphes peints,
/// glyphes ROGNÉS par un rectangle de découpe (`rognes`, sous-liste des glyphes), surfaces ombrées et
/// boîtes arrondies (version 4).
pub fn serialiser_tout(
    boites: &[BoiteEncre],
    glyphes: &[BoiteEncre],
    rognes: &[BoiteEncre],
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
    // Un glyphe par ligne du fichier, [x, y, largeur, hauteur] : la liste est longue.
    let glyphes: Vec<Value> = glyphes
        .iter()
        .map(|g| json!([g.x, g.y, g.largeur, g.hauteur]))
        .collect();
    let rognes: Vec<Value> = rognes
        .iter()
        .map(|g| json!([g.x, g.y, g.largeur, g.hauteur]))
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
    let json = json!({
        "version": 4, "boites": liste, "glyphes": glyphes, "rognes": rognes, "ombres": ombres, "arrondis": arrondis,
    });
    serde_json::to_string_pretty(&json).unwrap_or_default() + "\n"
}

/// Relit les glyphes peints d'un fichier de déclaration (aucun avant la version 4 : boîtes déduites).
pub fn deserialiser_glyphes(texte: &str) -> Option<Vec<BoiteEncre>> {
    deserialiser_rects(texte, "glyphes")
}

/// Relit les glyphes rognés par un rectangle de découpe (aucun dans un fichier sans la clé).
pub fn deserialiser_rognes(texte: &str) -> Option<Vec<BoiteEncre>> {
    deserialiser_rects(texte, "rognes")
}

/// Une liste de rectangles [x, y, largeur, hauteur] de glyphes, sous la clé `cle`.
fn deserialiser_rects(texte: &str, cle: &str) -> Option<Vec<BoiteEncre>> {
    let json: Value = serde_json::from_str(texte).ok()?;
    let Some(liste) = json.get(cle) else {
        return Some(Vec::new());
    };
    liste
        .as_array()?
        .iter()
        .map(|g| {
            let g = g.as_array()?;
            let champ = |i: usize| g.get(i)?.as_u64().and_then(|v| u32::try_from(v).ok());
            Some(BoiteEncre {
                categorie: Categorie::Texte,
                x: champ(0)?,
                y: champ(1)?,
                largeur: champ(2)?,
                hauteur: champ(3)?,
            })
        })
        .collect()
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

    /// Un « A » de 5 × 7 px agrandi ×2, d'encre anticrénelée, dont le coin haut gauche est (x0, y0).
    fn lettre_a(img: &mut RgbaImage, x0: u32, y0: u32) {
        let motif = [
            "..#..", ".#.#.", "#...#", "#...#", "#####", "#...#", "#...#",
        ];
        for (r, rangee) in motif.iter().enumerate() {
            for (k, point) in rangee.chars().enumerate() {
                if point != '#' {
                    continue;
                }
                let encre = 150 + (((r * 5 + k) * 29) % 70) as u8;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (x, y) = (x0 + 2 * k as u32 + dx, y0 + 2 * r as u32 + dy);
                    img.put_pixel(x, y, Rgba([encre, encre, encre, 255]));
                }
            }
        }
    }

    /// ★ fr-t1.5 : le A de « Aucun » touche la bordure verticale du ruban. La composante est privée
    /// du trait qu'elle longe : le A reste du texte, la bordure n'entre pas dans sa boîte.
    #[test]
    fn un_glyphe_qui_longe_une_bordure_reste_du_texte() {
        let mut img = fond(80, 60);
        for y in 0..60 {
            img.put_pixel(19, y, Rgba([90, 90, 90, 255]));
        }
        // Les pieds du A (colonne 20) touchent la bordure (colonne 19).
        lettre_a(&mut img, 20, 20);
        let boites = detecter(&img);
        assert_eq!(boites.len(), 1, "{boites:?}");
        assert!(
            boites[0].x >= 20,
            "la bordure n'est pas du texte : {boites:?}"
        );
        let traits = pixels_de_trait(&img);
        assert!(
            (0..60).all(|y| traits[y * 80 + 19]),
            "la bordure reste un trait"
        );
    }

    /// ★ fr-t1.5 : le haut de « New Tab » passe sous le bord haut du menu, qui coupe la suite
    /// uniforme en bouts trop courts : ces bouts restent du trait, les lettres du texte.
    #[test]
    fn un_passage_pose_sur_un_bord_reste_du_texte() {
        let mut img = fond(120, 60);
        for y in 30..60 {
            for x in 0..120 {
                img.put_pixel(x, y, Rgba([60, 60, 60, 255]));
            }
        }
        // Trois A dont la dernière rangée (y = 29) touche le bord (y = 30).
        for lettre in 0..3 {
            lettre_a(&mut img, 30 + 12 * lettre, 16);
        }
        let boites = detecter(&img);
        assert_eq!(boites.len(), 1, "{boites:?}");
        assert_eq!(boites[0].categorie, Categorie::Texte);
        assert!(boites[0].y + boites[0].hauteur <= 30, "{boites:?}");
        // Entre deux pieds, le fond posé sur le bord reste un pixel de trait.
        let traits = pixels_de_trait(&img);
        assert!(traits[29 * 120 + 34] && traits[30 * 120 + 34]);
    }

    /// Peint une boîte arrondie anticrénelée (x0, y0, x1, y1, rayon) : bordure de 1 px de valeur
    /// `bordure` (ou aucune), fond de valeur `remplissage`, sur le fond de l'image.
    fn boite_arrondie(
        img: &mut RgbaImage,
        (x0, y0, x1, y1, rayon): (f64, f64, f64, f64, f64),
        bordure: Option<f64>,
        remplissage: f64,
    ) {
        for y in 0..img.height() {
            for x in 0..img.width() {
                let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                // Distance au contour de la boîte arrondie (négative dedans).
                let (cx, cy) = (
                    px.clamp(x0 + rayon, x1 - rayon),
                    py.clamp(y0 + rayon, y1 - rayon),
                );
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - rayon;
                let dedans = (0.5 - d).clamp(0.0, 1.0);
                if dedans == 0.0 {
                    continue;
                }
                let fond = f64::from(img.get_pixel(x, y).0[0]);
                let mut v = fond + dedans * (remplissage - fond);
                if let Some(b) = bordure {
                    // Bordure de 1 px à l'intérieur du contour.
                    let couverture = (1.0 - (d + 0.5).abs()).clamp(0.0, 1.0);
                    v += couverture * (b - v);
                }
                let v = v.round() as u8;
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
    }

    /// ★ Piège : l'arc anticrénelé d'un coin arrondi prolonge les bordures ; privé des traits, il
    /// ne doit pas devenir du texte.
    #[test]
    fn un_coin_arrondi_au_bout_d_une_bordure_n_est_pas_du_texte() {
        let mut img = fond(120, 80);
        boite_arrondie(&mut img, (10.0, 10.0, 110.0, 70.0, 6.0), Some(90.0), 30.0);
        assert!(detecter(&img).is_empty(), "{:?}", detecter(&img));
    }

    /// ★ Piège (`t1-entete-vue`) : le fond arrondi d'un bouton, aux côtés trop courts pour être des
    /// traits, colle au trait de la bordure du dessus. Privé du trait, son contour reste géométrie.
    #[test]
    fn le_fond_arrondi_d_un_bouton_colle_a_une_bordure_reste_de_la_geometrie() {
        let mut img = fond(80, 60);
        for x in 0..80 {
            img.put_pixel(x, 9, Rgba([62, 62, 62, 255]));
        }
        boite_arrondie(&mut img, (20.0, 12.0, 48.0, 40.0, 6.0), None, 70.0);
        assert!(detecter(&img).is_empty(), "{:?}", detecter(&img));
    }

    /// ★ Piège (`t1-menu-onglet`) : un séparateur dégradé de 1 px (sa couleur change à chaque
    /// rangée, ce n'est pas un trait uniforme) posé sur une bordure reste de la géométrie.
    #[test]
    fn un_separateur_degrade_pose_sur_une_bordure_reste_de_la_geometrie() {
        let mut img = fond(80, 40);
        for x in 0..80 {
            img.put_pixel(x, 20, Rgba([76, 76, 76, 255]));
        }
        for y in 11..20 {
            let v = 70 - y as u8;
            img.put_pixel(40, y, Rgba([v, v, v, 255]));
        }
        assert!(detecter(&img).is_empty(), "{:?}", detecter(&img));
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

    fn sprite(categorie: Categorie, x: u32, y: u32, largeur: u32, hauteur: u32) -> BoiteEncre {
        BoiteEncre {
            categorie,
            x,
            y,
            largeur,
            hauteur,
        }
    }

    #[test]
    fn les_glyphes_peints_font_des_passages_et_les_icones_restent_seules() {
        use Categorie::{Icone, Texte};
        let sprites = [
            // « ab cd » sur une ligne : espace de 4 px entre « b » et « c ».
            sprite(Texte, 10, 20, 6, 9),
            sprite(Texte, 17, 18, 6, 11),
            sprite(Texte, 27, 22, 6, 7),
            sprite(Texte, 34, 18, 6, 11),
            // La ligne suivante, à 2 px sous le jambage de la première : un autre passage.
            sprite(Texte, 10, 31, 6, 9),
            // Un mot loin à droite sur la première ligne.
            sprite(Texte, 80, 20, 6, 9),
            // Une icône collée au texte reste seule.
            sprite(Icone, 42, 16, 16, 16),
        ];
        assert_eq!(
            regrouper(&sprites),
            vec![
                sprite(Icone, 42, 16, 16, 16),
                sprite(Texte, 10, 18, 30, 11),
                sprite(Texte, 80, 20, 6, 9),
                sprite(Texte, 10, 31, 6, 9),
            ]
        );
    }

    #[test]
    fn les_glyphes_declares_font_l_aller_retour() {
        let glyphes = vec![
            sprite(Categorie::Texte, 10, 20, 6, 9),
            sprite(Categorie::Texte, 17, 18, 6, 11),
        ];
        let rognes = vec![glyphes[0]];
        let texte = serialiser_tout(&regrouper(&glyphes), &glyphes, &rognes, &[], &[]);
        assert_eq!(deserialiser_glyphes(&texte), Some(glyphes));
        assert_eq!(deserialiser_rognes(&texte), Some(rognes));
        // Avant la version 4 : aucun glyphe déclaré.
        assert_eq!(
            deserialiser_glyphes(r#"{"version": 3, "boites": []}"#),
            Some(Vec::new())
        );
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
        let texte = serialiser_tout(&[], &[], &[], &ombres, &arrondis);
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
