// Port de app/src/ui/core/artefacts/cache.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le cache — la COPIE jetable du pli (§14.6, invariant §19.1-6).
//
//  Côté TS c'est IndexedDB, une base par coffre, dont le nom dérive de la racine
//  du coffre ET de la version du format : changer de coffre ne vide rien, et un
//  changement de format repart d'un cache vide, sans migration à écrire.
//
//      records    id        le record plié, ses estampilles, son state inline
//      segments   clé       le curseur : taille, lignes, scellé, objets cités
//      docs       chemin    taille et mtime des documents qui portent des artefacts
//      meta       nom       version du format, dernière estampille, quarantaine
//
//  Tout se reconstruit du journal : sans cache, le démarrage plie tous les
//  segments, et c'est tout. Ici : le TRAIT, le cache nul et le cache mémoire ;
//  un cache persistant (fichier dans userData) viendra avec l'app, hors de cette
//  crate (pas d'IndexedDB en Rust).
// ═══════════════════════════════════════════════════════════════════════════

use crate::types::{Rec, Stamp, Tx};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// La version du format du journal et de son cache — inscrite dans le nom de la base.
pub const FORMAT_VERSION: u32 = 2;

/// Quel journal : celui du coffre (synchronisé) ou celui de l'appareil (§14.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JournalKind {
    Vault,
    Local,
}

impl JournalKind {
    /// Les deux journaux, dans l'ordre où le TS les parcourt.
    pub const ALL: [JournalKind; 2] = [JournalKind::Vault, JournalKind::Local];

    pub fn as_str(self) -> &'static str {
        match self {
            JournalKind::Vault => "vault",
            JournalKind::Local => "local",
        }
    }
}

/// Une valeur par journal (`Record<JournalKind, T>`).
#[derive(Debug, Clone, Default)]
pub struct PerJournal<T> {
    pub vault: T,
    pub local: T,
}

impl<T> PerJournal<T> {
    pub fn get(&self, kind: JournalKind) -> &T {
        match kind {
            JournalKind::Vault => &self.vault,
            JournalKind::Local => &self.local,
        }
    }

    pub fn get_mut(&mut self, kind: JournalKind) -> &mut T {
        match kind {
            JournalKind::Vault => &mut self.vault,
            JournalKind::Local => &mut self.local,
        }
    }
}

/// Ce que ce processus a déjà plié d'un segment (§13 « Curseur »).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentCursor {
    /// `vault:<chemin>` ou `local:<chemin>` — la clé du magasin.
    pub key: String,
    pub journal: JournalKind,
    pub path: String,
    /// Taille en octets déjà lue.
    pub size: usize,
    /// Lignes complètes déjà lues (en-tête compris).
    pub lines: usize,
    /// Le segment porte sa ligne de fermeture.
    pub sealed: bool,
    /// Les empreintes d'objets citées par ses lignes (le GC en a besoin).
    pub objects: Vec<String>,
}

/// Taille et mtime d'un document qui porte des artefacts (rattachement d'un renommage externe).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocInfo {
    pub path: String,
    pub size: u64,
    pub mtime: i64,
}

/// Une transaction en quarantaine : gardée, non pliée (§14.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuarantinedTx {
    pub tx: Tx,
    pub journal: JournalKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CacheSnapshot {
    pub records: Vec<Rec>,
    pub segments: Vec<SegmentCursor>,
    pub docs: Vec<DocInfo>,
    pub last_stamp: Option<Stamp>,
    pub quarantine: Vec<QuarantinedTx>,
}

/// Un lot d'écritures, appliqué en UNE transaction.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CacheBatch {
    pub records: Vec<Rec>,
    pub segments: Vec<SegmentCursor>,
    pub delete_segments: Vec<String>,
    pub docs: Vec<DocInfo>,
    pub delete_docs: Vec<String>,
    pub last_stamp: Option<Stamp>,
    /// `Some` remplace la quarantaine entière (`None` : inchangée).
    pub quarantine: Option<Vec<QuarantinedTx>>,
}

/// Une écriture du cache en échec : le cache est jetable, le manager se contente de le noter.
#[derive(Debug, thiserror::Error)]
#[error("cache des artefacts : {0}")]
pub struct CacheError(pub String);

pub trait JournalCache {
    /// Ouvre la base de CE coffre ; `false` si le cache est indisponible.
    fn open(&self) -> bool;
    /// Tout, en une lecture (étape 3 de la figure 31).
    fn load(&self) -> CacheSnapshot;
    /// Un lot, en une transaction (étape 10).
    fn commit(&self, batch: CacheBatch) -> Result<(), CacheError>;
}

/// Un cache partagé entre deux « sessions » (le redémarrage du manager dans les tests).
pub type SharedCache = Rc<dyn JournalCache>;

/// Pas de cache : chaque démarrage plie tout.
#[derive(Debug, Default)]
pub struct NullJournalCache;

impl JournalCache for NullJournalCache {
    fn open(&self) -> bool {
        false
    }
    fn load(&self) -> CacheSnapshot {
        CacheSnapshot::default()
    }
    fn commit(&self, _batch: CacheBatch) -> Result<(), CacheError> {
        Ok(())
    }
}

/// Un cache en mémoire — le rôle d'IndexedDB dans les tests (survit à un « redémarrage » du manager).
#[derive(Debug, Default)]
pub struct MemoryJournalCache {
    pub records: RefCell<BTreeMap<String, Rec>>,
    pub segments: RefCell<BTreeMap<String, SegmentCursor>>,
    pub docs: RefCell<BTreeMap<String, DocInfo>>,
    pub last_stamp: RefCell<Option<Stamp>>,
    pub quarantine: RefCell<Vec<QuarantinedTx>>,
    pub commits: Cell<usize>,
}

impl MemoryJournalCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl JournalCache for MemoryJournalCache {
    fn open(&self) -> bool {
        true
    }

    fn load(&self) -> CacheSnapshot {
        CacheSnapshot {
            records: self.records.borrow().values().cloned().collect(),
            segments: self.segments.borrow().values().cloned().collect(),
            docs: self.docs.borrow().values().cloned().collect(),
            last_stamp: self.last_stamp.borrow().clone(),
            quarantine: self.quarantine.borrow().clone(),
        }
    }

    fn commit(&self, b: CacheBatch) -> Result<(), CacheError> {
        self.commits.set(self.commits.get() + 1);
        for r in b.records {
            self.records.borrow_mut().insert(r.id.clone(), r);
        }
        for s in b.segments {
            self.segments.borrow_mut().insert(s.key.clone(), s);
        }
        for k in b.delete_segments {
            self.segments.borrow_mut().remove(&k);
        }
        for d in b.docs {
            self.docs.borrow_mut().insert(d.path.clone(), d);
        }
        for p in b.delete_docs {
            self.docs.borrow_mut().remove(&p);
        }
        if b.last_stamp.is_some() {
            *self.last_stamp.borrow_mut() = b.last_stamp;
        }
        if let Some(q) = b.quarantine {
            *self.quarantine.borrow_mut() = q;
        }
        Ok(())
    }
}

/// Le nom de la base d'un coffre : la racine et la version du format.
pub fn db_name(vault_root: &str) -> String {
    format!("fragment-artefacts:{vault_root}:v{FORMAT_VERSION}")
}
