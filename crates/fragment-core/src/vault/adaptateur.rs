// Port de app/src/ui/core/vault/DataAdapter.ts
//! Le contrat du coffre avec le disque. Les chemins sont relatifs à la racine du coffre,
//! séparés par `/`, sans `./` ni `..` (comme le DataAdapter d'Obsidian).
//!
//! ★ SYNCHRONE, comme `fragment_artefacts::fs::JournalFs` (dont `Stat` et `ListedFiles`
//!   reprennent les formes : un même adaptateur disque servira les deux). Le TS est `async`
//!   parce que le renderer d'Electron l'est ; ici `std::fs` l'est aussi.
//!
//! Pas de `Result` pour `exists` / `stat` : un chemin absent est une réponse (`false`,
//! `None`), pas une panne — c'est la tolérance du coffre, qui est « ce qu'on peut lire du
//! disque ».

use std::io;
use std::rc::Rc;

/// Ce que `stat` rend (`type` du TS : fichier ou dossier). Millisecondes depuis l'époque Unix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub is_folder: bool,
    pub ctime: i64,
    pub mtime: i64,
    pub size: u64,
}

/// Ce que `list` rend : des chemins COMPLETS (`<dossier>/<nom>`), pas des noms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListedFiles {
    pub files: Vec<String>,
    pub folders: Vec<String>,
}

/// `DataWriteOptions` d'Obsidian : dates à imposer à l'écriture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataWriteOptions {
    pub ctime: Option<i64>,
    pub mtime: Option<i64>,
}

/// Ce que la surveillance annonce du disque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Create,
    Modify,
    Delete,
}

/// Le rappel que le coffre branche sur son adaptateur (`onChange` du TS).
pub type OnChange = Rc<dyn Fn(ChangeType, &str)>;

pub trait DataAdapter {
    fn get_name(&self) -> String;

    // Lecture
    fn exists(&self, path: &str) -> bool;
    /// `None` si le chemin n'existe pas.
    fn stat(&self, path: &str) -> Option<Stat>;
    /// Une erreur si le dossier est illisible (le coffre le laisse alors vide).
    fn list(&self, path: &str) -> io::Result<ListedFiles>;
    fn read(&self, path: &str) -> io::Result<String>;
    fn read_binary(&self, path: &str) -> io::Result<Vec<u8>>;

    // Écriture
    fn write(&self, path: &str, data: &str, options: DataWriteOptions) -> io::Result<()>;
    fn write_binary(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()>;
    fn append(&self, path: &str, data: &str, options: DataWriteOptions) -> io::Result<()>;
    fn append_binary(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()>;

    // Arborescence
    /// Récursif côté disque (`create_dir_all`).
    fn mkdir(&self, path: &str) -> io::Result<()>;
    fn rename(&self, path: &str, new_path: &str) -> io::Result<()>;
    fn copy(&self, path: &str, new_path: &str) -> io::Result<()>;
    /// Suppression définitive d'un fichier.
    fn remove(&self, path: &str) -> io::Result<()>;
    /// Suppression d'un dossier ; non récursive, elle échoue si le dossier n'est pas vide.
    fn rmdir(&self, path: &str, recursive: bool) -> io::Result<()>;
    /// Corbeille de l'OS : `Ok(false)` si l'OS refuse (le coffre se replie sur la locale).
    fn trash_system(&self, path: &str) -> io::Result<bool>;
    /// `.trash/` du coffre.
    fn trash_local(&self, path: &str) -> io::Result<()>;

    // Ressources
    /// Une adresse que la PLATEFORME sait lire seule (image, vidéo, PDF), sans transporter
    /// les octets. Pure fabrique de chaîne. `cache_key` (le mtime, fourni par le coffre) est
    /// HORS API Obsidian : sans lui, une image modifiée sur le disque ne se rafraîchirait jamais.
    fn get_resource_path(&self, path: &str, cache_key: Option<i64>) -> String;

    // Poussée
    /// Le coffre branche ici son rappel (`None` pour le débrancher). Seul « champ » de
    /// l'interface TS ; en Rust, une méthode, pour que l'adaptateur reste partageable (`Rc`).
    fn set_on_change(&self, callback: Option<OnChange>);
}
