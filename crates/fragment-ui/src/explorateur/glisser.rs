// Port de app/src/ui/views/explorer/FileTree.tsx (folderOf, freePath, importFiles, dropFolderPath,
// SPRING_DELAY, setTarget / collapseStale / endDrag, file-drag-over / file-move)
//! Le glisser-déposer de l'explorateur : un fichier glissé sur un dossier s'y déplace, les fichiers
//! déposés depuis le système sont importés au nom libre, et un dossier fermé survolé s'ouvre après
//! un délai (le « ressort »).
//!
//! Le modèle ne connaît pas GPUI : il reçoit des chemins, des positions et une horloge en ms.
//!
//! ★ Au dépôt on GARDE ouvert ce que le ressort a ouvert (on voit où le fichier a atterri) ; si le
//!   curseur repart sans lâcher, on referme ce que le glisser avait ouvert.
//! ★ Les DOSSIERS déposés depuis le système ne sont pas importés (comme Fragment : minimal assumé).

use std::path::PathBuf;

use fragment_core::vault::coffre::VaultResult;
use fragment_core::{DataWriteOptions, TFile, Vault};

use super::deplacer::deplacer_vers;

/// Le délai avant qu'un dossier fermé s'ouvre sous le curseur (`SPRING_DELAY`).
pub const DELAI_RESSORT_MS: u64 = 450;

/// Le dossier visé par un chemin (`folderOf`) : le dossier lui-même, le parent d'un fichier, ou la
/// racine (chemin vide, ou inconnu).
pub fn dossier_de(vault: &Vault, chemin: &str) -> String {
    if vault.get_folder_by_path(chemin).is_some() {
        return chemin.to_owned();
    }
    if vault.get_file_by_path(chemin).is_some() {
        return chemin.rfind('/').map_or("", |i| &chemin[..i]).to_owned();
    }
    String::new()
}

/// Le premier chemin libre « nom.ext », « nom 1.ext »… dans `dossier` (`freePath`). Un point en
/// première position ne sépare pas d'extension (`.gitignore` est un nom).
pub fn chemin_libre_pour(vault: &Vault, dossier: &str, nom_fichier: &str) -> String {
    let (base, ext) = match nom_fichier.rfind('.') {
        Some(i) if i > 0 => (&nom_fichier[..i], &nom_fichier[i + 1..]),
        _ => (nom_fichier, ""),
    };
    vault.get_available_path(dossier, base, ext)
}

/// La fin d'un glisser interne (`file-move`) : déplace `fichier` dans le dossier visé par `cible`.
/// Vrai si l'élément a bougé ; faux s'il est déjà dans ce dossier.
pub fn deplacer_fichier(vault: &Vault, fichier: &str, cible: &str) -> VaultResult<bool> {
    deplacer_vers(vault, fichier, &dossier_de(vault, cible))
}

/// Un fichier à importer : son nom et ses octets.
pub type Entrant = (String, Vec<u8>);

/// Importe des fichiers dans `dossier` au nom libre (`importFiles`) : chaque fichier est écrit par
/// le coffre (`create_binary` annonce `create`, l'arbre se rafraîchit seul). Un échec n'arrête pas
/// les suivants ; chaque issue est rendue avec le nom d'origine.
pub fn importer(
    vault: &Vault,
    dossier: &str,
    fichiers: &[Entrant],
) -> Vec<(String, VaultResult<TFile>)> {
    fichiers
        .iter()
        .map(|(nom, octets)| {
            let cible = chemin_libre_pour(vault, dossier, nom);
            (
                nom.clone(),
                vault.create_binary(&cible, octets, DataWriteOptions::default()),
            )
        })
        .collect()
}

/// Lit des chemins déposés depuis le système, puis les importe. Les dossiers et les fichiers
/// illisibles sont ignorés (leur nom est rendu avec l'erreur d'E/S).
pub fn importer_chemins(
    vault: &Vault,
    dossier: &str,
    chemins: &[PathBuf],
) -> Vec<(String, VaultResult<TFile>)> {
    let mut entrants = Vec::new();
    let mut echecs = Vec::new();
    for chemin in chemins {
        let nom = chemin
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if chemin.is_dir() {
            continue;
        }
        match std::fs::read(chemin) {
            Ok(octets) => entrants.push((nom, octets)),
            Err(e) => echecs.push((nom, Err(e.into()))),
        }
    }
    let mut issues = importer(vault, dossier, &entrants);
    issues.extend(echecs);
    issues
}

/// Le rang de la ligne sous l'ordonnée `y` (relative au haut de la liste, défilement compris),
/// d'après les hauteurs des lignes visibles ; `None` sous la dernière (le vide de l'arbre).
pub fn rang_a(hauteurs: &[f32], y: f32) -> Option<usize> {
    if y < 0.0 {
        return None;
    }
    let mut bas = 0.0;
    for (i, h) in hauteurs.iter().enumerate() {
        bas += h;
        if y < bas {
            return Some(i);
        }
    }
    None
}

/// Ce que le ressort demande à la vue de faire à l'arbre.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Ordres {
    /// Dossiers à refermer (ouverts par le glisser, plus sur le chemin de la cible).
    pub fermer: Vec<String>,
}

/// Le ressort : suit le dossier survolé pendant un dépôt, programme l'ouverture d'un dossier fermé
/// et retient ce qu'il a ouvert.
#[derive(Debug, Default)]
pub struct Ressort {
    cible: Option<String>,
    echeance: Option<(String, u64)>,
    auto_ouverts: Vec<String>,
}

impl Ressort {
    /// Un ressort au repos.
    pub fn new() -> Self {
        Self::default()
    }

    /// Le dossier actuellement visé (colore sa ligne), `""` = la racine.
    pub fn cible(&self) -> Option<&str> {
        self.cible.as_deref()
    }

    /// Les dossiers que le glisser a ouverts.
    pub fn auto_ouverts(&self) -> &[String] {
        &self.auto_ouverts
    }

    /// Le curseur est sur `dossier` à l'instant `maintenant` (ms). `ouvert` dit si un dossier est
    /// ouvert dans l'arbre. Même cible que la dernière : rien ne change (pas de churn).
    pub fn survoler(
        &mut self,
        dossier: &str,
        ouvert: impl Fn(&str) -> bool,
        maintenant: u64,
    ) -> Ordres {
        if self.cible.as_deref() == Some(dossier) {
            return Ordres::default();
        }
        self.cible = Some(dossier.to_owned());
        self.echeance = None;
        // Referme ce qu'on a ouvert et qui n'est plus sur le chemin de la cible.
        let (gardes, perimes): (Vec<String>, Vec<String>) = std::mem::take(&mut self.auto_ouverts)
            .into_iter()
            .partition(|p| dossier == p || est_ancetre(p, dossier));
        self.auto_ouverts = gardes;
        // Dossier fermé (jamais la racine) : programme son ouverture après le délai.
        if !dossier.is_empty() && !ouvert(dossier) {
            self.echeance = Some((dossier.to_owned(), maintenant + DELAI_RESSORT_MS));
        }
        Ordres { fermer: perimes }
    }

    /// L'horloge avance : si le délai est échu, rend le dossier à ouvrir (et le retient).
    pub fn avancer(&mut self, maintenant: u64) -> Option<String> {
        let (dossier, echeance) = self.echeance.as_ref()?;
        if maintenant < *echeance {
            return None;
        }
        let dossier = dossier.clone();
        self.echeance = None;
        self.auto_ouverts.push(dossier.clone());
        Some(dossier)
    }

    /// Le glisser est fini. `restaurer` : on repart sans lâcher, il faut refermer ce qui a été
    /// ouvert ; au dépôt on le garde ouvert. Rend les dossiers à refermer.
    pub fn finir(&mut self, restaurer: bool) -> Vec<String> {
        self.cible = None;
        self.echeance = None;
        let ouverts = std::mem::take(&mut self.auto_ouverts);
        if restaurer { ouverts } else { Vec::new() }
    }
}

/// `p` est-il un ancêtre strict de `chemin` ?
fn est_ancetre(p: &str, chemin: &str) -> bool {
    chemin.len() > p.len() && chemin.starts_with(p) && chemin.as_bytes()[p.len()] == b'/'
}
