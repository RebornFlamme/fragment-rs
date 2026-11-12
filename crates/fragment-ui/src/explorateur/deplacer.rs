// Port de app/src/ui/views/explorer/FileTree.tsx (FolderSuggestModal) et de
// app/src/ui/core/FuzzySuggestModal.ts
//! « Déplacer vers… » : la modale floue des dossiers où l'on peut envoyer un fichier ou un dossier.
//!
//! On propose tous les dossiers (racine comprise) SAUF la source elle-même, ses descendants et son
//! dossier parent actuel (déjà là). Le déplacement est un `rename` vers `dossier/nom` ; une collision
//! est refusée par le coffre (l'interface ne ment pas).

use std::rc::Rc;

use fragment_core::vault::coffre::VaultResult;
use fragment_core::{InputEvent, TAbstractFile, TFolder, Vault};

use crate::suggest::{ElementsFlous, Flou, ModaleSuggestion, ParametresSuggestion};

/// La clé de la modale (une seule ouverte à la fois).
pub const CLE_MODALE: &str = "explorateur-deplacer";

/// Le nom de la racine dans la liste.
pub const TEXTE_RACINE: &str = "/ (racine du coffre)";

/// Le dossier parent d'un chemin (`""` pour un élément de la racine).
fn parent_de(chemin: &str) -> &str {
    chemin.rfind('/').map_or("", |i| &chemin[..i])
}

/// Les dossiers où `chemin` peut aller : `getItems` de la modale.
pub fn dossiers_destination(vault: &Vault, chemin: &str) -> Vec<TFolder> {
    let source = vault.get_abstract_file_by_path(chemin);
    let dossier_source = source.as_ref().is_some_and(TAbstractFile::is_folder);
    let parent = parent_de(chemin);
    let dedans = format!("{chemin}/");
    vault
        .get_all_folders(true)
        .into_iter()
        .filter(|f| {
            if f.path == parent {
                return false;
            }
            if dossier_source && (f.path == chemin || f.path.starts_with(&dedans)) {
                return false;
            }
            true
        })
        .collect()
}

/// Le texte d'un dossier dans la liste (`getItemText`).
pub fn texte_dossier(dossier: &TFolder) -> String {
    if dossier.is_root() {
        TEXTE_RACINE.to_owned()
    } else {
        dossier.path.clone()
    }
}

/// Le champ vide de la modale (`setPlaceholder`).
pub fn placeholder(vault: &Vault, chemin: &str) -> String {
    let nom = vault
        .get_abstract_file_by_path(chemin)
        .map_or_else(|| chemin.to_owned(), |f| f.name().to_owned());
    format!("Déplacer « {nom} » vers…")
}

/// Le déplacement lui-même (`onChooseItem`) : `rename` vers `dossier/nom`. Vrai si quelque chose a
/// bougé ; faux si la destination est déjà la place de l'élément. Une collision remonte en erreur.
pub fn deplacer_vers(vault: &Vault, chemin: &str, dossier: &str) -> VaultResult<bool> {
    let Some(source) = vault.get_abstract_file_by_path(chemin) else {
        return Ok(false);
    };
    let destination = if dossier.is_empty() {
        source.name().to_owned()
    } else {
        format!("{dossier}/{}", source.name())
    };
    if destination == chemin {
        return Ok(false);
    }
    match &source {
        TAbstractFile::File(f) => vault.rename(f, &destination)?,
        TAbstractFile::Folder(d) => vault.rename(d, &destination)?,
    }
    Ok(true)
}

/// Les trous de la modale floue : les dossiers permis, leur texte, le déplacement au choix.
pub struct SourceDossiers {
    vault: Rc<Vault>,
    chemin: String,
    signaler: Rc<dyn Fn(&str)>,
}

impl SourceDossiers {
    /// La source des dossiers où `chemin` peut aller. `signaler` reçoit un déplacement refusé.
    pub fn new(vault: Rc<Vault>, chemin: &str, signaler: Rc<dyn Fn(&str)>) -> Self {
        Self {
            vault,
            chemin: chemin.to_owned(),
            signaler,
        }
    }
}

impl ElementsFlous for SourceDossiers {
    type Element = TFolder;

    fn elements(&mut self) -> Vec<TFolder> {
        dossiers_destination(&self.vault, &self.chemin)
    }

    fn texte(&self, dossier: &TFolder) -> String {
        texte_dossier(dossier)
    }

    fn choisir_element(&mut self, dossier: TFolder, _evenement: Option<InputEvent>) {
        if let Err(e) = deplacer_vers(&self.vault, &self.chemin, &dossier.path) {
            (self.signaler)(&format!("déplacement refusé : {e}"));
        }
    }
}

/// Ouvre la modale « Déplacer vers… » pour `chemin`.
pub fn ouvrir_modale(vault: Rc<Vault>, chemin: &str, signaler: Rc<dyn Fn(&str)>) {
    let parametres = ParametresSuggestion {
        placeholder: placeholder(&vault, chemin),
        ..ParametresSuggestion::default()
    };
    let source = Flou(SourceDossiers::new(vault, chemin, signaler));
    ModaleSuggestion::ouvrir(CLE_MODALE, source, parametres);
}
