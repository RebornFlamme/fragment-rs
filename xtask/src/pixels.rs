//! Diff d'images référence (Fragment) / candidat (fragment-rs), program.md « Parité au PIXEL ».
//!
//! Deux niveaux :
//! - STRICT : 0 pixel différent (la cible) ;
//! - TOLÉRÉ : delta de canal ≤ 16 ET ≤ 0,3 % des pixels ET chaque zone divergente
//!   tient dans moins de 20 × 20 px (bords de glyphes). ★ Tout le reste (boîtes, traits,
//!   rayons, fonds, positions) doit être strict : une grande zone = une vraie divergence.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use image::{Rgba, RgbaImage};

/// Delta de canal maximal toléré sur un pixel divergent.
pub const DELTA_TOLERE: u8 = 16;
/// Part maximale de pixels divergents tolérée (0,3 %).
pub const PART_TOLEREE: f64 = 0.003;
/// Côté maximal (exclu) de la boîte d'une zone divergente tolérée.
pub const ZONE_MAX: u32 = 20;

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

#[derive(Debug)]
pub struct Comparaison {
    pub differents: u64,
    pub total: u64,
    pub pire_delta: u8,
    pub zones: Vec<Boite>,
    pub verdict: Verdict,
    pub diff: RgbaImage,
}

/// Compare deux images de même taille. Une taille différente est un échec franc.
pub fn comparer(reference: &RgbaImage, candidat: &RgbaImage) -> anyhow::Result<Comparaison> {
    if reference.dimensions() != candidat.dimensions() {
        bail!(
            "tailles différentes : référence {:?}, candidat {:?}",
            reference.dimensions(),
            candidat.dimensions()
        );
    }
    let (largeur, hauteur) = reference.dimensions();
    let mut masque = vec![false; (largeur * hauteur) as usize];
    let mut diff = RgbaImage::new(largeur, hauteur);
    let mut differents = 0u64;
    let mut pire_delta = 0u8;
    for (x, y, r) in reference.enumerate_pixels() {
        let c = candidat.get_pixel(x, y);
        let delta = (0..4).map(|i| r.0[i].abs_diff(c.0[i])).max().unwrap_or(0);
        if delta > 0 {
            differents += 1;
            pire_delta = pire_delta.max(delta);
            masque[(y * largeur + x) as usize] = true;
            diff.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        } else {
            // Référence assombrie en fond, pour situer les divergences.
            let g = (u16::from(r.0[0]) + u16::from(r.0[1]) + u16::from(r.0[2])) / 12;
            let g = u8::try_from(g).unwrap_or(u8::MAX);
            diff.put_pixel(x, y, Rgba([g, g, g, 255]));
        }
    }
    let zones = zones_connexes(&masque, largeur, hauteur);
    let total = u64::from(largeur) * u64::from(hauteur);
    let verdict = if differents == 0 {
        Verdict::Strict
    } else if pire_delta <= DELTA_TOLERE
        && (differents as f64) <= PART_TOLEREE * total as f64
        && zones
            .iter()
            .all(|b| b.largeur < ZONE_MAX && b.hauteur < ZONE_MAX)
    {
        Verdict::Tolere
    } else {
        Verdict::Echec
    };
    Ok(Comparaison {
        differents,
        total,
        pire_delta,
        zones,
        verdict,
        diff,
    })
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

/// La tranche en cours. ★ Même valeur que `TRANCHE_COURANTE` de `fragment-app` (src/pixels/actions.rs) :
/// une scène marquée `"depuis": "T3"` n'est ni rejouée ni comparée avant que cette tranche l'atteigne.
pub const TRANCHE_COURANTE: u32 = 1;

struct Options {
    scene: Option<String>,
    reference: PathBuf,
    candidat: PathBuf,
    diff: PathBuf,
    rapport: PathBuf,
    scenes: PathBuf,
    tranche: u32,
    exiger_references: bool,
}

fn lire_options(args: &[String]) -> anyhow::Result<Options> {
    let mut o = Options {
        scene: None,
        reference: "pixels/reference".into(),
        candidat: "pixels/candidate".into(),
        diff: "pixels/diff".into(),
        rapport: "docs/parite-pixels.md".into(),
        scenes: "pixels/scenes".into(),
        tranche: TRANCHE_COURANTE,
        exiger_references: false,
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
            "--tranche" => {
                let v = valeur()?;
                o.tranche =
                    lire_tranche(&v).with_context(|| format!("tranche « {v} » inconnue"))?;
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

fn planifier(
    references: &BTreeSet<String>,
    candidats: &BTreeSet<String>,
    meta: &BTreeMap<String, MetaScene>,
    tranche: u32,
    exiger: bool,
    filtre: Option<&str>,
) -> Plan {
    let hors = |png: &str| {
        meta.get(scene_du_png(png))
            .is_some_and(|m| m.depuis.is_some_and(|d| d > tranche))
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
            if !dans_filtre || m.sans_reference || m.depuis.is_some_and(|d| d > tranche) {
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

/// Point d'entrée de `cargo xtask pixels`. Rend `Ok(true)` si toutes les scènes passent.
///
/// - divergence hors tolérance : échec ;
/// - référence absente : EN ATTENTE, code 0 (la référence d'une scène arrive quand Fragment l'a
///   rendue) ;
/// - `--exiger-references` : toute scène de la tranche doit avoir référence ET candidat, sinon échec.
pub fn run(args: &[String]) -> anyhow::Result<bool> {
    let o = lire_options(args)?;
    let meta = lire_meta(&o.scenes)?;
    let references = pngs(&o.reference);
    let candidats = pngs(&o.candidat);
    let plan = planifier(
        &references,
        &candidats,
        &meta,
        o.tranche,
        o.exiger_references,
        o.scene.as_deref(),
    );
    std::fs::create_dir_all(&o.diff)?;

    let mut ok = true;
    let mut rapport = String::from(
        "# Parité au pixel\n\nGénéré par `cargo xtask pixels`. Strict = 0 pixel ; toléré = delta ≤ 16, ≤ 0,3 %, zones < 20×20 px (glyphes).\n\n| Scène | Verdict | Pixels différents | % | Pire delta | Zones (x,y,l×h) |\n|---|---|---|---|---|---|\n",
    );
    for nom in &plan.paires {
        let r = image::open(o.reference.join(nom))?.to_rgba8();
        let c = image::open(o.candidat.join(nom))?.to_rgba8();
        let ligne = match comparer(&r, &c) {
            Ok(cmp) => {
                cmp.diff.save(o.diff.join(nom))?;
                if cmp.verdict == Verdict::Echec {
                    ok = false;
                }
                let zones: Vec<String> = cmp
                    .zones
                    .iter()
                    .take(8)
                    .map(|b| format!("{},{},{}×{}", b.x, b.y, b.largeur, b.hauteur))
                    .collect();
                format!(
                    "| {nom} | {:?} | {} | {:.4} | {} | {}{} |",
                    cmp.verdict,
                    cmp.differents,
                    100.0 * cmp.differents as f64 / cmp.total as f64,
                    cmp.pire_delta,
                    zones.join(" ; "),
                    if cmp.zones.len() > 8 { " …" } else { "" }
                )
            }
            Err(erreur) => {
                ok = false;
                format!("| {nom} | Echec | — | — | — | {erreur} |")
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
            writeln!(rapport, "| {nom} | {etat} | — | — | — | — |")?;
        }
        if !liste.is_empty() {
            println!("{etat} : {}", liste.join(", "));
        }
    }
    for nom in &plan.manquants {
        ok = false;
        writeln!(
            rapport,
            "| {nom} | MANQUANT (--exiger-references) | — | — | — | — |"
        )?;
        eprintln!(
            "xtask pixels : {nom} manque (référence ou candidat), exigé par --exiger-references"
        );
    }
    if plan.paires.is_empty() && !o.exiger_references {
        println!("xtask pixels : aucune paire référence/candidat à comparer (en attente)");
    }
    if let Some(parent) = o.rapport.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&o.rapport, rapport)?;
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unie(l: u32, h: u32, c: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(l, h, Rgba(c))
    }

    #[test]
    fn identiques_strict() {
        let a = unie(10, 10, [1, 2, 3, 255]);
        let cmp = comparer(&a, &a.clone()).unwrap();
        assert_eq!(cmp.verdict, Verdict::Strict);
        assert_eq!(cmp.differents, 0);
    }

    #[test]
    fn petit_ecart_de_glyphe_tolere() {
        let a = unie(100, 100, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.put_pixel(5, 5, Rgba([10, 10, 10, 255]));
        b.put_pixel(6, 5, Rgba([10, 10, 10, 255]));
        let cmp = comparer(&a, &b).unwrap();
        assert_eq!(cmp.verdict, Verdict::Tolere);
        assert_eq!(
            cmp.zones,
            vec![Boite {
                x: 5,
                y: 5,
                largeur: 2,
                hauteur: 1
            }]
        );
    }

    #[test]
    fn delta_fort_echoue() {
        let a = unie(100, 100, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.put_pixel(5, 5, Rgba([200, 0, 0, 255]));
        assert_eq!(comparer(&a, &b).unwrap().verdict, Verdict::Echec);
    }

    #[test]
    fn grande_zone_echoue() {
        let a = unie(100, 100, [0, 0, 0, 255]);
        let mut b = a.clone();
        for x in 0..25 {
            b.put_pixel(x, 0, Rgba([1, 1, 1, 255]));
        }
        assert_eq!(comparer(&a, &b).unwrap().verdict, Verdict::Echec);
    }

    #[test]
    fn tailles_differentes_echouent() {
        assert!(comparer(&unie(2, 2, [0; 4]), &unie(3, 2, [0; 4])).is_err());
    }

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
    fn reference_absente_est_en_attente() {
        let plan = planifier(
            &ens(&[]),
            &ens(&["t1-onglets-dark.png"]),
            &meta(),
            1,
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
            1,
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
            7,
            false,
            None,
        );
        assert_eq!(plan.paires, vec!["menu-dark.png"]);
    }

    #[test]
    fn une_scene_de_preuve_n_attend_pas_de_reference() {
        let plan = planifier(&ens(&[]), &ens(&["mire-dark.png"]), &meta(), 1, true, None);
        assert_eq!(plan.preuves, vec!["mire-dark.png"]);
        assert!(!plan.manquants.iter().any(|m| m.starts_with("mire")));
    }

    #[test]
    fn exiger_references_transforme_l_attente_en_echec() {
        let plan = planifier(
            &ens(&["t1-onglets-dark.png"]),
            &ens(&["t1-onglets-dark.png"]),
            &meta(),
            1,
            true,
            None,
        );
        // Le thème clair n'existe ni côté référence ni côté candidat.
        assert_eq!(plan.manquants, vec!["t1-onglets-light.png"]);
        let sans = planifier(
            &ens(&["t1-onglets-dark.png"]),
            &ens(&["t1-onglets-dark.png"]),
            &meta(),
            1,
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
            1,
            true,
            Some("mire"),
        );
        assert_eq!(plan.preuves, vec!["mire-dark.png"]);
        assert!(plan.en_attente.is_empty() && plan.manquants.is_empty());
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
        assert_eq!(o.tranche, 3);
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
}
