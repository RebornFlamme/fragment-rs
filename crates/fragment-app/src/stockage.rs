// Port des accès disque que Fragment fait par l'adapter du coffre : `.fragment/workspace.json`
// (Workspace.saveLayout / loadLayout), `.fragment/appearance.json` (ConfigStore),
// `.fragment/hotkeys.json` (HotkeyManager) et la lecture des plugins installés (Plugins).
//
//! Les stockages de l'application : sur un DOSSIER DE TRAVAIL (passé en argument au binaire), ou
//! en mémoire (tests, scènes de pixels).
//!
//! ★ T1 n'a pas de coffre (T2) : le dossier de travail ne sert qu'à porter `.fragment/`. Le
//!   workspace et l'apparence y vivent exactement aux chemins de Fragment, de sorte que T2 n'aura
//!   qu'à pointer le même code sur la racine du coffre.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use fragment_core::config::{ErreurConfig, StockageConfig, StockageMemoire};
use fragment_core::{
    ErreurLayout, HotkeyStore, LayoutMemoire, LayoutStore, Listing, PluginStorage,
};

/// Le dossier de configuration, à la racine du dossier de travail (le `configDir` de Fragment).
pub const DOSSIER_CONFIG: &str = ".fragment";
/// Le layout du workspace.
pub const FICHIER_WORKSPACE: &str = "workspace.json";
/// Les raccourcis personnalisés.
pub const FICHIER_HOTKEYS: &str = "hotkeys.json";

/// Les quatre stockages qu'il faut au démarrage.
#[derive(Clone)]
pub struct Stockages {
    pub layout: Rc<dyn LayoutStore>,
    pub config: Rc<dyn StockageConfig>,
    pub raccourcis: Rc<dyn HotkeyStore>,
    pub plugins: Rc<dyn PluginStorage>,
}

impl Stockages {
    /// Tout en mémoire. Les clones partagent les mêmes fichiers : deux démarrages successifs sur
    /// le même `Stockages` sont un REDÉMARRAGE (`workspace-layout`).
    pub fn memoire() -> Self {
        Self {
            layout: Rc::new(LayoutMemoire::new()),
            config: Rc::new(StockageMemoire::new()),
            raccourcis: Rc::new(RaccourcisMemoire::default()),
            plugins: Rc::new(AucunPlugin),
        }
    }

    /// Sur le dossier de travail `racine` : `.fragment/` y est créé à la première écriture.
    pub fn dossier(racine: &Path) -> Self {
        let config = racine.join(DOSSIER_CONFIG);
        Self {
            layout: Rc::new(LayoutFichier(config.join(FICHIER_WORKSPACE))),
            config: Rc::new(ConfigFichiers(config.clone())),
            raccourcis: Rc::new(RaccourcisFichier(config.join(FICHIER_HOTKEYS))),
            plugins: Rc::new(PluginsDossier(racine.to_path_buf())),
        }
    }
}

/// Lit un fichier ; `None` s'il n'existe pas.
fn lire(chemin: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(chemin) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Écrit un fichier, en créant son dossier au besoin (le `mkdir` de Fragment).
fn ecrire(chemin: &Path, texte: &str) -> std::io::Result<()> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(chemin, texte)
}

/// `.fragment/workspace.json`.
pub struct LayoutFichier(pub PathBuf);

impl LayoutStore for LayoutFichier {
    fn lire(&self) -> Result<Option<String>, ErreurLayout> {
        lire(&self.0).map_err(|e| ErreurLayout::Lecture(e.to_string()))
    }

    fn ecrire(&self, json: &str) -> Result<(), ErreurLayout> {
        ecrire(&self.0, json).map_err(|e| ErreurLayout::Ecriture(e.to_string()))
    }
}

/// Les fichiers de configuration de `.fragment/` (`appearance.json`…).
pub struct ConfigFichiers(pub PathBuf);

impl StockageConfig for ConfigFichiers {
    fn lire(&self, fichier: &str) -> Result<Option<String>, ErreurConfig> {
        lire(&self.0.join(fichier))
            .map_err(|e| ErreurConfig::Lecture(fichier.into(), e.to_string()))
    }

    fn ecrire(&self, fichier: &str, contenu: &str) -> Result<(), ErreurConfig> {
        ecrire(&self.0.join(fichier), contenu)
            .map_err(|e| ErreurConfig::Ecriture(fichier.into(), e.to_string()))
    }
}

/// `.fragment/hotkeys.json`.
pub struct RaccourcisFichier(pub PathBuf);

impl HotkeyStore for RaccourcisFichier {
    fn read(&self) -> std::io::Result<Option<String>> {
        lire(&self.0)
    }

    fn write(&self, json: &str) -> std::io::Result<()> {
        ecrire(&self.0, json)
    }
}

/// Les raccourcis en mémoire, partagés entre clones.
#[derive(Default, Clone)]
pub struct RaccourcisMemoire(Rc<RefCell<Option<String>>>);

impl HotkeyStore for RaccourcisMemoire {
    fn read(&self) -> std::io::Result<Option<String>> {
        Ok(self.0.borrow().clone())
    }

    fn write(&self, json: &str) -> std::io::Result<()> {
        *self.0.borrow_mut() = Some(json.to_owned());
        Ok(())
    }
}

/// Aucun plugin installé (mémoire) : le dossier `plugins/` n'existe pas.
pub struct AucunPlugin;

impl PluginStorage for AucunPlugin {
    fn exists(&self, _path: &str) -> std::io::Result<bool> {
        Ok(false)
    }
    fn list(&self, _path: &str) -> std::io::Result<Listing> {
        Ok(Listing::default())
    }
    fn read(&self, path: &str) -> std::io::Result<String> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            path.to_owned(),
        ))
    }
    fn write(&self, _path: &str, _data: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn mkdir(&self, _path: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn remove(&self, _path: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn rmdir(&self, _path: &str, _recursive: bool) -> std::io::Result<()> {
        Ok(())
    }
}

/// Les plugins du dossier de travail (chemins relatifs, séparateur `/`). Lecture seule en T1 : le
/// chargeur communautaire arrive avec T10, l'écriture ne sert qu'à l'activation.
pub struct PluginsDossier(pub PathBuf);

impl PluginsDossier {
    fn chemin(&self, relatif: &str) -> PathBuf {
        relatif
            .split('/')
            .filter(|s| !s.is_empty())
            .fold(self.0.clone(), |c, s| c.join(s))
    }
}

impl PluginStorage for PluginsDossier {
    fn exists(&self, path: &str) -> std::io::Result<bool> {
        Ok(self.chemin(path).exists())
    }

    fn list(&self, path: &str) -> std::io::Result<Listing> {
        let mut l = Listing::default();
        for entree in std::fs::read_dir(self.chemin(path))? {
            let entree = entree?;
            let nom = format!("{path}/{}", entree.file_name().to_string_lossy());
            if entree.file_type()?.is_dir() {
                l.folders.push(nom);
            } else {
                l.files.push(nom);
            }
        }
        Ok(l)
    }

    fn read(&self, path: &str) -> std::io::Result<String> {
        std::fs::read_to_string(self.chemin(path))
    }

    fn write(&self, path: &str, data: &str) -> std::io::Result<()> {
        ecrire(&self.chemin(path), data)
    }

    fn mkdir(&self, path: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(self.chemin(path))
    }

    fn remove(&self, path: &str) -> std::io::Result<()> {
        std::fs::remove_file(self.chemin(path))
    }

    fn rmdir(&self, path: &str, recursive: bool) -> std::io::Result<()> {
        if recursive {
            std::fs::remove_dir_all(self.chemin(path))
        } else {
            std::fs::remove_dir(self.chemin(path))
        }
    }
}
