//! Le moteur PDF : un `Document` au-dessus de pdfium (pages, rendu, caractères, liens, signets).
//!
//! Port de app/src/ui/views/pdf/{PdfView.ts, pdfOutline.ts} : côté TS, pdf.js fournissait pages, couche
//! texte, liens et outline ; ici pdfium-render les fournit, derrière un type qui ne laisse échapper
//! aucun type de pdfium (le reste du code ne dépend donc pas de la bibliothèque).
//!
//! Conventions :
//! - les pages sont numérotées à partir de 1 (comme `EntreeToc::page`) ;
//! - les tailles et boîtes sont en POINTS (1/72 de pouce), origine en HAUT à gauche, `y` vers le bas ;
//! - seul le `y` des destinations garde l'origine PDF (en BAS), comme `pdf_outline::EntreeToc::y`.

mod moteur;

use pdfium_render::prelude::*;

use crate::pdf_outline::EntreeToc;

pub use moteur::{VARIABLE_DOSSIER, initialiser};

/// Erreurs du moteur PDF.
#[derive(Debug, thiserror::Error)]
pub enum ErreurPdf {
    #[error("pdfium introuvable ({0}) ; lancer scripts/pdfium.ps1 ou définir PDFIUM_LIB_DIR")]
    Chargement(String),
    #[error("pdfium n'est pas chargé")]
    MoteurAbsent,
    #[error("document illisible : {0}")]
    Ouverture(String),
    #[error("page {page} hors limites (1..={total})")]
    PageHorsLimites { page: u32, total: u32 },
    #[error("échelle invalide : {0}")]
    Echelle(f64),
    #[error("rendu impossible : {0}")]
    Rendu(String),
}

/// Taille d'une page, en points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaillePage {
    pub largeur: f64,
    pub hauteur: f64,
}

/// Rectangle en points, origine en haut à gauche.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Boite {
    pub gauche: f64,
    pub haut: f64,
    pub droite: f64,
    pub bas: f64,
}

/// Rectangle en pixels du raster d'une page à une échelle donnée (origine en haut à gauche).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionPx {
    pub x: u32,
    pub y: u32,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Image RGBA 8 bits, lignes jointives (`octets.len() == largeur × hauteur × 4`), alpha opaque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRgba {
    pub largeur: u32,
    pub hauteur: u32,
    pub octets: Vec<u8>,
}

impl ImageRgba {
    /// Le pixel (x, y) en `[r, g, b, a]`.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.largeur + x) * 4) as usize;
        [
            self.octets[i],
            self.octets[i + 1],
            self.octets[i + 2],
            self.octets[i + 3],
        ]
    }
}

/// Un caractère de la couche texte.
#[derive(Debug, Clone, PartialEq)]
pub struct Caractere {
    /// Indice du caractère dans la page (celui de pdfium : c'est la position dans `texte_page`,
    /// comptée en caractères).
    pub index: u32,
    pub caractere: char,
    /// Boîte « large » (hauteur de ligne) : celle qu'on surligne pour une sélection.
    pub boite: Boite,
    /// Inséré par pdfium (espace, fin de ligne) : sans glyphe dans la page.
    pub genere: bool,
}

/// Où mène un lien.
#[derive(Debug, Clone, PartialEq)]
pub enum CibleLien {
    /// Une page du document ; `y` = haut visé en coordonnées PDF (origine en BAS), `None` = haut de page.
    Interne {
        page: u32,
        y: Option<f64>,
    },
    Externe {
        uri: String,
    },
}

/// Un rectangle cliquable de la page et sa cible.
#[derive(Debug, Clone, PartialEq)]
pub struct Lien {
    pub boite: Boite,
    pub cible: CibleLien,
}

/// Un PDF ouvert.
pub struct Document {
    pdf: PdfDocument<'static>,
}

impl Document {
    /// Ouvre un PDF à partir de ses octets (charge pdfium au besoin, voir [`initialiser`]).
    pub fn ouvrir(octets: Vec<u8>) -> Result<Self, ErreurPdf> {
        let pdf = moteur::moteur()?
            .load_pdf_from_byte_vec(octets, None)
            .map_err(|e| ErreurPdf::Ouverture(e.to_string()))?;
        Ok(Self { pdf })
    }

    pub fn nombre_de_pages(&self) -> u32 {
        self.pdf.pages().len().max(0) as u32
    }

    fn page(&self, numero: u32) -> Result<PdfPage<'_>, ErreurPdf> {
        let total = self.nombre_de_pages();
        if numero == 0 || numero > total {
            return Err(ErreurPdf::PageHorsLimites {
                page: numero,
                total,
            });
        }
        self.pdf
            .pages()
            .get((numero - 1) as PdfPageIndex)
            .map_err(|e| ErreurPdf::Rendu(e.to_string()))
    }

    /// Taille de la page en points (rotation de la page appliquée).
    pub fn taille_page(&self, page: u32) -> Result<TaillePage, ErreurPdf> {
        let p = self.page(page)?;
        Ok(TaillePage {
            largeur: f64::from(p.width().value),
            hauteur: f64::from(p.height().value),
        })
    }

    /// Taille physique du raster entier d'une page : `round(points × échelle)`, au moins 1 pixel.
    pub fn taille_raster(&self, page: u32, echelle: f64) -> Result<(u32, u32), ErreurPdf> {
        if !(echelle.is_finite() && echelle > 0.0) {
            return Err(ErreurPdf::Echelle(echelle));
        }
        let t = self.taille_page(page)?;
        Ok((
            (t.largeur * echelle).round().max(1.0) as u32,
            (t.hauteur * echelle).round().max(1.0) as u32,
        ))
    }

    /// Rend la page entière à l'échelle (pixels par point), en RGBA de taille physique exacte.
    pub fn rendre_page(&self, page: u32, echelle: f64) -> Result<ImageRgba, ErreurPdf> {
        let (l, h) = self.taille_raster(page, echelle)?;
        self.rendre_region(
            page,
            echelle,
            RegionPx {
                x: 0,
                y: 0,
                largeur: l,
                hauteur: h,
            },
        )
    }

    /// Rend une région du raster de la page (coordonnées en pixels à cette échelle). La région peut
    /// déborder de la page : le débordement est blanc. L'image a EXACTEMENT la taille demandée.
    pub fn rendre_region(
        &self,
        page: u32,
        echelle: f64,
        region: RegionPx,
    ) -> Result<ImageRgba, ErreurPdf> {
        let (pl, ph) = self.taille_raster(page, echelle)?;
        if region.largeur == 0 || region.hauteur == 0 {
            return Err(ErreurPdf::Rendu("région vide".into()));
        }
        let p = self.page(page)?;
        let (l, h) = (region.largeur as usize, region.hauteur as usize);
        // Tampon à nous, déjà blanc opaque : ce que la page ne couvre pas (débordement) reste blanc.
        // BGRA sans marge de fin de ligne : pas = largeur × 4.
        let mut tampon = vec![255u8; l * h * 4];
        {
            let mut bitmap = PdfBitmap::from_bytes(
                region.largeur as i32,
                region.hauteur as i32,
                PdfBitmapFormat::BGRA,
                &mut tampon,
            )
            .map_err(|e| ErreurPdf::Rendu(e.to_string()))?;
            // La page entière est dimensionnée à (pl, ph) puis décalée : seule la région tombe dans le tampon.
            // ★ Pas de `render_form_data(false)` : pdfium passerait par le chemin à matrice, qui ignore la
            //   taille fixe (la page sortirait à l'échelle 1).
            let config = PdfRenderConfig::new()
                .set_format(PdfBitmapFormat::BGRA)
                .set_fixed_size(pl as i32, ph as i32)
                .set_origin(-(region.x as i32), -(region.y as i32))
                .clear_before_rendering(false);
            p.render_into_bitmap_with_config(&mut bitmap, &config)
                .map_err(|e| ErreurPdf::Rendu(e.to_string()))?;
        }
        // BGRA → RGBA, alpha forcé opaque.
        for px in tampon.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
            px[3] = 255;
        }
        Ok(ImageRgba {
            largeur: region.largeur,
            hauteur: region.hauteur,
            octets: tampon,
        })
    }

    /// Texte brut de la page, tel que l'extrait pdfium.
    pub fn texte_page(&self, page: u32) -> Result<String, ErreurPdf> {
        let p = self.page(page)?;
        let texte = p.text().map_err(|e| ErreurPdf::Rendu(e.to_string()))?;
        Ok(texte.all())
    }

    /// Les caractères de la couche texte, dans l'ordre de lecture, avec leur boîte.
    pub fn caracteres(&self, page: u32) -> Result<Vec<Caractere>, ErreurPdf> {
        let p = self.page(page)?;
        let haut_page = f64::from(p.height().value);
        let texte = p.text().map_err(|e| ErreurPdf::Rendu(e.to_string()))?;
        let mut sortie = Vec::new();
        for c in texte.chars().iter() {
            let Some(caractere) = c.unicode_char() else {
                continue;
            };
            let boite = c
                .loose_bounds()
                .map(|r| boite_depuis(&r, haut_page))
                .unwrap_or_default();
            sortie.push(Caractere {
                index: c.index() as u32,
                caractere,
                boite,
                genere: c.is_generated().unwrap_or(false),
            });
        }
        Ok(sortie)
    }

    /// Les liens de la page (rectangles cliquables), internes ou externes. Les liens sans cible
    /// exploitable (lancement de programme, destination introuvable) sont ignorés.
    pub fn liens(&self, page: u32) -> Result<Vec<Lien>, ErreurPdf> {
        let p = self.page(page)?;
        let haut_page = f64::from(p.height().value);
        let mut sortie = Vec::new();
        for lien in p.links().iter() {
            let Ok(rect) = lien.rect() else { continue };
            let Some(cible) = cible_de(lien.action(), lien.destination()) else {
                continue;
            };
            sortie.push(Lien {
                boite: boite_depuis(&rect, haut_page),
                cible,
            });
        }
        Ok(sortie)
    }

    /// Les signets (table des matières) en arbre, déjà résolus en numéros de page.
    /// `page` vaut `None` pour une entrée sans destination interne (URL, destination perdue).
    pub fn signets(&self) -> Vec<EntreeToc> {
        entrees_depuis(self.pdf.bookmarks().root())
    }
}

/// Convertit un rectangle PDF (origine en bas) en boîte origine en haut.
fn boite_depuis(r: &PdfRect, haut_page: f64) -> Boite {
    Boite {
        gauche: f64::from(r.left().value),
        haut: haut_page - f64::from(r.top().value),
        droite: f64::from(r.right().value),
        bas: haut_page - f64::from(r.bottom().value),
    }
}

/// Résout une destination en (page 1-based, y PDF) ; mêmes types de vue que `haut_de_destination`.
fn resoudre_destination(d: &PdfDestination<'_>) -> Option<(u32, Option<f64>)> {
    let page = d.page_index().ok()? as u32 + 1;
    let y = match d.view_settings().ok()? {
        PdfDestinationViewSettings::SpecificCoordinatesAndZoom(_, y, _) => y,
        PdfDestinationViewSettings::FitPageHorizontallyToWindow(y)
        | PdfDestinationViewSettings::FitBoundsHorizontallyToWindow(y) => y,
        _ => None,
    };
    Some((page, y.map(|v| f64::from(v.value))))
}

fn cible_de(
    action: Option<PdfAction<'_>>,
    destination: Option<PdfDestination<'_>>,
) -> Option<CibleLien> {
    if let Some(a) = &action {
        if let Some(u) = a.as_uri_action() {
            return u.uri().ok().map(|uri| CibleLien::Externe { uri });
        }
        if let Some(l) = a.as_local_destination_action() {
            let d = l.destination().ok()?;
            let (page, y) = resoudre_destination(&d)?;
            return Some(CibleLien::Interne { page, y });
        }
        return None;
    }
    let (page, y) = resoudre_destination(&destination?)?;
    Some(CibleLien::Interne { page, y })
}

fn entrees_depuis(premier: Option<PdfBookmark<'_>>) -> Vec<EntreeToc> {
    let mut sortie = Vec::new();
    let mut courant = premier;
    while let Some(b) = courant {
        let (page, y) = match cible_de(b.action(), b.destination()) {
            Some(CibleLien::Interne { page, y }) => (Some(page), y),
            _ => (None, None),
        };
        sortie.push(EntreeToc {
            titre: b.title().unwrap_or_default(),
            page,
            y,
            enfants: entrees_depuis(b.first_child()),
        });
        courant = b.next_sibling();
    }
    sortie
}
