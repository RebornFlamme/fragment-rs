//! `cargo xtask mesure-glyphes` — écart de placement des glyphes, candidat contre référence (fr-p4.1.1).
//!
//! Entrées : un dossier de relevés `<scène>-<thème>.glyphes.json` (rectangles par caractère,
//! `Range.getClientRects` côté Fragment, outil hors dépôt `C:/dev/fragment-rs-mesure-glyphes`), les PNG
//! de référence et les PNG candidats de mêmes noms.
//!
//! Pour chaque caractère, on prend la même fenêtre (sa boîte d'avance, arrondie au pixel) dans les
//! deux images et on compare le barycentre de l'encre : `dx`, `dy` = candidat − référence, en pixels.
//! L'encre d'un pixel est son plus grand écart de canal au fond, et le fond la couleur la plus
//! fréquente du pourtour de la fenêtre. Une fenêtre où l'une des deux images n'a presque pas d'encre,
//! ou dont les masses diffèrent du simple au double, n'est pas jugée (contenu différent, pas un
//! déplacement).
//!
//! ★ On compare de l'encre à de l'encre : la boîte de Chromium ne sert qu'à fenêtrer et à lire la
//! phase sous-pixel du stylo (`x − floor(x)`). Si GPUI calait le stylo au pixel entier et Chromium au
//! sous-pixel, `dx` suivrait `round(x) − x` (dent de scie selon la phase) ; s'ils quantifient pareil,
//! `dx` est plat en phase.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use image::RgbaImage;

/// Un caractère relevé côté Fragment (pixels de l'image).
#[derive(Debug, Clone)]
struct Releve {
    c: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    police: usize,
    mot: u64,
}

/// La mesure d'un caractère.
#[derive(Debug, Clone)]
pub struct Mesure {
    pub scene: String,
    pub c: String,
    pub police: String,
    pub mot: u64,
    pub x: f64,
    pub dx: f64,
    pub dy: f64,
}

/// Barycentre de l'encre d'une fenêtre : (masse, cx, cy) en coordonnées image.
pub fn barycentre(img: &RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) -> Option<(f64, f64, f64)> {
    if x1 <= x0 + 1 || y1 <= y0 + 1 || x1 > img.width() || y1 > img.height() {
        return None;
    }
    // Fond : couleur la plus fréquente du pourtour.
    let mut compte: HashMap<[u8; 3], u32> = HashMap::new();
    for x in x0..x1 {
        for y in [y0, y1 - 1] {
            let p = img.get_pixel(x, y).0;
            *compte.entry([p[0], p[1], p[2]]).or_default() += 1;
        }
    }
    for y in y0..y1 {
        for x in [x0, x1 - 1] {
            let p = img.get_pixel(x, y).0;
            *compte.entry([p[0], p[1], p[2]]).or_default() += 1;
        }
    }
    let fond = compte.into_iter().max_by_key(|(c, n)| (*n, *c))?.0;
    let (mut m, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = img.get_pixel(x, y).0;
            let e = (0..3)
                .map(|i| (i32::from(p[i]) - i32::from(fond[i])).unsigned_abs())
                .max()
                .unwrap_or(0) as f64;
            m += e;
            sx += e * (f64::from(x) + 0.5);
            sy += e * (f64::from(y) + 0.5);
        }
    }
    (m > 0.0).then(|| (m, sx / m, sy / m))
}

/// Masse d'encre minimale (en niveaux) pour juger une fenêtre : trois pixels pleins.
const MASSE_MIN: f64 = 3.0 * 255.0;

/// Mesure les caractères d'une scène.
fn mesurer_scene(
    scene: &str,
    releves: &[Releve],
    polices: &[String],
    reference: &RgbaImage,
    candidat: &RgbaImage,
) -> Vec<Mesure> {
    let mut sortie = Vec::new();
    for r in releves {
        let x0 = r.x.floor().max(0.0) as u32;
        let y0 = r.y.floor().max(0.0) as u32;
        let x1 = ((r.x + r.w).ceil() as u32).min(reference.width());
        let y1 = ((r.y + r.h).ceil() as u32).min(reference.height());
        let (Some((mr, xr, yr)), Some((mc, xc, yc))) = (
            barycentre(reference, x0, y0, x1, y1),
            barycentre(candidat, x0, y0, x1, y1),
        ) else {
            continue;
        };
        if mr < MASSE_MIN || mc < MASSE_MIN || mc / mr > 2.0 || mr / mc > 2.0 {
            continue;
        }
        sortie.push(Mesure {
            scene: scene.to_string(),
            c: r.c.clone(),
            police: polices.get(r.police).cloned().unwrap_or_default(),
            mot: r.mot,
            x: r.x,
            dx: xc - xr,
            dy: yc - yr,
        });
    }
    sortie
}

/// Mesure les MOTS d'une scène : fenêtre = boîtes du mot élargies de 2 px à gauche et à droite.
///
/// ★ La fenêtre d'un caractère seul coupe l'encre déplacée : un « m » décalé de −0,7 px y mesure
/// −0,2 px. Les mots sont séparés par des espaces, l'élargissement n'y prend presque rien du voisin.
fn mesurer_mots(
    scene: &str,
    releves: &[Releve],
    polices: &[String],
    reference: &RgbaImage,
    candidat: &RgbaImage,
) -> Vec<Mesure> {
    let mut sortie = Vec::new();
    let mut debut = 0;
    while debut < releves.len() {
        let mut fin = debut + 1;
        while fin < releves.len() && releves[fin].mot == releves[debut].mot {
            fin += 1;
        }
        let mot = &releves[debut..fin];
        debut = fin;
        let (premier, dernier) = (&mot[0], &mot[mot.len() - 1]);
        let x0 = (premier.x.floor() - 2.0).max(0.0) as u32;
        let x1 = (((dernier.x + dernier.w).ceil() + 2.0) as u32).min(reference.width());
        let y0 = mot
            .iter()
            .map(|r| r.y)
            .fold(f64::MAX, f64::min)
            .floor()
            .max(0.0) as u32;
        let y1 = (mot.iter().map(|r| r.y + r.h).fold(0.0, f64::max).ceil() as u32)
            .min(reference.height());
        let (Some((mr, xr, yr)), Some((mc, xc, yc))) = (
            barycentre(reference, x0, y0, x1, y1),
            barycentre(candidat, x0, y0, x1, y1),
        ) else {
            continue;
        };
        if mr < MASSE_MIN || mc < MASSE_MIN || mc / mr > 1.6 || mr / mc > 1.6 {
            continue;
        }
        sortie.push(Mesure {
            scene: scene.to_string(),
            c: mot.iter().map(|r| r.c.as_str()).collect(),
            police: polices.get(premier.police).cloned().unwrap_or_default(),
            mot: premier.mot,
            x: premier.x,
            dx: xc - xr,
            dy: yc - yr,
        });
    }
    sortie
}

fn lire_releves(chemin: &Path) -> anyhow::Result<(Vec<String>, Vec<Releve>)> {
    let texte = std::fs::read_to_string(chemin)?;
    let v: serde_json::Value = serde_json::from_str(&texte)?;
    let polices = v["polices"]
        .as_array()
        .context("polices")?
        .iter()
        .map(|p| p.as_str().unwrap_or_default().to_string())
        .collect();
    let mut releves = Vec::new();
    for g in v["glyphes"].as_array().context("glyphes")? {
        releves.push(Releve {
            c: g["c"].as_str().unwrap_or_default().to_string(),
            x: g["x"].as_f64().unwrap_or_default(),
            y: g["y"].as_f64().unwrap_or_default(),
            w: g["w"].as_f64().unwrap_or_default(),
            h: g["h"].as_f64().unwrap_or_default(),
            police: g["p"].as_u64().unwrap_or_default() as usize,
            mot: g["m"].as_u64().unwrap_or_default(),
        });
    }
    Ok((polices, releves))
}

/// Statistiques d'un groupe de mesures.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub n: usize,
    pub moy_dx: f64,
    pub et_dx: f64,
    pub moy_abs_dx: f64,
    pub part_dx_025: f64,
    pub moy_dy: f64,
    pub moy_abs_dy: f64,
}

pub fn stats(m: &[&Mesure]) -> Stats {
    let n = m.len();
    if n == 0 {
        return Stats::default();
    }
    let nf = n as f64;
    let moy_dx = m.iter().map(|g| g.dx).sum::<f64>() / nf;
    let et_dx = (m.iter().map(|g| (g.dx - moy_dx).powi(2)).sum::<f64>() / nf).sqrt();
    Stats {
        n,
        moy_dx,
        et_dx,
        moy_abs_dx: m.iter().map(|g| g.dx.abs()).sum::<f64>() / nf,
        part_dx_025: m.iter().filter(|g| g.dx.abs() > 0.25).count() as f64 / nf,
        moy_dy: m.iter().map(|g| g.dy).sum::<f64>() / nf,
        moy_abs_dy: m.iter().map(|g| g.dy.abs()).sum::<f64>() / nf,
    }
}

/// Pente de `dx` le long des mots (px par caractère, moindres carrés sur les mots de 3 caractères ou
/// plus, `dx` recentré sur le mot) : non nulle si les avances diffèrent.
pub fn pente_mots(m: &[&Mesure]) -> (f64, usize) {
    let mut mots: BTreeMap<(&str, u64), Vec<&Mesure>> = BTreeMap::new();
    for g in m {
        mots.entry((g.scene.as_str(), g.mot)).or_default().push(g);
    }
    let (mut sxy, mut sxx, mut k) = (0.0, 0.0, 0);
    for v in mots.values().filter(|v| v.len() >= 3) {
        let n = v.len() as f64;
        let mi = (n - 1.0) / 2.0;
        let md = v.iter().map(|g| g.dx).sum::<f64>() / n;
        for (i, g) in v.iter().enumerate() {
            sxy += (i as f64 - mi) * (g.dx - md);
            sxx += (i as f64 - mi).powi(2);
        }
        k += 1;
    }
    (if sxx > 0.0 { sxy / sxx } else { 0.0 }, k)
}

fn ligne_stats(s: &Stats) -> String {
    format!(
        "{} | {:+.3} | {:.3} | {:.3} | {:.0} % | {:+.3} | {:.3}",
        s.n,
        s.moy_dx,
        s.et_dx,
        s.moy_abs_dx,
        100.0 * s.part_dx_025,
        s.moy_dy,
        s.moy_abs_dy
    )
}

/// Le rapport en Markdown.
pub fn rapport(mesures: &[Mesure]) -> String {
    let mut o = String::new();
    let tout: Vec<&Mesure> = mesures.iter().collect();
    let entete = "| n | moy dx | é.-t. dx | moy abs dx | abs dx > 0,25 | moy dy | moy abs dy |";
    let sep = "|---|---|---|---|---|---|---|";
    let _ = writeln!(
        o,
        "### Ensemble\n\n{entete}\n{sep}\n| {} |",
        ligne_stats(&stats(&tout))
    );
    let (p, k) = pente_mots(&tout);
    let _ = writeln!(
        o,
        "\nPente de dx le long des mots : {p:+.4} px par caractère ({k} mots).\n"
    );

    let _ = writeln!(
        o,
        "### Par phase sous-pixel du stylo de Chromium (x − floor(x))\n"
    );
    let _ = writeln!(o, "| phase |{}\n|---{sep}", &entete[1..]);
    for b in 0..8 {
        let (a, z) = (b as f64 / 8.0, (b + 1) as f64 / 8.0);
        let g: Vec<&Mesure> = mesures
            .iter()
            .filter(|m| {
                let f = m.x - m.x.floor();
                f >= a && f < z
            })
            .collect();
        let _ = writeln!(o, "| [{a:.3} ; {z:.3}[ | {} |", ligne_stats(&stats(&g)));
    }

    let _ = writeln!(
        o,
        "\n### Par police (famille | taille | graisse | style | interlettrage)\n"
    );
    let _ = writeln!(o, "| police |{} pente/car. |\n|---{sep}---|", &entete[1..]);
    let mut par_police: BTreeMap<&str, Vec<&Mesure>> = BTreeMap::new();
    for m in mesures {
        par_police.entry(m.police.as_str()).or_default().push(m);
    }
    for (pol, g) in &par_police {
        if g.len() < 20 {
            continue;
        }
        let (p, _) = pente_mots(g);
        let nom = pol.replace('|', " \\| ");
        let _ = writeln!(o, "| {nom} | {} | {p:+.4} |", ligne_stats(&stats(g)));
    }

    let _ = writeln!(o, "\n### Par texte (30 premiers par effectif)\n");
    let _ = writeln!(o, "| texte |{}\n|---{sep}", &entete[1..]);
    let mut par_car: BTreeMap<&str, Vec<&Mesure>> = BTreeMap::new();
    for m in mesures {
        par_car.entry(m.c.as_str()).or_default().push(m);
    }
    let mut cars: Vec<_> = par_car.into_iter().collect();
    cars.sort_by_key(|(_, g)| std::cmp::Reverse(g.len()));
    for (c, g) in cars.iter().take(30) {
        let _ = writeln!(o, "| `{c}` | {} |", ligne_stats(&stats(g)));
    }

    let _ = writeln!(o, "\n### Par scène\n");
    let _ = writeln!(o, "| scène |{}\n|---{sep}", &entete[1..]);
    let mut par_scene: BTreeMap<&str, Vec<&Mesure>> = BTreeMap::new();
    for m in mesures {
        par_scene.entry(m.scene.as_str()).or_default().push(m);
    }
    for (s, g) in &par_scene {
        let _ = writeln!(o, "| {s} | {} |", ligne_stats(&stats(g)));
    }
    o
}

pub fn run(args: &[String]) -> anyhow::Result<bool> {
    let mut releves = None;
    let mut reference = PathBuf::from("pixels/reference");
    let mut candidat = PathBuf::from("pixels/candidate");
    let mut sortie = None;
    let mut csv = None;
    let mut filtre: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let val = || args.get(i + 1).cloned().context("valeur manquante");
        match args[i].as_str() {
            "--releves" => releves = Some(PathBuf::from(val()?)),
            "--reference" => reference = PathBuf::from(val()?),
            "--candidate" => candidat = PathBuf::from(val()?),
            "--report" => sortie = Some(PathBuf::from(val()?)),
            "--csv" => csv = Some(PathBuf::from(val()?)),
            "--prefixe" => filtre.push(val()?),
            autre => bail!("argument inconnu : {autre}"),
        }
        i += 2;
    }
    let releves = releves.context("--releves D est requis")?;
    let mut fichiers: Vec<_> = std::fs::read_dir(&releves)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".glyphes.json"))
        .collect();
    fichiers.sort();
    let mut mesures = Vec::new();
    let mut mots = Vec::new();
    for f in fichiers {
        let nom = f
            .file_name()
            .map(|n| n.to_string_lossy().replace(".glyphes.json", ""))
            .unwrap_or_default();
        if !filtre.is_empty() && !filtre.iter().any(|p| nom.starts_with(p.as_str())) {
            continue;
        }
        let (r, c) = (
            reference.join(format!("{nom}.png")),
            candidat.join(format!("{nom}.png")),
        );
        if !r.exists() || !c.exists() {
            continue;
        }
        let (ri, ci) = (image::open(&r)?.to_rgba8(), image::open(&c)?.to_rgba8());
        if ri.dimensions() != ci.dimensions() {
            eprintln!("{nom} : tailles différentes, ignorée");
            continue;
        }
        let (polices, rel) = lire_releves(&f)?;
        let m = mesurer_scene(&nom, &rel, &polices, &ri, &ci);
        let w = mesurer_mots(&nom, &rel, &polices, &ri, &ci);
        println!(
            "{nom} : {} caractères relevés, {} jugés, {} mots jugés",
            rel.len(),
            m.len(),
            w.len()
        );
        mesures.extend(m);
        mots.extend(w);
    }
    let mut texte = format!(
        "## Par caractère (fenêtre = boîte d'avance)\n\n{}\n## Par mot (fenêtre élargie de 2 px)\n\n{}",
        rapport(&mesures),
        rapport(&mots)
    );
    let mut pires: Vec<&Mesure> = mots.iter().collect();
    pires.sort_by(|a, b| b.dx.abs().total_cmp(&a.dx.abs()));
    let _ = writeln!(
        texte,
        "\n### Les 20 plus grands écarts par mot\n\n| scène | mot | x Chromium | dx | dy |\n|---|---|---|---|---|"
    );
    for m in pires.iter().take(20) {
        let _ = writeln!(
            texte,
            "| {} | `{}` | {:.3} | {:+.2} | {:+.2} |",
            m.scene, m.c, m.x, m.dx, m.dy
        );
    }
    match sortie {
        Some(s) => std::fs::write(s, &texte)?,
        None => println!("{texte}"),
    }
    if let Some(csv) = csv {
        let mut t = String::from("scene\tcar\tpolice\tmot\tx\tdx\tdy\n");
        for m in &mesures {
            let _ = writeln!(
                t,
                "{}\t{}\t{}\t{}\t{:.3}\t{:.4}\t{:.4}",
                m.scene, m.c, m.police, m.mot, m.x, m.dx, m.dy
            );
        }
        std::fs::write(csv, t)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_avec_trait(x: u32) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(10, 6, image::Rgba([0, 0, 0, 255]));
        for y in 1..5 {
            img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
        }
        img
    }

    #[test]
    fn le_barycentre_suit_un_trait_deplace_d_un_pixel() {
        let a = barycentre(&image_avec_trait(4), 0, 0, 10, 6).unwrap();
        let b = barycentre(&image_avec_trait(5), 0, 0, 10, 6).unwrap();
        assert!((b.1 - a.1 - 1.0).abs() < 1e-9);
        assert!((a.2 - b.2).abs() < 1e-9);
    }

    #[test]
    fn la_mesure_au_mot_voit_l_encre_sortie_de_la_boite() {
        // Trait de 2 px : en x=4-5 côté référence, x=3-4 côté candidat (décalé d'un pixel à gauche).
        let trait_ = |x: u32| {
            let mut img = RgbaImage::from_pixel(12, 8, image::Rgba([0, 0, 0, 255]));
            for y in 1..7 {
                for dx in 0..2 {
                    img.put_pixel(x + dx, y, image::Rgba([255, 255, 255, 255]));
                }
            }
            img
        };
        let (r, c) = (trait_(4), trait_(3));
        let rel = [Releve {
            c: "l".into(),
            x: 4.0,
            y: 0.0,
            w: 2.0,
            h: 8.0,
            police: 0,
            mot: 1,
        }];
        let pol = ["p".to_string()];
        let mots = mesurer_mots("s", &rel, &pol, &r, &c);
        assert_eq!(mots.len(), 1);
        assert!((mots[0].dx + 1.0).abs() < 1e-9);
    }

    #[test]
    fn la_pente_des_mots_detecte_une_derive_d_avance() {
        let m: Vec<Mesure> = (0..5)
            .map(|i| Mesure {
                scene: "s".into(),
                c: "a".into(),
                police: "p".into(),
                mot: 1,
                x: 0.0,
                dx: 0.1 * f64::from(i),
                dy: 0.0,
            })
            .collect();
        let r: Vec<&Mesure> = m.iter().collect();
        assert!((pente_mots(&r).0 - 0.1).abs() < 1e-9);
    }
}
