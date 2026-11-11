// Port de app/src/ui/core/artefacts/device.ts (MemoryAdapter), pour le DataAdapter du coffre
//! Un adaptateur en mémoire : les tests du coffre et de tout ce qui en dépend.
//!
//! Contrairement à celui de `fragment-artefacts`, les dossiers existent pour de bon
//! (`mkdir`, dossier vide) : le coffre en a besoin, son arbre en contient. L'horloge est
//! LOGIQUE (un tic par écriture) : les `mtime` sont déterministes et toujours croissants,
//! ce que les tests de `cached_read` et de `modify` exigent.
//!
//! Il ne surveille rien. Pour simuler la surveillance, un test écrit « dehors »
//! ([`MemoryAdapter::put`], qui ne dit rien au coffre) puis annonce
//! ([`MemoryAdapter::emit`]) — exactement ce que fera le watcher de fr-t2-fs.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::io;

use super::adaptateur::{ChangeType, DataAdapter, DataWriteOptions, ListedFiles, OnChange, Stat};

#[derive(Clone)]
struct Entree {
    data: Vec<u8>,
    ctime: i64,
    mtime: i64,
}

#[derive(Default)]
struct Etat {
    files: BTreeMap<String, Entree>,
    folders: BTreeSet<String>,
    illisibles: BTreeSet<String>,
    corbeille_systeme: Vec<String>,
}

pub struct MemoryAdapter {
    nom: String,
    etat: RefCell<Etat>,
    horloge: Cell<i64>,
    corbeille_systeme_ok: Cell<bool>,
    on_change: RefCell<Option<OnChange>>,
}

impl Default for MemoryAdapter {
    fn default() -> Self {
        Self::new()
    }
}

fn parent_de(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) if i > 0 => &path[..i],
        _ => "",
    }
}

fn introuvable(path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("ENOENT {path}"))
}

fn existe_deja(path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::AlreadyExists, format!("EEXIST {path}"))
}

impl MemoryAdapter {
    pub fn new() -> Self {
        Self {
            nom: "memoire".to_owned(),
            etat: RefCell::new(Etat::default()),
            horloge: Cell::new(1_000),
            corbeille_systeme_ok: Cell::new(true),
            on_change: RefCell::new(None),
        }
    }

    fn tic(&self) -> i64 {
        let t = self.horloge.get() + 1;
        self.horloge.set(t);
        t
    }

    fn dossier_existe(etat: &Etat, path: &str) -> bool {
        path.is_empty() || etat.folders.contains(path)
    }

    fn creer_ancetres(etat: &mut Etat, path: &str) {
        let mut courant = String::new();
        for part in path.split('/').filter(|s| !s.is_empty()) {
            if !courant.is_empty() {
                courant.push('/');
            }
            courant.push_str(part);
            etat.folders.insert(courant.clone());
        }
    }

    // ── Aides des tests (rien n'est annoncé au coffre) ────────────────────────

    /// Pose un fichier « du dehors » (crée ses dossiers), sans événement.
    pub fn put(&self, path: &str, data: &str) {
        self.put_binary(path, data.as_bytes());
    }

    pub fn put_binary(&self, path: &str, data: &[u8]) {
        let t = self.tic();
        let mut e = self.etat.borrow_mut();
        Self::creer_ancetres(&mut e, parent_de(path));
        let ctime = e.files.get(path).map_or(t, |x| x.ctime);
        e.files.insert(
            path.to_owned(),
            Entree {
                data: data.to_vec(),
                ctime,
                mtime: t,
            },
        );
    }

    /// Pose un dossier (et ses ancêtres), sans événement.
    pub fn put_dir(&self, path: &str) {
        Self::creer_ancetres(&mut self.etat.borrow_mut(), path);
    }

    /// Retire du dehors un fichier ou un dossier entier, sans événement.
    pub fn retirer(&self, path: &str) {
        let mut e = self.etat.borrow_mut();
        let prefixe = format!("{path}/");
        e.files.retain(|k, _| k != path && !k.starts_with(&prefixe));
        e.folders.retain(|k| k != path && !k.starts_with(&prefixe));
    }

    /// Le contenu d'un fichier (les tests regardent le disque).
    pub fn get(&self, path: &str) -> Option<String> {
        let e = self.etat.borrow();
        let b = &e.files.get(path)?.data;
        Some(String::from_utf8_lossy(b).into_owned())
    }

    pub fn has(&self, path: &str) -> bool {
        self.exists(path)
    }

    /// Tous les chemins (fichiers et dossiers), triés.
    pub fn keys(&self) -> Vec<String> {
        let e = self.etat.borrow();
        let mut v: Vec<String> = e.files.keys().chain(e.folders.iter()).cloned().collect();
        v.sort();
        v
    }

    /// Ce que l'« OS » a reçu en corbeille.
    pub fn corbeille_systeme(&self) -> Vec<String> {
        self.etat.borrow().corbeille_systeme.clone()
    }

    /// `false` : l'OS refuse la corbeille (volume réseau) — le coffre doit se replier.
    pub fn set_system_trash(&self, ok: bool) {
        self.corbeille_systeme_ok.set(ok);
    }

    /// Fait échouer `list` sur ce dossier (droits, disque en panne).
    pub fn rendre_illisible(&self, path: &str) {
        self.etat.borrow_mut().illisibles.insert(path.to_owned());
    }

    /// Annonce un changement au coffre, comme le watcher.
    pub fn emit(&self, kind: ChangeType, path: &str) {
        let cb = self.on_change.borrow().clone();
        if let Some(cb) = cb {
            cb(kind, path);
        }
    }

    pub fn a_un_rappel(&self) -> bool {
        self.on_change.borrow().is_some()
    }

    fn ecrire(
        &self,
        path: &str,
        data: &[u8],
        options: DataWriteOptions,
        ajout: bool,
    ) -> io::Result<()> {
        let t = self.tic();
        let mut e = self.etat.borrow_mut();
        if !Self::dossier_existe(&e, parent_de(path)) {
            return Err(introuvable(parent_de(path)));
        }
        if e.folders.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("EISDIR {path}"),
            ));
        }
        let ancien = e.files.get(path).cloned();
        let mut octets = match (&ancien, ajout) {
            (Some(a), true) => a.data.clone(),
            _ => Vec::new(),
        };
        octets.extend_from_slice(data);
        let ctime = ancien.map(|a| a.ctime).or(options.ctime).unwrap_or(t);
        e.files.insert(
            path.to_owned(),
            Entree {
                data: octets,
                ctime,
                mtime: options.mtime.unwrap_or(t),
            },
        );
        Ok(())
    }
}

impl DataAdapter for MemoryAdapter {
    fn get_name(&self) -> String {
        self.nom.clone()
    }

    fn exists(&self, path: &str) -> bool {
        let e = self.etat.borrow();
        e.files.contains_key(path) || Self::dossier_existe(&e, path)
    }

    fn stat(&self, path: &str) -> Option<Stat> {
        let e = self.etat.borrow();
        if let Some(f) = e.files.get(path) {
            return Some(Stat {
                is_folder: false,
                ctime: f.ctime,
                mtime: f.mtime,
                size: f.data.len() as u64,
            });
        }
        Self::dossier_existe(&e, path).then_some(Stat {
            is_folder: true,
            ctime: 0,
            mtime: 0,
            size: 0,
        })
    }

    fn list(&self, path: &str) -> io::Result<ListedFiles> {
        let e = self.etat.borrow();
        if e.illisibles.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("EACCES {path}"),
            ));
        }
        if !Self::dossier_existe(&e, path) {
            return Err(introuvable(path));
        }
        let ici = |p: &&String| parent_de(p) == path;
        Ok(ListedFiles {
            files: e.files.keys().filter(ici).cloned().collect(),
            folders: e.folders.iter().filter(ici).cloned().collect(),
        })
    }

    fn read(&self, path: &str) -> io::Result<String> {
        let e = self.etat.borrow();
        let f = e.files.get(path).ok_or_else(|| introuvable(path))?;
        String::from_utf8(f.data.clone())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "UTF-8 invalide"))
    }

    fn read_binary(&self, path: &str) -> io::Result<Vec<u8>> {
        let e = self.etat.borrow();
        Ok(e.files
            .get(path)
            .ok_or_else(|| introuvable(path))?
            .data
            .clone())
    }

    fn write(&self, path: &str, data: &str, options: DataWriteOptions) -> io::Result<()> {
        self.ecrire(path, data.as_bytes(), options, false)
    }

    fn write_binary(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()> {
        self.ecrire(path, data, options, false)
    }

    fn append(&self, path: &str, data: &str, options: DataWriteOptions) -> io::Result<()> {
        self.ecrire(path, data.as_bytes(), options, true)
    }

    fn append_binary(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()> {
        self.ecrire(path, data, options, true)
    }

    fn mkdir(&self, path: &str) -> io::Result<()> {
        let mut e = self.etat.borrow_mut();
        if e.files.contains_key(path) {
            return Err(existe_deja(path));
        }
        Self::creer_ancetres(&mut e, path);
        Ok(())
    }

    fn rename(&self, path: &str, new_path: &str) -> io::Result<()> {
        let mut e = self.etat.borrow_mut();
        if e.files.contains_key(new_path) || e.folders.contains(new_path) {
            return Err(existe_deja(new_path));
        }
        if !Self::dossier_existe(&e, parent_de(new_path)) {
            return Err(introuvable(parent_de(new_path)));
        }
        if let Some(f) = e.files.remove(path) {
            e.files.insert(new_path.to_owned(), f);
            return Ok(());
        }
        if !e.folders.contains(path) {
            return Err(introuvable(path));
        }
        let prefixe = format!("{path}/");
        let refaire = |k: &str| format!("{new_path}{}", &k[path.len()..]);
        let fichiers: Vec<String> = e
            .files
            .keys()
            .filter(|k| k.starts_with(&prefixe))
            .cloned()
            .collect();
        for k in fichiers {
            if let Some(f) = e.files.remove(&k) {
                e.files.insert(refaire(&k), f);
            }
        }
        let dossiers: Vec<String> = e
            .folders
            .iter()
            .filter(|k| *k == path || k.starts_with(&prefixe))
            .cloned()
            .collect();
        for k in dossiers {
            e.folders.remove(&k);
            e.folders.insert(refaire(&k));
        }
        Ok(())
    }

    fn copy(&self, path: &str, new_path: &str) -> io::Result<()> {
        let t = self.tic();
        let mut e = self.etat.borrow_mut();
        if e.files.contains_key(new_path) || e.folders.contains(new_path) {
            return Err(existe_deja(new_path));
        }
        if !Self::dossier_existe(&e, parent_de(new_path)) {
            return Err(introuvable(parent_de(new_path)));
        }
        if let Some(f) = e.files.get(path).cloned() {
            e.files.insert(
                new_path.to_owned(),
                Entree {
                    ctime: t,
                    mtime: t,
                    ..f
                },
            );
            return Ok(());
        }
        if !e.folders.contains(path) {
            return Err(introuvable(path));
        }
        let prefixe = format!("{path}/");
        let refaire = |k: &str| format!("{new_path}{}", &k[path.len()..]);
        let fichiers: Vec<(String, Entree)> = e
            .files
            .iter()
            .filter(|(k, _)| k.starts_with(&prefixe))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (k, v) in fichiers {
            e.files.insert(
                refaire(&k),
                Entree {
                    ctime: t,
                    mtime: t,
                    ..v
                },
            );
        }
        let dossiers: Vec<String> = e
            .folders
            .iter()
            .filter(|k| *k == path || k.starts_with(&prefixe))
            .cloned()
            .collect();
        for k in dossiers {
            e.folders.insert(refaire(&k));
        }
        Ok(())
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        let mut e = self.etat.borrow_mut();
        e.files
            .remove(path)
            .map(|_| ())
            .ok_or_else(|| introuvable(path))
    }

    fn rmdir(&self, path: &str, recursive: bool) -> io::Result<()> {
        let mut e = self.etat.borrow_mut();
        if !e.folders.contains(path) {
            return Err(introuvable(path));
        }
        let prefixe = format!("{path}/");
        let non_vide = e.files.keys().any(|k| k.starts_with(&prefixe))
            || e.folders.iter().any(|k| k.starts_with(&prefixe));
        if non_vide && !recursive {
            return Err(io::Error::other(format!("ENOTEMPTY {path}")));
        }
        e.files.retain(|k, _| !k.starts_with(&prefixe));
        e.folders.retain(|k| k != path && !k.starts_with(&prefixe));
        Ok(())
    }

    fn trash_system(&self, path: &str) -> io::Result<bool> {
        if !self.corbeille_systeme_ok.get() {
            return Ok(false);
        }
        self.retirer(path);
        self.etat
            .borrow_mut()
            .corbeille_systeme
            .push(path.to_owned());
        Ok(true)
    }

    fn trash_local(&self, path: &str) -> io::Result<()> {
        if !self.exists(path) {
            return Err(introuvable(path));
        }
        let nom = path.rsplit('/').next().unwrap_or(path);
        let mut cible = format!(".trash/{nom}");
        let mut n = 0;
        while self.exists(&cible) {
            n += 1;
            cible = format!(".trash/{nom} {n}");
        }
        self.mkdir(".trash")?;
        self.rename(path, &cible)
    }

    fn get_resource_path(&self, path: &str, cache_key: Option<i64>) -> String {
        match cache_key {
            Some(k) => format!("memoire://{path}?{k}"),
            None => format!("memoire://{path}"),
        }
    }

    fn set_on_change(&self, callback: Option<OnChange>) {
        *self.on_change.borrow_mut() = callback;
    }
}
