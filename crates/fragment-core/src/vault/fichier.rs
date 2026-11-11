// Port de app/src/ui/core/vault/TAbstractFile.ts, TFile.ts, TFolder.ts
//! Les nœuds de l'arbre du coffre.
//!
//! ★ Le TS donne des objets MUTABLES : une vue garde « son » `TFile` et le voit changer de
//!   chemin au renommage. En Rust, sans `Rc<RefCell>`, l'arbre vit dans une arène du
//!   [`Vault`](super::Vault) et ces structs en sont des INSTANTANÉS : ce qui reste stable
//!   est l'identité ([`FileId`]). Toute opération du coffre retrouve le nœud par son
//!   identifiant, jamais par le chemin de l'instantané — un `TFile` périmé (renommé depuis)
//!   désigne donc toujours le bon fichier.

use crate::documents::EventOrigin;

/// L'identité d'un nœud, stable tant que le coffre vit (renommage compris).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub(crate) u32);

/// Ce que les opérations du coffre acceptent comme « un nœud » : `TFile`, `TFolder`
/// ou `TAbstractFile`.
pub trait FileHandle {
    fn id(&self) -> FileId;
}

/// `FileStats` : date de création, de modification (ms depuis l'époque Unix), taille.
///
/// ★ Pas de champ `type` : être un `TFile` le dit déjà (le `Stat` de l'adaptateur, lui,
///   l'annonce parce qu'il ne manipule que des chemins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileStats {
    pub ctime: i64,
    pub mtime: i64,
    pub size: u64,
}

/// `"dossier/note.md"` → name `note.md`, basename `note`, extension `md` ;
/// `"README"` → `README` / `README` / `` ; `".gitignore"` → `.gitignore` / `.gitignore` / ``.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TFile {
    pub id: FileId,
    pub path: String,
    pub parent: Option<FileId>,
    pub stat: FileStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TFolder {
    pub id: FileId,
    pub path: String,
    pub parent: Option<FileId>,
    /// Les enfants, dans l'ordre du coffre (fichiers puis sous-dossiers au chargement).
    pub children: Vec<FileId>,
}

/// Un fichier ou un dossier (la classe abstraite `TAbstractFile`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TAbstractFile {
    File(TFile),
    Folder(TFolder),
}

/// Le dernier segment d'un chemin.
pub(crate) fn nom_de(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

impl TFile {
    pub fn name(&self) -> &str {
        nom_de(&self.path)
    }

    /// Sans le point ; vide si le nom n'a pas d'extension (un point en tête n'en est pas un).
    pub fn extension(&self) -> &str {
        let n = self.name();
        match n.rfind('.') {
            Some(i) if i > 0 => &n[i + 1..],
            _ => "",
        }
    }

    pub fn basename(&self) -> &str {
        let n = self.name();
        match n.rfind('.') {
            Some(i) if i > 0 => &n[..i],
            _ => n,
        }
    }
}

impl TFolder {
    pub fn name(&self) -> &str {
        nom_de(&self.path)
    }

    /// La racine est le seul dossier sans parent.
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}

impl TAbstractFile {
    pub fn path(&self) -> &str {
        match self {
            Self::File(f) => &f.path,
            Self::Folder(f) => &f.path,
        }
    }

    pub fn name(&self) -> &str {
        nom_de(self.path())
    }

    pub fn parent(&self) -> Option<FileId> {
        match self {
            Self::File(f) => f.parent,
            Self::Folder(f) => f.parent,
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, Self::Folder(_))
    }

    pub fn as_file(&self) -> Option<&TFile> {
        match self {
            Self::File(f) => Some(f),
            Self::Folder(_) => None,
        }
    }

    pub fn as_folder(&self) -> Option<&TFolder> {
        match self {
            Self::Folder(f) => Some(f),
            Self::File(_) => None,
        }
    }
}

impl FileHandle for TFile {
    fn id(&self) -> FileId {
        self.id
    }
}
impl FileHandle for TFolder {
    fn id(&self) -> FileId {
        self.id
    }
}
impl FileHandle for TAbstractFile {
    fn id(&self) -> FileId {
        match self {
            Self::File(f) => f.id,
            Self::Folder(f) => f.id,
        }
    }
}
impl FileHandle for FileId {
    fn id(&self) -> FileId {
        *self
    }
}

impl From<TFile> for TAbstractFile {
    fn from(f: TFile) -> Self {
        Self::File(f)
    }
}
impl From<TFolder> for TAbstractFile {
    fn from(f: TFolder) -> Self {
        Self::Folder(f)
    }
}

/// D'où vient un `delete` / `rename` : `User` = une intention exprimée dans l'app
/// (`delete()`, `trash()`, `rename()`), `Disk` = une disparition constatée par la
/// surveillance. Le store d'artefacts ne détruit que sur une intention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultEventInfo {
    pub origin: EventOrigin,
}
