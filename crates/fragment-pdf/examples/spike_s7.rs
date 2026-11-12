//! Spike S7 : le raster d'une page par pdfium est-il identique au pixel à celui de pdf.js (Fragment) ?
//!
//! Trois étapes, chiffres reproductibles (dossier de travail HORS dépôt, par défaut C:/dev/fragment-rs-s7) :
//!
//! ```text
//! cargo run -p fragment-pdf --example spike_s7 -- fixture   [dossier]   # fixture.pdf
//! (capture pdf.js : electron capture/main.js, voir docs/spikes/S7-raster-pdf.md)
//! cargo run -p fragment-pdf --example spike_s7 -- rendre    [dossier]   # pdfium-<options>-sNNN.rgba
//! cargo run -p fragment-pdf --example spike_s7 -- document  [dossier]   # pdfium-document-sNNN.rgba (via Document)
//! cargo run -p fragment-pdf --example spike_s7 -- comparer  [dossier]   # tableau des écarts par zone
//! cargo run -p fragment-pdf --example spike_s7 -- epreuves  [dossier]   # contre-épreuves de la catégorie
//! ```
//!
//! Les rasters sont en RGBA brut, lignes jointives, de taille `round(pt × échelle)` (celle du canvas
//! de Fragment à densité 1 : `canvas.width = pixels.w`, transform `[w/vw, 0, 0, h/vh]`).
//!
//! `rendre` et `document` tournent dans des PROCESSUS séparés : `rendre` lie pdfium lui-même (pour
//! essayer les drapeaux que `Document` n'expose pas), `document` passe par le moteur du crate ; deux
//! liaisons dans un même processus initialiseraient la bibliothèque deux fois.

use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fragment_pdf::document::{Document, initialiser};
use pdfium_render::prelude::*;

type Resultat<T> = Result<T, Box<dyn Error>>;

/// Taille de la page de fixture, en points.
const PAGE: (f64, f64) = (400.0, 300.0);
/// Les trois échelles du spike (pixels physiques par point).
const ECHELLES: [f64; 3] = [0.5, 1.0, 2.0];

/// Zones déclarées depuis la MISE EN PAGE de la fixture (points, origine en haut à gauche),
/// jamais déduites des pixels : (nom, gauche, haut, droite, bas).
const ZONES: [(&str, f64, f64, f64, f64); 4] = [
    ("texte", 10.0, 10.0, 390.0, 112.0),
    ("traits", 10.0, 118.0, 196.0, 290.0),
    ("aplats", 204.0, 118.0, 296.0, 290.0),
    ("image", 300.0, 118.0, 390.0, 210.0),
];

const POLICE_DM_SANS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fragment-ui/assets/fonts/DMSans-400.ttf"
);

fn main() -> Resultat<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let commande = args.first().map(String::as_str).unwrap_or("");
    let dossier = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("C:/dev/fragment-rs-s7"),
    );
    std::fs::create_dir_all(&dossier)?;
    match commande {
        "fixture" => fixture(&dossier),
        "rendre" => rendre(&dossier),
        "document" => par_document(&dossier),
        "comparer" => comparer(&dossier),
        "epreuves" => epreuves(&dossier),
        _ => Err("usage : spike_s7 fixture|rendre|document|comparer|epreuves [dossier]".into()),
    }
}

fn dossier_pdfium() -> PathBuf {
    std::env::var_os("PDFIUM_LIB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:/dev/fragment-rs-pdfium-bin/chromium-8125/bin"))
}

fn lier() -> Resultat<Pdfium> {
    let chemin = Pdfium::pdfium_platform_library_name_at_path(&dossier_pdfium());
    Ok(Pdfium::new(Pdfium::bind_to_library(&chemin)?))
}

fn suffixe(echelle: f64) -> String {
    format!("s{:03}", (echelle * 100.0).round() as u32)
}

fn taille(echelle: f64) -> (u32, u32) {
    (
        (PAGE.0 * echelle).round() as u32,
        (PAGE.1 * echelle).round() as u32,
    )
}

// ───────────── Fixture ─────────────

/// Image 8 × 8 RGB : dégradé rouge→bleu en x, damier vert, pour voir le lissage d'image.
fn image_hex() -> String {
    let mut s = String::new();
    for y in 0..8u32 {
        for x in 0..8u32 {
            let r = 255 - x * 32;
            let g = if (x + y) % 2 == 0 { 200 } else { 40 };
            let b = x * 32 + 31;
            let _ = write!(s, "{r:02x}{g:02x}{b:02x}");
        }
        s.push('\n');
    }
    s.push('>');
    s
}

/// Le flux de contenu « à la main » : texte Helvetica NON embarquée (pdf.js prend ses
/// standard_fonts, pdfium ses polices de repli), traits de 0,5 / 0,75 / 1 pt, cercle en Bézier,
/// aplats à coordonnées entières et fractionnaires, image agrandie. Coordonnées PDF (origine en bas).
fn flux_contenu() -> String {
    let k = 0.552_284_75 * 30.0; // poignées d'un cercle de rayon 30
    let (cx, cy) = (110.0, 70.0);
    let mut f = String::new();
    // Texte (zone « texte », haut de page).
    f += "BT /F1 12 Tf 0 g 20 272 Td (Helvetica 12 pt : le vif zephyr jubile.) Tj ET\n";
    f += "BT /F1 9 Tf 0.2 0.2 0.6 rg 20 258 Td (Helvetica 9 pt, en couleur : 0123456789) Tj ET\n";
    // Traits (zone « traits »).
    f += "0 0 0 RG 0.5 w 30 170 m 186 170 l S\n";
    f += "1 w 30 162 m 186 162 l S\n";
    f += "0.5 w 40.25 30 m 40.25 150 l S\n";
    f += "1 w 50 30 m 50 150 l S\n";
    f += "0.75 w 60 30 m 186 150 l S\n";
    f += &format!(
        "0.2 0.4 0.8 RG 1 w {x0} {cy} m {x0} {a} {b} {y1} {cx} {y1} c {c} {y1} {x1} {a} {x1} {cy} c \
         {x1} {d} {c} {y0} {cx} {y0} c {b} {y0} {x0} {d} {x0} {cy} c S\n",
        x0 = cx - 30.0,
        x1 = cx + 30.0,
        y0 = cy - 30.0,
        y1 = cy + 30.0,
        a = cy + k,
        d = cy - k,
        b = cx - k,
        c = cx + k,
    );
    // Aplats (zone « aplats »).
    f += "0.9 0.3 0.2 rg 210 140 80 40 re f\n";
    f += "0.2 0.7 0.3 rg 210.3 90.6 79.5 40.2 re f\n";
    f += "0.5 g 210 20 80 60 re f\n";
    // Image (zone « image ») : 8 × 8 px peints sur 80 × 80 pt.
    f += "q 80 0 0 80 304 96 cm /Im1 Do Q\n";
    f
}

fn pdf_base() -> Vec<u8> {
    let flux = flux_contenu();
    let image = image_hex();
    let objets = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> >> >>",
            PAGE.0, PAGE.1
        ),
        format!("<< /Length {} >>\nstream\n{flux}endstream", flux.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_string(),
        format!(
            "<< /Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length {} >>\nstream\n{image}\nendstream",
            image.len() + 1
        ),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut decalages = Vec::new();
    for (k, corps) in objets.iter().enumerate() {
        decalages.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{corps}\nendobj\n", k + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objets.len() + 1).as_bytes(),
    );
    for d in decalages {
        pdf.extend_from_slice(format!("{d:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objets.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

/// La fixture : le PDF « à la main », plus deux lignes en DM Sans EMBARQUÉE ajoutées par pdfium.
fn fixture(dossier: &Path) -> Resultat<()> {
    let pdfium = lier()?;
    let mut doc = pdfium.load_pdf_from_byte_vec(pdf_base(), None)?;
    let police = std::fs::read(POLICE_DM_SANS)?;
    let jeton = doc.fonts_mut().load_true_type_from_bytes(&police, false)?;
    {
        let mut page = doc.pages().get(0)?;
        page.objects_mut().create_text_object(
            PdfPoints::new(20.0),
            PdfPoints::new(236.0),
            "DM Sans 11 pt embarquée : Fragment, pages et glyphes.",
            jeton,
            PdfPoints::new(11.0),
        )?;
        page.objects_mut().create_text_object(
            PdfPoints::new(20.0),
            PdfPoints::new(206.0),
            "DM Sans 20 pt Hamburgefonstiv",
            jeton,
            PdfPoints::new(20.0),
        )?;
    }
    let octets = doc.save_to_bytes()?;
    let chemin = dossier.join("fixture.pdf");
    std::fs::write(&chemin, octets)?;
    println!("{}", chemin.display());
    Ok(())
}

// ───────────── Rendu pdfium ─────────────

/// Les jeux d'options essayés ; `defaut` est la configuration de `Document::rendre_region`.
/// ★ Le texte LCD de pdfium n'agit que sur un bitmap SANS alpha (BGRx) : en BGRA, `lcd` = `defaut`.
fn jeux_options() -> Vec<(&'static str, PdfBitmapFormat, PdfRenderConfig)> {
    use PdfBitmapFormat::{BGRA, BGRx};
    let base = |f| PdfRenderConfig::new().set_format(f);
    vec![
        ("defaut", BGRA, base(BGRA)),
        (
            "sans-lissage-texte",
            BGRA,
            base(BGRA).set_text_smoothing(false),
        ),
        (
            "sans-lissage-chemins",
            BGRA,
            base(BGRA).set_path_smoothing(false),
        ),
        (
            "sans-lissage-image",
            BGRA,
            base(BGRA).set_image_smoothing(false),
        ),
        ("lcd", BGRA, base(BGRA).use_lcd_text_rendering(true)),
        (
            "sans-texte-natif",
            BGRA,
            base(BGRA).disable_native_text_rendering(true),
        ),
        ("gris", BGRA, base(BGRA).use_grayscale_rendering(true)),
        ("impression", BGRA, base(BGRA).use_print_quality(true)),
        ("demi-teinte", BGRA, base(BGRA).force_half_tone(true)),
        ("bgrx", BGRx, base(BGRx)),
        ("lcd-bgrx", BGRx, base(BGRx).use_lcd_text_rendering(true)),
    ]
}

fn rendre(dossier: &Path) -> Resultat<()> {
    let pdfium = lier()?;
    let doc = pdfium.load_pdf_from_file(&dossier.join("fixture.pdf"), None)?;
    let page = doc.pages().get(0)?;
    for echelle in ECHELLES {
        let (l, h) = taille(echelle);
        for (nom, format, config) in jeux_options() {
            let mut tampon = vec![255u8; (l * h * 4) as usize];
            {
                let mut bitmap = PdfBitmap::from_bytes(l as i32, h as i32, format, &mut tampon)?;
                let config = config
                    .set_fixed_size(l as i32, h as i32)
                    .clear_before_rendering(false);
                page.render_into_bitmap_with_config(&mut bitmap, &config)?;
            }
            // FPDF_REVERSE_BYTE_ORDER (défaut) : déjà RGBA.
            for px in tampon.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
            std::fs::write(
                dossier.join(format!("pdfium-{nom}-{}.rgba", suffixe(echelle))),
                tampon,
            )?;
        }
    }
    println!("rendus pdfium écrits dans {}", dossier.display());
    Ok(())
}

fn par_document(dossier: &Path) -> Resultat<()> {
    initialiser(Some(&dossier_pdfium()))?;
    let doc = Document::ouvrir(std::fs::read(dossier.join("fixture.pdf"))?)?;
    for echelle in ECHELLES {
        let r = doc.rendre_page(1, echelle)?;
        if (r.largeur, r.hauteur) != taille(echelle) {
            return Err(format!("taille inattendue à {echelle}").into());
        }
        std::fs::write(
            dossier.join(format!("pdfium-document-{}.rgba", suffixe(echelle))),
            r.octets,
        )?;
    }
    println!("rendus Document écrits dans {}", dossier.display());
    Ok(())
}

// ───────────── Comparaison ─────────────

#[derive(Default, Clone, Copy)]
struct Ecart {
    pixels: u64,
    differents: u64,
    /// Plus grand écart d'un canal (0..=255).
    max: u8,
    /// Somme des écarts absolus par canal, pour la moyenne sur les pixels différents.
    somme: u64,
}

impl Ecart {
    fn moyenne(&self) -> f64 {
        if self.differents == 0 {
            0.0
        } else {
            self.somme as f64 / (self.differents * 3) as f64
        }
    }
    fn ligne(&self) -> String {
        format!(
            "{:>6} / {:>6} ({:>5.1} %)  max {:>3}  moy {:>5.1}",
            self.differents,
            self.pixels,
            100.0 * self.differents as f64 / self.pixels.max(1) as f64,
            self.max,
            self.moyenne()
        )
    }
}

/// La zone en pixels à cette échelle : bords arrondis vers l'EXTÉRIEUR (l'anticrénelage déborde).
fn zone_px(z: (f64, f64, f64, f64), echelle: f64, (l, h): (u32, u32)) -> (u32, u32, u32, u32) {
    let x0 = (z.0 * echelle).floor().max(0.0) as u32;
    let y0 = (z.1 * echelle).floor().max(0.0) as u32;
    let x1 = ((z.2 * echelle).ceil() as u32).min(l);
    let y1 = ((z.3 * echelle).ceil() as u32).min(h);
    (x0, y0, x1, y1)
}

fn ecart(a: &[u8], b: &[u8], l: u32, zone: (u32, u32, u32, u32), gamma: Option<f64>) -> Ecart {
    let mut e = Ecart::default();
    for y in zone.1..zone.3 {
        for x in zone.0..zone.2 {
            let i = ((y * l + x) * 4) as usize;
            e.pixels += 1;
            let mut d_max = 0u8;
            let mut d_somme = 0u64;
            for c in 0..3 {
                let mut v = b[i + c];
                if let Some(g) = gamma {
                    // Gamma « émulé » : la couverture (1 − v) est élevée à la puissance g.
                    let couv = 1.0 - f64::from(v) / 255.0;
                    v = (255.0 * (1.0 - couv.powf(g))).round() as u8;
                }
                let d = a[i + c].abs_diff(v);
                d_max = d_max.max(d);
                d_somme += u64::from(d);
            }
            if d_max > 0 {
                e.differents += 1;
                e.max = e.max.max(d_max);
                e.somme += d_somme;
            }
        }
    }
    e
}

/// Carte des différences en PPM (gris = identique atténué, rouge = écart), pour l'œil.
fn carte(a: &[u8], b: &[u8], (l, h): (u32, u32), chemin: &Path) -> Resultat<()> {
    let mut sortie = format!("P6\n{l} {h}\n255\n").into_bytes();
    for (pa, pb) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        let d = (0..3).map(|c| pa[c].abs_diff(pb[c])).max().unwrap_or(0);
        if d == 0 {
            let g = 191 + pa[1] / 4;
            sortie.extend_from_slice(&[g, g, g]);
        } else {
            sortie.extend_from_slice(&[255, 255 - d.saturating_mul(4), 0]);
        }
    }
    std::fs::write(chemin, sortie)?;
    Ok(())
}

fn lire(chemin: &Path, (l, h): (u32, u32)) -> Resultat<Vec<u8>> {
    let o = std::fs::read(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
    if o.len() != (l * h * 4) as usize {
        return Err(format!(
            "{} : {} octets, attendu {}",
            chemin.display(),
            o.len(),
            l * h * 4
        )
        .into());
    }
    Ok(o)
}

fn comparer(dossier: &Path) -> Resultat<()> {
    let mut noms: Vec<&str> = jeux_options().iter().map(|(n, _, _)| *n).collect();
    noms.push("document");
    for echelle in ECHELLES {
        let t = taille(echelle);
        let s = suffixe(echelle);
        let reference = lire(&dossier.join(format!("pdfjs-{s}.rgba")), t)?;
        println!("\n=== échelle {echelle} ({} × {} px) ===", t.0, t.1);
        let page = (0, 0, t.0, t.1);
        for nom in &noms {
            let chemin = dossier.join(format!("pdfium-{nom}-{s}.rgba"));
            let Ok(candidat) = lire(&chemin, t) else {
                println!("{nom:<22} absent");
                continue;
            };
            println!(
                "{nom:<22} page    {}",
                ecart(&reference, &candidat, t.0, page, None).ligne()
            );
            for (z, g, hz, d, b) in ZONES {
                let zp = zone_px((g, hz, d, b), echelle, t);
                println!(
                    "{:<22} {z:<7} {}",
                    "",
                    ecart(&reference, &candidat, t.0, zp, None).ligne()
                );
            }
            if *nom == "defaut" {
                carte(
                    &reference,
                    &candidat,
                    t,
                    &dossier.join(format!("carte-{s}.ppm")),
                )?;
                let zt = ZONES[0];
                let zp = zone_px((zt.1, zt.2, zt.3, zt.4), echelle, t);
                for gamma in [0.8, 0.9, 1.1, 1.2, 1.45, 1.8] {
                    println!(
                        "{:<22} texte γ{gamma:<4} {}",
                        "",
                        ecart(&reference, &candidat, t.0, zp, Some(gamma)).ligne()
                    );
                }
            }
        }
    }
    Ok(())
}

// ───────────── Contre-épreuves ─────────────

/// Mesure de la catégorie proposée (ADR-014) : part des pixels de delta > 32 et delta moyen, sur TOUTE la zone.
fn mesure_categorie(a: &[u8], b: &[u8], l: u32, z: (u32, u32, u32, u32)) -> (f64, f64) {
    let (mut forts, mut somme, mut n) = (0u64, 0u64, 0u64);
    for y in z.1..z.3 {
        for x in z.0..z.2 {
            let i = ((y * l + x) * 4) as usize;
            let d = (0..3)
                .map(|c| a[i + c].abs_diff(b[i + c]))
                .max()
                .unwrap_or(0);
            forts += u64::from(d > 32);
            somme += u64::from(d);
            n += 1;
        }
    }
    let n = n.max(1) as f64;
    (100.0 * forts as f64 / n, somme as f64 / n)
}

fn decale(b: &[u8], (l, h): (u32, u32), dx: i64, dy: i64) -> Vec<u8> {
    let mut o = vec![255u8; b.len()];
    for y in 0..i64::from(h) {
        for x in 0..i64::from(l) {
            let (sx, sy) = (x - dx, y - dy);
            if (0..i64::from(l)).contains(&sx) && (0..i64::from(h)).contains(&sy) {
                let (i, j) = (
                    ((y * i64::from(l) + x) * 4) as usize,
                    ((sy * i64::from(l) + sx) * 4) as usize,
                );
                o[i..i + 4].copy_from_slice(&b[j..j + 4]);
            }
        }
    }
    o
}

/// Efface (blanc) le « H » de « Helvetica 12 » : 20..28 × 19..29 pt.
fn sans_h(b: &[u8], (l, _): (u32, u32), echelle: f64) -> Vec<u8> {
    let mut o = b.to_vec();
    let px = |v: f64| (v * echelle) as u32;
    for y in px(19.0)..px(29.0).max(px(19.0) + 1) {
        for x in px(20.0)..px(28.0).max(px(20.0) + 1) {
            let i = ((y * l + x) * 4) as usize;
            o[i..i + 3].copy_from_slice(&[255, 255, 255]);
        }
    }
    o
}

fn epreuves(dossier: &Path) -> Resultat<()> {
    println!("zone : % des pixels de delta > 32, delta moyen (sur toute la zone)");
    for echelle in ECHELLES {
        let t = taille(echelle);
        let s = suffixe(echelle);
        let reference = lire(&dossier.join(format!("pdfjs-{s}.rgba")), t)?;
        let defaut = lire(&dossier.join(format!("pdfium-defaut-{s}.rgba")), t)?;
        let lcd = lire(&dossier.join(format!("pdfium-lcd-bgrx-{s}.rgba")), t)?;
        println!(
            "
=== échelle {echelle} ==="
        );
        let mut cas: Vec<(&str, Vec<u8>)> = vec![
            ("defaut", defaut),
            ("lcd-bgrx", lcd.clone()),
            ("lcd-bgrx +1 px x", decale(&lcd, t, 1, 0)),
            ("lcd-bgrx +1 px y", decale(&lcd, t, 0, 1)),
            ("lcd-bgrx sans H", sans_h(&lcd, t, echelle)),
        ];
        // Le vrai `Document::rendre_page` (étape `document`), s'il a été rendu : c'est lui que
        // jugent les garde-fous d'ADR-014 (fr-t4.2).
        if let Ok(doc) = lire(&dossier.join(format!("pdfium-document-{s}.rgba")), t) {
            cas.push(("document", doc));
        }
        for (nom, candidat) in &cas {
            let mut ligne = format!("{nom:<18}");
            let (p, m) = mesure_categorie(&reference, candidat, t.0, (0, 0, t.0, t.1));
            let _ = write!(ligne, "  page {p:>5.2} % {m:>5.2}");
            for (z, g, hz, d, b) in ZONES {
                let (p, m) = mesure_categorie(
                    &reference,
                    candidat,
                    t.0,
                    zone_px((g, hz, d, b), echelle, t),
                );
                let _ = write!(ligne, "  {z} {p:>5.2} % {m:>5.2}");
            }
            println!("{ligne}");
        }
    }
    Ok(())
}
