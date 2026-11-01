//! Diff d'images référence (Fragment) / candidat (fragment-rs), program.md « Parité au PIXEL ».
//!
//! Deux niveaux :
//! - STRICT : 0 pixel différent (la cible) ;
//! - TOLÉRÉ : delta de canal ≤ 16 ET ≤ 0,3 % des pixels ET chaque zone divergente
//!   tient dans moins de 20 × 20 px (bords de glyphes). ★ Tout le reste (boîtes, traits,
//!   rayons, fonds, positions) doit être strict : une grande zone = une vraie divergence.

use std::collections::BTreeSet;
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

struct Options {
    scene: Option<String>,
    reference: PathBuf,
    candidat: PathBuf,
    diff: PathBuf,
    rapport: PathBuf,
    exiger_tout: bool,
}

fn lire_options(args: &[String]) -> anyhow::Result<Options> {
    let mut o = Options {
        scene: None,
        reference: "pixels/reference".into(),
        candidat: "pixels/candidate".into(),
        diff: "pixels/diff".into(),
        rapport: "docs/parite-pixels.md".into(),
        exiger_tout: false,
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
            "--require-all" => o.exiger_tout = true,
            s if s.starts_with("--") => bail!("option inconnue : {s}"),
            s => o.scene = Some(s.to_owned()),
        }
    }
    Ok(o)
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

/// Point d'entrée de `cargo xtask pixels`. Rend `Ok(true)` si toutes les scènes passent.
pub fn run(args: &[String]) -> anyhow::Result<bool> {
    let o = lire_options(args)?;
    let filtre = |n: &String| o.scene.as_ref().is_none_or(|s| n.starts_with(s.as_str()));
    let references: BTreeSet<String> = pngs(&o.reference).into_iter().filter(filtre).collect();
    let candidats: BTreeSet<String> = pngs(&o.candidat).into_iter().filter(filtre).collect();
    std::fs::create_dir_all(&o.diff)?;

    let mut ok = true;
    let mut rapport = String::from(
        "# Parité au pixel\n\nGénéré par `cargo xtask pixels`. Strict = 0 pixel ; toléré = delta ≤ 16, ≤ 0,3 %, zones < 20×20 px (glyphes).\n\n| Scène | Verdict | Pixels différents | % | Pire delta | Zones (x,y,l×h) |\n|---|---|---|---|---|---|\n",
    );
    for nom in references.intersection(&candidats) {
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
    for nom in references.difference(&candidats) {
        if o.exiger_tout {
            ok = false;
        }
        writeln!(rapport, "| {nom} | sans candidat | — | — | — | — |")?;
    }
    for nom in candidats.difference(&references) {
        writeln!(rapport, "| {nom} | sans référence | — | — | — | — |")?;
    }
    if references.intersection(&candidats).next().is_none() {
        eprintln!("xtask pixels : aucune paire référence/candidat à comparer");
        ok = false;
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
}
