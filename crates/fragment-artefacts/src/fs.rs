// Port de app/src/ui/core/artefacts/objects.ts (JournalAdapter) et device.ts
// (LocalFsAdapter, MemoryAdapter)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le système de fichiers vu par le journal : le sous-ensemble du DataAdapter du
//  coffre dont il a besoin (`JournalFs`), avec deux implémentations — `std::fs`
//  enraciné dans un dossier (le coffre, ou userData pour le journal local) et la
//  mémoire (les tests, le journal local sans userData).
//
//  Les chemins sont relatifs à la racine, séparés par `/`, sans `./` ni `..`,
//  comme ceux du DataAdapter d'Obsidian.
//
//  ★ Synchrone : le TS est `async` parce que le DataAdapter d'Electron l'est. En
//    Rust, `std::fs` est synchrone et le manager tourne sur le fil de l'app ; les
//    erreurs sont des `io::Error` (le TS lève, l'appelant attrape).
// ═══════════════════════════════════════════════════════════════════════════

use crate::clock::Now;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Ce qu'un `stat` rend. `type` du DataAdapter : fichier ou dossier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub is_folder: bool,
    /// Millisecondes depuis l'époque Unix (`ctimeMs`, `mtimeMs`).
    pub ctime: i64,
    pub mtime: i64,
    pub size: u64,
}

/// Ce qu'un `list` rend : des chemins complets (`<dossier>/<nom>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listed {
    pub files: Vec<String>,
    pub folders: Vec<String>,
}

/// Le contrat du journal avec le disque (`JournalAdapter`).
pub trait JournalFs {
    fn exists(&self, path: &str) -> bool;
    /// `None` si le chemin n'existe pas.
    fn stat(&self, path: &str) -> Option<Stat>;
    /// Un dossier absent se liste vide.
    fn list(&self, path: &str) -> Listed;
    fn read(&self, path: &str) -> io::Result<String>;
    /// `mtime` : pose la date de modification (rafraîchir un objet, §14.5).
    fn write(&self, path: &str, data: &str, mtime: Option<i64>) -> io::Result<()>;
    fn append(&self, path: &str, data: &str) -> io::Result<()>;
    fn mkdir(&self, path: &str) -> io::Result<()>;
    fn rename(&self, path: &str, new_path: &str) -> io::Result<()>;
    /// `force` : un chemin absent n'est pas une erreur.
    fn remove(&self, path: &str) -> io::Result<()>;
}

/// Un système de fichiers partagé entre le manager, ses lecteurs et ses écrivains.
pub type SharedFs = Rc<dyn JournalFs>;

// ── std::fs ────────────────────────────────────────────────────────────────────

/// Un adaptateur minimal sur `std::fs`, enraciné dans un dossier (le coffre, ou
/// userData hors du coffre). Mêmes chemins relatifs à `/` que le DataAdapter ;
/// aucun watcher.
pub struct StdFs {
    root: PathBuf,
}

impl StdFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        StdFs { root: root.into() }
    }

    /// Le chemin absolu d'un chemin relatif.
    pub fn absolute(&self, path: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in path.split('/').filter(|s| !s.is_empty()) {
            p.push(part);
        }
        p
    }

    fn ms(t: io::Result<std::time::SystemTime>) -> i64 {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

impl JournalFs for StdFs {
    fn exists(&self, path: &str) -> bool {
        self.absolute(path).exists()
    }

    fn stat(&self, path: &str) -> Option<Stat> {
        let m = std::fs::metadata(self.absolute(path)).ok()?;
        Some(Stat {
            is_folder: m.is_dir(),
            ctime: Self::ms(m.created()),
            mtime: Self::ms(m.modified()),
            size: m.len(),
        })
    }

    fn list(&self, path: &str) -> Listed {
        let base = path.trim_end_matches('/');
        let mut out = Listed::default();
        let Ok(entries) = std::fs::read_dir(self.absolute(path)) else {
            return out; // dossier absent : vide
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let full = format!("{base}/{name}");
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                out.folders.push(full);
            } else {
                out.files.push(full);
            }
        }
        out.files.sort();
        out.folders.sort();
        out
    }

    fn read(&self, path: &str) -> io::Result<String> {
        std::fs::read_to_string(self.absolute(path))
    }

    fn write(&self, path: &str, data: &str, mtime: Option<i64>) -> io::Result<()> {
        let abs = self.absolute(path);
        std::fs::write(&abs, data)?;
        if let Some(ms) = mtime {
            let t = if ms >= 0 {
                std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64)
            } else {
                std::time::UNIX_EPOCH
            };
            std::fs::File::options()
                .write(true)
                .open(&abs)?
                .set_modified(t)?;
        }
        Ok(())
    }

    fn append(&self, path: &str, data: &str) -> io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::options()
            .append(true)
            .create(true)
            .open(self.absolute(path))?;
        f.write_all(data.as_bytes())
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        std::fs::create_dir_all(self.absolute(path))
    }

    fn rename(&self, path: &str, new_path: &str) -> io::Result<()> {
        std::fs::rename(self.absolute(path), self.absolute(new_path))
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        let abs = self.absolute(path);
        let r = if abs.is_dir() {
            std::fs::remove_dir_all(&abs)
        } else {
            std::fs::remove_file(&abs)
        };
        match r {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}

// ── mémoire ────────────────────────────────────────────────────────────────────

/// Un adaptateur en mémoire : le journal local sans userData, et les tests. Même
/// contrat que le précédent. Les dossiers n'existent que par leurs fichiers
/// (`mkdir` ne fait rien, `exists(dossier)` regarde les préfixes).
pub struct MemoryFs {
    files: RefCell<BTreeMap<String, String>>,
    mtimes: RefCell<BTreeMap<String, i64>>,
    now: Now,
    /// Combien d'appels `read` (diagnostic des tests).
    reads: Cell<usize>,
}

impl MemoryFs {
    pub fn new(now: Now) -> Self {
        MemoryFs {
            files: RefCell::new(BTreeMap::new()),
            mtimes: RefCell::new(BTreeMap::new()),
            now,
            reads: Cell::new(0),
        }
    }

    pub fn reads(&self) -> usize {
        self.reads.get()
    }

    /// Le contenu d'un fichier (les tests regardent le disque).
    pub fn get(&self, path: &str) -> Option<String> {
        self.files.borrow().get(path).cloned()
    }

    pub fn has(&self, path: &str) -> bool {
        self.files.borrow().contains_key(path)
    }

    /// Tous les chemins, triés.
    pub fn keys(&self) -> Vec<String> {
        self.files.borrow().keys().cloned().collect()
    }

    /// Pose un fichier tel quel, sans toucher au mtime (les tests fabriquent des fixtures).
    pub fn set(&self, path: &str, data: &str) {
        self.files
            .borrow_mut()
            .insert(path.to_string(), data.to_string());
        self.mtimes
            .borrow_mut()
            .entry(path.to_string())
            .or_insert((self.now)());
    }

    pub fn mtime(&self, path: &str) -> Option<i64> {
        self.mtimes.borrow().get(path).copied()
    }
}

impl JournalFs for MemoryFs {
    fn exists(&self, path: &str) -> bool {
        let files = self.files.borrow();
        if files.contains_key(path) {
            return true;
        }
        let prefix = format!("{path}/");
        files.keys().any(|k| k.starts_with(&prefix))
    }

    fn stat(&self, path: &str) -> Option<Stat> {
        let files = self.files.borrow();
        let v = files.get(path)?;
        let mtime = self.mtimes.borrow().get(path).copied().unwrap_or(0);
        Some(Stat {
            is_folder: false,
            ctime: mtime,
            mtime,
            size: v.len() as u64,
        })
    }

    fn list(&self, dir: &str) -> Listed {
        let prefix = if dir.ends_with('/') {
            dir.to_string()
        } else {
            format!("{dir}/")
        };
        let mut out = Listed::default();
        let mut folders = std::collections::BTreeSet::new();
        for key in self.files.borrow().keys() {
            let Some(rest) = key.strip_prefix(&prefix) else {
                continue;
            };
            match rest.find('/') {
                None => out.files.push(key.clone()),
                Some(i) => {
                    folders.insert(format!("{prefix}{}", &rest[..i]));
                }
            }
        }
        out.folders = folders.into_iter().collect();
        out
    }

    fn read(&self, path: &str) -> io::Result<String> {
        match self.files.borrow().get(path) {
            Some(v) => {
                self.reads.set(self.reads.get() + 1);
                Ok(v.clone())
            }
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("ENOENT {path}"),
            )),
        }
    }

    fn write(&self, path: &str, data: &str, mtime: Option<i64>) -> io::Result<()> {
        self.files
            .borrow_mut()
            .insert(path.to_string(), data.to_string());
        self.mtimes
            .borrow_mut()
            .insert(path.to_string(), mtime.unwrap_or_else(|| (self.now)()));
        Ok(())
    }

    fn append(&self, path: &str, data: &str) -> io::Result<()> {
        self.files
            .borrow_mut()
            .entry(path.to_string())
            .or_default()
            .push_str(data);
        self.mtimes
            .borrow_mut()
            .insert(path.to_string(), (self.now)());
        Ok(())
    }

    fn mkdir(&self, _path: &str) -> io::Result<()> {
        Ok(())
    }

    fn rename(&self, path: &str, new_path: &str) -> io::Result<()> {
        let mut files = self.files.borrow_mut();
        let mut mtimes = self.mtimes.borrow_mut();
        if let Some(v) = files.remove(path) {
            let m = mtimes.remove(path).unwrap_or_else(|| (self.now)());
            files.insert(new_path.to_string(), v);
            mtimes.insert(new_path.to_string(), m);
            return Ok(());
        }
        // un dossier : on déplace tout ce qui est dessous
        let prefix = format!("{path}/");
        let keys: Vec<String> = files
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        if keys.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("ENOENT {path}"),
            ));
        }
        for k in keys {
            let nk = format!("{new_path}{}", &k[path.len()..]);
            if let Some(v) = files.remove(&k) {
                files.insert(nk.clone(), v);
            }
            let m = mtimes.remove(&k).unwrap_or_else(|| (self.now)());
            mtimes.insert(nk, m);
        }
        Ok(())
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.files.borrow_mut().remove(path);
        self.mtimes.borrow_mut().remove(path);
        Ok(())
    }
}

/// Un `JournalFs` sur un dossier temporaire (tests de `StdFs`).
pub fn std_fs_in(dir: &Path) -> StdFs {
    StdFs::new(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_now() -> Now {
        Rc::new(|| 1_000)
    }

    #[test]
    fn memoire_liste_fichiers_et_dossiers_par_prefixe() {
        let fs = MemoryFs::new(fixed_now());
        fs.write("a/b/c.txt", "1", None).unwrap();
        fs.write("a/d.txt", "22", None).unwrap();
        assert!(fs.exists("a") && fs.exists("a/b") && !fs.exists("x"));
        let l = fs.list("a");
        assert_eq!(l.files, vec!["a/d.txt"]);
        assert_eq!(l.folders, vec!["a/b"]);
        assert_eq!(fs.stat("a/d.txt").map(|s| s.size), Some(2));
        fs.rename("a/b", "a/e").unwrap();
        assert_eq!(fs.get("a/e/c.txt").as_deref(), Some("1"));
        assert!(fs.rename("rien", "ailleurs").is_err());
        fs.append("a/d.txt", "3").unwrap();
        assert_eq!(fs.get("a/d.txt").as_deref(), Some("223"));
    }

    #[test]
    fn std_fs_fait_la_meme_chose_sur_le_disque() {
        let dir =
            std::env::temp_dir().join(format!("fragment-artefacts-fs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fs = std_fs_in(&dir);
        fs.mkdir("j/k").unwrap();
        fs.append("j/k/s.ndjson", "a\n").unwrap();
        fs.append("j/k/s.ndjson", "b\n").unwrap();
        fs.write("j/o.json", "{}", Some(5_000)).unwrap();
        assert_eq!(fs.read("j/k/s.ndjson").unwrap(), "a\nb\n");
        assert_eq!(fs.stat("j/o.json").map(|s| s.mtime), Some(5_000));
        let l = fs.list("j");
        assert_eq!(l.files, vec!["j/o.json"]);
        assert_eq!(l.folders, vec!["j/k"]);
        assert_eq!(fs.list("absent"), Listed::default());
        fs.rename("j/o.json", "j/p.json").unwrap();
        assert!(fs.exists("j/p.json") && !fs.exists("j/o.json"));
        fs.remove("j/p.json").unwrap();
        fs.remove("j/p.json").unwrap(); // force : absent n'est pas une erreur
        assert!(fs.read("j/p.json").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
