// Port de app/src/ui/core/vault/FileSystemAdapter.ts et de app/src/shared/vaultPath.ts
//! Le DataAdapter sur `std::fs` : le coffre, lu directement sur le disque.
//!
//! ★ Tous les chemins reçus sont de coffre (relatifs, séparés par `/`) ; [`FileSystemAdapter::absolu`]
//!   est l'UNIQUE point de passage vers le disque, et il porte le garde-fou : octet nul, chemin
//!   absolu, lecteur Windows et `..` qui sortirait de la racine sont refusés.
//!
//! ★ AUCUNE normalisation Unicode des noms : un nom de fichier est un identifiant comparé point de
//!   code par point de code (NFD d'un dossier venu de macOS compris). Les chemins viennent de
//!   `read_dir` et repartent tels quels, c'est ce qui fait que les clefs de l'arbre du coffre
//!   s'apparient.
//!
//! ★ La surveillance (voir `surveillance.rs`) tourne dans son fil et ne parle pas au coffre
//!   directement : `pomper` vide ses changements SUR LE FIL DE L'HÔTE et rappelle le coffre.
//!   Les écritures de l'app ne sont pas doublées ici : leur écho (même mtime, même taille)
//!   est absorbé par le coffre, qui annonce l'événement `user` une seule fois.

use std::cell::RefCell;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::adaptateur::{DataAdapter, DataWriteOptions, ListedFiles, OnChange, Stat};
use super::surveillance::{OptionsSurveillance, Surveillance, SurveillanceError};

/// Le dossier de corbeille interne au coffre.
const DOSSIER_CORBEILLE: &str = ".trash";

fn invalide(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

/// Chemin de coffre → morceaux sûrs. `.` et les segments vides disparaissent, `..` remonte, et
/// remonter au-dessus de la racine est refusé.
fn morceaux_surs(requested: &str) -> io::Result<Vec<&str>> {
    if requested.contains('\0') {
        return Err(invalide(format!(
            "chemin invalide (octet nul) : {requested:?}"
        )));
    }
    if requested.starts_with('/') || requested.starts_with('\\') {
        return Err(invalide(format!("chemin absolu refusé : {requested}")));
    }
    let mut octets = requested.bytes();
    if let (Some(a), Some(b':')) = (octets.next(), octets.next())
        && a.is_ascii_alphabetic()
    {
        return Err(invalide(format!(
            "chemin relatif à un lecteur refusé : {requested}"
        )));
    }
    let mut morceaux: Vec<&str> = Vec::new();
    for s in requested.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                if morceaux.pop().is_none() {
                    return Err(invalide(format!("hors du coffre : {requested}")));
                }
            }
            _ => {
                // Sous Windows, `\` est un séparateur : « a\..\.. » échapperait au contrôle ci-dessus.
                if cfg!(windows) && s.contains(['\\', ':']) {
                    return Err(invalide(format!("segment refusé : {requested}")));
                }
                morceaux.push(s);
            }
        }
    }
    Ok(morceaux)
}

fn en_millis(t: io::Result<SystemTime>) -> Option<i64> {
    let d = t.ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(d.as_millis()).ok()
}

/// L'encodage de `encodeURIComponent` : tout est échappé sauf `A-Z a-z 0-9 - _ . ! ~ * ' ( )`.
fn encoder_segment(segment: &str) -> String {
    let mut sortie = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            sortie.push(char::from(b));
        } else {
            sortie.push_str(&format!("%{b:02X}"));
        }
    }
    sortie
}

fn copier_recursif(src: &Path, dst: &Path) -> io::Result<()> {
    if fs::metadata(src)?.is_dir() {
        // `create_dir` et non `create_dir_all` : une destination qui existe est une erreur.
        fs::create_dir(dst)?;
        for entree in fs::read_dir(src)? {
            let entree = entree?;
            copier_recursif(&entree.path(), &dst.join(entree.file_name()))?;
        }
        Ok(())
    } else {
        // `copy` écraserait : on réserve la destination d'abord.
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dst)?;
        fs::copy(src, dst).map(|_| ())
    }
}

pub struct FileSystemAdapter {
    racine: PathBuf,
    on_change: RefCell<Option<OnChange>>,
    surveillance: RefCell<Option<Surveillance>>,
}

impl FileSystemAdapter {
    pub fn new(racine: impl Into<PathBuf>) -> Self {
        Self {
            racine: racine.into(),
            on_change: RefCell::new(None),
            surveillance: RefCell::new(None),
        }
    }

    /// La racine absolue du coffre (`getBasePath` d'Obsidian) : pour lancer un outil externe
    /// DANS le coffre. Les lectures et écritures passent toujours par l'adaptateur.
    pub fn get_base_path(&self) -> &Path {
        &self.racine
    }

    /// Chemin de coffre → chemin absolu, avec le garde-fou.
    pub fn absolu(&self, path: &str) -> io::Result<PathBuf> {
        let mut p = self.racine.clone();
        for m in morceaux_surs(path)? {
            p.push(m);
        }
        Ok(p)
    }

    // ── Surveillance ────────────────────────────────────────────────────────

    /// Lance la surveillance de la racine (sans effet si elle tourne déjà). Les changements
    /// n'atteignent le coffre qu'à `pomper` / `attendre_changements`.
    pub fn surveiller(&self, options: OptionsSurveillance) -> Result<(), SurveillanceError> {
        let mut s = self.surveillance.borrow_mut();
        if s.is_none() {
            *s = Some(Surveillance::demarrer(&self.racine, options)?);
        }
        Ok(())
    }

    /// Arrêt propre : le moteur est débranché et le fil attendu. Les changements pas encore
    /// pompés sont perdus.
    pub fn arreter_surveillance(&self) {
        if let Some(s) = self.surveillance.borrow_mut().take() {
            s.arreter();
        }
    }

    pub fn surveille(&self) -> bool {
        self.surveillance.borrow().is_some()
    }

    /// Rappelle le coffre pour chaque changement du disque en attente ; rend leur nombre.
    pub fn pomper(&self) -> usize {
        let mut en_attente = Vec::new();
        if let Some(s) = self.surveillance.borrow().as_ref() {
            while let Some(c) = s.essayer_recevoir() {
                en_attente.push(c);
            }
        }
        self.livrer(en_attente)
    }

    /// Comme [`pomper`](Self::pomper), mais attend au plus `delai` un premier changement.
    pub fn attendre_changements(&self, delai: Duration) -> usize {
        let premier = self
            .surveillance
            .borrow()
            .as_ref()
            .and_then(|s| s.recevoir_avant(delai));
        let Some(premier) = premier else { return 0 };
        let mut lot = vec![premier];
        if let Some(s) = self.surveillance.borrow().as_ref() {
            while let Some(c) = s.essayer_recevoir() {
                lot.push(c);
            }
        }
        self.livrer(lot)
    }

    /// ★ Le rappel est appelé SANS emprunt tenu : le coffre rappelle l'adaptateur (`stat`…).
    fn livrer(&self, lot: Vec<(super::adaptateur::ChangeType, String)>) -> usize {
        let Some(cb) = self.on_change.borrow().clone() else {
            return 0;
        };
        let n = lot.len();
        for (kind, path) in lot {
            cb(kind, &path);
        }
        n
    }

    fn appliquer_options(&self, chemin: &Path, options: DataWriteOptions) -> io::Result<()> {
        // Sans ça, l'OS écrit l'heure de CETTE écriture, alors qu'elle peut être une synchro :
        // la vraie date du contenu est celle que l'appelant connaît. Seul le mtime est imposé
        // (la date de naissance d'un fichier ne se modifie pas).
        let Some(ms) = options.mtime else {
            return Ok(());
        };
        let t = UNIX_EPOCH + Duration::from_millis(u64::try_from(ms).unwrap_or(0));
        fs::OpenOptions::new()
            .write(true)
            .open(chemin)?
            .set_modified(t)
    }

    fn ecrire(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()> {
        let chemin = self.absolu(path)?;
        fs::write(&chemin, data)?;
        self.appliquer_options(&chemin, options)
    }

    fn ajouter(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()> {
        let chemin = self.absolu(path)?;
        fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&chemin)?
            .write_all(data)?;
        self.appliquer_options(&chemin, options)
    }
}

impl Drop for FileSystemAdapter {
    fn drop(&mut self) {
        self.arreter_surveillance();
    }
}

impl DataAdapter for FileSystemAdapter {
    fn get_name(&self) -> String {
        self.racine
            .file_name()
            .map_or_else(|| "coffre".to_owned(), |n| n.to_string_lossy().into_owned())
    }

    // ── Lecture ─────────────────────────────────────────────────────────────

    fn exists(&self, path: &str) -> bool {
        // Tolérant : EACCES est aussi rapporté « absent » (compromis du coffre local).
        self.absolu(path).is_ok_and(|p| p.exists())
    }

    fn stat(&self, path: &str) -> Option<Stat> {
        let meta = fs::metadata(self.absolu(path).ok()?).ok()?;
        let mtime = en_millis(meta.modified()).unwrap_or(0);
        Some(Stat {
            is_folder: meta.is_dir(),
            // Certains systèmes de fichiers n'ont pas de date de naissance : on se rabat sur mtime.
            ctime: en_millis(meta.created()).unwrap_or(mtime),
            mtime,
            size: meta.len(),
        })
    }

    fn list(&self, path: &str) -> io::Result<ListedFiles> {
        let mut liste = ListedFiles::default();
        for entree in fs::read_dir(self.absolu(path)?)? {
            let entree = entree?;
            let Some(nom) = entree.file_name().to_str().map(str::to_owned) else {
                continue; // nom non UTF-8 : indicible en chemin de coffre
            };
            // Chemin de coffre : `/` sur toutes les plateformes, racine sans préfixe.
            let enfant = match morceaux_surs(path)?.join("/") {
                d if d.is_empty() => nom,
                d => format!("{d}/{nom}"),
            };
            // Le type n'est pas suivi : un lien symbolique est un fichier (comme Dirent).
            if entree.file_type()?.is_dir() {
                liste.folders.push(enfant);
            } else {
                liste.files.push(enfant);
            }
        }
        Ok(liste)
    }

    fn read(&self, path: &str) -> io::Result<String> {
        fs::read_to_string(self.absolu(path)?)
    }

    fn read_binary(&self, path: &str) -> io::Result<Vec<u8>> {
        fs::read(self.absolu(path)?)
    }

    // ── Écriture ────────────────────────────────────────────────────────────

    fn write(&self, path: &str, data: &str, options: DataWriteOptions) -> io::Result<()> {
        self.ecrire(path, data.as_bytes(), options)
    }

    fn write_binary(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()> {
        self.ecrire(path, data, options)
    }

    fn append(&self, path: &str, data: &str, options: DataWriteOptions) -> io::Result<()> {
        self.ajouter(path, data.as_bytes(), options)
    }

    fn append_binary(&self, path: &str, data: &[u8], options: DataWriteOptions) -> io::Result<()> {
        self.ajouter(path, data, options)
    }

    // ── Arborescence ────────────────────────────────────────────────────────

    fn mkdir(&self, path: &str) -> io::Result<()> {
        fs::create_dir_all(self.absolu(path)?)
    }

    /// ★ Divergence de plateforme : si la destination existe, POSIX l'ÉCRASE, Windows refuse.
    ///   C'est au coffre de gérer cette divergence.
    fn rename(&self, path: &str, new_path: &str) -> io::Result<()> {
        fs::rename(self.absolu(path)?, self.absolu(new_path)?)
    }

    fn copy(&self, path: &str, new_path: &str) -> io::Result<()> {
        copier_recursif(&self.absolu(path)?, &self.absolu(new_path)?)
    }

    /// Fichiers seulement : sur un dossier, `remove_file` échoue (ce qu'on veut). Un dossier
    /// avalé sans broncher sur une erreur d'aiguillage détruirait une arborescence.
    fn remove(&self, path: &str) -> io::Result<()> {
        fs::remove_file(self.absolu(path)?)
    }

    /// Non récursif : échoue si le dossier n'est pas vide (c'est le garde-fou).
    fn rmdir(&self, path: &str, recursive: bool) -> io::Result<()> {
        let chemin = self.absolu(path)?;
        if recursive {
            fs::remove_dir_all(chemin)
        } else {
            fs::remove_dir(chemin)
        }
    }

    fn trash_system(&self, path: &str) -> io::Result<bool> {
        let chemin = self.absolu(path)?;
        if !chemin.exists() {
            return Ok(false);
        }
        // L'OS refuse (pas de corbeille, volume réseau) : le coffre se replie sur la locale.
        Ok(trash::delete(chemin).is_ok())
    }

    /// La corbeille interne est PLATE : on ne garde que le nom du fichier (comme Obsidian),
    /// d'où le suffixe numéroté quand deux notes de dossiers différents se rencontrent.
    fn trash_local(&self, path: &str) -> io::Result<()> {
        let source = self.absolu(path)?;
        let nom = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalide(format!("rien à mettre à la corbeille : {path}")))?
            .to_owned();
        let radical = source.file_stem().and_then(|n| n.to_str()).unwrap_or(&nom);
        let extension = source
            .extension()
            .and_then(|n| n.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();

        let dossier = self.racine.join(DOSSIER_CORBEILLE);
        let mut destination = dossier.join(&nom);
        let mut compteur = 1;
        while destination.exists() {
            destination = dossier.join(format!("{radical} {compteur}{extension}"));
            compteur += 1;
        }
        // Le dossier est le nôtre (pas un chemin fourni par l'appelant) : le créer est légitime.
        fs::create_dir_all(&dossier)?;
        // Un renommage : atomique et instantané, `.trash/` étant dans le coffre (même volume).
        fs::rename(source, destination)
    }

    // ── Ressources ──────────────────────────────────────────────────────────

    /// `fragment-local://vault/<chemin encodé>[?<clef de cache>]` : le contrat partagé avec
    /// le gestionnaire de protocole. Pas d'octets transportés, pas de chemin absolu exposé.
    fn get_resource_path(&self, path: &str, cache_key: Option<i64>) -> String {
        let encode: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(encoder_segment)
            .collect();
        let requete = cache_key.map_or_else(String::new, |k| format!("?{k}"));
        format!("fragment-local://vault/{}{requete}", encode.join("/"))
    }

    fn set_on_change(&self, callback: Option<OnChange>) {
        *self.on_change.borrow_mut() = callback;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garde_fou_refuse_les_evasions() {
        for mauvais in [
            "/etc/passwd",
            "C:/x",
            "../dehors",
            "a/../../b",
            "a\0b",
            "\\x",
        ] {
            assert!(morceaux_surs(mauvais).is_err(), "{mauvais:?}");
        }
        assert_eq!(
            morceaux_surs("a/./b//c").unwrap_or_default(),
            ["a", "b", "c"]
        );
        assert_eq!(morceaux_surs("a/../b").unwrap_or_default(), ["b"]);
        assert!(morceaux_surs("").unwrap_or_default().is_empty());
    }

    #[test]
    fn encodage_comme_encode_uri_component() {
        assert_eq!(encoder_segment("a b é.png"), "a%20b%20%C3%A9.png");
        assert_eq!(encoder_segment("x-_.!~*'()"), "x-_.!~*'()");
    }
}
