// Port de app/src/ui/core/artefacts/ArtefactManager.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Gestionnaire des artefacts, côté journal (journal-artefacts.pdf, partie III).
//
//  Chemin local : set/del des plugins → dispatch → horloge → pli → index → abonnés ;
//  dispatch alimente aussi le tampon d'écriture, qui ajoute au segment
//  journal/<appareil>/<segment>.ndjson.
//  Chemin distant : watcher (raw) → lecteur de queue → pli, avec l'origine disk.
//
//  Figure 23 : qu'une transaction vienne d'ici ou du fichier d'un autre écrivain,
//  elle traverse le même pli ; ni l'index ni les vues ne peuvent les distinguer.
//
//  Invariants (§19.1) :
//    1. L'index ne change que par dispatch.
//    2. Par segment, un écrivain unique et de l'ajout seulement.
//    3. Pli pur, commutatif, associatif, idempotent.
//    4. L'objet existe sur disque avant la ligne qui le référence.
//    5. Aucune destruction sans intention écrite ni avant la fin de la rétention ;
//       les tombes restent pour toujours.
//    6. Le cache se jette sans perte : tout se reconstruit du journal.
//    7. Le cœur ne lit jamais `state`.
//    8. Un kind absent, ou trop ancien, laisse ses records inertes.
//    9. Le DOM d'un artefact sort toujours de rehydrate (le calque, ArtefactsPlugin).
//   10. Le cœur ne dépend ni du DOM ni du workspace. Seul le calque en dépend.
//
//  ★ Ce que le port change de forme, pas de fond :
//    - tout est SYNCHRONE (le disque l'est) : `flushed()`, `load()`, `state()`,
//      `history()`, `maintain()` rendent leur résultat au retour de l'appel ;
//    - les minuteries (`setTimeout`/`setInterval` : tic du tampon, calme des
//      ancres suivies, 200 ms de queue sur un segment, quarantaine, repos) sont
//      des ÉCHÉANCES tenues par le manager et servies par `poll()`, que l'hôte
//      appelle régulièrement (GPUI : un timer de fond). Les tests avancent
//      l'horloge injectée et appellent `poll()` ;
//    - les événements du Vault et du flux des documents arrivent par des
//      MÉTHODES (`on_document_event`, `on_raw`) que l'app branche ; `Events`
//      (`clock-skew`) devient un rappel (`on_clock_skew`) ;
//    - les abonnés reçoivent `(&Tx, &[Change])` et ne peuvent pas rappeler le
//      manager pendant la notification (emprunt exclusif) : ils lisent les
//      instantanés avant/après, ce qui est le contrat.
// ═══════════════════════════════════════════════════════════════════════════

use crate::cache::{
    CacheBatch, DocInfo, JournalKind, NullJournalCache, PerJournal, QuarantinedTx, SegmentCursor,
    SharedCache,
};
use crate::clock::{HybridClock, Now, stamp_writer, system_now};
use crate::device::{local_root, random_id, random_uuid, read_device_id};
use crate::fold::{alive, fold, newer};
use crate::fs::{MemoryFs, SharedFs, StdFs};
use crate::journal::compact::{
    ABANDONED_MS, COMPACT_MIN_BYTES, COMPACT_MIN_SEGMENTS, WriteCompacted, compact_txs,
    content_stamps_of, write_compacted,
};
use crate::journal::import::import_legacy_refs;
use crate::journal::reader::{JournalReader, cursor_key};
use crate::journal::writer::{FlushResult, SegmentWriter, SegmentWriterOptions, WriteBuffer};
use crate::objects::{ObjectStore, fits_inline};
use crate::types::{
    ArtefactKind, ArtefactRef, ArtefactScope, Change, Fields, Filter, Index, Op, Origin, Rec,
    SavePatch, SaveSpec, ScopeFilter, SetOp, Stamp, Tx,
};
use serde_json::Value;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

/// Le calme attendu sur un segment d'un autre écrivain avant de le relire (§15.4).
pub const TAIL_CALM_MS: i64 = 200;
/// Le délai avant la maintenance au repos (compaction, balayage) après `ready`.
const IDLE_DELAY_MS: i64 = 5000;
/// La fréquence à laquelle on regarde si l'heure a rattrapé la quarantaine.
const QUARANTINE_CHECK_MS: i64 = 30_000;

/// Le rappel d'un abonné : la transaction et ce qu'elle a changé aux records du filtre.
pub type SubscriberFn = Box<dyn FnMut(&Tx, &[Change])>;
/// Le rappel de `clock-skew` : l'écrivain dont l'horloge est en avance.
pub type ClockSkewFn = Box<dyn FnMut(&str)>;

/// Un abonnement : un filtre et son rappel (§13).
struct Subscription {
    id: u64,
    filter: Filter,
    cb: SubscriberFn,
}

/// Le retrait d'un abonnement (`subscribe` rend ce jeton).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

/// Taille et mtime d'un fichier du coffre (`TFile.stat`, sans `ctime`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStats {
    pub size: u64,
    pub mtime: i64,
}

/// Ce que le manager lit du Vault : les fichiers présents et leur stat
/// (`getFileByPath`, `getFiles`). L'app le branche sur son coffre.
pub trait VaultFiles {
    /// Le fichier à ce chemin, s'il existe dans le coffre.
    fn file_stat(&self, path: &str) -> Option<FileStats>;
    /// Tous les fichiers du coffre, ou `None` si le coffre ne sait pas les énumérer
    /// (le `typeof vault.getFiles !== 'function'` du TS).
    fn files(&self) -> Option<Vec<(String, FileStats)>>;
}

/// Un coffre sans fichiers (le défaut : aucun rattachement).
pub struct NoVaultFiles;

impl VaultFiles for NoVaultFiles {
    fn file_stat(&self, _path: &str) -> Option<FileStats> {
        None
    }
    fn files(&self) -> Option<Vec<(String, FileStats)>> {
        None
    }
}

/// `origin` d'un événement de document : `user` = geste dans l'app, `disk` = constaté de dehors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOrigin {
    User,
    Disk,
}

/// Ce qui arrive à un document (`documents/types.ts`, DocumentEvent). La clé est
/// un chemin du coffre ou `schéma:reste`.
/// ★ Copie minimale du type de fragment-core (fr-p1-core-b) : à réexporter quand il existera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentEvent {
    Create {
        key: String,
    },
    Modify {
        key: String,
    },
    Rename {
        key: String,
        old_key: String,
        origin: Option<EventOrigin>,
    },
    Delete {
        key: String,
        origin: Option<EventOrigin>,
    },
}

#[derive(Default)]
pub struct ArtefactManagerOptions {
    /// Le cache (défaut : aucun, chaque démarrage plie tout).
    pub cache: Option<SharedCache>,
    /// L'adaptateur du journal local (défaut : userData, ou mémoire sans userData).
    pub local: Option<SharedFs>,
    pub device_id: Option<String>,
    pub session: Option<String>,
    /// L'horloge physique (tests).
    pub now: Option<Now>,
    /// `<userData>` d'Electron : `device.json`, journal local. `None` = tout en mémoire.
    pub user_data_dir: Option<PathBuf>,
    /// Le chemin du coffre sur le disque (clé du journal local).
    pub vault_path: Option<String>,
    /// La version de l'app, écrite dans l'en-tête des segments.
    pub app_version: String,
}

/// Une surface déclarée par le calque `artefacts` pour une vue : sa WidgetLayer
/// partagée (un type de fragment-ui, opaque ici) et si la vue a un éditeur.
pub struct Surface {
    pub widgets: Rc<dyn Any>,
    pub has_editor: bool,
}

/// Un message de diagnostic (les `console.warn`/`console.error` du TS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub error: bool,
    pub message: String,
}

fn add_to(map: &mut HashMap<String, BTreeSet<String>>, key: &str, id: &str) {
    map.entry(key.to_string())
        .or_default()
        .insert(id.to_string());
}

fn remove_from(map: &mut HashMap<String, BTreeSet<String>>, key: &str, id: &str) {
    if let Some(set) = map.get_mut(key) {
        set.remove(id);
        if set.is_empty() {
            map.remove(key);
        }
    }
}

/// Le record correspond-il au filtre ?
pub fn matches(r: &Rec, f: Option<&Filter>) -> bool {
    let Some(f) = f else { return true };
    if f.id.as_ref().is_some_and(|id| &r.id != id) {
        return false;
    }
    if f.type_.is_some() && r.type_ != f.type_ {
        return false;
    }
    if let Some(s) = &f.scope {
        let Some(rs) = &r.scope else { return false };
        if s.kind.is_some_and(|k| rs.kind() != k) {
            return false;
        }
        if let Some(p) = &s.path
            && !matches!(rs, ArtefactScope::File { path } if path == p)
        {
            return false;
        }
        if let Some(c) = &s.collection
            && !matches!(rs, ArtefactScope::Plugin { collection } if collection == c)
        {
            return false;
        }
    }
    true
}

/// Fusionne un record du cache dans un record déjà présent, groupe par groupe —
/// le même arbitrage que le pli (une écriture locale peut précéder le chargement).
fn merge_rec(cur: &mut Rec, r: &Rec) {
    if let Some(t) = &r.content_t
        && newer(t, cur.content_t.as_deref())
    {
        cur.type_ = r.type_.clone();
        cur.v = r.v;
        cur.state = r.state.clone();
        cur.head = r.head.clone();
        cur.content_t = r.content_t.clone();
    }
    if let Some(t) = &r.scope_t
        && newer(t, cur.scope_t.as_deref())
    {
        cur.scope = r.scope.clone();
        cur.scope_t = r.scope_t.clone();
    }
    if let Some(t) = &r.anchor_t
        && newer(t, cur.anchor_t.as_deref())
    {
        cur.anchor = r.anchor.clone();
        cur.anchor_t = r.anchor_t.clone();
    }
    if let Some(t) = &r.del_t
        && newer(t, cur.del_t.as_deref())
    {
        cur.del_t = r.del_t.clone();
    }
    if r.local {
        cur.local = true;
    }
}

// ── la forme d'une clé de document (documents/documentKey.ts) ─────────────────
// ★ Copies minimales, privées, de fragment-core (fr-p1-core-b n'est pas fusionnée) :
//   à remplacer par un import quand la crate les exposera.

/// Un chemin du coffre (sans schéma) ?
fn is_vault_key(key: &str) -> bool {
    match key.find(':') {
        Some(i) => i == 0,
        None => true,
    }
}

/// Applique un renommage `old_base → new_base` à une clé (elle-même OU sous `old_base/`), sinon `None`.
fn move_key(key: &str, old_base: &str, new_base: &str) -> Option<String> {
    if key == old_base {
        return Some(new_base.to_string());
    }
    key.strip_prefix(&format!("{old_base}/"))
        .map(|rest| format!("{new_base}/{rest}"))
}

/// `key` est-elle `base` ou sous `base/` ?
fn is_under(key: &str, base: &str) -> bool {
    key == base || key.starts_with(&format!("{base}/"))
}

pub struct ArtefactManager {
    /// Le catalogue des kinds — le comportement, contribué par les plugins (type → kind, propriétaire).
    kinds: BTreeMap<String, (ArtefactKind, String)>,

    /// L'appareil et la session : l'écrivain `k3f9-b71` (§13).
    pub device_id: String,
    pub session: String,
    pub writer_id: String,

    /// Vrai quand l'index reflète le journal (fin du démarrage, figure 31).
    ready: bool,
    loaded: bool,

    vault_files: Rc<dyn VaultFiles>,
    now: Now,
    clock: HybridClock,
    base: String,
    app_version: String,
    adapters: PerJournal<SharedFs>,
    objects: PerJournal<Rc<ObjectStore>>,
    readers: PerJournal<JournalReader>,
    buffer: WriteBuffer,
    cache: SharedCache,

    /// id → record plié (vivants ET tombés : une tombe se garde).
    index: Index,
    /// Index inversés : la requête par document / collection en O(résultats).
    by_path: HashMap<String, BTreeSet<String>>,
    by_collection: HashMap<String, BTreeSet<String>>,
    /// Les curseurs de segments (clé `vault:<chemin>` / `local:<chemin>`).
    cursors: BTreeMap<String, SegmentCursor>,
    /// Taille et mtime des documents qui portent des artefacts (rattachement).
    docs: BTreeMap<String, DocInfo>,
    /// Les transactions en avance de plus de 5 minutes, gardées non pliées.
    quarantine: Vec<QuarantinedTx>,
    warned_clocks: HashSet<String>,
    quarantine_check_at: Option<i64>,

    subs: Vec<Subscription>,
    next_sub: u64,
    clock_skew_listeners: Vec<ClockSkewFn>,
    /// vue (par son id) → sa WidgetLayer partagée (montée par le calque `artefacts`) + son éditeur.
    surfaces: HashMap<String, Surface>,

    /// Un segment n'a pas pu être lu cette session : aucun balayage d'objets (§15.10).
    gc_blocked: bool,
    /// La dernière estampille émise, à mettre en cache au prochain commit.
    last_emitted: Option<Stamp>,
    /// Les 200 ms de calme par fichier signalé par le watcher.
    tail_timers: BTreeMap<String, i64>,
    /// La maintenance au repos, après `load`.
    maintenance_at: Option<i64>,
    /// Les `console.warn`/`console.error` du TS, lisibles par l'hôte (et les tests).
    diagnostics: Vec<Diagnostic>,
}

impl ArtefactManager {
    /// `vault_fs` : le DataAdapter du coffre ; `config_dir` : `.fragment` (le
    /// journal vit sous `<config_dir>/artefacts`) ; `vault_files` : les fichiers du coffre.
    pub fn new(
        vault_fs: SharedFs,
        config_dir: &str,
        vault_files: Rc<dyn VaultFiles>,
        opts: ArtefactManagerOptions,
    ) -> Self {
        let now = opts.now.unwrap_or_else(system_now);
        let base = format!("{config_dir}/artefacts");
        let user_data_dir = opts.user_data_dir.as_deref();
        let vault_path = opts.vault_path.as_deref();

        let device_id = opts
            .device_id
            .unwrap_or_else(|| read_device_id(user_data_dir));
        let session = opts.session.unwrap_or_else(|| random_id(3));
        let writer_id = format!("{device_id}-{session}");
        let clock = HybridClock::new(&writer_id, now.clone());

        let local_dir = local_root(user_data_dir, vault_path);
        let local: SharedFs = opts.local.unwrap_or_else(|| match &local_dir {
            Some(dir) => Rc::new(StdFs::new(dir.clone())),
            None => Rc::new(MemoryFs::new(now.clone())),
        });
        let adapters = PerJournal {
            vault: vault_fs,
            local,
        };
        // Le journal local a la même forme, enraciné hors du coffre : `<racine>/journal/…`.
        let bases = PerJournal {
            vault: base.clone(),
            local: String::new(),
        };
        let objects = PerJournal {
            vault: Rc::new(ObjectStore::new(adapters.vault.clone(), &bases.vault)),
            local: Rc::new(ObjectStore::new(adapters.local.clone(), &bases.local)),
        };
        let readers = PerJournal {
            vault: JournalReader::new(adapters.vault.clone(), &bases.vault, JournalKind::Vault),
            local: JournalReader::new(adapters.local.clone(), &bases.local, JournalKind::Local),
        };
        let mk = |journal: JournalKind| {
            SegmentWriter::new(SegmentWriterOptions {
                fs: adapters.get(journal).clone(),
                base: bases.get(journal),
                journal,
                device_id: &device_id,
                session: &session,
                app: &opts.app_version,
                now: now.clone(),
            })
        };
        let writers = PerJournal {
            vault: mk(JournalKind::Vault),
            local: mk(JournalKind::Local),
        };
        let buffer = WriteBuffer::new(
            writers,
            PerJournal {
                vault: objects.vault.clone(),
                local: objects.local.clone(),
            },
            now.clone(),
        );
        let cache: SharedCache = opts.cache.unwrap_or_else(|| Rc::new(NullJournalCache));

        // Les documents bougent sous nos pieds (§15.8) — fichiers du coffre ET
        // documents des plugins : l'app relaie le flux de `app.documents` par
        // `on_document_event`, et le watcher par `on_raw`. La fermeture et la perte
        // du focus (§15.1) sont branchées par le calque (ArtefactsPlugin) : le cœur
        // ne dépend pas du DOM (invariant 10).
        ArtefactManager {
            kinds: BTreeMap::new(),
            device_id,
            session,
            writer_id,
            ready: false,
            loaded: false,
            vault_files,
            now,
            clock,
            base,
            app_version: opts.app_version,
            adapters,
            objects,
            readers,
            buffer,
            cache,
            index: Index::new(),
            by_path: HashMap::new(),
            by_collection: HashMap::new(),
            cursors: BTreeMap::new(),
            docs: BTreeMap::new(),
            quarantine: Vec::new(),
            warned_clocks: HashSet::new(),
            quarantine_check_at: None,
            subs: Vec::new(),
            next_sub: 1,
            clock_skew_listeners: Vec::new(),
            surfaces: HashMap::new(),
            gc_blocked: false,
            last_emitted: None,
            tail_timers: BTreeMap::new(),
            maintenance_at: None,
            diagnostics: Vec::new(),
        }
    }

    fn now(&self) -> i64 {
        (self.now)()
    }

    fn warn(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            error: false,
            message,
        });
    }

    fn error(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            error: true,
            message,
        });
    }

    /// Les diagnostics accumulés (les `console.warn`/`console.error` du TS), vidés.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Écrire (§15.1) — tout passe par dispatch (R1).
    // ═══════════════════════════════════════════════════════════════════════

    /// Affecte des champs d'un record (le crée si `id` est `None`) ; rend l'id.
    /// Synchrone : l'index est à jour avant le retour de l'appel.
    pub fn set(&mut self, id: Option<&str>, fields: Fields, origin: Origin) -> String {
        let rid = id.map(str::to_string).unwrap_or_else(random_uuid);
        let op = self.op_for(&rid, fields);
        self.dispatch(vec![op], origin);
        rid
    }

    /// Pose une tombe : une intention écrite et datée (R5).
    pub fn del(&mut self, id: &str, origin: Origin) {
        self.dispatch(vec![Op::del(id)], origin);
    }

    /// Une transaction : estampillée, pliée, notifiée, mise en attente d'écriture.
    pub fn dispatch(&mut self, ops: Vec<Op>, origin: Origin) -> Tx {
        let tx = Tx {
            t: self.clock.next(),
            o: origin,
            ops,
        };
        self.last_emitted = Some(tx.t.clone());
        let changes = self.apply(&tx);

        // Chaque op part dans le journal de son record : coffre, ou appareil (§14.1).
        let mut local = Vec::new();
        let mut vault = Vec::new();
        for op in &tx.ops {
            if self.index.get(op.id()).is_some_and(|r| r.local) {
                local.push(op.clone());
            } else {
                vault.push(op.clone());
            }
        }
        if !vault.is_empty() {
            self.buffer.push(
                Tx {
                    ops: vault,
                    ..tx.clone()
                },
                JournalKind::Vault,
            );
        }
        if !local.is_empty() {
            self.buffer.push(
                Tx {
                    ops: local,
                    ..tx.clone()
                },
                JournalKind::Local,
            );
        }

        self.notify(&tx, &changes);
        tx
    }

    /// Écrit le tampon sur le disque, puis le cache. Rend quand tout est écrit.
    pub fn flushed(&mut self) {
        match self.buffer.flushed() {
            Ok(r) => self.on_flushed(r),
            Err(e) => self.error(format!("[artefacts] écriture du journal en échec : {e}")),
        }
    }

    /// La fenêtre se ferme : lignes restantes et ligne de fermeture.
    pub fn close(&mut self) {
        for e in self.buffer.close() {
            self.warn(format!("[artefacts] fermeture du segment en échec : {e}"));
        }
    }

    /// Sert les échéances passées (le rôle des minuteries du TS) : vidage du
    /// tampon, queue des segments d'autres écrivains, quarantaine, maintenance au
    /// repos. À appeler régulièrement (toutes les 50 ms suffisent).
    pub fn poll(&mut self) {
        let now = self.now();
        if self.buffer.due(now) {
            self.flushed();
        }
        let due: Vec<String> = self
            .tail_timers
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(p, _)| p.clone())
            .collect();
        for path in due {
            self.tail_timers.remove(&path);
            let is_object =
                path.starts_with(&format!("{}/objects/", self.base)) && path.ends_with(".json");
            if is_object {
                self.on_object_arrived(&path);
            } else {
                self.tail(&path);
            }
        }
        if self.quarantine_check_at.is_some_and(|at| at <= now) {
            self.quarantine_check_at = if self.quarantine.is_empty() {
                None
            } else {
                Some(now + QUARANTINE_CHECK_MS)
            };
            self.check_quarantine();
        }
        if self.maintenance_at.is_some_and(|at| at <= now) {
            self.maintenance_at = None;
            self.maintain(false);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Lire
    // ═══════════════════════════════════════════════════════════════════════

    /// Le record vivant (synchrone), ou `None`. Un gros state est à lire par `state()`.
    pub fn get(&self, id: &str) -> Option<Rec> {
        self.index.get(id).filter(|r| alive(r)).cloned()
    }

    /// Le contenu d'un record, chargé de son objet si besoin. `None` si inconnu ou objet absent.
    pub fn state(&self, id: &str) -> Option<Value> {
        let r = self.index.get(id).filter(|r| alive(r))?;
        match (&r.state, &r.head) {
            (Some(s), _) => Some(s.clone()),
            (None, None) => None,
            (None, Some(head)) => self.objects.get(Self::journal_of(r)).get(head),
        }
    }

    fn journal_of(r: &Rec) -> JournalKind {
        if r.local {
            JournalKind::Local
        } else {
            JournalKind::Vault
        }
    }

    /// Les records vivants qui correspondent au filtre — synchrone, O(résultats).
    pub fn list(&self, filter: Option<&Filter>) -> Vec<Rec> {
        let ids: Vec<String> = match filter {
            Some(Filter {
                scope: Some(ScopeFilter { path: Some(p), .. }),
                ..
            }) => self
                .by_path
                .get(p)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
            Some(Filter {
                scope:
                    Some(ScopeFilter {
                        collection: Some(c),
                        ..
                    }),
                ..
            }) => self
                .by_collection
                .get(c)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
            Some(Filter { id: Some(id), .. }) => vec![id.clone()],
            _ => self.index.keys().cloned().collect(),
        };
        ids.iter()
            .filter_map(|id| self.index.get(id))
            .filter(|r| alive(r) && matches(r, filter))
            .cloned()
            .collect()
    }

    /// L'historique d'un record : les transactions qui l'ont touché, dans la
    /// limite de la rétention (ce que la compaction a laissé), triées.
    pub fn history(&self, id: &str) -> Vec<Tx> {
        let mut out = Vec::new();
        for journal in JournalKind::ALL {
            let reader = self.readers.get(journal);
            for path in reader.list_segments() {
                for tx in reader.read_all(&path).unwrap_or_default() {
                    let ops: Vec<Op> = tx.ops.iter().filter(|op| op.id() == id).cloned().collect();
                    if !ops.is_empty() {
                        out.push(Tx { ops, ..tx });
                    }
                }
            }
        }
        // Une ligne relue en double (compaction interrompue) n'apparaît qu'une fois.
        out.sort_by(|a, b| a.t.cmp(&b.t));
        let mut seen = HashSet::new();
        out.retain(|tx| {
            seen.insert(format!(
                "{}|{}",
                tx.t,
                serde_json::to_string(&tx.ops).unwrap_or_default()
            ))
        });
        out
    }

    /// Combien de lignes de transaction ont été lues du disque cette session (e2e).
    pub fn lines_read(&self) -> usize {
        self.readers.vault.lines_read() + self.readers.local.lines_read()
    }

    /// L'index reflète-t-il le journal (le `ready` du TS) ?
    pub fn ready(&self) -> bool {
        self.ready
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Suivre
    // ═══════════════════════════════════════════════════════════════════════

    /// Un rappel pour chaque transaction qui touche un record du filtre, d'où
    /// qu'elle vienne (geste local, autre fenêtre, autre appareil). Rend le jeton du retrait.
    pub fn subscribe(
        &mut self,
        filter: Filter,
        cb: impl FnMut(&Tx, &[Change]) + 'static,
    ) -> SubscriptionId {
        let id = self.next_sub;
        self.next_sub += 1;
        self.subs.push(Subscription {
            id,
            filter,
            cb: Box::new(cb),
        });
        SubscriptionId(id)
    }

    pub fn unsubscribe(&mut self, id: SubscriptionId) {
        self.subs.retain(|s| s.id != id.0);
    }

    /// Prévenu qu'un appareil a une horloge en avance (l'événement `clock-skew` ;
    /// le calque montre une Notice — le cœur reste sans DOM).
    pub fn on_clock_skew(&mut self, cb: impl FnMut(&str) + 'static) {
        self.clock_skew_listeners.push(Box::new(cb));
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Kinds et calque partagé
    // ═══════════════════════════════════════════════════════════════════════

    /// Le mariage — appelé par `Plugin.registerArtefactKind`. Rend vrai si le type était libre.
    pub fn register_kind(&mut self, kind: ArtefactKind, owner: &str) -> bool {
        let type_ = kind.type_.clone();
        if self.kinds.contains_key(&type_) {
            return false;
        }
        self.kinds.insert(type_, (kind, owner.to_string()));
        true
    }

    pub fn unregister_kind(&mut self, type_: &str) {
        self.kinds.remove(type_);
    }

    pub fn kind(&self, type_: &str) -> Option<&ArtefactKind> {
        self.kinds.get(type_).map(|(k, _)| k)
    }

    /// Les kinds enregistrés, par type.
    pub fn kinds(&self) -> impl Iterator<Item = &ArtefactKind> {
        self.kinds.values().map(|(k, _)| k)
    }

    /// Le calque `artefacts` déclare sa WidgetLayer partagée pour une vue (par son id).
    pub fn register_surface(&mut self, view: &str, widgets: Rc<dyn Any>, has_editor: bool) {
        self.surfaces.insert(
            view.to_string(),
            Surface {
                widgets,
                has_editor,
            },
        );
    }

    /// Le retrait : seulement si c'est encore cette WidgetLayer qui est déclarée.
    pub fn unregister_surface(&mut self, view: &str, widgets: &Rc<dyn Any>) {
        if self
            .surfaces
            .get(view)
            .is_some_and(|s| Rc::ptr_eq(&s.widgets, widgets))
        {
            self.surfaces.remove(view);
        }
    }

    /// La WidgetLayer partagée d'une vue, ou `None` si le calque n'y est pas actif.
    pub fn surface_for(&self, view: &str) -> Option<&Surface> {
        self.surfaces.get(view)
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  La façade de l'ancienne API (§18) — au-dessus de dispatch.
    // ═══════════════════════════════════════════════════════════════════════

    /// `set(id, { type, scope, anchor, state })`.
    pub fn save(&mut self, spec: SaveSpec) -> ArtefactRef {
        let fields = Fields {
            type_: Some(spec.type_),
            v: spec.version,
            state: Some(spec.state),
            scope: Some(spec.scope),
            anchor: spec.anchor.map(Some),
            local: false,
        };
        ArtefactRef {
            id: self.set(spec.id.as_deref(), fields, Origin::Plugin),
        }
    }

    /// `set(id, champs)`.
    pub fn update(&mut self, id: &str, patch: SavePatch) {
        if self.get(id).is_none() {
            return;
        }
        let fields = Fields {
            type_: patch.type_,
            v: patch.version,
            state: patch.state,
            scope: patch.scope,
            anchor: patch.anchor,
            local: false,
        };
        self.set(Some(id), fields, Origin::Plugin);
    }

    /// `list({ scope })`.
    pub fn list_sync(&self, filter: Option<ScopeFilter>) -> Vec<Rec> {
        self.list(filter.map(Filter::scope).as_ref())
    }

    /// `del(id)`.
    pub fn delete(&mut self, id: &str) {
        if self.get(id).is_some() {
            self.del(id, Origin::Plugin);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Démarrer (§15.3)
    // ═══════════════════════════════════════════════════════════════════════

    /// Reconstruit l'index. `ready()` dit quand c'est fait, et les calques
    /// réconcilient à ce moment-là.
    ///
    /// Le coût ne dépend plus du nombre d'artefacts : un segment scellé et déjà lu
    /// ne coûte rien, pas même un stat. Il dépend du nombre d'appareils et de ce qui
    /// a été écrit depuis la dernière session.
    pub fn load(&mut self) {
        // 3-4. Records et curseurs : une lecture.
        if self.cache.open() {
            let snap = self.cache.load();
            for r in snap.records {
                match self.index.get_mut(&r.id) {
                    Some(cur) => merge_rec(cur, &r),
                    None => {
                        self.index.insert(r.id.clone(), r.clone());
                    }
                }
                self.reindex(&r.id, None);
            }
            for c in snap.segments {
                self.cursors.insert(c.key.clone(), c);
            }
            for d in snap.docs {
                self.docs.insert(d.path.clone(), d);
            }
            self.quarantine = snap.quarantine;
            if let Some(t) = snap.last_stamp {
                self.clock.observe(&t);
            }
        }

        // La migration depuis les refs (§18), une seule fois.
        if let Err(e) = import_legacy_refs(&self.adapters.vault, &self.base) {
            self.error(format!(
                "[artefacts] import des refs en échec — refs/ laissé en place : {e}"
            ));
        }

        // 5-10. Chaque journal : ne lire que ce qui est nouveau.
        let mut dirty = BTreeSet::new();
        let mut batch = CacheBatch::default();
        for journal in JournalKind::ALL {
            self.scan_journal(journal, &mut dirty, &mut batch, false);
        }
        self.release_quarantine(&mut dirty, false);
        batch.records = self.records_for(dirty.iter());
        batch.quarantine = Some(self.quarantine.clone());
        self.commit(batch);

        self.loaded = true;
        // 11. ready : les calques réconcilient.
        self.ready = true;

        self.refresh_docs();
        if !self.quarantine.is_empty() {
            self.arm_quarantine();
        }
        // 12. Au repos : compaction, balayage avec délai de grâce.
        self.maintenance_at = Some(self.now() + IDLE_DELAY_MS);
    }

    /// Parcourt les segments d'un journal et plie les lignes nouvelles. Un segment
    /// connu et scellé est ignoré sans stat ; un segment non scellé est statué, et
    /// relu s'il a grandi. `notify` : les lignes viennent d'un autre écrivain
    /// pendant la session (lecteur de queue) — les abonnés en sont prévenus.
    fn scan_journal(
        &mut self,
        journal: JournalKind,
        dirty: &mut BTreeSet<String>,
        batch: &mut CacheBatch,
        notify: bool,
    ) {
        let present: BTreeSet<String> = self
            .readers
            .get(journal)
            .list_segments()
            .into_iter()
            .collect();
        for path in &present {
            self.read_segment(journal, path, dirty, batch, notify);
        }
        // Les curseurs des segments disparus (compactés ailleurs) partent ; les records restent.
        let gone: Vec<String> = self
            .cursors
            .iter()
            .filter(|(_, c)| c.journal == journal && !present.contains(&c.path))
            .map(|(k, _)| k.clone())
            .collect();
        for key in gone {
            self.cursors.remove(&key);
            batch.delete_segments.push(key);
        }
    }

    /// Lit un segment depuis son curseur et plie les lignes nouvelles.
    fn read_segment(
        &mut self,
        journal: JournalKind,
        path: &str,
        dirty: &mut BTreeSet<String>,
        batch: &mut CacheBatch,
        notify: bool,
    ) {
        // Mon propre segment : ce que j'y écris est déjà plié.
        if self.buffer.writer(journal).is_mine(path) {
            return;
        }
        let key = cursor_key(journal, path);
        let cursor = self.cursors.get(&key).cloned();
        if cursor.as_ref().is_some_and(|c| c.sealed) {
            return; // scellé et déjà lu : ne coûte rien
        }
        if let Some(c) = &cursor {
            match self.readers.get(journal).size(path) {
                None => {
                    self.cursors.remove(&key);
                    batch.delete_segments.push(key);
                    return;
                }
                Some(size) if size == c.size => return,
                Some(_) => {}
            }
        }
        let Some(parsed) = self.readers.get(journal).read(path, cursor.as_ref()) else {
            self.warn(format!("[artefacts] segment illisible {path}"));
            self.gc_blocked = true; // un inventaire incomplet ne supprime rien
            return;
        };
        if parsed.unreadable > 0 {
            self.warn(format!(
                "[artefacts] {} ligne(s) illisible(s) ignorée(s) : {path}",
                parsed.unreadable
            ));
        }
        for tx in parsed.txs {
            self.ingest(tx, journal, dirty, notify);
        }
        self.cursors.insert(key, parsed.cursor.clone());
        batch.segments.push(parsed.cursor);
    }

    /// Une transaction lue sur le disque. En avance de plus de cinq minutes : en
    /// quarantaine (gardée, non pliée, signalée). Sinon : l'horloge l'observe, le
    /// pli l'applique.
    fn ingest(
        &mut self,
        mut tx: Tx,
        journal: JournalKind,
        dirty: &mut BTreeSet<String>,
        notify: bool,
    ) {
        if self.clock.is_future(&tx.t) {
            if !self
                .quarantine
                .iter()
                .any(|q| q.tx.t == tx.t && q.journal == journal)
            {
                let t = tx.t.clone();
                self.quarantine.push(QuarantinedTx { tx, journal });
                self.warn_clock(&t);
                self.arm_quarantine();
            }
            return;
        }
        self.clock.observe(&tx.t);
        // Un record lu dans le journal local appartient à l'appareil.
        if journal == JournalKind::Local {
            for op in &mut tx.ops {
                if let Op::Set(s) = op {
                    s.local = true;
                }
            }
        }
        let changes = self.apply(&tx);
        for c in &changes {
            dirty.insert(c.id.clone());
        }
        if notify {
            let disk = Tx {
                o: Origin::Disk,
                ..tx
            };
            self.notify(&disk, &changes);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Le pli et l'index
    // ═══════════════════════════════════════════════════════════════════════

    /// Plie une transaction ; tient les index inversés ; rend ce qui a changé.
    fn apply(&mut self, tx: &Tx) -> Vec<Change> {
        let mut ids: Vec<String> = Vec::new();
        for op in &tx.ops {
            if !ids.iter().any(|i| i == op.id()) {
                ids.push(op.id().to_string());
            }
        }
        let before: Vec<Option<Rec>> = ids.iter().map(|id| self.index.get(id).cloned()).collect();
        fold(&mut self.index, tx);
        let mut changes = Vec::with_capacity(ids.len());
        for (id, prev) in ids.into_iter().zip(before) {
            let Some(next) = self.index.get(&id).cloned() else {
                continue;
            };
            self.reindex(&id, prev.as_ref());
            changes.push(Change {
                id: id.clone(),
                before: prev.filter(alive),
                after: Some(next.clone()).filter(alive),
            });
            if let Some(ArtefactScope::File { path }) = &next.scope {
                self.touch_doc(&path.clone());
            }
        }
        changes
    }

    /// Remet un record dans les index inversés (après un changement de scope).
    fn reindex(&mut self, id: &str, prev: Option<&Rec>) {
        let ps = prev.and_then(|p| p.scope.clone());
        let ns = self.index.get(id).and_then(|r| r.scope.clone());
        if ps.is_some() && ps == ns {
            return;
        }
        match &ps {
            Some(ArtefactScope::File { path }) => remove_from(&mut self.by_path, path, id),
            Some(ArtefactScope::Plugin { collection }) => {
                remove_from(&mut self.by_collection, collection, id)
            }
            _ => {}
        }
        match &ns {
            Some(ArtefactScope::File { path }) => add_to(&mut self.by_path, path, id),
            Some(ArtefactScope::Plugin { collection }) => {
                add_to(&mut self.by_collection, collection, id)
            }
            _ => {}
        }
    }

    /// L'op d'un `set` : complète le groupe contenu à partir du record courant.
    ///
    /// ★ `set(id)` sans type connu : le TS lève ; ici l'op part sans groupe contenu
    ///   (un `set` qui ne touche que scope/ancre) et un diagnostic est noté.
    fn op_for(&mut self, id: &str, fields: Fields) -> Op {
        let cur = self.index.get(id).cloned();
        let mut op = SetOp::new(id);
        if fields.type_.is_some() || fields.v.is_some() || fields.state.is_some() {
            let type_ = fields
                .type_
                .clone()
                .or_else(|| cur.as_ref().and_then(|c| c.type_.clone()));
            match type_ {
                None => self.error(format!("[artefacts] set({id}) : type inconnu")),
                Some(type_) => {
                    let changed_type = fields.type_.is_some()
                        && fields.type_ != cur.as_ref().and_then(|c| c.type_.clone());
                    let kind_version = self.kinds.get(&type_).and_then(|(k, _)| k.version);
                    op.v = Some(
                        fields
                            .v
                            .or(if changed_type {
                                None
                            } else {
                                cur.as_ref().and_then(|c| c.v)
                            })
                            .or(kind_version)
                            .unwrap_or(1),
                    );
                    op.type_ = Some(type_);
                    if fields.state.is_some() {
                        op.state = fields.state;
                    } else if let Some(c) = cur
                        .as_ref()
                        .filter(|c| c.head.is_some() && c.state.is_none())
                    {
                        op.head = c.head.clone();
                    } else {
                        op.state = cur.as_ref().and_then(|c| c.state.clone());
                    }
                }
            }
        }
        if fields.scope.is_some() {
            op.scope = fields.scope;
        } else if cur.is_none() && op.type_.is_some() {
            op.scope = Some(ArtefactScope::Global); // création sans scope
        }
        if fields.anchor.is_some() {
            op.anchor = fields.anchor;
        }
        if cur.is_none() && fields.local {
            op.local = true;
        }
        Op::Set(op)
    }

    /// Prévient les abonnés concernés.
    fn notify(&mut self, tx: &Tx, changes: &[Change]) {
        if changes.is_empty() {
            return;
        }
        let mut subs = std::mem::take(&mut self.subs);
        for sub in subs.iter_mut() {
            let mine: Vec<Change> = changes
                .iter()
                .filter(|c| {
                    let raw = self.index.get(&c.id);
                    c.before
                        .as_ref()
                        .is_some_and(|b| matches(b, Some(&sub.filter)))
                        || c.after
                            .as_ref()
                            .is_some_and(|a| matches(a, Some(&sub.filter)))
                        || raw.is_some_and(|r| matches(r, Some(&sub.filter)))
                })
                .cloned()
                .collect();
            if mine.is_empty() {
                continue;
            }
            (sub.cb)(tx, &mine);
        }
        // Les abonnements posés pendant la notification restent.
        subs.extend(std::mem::take(&mut self.subs));
        self.subs = subs;
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Le disque suit (§15.1, étapes 7-10)
    // ═══════════════════════════════════════════════════════════════════════

    fn on_flushed(&mut self, r: FlushResult) {
        // Un gros state est parti en objet : le record cite désormais l'empreinte.
        for h in &r.heads {
            if let Some(rec) = self.index.get_mut(&h.id)
                && rec.content_t.as_deref() == Some(h.t.as_str())
            {
                rec.head = Some(h.head.clone());
                let journal = Self::journal_of(rec);
                if let Some(state) = rec.state.clone() {
                    self.objects.get(journal).remember(&h.head, state);
                }
            }
        }
        let mut ids = BTreeSet::new();
        for p in &r.written {
            for op in &p.tx.ops {
                ids.insert(op.id().to_string());
            }
        }
        for c in &r.cursors {
            self.cursors.insert(c.key.clone(), c.clone());
        }
        if r.written.is_empty() && r.cursors.is_empty() {
            return;
        }
        let batch = CacheBatch {
            records: self.records_for(ids.iter()),
            segments: r.cursors,
            last_stamp: self.last_emitted.clone(),
            docs: self.docs.values().cloned().collect(),
            ..Default::default()
        };
        self.commit(batch);
    }

    /// Les records à mettre en cache : un gros state ne voyage que par son empreinte.
    fn records_for<'a>(&self, ids: impl IntoIterator<Item = &'a String>) -> Vec<Rec> {
        let mut out = Vec::new();
        for id in ids {
            let Some(r) = self.index.get(id) else {
                continue;
            };
            if r.head.is_some() && r.state.is_some() {
                out.push(Rec {
                    state: None,
                    ..r.clone()
                });
            } else if r.state.as_ref().is_some_and(|s| !fits_inline(s)) {
                // pas encore parti en objet : le cache le prendra au prochain vidage
                continue;
            } else {
                out.push(r.clone());
            }
        }
        out
    }

    /// Un lot du cache. Un échec du cache ne casse rien (il est jetable).
    fn commit(&mut self, batch: CacheBatch) {
        if let Err(e) = self.cache.commit(batch) {
            self.warn(format!("[artefacts] cache non écrit : {e}"));
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Un changement arrive d'ailleurs (§15.4)
    // ═══════════════════════════════════════════════════════════════════════

    /// Le watcher signale un chemin du coffre (`vault.on('raw')`).
    pub fn on_raw(&mut self, path: &str) {
        if !self.loaded {
            return; // le démarrage lira tout de toute façon
        }
        let is_object =
            path.starts_with(&format!("{}/objects/", self.base)) && path.ends_with(".json");
        // 4. Dans journal/, et pas mon segment ?
        if !is_object
            && (!self.readers.vault.is_segment(path)
                || self.buffer.writer(JournalKind::Vault).is_mine(path))
        {
            return;
        }
        // 5. 200 ms de calme sur ce fichier (servies par `poll`).
        self.tail_timers
            .insert(path.to_string(), self.now() + TAIL_CALM_MS);
    }

    /// 6-8. stat, read, plie les lignes nouvelles (origine disk), abonnés.
    fn tail(&mut self, path: &str) {
        let mut dirty = BTreeSet::new();
        let mut batch = CacheBatch::default();
        self.read_segment(JournalKind::Vault, path, &mut dirty, &mut batch, true);
        if !dirty.is_empty() || !batch.segments.is_empty() || !batch.delete_segments.is_empty() {
            batch.records = self.records_for(dirty.iter());
            batch.quarantine = Some(self.quarantine.clone());
            self.commit(batch);
        }
    }

    /// Un objet arrivé après la ligne qui le cite : les records qui l'attendaient sont prévenus.
    fn on_object_arrived(&mut self, path: &str) {
        let sha = path
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".json"))
            .unwrap_or("")
            .to_string();
        let changes: Vec<Change> = self
            .index
            .values()
            .filter(|r| r.head.as_deref() == Some(sha.as_str()) && alive(r))
            .map(|r| Change {
                id: r.id.clone(),
                before: Some(r.clone()),
                after: Some(r.clone()),
            })
            .collect();
        if let Some(t) = changes
            .first()
            .and_then(|c| c.after.as_ref())
            .and_then(|a| a.content_t.clone())
        {
            let tx = Tx {
                t,
                o: Origin::Disk,
                ops: Vec::new(),
            };
            self.notify(&tx, &changes);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Quarantaine (§14.3)
    // ═══════════════════════════════════════════════════════════════════════

    /// Plie ce que l'heure a rattrapé.
    fn release_quarantine(&mut self, dirty: &mut BTreeSet<String>, notify: bool) {
        if self.quarantine.is_empty() {
            return;
        }
        let (mut due, kept): (Vec<QuarantinedTx>, Vec<QuarantinedTx>) =
            std::mem::take(&mut self.quarantine)
                .into_iter()
                .partition(|q| !self.clock.is_future(&q.tx.t));
        self.quarantine = kept;
        due.sort_by(|a, b| a.tx.t.cmp(&b.tx.t));
        for q in due {
            self.ingest(q.tx, q.journal, dirty, notify);
        }
    }

    fn arm_quarantine(&mut self) {
        if self.quarantine_check_at.is_none() {
            self.quarantine_check_at = Some(self.now() + QUARANTINE_CHECK_MS);
        }
    }

    /// Le minuteur de quarantaine (public pour les tests à horloge injectée).
    pub fn check_quarantine(&mut self) {
        let mut dirty = BTreeSet::new();
        let before = self.quarantine.len();
        self.release_quarantine(&mut dirty, true);
        if self.quarantine.len() != before {
            let batch = CacheBatch {
                records: self.records_for(dirty.iter()),
                quarantine: Some(self.quarantine.clone()),
                ..Default::default()
            };
            self.commit(batch);
        }
        if self.quarantine.is_empty() {
            self.quarantine_check_at = None;
        }
    }

    /// Les transactions en quarantaine (diagnostic).
    pub fn quarantined(&self) -> Vec<Tx> {
        self.quarantine.iter().map(|q| q.tx.clone()).collect()
    }

    /// L'utilisateur est prévenu qu'un appareil a une heure fausse — une fois par écrivain.
    fn warn_clock(&mut self, t: &str) {
        let writer = stamp_writer(t).to_string();
        if !self.warned_clocks.insert(writer.clone()) {
            return;
        }
        self.warn(format!(
            "[artefacts] l'appareil « {writer} » a une horloge en avance : ses changements attendent en quarantaine."
        ));
        // Le calque prévient l'utilisateur (Notice) : le cœur reste sans DOM.
        let mut listeners = std::mem::take(&mut self.clock_skew_listeners);
        for cb in listeners.iter_mut() {
            cb(&writer);
        }
        listeners.extend(std::mem::take(&mut self.clock_skew_listeners));
        self.clock_skew_listeners = listeners;
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Le document est renommé ou disparaît (§15.8)
    // ═══════════════════════════════════════════════════════════════════════

    /// Le flux des documents de l'app (`app.documents` : fichiers du coffre relayés
    /// par le Vault ET documents des plugins — une conversation supprimée enterre ses fils).
    pub fn on_document_event(&mut self, e: &DocumentEvent) {
        match e {
            DocumentEvent::Rename {
                key,
                old_key,
                origin,
            } => self.on_doc_rename(key, old_key, *origin),
            DocumentEvent::Delete { key, origin } => self.on_doc_delete(key, *origin),
            DocumentEvent::Create { key } => self.on_doc_create(key),
            DocumentEvent::Modify { key } => self.on_doc_modify(key),
        }
    }

    /// Les ids des records scopés sous `base` (le document, ou un dossier entier).
    fn ids_under(&self, base: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (path, ids) in &self.by_path {
            if is_under(path, base) {
                out.extend(ids.iter().cloned());
            }
        }
        out
    }

    /// Renommé dans l'app : une transaction, N ops `set scope`. Origine `vault`
    /// pour un fichier, `plugin` pour un document de plugin (aucune origine
    /// nouvelle au journal : un client ancien les lit tels quels).
    fn on_doc_rename(&mut self, new_key: &str, old_key: &str, origin: Option<EventOrigin>) {
        if origin == Some(EventOrigin::Disk) || new_key == old_key {
            return;
        }
        let mut ops = Vec::new();
        for id in self.ids_under(old_key) {
            let Some(ArtefactScope::File { path }) =
                self.index.get(&id).and_then(|r| r.scope.clone())
            else {
                continue;
            };
            if let Some(moved) = move_key(&path, old_key, new_key) {
                ops.push(Op::Set(SetOp::new(&id).scope(ArtefactScope::file(moved))));
            }
        }
        for (path, d) in std::mem::take(&mut self.docs) {
            match move_key(&path, old_key, new_key) {
                Some(moved) => {
                    self.docs
                        .insert(moved.clone(), DocInfo { path: moved, ..d });
                }
                None => {
                    self.docs.insert(path, d);
                }
            }
        }
        if !ops.is_empty() {
            let origin = if is_vault_key(old_key) {
                Origin::Vault
            } else {
                Origin::Plugin
            };
            self.dispatch(ops, origin);
        }
    }

    /// Supprimé dans l'app (origine user) : une transaction, N tombes, restaurables
    /// pendant la rétention. Disparu hors de l'app (disk) : AUCUNE écriture — les
    /// records deviennent orphelins, gardés, et se rattachent si le fichier revient.
    /// Même règle pour tout document : rien de propre à un plugin.
    fn on_doc_delete(&mut self, key: &str, origin: Option<EventOrigin>) {
        if origin != Some(EventOrigin::User) {
            return; // le store ne devine pas (R5)
        }
        let ops: Vec<Op> = self
            .ids_under(key)
            .into_iter()
            .filter(|id| self.index.get(id).is_some_and(alive))
            .map(Op::del)
            .collect();
        if !ops.is_empty() {
            let origin = if is_vault_key(key) {
                Origin::Vault
            } else {
                Origin::Plugin
            };
            self.dispatch(ops, origin);
        }
    }

    /// Un fichier apparaît. S'il a la taille et le mtime d'un document devenu
    /// introuvable, c'est un renommage fait hors de l'app : le cœur écrit les `set
    /// scope` correspondants. La SEULE inférence du design ; elle rattache, elle ne
    /// détruit pas, et elle se défait par l'historique.
    fn on_doc_create(&mut self, key: &str) {
        // Le rattachement par taille + mtime ne vaut que pour les fichiers du coffre.
        if !self.loaded || !is_vault_key(key) {
            return;
        }
        let Some(stat) = self.vault_files.file_stat(key) else {
            return;
        };
        if self.by_path.contains_key(key) {
            self.touch_doc(key);
            return;
        }
        self.rattach(key, stat);
    }

    fn on_doc_modify(&mut self, key: &str) {
        if is_vault_key(key) && self.docs.contains_key(key) {
            self.touch_doc(key);
        }
    }

    /// Tient à jour taille et mtime d'un document qui porte des artefacts (s'il est là).
    fn touch_doc(&mut self, path: &str) {
        let Some(stat) = self.vault_files.file_stat(path) else {
            return;
        };
        if self
            .docs
            .get(path)
            .is_some_and(|prev| prev.size == stat.size && prev.mtime == stat.mtime)
        {
            return;
        }
        self.docs.insert(
            path.to_string(),
            DocInfo {
                path: path.to_string(),
                size: stat.size,
                mtime: stat.mtime,
            },
        );
    }

    /// Rattache les records d'un document introuvable de même taille et mtime à `file`.
    fn rattach(&mut self, file: &str, stat: FileStats) -> bool {
        let candidates: Vec<(String, DocInfo)> = self
            .docs
            .iter()
            .map(|(p, d)| (p.clone(), d.clone()))
            .collect();
        for (path, d) in candidates {
            if path == file || self.vault_files.file_stat(&path).is_some() {
                continue;
            }
            if d.size != stat.size || d.mtime != stat.mtime {
                continue;
            }
            let ops: Vec<Op> = self
                .ids_under(&path)
                .into_iter()
                .filter(|id| {
                    self.index.get(id).is_some_and(|r| {
                        alive(r) && matches!(&r.scope, Some(ArtefactScope::File { path: p }) if p == &path)
                    })
                })
                .map(|id| Op::Set(SetOp::new(&id).scope(ArtefactScope::file(file))))
                .collect();
            if ops.is_empty() {
                continue;
            }
            self.docs.remove(&path);
            self.docs.insert(
                file.to_string(),
                DocInfo {
                    path: file.to_string(),
                    ..d
                },
            );
            self.dispatch(ops, Origin::Vault);
            let batch = CacheBatch {
                docs: self.docs.values().cloned().collect(),
                delete_docs: vec![path],
                ..Default::default()
            };
            self.commit(batch);
            return true;
        }
        false
    }

    /// À `ready` : les documents des records sont relevés, et chaque document
    /// introuvable cherche un fichier de même taille et mtime qui ne porte rien.
    fn refresh_docs(&mut self) {
        let Some(files) = self.vault_files.files() else {
            return;
        };
        let paths: Vec<String> = self.by_path.keys().cloned().collect();
        for path in paths {
            self.touch_doc(&path);
        }
        let missing = self
            .docs
            .keys()
            .any(|p| self.vault_files.file_stat(p).is_none());
        if missing {
            for (path, stat) in files {
                if !self.by_path.contains_key(&path) {
                    self.rattach(&path, stat);
                }
            }
        }
        let batch = CacheBatch {
            docs: self.docs.values().cloned().collect(),
            ..Default::default()
        };
        self.commit(batch);
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Au repos : compaction (§15.9) et balayage des objets (§15.10)
    // ═══════════════════════════════════════════════════════════════════════

    /// La maintenance : compacter les segments de cet appareil, puis balayer les
    /// objets. `force` (la commande) ignore les seuils de déclenchement.
    pub fn maintain(&mut self, force: bool) {
        if !self.loaded {
            return;
        }
        self.flushed();
        for journal in JournalKind::ALL {
            if let Err(e) = self.compact(journal, force) {
                self.error(format!("[artefacts] maintenance en échec : {e}"));
                return;
            }
        }
        for journal in JournalKind::ALL {
            self.sweep(journal);
        }
    }

    /// Compacte les segments scellés (ou abandonnés depuis 24 h) du dossier de CET appareil.
    fn compact(&mut self, journal: JournalKind, force: bool) -> std::io::Result<()> {
        let reader = self.readers.get(journal);
        let adapter = self.adapters.get(journal).clone();
        let device_dir = format!("{}/{}", reader.journal_dir(), self.device_id);
        let all = reader.list_segments();
        let own: Vec<&String> = all
            .iter()
            .filter(|p| p.starts_with(&format!("{device_dir}/")))
            .collect();
        let active = self
            .buffer
            .writer(journal)
            .active_path()
            .map(str::to_string);

        let now = self.now();
        let mut total: u64 = 0;
        let mut eligible: Vec<String> = Vec::new();
        for path in &own {
            let Some(s) = adapter.stat(path) else {
                continue;
            };
            total += s.size;
            if active.as_deref() == Some(path.as_str()) {
                continue; // jamais un segment actif
            }
            let sealed = self
                .cursors
                .get(&cursor_key(journal, path))
                .is_some_and(|c| c.sealed);
            if sealed || now - s.mtime > ABANDONED_MS {
                eligible.push((*path).clone());
            }
        }
        if !force && own.len() <= COMPACT_MIN_SEGMENTS && total <= COMPACT_MIN_BYTES {
            return Ok(());
        }
        if eligible.is_empty() {
            return Ok(());
        }

        // Les dates de remplacement viennent de TOUS les écrivains de ce journal.
        let mut everything: Vec<Tx> = Vec::new();
        let mut sources: Vec<Tx> = Vec::new();
        for path in &all {
            let Some(txs) = reader.read_all(path) else {
                return Ok(()); // un segment illisible : on ne compacte pas sur un inventaire incomplet
            };
            if eligible.contains(path) {
                sources.extend(txs.iter().cloned());
            }
            everything.extend(txs);
        }
        let kept = compact_txs(&sources, &self.index, &content_stamps_of(&everything), now);
        let res = write_compacted(WriteCompacted {
            fs: &adapter,
            device_dir: &device_dir,
            sources: &eligible,
            kept: &kept,
            writer: &self.writer_id,
            session: &self.session,
            app: &self.app_version,
        })?;

        let mut batch = CacheBatch::default();
        for path in &eligible {
            let key = cursor_key(journal, path);
            self.cursors.remove(&key);
            batch.delete_segments.push(key);
        }
        if let Some(res) = res {
            let c = SegmentCursor {
                key: cursor_key(journal, &res.path),
                journal,
                sealed: true,
                lines: res.lines,
                size: res.size,
                objects: res.objects,
                path: res.path,
            };
            self.cursors.insert(c.key.clone(), c.clone());
            batch.segments.push(c);
        }
        self.commit(batch);
        Ok(())
    }

    /// Le balayage : un objet part quand plus aucune ligne présente sur le disque
    /// ne le cite, et que son fichier a plus de quatorze jours. Annulé pour la
    /// session si un seul segment n'a pas pu être lu.
    fn sweep(&mut self, journal: JournalKind) {
        if self.gc_blocked {
            return;
        }
        let mut cited: HashSet<String> = HashSet::new();
        for c in self.cursors.values().filter(|c| c.journal == journal) {
            cited.extend(c.objects.iter().cloned());
        }
        // Ce qui attend encore (quarantaine, records pas encore relus) reste cité.
        for q in &self.quarantine {
            for op in &q.tx.ops {
                if let Some(head) = op.as_set().and_then(|s| s.head.clone()) {
                    cited.insert(head);
                }
            }
        }
        for r in self.index.values() {
            if let Some(head) = &r.head
                && alive(r)
                && Self::journal_of(r) == journal
            {
                cited.insert(head.clone());
            }
        }
        self.objects.get(journal).sweep(&cited, self.now());
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Divers
    // ═══════════════════════════════════════════════════════════════════════

    /// L'index plié en entier, vivants et tombes (la commande `artefacts:dump`).
    pub fn dump(&self) -> Vec<Rec> {
        self.index.values().cloned().collect()
    }

    /// Le dossier du journal du coffre (`<config_dir>/artefacts`).
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Le système de fichiers d'un journal (le calque et les commandes en ont besoin).
    pub fn fs(&self, journal: JournalKind) -> &SharedFs {
        self.adapters.get(journal)
    }
}

impl std::fmt::Debug for ArtefactManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtefactManager")
            .field("writer_id", &self.writer_id)
            .field("ready", &self.ready)
            .field("records", &self.index.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
