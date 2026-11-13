//! Diff d'images référence (Fragment) / candidat (fragment-rs), program.md « Parité au PIXEL » et
//! ADR-009 (sas par catégorie).
//!
//! - STRICT : 0 pixel différent (la cible) ;
//! - TOLÉRÉ : seuls des pixels DANS les boîtes de texte et d'icônes déclarées par le candidat
//!   diffèrent, et les garde-fous de chaque boîte tiennent (voir [`sas`]) ;
//! - ÉCHEC : un pixel différent hors d'une boîte, ou un garde-fou violé, ou un cliquet aggravé.
//!
//! ★ Tout le reste (boîtes, traits, rayons, fonds, positions) est strict : une divergence hors d'une
//! boîte est une vraie divergence. Les anciens seuils (delta ≤ 16, 0,3 %, zones < 20 × 20) sont
//! remplacés : ils faisaient échouer toute scène textuelle (ADR-009).

mod arrondis;
mod boites;
mod exceptions;
mod ombres;
mod sas;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use image::RgbaImage;

use boites::{ArrondiDeclare, BoiteEncre, OmbreDeclaree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Strict,
    Tolere,
    Echec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boite {
    pub x: u32,
    pub y: u32,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Boîtes englobantes des composantes 8-connexes du masque de divergence.
fn zones_connexes(masque: &[bool], largeur: u32, hauteur: u32) -> Vec<Boite> {
    let mut vu = vec![false; masque.len()];
    let mut zones = Vec::new();
    let mut pile = Vec::new();
    for depart in 0..masque.len() {
        if !masque[depart] || vu[depart] {
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        vu[depart] = true;
        pile.push(depart);
        while let Some(i) = pile.pop() {
            let (x, y) = (i as u32 % largeur, i as u32 / largeur);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
                    if nx < 0 || ny < 0 || nx >= i64::from(largeur) || ny >= i64::from(hauteur) {
                        continue;
                    }
                    let j = (ny as u32 * largeur + nx as u32) as usize;
                    if masque[j] && !vu[j] {
                        vu[j] = true;
                        pile.push(j);
                    }
                }
            }
        }
        zones.push(Boite {
            x: x0,
            y: y0,
            largeur: x1 - x0 + 1,
            hauteur: y1 - y0 + 1,
        });
    }
    zones
}

/// Les tranches actives, lues dans `pixels/tranches.json` (`{"actives": ["T1", "T4"]}`).
/// ★ Une seule source, lue aussi par `fragment-pixels` : une scène marquée `"depuis": "T4"` est
/// comparée dès que T4 est active, quel que soit l'état de T3 (les tranches finissent dans le désordre).
fn lire_tranches(chemin: &Path) -> anyhow::Result<Vec<u32>> {
    let texte = std::fs::read_to_string(chemin)
        .with_context(|| format!("{} introuvable ou illisible", chemin.display()))?;
    let json: serde_json::Value = serde_json::from_str(&texte)
        .with_context(|| format!("{} n'est pas du JSON valide", chemin.display()))?;
    let Some(liste) = json["actives"].as_array() else {
        bail!("{} : le champ « actives » (liste) manque", chemin.display());
    };
    liste
        .iter()
        .map(|t| {
            t.as_str().and_then(lire_tranche).with_context(|| {
                format!(
                    "{} : tranche {t} inconnue (attendu \"T3\")",
                    chemin.display()
                )
            })
        })
        .collect()
}

struct Options {
    scene: Option<String>,
    reference: PathBuf,
    candidat: PathBuf,
    diff: PathBuf,
    rapport: PathBuf,
    scenes: PathBuf,
    /// Tranches actives forcées par `--tranche` (vide = celles de `tranches.json`).
    tranches: Vec<u32>,
    tranches_fichier: PathBuf,
    exiger_references: bool,
    /// Base commitée du cliquet (delta max et % dans les boîtes, par PNG).
    cliquet: PathBuf,
    /// Réécrit la base du cliquet avec les mesures du run.
    maj_cliquet: bool,
}

fn lire_options(args: &[String]) -> anyhow::Result<Options> {
    let mut o = Options {
        scene: None,
        reference: "pixels/reference".into(),
        candidat: "pixels/candidate".into(),
        diff: "pixels/diff".into(),
        rapport: "docs/parite-pixels.md".into(),
        scenes: "pixels/scenes".into(),
        tranches: Vec::new(),
        tranches_fichier: "pixels/tranches.json".into(),
        exiger_references: false,
        cliquet: "pixels/cliquet.json".into(),
        maj_cliquet: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut valeur = || {
            it.next()
                .cloned()
                .with_context(|| format!("{a} attend une valeur"))
        };
        match a.as_str() {
            "--reference" => o.reference = valeur()?.into(),
            "--candidate" => o.candidat = valeur()?.into(),
            "--diff" => o.diff = valeur()?.into(),
            "--report" => o.rapport = valeur()?.into(),
            "--scenes" => o.scenes = valeur()?.into(),
            "--tranches" => o.tranches_fichier = valeur()?.into(),
            "--cliquet" => o.cliquet = valeur()?.into(),
            "--maj-cliquet" => o.maj_cliquet = true,
            "--tranche" => {
                let v = valeur()?;
                o.tranches
                    .push(lire_tranche(&v).with_context(|| format!("tranche « {v} » inconnue"))?);
            }
            // `--require-all` : ancien nom, gardé pour les appels existants.
            "--exiger-references" | "--require-all" => o.exiger_references = true,
            s if s.starts_with("--") => bail!("option inconnue : {s}"),
            s => o.scene = Some(s.to_owned()),
        }
    }
    Ok(o)
}

/// `T3` donne 3.
fn lire_tranche(texte: &str) -> Option<u32> {
    let chiffres = texte.strip_prefix('T')?;
    if chiffres.is_empty() || !chiffres.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    chiffres.parse().ok()
}

/// Ce que le JSON d'une scène dit pour la comparaison.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MetaScene {
    /// Tranche à partir de laquelle la scène est comparée (`None` = toujours).
    depuis: Option<u32>,
    /// Scène de preuve de la CI Rust, sans équivalent Fragment.
    sans_reference: bool,
    themes: Vec<String>,
}

fn lire_meta(dossier: &Path) -> anyhow::Result<BTreeMap<String, MetaScene>> {
    let mut meta = BTreeMap::new();
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return Ok(meta);
    };
    for chemin in entrees
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
    {
        let texte = std::fs::read_to_string(&chemin)?;
        let json: serde_json::Value = serde_json::from_str(&texte)
            .with_context(|| format!("{} illisible", chemin.display()))?;
        let nom = json["scene"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| chemin.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let depuis = match json["depuis"].as_str() {
            None => None,
            Some(t) => {
                Some(lire_tranche(t).with_context(|| format!("{nom} : tranche « {t} » inconnue"))?)
            }
        };
        let themes = json["themes"]
            .as_array()
            .map(|t| {
                t.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_else(|| vec!["dark".to_owned()]);
        meta.insert(
            nom,
            MetaScene {
                depuis,
                sans_reference: json["sans_reference"].as_bool().unwrap_or(false),
                themes,
            },
        );
    }
    Ok(meta)
}

/// `menu-dark.png` donne `menu`.
fn scene_du_png(png: &str) -> &str {
    let sans = png.strip_suffix(".png").unwrap_or(png);
    sans.strip_suffix("-dark")
        .or_else(|| sans.strip_suffix("-light"))
        .unwrap_or(sans)
}

fn pngs(dossier: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dossier)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".png"))
                .collect()
        })
        .unwrap_or_default()
}

/// Le classement des PNG avant comparaison.
#[derive(Debug, Default, PartialEq, Eq)]
struct Plan {
    /// Référence ET candidat : à comparer.
    paires: Vec<String>,
    /// Référence sans candidat (la partie Rust n'a pas encore rendu cette scène).
    sans_candidat: Vec<String>,
    /// Candidat sans référence : EN ATTENTE (code 0), sauf `--exiger-references`.
    en_attente: Vec<String>,
    /// Candidat d'une scène de preuve (`sans_reference`) : rien à comparer à Fragment.
    preuves: Vec<String>,
    /// PNG d'une scène d'une tranche postérieure : ignoré.
    hors_tranche: Vec<String>,
    /// Sous `--exiger-references` : PNG attendus (scène × thème) manquants d'un côté ou de l'autre.
    manquants: Vec<String>,
}

/// La tranche (`"depuis"`) de la scène du PNG `png`, `None` si elle n'en déclare pas (T1).
fn scene_depuis(meta: &BTreeMap<String, MetaScene>, png: &str) -> Option<u32> {
    meta.get(scene_du_png(png)).and_then(|m| m.depuis)
}

fn planifier(
    references: &BTreeSet<String>,
    candidats: &BTreeSet<String>,
    meta: &BTreeMap<String, MetaScene>,
    tranches: &[u32],
    exiger: bool,
    filtre: Option<&str>,
) -> Plan {
    let hors = |png: &str| {
        meta.get(scene_du_png(png))
            .is_some_and(|m| m.depuis.is_some_and(|d| !tranches.contains(&d)))
    };
    let garde = |n: &&String| filtre.is_none_or(|s| n.starts_with(s));
    let mut plan = Plan::default();
    for nom in references.union(candidats).filter(garde) {
        if hors(nom) {
            plan.hors_tranche.push(nom.clone());
            continue;
        }
        let m = meta.get(scene_du_png(nom));
        match (references.contains(nom), candidats.contains(nom)) {
            (true, true) => plan.paires.push(nom.clone()),
            (true, false) => plan.sans_candidat.push(nom.clone()),
            (false, true) if m.is_some_and(|m| m.sans_reference) => plan.preuves.push(nom.clone()),
            (false, true) => plan.en_attente.push(nom.clone()),
            (false, false) => {}
        }
    }
    if exiger {
        for (scene, m) in meta {
            let dans_filtre = filtre.is_none_or(|s| scene.starts_with(s));
            if !dans_filtre || m.sans_reference || m.depuis.is_some_and(|d| !tranches.contains(&d))
            {
                continue;
            }
            for theme in &m.themes {
                let png = format!("{scene}-{theme}.png");
                if !references.contains(&png) || !candidats.contains(&png) {
                    plan.manquants.push(png);
                }
            }
        }
    }
    plan
}

/// Une entrée de la base du cliquet : le pire écart toléré, par PNG.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Base {
    /// Plus fort delta de canal dans les boîtes.
    delta_max: u8,
    /// Part de pixels divergents dans les boîtes, en %, arrondie à 4 décimales.
    pourcent: f64,
    /// Plus fort delta toléré dans les halos d'ombre (catégorie ombre, amendement du 11/11).
    ombre_delta_max: u8,
    /// Part de pixels tolérés dans les halos, en %, arrondie à 4 décimales.
    ombre_pourcent: f64,
    /// Plus fort delta toléré dans les bandes d'arrondi (amendement de 11:15, point a).
    arrondi_delta_max: u8,
    /// Part de pixels tolérés dans les bandes, en %, arrondie à 4 décimales.
    arrondi_pourcent: f64,
    /// L'exception nommée `--shadow-l` (point b), quand le PNG en a une surface.
    exception: Option<BaseException>,
    /// Glyphes NON JUGEABLES (fr-p3.1, décision du 13/11) : rognés par un rectangle de découpe à
    /// 3×2 px visibles ou moins. Dette nommée (`docs/parite-pixels.md`), en cliquet.
    non_jugeables: u64,
    /// Lettres changées que la contre-épreuve n'a PAS détectées (fr-p3.1, orchestrateur 13/11
    /// 03:15) : surtout des glyphes étroits (« l », « i », point, virgule : 1 à 3 px de large).
    /// Dette nommée « lettre manquée », comptée par PNG, en cliquet ; à retirer par `fr-p3.4`.
    lettres_manquees: u64,
    /// Boîtes de texte en dette « rastérisation des glyphes » (`fr-p4.1.3`, dès T2) : largeur seule,
    /// ≤ 2 px. Écrit seulement quand il est non nul.
    rasterisation_glyphes: u64,
}

/// Les valeurs mesurées de l'exception `--shadow-l` d'un PNG (donc d'un thème) : le pire, en valeur
/// absolue, des trois garde-fous, et les pixels de delta > 2 de son halo.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct BaseException {
    etalement: f64,
    encre: f64,
    glissement: f64,
    delta_max: u8,
    forts: u64,
}

impl Base {
    fn nulle(&self) -> bool {
        *self == Base::default()
    }
}

fn arrondir(pourcent: f64) -> f64 {
    (pourcent * 10_000.0).round() / 10_000.0
}

/// Lit la base du cliquet ; un fichier absent donne une base vide. Une entrée sans champs d'ombre
/// (base antérieure à la catégorie) les lit à 0.
fn lire_cliquet(chemin: &Path) -> anyhow::Result<BTreeMap<String, Base>> {
    let Ok(texte) = std::fs::read_to_string(chemin) else {
        return Ok(BTreeMap::new());
    };
    let json: serde_json::Value =
        serde_json::from_str(&texte).with_context(|| format!("{} illisible", chemin.display()))?;
    let mut base = BTreeMap::new();
    for (nom, v) in json["scenes"].as_object().into_iter().flatten() {
        let delta = |cle: &str| -> anyhow::Result<u8> {
            match v.get(cle) {
                None => Ok(0),
                Some(d) => d
                    .as_u64()
                    .and_then(|d| u8::try_from(d).ok())
                    .with_context(|| format!("{nom} : {cle} invalide")),
            }
        };
        let part = |cle: &str| -> anyhow::Result<f64> {
            match v.get(cle) {
                None => Ok(0.0),
                Some(p) => p
                    .as_f64()
                    .with_context(|| format!("{nom} : {cle} invalide")),
            }
        };
        base.insert(
            nom.clone(),
            Base {
                delta_max: delta("delta_max")?,
                pourcent: part("pourcent")?,
                ombre_delta_max: delta("ombre_delta_max")?,
                ombre_pourcent: part("ombre_pourcent")?,
                arrondi_delta_max: delta("arrondi_delta_max")?,
                arrondi_pourcent: part("arrondi_pourcent")?,
                non_jugeables: v
                    .get("non_jugeables")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                lettres_manquees: v
                    .get("lettres_manquees")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                rasterisation_glyphes: v.get("rasterisation_glyphes").map_or(Ok(0), |d| {
                    d.as_u64()
                        .with_context(|| format!("{nom} : rasterisation_glyphes invalide"))
                })?,
                exception: match v.get("shadow_l") {
                    None => None,
                    Some(e) => {
                        let f = |cle: &str| {
                            e[cle]
                                .as_f64()
                                .with_context(|| format!("{nom} : shadow_l.{cle} invalide"))
                        };
                        Some(BaseException {
                            etalement: f("etalement")?,
                            encre: f("encre")?,
                            glissement: f("glissement")?,
                            delta_max: e["delta_max"]
                                .as_u64()
                                .and_then(|d| u8::try_from(d).ok())
                                .with_context(|| {
                                format!("{nom} : shadow_l.delta_max invalide")
                            })?,
                            forts: e["forts"]
                                .as_u64()
                                .with_context(|| format!("{nom} : shadow_l.forts invalide"))?,
                        })
                    }
                },
            },
        );
    }
    Ok(base)
}

/// Les mesures d'exceptions nommées d'un PNG : par élément, ses valeurs (en valeur absolue).
type Nommees = BTreeMap<String, BTreeMap<String, f64>>;

/// Lit les exceptions nommées temporaires de T1 du cliquet (clé `exceptions_t1`, à retirer par
/// `fr-p4.1`), par PNG.
fn lire_nommees(chemin: &Path) -> anyhow::Result<BTreeMap<String, Nommees>> {
    let Ok(texte) = std::fs::read_to_string(chemin) else {
        return Ok(BTreeMap::new());
    };
    let json: serde_json::Value =
        serde_json::from_str(&texte).with_context(|| format!("{} illisible", chemin.display()))?;
    let mut tout = BTreeMap::new();
    for (nom, v) in json["scenes"].as_object().into_iter().flatten() {
        let Some(elements) = v.get("exceptions_t1").and_then(|e| e.as_object()) else {
            continue;
        };
        let mut n = Nommees::new();
        for (element, mesures) in elements {
            let mut m = BTreeMap::new();
            for (cle, val) in mesures.as_object().into_iter().flatten() {
                let val = val
                    .as_f64()
                    .with_context(|| format!("{nom} : exceptions_t1.{element}.{cle} invalide"))?;
                m.insert(cle.clone(), val);
            }
            n.insert(element.clone(), m);
        }
        tout.insert(nom.clone(), n);
    }
    Ok(tout)
}

/// Le cliquet des exceptions nommées : `None` si chaque mesure tient sous sa base.
fn verifier_nommees(mesure: &Nommees, base: Option<&Nommees>) -> Option<String> {
    let mut raisons = Vec::new();
    for (element, valeurs) in mesure {
        let Some(b) = base.and_then(|b| b.get(element)) else {
            if valeurs.values().any(|v| *v != 0.0) {
                raisons.push(format!("exception nommée « {element} » sans base"));
            }
            continue;
        };
        for (cle, v) in valeurs {
            let max = b.get(cle).copied().unwrap_or(0.0);
            if arrondir(*v) > max {
                raisons.push(format!("« {element} » : {cle} {v:.4} > base {max:.4}"));
            }
        }
    }
    (!raisons.is_empty()).then(|| format!("cliquet aggravé : {}", raisons.join(" ; ")))
}

fn ecrire_cliquet(
    chemin: &Path,
    base: &BTreeMap<String, Base>,
    nommees: &BTreeMap<String, Nommees>,
) -> anyhow::Result<()> {
    let scenes: serde_json::Map<String, serde_json::Value> = base
        .iter()
        .map(|(nom, b)| {
            let mut v = serde_json::json!({
                "delta_max": b.delta_max,
                "pourcent": b.pourcent,
                "ombre_delta_max": b.ombre_delta_max,
                "ombre_pourcent": b.ombre_pourcent,
                "arrondi_delta_max": b.arrondi_delta_max,
                "arrondi_pourcent": b.arrondi_pourcent,
            });
            if b.non_jugeables > 0 {
                v["non_jugeables"] = serde_json::json!(b.non_jugeables);
            }
            if b.lettres_manquees > 0 {
                v["lettres_manquees"] = serde_json::json!(b.lettres_manquees);
            }
            if let Some(e) = b.exception {
                v["shadow_l"] = serde_json::json!({
                    "etalement": e.etalement,
                    "encre": e.encre,
                    "glissement": e.glissement,
                    "delta_max": e.delta_max,
                    "forts": e.forts,
                });
            }
            if b.rasterisation_glyphes > 0 {
                v["rasterisation_glyphes"] = serde_json::json!(b.rasterisation_glyphes);
            }
            if let Some(n) = nommees.get(nom).filter(|n| !n.is_empty()) {
                v["exceptions_t1"] = serde_json::json!(n);
            }
            (nom.clone(), v)
        })
        .collect();
    let json = serde_json::json!({
        "commentaire": "Cliquet ADR-009 : pire écart toléré, par PNG, dans les boîtes de texte et d'icônes (delta_max, pourcent), dans les halos d'ombre (ombre_delta_max, ombre_pourcent), dans les bandes d'arrondi (arrondi_delta_max, arrondi_pourcent) et, pour l'exception nommée --shadow-l, ses garde-fous mesurés (shadow_l : étalement en px, encre et glissement relatifs, en valeur absolue ; delta max et pixels de delta > 2 de son halo). exceptions_t1 : exceptions NOMMÉES temporaires de T1, À RETIRER PAR fr-p4.1 (xtask/src/pixels/exceptions.rs), par élément : ombre (etalement, encre, glissement) ou arcs (deplacement en px, delta_max et forts de delta > 6 de ses bandes) ou teinte d'une boîte de texte (teinte = 1 − cosinus), en valeur absolue. rasterisation_glyphes : boîtes de texte en dette « rastérisation des glyphes » (fr-p4.1.3, dès T2 : largeur d'encre seule, ≤ 2 px). Une PR qui l'aggrave échoue ; `cargo xtask pixels --maj-cliquet` le réécrit (à relire en PR).",
        "scenes": scenes,
    });
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(chemin, serde_json::to_string_pretty(&json)? + "\n")?;
    Ok(())
}

/// Le cliquet : `None` si la mesure tient, sinon la raison de l'échec.
fn verifier_cliquet(mesure: Base, base: Option<&Base>) -> Option<String> {
    if mesure.nulle() {
        return None;
    }
    let Some(base) = base else {
        return Some(
            "pas de base : lancer `cargo xtask pixels --maj-cliquet` et commiter pixels/cliquet.json"
                .to_owned(),
        );
    };
    let mut raisons = Vec::new();
    if mesure.delta_max > base.delta_max {
        raisons.push(format!(
            "delta max {} > base {}",
            mesure.delta_max, base.delta_max
        ));
    }
    if arrondir(mesure.pourcent) > base.pourcent {
        raisons.push(format!(
            "{:.4} % dans les boîtes > base {:.4} %",
            mesure.pourcent, base.pourcent
        ));
    }
    if mesure.ombre_delta_max > base.ombre_delta_max {
        raisons.push(format!(
            "delta max d'ombre {} > base {}",
            mesure.ombre_delta_max, base.ombre_delta_max
        ));
    }
    if arrondir(mesure.ombre_pourcent) > base.ombre_pourcent {
        raisons.push(format!(
            "{:.4} % dans les halos > base {:.4} %",
            mesure.ombre_pourcent, base.ombre_pourcent
        ));
    }
    if mesure.arrondi_delta_max > base.arrondi_delta_max {
        raisons.push(format!(
            "delta max d'arrondi {} > base {}",
            mesure.arrondi_delta_max, base.arrondi_delta_max
        ));
    }
    if arrondir(mesure.arrondi_pourcent) > base.arrondi_pourcent {
        raisons.push(format!(
            "{:.4} % dans les bandes d'arrondi > base {:.4} %",
            mesure.arrondi_pourcent, base.arrondi_pourcent
        ));
    }
    if mesure.non_jugeables > base.non_jugeables {
        raisons.push(format!(
            "{} glyphes non jugeables (rognés à 3×2 px) > base {}",
            mesure.non_jugeables, base.non_jugeables
        ));
    }
    if mesure.lettres_manquees > base.lettres_manquees {
        raisons.push(format!(
            "{} lettres manquées par la contre-épreuve > base {}",
            mesure.lettres_manquees, base.lettres_manquees
        ));
    }
    if mesure.rasterisation_glyphes > base.rasterisation_glyphes {
        raisons.push(format!(
            "{} boîtes en dette « rastérisation des glyphes » (fr-p4.1.3) > base {}",
            mesure.rasterisation_glyphes, base.rasterisation_glyphes
        ));
    }
    match (mesure.exception, base.exception) {
        (None, _) => {}
        (Some(_), None) => raisons.push("exception --shadow-l sans base".to_owned()),
        (Some(m), Some(b)) => {
            for (quoi, v, max) in [
                ("étalement", m.etalement, b.etalement),
                ("encre", m.encre, b.encre),
                ("glissement", m.glissement, b.glissement),
            ] {
                if arrondir(v.abs()) > max.abs() {
                    raisons.push(format!(
                        "--shadow-l : {quoi} {:.4} > base {:.4}",
                        v.abs(),
                        max.abs()
                    ));
                }
            }
            if m.delta_max > b.delta_max {
                raisons.push(format!(
                    "--shadow-l : delta max {} > base {}",
                    m.delta_max, b.delta_max
                ));
            }
            if m.forts > b.forts {
                raisons.push(format!(
                    "--shadow-l : {} px de delta > 2 > base {}",
                    m.forts, b.forts
                ));
            }
        }
    }
    (!raisons.is_empty()).then(|| format!("cliquet aggravé : {}", raisons.join(" ; ")))
}

const DEBUT_RAPPORT: &str = "<!-- sas:debut -->";
const FIN_RAPPORT: &str = "<!-- sas:fin -->";

/// Écrit `section` dans `chemin`, entre les marqueurs : le reste du fichier (les dettes écrites à
/// la main dans `docs/parite-pixels.md`) n'est jamais touché. Sans marqueurs, la section est ajoutée.
fn ecrire_rapport(chemin: &Path, section: &str) -> anyhow::Result<()> {
    let bloc = format!("{DEBUT_RAPPORT}\n{section}{FIN_RAPPORT}\n");
    let existant = std::fs::read_to_string(chemin).unwrap_or_default();
    let nouveau = match (existant.find(DEBUT_RAPPORT), existant.find(FIN_RAPPORT)) {
        (Some(d), Some(f)) if d < f => format!(
            "{}{bloc}{}",
            &existant[..d],
            existant[f + FIN_RAPPORT.len()..].trim_start_matches('\n')
        ),
        _ if existant.is_empty() => format!("# Parité au pixel\n\n{bloc}"),
        _ => format!("{}\n\n{bloc}", existant.trim_end()),
    };
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(chemin, nouveau)?;
    Ok(())
}

/// Les boîtes et les surfaces ombrées d'un candidat : son fichier de déclaration voisin du PNG,
/// sinon des boîtes dérivées de l'image et AUCUNE ombre (un candidat plus ancien que l'export n'a
/// pas de fichier ; ses ombres restent strictes, le halo ne se déduit jamais de l'image).
fn boites_du_candidat(dossier: &Path, nom: &str, image: &RgbaImage) -> anyhow::Result<Declaration> {
    let chemin = dossier.join(nom.replace(".png", ".boites.json"));
    match std::fs::read_to_string(&chemin) {
        Ok(texte) => {
            let illisible = || format!("{} illisible", chemin.display());
            let rognes = boites::deserialiser_rognes(&texte).with_context(illisible)?;
            let (non_jugeables, glyphes) = boites::deserialiser_glyphes(&texte)
                .with_context(illisible)?
                .into_iter()
                .partition(|g| rognes.contains(g) && non_jugeable(g));
            Ok(Declaration {
                boites: boites::deserialiser(&texte).with_context(illisible)?,
                glyphes,
                non_jugeables,
                ombres: boites::deserialiser_ombres(&texte).with_context(illisible)?,
                arrondis: boites::deserialiser_arrondis(&texte).with_context(illisible)?,
            })
        }
        Err(_) => Ok(Declaration {
            boites: boites::detecter(image),
            glyphes: Vec::new(),
            non_jugeables: Vec::new(),
            ombres: Vec::new(),
            arrondis: Vec::new(),
        }),
    }
}

/// Un glyphe ROGNÉ par un rectangle de découpe n'est pas jugeable à 3×2 px visibles ou moins
/// (fr-p3.1, décision du chef confirmée par l'orchestrateur le 13/11) : un fragment ne porte plus
/// de forme de lettre. Le rectangle de découpe, lui, reste en strict ; un petit glyphe ENTIER
/// (point, virgule, tiret) reste jugeable.
fn non_jugeable(g: &BoiteEncre) -> bool {
    g.largeur.min(g.hauteur) <= 2 && g.largeur.max(g.hauteur) <= 3
}

/// Ce que le candidat déclare d'une capture (ADR-009) : boîtes, surfaces ombrées, boîtes arrondies.
struct Declaration {
    boites: Vec<BoiteEncre>,
    /// Les glyphes peints JUGEABLES (version 4) : le garde-fou de forme par glyphe et la
    /// contre-épreuve.
    glyphes: Vec<BoiteEncre>,
    /// Les glyphes non jugeables, ôtés de `glyphes` (voir [`non_jugeable`]).
    non_jugeables: Vec<BoiteEncre>,
    ombres: Vec<OmbreDeclaree>,
    arrondis: Vec<ArrondiDeclare>,
}

/// Point d'entrée de `cargo xtask pixels`. Rend `Ok(true)` si toutes les scènes passent.
///
/// - divergence hors d'une boîte, garde-fou violé ou cliquet aggravé : échec ;
/// - référence absente : EN ATTENTE, code 0 (la référence d'une scène arrive quand Fragment l'a
///   rendue) ;
/// - `--exiger-references` : toute scène de la tranche doit avoir référence ET candidat, sinon échec.
pub fn run(args: &[String]) -> anyhow::Result<bool> {
    let o = lire_options(args)?;
    let meta = lire_meta(&o.scenes)?;
    let tranches = if o.tranches.is_empty() {
        lire_tranches(&o.tranches_fichier)?
    } else {
        o.tranches.clone()
    };
    let references = pngs(&o.reference);
    let candidats = pngs(&o.candidat);
    let plan = planifier(
        &references,
        &candidats,
        &meta,
        &tranches,
        o.exiger_references,
        o.scene.as_deref(),
    );
    std::fs::create_dir_all(&o.diff)?;
    let base = lire_cliquet(&o.cliquet)?;
    let mut nouvelle_base = base.clone();
    let base_nommees = lire_nommees(&o.cliquet)?;
    let mut nouvelles_nommees = base_nommees.clone();

    let mut ok = true;
    let mut rapport = String::from(
        "Généré par `cargo xtask pixels` (ADR-009). Hors des boîtes de texte et d'icônes : strict, 0 pixel. Dans une boîte : anticrénelage seul, sous quatre garde-fous (encre totale, boîte d'encre, forme, teinte). Dans le halo d'une ombre déclarée (hors intérieur de la surface) : delta ≤ 2, sous trois garde-fous (étalement, glissement, encre) ; exception nommée `--shadow-l` en cliquet. Dans la bande (±1 px) d'un arc de coin déclaré : delta ≤ 6, sous le garde-fou de position de l'arc.\n\n| Scène | Verdict | Boîtes (texte/icônes) | Hors boîtes (px) | Dans les boîtes (px) | % dans les boîtes | Delta max | Largeur ±1 (dette fr-t1.3) | Ombres | Halos (px tolérés) | % halos | Delta ombre | Arrondis | Bandes (px tolérés) | Delta arrondi | Cliquet |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let mut details = String::new();
    let mut mesures_ombres = String::new();
    let mut mesures_arrondis = String::new();
    let mut mesures_barycentres = String::new();
    let mut mesures_glyphes = String::new();
    let mut mesures_nommees = String::new();
    let mut mesures_contre_epreuve = String::new();
    let (mut essais_contre_epreuve, mut detectes_contre_epreuve) = (0usize, 0usize);
    let mut non_jugeables_total = 0usize;
    let mut mesures_rasterisation = String::new();
    for nom in &plan.paires {
        let r = image::open(o.reference.join(nom))?.to_rgba8();
        let c = image::open(o.candidat.join(nom))?.to_rgba8();
        let ligne = match boites_du_candidat(&o.candidat, nom, &c).and_then(|d| {
            // fr-p3.2 : la tranche de la scène décide si la largeur d'encre est stricte.
            sas::comparer_tranche(
                &r,
                &c,
                &d.boites,
                &d.glyphes,
                &d.ombres,
                &d.arrondis,
                &exceptions::du_png(nom),
                scene_depuis(&meta, nom),
            )
            .map(|s| (d, s))
        }) {
            Ok((d, s)) => {
                s.diff.save(o.diff.join(nom))?;
                // fr-p3.1 : une lettre changée sur le texte réel du candidat doit faire échouer le sas.
                let ce = sas::contre_epreuve(
                    &r,
                    &c,
                    &d.boites,
                    &d.glyphes,
                    &exceptions::du_png(nom),
                    scene_depuis(&meta, nom),
                );
                if ce.essais > 0 || !d.non_jugeables.is_empty() {
                    writeln!(
                        mesures_contre_epreuve,
                        "- {nom} : {} lettres changées (dont {} petits glyphes entiers), {} détectées ; {} glyphes sans autre lettre dans leur passage (pas d'essai) ; {} glyphes non jugeables (rognés à 3×2 px){}",
                        ce.essais,
                        ce.petits,
                        ce.detectes,
                        ce.sans_autre_lettre,
                        d.non_jugeables.len(),
                        d.non_jugeables
                            .iter()
                            .map(|g| format!(" ({},{}) {}×{}", g.x, g.y, g.largeur, g.hauteur))
                            .collect::<String>()
                    )?;
                }
                non_jugeables_total += d.non_jugeables.len();
                // ★ Une lettre manquée n'échoue plus seule : c'est une dette nommée, comptée par
                // PNG en cliquet (orchestrateur, 13/11 03:15), sous un taux global de détection.
                for m in &ce.manques {
                    writeln!(
                        details,
                        "- {nom} : contre-épreuve : lettre manquée (dette nommée, fr-p3.4) : {m}"
                    )?;
                }
                essais_contre_epreuve += ce.essais;
                detectes_contre_epreuve += ce.detectes;
                let boites = d.boites;
                let mesure = Base {
                    delta_max: s.delta_max_boites,
                    pourcent: arrondir(s.pourcent_boites()),
                    ombre_delta_max: s.delta_max_ombre,
                    ombre_pourcent: arrondir(s.pourcent_ombre()),
                    arrondi_delta_max: s.delta_max_arrondi,
                    arrondi_pourcent: arrondir(s.pourcent_arrondi()),
                    non_jugeables: d.non_jugeables.len() as u64,
                    lettres_manquees: ce.manques.len() as u64,
                    rasterisation_glyphes: s.dettes_rasterisation() as u64,
                    exception: s
                        .gardes_exception()
                        .map(|g| BaseException {
                            etalement: arrondir(g.etalement),
                            encre: arrondir(g.encre),
                            glissement: arrondir(g.glissement),
                            delta_max: s.delta_max_exception,
                            forts: s.forts_exception,
                        })
                        // Une exception sans écart (capture comparée à elle-même) ne demande pas de base.
                        .filter(|e| *e != BaseException::default()),
                };
                let mesure_nommees: Nommees = s
                    .nommees
                    .iter()
                    .map(|(e, m)| {
                        (
                            (*e).to_owned(),
                            m.iter()
                                .map(|(k, v)| ((*k).to_owned(), arrondir(*v)))
                                .collect(),
                        )
                    })
                    .collect();
                for (e, m) in &mesure_nommees {
                    let valeurs = m
                        .iter()
                        .map(|(k, v)| format!("{k} {v}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    writeln!(mesures_nommees, "- {nom} : {e} : {valeurs}")?;
                }
                if mesure.rasterisation_glyphes > 0 {
                    let boites: Vec<String> = s
                        .boites
                        .iter()
                        .filter(|b| b.dette_rasterisation)
                        .map(|b| {
                            let l = b.ecart_bords.map_or(0, |e| e[2] - e[0]);
                            format!("({},{}) {l:+}", b.boite.x, b.boite.y)
                        })
                        .collect();
                    writeln!(
                        mesures_rasterisation,
                        "- {nom} : {} boîtes : {}",
                        mesure.rasterisation_glyphes,
                        boites.join(", ")
                    )?;
                }
                let raison_cliquet = verifier_cliquet(mesure, base.get(nom))
                    .into_iter()
                    .chain(verifier_nommees(&mesure_nommees, base_nommees.get(nom)))
                    .reduce(|a, b| format!("{a} ; {b}"));
                let mut verdict = s.verdict;
                let cliquet = if verdict == Verdict::Echec {
                    "—".to_owned()
                } else if let Some(raison) = raison_cliquet {
                    if o.maj_cliquet {
                        // Mise à jour voulue : la base change, la PR montre la différence.
                        println!("{nom} : base réécrite ({raison})");
                        "réécrit".to_owned()
                    } else {
                        verdict = Verdict::Echec;
                        writeln!(details, "- {nom} : {raison}")?;
                        raison
                    }
                } else {
                    "ok".to_owned()
                };
                if o.maj_cliquet && s.verdict != Verdict::Echec {
                    if mesure.nulle() {
                        nouvelle_base.remove(nom);
                    } else {
                        nouvelle_base.insert(nom.clone(), mesure);
                    }
                    if mesure_nommees.is_empty() {
                        nouvelles_nommees.remove(nom);
                    } else {
                        nouvelles_nommees.insert(nom.clone(), mesure_nommees.clone());
                    }
                }
                if verdict == Verdict::Echec {
                    ok = false;
                    if s.differents_hors > 0 {
                        writeln!(
                            details,
                            "- {nom} : {} px HORS boîtes (delta max {}, dont {} de delta ≤ 2)",
                            s.differents_hors, s.delta_max_hors, s.differents_hors_faibles
                        )?;
                    }
                    for z in s.zones_hors.iter().take(8) {
                        writeln!(
                            details,
                            "- {nom} : divergence HORS boîte en ({},{}) {}×{} px",
                            z.x, z.y, z.largeur, z.hauteur
                        )?;
                    }
                    if s.forts_dans_halos > 0 {
                        writeln!(
                            details,
                            "- {nom} : {} px de delta > {} dans les halos d'ombre",
                            s.forts_dans_halos,
                            ombres::DELTA_OMBRE
                        )?;
                    }
                    for res in s.ombres.iter().filter(|o| !o.ok()) {
                        writeln!(
                            details,
                            "- {nom} : surface ombrée ({:.0},{:.0}) {:.0}×{:.0} : {} [{}]",
                            res.ombre.x,
                            res.ombre.y,
                            res.ombre.largeur,
                            res.ombre.hauteur,
                            res.violations.join(" ; "),
                            res.mesures()
                        )?;
                    }
                    if s.forts_dans_bandes > 0 {
                        writeln!(
                            details,
                            "- {nom} : {} px de delta > {} dans les bandes d'arrondi",
                            s.forts_dans_bandes,
                            arrondis::DELTA_ARRONDI
                        )?;
                    }
                    for res in s.arrondis.iter().filter(|a| !a.ok()) {
                        writeln!(
                            details,
                            "- {nom} : arrondi ({:.0},{:.0}) {:.0}×{:.0} r {} : {} [{}]",
                            res.arrondi.x,
                            res.arrondi.y,
                            res.arrondi.largeur,
                            res.arrondi.hauteur,
                            res.arrondi.rayon,
                            res.violations.join(" ; "),
                            res.mesures()
                        )?;
                    }
                    for b in s.boites.iter().filter(|b| !b.ok()) {
                        writeln!(
                            details,
                            "- {nom} : boîte {} ({},{}) {}×{} : {} [{}]",
                            b.boite.categorie.nom(),
                            b.boite.x,
                            b.boite.y,
                            b.boite.largeur,
                            b.boite.hauteur,
                            b.violations.join(" ; "),
                            b.mesures()
                        )?;
                    }
                }
                for (k, res) in s.ombres.iter().enumerate() {
                    let exception = if res.exception {
                        format!(
                            " EXCEPTION {} (étalement {:+.2} px, encre {:+.0} %, glissement {:+.0} %, {} px de delta > 2, delta max {})",
                            ombres::JETON_EXCEPTION,
                            res.gardes.etalement,
                            100.0 * res.gardes.encre,
                            100.0 * res.gardes.glissement,
                            s.forts_exception,
                            s.delta_max_exception
                        )
                    } else {
                        String::new()
                    };
                    writeln!(
                        mesures_ombres,
                        "- {nom} : surface {k} ({:.0},{:.0}) {:.0}×{:.0}{exception}{} : {}",
                        res.ombre.x,
                        res.ombre.y,
                        res.ombre.largeur,
                        res.ombre.hauteur,
                        if res.ok() { "" } else { " ÉCHEC" },
                        res.mesures()
                    )?;
                }
                for res in &s.arrondis {
                    let pire = res.pire_aire();
                    if pire > 0.0 {
                        writeln!(
                            mesures_arrondis,
                            "- {nom} : ({:.0},{:.0}) {:.0}×{:.0} r {}{} : pire déplacement {pire:.3} px ({})",
                            res.arrondi.x,
                            res.arrondi.y,
                            res.arrondi.largeur,
                            res.arrondi.hauteur,
                            res.arrondi.rayon,
                            if res.ok() { "" } else { " ÉCHEC" },
                            res.mesures()
                        )?;
                    }
                }
                if let Some((part, ou)) = s.pire_glyphe() {
                    writeln!(
                        mesures_glyphes,
                        "- {nom} : pire glyphe à {part:.2} de sa borne (boîte {ou})"
                    )?;
                }
                if let Some((x, y, ou)) = s.pire_barycentre() {
                    writeln!(
                        mesures_barycentres,
                        "- {nom} : pire écart {x:.2} px en x, {y:.2} px en y ({ou})"
                    )?;
                }
                let (texte, icones) = sas::par_categorie(&boites);
                format!(
                    "| {nom} | {verdict:?} | {texte}/{icones} | {} | {} | {:.4} | {} | {} | {} | {} | {:.4} | {} | {} | {} | {} | {cliquet} |",
                    s.differents_hors,
                    s.differents_boites,
                    s.pourcent_boites(),
                    s.delta_max_boites,
                    s.dettes_largeur(),
                    s.ombres.len(),
                    s.differents_ombre,
                    s.pourcent_ombre(),
                    s.delta_max_ombre,
                    s.arrondis.len(),
                    s.differents_arrondi,
                    s.delta_max_arrondi,
                )
            }
            Err(erreur) => {
                ok = false;
                format!(
                    "| {nom} | Echec | — | — | — | — | — | — | — | — | — | — | — | — | — | {erreur} |"
                )
            }
        };
        println!("{ligne}");
        writeln!(rapport, "{ligne}")?;
    }
    for (liste, etat) in [
        (&plan.sans_candidat, "sans candidat"),
        (&plan.en_attente, "en attente de référence"),
        (&plan.preuves, "preuve (sans référence Fragment)"),
        (&plan.hors_tranche, "hors tranche"),
    ] {
        for nom in liste {
            writeln!(
                rapport,
                "| {nom} | {etat} | — | — | — | — | — | — | — | — | — | — | — | — | — | — |"
            )?;
        }
        if !liste.is_empty() {
            println!("{etat} : {}", liste.join(", "));
        }
    }
    for nom in &plan.manquants {
        ok = false;
        writeln!(
            rapport,
            "| {nom} | MANQUANT (--exiger-references) | — | — | — | — | — | — | — | — | — | — | — | — | — | — |"
        )?;
        eprintln!(
            "xtask pixels : {nom} manque (référence ou candidat), exigé par --exiger-references"
        );
    }
    if !details.is_empty() {
        write!(rapport, "\nÉchecs :\n\n{details}")?;
        eprint!("{details}");
    }
    if !mesures_ombres.is_empty() {
        // Les chiffres des garde-fous d'ombre, par côté jugé (candidat moins référence) : de quoi
        // suivre la dette et recaler les bornes (ADR-009).
        write!(
            rapport,
            "\nOmbres (distance au bord et encre, candidat moins référence) :\n\n{mesures_ombres}"
        )?;
        print!("{mesures_ombres}");
    }
    if !mesures_arrondis.is_empty() {
        write!(
            rapport,
            "\nArrondis (déplacement radial moyen de chaque arc, candidat moins référence) :\n\n{mesures_arrondis}"
        )?;
        print!("{mesures_arrondis}");
    }
    if !mesures_nommees.is_empty() {
        write!(
            rapport,
            "
Exceptions NOMMÉES temporaires de T1, à retirer par fr-p4.1 (mesures en valeur absolue, en cliquet) :

{mesures_nommees}"
        )?;
    }
    if !mesures_glyphes.is_empty() {
        write!(
            rapport,
            "
Forme par glyphe peint (fr-p3.1, quatre mesures : L1 floutée ≤ {}, masses nettes 3×3 ≤ {} et 2×2 ≤ {}, rapport relatif ≥ {} ; pire marge des boîtes qui passent, borne 1) :

{mesures_glyphes}",
            sas::GLYPHE_L1,
            sas::GLYPHE_NET3,
            sas::GLYPHE_NET2,
            sas::GLYPHE_RELATIF
        )?;
    }
    if !mesures_rasterisation.is_empty() {
        write!(
            rapport,
            "
Dette « rastérisation des glyphes » (fr-p4.1.3), dès T2 : boîtes de texte en échec sur la seule largeur d'encre, ≤ 2 px (en cliquet par PNG) :

{mesures_rasterisation}"
        )?;
    }
    if !mesures_barycentres.is_empty() {
        write!(
            rapport,
            "
Barycentres d'encre (pire écart absolu des passages et de leurs moitiés, borne ±{} px) :

{mesures_barycentres}",
            sas::ECART_BARYCENTRE
        )?;
    }
    if essais_contre_epreuve > 0 {
        let ligne = format!(
            "contre-épreuve (fr-p3.1) : {essais_contre_epreuve} lettres changées sur le texte réel des candidats, {detectes_contre_epreuve} détectées ({:.1} % des glyphes jugeables, minimum {:.1} %) ; {} lettres manquées (dette nommée par PNG, en cliquet) ; {non_jugeables_total} glyphes non jugeables (rognés par un rectangle de découpe à 3×2 px visibles ou moins, dette nommée, en cliquet)",
            100.0 * detectes_contre_epreuve as f64 / essais_contre_epreuve as f64,
            100.0 * sas::DETECTION_MIN,
            essais_contre_epreuve - detectes_contre_epreuve
        );
        println!("{ligne}");
        let taux = detectes_contre_epreuve as f64 / essais_contre_epreuve as f64;
        if taux < sas::DETECTION_MIN {
            ok = false;
            println!(
                "xtask pixels : contre-épreuve à {:.1} % < {:.1} % (détection minimale)",
                100.0 * taux,
                100.0 * sas::DETECTION_MIN
            );
        }
        write!(
            rapport,
            "
Contre-épreuve d'une lettre changée (fr-p3.1, ADR-009) : dans chaque boîte de texte qui passe, un glyphe peint du candidat remplacé par une AUTRE lettre du même passage (écart floutée ≥ {} : le banc ne génère pas d'essai « même lettre ailleurs ») doit faire échouer la boîte ; chaque petit glyphe entier (point, virgule, tiret) est essayé. {ligne}.

{mesures_contre_epreuve}",
            sas::MEME_LETTRE
        )?;
    }
    if plan.paires.is_empty() && !o.exiger_references {
        println!("xtask pixels : aucune paire référence/candidat à comparer (en attente)");
    }
    ecrire_rapport(&o.rapport, &rapport)?;
    if o.maj_cliquet {
        ecrire_cliquet(&o.cliquet, &nouvelle_base, &nouvelles_nommees)?;
        println!("cliquet réécrit : {}", o.cliquet.display());
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ens(noms: &[&str]) -> BTreeSet<String> {
        noms.iter().map(|n| (*n).to_owned()).collect()
    }

    fn meta() -> BTreeMap<String, MetaScene> {
        let sc = |depuis, sans_reference| MetaScene {
            depuis,
            sans_reference,
            themes: vec!["dark".into(), "light".into()],
        };
        BTreeMap::from([
            ("t1-onglets".to_owned(), sc(None, false)),
            ("menu".to_owned(), sc(Some(7), false)),
            ("mire".to_owned(), sc(None, true)),
        ])
    }

    #[test]
    fn le_cliquet_des_exceptions_nommees_refuse_une_aggravation() {
        let n = |v: f64| -> Nommees {
            BTreeMap::from([(
                "arcs du menu".to_owned(),
                BTreeMap::from([("deplacement".to_owned(), v)]),
            )])
        };
        assert_eq!(verifier_nommees(&n(0.22), Some(&n(0.224))), None);
        assert!(verifier_nommees(&n(0.23), Some(&n(0.224))).is_some());
        // Sans base : refusé, sauf mesure nulle.
        assert!(verifier_nommees(&n(0.1), None).is_some());
        assert_eq!(verifier_nommees(&n(0.0), None), None);
    }

    #[test]
    fn reference_absente_est_en_attente() {
        let plan = planifier(
            &ens(&[]),
            &ens(&["t1-onglets-dark.png"]),
            &meta(),
            &[1],
            false,
            None,
        );
        assert_eq!(plan.en_attente, vec!["t1-onglets-dark.png"]);
        assert!(plan.paires.is_empty() && plan.manquants.is_empty());
    }

    #[test]
    fn scene_d_une_tranche_posterieure_est_ignoree() {
        let plan = planifier(
            &ens(&["menu-dark.png"]),
            &ens(&["menu-dark.png"]),
            &meta(),
            &[1],
            true,
            None,
        );
        assert_eq!(plan.hors_tranche, vec!["menu-dark.png"]);
        assert!(plan.paires.is_empty());
        // Et la tranche suivante la reprend.
        let plan = planifier(
            &ens(&["menu-dark.png"]),
            &ens(&["menu-dark.png"]),
            &meta(),
            &[7],
            false,
            None,
        );
        assert_eq!(plan.paires, vec!["menu-dark.png"]);
    }

    #[test]
    fn une_scene_de_preuve_n_attend_pas_de_reference() {
        let plan = planifier(
            &ens(&[]),
            &ens(&["mire-dark.png"]),
            &meta(),
            &[1],
            true,
            None,
        );
        assert_eq!(plan.preuves, vec!["mire-dark.png"]);
        assert!(!plan.manquants.iter().any(|m| m.starts_with("mire")));
    }

    #[test]
    fn exiger_references_transforme_l_attente_en_echec() {
        let plan = planifier(
            &ens(&["t1-onglets-dark.png"]),
            &ens(&["t1-onglets-dark.png"]),
            &meta(),
            &[1],
            true,
            None,
        );
        // Le thème clair n'existe ni côté référence ni côté candidat.
        assert_eq!(plan.manquants, vec!["t1-onglets-light.png"]);
        let sans = planifier(
            &ens(&["t1-onglets-dark.png"]),
            &ens(&["t1-onglets-dark.png"]),
            &meta(),
            &[1],
            false,
            None,
        );
        assert!(sans.manquants.is_empty());
    }

    #[test]
    fn le_filtre_de_scene_restreint_le_plan() {
        let plan = planifier(
            &ens(&[]),
            &ens(&["t1-onglets-dark.png", "mire-dark.png"]),
            &meta(),
            &[1],
            true,
            Some("mire"),
        );
        assert_eq!(plan.preuves, vec!["mire-dark.png"]);
        assert!(plan.en_attente.is_empty() && plan.manquants.is_empty());
    }

    #[test]
    fn une_scene_depuis_t4_est_comparee_des_que_t4_est_active_quel_que_soit_t3() {
        let paire = ens(&["menu-dark.png"]);
        let m = BTreeMap::from([(
            "menu".to_owned(),
            MetaScene {
                depuis: Some(4),
                sans_reference: false,
                themes: vec!["dark".into()],
            },
        )]);
        // T4 active sans T3 : comparée.
        let plan = planifier(&paire, &paire, &m, &[1, 4], false, None);
        assert_eq!(plan.paires, vec!["menu-dark.png"]);
        // T3 active sans T4 : sautée.
        let plan = planifier(&paire, &paire, &m, &[1, 3], true, None);
        assert_eq!(plan.hors_tranche, vec!["menu-dark.png"]);
        assert!(plan.paires.is_empty() && plan.manquants.is_empty());
    }

    #[test]
    fn les_tranches_actives_sont_lues_dans_le_fichier() {
        let dossier = std::env::temp_dir().join(format!("tranches-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let f = dossier.join("tranches.json");
        std::fs::write(&f, r#"{"actives": ["T1", "T4"]}"#).unwrap();
        assert_eq!(lire_tranches(&f).unwrap(), vec![1, 4]);
        std::fs::write(&f, r#"{"actives": ["quatre"]}"#).unwrap();
        assert!(lire_tranches(&f).is_err());
        std::fs::write(&f, "{ pas du json").unwrap();
        assert!(lire_tranches(&f).is_err());
        std::fs::write(&f, "{}").unwrap();
        assert!(lire_tranches(&f).is_err());
        // Fichier absent : erreur claire qui nomme le chemin.
        let erreur = lire_tranches(&dossier.join("absent.json")).unwrap_err();
        assert!(format!("{erreur:#}").contains("absent.json"));
        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn le_fichier_du_depot_active_t1() {
        let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("../pixels/tranches.json");
        assert!(lire_tranches(&f).unwrap().contains(&1));
    }

    #[test]
    fn nom_de_scene_et_tranche() {
        assert_eq!(
            scene_du_png("reglages-appearance-light.png"),
            "reglages-appearance"
        );
        assert_eq!(scene_du_png("x.png"), "x");
        assert_eq!(lire_tranche("T11"), Some(11));
        assert_eq!(lire_tranche("11"), None);
    }

    #[test]
    fn les_options_de_tranche_et_d_exigence() {
        let o = lire_options(&[
            "--exiger-references".into(),
            "--tranche".into(),
            "T3".into(),
        ])
        .unwrap();
        assert!(o.exiger_references);
        assert_eq!(o.tranches, vec![3]);
        assert!(lire_options(&["--tranche".into(), "3".into()]).is_err());
    }

    #[test]
    fn les_scenes_du_depot_sont_lues() {
        let dossier = Path::new(env!("CARGO_MANIFEST_DIR")).join("../pixels/scenes");
        let meta = lire_meta(&dossier).unwrap();
        assert_eq!(meta["menu"].depuis, Some(7));
        assert!(meta["mire"].sans_reference);
        assert_eq!(meta["fenetre-vide"].themes.len(), 2);
    }

    fn dossier_temporaire(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xtask-pixels-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn lancer(racine: &Path, extra: &[&str]) -> anyhow::Result<bool> {
        let chemin = |s: &str| racine.join(s).to_string_lossy().into_owned();
        let mut args: Vec<String> = [
            "--reference",
            &chemin("ref"),
            "--candidate",
            &chemin("cand"),
            "--diff",
            &chemin("diff"),
            "--report",
            &chemin("rapport.md"),
            "--cliquet",
            &chemin("cliquet.json"),
            "--scenes",
            &chemin("scenes"),
            "--tranche",
            "T1",
        ]
        .map(str::to_owned)
        .to_vec();
        args.extend(extra.iter().map(|s| (*s).to_owned()));
        run(&args)
    }

    #[test]
    fn le_sas_et_le_cliquet_de_bout_en_bout() {
        use sas::tests::{ENCRE, ligne};
        let racine = dossier_temporaire("sas");
        std::fs::create_dir_all(racine.join("ref")).unwrap();
        std::fs::create_dir_all(racine.join("cand")).unwrap();
        ligne("panneau", 1.0, ENCRE, 0)
            .save(racine.join("ref/s-dark.png"))
            .unwrap();
        let candidat = |texte: &str, couverture: f64| {
            ligne(texte, couverture, ENCRE, 0)
                .save(racine.join("cand/s-dark.png"))
                .unwrap();
        };
        // Un trait plus fin passe le sas, mais sans base de cliquet l'écart n'est pas publié : échec.
        candidat("panneau", 0.85);
        assert!(!lancer(&racine, &[]).unwrap());
        // --maj-cliquet publie la base ; le run suivant passe.
        assert!(lancer(&racine, &["--maj-cliquet"]).unwrap());
        assert!(lancer(&racine, &[]).unwrap());
        let rapport = std::fs::read_to_string(racine.join("rapport.md")).unwrap();
        assert!(rapport.contains("| s-dark.png | Tolere |"), "{rapport}");
        // Un trait plus pâle aggrave le delta : le cliquet échoue.
        candidat("panneau", 0.78);
        assert!(!lancer(&racine, &[]).unwrap());
        let rapport = std::fs::read_to_string(racine.join("rapport.md")).unwrap();
        assert!(rapport.contains("cliquet aggravé"), "{rapport}");
        // ★ Contre-épreuve : une lettre changée sort rouge, même avec la base.
        candidat("ponneau", 0.85);
        assert!(!lancer(&racine, &[]).unwrap());
        let rapport = std::fs::read_to_string(racine.join("rapport.md")).unwrap();
        assert!(rapport.contains("| s-dark.png | Echec |"), "{rapport}");
        assert!(rapport.contains("forme"), "{rapport}");
        let _ = std::fs::remove_dir_all(&racine);
    }

    #[test]
    fn le_cliquet_refuse_une_aggravation_et_accepte_une_amelioration() {
        let base = Base {
            delta_max: 100,
            pourcent: 1.0,
            ombre_delta_max: 2,
            ombre_pourcent: 5.0,
            arrondi_delta_max: 6,
            arrondi_pourcent: 10.0,
            non_jugeables: 3,
            lettres_manquees: 2,
            rasterisation_glyphes: 4,
            exception: Some(BaseException {
                etalement: 1.5,
                encre: 0.3,
                glissement: 0.4,
                delta_max: 4,
                forts: 30,
            }),
        };
        let mieux = Base {
            delta_max: 90,
            pourcent: 0.5,
            ombre_delta_max: 1,
            ombre_pourcent: 4.0,
            arrondi_delta_max: 5,
            arrondi_pourcent: 9.0,
            non_jugeables: 2,
            lettres_manquees: 1,
            rasterisation_glyphes: 3,
            exception: Some(BaseException {
                etalement: -1.4,
                encre: 0.2,
                glissement: -0.3,
                delta_max: 3,
                forts: 20,
            }),
        };
        assert_eq!(verifier_cliquet(mieux, Some(&base)), None);
        assert_eq!(verifier_cliquet(base, Some(&base)), None);
        let pire_delta = Base {
            delta_max: 101,
            ..base
        };
        assert!(verifier_cliquet(pire_delta, Some(&base)).is_some());
        let pire_part = Base {
            pourcent: 1.0001,
            ..base
        };
        assert!(verifier_cliquet(pire_part, Some(&base)).is_some());
        // Les mêmes règles pour l'ombre (amendement du 11/11).
        let pire_ombre = Base {
            ombre_delta_max: 3,
            ..base
        };
        assert!(verifier_cliquet(pire_ombre, Some(&base)).is_some());
        let pire_halo = Base {
            ombre_pourcent: 5.0001,
            ..base
        };
        assert!(verifier_cliquet(pire_halo, Some(&base)).is_some());
        // Et pour les arrondis (amendement de 11:15, point a).
        let pire_arrondi = Base {
            arrondi_delta_max: 7,
            ..base
        };
        assert!(verifier_cliquet(pire_arrondi, Some(&base)).is_some());
        let pire_bande = Base {
            arrondi_pourcent: 10.0001,
            ..base
        };
        assert!(verifier_cliquet(pire_bande, Some(&base)).is_some());
        // Les glyphes non jugeables (fr-p3.1) : dette nommée, en cliquet.
        let plus_de_fragments = Base {
            non_jugeables: 4,
            ..base
        };
        assert!(verifier_cliquet(plus_de_fragments, Some(&base)).is_some());
        // Les lettres manquées par la contre-épreuve (fr-p3.1, 13/11 03:15) : idem.
        let plus_de_manquees = Base {
            lettres_manquees: 3,
            ..base
        };
        assert!(verifier_cliquet(plus_de_manquees, Some(&base)).is_some());
        // La dette « rastérisation des glyphes » (fr-p4.1.3) : une boîte de plus fait échouer.
        let pire_rasterisation = Base {
            rasterisation_glyphes: 5,
            ..base
        };
        assert!(verifier_cliquet(pire_rasterisation, Some(&base)).is_some());
        // L'exception --shadow-l : chaque valeur mesurée fait cliquet, en valeur absolue.
        let e = base.exception.unwrap_or_default();
        for pire in [
            BaseException {
                etalement: -1.51,
                ..e
            },
            BaseException { encre: 0.31, ..e },
            BaseException {
                glissement: -0.41,
                ..e
            },
            BaseException { delta_max: 5, ..e },
            BaseException { forts: 31, ..e },
        ] {
            let m = Base {
                exception: Some(pire),
                ..base
            };
            assert!(verifier_cliquet(m, Some(&base)).is_some(), "{pire:?}");
        }
        let sans_base = Base {
            exception: None,
            ..base
        };
        assert!(verifier_cliquet(base, Some(&sans_base)).is_some());
        // Rien à publier si rien ne diverge dans les boîtes ; sinon une base est exigée.
        let zero = Base::default();
        assert_eq!(verifier_cliquet(zero, None), None);
        assert!(verifier_cliquet(base, None).is_some());
    }

    #[test]
    fn le_rapport_ne_touche_que_sa_section() {
        let d = dossier_temporaire("rapport");
        let f = d.join("parite.md");
        std::fs::write(
            &f,
            "# Dettes
écrites à la main
",
        )
        .unwrap();
        ecrire_rapport(
            &f, "premier
",
        )
        .unwrap();
        ecrire_rapport(
            &f, "second
",
        )
        .unwrap();
        let texte = std::fs::read_to_string(&f).unwrap();
        assert!(texte.starts_with(
            "# Dettes
écrites à la main"
        ));
        assert!(texte.contains("second") && !texte.contains("premier"));
        assert_eq!(texte.matches(DEBUT_RAPPORT).count(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
