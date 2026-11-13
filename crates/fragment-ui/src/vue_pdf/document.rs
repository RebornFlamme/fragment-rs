// Port de app/src/ui/views/pdf/PdfView.ts (onLoadFile : UN document ouvert par vue, que les pages,
// les vignettes, la couche texte et la recherche interrogent tous).
//! Le document partagé d'une vue PDF : un seul [`Document`] ouvert, sur un fil qui lui est propre,
//! et des mandataires qui lui posent leurs questions.
//!
//! ★ DÉCISION DU CHEF (fr-t4-texte) : le PDF n'est jamais chargé deux fois. Les quatre clients
//!   (colonne des pages, vignettes, couche texte, recherche) ont chacun leur fil — ils ne doivent
//!   pas s'attendre entre eux côté interface —, mais leurs fils ne tiennent qu'un MANDATAIRE : une
//!   file vers le fil du document, qui seul possède l'objet pdfium (pdfium n'est pas réentrant,
//!   ses appels étaient de toute façon sérialisés par le moteur).
//! ★ Le `Document` n'est pas `Send` : il naît SUR son fil, à partir des octets, et y meurt quand le
//!   dernier mandataire est lâché (fin de la file).

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use fragment_pdf::document::{Caractere, Document, Lien, Raster, RegionPx, TaillePage};
use fragment_pdf::pdf_outline::EntreeToc;
use fragment_pdf::vue_modele::Pixels;

use crate::pdf_pages::{FilDeRendu, Rasteriseur};
use crate::pdf_texte::{FilDeTexte, SourceTexte, TextePage};

type Reponse<T> = Sender<Result<T, String>>;

/// Une question au fil du document.
enum Requete {
    Tailles(Reponse<Vec<TaillePage>>),
    Rendre(usize, Pixels, Reponse<Raster>),
    Texte(usize, Reponse<(Vec<Caractere>, Vec<Lien>)>),
    Signets(Reponse<Vec<EntreeToc>>),
}

/// La poignée du fil du document : clonable, `Send`, une file d'envoi.
#[derive(Clone)]
pub struct DocumentPartage {
    envoi: Sender<Requete>,
}

/// Une page rendue en EXACTEMENT `pixels` (même contrat que le rastériseur pdfium des pages).
fn rendre(document: &Document, page: usize, pixels: Pixels) -> Result<Raster, String> {
    let p = u32::try_from(page).map_err(|e| e.to_string())?;
    let t = document.taille_page(p).map_err(|e| e.to_string())?;
    if t.largeur <= 0.0 || pixels.w == 0 || pixels.h == 0 {
        return Err("page vide".into());
    }
    let echelle = f64::from(pixels.w) / t.largeur;
    document
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

fn repondre(document: &Result<Document, String>, requete: Requete) {
    let d = document.as_ref().map_err(Clone::clone);
    // Un client parti (vue fermée pendant un rendu) : la réponse se perd, sans conséquence.
    match requete {
        Requete::Tailles(r) => {
            let _ = r.send(d.and_then(|d| {
                (1..=d.nombre_de_pages())
                    .map(|p| d.taille_page(p).map_err(|e| e.to_string()))
                    .collect()
            }));
        }
        Requete::Rendre(page, pixels, r) => {
            let _ = r.send(d.and_then(|d| rendre(d, page, pixels)));
        }
        Requete::Texte(page, r) => {
            let _ = r.send(d.and_then(|d| {
                let p = u32::try_from(page).map_err(|e| e.to_string())?;
                let caracteres = d.caracteres(p).map_err(|e| e.to_string())?;
                // Des liens illisibles n'empêchent pas le texte.
                Ok((caracteres, d.liens(p).unwrap_or_default()))
            }));
        }
        Requete::Signets(r) => {
            let _ = r.send(d.map(Document::signets));
        }
    }
}

impl DocumentPartage {
    /// Ouvre `octets` sur un fil dédié. Une ouverture ratée rend chaque réponse en erreur.
    pub fn ouvrir(octets: Vec<u8>) -> Self {
        let (envoi, recu) = mpsc::channel::<Requete>();
        let lance = thread::Builder::new()
            .name("fragment-pdf-document".into())
            .spawn(move || {
                let document = Document::ouvrir(octets).map_err(|e| e.to_string());
                while let Ok(requete) = recu.recv() {
                    repondre(&document, requete);
                }
            });
        // Fil impossible à lancer : chaque envoi échouera, chaque client verra une erreur.
        let _ = lance;
        Self { envoi }
    }

    fn demander<T>(&self, faire: impl FnOnce(Reponse<T>) -> Requete) -> Result<T, String> {
        let (r, attente) = mpsc::channel();
        self.envoi
            .send(faire(r))
            .map_err(|_| String::from("fil du document arrêté"))?;
        attente
            .recv()
            .map_err(|_| String::from("fil du document arrêté"))?
    }

    /// La table des matières, sans attendre : la réponse arrive sur le récepteur rendu.
    pub fn signets(&self) -> Receiver<Result<Vec<EntreeToc>, String>> {
        let (r, attente) = mpsc::channel();
        let _ = self.envoi.send(Requete::Signets(r));
        attente
    }

    /// Un fil de rendu (pages ou vignettes) branché sur ce document.
    pub fn fil_de_rendu(&self) -> FilDeRendu {
        let d = self.clone();
        FilDeRendu::demarrer(Box::new(move || Ok(Box::new(d) as Box<dyn Rasteriseur>)))
    }

    /// Un fil de texte (couche texte ou recherche) branché sur ce document.
    pub fn fil_de_texte(&self) -> FilDeTexte {
        let d = self.clone();
        FilDeTexte::demarrer(Box::new(move || Ok(Box::new(d) as Box<dyn SourceTexte>)))
    }
}

impl Rasteriseur for DocumentPartage {
    fn tailles(&mut self) -> Result<Vec<TaillePage>, String> {
        self.demander(Requete::Tailles)
    }
    fn rendre(&mut self, page: usize, pixels: Pixels) -> Result<Raster, String> {
        self.demander(|r| Requete::Rendre(page, pixels, r))
    }
}

impl SourceTexte for DocumentPartage {
    fn page(&mut self, page: usize) -> Result<TextePage, String> {
        let (caracteres, liens) = self.demander(|r| Requete::Texte(page, r))?;
        Ok(TextePage::depuis(&caracteres, &liens))
    }
}
