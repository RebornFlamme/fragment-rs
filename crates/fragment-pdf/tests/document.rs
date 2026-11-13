//! Tests du `Document` (pdfium). Les PDF sont GÉNÉRÉS ici, octet par octet : aucun fichier de Fragment.
//! Prérequis : le binaire pdfium épinglé (`scripts/pdfium.ps1` / `scripts/pdfium.sh`, dossier dans
//! `PDFIUM_LIB_DIR`).

#![allow(clippy::expect_used, clippy::unwrap_used)] // fichier de tests

use std::collections::BTreeMap;
use std::path::Path;

use fragment_pdf::document::{CibleLien, Document, ErreurPdf, RegionPx, initialiser};

// ───────────── Générateur de PDF ─────────────

enum Cible {
    Uri(&'static str),
    /// Index (0-based) de la page visée et `y` PDF.
    Page(usize, f64),
}

struct Item {
    titre: &'static str,
    page: usize,
    y: f64,
    enfants: Vec<Item>,
}

struct SpecPage {
    largeur: f64,
    hauteur: f64,
    /// (x, y, corps) en coordonnées PDF.
    textes: Vec<(f64, f64, &'static str)>,
    /// Rectangles pleins noirs (x, y, l, h).
    pleins: Vec<(f64, f64, f64, f64)>,
    /// Rectangles de lien [bas, gauche, haut, droite] → cible.
    liens: Vec<([f64; 4], Cible)>,
}

impl SpecPage {
    fn nue(largeur: f64, hauteur: f64) -> Self {
        Self {
            largeur,
            hauteur,
            textes: vec![],
            pleins: vec![],
            liens: vec![],
        }
    }
}

fn emettre_signets(
    items: &[Item],
    parent: u32,
    prochain: &mut u32,
    pages_ids: &[u32],
    sortie: &mut BTreeMap<u32, String>,
) -> Option<(u32, u32)> {
    if items.is_empty() {
        return None;
    }
    let ids: Vec<u32> = items
        .iter()
        .map(|_| {
            let i = *prochain;
            *prochain += 1;
            i
        })
        .collect();
    for (k, item) in items.iter().enumerate() {
        let enfants = emettre_signets(&item.enfants, ids[k], prochain, pages_ids, sortie);
        let mut d = format!(
            "<< /Title ({}) /Parent {parent} 0 R /Dest [{} 0 R /XYZ 0 {} 0]",
            item.titre, pages_ids[item.page], item.y
        );
        if k > 0 {
            d += &format!(" /Prev {} 0 R", ids[k - 1]);
        }
        if k + 1 < ids.len() {
            d += &format!(" /Next {} 0 R", ids[k + 1]);
        }
        if let Some((premier, dernier)) = enfants {
            d += &format!(
                " /First {premier} 0 R /Last {dernier} 0 R /Count {}",
                item.enfants.len()
            );
        }
        d += " >>";
        sortie.insert(ids[k], d);
    }
    Some((ids[0], ids[ids.len() - 1]))
}

fn fabriquer_pdf(pages: &[SpecPage], signets: &[Item]) -> Vec<u8> {
    let n = pages.len() as u32;
    let page_id = |i: u32| 4 + 2 * i;
    let pages_ids: Vec<u32> = (0..n).map(page_id).collect();
    let racine_signets = 4 + 2 * n;
    let mut objets: BTreeMap<u32, String> = BTreeMap::new();

    let sortie_outline = if signets.is_empty() {
        String::new()
    } else {
        format!(" /Outlines {racine_signets} 0 R")
    };
    objets.insert(
        1,
        format!("<< /Type /Catalog /Pages 2 0 R{sortie_outline} >>"),
    );
    let kids: Vec<String> = pages_ids.iter().map(|i| format!("{i} 0 R")).collect();
    objets.insert(
        2,
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")),
    );
    objets.insert(
        3,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    );
    for (i, p) in pages.iter().enumerate() {
        let id = page_id(i as u32);
        let mut flux = String::new();
        for (x, y, l, h) in &p.pleins {
            flux += &format!("0 0 0 rg {x} {y} {l} {h} re f\n");
        }
        for (x, y, corps) in &p.textes {
            flux += &format!("BT /F1 12 Tf 0 g {x} {y} Td ({corps}) Tj ET\n");
        }
        let annots: Vec<String> = p
            .liens
            .iter()
            .map(|(r, c)| {
                let action = match c {
                    Cible::Uri(u) => format!("/A << /S /URI /URI ({u}) >>"),
                    Cible::Page(k, y) => {
                        format!("/Dest [{} 0 R /XYZ 0 {y} 0]", pages_ids[*k])
                    }
                };
                format!(
                    "<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] {action} >>",
                    r[1], r[0], r[3], r[2]
                )
            })
            .collect();
        objets.insert(
            id,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {} 0 R \
                 /Resources << /Font << /F1 3 0 R >> >> /Annots [{}] >>",
                p.largeur,
                p.hauteur,
                id + 1,
                annots.join(" ")
            ),
        );
        objets.insert(
            id + 1,
            format!("<< /Length {} >>\nstream\n{flux}endstream", flux.len()),
        );
    }
    if !signets.is_empty() {
        let mut prochain = racine_signets + 1;
        let (premier, dernier) = emettre_signets(
            signets,
            racine_signets,
            &mut prochain,
            &pages_ids,
            &mut objets,
        )
        .unwrap_or((0, 0));
        objets.insert(
            racine_signets,
            format!(
                "<< /Type /Outlines /First {premier} 0 R /Last {dernier} 0 R /Count {} >>",
                signets.len()
            ),
        );
    }

    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = BTreeMap::new();
    for (id, corps) in &objets {
        offsets.insert(*id, pdf.len());
        pdf.extend_from_slice(format!("{id} 0 obj\n{corps}\nendobj\n").as_bytes());
    }
    let plus_grand = *objets.keys().last().unwrap_or(&0);
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", plus_grand + 1).as_bytes());
    for id in 1..=plus_grand {
        match offsets.get(&id) {
            Some(o) => pdf.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes()),
            None => pdf.extend_from_slice(b"0000000000 65535 f \n"),
        }
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            plus_grand + 1
        )
        .as_bytes(),
    );
    pdf
}

/// ★ Une seule liaison pdfium par processus : les tests tournent en parallèle, et deux
///   `initialiser` simultanés font échouer le second (`PdfiumLibraryBindingsAlreadyInitialized`).
///   Le `Once` sérialise ; `initialiser(None)` lit `PDFIUM_LIB_DIR` lui-même.
fn charger_pdfium() {
    static UNE_FOIS: std::sync::Once = std::sync::Once::new();
    UNE_FOIS.call_once(|| {
        let defaut = Path::new("C:/dev/fragment-rs-pdfium-bin/chromium-8125/bin");
        let dossier = std::env::var_os("PDFIUM_LIB_DIR")
            .is_none()
            .then_some(defaut);
        initialiser(dossier).expect("pdfium : lancer scripts/pdfium.ps1");
    });
}

/// Charge pdfium : `PDFIUM_LIB_DIR`, sinon le dossier par défaut de `scripts/pdfium.ps1`.
fn ouvrir(pages: &[SpecPage], signets: &[Item]) -> Document {
    charger_pdfium();
    Document::ouvrir(fabriquer_pdf(pages, signets)).expect("PDF généré lisible")
}

/// Une page 200 × 100 pt avec un carré noir (20,20)–(60,60) et un mot.
fn page_temoin() -> SpecPage {
    let mut p = SpecPage::nue(200.0, 100.0);
    p.pleins.push((20.0, 20.0, 40.0, 40.0));
    p.textes.push((100.0, 70.0, "Hello"));
    p
}

// ───────────── Tests ─────────────

#[test]
fn pages_et_tailles() {
    let doc = ouvrir(
        &[SpecPage::nue(200.0, 100.0), SpecPage::nue(300.0, 400.0)],
        &[],
    );
    assert_eq!(doc.nombre_de_pages(), 2);
    let t1 = doc.taille_page(1).unwrap();
    let t2 = doc.taille_page(2).unwrap();
    assert!((t1.largeur - 200.0).abs() < 1e-3 && (t1.hauteur - 100.0).abs() < 1e-3);
    assert!((t2.largeur - 300.0).abs() < 1e-3 && (t2.hauteur - 400.0).abs() < 1e-3);
}

#[test]
fn page_hors_limites_et_echelle_invalide() {
    let doc = ouvrir(&[SpecPage::nue(200.0, 100.0)], &[]);
    assert!(matches!(
        doc.taille_page(0),
        Err(ErreurPdf::PageHorsLimites { page: 0, total: 1 })
    ));
    assert!(matches!(
        doc.taille_page(2),
        Err(ErreurPdf::PageHorsLimites { page: 2, total: 1 })
    ));
    assert!(matches!(
        doc.rendre_page(1, 0.0),
        Err(ErreurPdf::Echelle(_))
    ));
    assert!(matches!(
        doc.rendre_page(1, f64::NAN),
        Err(ErreurPdf::Echelle(_))
    ));
}

#[test]
fn octets_illisibles_donnent_une_erreur() {
    ouvrir(&[SpecPage::nue(10.0, 10.0)], &[]);
    assert!(matches!(
        Document::ouvrir(b"ceci n'est pas un pdf".to_vec()),
        Err(ErreurPdf::Ouverture(_))
    ));
}

#[test]
fn rendu_a_la_taille_physique_exacte() {
    let doc = ouvrir(&[page_temoin()], &[]);
    for (echelle, l, h) in [
        (1.0, 200, 100),
        (1.5, 300, 150),
        (2.0, 400, 200),
        (0.5, 100, 50),
    ] {
        let img = doc.rendre_page(1, echelle).unwrap();
        assert_eq!((img.largeur, img.hauteur), (l, h), "échelle {echelle}");
        assert_eq!(img.octets.len(), (l * h * 4) as usize);
    }
    // Taille non entière : arrondie, jamais tronquée.
    let doc = ouvrir(&[SpecPage::nue(200.4, 100.6)], &[]);
    let img = doc.rendre_page(1, 1.0).unwrap();
    assert_eq!((img.largeur, img.hauteur), (200, 101));
    assert_eq!(doc.taille_raster(1, 1.0).unwrap(), (200, 101));
}

#[test]
fn rendu_pixel_temoin() {
    let doc = ouvrir(&[page_temoin()], &[]);
    let img = doc.rendre_page(1, 2.0).unwrap();
    // Carré noir : x 40..120 px, y (100 − 60) × 2 = 80 .. 160 px (origine en haut).
    assert_eq!(img.pixel(80, 120), [0, 0, 0, 255]);
    assert_eq!(img.pixel(300, 20), [255, 255, 255, 255]);
    assert_eq!(img.pixel(0, 0), [255, 255, 255, 255]);
    assert_eq!(img.pixel(39, 120), [255, 255, 255, 255]);
    assert_eq!(img.pixel(120, 120), [255, 255, 255, 255]);
    assert_eq!(img.pixel(80, 79), [255, 255, 255, 255]);
    assert_eq!(img.pixel(80, 160), [255, 255, 255, 255]);
}

#[test]
fn region_egale_au_recadrage_du_rendu_entier() {
    let doc = ouvrir(&[page_temoin()], &[]);
    let entier = doc.rendre_page(1, 2.0).unwrap();
    let region = RegionPx {
        x: 30,
        y: 70,
        largeur: 100,
        hauteur: 60,
    };
    let morceau = doc.rendre_region(1, 2.0, region).unwrap();
    assert_eq!((morceau.largeur, morceau.hauteur), (100, 60));
    for y in 0..60 {
        for x in 0..100 {
            assert_eq!(
                morceau.pixel(x, y),
                entier.pixel(30 + x, 70 + y),
                "pixel ({x}, {y})"
            );
        }
    }
}

#[test]
fn region_qui_deborde_de_la_page_reste_blanche() {
    let doc = ouvrir(&[page_temoin()], &[]);
    let region = RegionPx {
        x: 380,
        y: 180,
        largeur: 50,
        hauteur: 50,
    };
    let img = doc.rendre_region(1, 2.0, region).unwrap();
    assert_eq!((img.largeur, img.hauteur), (50, 50));
    assert_eq!(img.pixel(49, 49), [255, 255, 255, 255]);
    assert_eq!(img.pixel(0, 0), [255, 255, 255, 255]);
    assert!(matches!(
        doc.rendre_region(
            1,
            2.0,
            RegionPx {
                x: 0,
                y: 0,
                largeur: 0,
                hauteur: 5
            }
        ),
        Err(ErreurPdf::Rendu(_))
    ));
}

#[test]
fn texte_de_page_et_caracteres_avec_boites() {
    let doc = ouvrir(&[page_temoin()], &[]);
    assert!(doc.texte_page(1).unwrap().contains("Hello"));

    let chars = doc.caracteres(1).unwrap();
    let mot: String = chars
        .iter()
        .filter(|c| !c.genere)
        .map(|c| c.caractere)
        .collect();
    assert_eq!(mot, "Hello");
    // Indices consécutifs depuis 0 ; boîtes dans la page, de gauche à droite.
    let reels: Vec<_> = chars.iter().filter(|c| !c.genere).collect();
    for (k, c) in reels.iter().enumerate() {
        assert_eq!(c.index as usize, k);
        assert!(c.boite.gauche >= 0.0 && c.boite.droite <= 200.0);
        assert!(c.boite.droite > c.boite.gauche);
        if k > 0 {
            assert!(c.boite.gauche >= reels[k - 1].boite.gauche);
        }
    }
    let h = reels[0].boite;
    // Ligne de base à y = 70 (PDF) → 30 depuis le haut ; départ à x = 100.
    assert!((h.gauche - 100.0).abs() < 1.5, "gauche {}", h.gauche);
    assert!(h.haut < 30.0 && 30.0 < h.bas, "boîte {h:?}");
    // « H » en Helvetica 12 pt : 0,722 em ≈ 8,7 pt de large.
    assert!(
        (h.droite - h.gauche - 8.66).abs() < 1.0,
        "largeur {}",
        h.droite - h.gauche
    );
}

#[test]
fn liens_externes_et_internes() {
    let mut p1 = SpecPage::nue(200.0, 100.0);
    p1.liens.push((
        [10.0, 10.0, 30.0, 60.0],
        Cible::Uri("https://example.org/x"),
    ));
    p1.liens
        .push(([50.0, 10.0, 70.0, 60.0], Cible::Page(1, 250.0)));
    let doc = ouvrir(&[p1, SpecPage::nue(300.0, 400.0)], &[]);

    let liens = doc.liens(1).unwrap();
    assert_eq!(liens.len(), 2);
    let externe = liens
        .iter()
        .find(|l| matches!(l.cible, CibleLien::Externe { .. }))
        .expect("lien externe");
    assert_eq!(
        externe.cible,
        CibleLien::Externe {
            uri: "https://example.org/x".into()
        }
    );
    // Rect PDF [bas 10, gauche 10, haut 30, droite 60] sur une page de 100 pt → haut 70, bas 90.
    assert!((externe.boite.gauche - 10.0).abs() < 1e-3);
    assert!((externe.boite.droite - 60.0).abs() < 1e-3);
    assert!((externe.boite.haut - 70.0).abs() < 1e-3);
    assert!((externe.boite.bas - 90.0).abs() < 1e-3);

    let interne = liens
        .iter()
        .find(|l| matches!(l.cible, CibleLien::Interne { .. }))
        .expect("lien interne");
    assert_eq!(
        interne.cible,
        CibleLien::Interne {
            page: 2,
            y: Some(250.0)
        }
    );
    assert!(doc.liens(2).unwrap().is_empty());
}

#[test]
fn signets_en_arbre_resolus_en_pages() {
    let signets = vec![
        Item {
            titre: "Chapitre 1",
            page: 0,
            y: 90.0,
            enfants: vec![
                Item {
                    titre: "Section 1.1",
                    page: 1,
                    y: 300.0,
                    enfants: vec![],
                },
                Item {
                    titre: "Section 1.2",
                    page: 2,
                    y: 10.0,
                    enfants: vec![],
                },
            ],
        },
        Item {
            titre: "Chapitre 2",
            page: 2,
            y: 50.0,
            enfants: vec![],
        },
    ];
    let pages = [
        SpecPage::nue(200.0, 100.0),
        SpecPage::nue(300.0, 400.0),
        SpecPage::nue(300.0, 400.0),
    ];
    let doc = ouvrir(&pages, &signets);
    let toc = doc.signets();
    assert_eq!(toc.len(), 2);
    assert_eq!(toc[0].titre, "Chapitre 1");
    assert_eq!(toc[0].page, Some(1));
    assert_eq!(toc[0].y, Some(90.0));
    assert_eq!(toc[0].enfants.len(), 2);
    assert_eq!(toc[0].enfants[0].titre, "Section 1.1");
    assert_eq!(toc[0].enfants[0].page, Some(2));
    assert_eq!(toc[0].enfants[1].page, Some(3));
    assert_eq!(toc[1].titre, "Chapitre 2");
    assert_eq!(toc[1].page, Some(3));
    assert!(toc[1].enfants.is_empty());

    // Branché sur le modèle de phase 1 : la page 2 révèle « Section 1.1 ».
    let chemin = fragment_pdf::pdf_outline::entree_pour_page(&toc, 2).unwrap();
    assert_eq!(chemin.last().unwrap().titre, "Section 1.1");
}

#[test]
fn document_sans_signets() {
    let doc = ouvrir(&[SpecPage::nue(200.0, 100.0)], &[]);
    assert!(doc.signets().is_empty());
}

// ───────────── Texte LCD (fr-t4.2, ADR-014) ─────────────

/// Une page de texte noir seul : en niveaux de gris, chaque pixel aurait r = g = b.
fn page_de_texte() -> SpecPage {
    let mut p = SpecPage::nue(200.0, 60.0);
    p.textes.push((10.0, 30.0, "Hamburgefonstiv 0123"));
    p
}

/// ★ Le texte est rastérisé en LCD (sous-pixel, comme le canvas de pdf.js chez Fragment) : du texte
///   NOIR laisse des franges colorées (r ≠ b). Le LCD de pdfium n'agit que sur un bitmap sans alpha.
#[test]
fn texte_noir_rendu_en_lcd_avec_franges() {
    let doc = ouvrir(&[page_de_texte()], &[]);
    for echelle in [0.5, 1.0, 2.0] {
        let img = doc.rendre_page(1, echelle).unwrap();
        let franges = img
            .octets
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0].abs_diff(p[2]) > 16)
            .count();
        assert!(franges > 0, "aucune frange LCD à {echelle}");
        // Fond toujours blanc opaque, alpha opaque partout.
        assert_eq!(img.pixel(0, 0), [255, 255, 255, 255]);
        assert!(img.octets.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    }
}

/// Une région rendue en LCD est le recadrage exact du rendu entier, texte compris.
#[test]
fn region_de_texte_lcd_egale_au_recadrage() {
    let doc = ouvrir(&[page_de_texte()], &[]);
    let entier = doc.rendre_page(1, 2.0).unwrap();
    let region = RegionPx {
        x: 17,
        y: 41,
        largeur: 150,
        hauteur: 40,
    };
    let morceau = doc.rendre_region(1, 2.0, region).unwrap();
    for y in 0..40 {
        for x in 0..150 {
            assert_eq!(
                morceau.pixel(x, y),
                entier.pixel(17 + x, 41 + y),
                "pixel ({x}, {y})"
            );
        }
    }
}
