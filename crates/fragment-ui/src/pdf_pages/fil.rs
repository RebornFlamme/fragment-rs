// Port de app/src/ui/views/pdf/PdfView.ts (peindre : `page.render`, annulation des RenderTask)
//
// Le rendu des pages HORS du fil de l'interface. Côté TS, pdf.js rastérisait dans son worker ;
// ici un fil dédié possède le document et rend une page à la fois.
//
// ★ pdfium n'est pas réentrant et un `Document` ne change pas de fil : le fil de rendu OUVRE
//   lui-même le document (une fabrique lui est confiée) et le garde jusqu'à la fin. Un seul fil,
//   une file d'attente ; une demande annulée (page sortie, repeint plus récent) est sautée si elle
//   n'a pas commencé, et son résultat jeté sinon.
//
// Le mode « en ligne » rend sur le fil appelant, à la récolte : pour les tests (horloge simulée,
// déterministe) et les outils sans fenêtre.

use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use fragment_pdf::document::{Document, ErreurPdf, Raster, RegionPx, TaillePage};
use fragment_pdf::vue_modele::Pixels;
use gpui_kit::RenderImage;

use super::emplacements::Demande;

/// Ce qui sait rastériser les pages d'un document : pdfium en vrai, un faux dans les tests.
pub trait Rasteriseur {
    /// Les tailles de toutes les pages, en points (rotation appliquée).
    fn tailles(&mut self) -> Result<Vec<TaillePage>, String>;
    /// La page `page` (à partir de 1) rendue en EXACTEMENT `pixels` (RGBA).
    fn rendre(&mut self, page: usize, pixels: Pixels) -> Result<Raster, String>;
}

/// Le rastériseur pdfium : un [`Document`] ouvert.
pub struct RasteriseurPdfium {
    document: Document,
}

impl RasteriseurPdfium {
    /// Ouvre le PDF (charge pdfium au besoin, voir `fragment_pdf::document::initialiser`).
    pub fn ouvrir(octets: Vec<u8>) -> Result<Self, ErreurPdf> {
        Ok(Self {
            document: Document::ouvrir(octets)?,
        })
    }
}

impl Rasteriseur for RasteriseurPdfium {
    fn tailles(&mut self) -> Result<Vec<TaillePage>, String> {
        (1..=self.document.nombre_de_pages())
            .map(|p| self.document.taille_page(p).map_err(|e| e.to_string()))
            .collect()
    }

    fn rendre(&mut self, page: usize, pixels: Pixels) -> Result<Raster, String> {
        let p = u32::try_from(page).map_err(|e| e.to_string())?;
        let t = self.document.taille_page(p).map_err(|e| e.to_string())?;
        if t.largeur <= 0.0 || pixels.w == 0 || pixels.h == 0 {
            return Err("page vide".into());
        }
        // L'échelle de la largeur : pdfium rend la page à `round(largeur × e)` = `w` pixels.
        // La hauteur accrochée peut différer d'un pixel de `round(hauteur × e)` : la région
        // demandée fait EXACTEMENT `h` lignes (un débordement est blanc), comme le canvas
        // de Fragment, rempli par pdf.js à sa taille mesurée.
        let echelle = f64::from(pixels.w) / t.largeur;
        self.document
            .rendre_region(
                p,
                echelle,
                RegionPx {
                    x: 0,
                    y: 0,
                    largeur: pixels.w,
                    hauteur: pixels.h,
                },
            )
            .map_err(|e| e.to_string())
    }
}

/// Une image prête pour GPUI et sa taille physique.
pub type ImageRendue = (Arc<RenderImage>, Pixels);

/// Ce que le fil renvoie.
pub enum Reponse {
    /// Le document est ouvert : tailles des pages (points), ou l'erreur d'ouverture.
    Ouvert(Result<Vec<TaillePage>, String>),
    /// Une page rendue (ou son échec).
    Rendu {
        page: usize,
        jeton: u64,
        resultat: Result<ImageRendue, String>,
    },
}

enum Commande {
    Rendre(Demande),
    Fin,
}

/// La fabrique d'un rastériseur, appelée SUR le fil de rendu.
pub type Fabrique = Box<dyn FnOnce() -> Result<Box<dyn Rasteriseur>, String> + Send>;

enum Mode {
    Fil {
        envoi: Sender<Commande>,
    },
    EnLigne {
        rasteriseur: Option<Box<dyn Rasteriseur>>,
        file: VecDeque<Demande>,
    },
}

/// Le fil de rendu d'un document.
pub struct FilDeRendu {
    mode: Mode,
    /// Les jetons des demandes encore voulues (une annulation les retire).
    vivants: Arc<Mutex<HashSet<u64>>>,
    /// Les réponses prêtes, à récolter par l'interface.
    boite: Arc<Mutex<VecDeque<Reponse>>>,
}

/// Un verrou empoisonné (un rendu a paniqué) reste utilisable : son contenu est cohérent.
fn verrou<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// RGBA (pdfium, via `Document`) → BGRA à alpha droit (les images de GPUI).
fn vers_image(raster: Raster) -> Result<ImageRendue, String> {
    let Raster {
        largeur,
        hauteur,
        mut octets,
    } = raster;
    for px in octets.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let tampon = image::RgbaImage::from_raw(largeur, hauteur, octets)
        .ok_or_else(|| String::from("raster de taille incohérente"))?;
    Ok((
        Arc::new(RenderImage::new(vec![image::Frame::new(tampon)])),
        Pixels {
            w: largeur,
            h: hauteur,
        },
    ))
}

fn rendre(r: &mut dyn Rasteriseur, d: &Demande) -> Result<ImageRendue, String> {
    r.rendre(d.page, d.pixels).and_then(vers_image)
}

fn boucle(
    fabrique: Fabrique,
    recu: Receiver<Commande>,
    vivants: Arc<Mutex<HashSet<u64>>>,
    boite: Arc<Mutex<VecDeque<Reponse>>>,
) {
    let mut r = match fabrique() {
        Ok(r) => r,
        Err(e) => {
            verrou(&boite).push_back(Reponse::Ouvert(Err(e)));
            return;
        }
    };
    let tailles = r.tailles();
    let ouvert = tailles.is_ok();
    verrou(&boite).push_back(Reponse::Ouvert(tailles));
    if !ouvert {
        return;
    }
    while let Ok(commande) = recu.recv() {
        let d = match commande {
            Commande::Rendre(d) => d,
            Commande::Fin => break,
        };
        // Annulée avant de commencer : sautée.
        if !verrou(&vivants).contains(&d.jeton) {
            continue;
        }
        let resultat = rendre(r.as_mut(), &d);
        // Annulée pendant le rendu : jetée.
        if verrou(&vivants).remove(&d.jeton) {
            verrou(&boite).push_back(Reponse::Rendu {
                page: d.page,
                jeton: d.jeton,
                resultat,
            });
        }
    }
}

impl FilDeRendu {
    /// Démarre un fil de rendu dédié ; `fabrique` y ouvre le document.
    pub fn demarrer(fabrique: Fabrique) -> Self {
        let (envoi, recu) = mpsc::channel();
        let vivants = Arc::new(Mutex::new(HashSet::new()));
        let boite = Arc::new(Mutex::new(VecDeque::new()));
        let (v, b) = (Arc::clone(&vivants), Arc::clone(&boite));
        let lance = thread::Builder::new()
            .name("fragment-pdf-rendu".into())
            .spawn(move || boucle(fabrique, recu, v, b));
        if let Err(e) = lance {
            verrou(&boite).push_back(Reponse::Ouvert(Err(e.to_string())));
        }
        Self {
            mode: Mode::Fil { envoi },
            vivants,
            boite,
        }
    }

    /// Le fil de rendu d'un PDF (pdfium), à partir de ses octets.
    pub fn pdfium(octets: Vec<u8>) -> Self {
        Self::demarrer(Box::new(move || {
            RasteriseurPdfium::ouvrir(octets)
                .map(|r| Box::new(r) as Box<dyn Rasteriseur>)
                .map_err(|e| e.to_string())
        }))
    }

    /// Rendu sur le fil appelant, à la récolte (tests, outils) : le document est ouvert tout de suite.
    pub fn en_ligne(mut rasteriseur: Box<dyn Rasteriseur>) -> Self {
        let tailles = rasteriseur.tailles();
        let ouvert = tailles.is_ok();
        let boite = Arc::new(Mutex::new(VecDeque::from([Reponse::Ouvert(tailles)])));
        Self {
            mode: Mode::EnLigne {
                rasteriseur: ouvert.then_some(rasteriseur),
                file: VecDeque::new(),
            },
            vivants: Arc::new(Mutex::new(HashSet::new())),
            boite,
        }
    }

    /// Met une demande dans la file.
    pub fn soumettre(&mut self, d: Demande) {
        verrou(&self.vivants).insert(d.jeton);
        match &mut self.mode {
            Mode::Fil { envoi } => {
                // Fil mort (ouverture ratée) : la demande ne reviendra jamais, on l'oublie.
                if envoi.send(Commande::Rendre(d)).is_err() {
                    verrou(&self.vivants).remove(&d.jeton);
                }
            }
            Mode::EnLigne { file, .. } => file.push_back(d),
        }
    }

    /// Annule une demande : sautée si elle attend, jetée si elle est en cours.
    pub fn annuler(&mut self, jeton: u64) {
        verrou(&self.vivants).remove(&jeton);
    }

    /// Les réponses arrivées depuis la dernière récolte. En ligne, rend d'abord la file.
    pub fn recolter(&mut self) -> Vec<Reponse> {
        if let Mode::EnLigne { rasteriseur, file } = &mut self.mode {
            while let Some(d) = file.pop_front() {
                if !verrou(&self.vivants).remove(&d.jeton) {
                    continue;
                }
                let resultat = match rasteriseur.as_mut() {
                    Some(r) => rendre(r.as_mut(), &d),
                    None => Err("document fermé".into()),
                };
                verrou(&self.boite).push_back(Reponse::Rendu {
                    page: d.page,
                    jeton: d.jeton,
                    resultat,
                });
            }
        }
        verrou(&self.boite).drain(..).collect()
    }

    /// Des demandes sont encore attendues.
    pub fn occupe(&self) -> bool {
        !verrou(&self.vivants).is_empty()
    }
}

impl Drop for FilDeRendu {
    fn drop(&mut self) {
        // Le fil finit son rendu en cours puis s'arrête ; on ne l'attend pas (pas de blocage
        // de l'interface à la fermeture d'un onglet).
        verrou(&self.vivants).clear();
        if let Mode::Fil { envoi } = &self.mode {
            let _ = envoi.send(Commande::Fin);
        }
    }
}
