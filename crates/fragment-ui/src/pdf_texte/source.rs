// Port de app/src/ui/views/pdf/PdfView.ts (batirTexte, batirLiens : `getTextContent`,
// `getAnnotations`)
//
// D'où vient le texte des pages. Côté TS, pdf.js le lisait dans son worker, page par page, après
// l'image ; ici un fil dédié possède son propre `Document` et rend, pour une page demandée, ses
// caractères et ses liens.
//
// ★ Un `Document` ne change pas de fil (voir `pdf_pages::fil`) : le fil du texte ouvre le sien.
//   La feature `thread_safe` de pdfium-render sérialise les appels entre ce fil et celui du
//   rendu : deux documents, un seul pdfium à la fois. Le texte ne retarde donc jamais le fil des
//   images au-delà d'une page.
//
// Le mode « en ligne » lit sur le fil appelant, à la récolte : pour les tests (déterministes).

use std::collections::VecDeque;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use fragment_pdf::document::{Document, ErreurPdf};

use super::modele::TextePage;

/// Ce qui sait lire le texte d'une page : pdfium en vrai, un faux dans les tests.
pub trait SourceTexte {
    /// Le texte et les liens de la page `page` (à partir de 1).
    fn page(&mut self, page: usize) -> Result<TextePage, String>;
}

/// La source pdfium : un [`Document`] ouvert.
pub struct SourceTextePdfium {
    document: Document,
}

impl SourceTextePdfium {
    /// Ouvre le PDF (charge pdfium au besoin, voir `fragment_pdf::document::initialiser`).
    pub fn ouvrir(octets: Vec<u8>) -> Result<Self, ErreurPdf> {
        Ok(Self {
            document: Document::ouvrir(octets)?,
        })
    }
}

impl SourceTexte for SourceTextePdfium {
    fn page(&mut self, page: usize) -> Result<TextePage, String> {
        let p = u32::try_from(page).map_err(|e| e.to_string())?;
        let caracteres = self.document.caracteres(p).map_err(|e| e.to_string())?;
        // Des liens illisibles n'empêchent pas le texte : la page reste sélectionnable.
        let liens = self.document.liens(p).unwrap_or_default();
        Ok(TextePage::depuis(&caracteres, &liens))
    }
}

/// La fabrique d'une source, appelée SUR le fil du texte.
pub type FabriqueTexte = Box<dyn FnOnce() -> Result<Box<dyn SourceTexte>, String> + Send>;

/// Une page lue (ou son échec).
pub type Lecture = (usize, Result<TextePage, String>);

enum Mode {
    Fil {
        envoi: Sender<usize>,
    },
    EnLigne {
        source: Box<dyn SourceTexte>,
        file: VecDeque<usize>,
    },
}

/// Le fil du texte d'un document.
pub struct FilDeTexte {
    mode: Mode,
    /// Les lectures prêtes, à récolter par l'interface.
    boite: Arc<Mutex<VecDeque<Lecture>>>,
    /// Les demandes envoyées et pas encore récoltées.
    en_vol: usize,
}

/// Un verrou empoisonné reste utilisable : son contenu est cohérent.
fn verrou<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl FilDeTexte {
    /// Démarre un fil dédié ; `fabrique` y ouvre la source. Une source qui ne s'ouvre pas rend
    /// chaque page en erreur (une page sans texte, comme un scan).
    pub fn demarrer(fabrique: FabriqueTexte) -> Self {
        let (envoi, recu) = mpsc::channel::<usize>();
        let boite = Arc::new(Mutex::new(VecDeque::new()));
        let b = Arc::clone(&boite);
        let lance = thread::Builder::new()
            .name("fragment-pdf-texte".into())
            .spawn(move || {
                let mut source = fabrique();
                while let Ok(page) = recu.recv() {
                    let lu = match source.as_mut() {
                        Ok(s) => s.page(page),
                        Err(e) => Err(e.clone()),
                    };
                    verrou(&b).push_back((page, lu));
                }
            });
        // Fil impossible à lancer : `envoi` échouera, chaque demande reviendra en erreur.
        let _ = lance;
        Self {
            mode: Mode::Fil { envoi },
            boite,
            en_vol: 0,
        }
    }

    /// Le fil du texte d'un PDF (pdfium), à partir de ses octets.
    pub fn pdfium(octets: Vec<u8>) -> Self {
        Self::demarrer(Box::new(move || {
            SourceTextePdfium::ouvrir(octets)
                .map(|s| Box::new(s) as Box<dyn SourceTexte>)
                .map_err(|e| e.to_string())
        }))
    }

    /// Lecture sur le fil appelant, à la récolte (tests, outils).
    pub fn en_ligne(source: Box<dyn SourceTexte>) -> Self {
        Self {
            mode: Mode::EnLigne {
                source,
                file: VecDeque::new(),
            },
            boite: Arc::new(Mutex::new(VecDeque::new())),
            en_vol: 0,
        }
    }

    /// Demande le texte d'une page.
    pub fn demander(&mut self, page: usize) {
        self.en_vol += 1;
        match &mut self.mode {
            Mode::Fil { envoi } => {
                if envoi.send(page).is_err() {
                    verrou(&self.boite).push_back((page, Err("fil du texte arrêté".into())));
                }
            }
            Mode::EnLigne { file, .. } => file.push_back(page),
        }
    }

    /// Les lectures arrivées depuis la dernière récolte. En ligne, lit d'abord la file.
    pub fn recolter(&mut self) -> Vec<Lecture> {
        if let Mode::EnLigne { source, file } = &mut self.mode {
            while let Some(page) = file.pop_front() {
                let lu = source.page(page);
                verrou(&self.boite).push_back((page, lu));
            }
        }
        let lues: Vec<Lecture> = verrou(&self.boite).drain(..).collect();
        self.en_vol = self.en_vol.saturating_sub(lues.len());
        lues
    }

    /// Des lectures sont encore attendues.
    pub fn occupe(&self) -> bool {
        self.en_vol > 0
    }
}
