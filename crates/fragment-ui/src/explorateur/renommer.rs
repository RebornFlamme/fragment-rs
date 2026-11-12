// Port de app/src/ui/views/explorer/FileTree.tsx (RenameInput, commitRename, startRename)
//! Le renommage en place : le champ qui remplace le nom d'une ligne, et ce qu'il fait au commit.
//!
//! Le modèle ne connaît ni GPUI ni le focus : [`Renommage`] tient la saisie, [`Renommage::touche`]
//! traduit Entrée / Échap, [`valider`] applique au coffre. La vue lui livre les frappes.
//!
//! ★ Pour un fichier on édite le NOM SANS EXTENSION ; l'extension est recollée au commit. Un nom
//!   vide ou inchangé annule sans rien dire ; une collision est REFUSÉE par le coffre (l'erreur
//!   remonte, l'arbre garde sa vérité).

use fragment_core::{Vault, VaultError};

/// Le champ de renommage d'une ligne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renommage {
    chemin: String,
    initial: String,
    saisie: String,
    /// Tout le texte initial est encore sélectionné (`onFocus` : `select()`).
    selectionne: bool,
}

/// Ce qu'une touche fait au champ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suite {
    /// Rien : la touche est une frappe, ou ne concerne pas le champ.
    Continuer,
    /// Entrée : valider la saisie.
    Valider,
    /// Échap : abandonner sans rien changer.
    Annuler,
}

/// Le résultat d'un commit.
#[derive(Debug)]
pub enum Issue {
    /// Renommé : le nouveau chemin.
    Renomme(String),
    /// Annulé, vide ou inchangé : rien n'a bougé.
    Ignore,
    /// Refusé par le coffre (collision, parent inconnu…).
    Refuse(VaultError),
}

impl Renommage {
    /// Ouvre le champ sur `chemin` : le nom sans extension pour un fichier, le nom entier pour un
    /// dossier. `None` si le chemin n'existe pas ou est la racine (qui ne se renomme pas).
    pub fn ouvrir(vault: &Vault, chemin: &str) -> Option<Self> {
        if chemin.is_empty() {
            return None;
        }
        let initial = match vault.get_file_by_path(chemin) {
            Some(f) => f.basename().to_owned(),
            None => vault.get_folder_by_path(chemin)?.name().to_owned(),
        };
        Some(Self {
            chemin: chemin.to_owned(),
            saisie: initial.clone(),
            initial,
            selectionne: true,
        })
    }

    /// Le chemin de la ligne en renommage.
    pub fn chemin(&self) -> &str {
        &self.chemin
    }

    /// Le texte de départ (tout est sélectionné à l'ouverture).
    pub fn initial(&self) -> &str {
        &self.initial
    }

    /// Le texte saisi.
    pub fn saisie(&self) -> &str {
        &self.saisie
    }

    /// Frappe du texte : remplace la sélection initiale la première fois (`onFocus` sélectionne
    /// tout), s'ajoute ensuite.
    pub fn inserer(&mut self, texte: &str) {
        if self.selectionne {
            self.saisie.clear();
            self.selectionne = false;
        }
        self.saisie.push_str(texte);
    }

    /// Retour arrière : le dernier caractère (toute la sélection initiale d'abord).
    pub fn effacer(&mut self) {
        if self.selectionne {
            self.saisie.clear();
            self.selectionne = false;
        } else {
            self.saisie.pop();
        }
    }

    /// Une touche nommée comme `KeyDownEvent` (`"enter"`, `"escape"`, `"backspace"`, ou un texte).
    pub fn touche(&mut self, touche: &str) -> Suite {
        match touche {
            "enter" => Suite::Valider,
            "escape" => Suite::Annuler,
            "backspace" => {
                self.effacer();
                Suite::Continuer
            }
            _ => Suite::Continuer,
        }
    }
}

/// Applique un renommage (`commitRename`) : `saisie` à `None` ou vide (une fois rognée), ou
/// qui redonne le même chemin, n'est pas un renommage. Un fichier garde son extension.
pub fn valider(vault: &Vault, chemin: &str, saisie: Option<&str>) -> Issue {
    let Some(nom) = saisie.map(str::trim).filter(|n| !n.is_empty()) else {
        return Issue::Ignore;
    };
    let prefixe = parent_de(chemin);
    let extension = vault
        .get_file_by_path(chemin)
        .filter(|f| !f.extension().is_empty())
        .map_or_else(String::new, |f| format!(".{}", f.extension()));
    let cible = format!("{prefixe}{nom}{extension}");
    if cible == chemin {
        return Issue::Ignore;
    }
    let renomme = match vault.get_abstract_file_by_path(chemin) {
        Some(fichier) => match &fichier {
            fragment_core::TAbstractFile::File(f) => vault.rename(f, &cible),
            fragment_core::TAbstractFile::Folder(d) => vault.rename(d, &cible),
        },
        None => Err(VaultError::NoeudInconnu),
    };
    match renomme {
        Ok(()) => Issue::Renomme(cible),
        Err(e) => Issue::Refuse(e),
    }
}

/// `"a/b/c.md"` donne `"a/b/"`, `"c.md"` donne `""` : le préfixe d'un chemin frère.
fn parent_de(chemin: &str) -> String {
    chemin
        .rfind('/')
        .map_or_else(String::new, |i| chemin[..=i].to_owned())
}
