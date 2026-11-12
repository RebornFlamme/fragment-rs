//! Spike S7 : tests du raster pdfium (ordre des canaux). Les PDF sont GÉNÉRÉS ici, aucun fichier de
//! Fragment. Prérequis : le binaire pdfium épinglé (`PDFIUM_LIB_DIR` ou le dossier par défaut).
//!
//! ★ Les tests de `document.rs` ne rendent que du noir et du blanc, où l'inversion rouge/bleu est
//!   invisible : il faut une page COLORÉE pour vérifier que `Raster` est bien en RGBA.

#![allow(clippy::expect_used, clippy::unwrap_used)] // fichier de tests

use std::path::Path;

use fragment_pdf::document::{Document, RegionPx, initialiser};

/// Un PDF d'une page `l × h` pt dont le flux de contenu est `flux` (xref aux bons décalages).
fn pdf_une_page(l: f64, h: f64, flux: &str) -> Vec<u8> {
    let objets = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {l} {h}] /Contents 4 0 R >>"),
        format!("<< /Length {} >>\nstream\n{flux}endstream", flux.len()),
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

fn ouvrir(octets: Vec<u8>) -> Document {
    if std::env::var_os("PDFIUM_LIB_DIR").is_none() {
        let defaut = Path::new("C:/dev/fragment-rs-pdfium-bin/chromium-8125/bin");
        initialiser(Some(defaut)).expect("pdfium : lancer scripts/pdfium.ps1");
    }
    Document::ouvrir(octets).expect("PDF généré lisible")
}

/// Page 300 × 100 pt : trois bandes pleines rouge, vert, bleu purs, de 100 pt chacune.
fn page_rvb() -> Document {
    ouvrir(pdf_une_page(
        300.0,
        100.0,
        "1 0 0 rg 0 0 100 100 re f\n0 1 0 rg 100 0 100 100 re f\n0 0 1 rg 200 0 100 100 re f\n",
    ))
}

#[test]
fn rendu_page_coloree_en_rgba() {
    let doc = page_rvb();
    for echelle in [0.5, 1.0, 2.0] {
        let img = doc.rendre_page(1, echelle).unwrap();
        let y = img.hauteur / 2;
        let tiers = img.largeur / 3;
        assert_eq!(
            img.pixel(tiers / 2, y),
            [255, 0, 0, 255],
            "rouge à {echelle}"
        );
        assert_eq!(
            img.pixel(tiers + tiers / 2, y),
            [0, 255, 0, 255],
            "vert à {echelle}"
        );
        assert_eq!(
            img.pixel(2 * tiers + tiers / 2, y),
            [0, 0, 255, 255],
            "bleu à {echelle}"
        );
    }
}

#[test]
fn region_coloree_en_rgba() {
    let doc = page_rvb();
    // Région à cheval sur le rouge et le vert, à l'échelle 1 (frontière à x = 100).
    let region = RegionPx {
        x: 90,
        y: 40,
        largeur: 20,
        hauteur: 20,
    };
    let img = doc.rendre_region(1, 1.0, region).unwrap();
    assert_eq!(img.pixel(2, 10), [255, 0, 0, 255]);
    assert_eq!(img.pixel(17, 10), [0, 255, 0, 255]);
    // Débordement à droite : bleu puis blanc.
    let bord = RegionPx {
        x: 290,
        y: 40,
        largeur: 20,
        hauteur: 20,
    };
    let img = doc.rendre_region(1, 1.0, bord).unwrap();
    assert_eq!(img.pixel(2, 10), [0, 0, 255, 255]);
    assert_eq!(img.pixel(17, 10), [255, 255, 255, 255]);
}

/// Taille du canvas de Fragment à densité 1 : `round(pt × échelle)`, les trois échelles de S7.
#[test]
fn taille_raster_egale_au_canvas_de_pdfjs() {
    let doc = ouvrir(pdf_une_page(400.0, 300.0, ""));
    for (echelle, attendu) in [(0.5, (200, 150)), (1.0, (400, 300)), (2.0, (800, 600))] {
        assert_eq!(doc.taille_raster(1, echelle).unwrap(), attendu);
    }
    // Arrondi au plus proche sur une page fractionnaire (595,28 × 841,89 pt, A4).
    let a4 = ouvrir(pdf_une_page(595.28, 841.89, ""));
    assert_eq!(a4.taille_raster(1, 1.25).unwrap(), (744, 1052));
}
