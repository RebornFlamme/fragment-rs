//! Chargement de la bibliothèque pdfium, une seule fois par processus.
//!
//! pdfium n'est JAMAIS lié statiquement : le binaire épinglé (`scripts/pdfium.ps1`, `scripts/pdfium.sh`)
//! est téléchargé hors dépôt, et son dossier est donné par `PDFIUM_LIB_DIR` ou par l'appelant.
//! Licences du binaire : BSD-3 (pdfium) et Apache-2 (pdfium-binaries), voir la PR de fr-t4-pdfium.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use pdfium_render::prelude::Pdfium;

use super::ErreurPdf;

/// Variable d'environnement qui désigne le dossier contenant `pdfium.dll` / `libpdfium.so`.
pub const VARIABLE_DOSSIER: &str = "PDFIUM_LIB_DIR";

static MOTEUR: OnceLock<Pdfium> = OnceLock::new();

/// ★ Sérialise le chargement : pdfium refuse une seconde liaison dans le processus
///   (`PdfiumLibraryBindingsAlreadyInitialized`), donc deux fils ne doivent jamais lier ensemble.
static VERROU: Mutex<()> = Mutex::new(());

/// Le moteur chargé, ou une erreur claire s'il ne l'est pas.
pub(super) fn moteur() -> Result<&'static Pdfium, ErreurPdf> {
    if let Some(m) = MOTEUR.get() {
        return Ok(m);
    }
    initialiser(None)?;
    MOTEUR.get().ok_or(ErreurPdf::MoteurAbsent)
}

/// Charge pdfium (idempotent : un moteur déjà chargé est conservé, même si `dossier` diffère).
///
/// Ordre de recherche : `dossier`, puis `PDFIUM_LIB_DIR`, puis la bibliothèque du système.
/// Dans un dossier, on essaie le dossier lui-même, puis `bin/` et `lib/` (disposition de l'archive).
pub fn initialiser(dossier: Option<&Path>) -> Result<(), ErreurPdf> {
    if MOTEUR.get().is_some() {
        return Ok(());
    }
    // Un verrou empoisonné ne protège aucune donnée : on le reprend tel quel.
    let _garde = VERROU
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if MOTEUR.get().is_some() {
        return Ok(()); // un autre fil a chargé pendant l'attente
    }
    let demande: Option<PathBuf> = dossier
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os(VARIABLE_DOSSIER).map(PathBuf::from));

    let mut dernier = String::from("aucune bibliothèque trouvée");
    let mut liaison = None;
    if let Some(base) = demande {
        for candidat in [base.clone(), base.join("bin"), base.join("lib")] {
            let chemin = Pdfium::pdfium_platform_library_name_at_path(&candidat);
            match Pdfium::bind_to_library(&chemin) {
                Ok(l) => {
                    liaison = Some(l);
                    break;
                }
                Err(e) => dernier = format!("{} : {e}", chemin.display()),
            }
        }
    } else {
        match Pdfium::bind_to_system_library() {
            Ok(l) => liaison = Some(l),
            Err(e) => dernier = format!("bibliothèque système : {e}"),
        }
    }
    let liaison = liaison.ok_or(ErreurPdf::Chargement(dernier))?;
    // Sous verrou : aucun autre fil ne peut avoir posé le moteur entre-temps.
    let _ = MOTEUR.set(Pdfium::new(liaison));
    Ok(())
}
