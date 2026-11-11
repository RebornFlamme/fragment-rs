// Port de app/src/ui/core/vault/vault.ts
//! Le `Vault` : l'arbre des fichiers du coffre, les opérations dessus et les événements.
//!
//! ★ L'arbre est une ARÈNE (`HashMap<FileId, Noeud>` + index par chemin), pas des objets
//!   qui se pointent : pas de `Rc<RefCell>`, pas de cycle parent/enfant. Les `TFile` /
//!   `TFolder` rendus sont des instantanés ; toute opération retrouve le nœud par son
//!   `FileId`, donc un instantané périmé (renommé depuis) reste valable.
//!
//! ★ Jamais d'emprunt de l'arène pendant un appel à l'adaptateur ou à un écouteur : un
//!   écouteur d'événement a le droit de rappeler le coffre (le journal d'artefacts le fait).
//!
//! ★ Les événements : `create`, `modify`, `delete`, `rename` parlent de l'ARBRE ; `raw` parle
//!   du DISQUE (un chemin brut, y compris dans `.fragment/` que l'arbre ne contient jamais).
//!   `delete` et `rename` portent leur origine (`user` : intention exprimée dans l'app ;
//!   `disk` : constat de la surveillance).

use std::collections::HashMap;
use std::io;
use std::rc::Rc;

use thiserror::Error;

use super::adaptateur::{ChangeType, DataAdapter, DataWriteOptions, ListedFiles};
use super::fichier::{
    FileHandle, FileId, FileStats, TAbstractFile, TFile, TFolder, VaultEventInfo, nom_de,
};
use crate::documents::EventOrigin;
use crate::events::{EventRef, Events};
use crate::resolve_link::{LinkFile, LinkVault};

/// Le dossier de configuration du coffre (`CONFIG_DIR` de shared/configDir.ts).
pub const CONFIG_DIR: &str = ".fragment";

/// Ce chemin est-il dans le dossier de configuration (ou est-ce lui) ?
pub fn dans_la_config(path: &str) -> bool {
    path == CONFIG_DIR || path.starts_with(&format!("{CONFIG_DIR}/"))
}

/// Les `throw` du Vault.
#[derive(Debug, Error)]
pub enum VaultError {
    #[error("existe déjà : {0}")]
    Existe(String),
    #[error("dossier parent inconnu du coffre : {0}")]
    ParentInconnu(String),
    #[error("{op} : écrit mais introuvable — {path}")]
    Introuvable { op: &'static str, path: String },
    #[error("ensure_folder : un fichier occupe {0}")]
    FichierOccupe(String),
    /// Le nœud n'est plus dans le coffre (supprimé, ou coffre rechargé depuis).
    #[error("nœud inconnu du coffre (supprimé ?)")]
    NoeudInconnu,
    #[error("un dossier n'est pas un fichier : {0}")]
    PasUnFichier(String),
    /// Ajout au TS : renommer ou supprimer la racine cassait l'arbre sans erreur claire.
    #[error("la racine du coffre ne peut être ni renommée ni supprimée")]
    Racine,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type VaultResult<T> = Result<T, VaultError>;

/// Un événement du coffre. Le `type` est [`VaultEventType`] ; le nom TS (`create`…) sert à
/// s'abonner par `events().on(...)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultEvent {
    /// Un fichier OU un dossier.
    Create { file: TAbstractFile },
    /// Un fichier seulement (un dossier n'a pas de contenu à modifier).
    Modify { file: TFile },
    /// L'instantané est pris AVANT le retrait : son chemin est celui d'avant.
    Delete {
        file: TAbstractFile,
        info: VaultEventInfo,
    },
    /// `file` porte déjà le nouveau chemin.
    Rename {
        file: TAbstractFile,
        old_path: String,
        info: VaultEventInfo,
    },
    /// Le disque a bougé, avant toute interprétation.
    Raw { path: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultEventType {
    Create,
    Modify,
    Delete,
    Rename,
    Raw,
}

impl VaultEventType {
    /// Le nom d'événement du TS.
    pub fn name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Modify => "modify",
            Self::Delete => "delete",
            Self::Rename => "rename",
            Self::Raw => "raw",
        }
    }
}

impl VaultEvent {
    pub fn event_type(&self) -> VaultEventType {
        match self {
            Self::Create { .. } => VaultEventType::Create,
            Self::Modify { .. } => VaultEventType::Modify,
            Self::Delete { .. } => VaultEventType::Delete,
            Self::Rename { .. } => VaultEventType::Rename,
            Self::Raw { .. } => VaultEventType::Raw,
        }
    }
}

enum Genre {
    Fichier(FileStats),
    Dossier(Vec<FileId>),
}

struct Noeud {
    path: String,
    parent: Option<FileId>,
    genre: Genre,
    /// Rang d'insertion : `Map` de JS garde l'ordre, et `resolve_link` s'en sert pour
    /// départager des homonymes. Un nœud déplacé repasse en fin (delete puis set).
    rang: u64,
}

struct Etat {
    noeuds: HashMap<FileId, Noeud>,
    par_chemin: HashMap<String, FileId>,
    racine: FileId,
    prochain_id: u32,
    prochain_rang: u64,
    /// `readCache` : chemin → (mtime, contenu). Périmé dès que le mtime change.
    cache_lecture: HashMap<String, (i64, String)>,
}

fn parent_de(path: &str) -> &str {
    // "a/b/note.md" → "a/b" · "note.md" → "" (la racine)
    match path.rfind('/') {
        Some(i) if i > 0 => &path[..i],
        _ => "",
    }
}

/// Les entrées commençant par `.` n'appartiennent pas au coffre visible (`.fragment`,
/// `.trash`, `.git`). ★ La règle vit dans le VAULT, pas dans l'adaptateur : savoir quoi
/// exclure est de la sémantique ; sinon `load` et la surveillance se contrediraient.
fn est_cache(path: &str) -> bool {
    nom_de(path).starts_with('.')
}

impl Etat {
    fn vide() -> Self {
        let mut e = Etat {
            noeuds: HashMap::new(),
            par_chemin: HashMap::new(),
            racine: FileId(0),
            prochain_id: 0,
            prochain_rang: 0,
            cache_lecture: HashMap::new(),
        };
        e.poser_racine();
        e
    }

    fn nouvel_id(&mut self) -> FileId {
        self.prochain_id += 1;
        FileId(self.prochain_id)
    }

    fn nouveau_rang(&mut self) -> u64 {
        self.prochain_rang += 1;
        self.prochain_rang
    }

    /// La racine : chemin vide, pas de parent (c'est `is_root`).
    fn poser_racine(&mut self) {
        let id = self.nouvel_id();
        let rang = self.nouveau_rang();
        self.noeuds.insert(
            id,
            Noeud {
                path: String::new(),
                parent: None,
                genre: Genre::Dossier(Vec::new()),
                rang,
            },
        );
        self.par_chemin.insert(String::new(), id);
        self.racine = id;
    }

    fn instantane(&self, id: FileId) -> Option<TAbstractFile> {
        let n = self.noeuds.get(&id)?;
        Some(match &n.genre {
            Genre::Fichier(stat) => TAbstractFile::File(TFile {
                id,
                path: n.path.clone(),
                parent: n.parent,
                stat: *stat,
            }),
            Genre::Dossier(enfants) => TAbstractFile::Folder(TFolder {
                id,
                path: n.path.clone(),
                parent: n.parent,
                children: enfants.clone(),
            }),
        })
    }

    fn est_dossier(&self, id: FileId) -> bool {
        matches!(
            self.noeuds.get(&id),
            Some(Noeud {
                genre: Genre::Dossier(_),
                ..
            })
        )
    }

    fn dossier_par_chemin(&self, path: &str) -> Option<FileId> {
        let id = *self.par_chemin.get(path)?;
        self.est_dossier(id).then_some(id)
    }

    /// `assertCreatable` : appelée AVANT d'écrire, pour ne pas désynchroniser arbre et disque.
    fn verifier_creable(&self, path: &str) -> VaultResult<FileId> {
        if self.par_chemin.contains_key(path) {
            return Err(VaultError::Existe(path.to_owned()));
        }
        let p = parent_de(path);
        self.dossier_par_chemin(p)
            .ok_or_else(|| VaultError::ParentInconnu(p.to_owned()))
    }

    /// `addToTree` : un NOUVEAU nœud, dont le parent doit déjà exister.
    fn ajouter(&mut self, path: &str, genre: Genre) -> VaultResult<FileId> {
        let parent = self
            .dossier_par_chemin(parent_de(path))
            .ok_or_else(|| VaultError::ParentInconnu(parent_de(path).to_owned()))?;
        let id = self.nouvel_id();
        let rang = self.nouveau_rang();
        self.noeuds.insert(
            id,
            Noeud {
                path: path.to_owned(),
                parent: Some(parent),
                genre,
                rang,
            },
        );
        self.enfants_de(parent).push(id);
        self.par_chemin.insert(path.to_owned(), id);
        Ok(id)
    }

    fn enfants_de(&mut self, dossier: FileId) -> &mut Vec<FileId> {
        match self.noeuds.get_mut(&dossier).map(|n| &mut n.genre) {
            Some(Genre::Dossier(v)) => v,
            // Un dossier connu de l'index l'est toujours de l'arène : cas inatteignable.
            _ => unreachable!("enfants_de : {dossier:?} n'est pas un dossier"),
        }
    }

    /// Les descendants, en pré-ordre (le nœud lui-même exclu).
    fn descendants(&self, id: FileId) -> Vec<FileId> {
        let mut out = Vec::new();
        self.descendre(id, &mut out);
        out
    }

    fn descendre(&self, id: FileId, out: &mut Vec<FileId>) {
        if let Some(Noeud {
            genre: Genre::Dossier(enfants),
            ..
        }) = self.noeuds.get(&id)
        {
            for &e in enfants {
                out.push(e);
                self.descendre(e, out);
            }
        }
    }

    /// `removeFromTree` : retire le nœud et tout ce qu'il contient (index, cache de lecture).
    fn retirer(&mut self, id: FileId) {
        for d in self.descendants(id).into_iter().rev() {
            self.oublier(d);
        }
        if let Some(parent) = self.noeuds.get(&id).and_then(|n| n.parent) {
            self.enfants_de(parent).retain(|&e| e != id);
        }
        self.oublier(id);
    }

    fn oublier(&mut self, id: FileId) {
        if let Some(n) = self.noeuds.remove(&id) {
            self.par_chemin.remove(&n.path);
            self.cache_lecture.remove(&n.path);
        }
    }

    /// `moveInTree` : les mêmes objets, de nouveaux chemins. Les descendants sont collectés
    /// AVANT de muter (leurs chemins dérivent de l'ancien chemin du parent).
    fn deplacer(&mut self, id: FileId, nouveau: &str) -> VaultResult<()> {
        let descendants = self.descendants(id);
        let ancien = match self.noeuds.get(&id) {
            Some(n) => n.path.clone(),
            None => return Err(VaultError::NoeudInconnu),
        };
        let nouveau_parent = self
            .dossier_par_chemin(parent_de(nouveau))
            .ok_or_else(|| VaultError::ParentInconnu(parent_de(nouveau).to_owned()))?;

        if let Some(vieux_parent) = self.noeuds.get(&id).and_then(|n| n.parent) {
            self.enfants_de(vieux_parent).retain(|&e| e != id);
        }
        self.par_chemin.remove(&ancien);
        self.cache_lecture.remove(&ancien);

        let rang = self.nouveau_rang();
        if let Some(n) = self.noeuds.get_mut(&id) {
            n.path = nouveau.to_owned();
            n.parent = Some(nouveau_parent);
            n.rang = rang;
        }
        self.enfants_de(nouveau_parent).push(id);
        self.par_chemin.insert(nouveau.to_owned(), id);

        for d in descendants {
            let rang = self.nouveau_rang();
            let Some(n) = self.noeuds.get_mut(&d) else {
                continue;
            };
            let refait = format!("{nouveau}{}", &n.path[ancien.len()..]);
            let vieux = std::mem::replace(&mut n.path, refait);
            n.rang = rang;
            let neuf = n.path.clone();
            self.par_chemin.remove(&vieux);
            self.cache_lecture.remove(&vieux);
            self.par_chemin.insert(neuf, d);
        }
        Ok(())
    }

    /// Tous les nœuds, dans l'ordre d'insertion.
    fn tous(&self) -> Vec<FileId> {
        let mut v: Vec<(u64, FileId)> = self.noeuds.iter().map(|(&id, n)| (n.rang, id)).collect();
        v.sort_unstable();
        v.into_iter().map(|(_, id)| id).collect()
    }
}

fn stats_de(s: &super::adaptateur::Stat) -> FileStats {
    FileStats {
        ctime: s.ctime,
        mtime: s.mtime,
        size: s.size,
    }
}

pub struct Vault {
    adapter: Rc<dyn DataAdapter>,
    events: Events,
    /// Le dossier de configuration (`Vault.configDir`).
    pub config_dir: String,
    etat: std::cell::RefCell<Etat>,
}

impl Vault {
    /// Branche le coffre sur son adaptateur : le rappel de surveillance y est posé (le TS
    /// remplit `adapter.onChange`). L'arbre n'est lu qu'au [`load`](Self::load).
    pub fn new(adapter: Rc<dyn DataAdapter>) -> Rc<Self> {
        Rc::new_cyclic(|faible: &std::rc::Weak<Vault>| {
            let f = faible.clone();
            adapter.set_on_change(Some(Rc::new(move |kind, path| {
                if let Some(v) = f.upgrade() {
                    v.on_change(kind, path);
                }
            })));
            Vault {
                adapter,
                events: Events::new(),
                config_dir: CONFIG_DIR.to_owned(),
                etat: std::cell::RefCell::new(Etat::vide()),
            }
        })
    }

    pub fn adapter(&self) -> &Rc<dyn DataAdapter> {
        &self.adapter
    }

    /// Le flux brut, pour `register_event` d'un composant.
    pub fn events(&self) -> &Events {
        &self.events
    }

    /// S'abonne à un genre d'événement (le `on` typé du TS).
    pub fn on<F>(&self, kind: VaultEventType, cb: F) -> EventRef
    where
        F: Fn(&VaultEvent) + 'static,
    {
        self.events.on(kind.name(), move |d| {
            if let Some(e) = d.downcast_ref::<VaultEvent>() {
                cb(e);
            }
        })
    }

    fn annoncer(&self, e: &VaultEvent) {
        self.events.trigger(e.event_type().name(), e);
    }

    // ── Chargement ─────────────────────────────────────────────────────────────

    /// Reconstruit l'arbre depuis le disque (descente, stats).
    ///
    /// ★ UN dossier illisible ne fait pas échouer le coffre : il reste dans l'arbre, vide
    ///   (le voir vide est un fait, le faire disparaître serait un mensonge), et un
    ///   avertissement est journalisé. Seule la racine illisible est une erreur. Le coffre
    ///   est ce qu'on peut lire du disque, pas une transaction tout-ou-rien.
    pub fn load(&self) -> VaultResult<()> {
        let racine = {
            let mut e = self.etat.borrow_mut();
            // ★ Les identifiants continuent de croître : un instantané d'avant le chargement
            //   ne peut pas désigner un nœud d'après.
            e.noeuds.clear();
            e.par_chemin.clear();
            e.cache_lecture.clear();
            e.poser_racine();
            e.racine
        };
        self.construire(racine, "")?;
        Ok(())
    }

    /// Un niveau de l'arbre, puis la descente : fichiers d'abord (stat les omet s'ils ont
    /// disparu entre `list` et `stat`), tous les sous-dossiers créés AVANT de descendre.
    fn construire(&self, dossier: FileId, chemin: &str) -> io::Result<()> {
        let ListedFiles { files, folders } = self.adapter.list(chemin)?;
        let avec_stats: Vec<(String, super::adaptateur::Stat)> = files
            .into_iter()
            .filter(|p| !est_cache(p))
            .filter_map(|p| self.adapter.stat(&p).map(|s| (p, s)))
            .collect();

        let mut sous = Vec::new();
        {
            let mut e = self.etat.borrow_mut();
            for (path, stat) in avec_stats {
                Self::ranger(&mut e, dossier, &path, Genre::Fichier(stats_de(&stat)));
            }
            for path in folders.into_iter().filter(|p| !est_cache(p)) {
                let id = Self::ranger(&mut e, dossier, &path, Genre::Dossier(Vec::new()));
                sous.push((id, path));
            }
        }
        for (id, path) in sous {
            if let Err(err) = self.construire(id, &path) {
                log::warn!("[vault] dossier illisible, laissé vide : {path} ({err})");
            }
        }
        Ok(())
    }

    /// Ajoute sous un parent connu, sans repasser par l'index des chemins du parent.
    fn ranger(e: &mut Etat, parent: FileId, path: &str, genre: Genre) -> FileId {
        let id = e.nouvel_id();
        let rang = e.nouveau_rang();
        e.noeuds.insert(
            id,
            Noeud {
                path: path.to_owned(),
                parent: Some(parent),
                genre,
                rang,
            },
        );
        e.enfants_de(parent).push(id);
        e.par_chemin.insert(path.to_owned(), id);
        id
    }

    // ── Surveillance : réconcilier les changements faits sur le disque ────────

    /// Appelée par l'adaptateur (la surveillance). Une panne ici ne remonte pas : elle est
    /// journalisée, comme le `try/catch` du TS.
    pub fn on_change(&self, kind: ChangeType, path: &str) {
        // ★ `raw` AVANT tout : il annonce le disque, pas l'arbre. C'est le seul canal par
        //   lequel un changement dans `.fragment/` est visible (le dossier de config n'est
        //   pas dans l'arbre et n'y sera jamais).
        self.annoncer(&VaultEvent::Raw {
            path: path.to_owned(),
        });

        // La config n'est pas une note : sans ce garde, chaque écriture de workspace.json
        // (débouncée, donc régulière) déclencherait une réconciliation inutile.
        if dans_la_config(path) {
            return;
        }

        match kind {
            ChangeType::Create => self.disque_cree(path),
            ChangeType::Modify => self.disque_modifie(path),
            ChangeType::Delete => {
                // Constaté sur le disque (git, synchro, autre outil) : pas une intention.
                // Ceux qui détruisent des données sur `delete` regardent l'origine.
                let id = self.etat.borrow().par_chemin.get(path).copied();
                if let Some(id) = id {
                    self.detacher_et_annoncer(id, EventOrigin::Disk);
                }
            }
        }
    }

    fn disque_cree(&self, path: &str) {
        // Déjà connu : c'est l'écho de notre propre `create()`.
        if self.etat.borrow().par_chemin.contains_key(path) {
            return;
        }
        if self
            .etat
            .borrow()
            .dossier_par_chemin(parent_de(path))
            .is_none()
        {
            // Création en cascade dont le dossier parent ne nous est pas encore parvenu :
            // l'événement du parent arrivera, et un rechargement rattraperait de toute façon.
            log::warn!("[vault] create ignoré, parent inconnu : {path}");
            return;
        }
        let Some(stat) = self.adapter.stat(path) else {
            return; // déjà reparti
        };
        let genre = if stat.is_folder {
            Genre::Dossier(Vec::new())
        } else {
            Genre::Fichier(stats_de(&stat))
        };
        let instantane = {
            let mut e = self.etat.borrow_mut();
            match e.ajouter(path, genre) {
                Ok(id) => e.instantane(id),
                Err(err) => {
                    log::error!("[vault] on_change(create, {path}) a échoué : {err}");
                    None
                }
            }
        };
        if let Some(file) = instantane {
            self.annoncer(&VaultEvent::Create { file });
        }
    }

    fn disque_modifie(&self, path: &str) {
        let Some(file) = self.get_file_by_path(path) else {
            return; // inconnu, ou c'est un dossier
        };
        let Some(fresh) = self.adapter.stat(path) else {
            return;
        };
        // Ici il faudra enrichir avec le hash du contenu quand le MetadataCache existera.
        if fresh.mtime == file.stat.mtime && fresh.size == file.stat.size {
            return;
        }
        if let Some(f) = self.maj_stat(file.id, &fresh) {
            self.annoncer(&VaultEvent::Modify { file: f });
        }
    }

    // ── Lecture de l'arbre ─────────────────────────────────────────────────────

    pub fn get_name(&self) -> String {
        self.adapter.get_name()
    }

    pub fn get_root(&self) -> TFolder {
        let e = self.etat.borrow();
        match e.instantane(e.racine) {
            Some(TAbstractFile::Folder(f)) => f,
            _ => unreachable!("la racine est toujours un dossier"),
        }
    }

    pub fn get_abstract_file_by_path(&self, path: &str) -> Option<TAbstractFile> {
        let e = self.etat.borrow();
        e.instantane(*e.par_chemin.get(path)?)
    }

    /// `None` si le chemin désigne un dossier — pas seulement s'il est absent.
    pub fn get_file_by_path(&self, path: &str) -> Option<TFile> {
        match self.get_abstract_file_by_path(path)? {
            TAbstractFile::File(f) => Some(f),
            TAbstractFile::Folder(_) => None,
        }
    }

    pub fn get_folder_by_path(&self, path: &str) -> Option<TFolder> {
        match self.get_abstract_file_by_path(path)? {
            TAbstractFile::Folder(f) => Some(f),
            TAbstractFile::File(_) => None,
        }
    }

    /// Le nœud d'un identifiant (l'instantané À JOUR d'un instantané périmé).
    pub fn get_by_id(&self, id: FileId) -> Option<TAbstractFile> {
        self.etat.borrow().instantane(id)
    }

    /// Les enfants d'un dossier, dans l'ordre du coffre.
    pub fn children(&self, folder: &impl FileHandle) -> Vec<TAbstractFile> {
        let e = self.etat.borrow();
        match e.noeuds.get(&folder.id()).map(|n| &n.genre) {
            Some(Genre::Dossier(ids)) => ids.iter().filter_map(|&i| e.instantane(i)).collect(),
            _ => Vec::new(),
        }
    }

    /// Tout l'arbre, racine comprise, dans l'ordre d'insertion.
    pub fn get_all_loaded_files(&self) -> Vec<TAbstractFile> {
        let e = self.etat.borrow();
        e.tous()
            .into_iter()
            .filter_map(|id| e.instantane(id))
            .collect()
    }

    pub fn get_files(&self) -> Vec<TFile> {
        self.get_all_loaded_files()
            .into_iter()
            .filter_map(|f| match f {
                TAbstractFile::File(f) => Some(f),
                TAbstractFile::Folder(_) => None,
            })
            .collect()
    }

    pub fn get_markdown_files(&self) -> Vec<TFile> {
        self.get_files()
            .into_iter()
            .filter(|f| f.extension() == "md")
            .collect()
    }

    /// ★ La racine EST dans l'index : `include_root` doit donc EXCLURE quand il est faux,
    ///   pas ajouter quand il est vrai — sinon on la dupliquerait.
    pub fn get_all_folders(&self, include_root: bool) -> Vec<TFolder> {
        self.get_all_loaded_files()
            .into_iter()
            .filter_map(|f| match f {
                TAbstractFile::Folder(f) if include_root || !f.is_root() => Some(f),
                _ => None,
            })
            .collect()
    }

    /// Parcours en pré-ordre ; le nœud de départ est inclus (comportement d'Obsidian).
    pub fn recurse_children<F: FnMut(&TAbstractFile)>(&self, root: &impl FileHandle, mut cb: F) {
        let mut ids = Vec::new();
        {
            let e = self.etat.borrow();
            if e.noeuds.contains_key(&root.id()) {
                ids.push(root.id());
                e.descendre(root.id(), &mut ids);
            }
        }
        for id in ids {
            if let Some(n) = self.get_by_id(id) {
                cb(&n);
            }
        }
    }

    // ── Lecture du contenu ─────────────────────────────────────────────────────

    fn chemin_de(&self, h: &impl FileHandle) -> VaultResult<String> {
        self.etat
            .borrow()
            .noeuds
            .get(&h.id())
            .map(|n| n.path.clone())
            .ok_or(VaultError::NoeudInconnu)
    }

    fn fichier_de(&self, h: &impl FileHandle) -> VaultResult<TFile> {
        match self.get_by_id(h.id()).ok_or(VaultError::NoeudInconnu)? {
            TAbstractFile::File(f) => Ok(f),
            TAbstractFile::Folder(f) => Err(VaultError::PasUnFichier(f.path)),
        }
    }

    pub fn read(&self, file: &TFile) -> VaultResult<String> {
        Ok(self.adapter.read(&self.chemin_de(file)?)?)
    }

    /// Lecture avec cache, périmé quand le `mtime` du fichier change.
    pub fn cached_read(&self, file: &TFile) -> VaultResult<String> {
        let f = self.fichier_de(file)?;
        if let Some((mtime, data)) = self.etat.borrow().cache_lecture.get(&f.path)
            && *mtime == f.stat.mtime
        {
            return Ok(data.clone());
        }
        let data = self.adapter.read(&f.path)?;
        self.etat
            .borrow_mut()
            .cache_lecture
            .insert(f.path, (f.stat.mtime, data.clone()));
        Ok(data)
    }

    pub fn read_binary(&self, file: &TFile) -> VaultResult<Vec<u8>> {
        Ok(self.adapter.read_binary(&self.chemin_de(file)?)?)
    }

    /// L'adresse d'un fichier pour la plateforme (image, audio, PDF…). Le coffre est le
    /// seul à tenir le `mtime` : c'est lui qui fournit le cache-buster ; l'adaptateur décide
    /// de ce qu'il en fait.
    pub fn get_resource_path(&self, file: &TFile) -> VaultResult<String> {
        let f = self.fichier_de(file)?;
        Ok(self.adapter.get_resource_path(&f.path, Some(f.stat.mtime)))
    }

    // ── Écriture ───────────────────────────────────────────────────────────────

    /// Met à jour le `stat` d'un fichier de l'arbre ; rend l'instantané frais.
    fn maj_stat(&self, id: FileId, stat: &super::adaptateur::Stat) -> Option<TFile> {
        let mut e = self.etat.borrow_mut();
        if let Some(Noeud {
            genre: Genre::Fichier(s),
            ..
        }) = e.noeuds.get_mut(&id)
        {
            *s = stats_de(stat);
        }
        match e.instantane(id) {
            Some(TAbstractFile::File(f)) => Some(f),
            _ => None,
        }
    }

    /// Appelée après chaque écriture ; rend l'instantané frais.
    fn rafraichir(&self, file: &TFile) -> VaultResult<TFile> {
        let path = self.chemin_de(file)?;
        if let Some(fresh) = self.adapter.stat(&path)
            && let Some(f) = self.maj_stat(file.id, &fresh)
        {
            return Ok(f);
        }
        self.fichier_de(file)
    }

    /// Fichier neuf : écrit, relit le stat, range dans l'arbre, annonce `create`.
    fn creer(
        &self,
        op: &'static str,
        path: &str,
        ecrire: impl FnOnce(&dyn DataAdapter) -> io::Result<()>,
    ) -> VaultResult<TFile> {
        self.etat.borrow().verifier_creable(path)?;
        ecrire(self.adapter.as_ref())?;
        let stat = self
            .adapter
            .stat(path)
            .ok_or_else(|| VaultError::Introuvable {
                op,
                path: path.to_owned(),
            })?;
        let file = {
            let mut e = self.etat.borrow_mut();
            let id = e.ajouter(path, Genre::Fichier(stats_de(&stat)))?;
            match e.instantane(id) {
                Some(TAbstractFile::File(f)) => f,
                _ => unreachable!("un fichier vient d'être ajouté"),
            }
        };
        self.annoncer(&VaultEvent::Create {
            file: file.clone().into(),
        });
        Ok(file)
    }

    pub fn create(&self, path: &str, data: &str, options: DataWriteOptions) -> VaultResult<TFile> {
        self.creer("create", path, |a| a.write(path, data, options))
    }

    pub fn create_binary(
        &self,
        path: &str,
        data: &[u8],
        options: DataWriteOptions,
    ) -> VaultResult<TFile> {
        self.creer("create_binary", path, |a| {
            a.write_binary(path, data, options)
        })
    }

    /// ★ L'adaptateur fait un `mkdir` récursif côté disque, mais l'arbre n'accepte qu'un
    ///   ajout dont le parent existe DÉJÀ : la vérification passe avant l'écriture, sinon
    ///   `create_folder("a/b/c")` avec `a` inconnu créerait trois dossiers sur le disque et
    ///   zéro dans l'arbre.
    pub fn create_folder(&self, path: &str) -> VaultResult<TFolder> {
        self.etat.borrow().verifier_creable(path)?;
        self.adapter.mkdir(path)?;
        let folder = {
            let mut e = self.etat.borrow_mut();
            let id = e.ajouter(path, Genre::Dossier(Vec::new()))?;
            match e.instantane(id) {
                Some(TAbstractFile::Folder(f)) => f,
                _ => unreachable!("un dossier vient d'être ajouté"),
            }
        };
        self.annoncer(&VaultEvent::Create {
            file: folder.clone().into(),
        });
        Ok(folder)
    }

    /// Garantit que le dossier existe, en créant chaque niveau manquant (`create_folder`,
    /// lui, exige un parent connu). `""` (la racine) existe toujours. Échoue si un FICHIER
    /// occupe l'un des chemins.
    pub fn ensure_folder(&self, path: &str) -> VaultResult<TFolder> {
        let mut courant = self.get_root();
        if path.is_empty() {
            return Ok(courant);
        }
        let parts: Vec<&str> = path.split('/').collect();
        for i in 1..=parts.len() {
            let p = parts[..i].join("/");
            match self.get_abstract_file_by_path(&p) {
                Some(TAbstractFile::Folder(f)) => courant = f,
                Some(TAbstractFile::File(_)) => return Err(VaultError::FichierOccupe(p)),
                None => courant = self.create_folder(&p)?,
            }
        }
        Ok(courant)
    }

    /// Le premier chemin LIBRE `base.ext`, puis `base 1.ext`, `base 2.ext`… dans `folder`
    /// (`""` = racine) : l'anti-collision d'Obsidian. `ext` sans point ; vide = sans extension.
    pub fn get_available_path(&self, folder: &str, base: &str, ext: &str) -> String {
        let prefix = if folder.is_empty() {
            String::new()
        } else {
            format!("{folder}/")
        };
        let suffix = if ext.is_empty() {
            String::new()
        } else {
            format!(".{ext}")
        };
        let e = self.etat.borrow();
        let mut path = format!("{prefix}{base}{suffix}");
        let mut n = 0;
        while e.par_chemin.contains_key(&path) {
            n += 1;
            path = format!("{prefix}{base} {n}{suffix}");
        }
        path
    }

    /// Ecrit puis annonce `modify` (le `stat` est relu après l'écriture).
    fn modifier(
        &self,
        file: &TFile,
        ecrire: impl FnOnce(&dyn DataAdapter, &str) -> io::Result<()>,
    ) -> VaultResult<TFile> {
        let path = self.chemin_de(file)?;
        ecrire(self.adapter.as_ref(), &path)?;
        let f = self.rafraichir(file)?;
        self.annoncer(&VaultEvent::Modify { file: f.clone() });
        Ok(f)
    }

    pub fn modify(&self, file: &TFile, data: &str, options: DataWriteOptions) -> VaultResult<()> {
        self.modifier(file, |a, p| a.write(p, data, options))
            .map(|_| ())
    }

    pub fn modify_binary(
        &self,
        file: &TFile,
        data: &[u8],
        options: DataWriteOptions,
    ) -> VaultResult<()> {
        self.modifier(file, |a, p| a.write_binary(p, data, options))
            .map(|_| ())
    }

    pub fn append(&self, file: &TFile, data: &str, options: DataWriteOptions) -> VaultResult<()> {
        self.modifier(file, |a, p| a.append(p, data, options))
            .map(|_| ())
    }

    pub fn append_binary(
        &self,
        file: &TFile,
        data: &[u8],
        options: DataWriteOptions,
    ) -> VaultResult<()> {
        self.modifier(file, |a, p| a.append_binary(p, data, options))
            .map(|_| ())
    }

    /// Lecture, transformation, écriture : le contenu rendu est le nouveau.
    ///
    /// ★ Le TS chaîne les `process` d'un même chemin dans une file de promesses, pour que
    ///   deux plugins ne s'écrasent pas. Ici tout est synchrone sur le fil de l'app : deux
    ///   `process` ne peuvent pas s'entrelacer, la file n'a plus d'objet.
    pub fn process<F: FnOnce(&str) -> String>(
        &self,
        file: &TFile,
        f: F,
        options: DataWriteOptions,
    ) -> VaultResult<String> {
        let path = self.chemin_de(file)?;
        let data = self.adapter.read(&path)?;
        let next = f(&data);
        self.adapter.write(&path, &next, options)?;
        let fresh = self.rafraichir(file)?;
        self.annoncer(&VaultEvent::Modify { file: fresh });
        Ok(next)
    }

    // ── Suppression, corbeille, renommage, copie ──────────────────────────────

    /// `force` : un dossier non vide est supprimé avec son contenu.
    pub fn delete(&self, file: &impl FileHandle, force: bool) -> VaultResult<()> {
        let (path, dossier) = self.noeud_modifiable(file)?;
        if dossier {
            self.adapter.rmdir(&path, force)?;
        } else {
            self.adapter.remove(&path)?;
        }
        self.detacher_et_annoncer(file.id(), EventOrigin::User);
        Ok(())
    }

    /// `system` : corbeille de l'OS ; sinon `.trash/` du coffre. Repli sur la locale si l'OS
    /// refuse (volume réseau, corbeille désactivée) : mieux vaut un fichier récupérable dans
    /// `.trash/` qu'un échec.
    pub fn trash(&self, file: &impl FileHandle, system: bool) -> VaultResult<()> {
        let (path, _) = self.noeud_modifiable(file)?;
        if system {
            if !self.adapter.trash_system(&path)? {
                self.adapter.trash_local(&path)?;
            }
        } else {
            self.adapter.trash_local(&path)?;
        }
        self.detacher_et_annoncer(file.id(), EventOrigin::User);
        Ok(())
    }

    /// Le chemin et la nature d'un nœud qu'on a le droit de détruire ou déplacer (pas la racine).
    fn noeud_modifiable(&self, h: &impl FileHandle) -> VaultResult<(String, bool)> {
        let e = self.etat.borrow();
        let n = e.noeuds.get(&h.id()).ok_or(VaultError::NoeudInconnu)?;
        if n.parent.is_none() {
            return Err(VaultError::Racine);
        }
        Ok((n.path.clone(), matches!(n.genre, Genre::Dossier(_))))
    }

    /// Retrait de l'arbre puis événements, FEUILLES D'ABORD : les descendants en ordre
    /// inverse du pré-ordre, puis le nœud. Commun à `delete`, `trash` et au constat disque.
    fn detacher_et_annoncer(&self, id: FileId, origin: EventOrigin) {
        let info = VaultEventInfo { origin };
        let (condamnes, fichier) = {
            let mut e = self.etat.borrow_mut();
            let Some(fichier) = e.instantane(id) else {
                return;
            };
            let mut condamnes: Vec<TAbstractFile> = e
                .descendants(id)
                .into_iter()
                .filter_map(|d| e.instantane(d))
                .collect();
            condamnes.reverse();
            e.retirer(id);
            (condamnes, fichier)
        };
        for file in condamnes {
            self.annoncer(&VaultEvent::Delete { file, info });
        }
        self.annoncer(&VaultEvent::Delete {
            file: fichier,
            info,
        });
    }

    /// Renomme ou déplace. Un dossier renommé déplace ses enfants (mêmes nœuds, nouveaux
    /// chemins).
    ///
    /// ★ UN SEUL événement `rename`, pour le nœud renommé (comme le TS) : ses enfants ne
    ///   reçoivent rien. C'est `DocumentEvent::Rename { folder: true }` qui dit aux
    ///   abonnés que tout ce qui est sous l'ancien préfixe a suivi. À l'inverse de `delete`,
    ///   qui annonce chaque descendant.
    pub fn rename(&self, file: &impl FileHandle, new_path: &str) -> VaultResult<()> {
        let (old_path, _) = self.noeud_modifiable(file)?;
        self.etat.borrow().verifier_creable(new_path)?;
        self.adapter.rename(&old_path, new_path)?;
        let instantane = {
            let mut e = self.etat.borrow_mut();
            e.deplacer(file.id(), new_path)?;
            e.instantane(file.id())
        };
        if let Some(file) = instantane {
            self.annoncer(&VaultEvent::Rename {
                file,
                old_path,
                info: VaultEventInfo {
                    origin: EventOrigin::User,
                },
            });
        }
        Ok(())
    }

    /// Copie un fichier ou un dossier ; rend la copie. Les enfants d'un dossier copié sont
    /// relus du disque sans événement : seul le dossier annonce `create`.
    pub fn copy(&self, file: &impl FileHandle, new_path: &str) -> VaultResult<TAbstractFile> {
        let path = self.chemin_de(file)?;
        let dossier = self.etat.borrow().est_dossier(file.id());
        self.etat.borrow().verifier_creable(new_path)?;
        self.adapter.copy(&path, new_path)?;

        if dossier {
            let id = self
                .etat
                .borrow_mut()
                .ajouter(new_path, Genre::Dossier(Vec::new()))?;
            self.construire(id, new_path)?;
            let copie = self.get_by_id(id).ok_or(VaultError::NoeudInconnu)?;
            self.annoncer(&VaultEvent::Create {
                file: copie.clone(),
            });
            return Ok(copie);
        }

        let stat = self
            .adapter
            .stat(new_path)
            .ok_or_else(|| VaultError::Introuvable {
                op: "copy",
                path: new_path.to_owned(),
            })?;
        let copie: TAbstractFile = {
            let mut e = self.etat.borrow_mut();
            let id = e.ajouter(new_path, Genre::Fichier(stats_de(&stat)))?;
            e.instantane(id).ok_or(VaultError::NoeudInconnu)?
        };
        self.annoncer(&VaultEvent::Create {
            file: copie.clone(),
        });
        Ok(copie)
    }
}

// ── resolve_link : le coffre comme `LinkVault` ─────────────────────────────────

impl LinkFile for TFile {
    fn path(&self) -> &str {
        &self.path
    }
    fn name(&self) -> &str {
        TFile::name(self)
    }
    fn basename(&self) -> &str {
        TFile::basename(self)
    }
}

impl LinkVault for Vault {
    type File = TFile;
    fn get_file_by_path(&self, path: &str) -> Option<TFile> {
        Vault::get_file_by_path(self, path)
    }
    fn get_files(&self) -> Vec<TFile> {
        Vault::get_files(self)
    }
}
