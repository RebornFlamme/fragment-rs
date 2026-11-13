// Port du moteur de « Thinking Orbs » (Jakub Antalik, MIT, voir LICENSE-THINKING-ORBS) :
// `engine/profiles.ts` et `presets.ts` de la bibliothèque d'origine. Traduits en Rust,
// commentaires réécrits.
//
// Les profils de base (densité « fine ») de chaque forme, les multiplicateurs de nombre et de
// rayon, et les réglages livrés : neuf états × trois tailles. Le dictionnaire de clés optionnelles
// du moteur devient une structure de champs `Option<f64>` : un champ absent prend la valeur par
// défaut écrite dans la forme elle-même, comme `o.cle ?? défaut`.

use fragment_agent::trace_model::OrbState;

use super::noyau::arrondi;

/// Les réglages d'une forme (les clés du moteur d'origine, en `snake_case`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Reglages {
    // Nombres : grilles (paires) et listes.
    pub lat_rings: Option<f64>,
    pub rings: Option<f64>,
    pub lon_density: Option<f64>,
    pub lanes: Option<f64>,
    pub segs: Option<f64>,
    pub orbit_n: Option<f64>,
    pub ghost_n: Option<f64>,
    pub node_n: Option<f64>,
    pub strand_n: Option<f64>,
    pub signals: Option<f64>,
    pub icon_d: Option<f64>,
    // Rayons.
    pub r_base: Option<f64>,
    pub r_depth: Option<f64>,
    pub r_active: Option<f64>,
    pub r_dot: Option<f64>,
    pub ghost_r: Option<f64>,
    pub part_r: Option<f64>,
    pub part_r_depth: Option<f64>,
    pub node_r: Option<f64>,
    pub node_r_depth: Option<f64>,
    pub r_size_mul: Option<f64>,
    // Le reste.
    pub r_boost: Option<f64>,
    pub ink_far: Option<f64>,
    pub ink_span: Option<f64>,
    pub rs_pow: Option<f64>,
    pub r_min: Option<f64>,
    pub ghost_a: Option<f64>,
    pub particles: Option<f64>,
    pub move_count: Option<f64>,
    pub thr: Option<f64>,
    pub line_w: Option<f64>,
    pub turns: Option<f64>,
    pub face_on: Option<f64>,
    pub scan_mul: Option<f64>,
    pub dim_base: Option<f64>,
    pub spin: Option<f64>,
    pub band_mul: Option<f64>,
    pub wob_mul: Option<f64>,
    pub spread: Option<f64>,
    pub shape: Option<f64>,
}

/// Multiplie les NOMBRES de points par `facteur`. Une grille (anneaux × points par anneau) prend
/// √facteur de chaque côté pour que son total suive `facteur` ; une liste suit `facteur` ; une
/// liste à 0 (couche absente, l'anneau n'a pas de sphère fantôme) reste à 0.
#[must_use]
pub fn echelonner_nombres(o: &Reglages, facteur: f64) -> Reglages {
    let mut r = *o;
    let racine = facteur.sqrt();
    let grille = |v: f64| arrondi(v * racine).max(2.0);
    // Une seule paire par profil porte `lon_density` : `lat_rings` d'abord, sinon `rings`.
    if let (Some(a), Some(b)) = (r.lat_rings, r.lon_density) {
        r.lat_rings = Some(grille(a));
        r.lon_density = Some(grille(b));
    } else if let (Some(a), Some(b)) = (r.rings, r.lon_density) {
        r.rings = Some(grille(a));
        r.lon_density = Some(grille(b));
    }
    if let (Some(a), Some(b)) = (r.lanes, r.segs) {
        r.lanes = Some(grille(a));
        r.segs = Some(grille(b));
    }
    for champ in [
        &mut r.orbit_n,
        &mut r.ghost_n,
        &mut r.node_n,
        &mut r.strand_n,
        &mut r.signals,
    ] {
        if let Some(v) = *champ
            && v != 0.0
        {
            *champ = Some(arrondi(v * facteur).max(1.0));
        }
    }
    if let Some(v) = r.icon_d {
        r.icon_d = Some((v * facteur).max(0.02));
    }
    r
}

/// Multiplie tous les RAYONS par `facteur` (la chute près / loin reste intacte) et retient le
/// facteur lui-même (`r_size_mul`).
#[must_use]
pub fn echelonner_rayons(o: &Reglages, facteur: f64) -> Reglages {
    let mut r = *o;
    for champ in [
        &mut r.r_base,
        &mut r.r_depth,
        &mut r.r_active,
        &mut r.r_dot,
        &mut r.ghost_r,
        &mut r.part_r,
        &mut r.part_r_depth,
        &mut r.node_r,
        &mut r.node_r_depth,
    ] {
        if let Some(v) = *champ {
            *champ = Some(v * facteur);
        }
    }
    r.r_size_mul = Some(r.r_size_mul.unwrap_or(1.0) * facteur);
    r
}

/// Le total de points d'un profil : les grilles en produit, les listes telles quelles.
#[must_use]
pub fn nombre_de_points(o: &Reglages) -> f64 {
    let mut total = 0.0;
    if let (Some(a), Some(b)) = (o.lat_rings, o.lon_density) {
        total += a * b;
    } else if let (Some(a), Some(b)) = (o.rings, o.lon_density) {
        total += a * b;
    }
    if let (Some(a), Some(b)) = (o.lanes, o.segs) {
        total += a * b;
    }
    for v in [o.orbit_n, o.ghost_n, o.node_n, o.strand_n, o.signals]
        .into_iter()
        .flatten()
    {
        total += v;
    }
    arrondi(total)
}

/// Les neuf formes du moteur (une par état ; l'anneau partage la géométrie de l'écharpe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Forme {
    Orbites,
    Globe,
    Rubik,
    Onde,
    Toile,
    Tresse,
    Echarpe,
    Anneau,
    Morph,
}

/// La forme de chaque état de l'agent.
#[must_use]
pub fn forme_de(etat: OrbState) -> Forme {
    match etat {
        OrbState::Working => Forme::Orbites,
        OrbState::Searching => Forme::Globe,
        OrbState::Solving => Forme::Rubik,
        OrbState::Listening => Forme::Onde,
        OrbState::Connecting => Forme::Toile,
        OrbState::Weaving => Forme::Tresse,
        OrbState::Composing => Forme::Echarpe,
        OrbState::Breathing => Forme::Anneau,
        OrbState::Shaping => Forme::Morph,
    }
}

/// Les tailles réglées : 64 et 20 à la main, 32 interpolée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Taille {
    T64,
    T32,
    T20,
}

impl Taille {
    /// Le côté en pixels logiques.
    #[must_use]
    pub fn cote(self) -> f64 {
        match self {
            Self::T64 => 64.0,
            Self::T32 => 32.0,
            Self::T20 => 20.0,
        }
    }

    /// La taille réglée la plus proche d'un côté quelconque.
    #[must_use]
    pub fn proche(cote: f32) -> Self {
        if cote >= 48.0 {
            Self::T64
        } else if cote >= 26.0 {
            Self::T32
        } else {
            Self::T20
        }
    }
}

/// Le profil de base (« fine ») d'une forme, avant les multiplicateurs.
#[must_use]
pub fn profil_de_base(forme: Forme) -> Reglages {
    let rs = Some(0.6);
    let r_min = Some(0.3);
    let base = Reglages::default();
    match forme {
        Forme::Globe => Reglages {
            lat_rings: Some(17.0),
            lon_density: Some(44.0),
            r_base: Some(0.6),
            r_depth: Some(1.7),
            r_boost: Some(1.0),
            ink_far: Some(0.62),
            ink_span: Some(0.54),
            rs_pow: rs,
            r_min,
            ..base
        },
        Forme::Orbites => Reglages {
            orbit_n: Some(12.0),
            ghost_n: Some(40.0),
            ghost_r: Some(0.9),
            ghost_a: Some(0.5),
            particles: Some(3.0),
            part_r: Some(1.2),
            part_r_depth: Some(1.6),
            rs_pow: rs,
            r_min,
            ..base
        },
        Forme::Rubik => Reglages {
            lat_rings: Some(15.0),
            lon_density: Some(40.0),
            move_count: Some(14.0),
            r_base: Some(0.6),
            r_depth: Some(1.7),
            r_active: Some(0.3),
            ink_far: Some(0.62),
            ink_span: Some(0.54),
            rs_pow: rs,
            r_min,
            ..base
        },
        Forme::Onde => Reglages {
            rings: Some(15.0),
            lon_density: Some(40.0),
            r_base: Some(0.6),
            r_depth: Some(1.7),
            rs_pow: rs,
            r_min,
            ..base
        },
        Forme::Toile => Reglages {
            node_n: Some(30.0),
            thr: Some(0.72),
            signals: Some(5.0),
            node_r: Some(1.4),
            node_r_depth: Some(1.8),
            line_w: Some(0.8),
            rs_pow: rs,
            r_min,
            ..base
        },
        Forme::Tresse => Reglages {
            strand_n: Some(52.0),
            turns: Some(3.0),
            ghost_n: Some(150.0),
            r_base: Some(1.2),
            r_depth: Some(1.8),
            rs_pow: rs,
            r_min,
            ..base
        },
        Forme::Echarpe => Reglages {
            lanes: Some(5.0),
            segs: Some(88.0),
            ghost_n: Some(150.0),
            r_base: Some(1.1),
            r_depth: Some(1.7),
            rs_pow: rs,
            r_min,
            ..base
        },
        // L'écharpe vue de face : pas d'inclinaison de caméra, l'ondulation passe sur le rayon,
        // et pas de sphère fantôme derrière.
        Forme::Anneau => Reglages {
            lanes: Some(5.0),
            segs: Some(88.0),
            ghost_n: Some(0.0),
            face_on: Some(1.0),
            r_base: Some(1.1),
            r_depth: Some(1.7),
            rs_pow: rs,
            r_min,
            ..base
        },
        Forme::Morph => Reglages {
            r_dot: Some(0.021),
            icon_d: Some(1.0),
            r_min: Some(0.25),
            ..base
        },
    }
}

/// Un réglage livré : vitesse de l'horloge, multiplicateurs de nombre et de taille, et les
/// réglages en plus posés tels quels après mise à l'échelle.
#[derive(Debug, Clone, Copy)]
struct Prereglage {
    vitesse: f64,
    nombre: f64,
    taille: f64,
    en_plus: Reglages,
}

const fn pr(vitesse: f64, nombre: f64, taille: f64) -> Prereglage {
    Prereglage {
        vitesse,
        nombre,
        taille,
        en_plus: AUCUN,
    }
}

const AUCUN: Reglages = Reglages {
    lat_rings: None,
    rings: None,
    lon_density: None,
    lanes: None,
    segs: None,
    orbit_n: None,
    ghost_n: None,
    node_n: None,
    strand_n: None,
    signals: None,
    icon_d: None,
    r_base: None,
    r_depth: None,
    r_active: None,
    r_dot: None,
    ghost_r: None,
    part_r: None,
    part_r_depth: None,
    node_r: None,
    node_r_depth: None,
    r_size_mul: None,
    r_boost: None,
    ink_far: None,
    ink_span: None,
    rs_pow: None,
    r_min: None,
    ghost_a: None,
    particles: None,
    move_count: None,
    thr: None,
    line_w: None,
    turns: None,
    face_on: None,
    scan_mul: None,
    dim_base: None,
    spin: None,
    band_mul: None,
    wob_mul: None,
    spread: None,
    shape: None,
};

/// Le balayage du globe : multiplicateur de vitesse et opacité des points non balayés.
const fn globe(scan_mul: f64) -> Reglages {
    Reglages {
        scan_mul: Some(scan_mul),
        dim_base: Some(0.45),
        ..AUCUN
    }
}

/// L'écharpe et l'anneau : sans culbute (`spin` 0), largeur de bande et ondulation.
const fn bande(band_mul: f64, wob_mul: f64) -> Reglages {
    Reglages {
        spin: Some(0.0),
        band_mul: Some(band_mul),
        wob_mul: Some(wob_mul),
        ..AUCUN
    }
}

const MORPH_EN_PLUS: Reglages = Reglages {
    spread: Some(1.45),
    ..AUCUN
};

/// Le réglage livré d'une forme à une taille (valeurs de la bibliothèque d'origine).
fn prereglage(forme: Forme, taille: Taille) -> Prereglage {
    use Taille::{T20, T32, T64};
    let avec = |mut p: Prereglage, en_plus: Reglages| {
        p.en_plus = en_plus;
        p
    };
    match (forme, taille) {
        (Forme::Orbites, T64) => pr(1.885, 1.0, 1.0),
        (Forme::Orbites, T32) => pr(2.9072, 0.4251, 1.6849),
        (Forme::Orbites, T20) => pr(3.9, 0.238, 2.4),
        (Forme::Globe, T64) => avec(pr(2.015, 0.42, 1.15), globe(4.08)),
        (Forme::Globe, T32) => avec(pr(2.3803, 0.1839, 1.4769), globe(4.2301)),
        (Forme::Globe, T20) => avec(pr(2.665, 0.105, 1.75), globe(4.335)),
        (Forme::Rubik, T64) => pr(1.82, 0.35, 1.05),
        (Forme::Rubik, T32) => pr(1.8964, 0.1537, 1.4951),
        (Forme::Rubik, T20) => pr(1.95, 0.088, 1.9),
        (Forme::Onde, T64) => pr(4.388, 0.341, 1.0),
        (Forme::Onde, T32) => pr(4.1512, 0.169, 1.3232),
        (Forme::Onde, T20) => pr(3.998, 0.105, 1.6),
        (Forme::Toile, T64) => pr(3.315, 1.35, 0.95),
        (Forme::Toile, T32) => pr(5.0104, 0.4942, 1.2571),
        (Forme::Toile, T20) => pr(6.63, 0.25, 1.52),
        (Forme::Tresse, T64) => pr(1.625, 0.5, 1.0),
        (Forme::Tresse, T32) => pr(2.2234, 0.2056, 1.2011),
        (Forme::Tresse, T20) => pr(2.75, 0.1125, 1.36),
        (Forme::Echarpe, T64) => avec(pr(2.34, 0.25, 0.85), bande(3.9, 1.0)),
        (Forme::Echarpe, T32) => avec(pr(2.7776, 0.0969, 0.9766), bande(4.49, 1.0)),
        (Forme::Echarpe, T20) => avec(pr(3.12, 0.051, 1.073), bande(4.94, 1.0)),
        (Forme::Anneau, T64) => avec(pr(3.24, 0.25, 0.956), bande(3.627, 0.368)),
        (Forme::Anneau, T32) => avec(pr(3.5517, 0.0678, 1.31), bande(3.8265, 0.4751)),
        (Forme::Anneau, T20) => avec(pr(3.78, 0.028, 1.622), bande(3.968, 0.565)),
        (Forme::Morph, T64) => avec(pr(2.405, 0.702, 0.395), MORPH_EN_PLUS),
        (Forme::Morph, T32) => avec(pr(2.2057, 0.5937, 0.6916), MORPH_EN_PLUS),
        (Forme::Morph, T20) => avec(pr(2.08, 0.53, 1.011), MORPH_EN_PLUS),
    }
}

/// Pose sur `o` les champs présents dans `en_plus` (la fusion `{ ...o, ...en_plus }`).
fn fusionner(o: Reglages, en_plus: &Reglages) -> Reglages {
    let mut r = o;
    let paires: [(&mut Option<f64>, Option<f64>); 6] = [
        (&mut r.scan_mul, en_plus.scan_mul),
        (&mut r.dim_base, en_plus.dim_base),
        (&mut r.spin, en_plus.spin),
        (&mut r.band_mul, en_plus.band_mul),
        (&mut r.wob_mul, en_plus.wob_mul),
        (&mut r.spread, en_plus.spread),
    ];
    for (champ, v) in paires {
        if v.is_some() {
            *champ = v;
        }
    }
    r
}

/// Un état résolu : sa forme, la vitesse de son horloge et ses réglages mis à l'échelle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolu {
    pub forme: Forme,
    pub vitesse: f64,
    pub reglages: Reglages,
}

/// Résout un couple (état, taille) : forme, vitesse et réglages finaux.
#[must_use]
pub fn resoudre(etat: OrbState, taille: Taille) -> Resolu {
    let forme = forme_de(etat);
    let p = prereglage(forme, taille);
    let mut o = profil_de_base(forme);
    if p.nombre != 1.0 {
        o = echelonner_nombres(&o, p.nombre);
    }
    if p.taille != 1.0 {
        o = echelonner_rayons(&o, p.taille);
    }
    o = fusionner(o, &p.en_plus);
    Resolu {
        forme,
        vitesse: p.vitesse,
        reglages: o,
    }
}
