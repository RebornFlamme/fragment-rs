// Port de app/src/ui/core/artefacts/ArtefactManager.test.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le manager du journal sans DOM ni Electron : un coffre en mémoire, une horloge
//  injectée, un cache en mémoire. Les preuves de la partie III, §19.3.
//
//  ★ Le « watcher » du TS (chaque écriture revient en `raw`) et ses `setTimeout`
//    deviennent : l'adaptateur espion note les chemins écrits, le test les rend
//    au manager par `on_raw`, avance l'horloge et appelle `poll()`.
// ═══════════════════════════════════════════════════════════════════════════

use super::*;
use crate::cache::MemoryJournalCache;
use crate::clock::{date_utc, format_stamp};
use crate::fs::{JournalFs, Listed, MemoryFs, Stat};
use crate::journal::import::{LegacyRef, import_file_content, import_path, legacy_line};
use crate::journal::reader::{empty_cursor, parse_segment};
use crate::objects::GRACE_MS;
use crate::types::{
    ArtefactScope, DurableAnchor, Fields, Filter, Origin, ScopeFilter, ScopeKind, WidgetAnchor,
};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::io;

fn t0() -> i64 {
    date_utc(2026, 8, 29, 17, 20)
}
const BASE: &str = ".fragment/artefacts";

/// Une horloge qu'on avance à la main.
#[derive(Clone)]
struct Clock(Rc<Cell<i64>>);
fn clock(start: i64) -> Clock {
    Clock(Rc::new(Cell::new(start)))
}
impl Clock {
    fn now(&self) -> Now {
        let c = self.0.clone();
        Rc::new(move || c.get())
    }
    fn advance(&self, ms: i64) {
        self.0.set(self.0.get() + ms);
    }
}

/// Un faux coffre : des fichiers de notes avec leur stat.
#[derive(Default)]
struct FakeVault {
    notes: RefCell<BTreeMap<String, FileStats>>,
}
impl FakeVault {
    fn add_note(&self, path: &str, size: u64, mtime: i64) {
        self.notes
            .borrow_mut()
            .insert(path.into(), FileStats { size, mtime });
    }
    fn remove_note(&self, path: &str) -> Option<FileStats> {
        self.notes.borrow_mut().remove(path)
    }
    fn rename_note(&self, from: &str, to: &str) {
        if let Some(s) = self.remove_note(from) {
            self.notes.borrow_mut().insert(to.into(), s);
        }
    }
}
impl VaultFiles for FakeVault {
    fn file_stat(&self, path: &str) -> Option<FileStats> {
        self.notes.borrow().get(path).copied()
    }
    fn files(&self) -> Option<Vec<(String, FileStats)>> {
        Some(
            self.notes
                .borrow()
                .iter()
                .map(|(p, s)| (p.clone(), *s))
                .collect(),
        )
    }
}

/// L'adaptateur du coffre, espionné : l'ordre des écritures, les chemins écrits
/// (le watcher), une lecture qu'on fait échouer.
struct SpyFs {
    inner: MemoryFs,
    log: RefCell<Vec<String>>,
    raw: RefCell<Vec<String>>,
    fail_read_suffix: RefCell<Option<String>>,
}
impl SpyFs {
    fn new(now: Now) -> Rc<Self> {
        Rc::new(SpyFs {
            inner: MemoryFs::new(now),
            log: RefCell::new(Vec::new()),
            raw: RefCell::new(Vec::new()),
            fail_read_suffix: RefCell::new(None),
        })
    }
    fn drain_raw(&self) -> Vec<String> {
        std::mem::take(&mut *self.raw.borrow_mut())
    }
}
impl std::ops::Deref for SpyFs {
    type Target = MemoryFs;
    fn deref(&self) -> &MemoryFs {
        &self.inner
    }
}
impl JournalFs for SpyFs {
    fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }
    fn stat(&self, path: &str) -> Option<Stat> {
        self.inner.stat(path)
    }
    fn list(&self, path: &str) -> Listed {
        self.inner.list(path)
    }
    fn read(&self, path: &str) -> io::Result<String> {
        if self
            .fail_read_suffix
            .borrow()
            .as_ref()
            .is_some_and(|s| path.ends_with(s))
        {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "EACCES"));
        }
        self.inner.read(path)
    }
    fn write(&self, path: &str, data: &str, mtime: Option<i64>) -> io::Result<()> {
        self.log.borrow_mut().push(format!("write {path}"));
        self.inner.write(path, data, mtime)?;
        self.raw.borrow_mut().push(path.to_string());
        Ok(())
    }
    fn append(&self, path: &str, data: &str) -> io::Result<()> {
        self.log.borrow_mut().push(format!("append {path}"));
        self.inner.append(path, data)?;
        self.raw.borrow_mut().push(path.to_string());
        Ok(())
    }
    fn mkdir(&self, path: &str) -> io::Result<()> {
        self.inner.mkdir(path)
    }
    fn rename(&self, path: &str, new_path: &str) -> io::Result<()> {
        self.inner.rename(path, new_path)
    }
    fn remove(&self, path: &str) -> io::Result<()> {
        self.inner.remove(path)
    }
}

struct Setup {
    m: ArtefactManager,
    vault: Rc<FakeVault>,
    adapter: Rc<SpyFs>,
    local: Rc<MemoryFs>,
    cache: Rc<MemoryJournalCache>,
}

#[derive(Default)]
struct MakeOpts {
    adapter: Option<Rc<SpyFs>>,
    vault: Option<Rc<FakeVault>>,
    local: Option<Rc<MemoryFs>>,
    cache: Option<Rc<MemoryJournalCache>>,
    device_id: Option<&'static str>,
    session: Option<&'static str>,
    now: Option<Now>,
}

fn make(opts: MakeOpts) -> Setup {
    let now = opts.now.unwrap_or_else(|| Rc::new(t0));
    let adapter = opts.adapter.unwrap_or_else(|| SpyFs::new(now.clone()));
    let vault = opts.vault.unwrap_or_default();
    let local = opts
        .local
        .unwrap_or_else(|| Rc::new(MemoryFs::new(now.clone())));
    let cache = opts.cache.unwrap_or_default();
    let m = ArtefactManager::new(
        adapter.clone(),
        ".fragment",
        vault.clone(),
        ArtefactManagerOptions {
            cache: Some(cache.clone()),
            local: Some(local.clone()),
            device_id: Some(opts.device_id.unwrap_or("k3f9").to_string()),
            session: Some(opts.session.unwrap_or("b71").to_string()),
            now: Some(now),
            ..Default::default()
        },
    );
    Setup {
        m,
        vault,
        adapter,
        local,
        cache,
    }
}

/// Le watcher : chaque chemin écrit dans le coffre revient en `raw`, puis 200 ms passent.
fn watch(main: &mut ArtefactManager, adapter: &SpyFs, c: &Clock) {
    for p in adapter.drain_raw() {
        main.on_raw(&p);
    }
    c.advance(300); // 200 ms de calme
    main.poll();
}

/// Les segments du journal du coffre.
fn segments(adapter: &SpyFs) -> Vec<String> {
    adapter
        .keys()
        .into_iter()
        .filter(|k| k.starts_with(&format!("{BASE}/journal/")) && k.ends_with(".ndjson"))
        .collect()
}

/// Les transactions d'un segment.
fn txs_of(adapter: &SpyFs, path: &str) -> Vec<Tx> {
    parse_segment(
        &adapter.get(path).unwrap(),
        &empty_cursor(JournalKind::Vault, path),
    )
    .txs
}

fn file(path: &str) -> ArtefactScope {
    ArtefactScope::file(path)
}
fn content(type_: &str, scope: ArtefactScope, state: Value) -> Fields {
    Fields::new().type_(type_).scope(scope).state(state)
}
fn by_path(path: &str) -> Filter {
    Filter::path(path)
}
fn ids(recs: &[Rec]) -> Vec<String> {
    recs.iter().map(|r| r.id.clone()).collect()
}

mod ecrire_15_1 {
    use super::*;

    #[test]
    fn set_est_synchrone_la_memoire_est_a_jour_avant_le_retour_le_disque_suit() {
        let Setup { mut m, adapter, .. } = make(MakeOpts::default());
        let id = m.set(
            None,
            content("sticky:note", file("note.md"), json!({"text": "coucou"})),
            Origin::Plugin,
        );
        assert_eq!(m.get(&id).unwrap().state, Some(json!({"text": "coucou"})));
        assert_eq!(ids(&m.list(Some(&by_path("note.md")))), vec![id.clone()]);
        assert!(segments(&adapter).is_empty()); // pas encore écrit

        m.flushed();
        let segs = segments(&adapter);
        assert_eq!(
            segs,
            vec![".fragment/artefacts/journal/k3f9/20260929T172000-b71.ndjson"]
        );
        let text = adapter.get(&segs[0]).unwrap();
        let lines: Vec<&str> = text.trim_end().split('\n').collect();
        let header: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header["journal"], json!(1));
        assert_eq!(header["writer"], json!("k3f9-b71"));
        let tx: Tx = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(tx.t, "2026-09-29T17:20:00.000Z-0000-k3f9-b71");
        assert_eq!(tx.o, Origin::Plugin);
        let op = tx.ops[0].as_set().unwrap();
        assert_eq!(
            (op.set.as_str(), op.type_.as_deref(), op.v),
            (id.as_str(), Some("sticky:note"), Some(1))
        );
        assert_eq!(op.state, Some(json!({"text": "coucou"})));
    }

    #[test]
    fn un_gros_state_part_en_objet_ecrit_avant_la_ligne_qui_le_cite() {
        let Setup { mut m, adapter, .. } = make(MakeOpts::default());
        let gros = json!({"text": "x".repeat(3000)});
        let id = m.set(
            None,
            content("t:big", ArtefactScope::Global, gros.clone()),
            Origin::Plugin,
        );
        m.flushed();
        let order = adapter.log.borrow().clone();
        let obj_write = order
            .iter()
            .position(|o| o.starts_with(&format!("write {BASE}/objects/")))
            .unwrap();
        let line_append = order.iter().rposition(|o| o.starts_with("append")).unwrap();
        assert!(obj_write < line_append);
        let txs = txs_of(&adapter, &segments(&adapter)[0]);
        let op = txs[0].ops[0].as_set().unwrap();
        assert_eq!(op.state, None);
        let head = op.head.as_deref().unwrap();
        assert!(
            head.len() == 64
                && head
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_eq!(m.state(&id), Some(gros));
    }

    #[test]
    fn les_ancres_suivies_sont_fusionnees_une_seule_ligne_pour_une_rafale() {
        let Setup { mut m, adapter, .. } = make(MakeOpts::default());
        let id = m.set(None, content("t", file("n.md"), json!(1)), Origin::Plugin);
        m.flushed();
        for pos in [10, 11, 12, 13] {
            m.set(
                Some(&id),
                Fields::new().anchor(WidgetAnchor::document(pos)),
                Origin::Follow,
            );
        }
        m.flushed();
        let txs = txs_of(&adapter, &segments(&adapter)[0]);
        let follows: Vec<&Tx> = txs.iter().filter(|t| t.o == Origin::Follow).collect();
        assert_eq!(follows.len(), 1);
        assert_eq!(
            follows[0].ops,
            vec![Op::Set(SetOp::new(&id).anchor(WidgetAnchor::document(13)))]
        );
        assert_eq!(
            m.get(&id).unwrap().anchor().map(|a| a.anchor.pos()),
            Some(Some(13))
        );
    }

    #[test]
    fn un_record_local_n_apparait_dans_aucun_fichier_du_coffre() {
        let Setup {
            mut m,
            adapter,
            local,
            ..
        } = make(MakeOpts::default());
        let id = m.set(
            None,
            content("t:brouillon", ArtefactScope::Global, json!({"secret": 42})).local(),
            Origin::Plugin,
        );
        m.set(
            Some(&id),
            Fields::new().state(json!({"secret": 43})),
            Origin::Plugin,
        );
        m.flushed();
        for k in adapter.keys() {
            assert!(!adapter.get(&k).unwrap().contains(&id));
        }
        let local_segs: Vec<String> = local
            .keys()
            .into_iter()
            .filter(|k| k.ends_with(".ndjson"))
            .collect();
        assert_eq!(local_segs.len(), 1);
        assert!(local.get(&local_segs[0]).unwrap().contains("\"secret\":43"));
        assert!(m.get(&id).unwrap().local);
    }

    #[test]
    fn del_pose_une_tombe_un_set_ulterieur_restaure() {
        let Setup { mut m, .. } = make(MakeOpts::default());
        let id = m.set(
            None,
            content("t", ArtefactScope::Global, json!(1)),
            Origin::Plugin,
        );
        m.del(&id, Origin::Plugin);
        assert!(m.get(&id).is_none());
        m.set(Some(&id), Fields::new().state(json!(2)), Origin::Plugin);
        assert_eq!(m.get(&id).unwrap().state, Some(json!(2)));
    }
}

mod demarrer_15_3 {
    use super::*;

    #[test]
    fn une_nouvelle_session_sans_cache_plie_tout_le_journal() {
        let mut a = make(MakeOpts::default());
        let id =
            a.m.set(None, content("t", file("n.md"), json!("x")), Origin::Plugin);
        a.m.set(Some(&id), Fields::new().state(json!("y")), Origin::Plugin);
        a.m.flushed();

        let mut b = make(MakeOpts {
            adapter: Some(a.adapter.clone()),
            session: Some("c02"),
            ..Default::default()
        });
        b.m.load();
        assert_eq!(b.m.get(&id).unwrap().state, Some(json!("y")));
        assert_eq!(b.m.lines_read(), 2);
    }

    #[test]
    fn un_rechargement_sans_changement_externe_ne_lit_aucune_ligne() {
        let mut a = make(MakeOpts::default());
        a.m.set(None, content("t", file("n.md"), json!("x")), Origin::Plugin);
        a.m.flushed();
        a.m.close();

        let mut b = make(MakeOpts {
            adapter: Some(a.adapter.clone()),
            cache: Some(a.cache.clone()),
            session: Some("c02"),
            ..Default::default()
        });
        b.m.load();
        assert_eq!(b.m.lines_read(), 0);
        assert_eq!(b.m.list(None).len(), 1);
    }

    #[test]
    fn un_segment_allonge_par_un_autre_ecrivain_seules_les_lignes_nouvelles_sont_pliees() {
        let mut a = make(MakeOpts::default());
        a.m.set(
            None,
            content("t", ArtefactScope::Global, json!(1)),
            Origin::Plugin,
        );
        a.m.flushed();
        let mut b = make(MakeOpts {
            adapter: Some(a.adapter.clone()),
            device_id: Some("p2x8"),
            session: Some("e02"),
            ..Default::default()
        });
        b.m.load(); // b lit la ligne de a
        a.m.set(
            None,
            content("t", ArtefactScope::Global, json!(2)),
            Origin::Plugin,
        );
        a.m.flushed();
        let mut c = make(MakeOpts {
            adapter: Some(a.adapter.clone()),
            cache: Some(b.cache.clone()),
            device_id: Some("p2x8"),
            session: Some("e03"),
            ..Default::default()
        });
        c.m.load();
        assert_eq!(c.m.lines_read(), 1);
        assert_eq!(c.m.list(None).len(), 2);
    }

    #[test]
    fn une_ligne_tronquee_en_fin_de_segment_est_ignoree_le_reste_est_plie() {
        let mut a = make(MakeOpts::default());
        let id = a.m.set(
            None,
            content("t", ArtefactScope::Global, json!(1)),
            Origin::Plugin,
        );
        a.m.flushed();
        let seg = segments(&a.adapter)[0].clone();
        let text = a.adapter.get(&seg).unwrap();
        a.adapter.set(
            &seg,
            &format!("{text}{{\"t\":\"2026-09-29T17:20:01.000Z-0000-k3f9-b71\",\"o\":\"user\",\"ops\":[{{\"del\""),
        );
        let mut b = make(MakeOpts {
            adapter: Some(a.adapter.clone()),
            session: Some("c02"),
            ..Default::default()
        });
        b.m.load();
        assert_eq!(b.m.get(&id).unwrap().state, Some(json!(1)));
    }

    #[test]
    fn une_copie_de_conflit_posee_par_la_synchro_devient_un_segment_de_plus_sans_danger() {
        let mut a = make(MakeOpts::default());
        let id = a.m.set(
            None,
            content("t", ArtefactScope::Global, json!(1)),
            Origin::Plugin,
        );
        a.m.flushed();
        let seg = segments(&a.adapter)[0].clone();
        let text = a.adapter.get(&seg).unwrap();
        a.adapter
            .set(&seg.replace(".ndjson", " (conflit).ndjson"), &text);
        let mut b = make(MakeOpts {
            adapter: Some(a.adapter.clone()),
            session: Some("c02"),
            ..Default::default()
        });
        b.m.load();
        assert_eq!(ids(&b.m.list(None)), vec![id]);
    }
}

mod un_changement_arrive_d_ailleurs_15_4 {
    use super::*;

    #[test]
    fn une_ligne_ajoutee_par_un_autre_processus_est_pliee_en_direct_origine_disk() {
        let c = clock(t0());
        let adapter = SpyFs::new(c.now());
        let mut main = make(MakeOpts {
            adapter: Some(adapter.clone()),
            now: Some(c.now()),
            ..Default::default()
        });
        main.m.load();
        adapter.drain_raw();
        type Seen = Rc<RefCell<Vec<(Tx, Vec<Change>)>>>;
        let seen: Seen = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        main.m.subscribe(by_path("host.md"), move |tx, ch| {
            sink.borrow_mut().push((tx.clone(), ch.to_vec()))
        });

        // L'autre écrivain : une autre fenêtre, un autre appareil, le CLI — sans index.
        let mut other = make(MakeOpts {
            adapter: Some(adapter.clone()),
            device_id: Some("p2x8"),
            session: Some("e02"),
            now: Some(c.now()),
            ..Default::default()
        });
        let id = other.m.set(
            None,
            content("doc-widget:pin", file("host.md"), json!({"w": 1})),
            Origin::Plugin,
        );
        other.m.flushed();
        watch(&mut main.m, &adapter, &c);

        assert_eq!(main.m.get(&id).unwrap().state, Some(json!({"w": 1})));
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0.o, Origin::Disk);
        assert_eq!(
            (seen[0].1[0].id.as_str(), seen[0].1[0].before.is_none()),
            (id.as_str(), true)
        );
    }

    #[test]
    fn les_ecritures_de_ce_processus_reviennent_par_le_watcher_et_sont_ecartees() {
        let c = clock(t0());
        let adapter = SpyFs::new(c.now());
        let Setup { mut m, .. } = make(MakeOpts {
            adapter: Some(adapter.clone()),
            now: Some(c.now()),
            ..Default::default()
        });
        m.load();
        m.set(
            None,
            content("t", ArtefactScope::Global, json!(1)),
            Origin::Plugin,
        );
        m.flushed();
        watch(&mut m, &adapter, &c);
        assert_eq!(m.lines_read(), 0);
    }
}

mod deux_appareils_hors_ligne_15_5 {
    use super::*;

    #[test]
    fn chacun_son_fichier_le_pli_donne_le_meme_resultat_des_deux_cotes() {
        let c = clock(t0());
        let adapter = SpyFs::new(c.now());
        let mk = |device: &'static str, session: &'static str| {
            make(MakeOpts {
                adapter: Some(adapter.clone()),
                device_id: Some(device),
                session: Some(session),
                now: Some(c.now()),
                ..Default::default()
            })
        };
        let mut portable = mk("k3f9", "b71");
        let mut fixe = mk("p2x8", "e02");
        let x = portable.m.set(
            Some("X"),
            content("t", file("note.md"), json!("init")),
            Origin::Plugin,
        );
        portable.m.flushed();
        fixe.m.load();

        c.advance(1000);
        fixe.m
            .set(Some(&x), Fields::new().state(json!("pain")), Origin::Plugin);
        c.advance(1000);
        portable.m.set(
            Some(&x),
            Fields::new().anchor(WidgetAnchor::document(140)),
            Origin::Plugin,
        );
        c.advance(1000);
        portable
            .m
            .set(Some(&x), Fields::new().state(json!("lait")), Origin::Plugin);
        portable.m.flushed();
        fixe.m.flushed();

        let mut p2 = mk("k3f9", "b72");
        let mut f2 = mk("p2x8", "e03");
        p2.m.load();
        f2.m.load();
        for m in [&p2.m, &f2.m] {
            let r = m.get(&x).unwrap();
            assert_eq!(r.state, Some(json!("lait")));
            assert_eq!(r.anchor().map(|a| a.anchor.pos()), Some(Some(140)));
        }
        // « pain » a perdu l'arbitrage : il reste lisible dans l'historique.
        let h = p2.m.history(&x);
        assert!(h.iter().any(|tx| {
            tx.ops
                .iter()
                .any(|op| op.as_set().is_some_and(|s| s.state == Some(json!("pain"))))
        }));
    }
}

mod quarantaine_14_3 {
    use super::*;

    #[test]
    fn une_transaction_datee_de_dix_minutes_dans_le_futur_attend_puis_est_pliee() {
        let c = clock(t0());
        let adapter = SpyFs::new(c.now());
        // Un appareil dont l'horloge avance de dix minutes.
        let futur_now: Now = {
            let n = c.now();
            Rc::new(move || n() + 10 * 60_000)
        };
        let mut futur = make(MakeOpts {
            adapter: Some(adapter.clone()),
            device_id: Some("ffff"),
            session: Some("f01"),
            now: Some(futur_now),
            ..Default::default()
        });
        let id = futur.m.set(
            None,
            content("t", ArtefactScope::Global, json!("du futur")),
            Origin::Plugin,
        );
        futur.m.flushed();

        let mut ici = make(MakeOpts {
            adapter: Some(adapter.clone()),
            now: Some(c.now()),
            ..Default::default()
        });
        let warned: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = warned.clone();
        ici.m
            .on_clock_skew(move |w| sink.borrow_mut().push(w.to_string()));
        ici.m.load();
        assert!(ici.m.get(&id).is_none());
        assert_eq!(ici.m.quarantined().len(), 1);
        assert_eq!(*warned.borrow(), vec!["ffff-f01".to_string()]);
        // L'horloge locale n'a pas été entraînée.
        let local_id = ici.m.set(
            None,
            content("t", ArtefactScope::Global, json!(0)),
            Origin::Plugin,
        );
        assert!(!local_id.is_empty());
        assert!(
            ici.m.get(&local_id).unwrap().content_t.unwrap() < format_stamp(t0() + 60_000, 0, "")
        );

        c.advance(11 * 60_000);
        ici.m.check_quarantine();
        assert_eq!(ici.m.get(&id).unwrap().state, Some(json!("du futur")));
        assert!(ici.m.quarantined().is_empty());
    }
}

mod le_document_est_renomme_ou_disparait_15_8 {
    use super::*;

    fn with_note() -> (Setup, String) {
        let mut s = make(MakeOpts::default());
        s.vault.add_note("note.md", 321, t0() - 5000);
        let id = s.m.set(
            None,
            content("t", file("note.md"), json!(1)),
            Origin::Plugin,
        );
        (s, id)
    }

    fn rename(key: &str, old_key: &str, origin: EventOrigin) -> DocumentEvent {
        DocumentEvent::Rename {
            key: key.into(),
            old_key: old_key.into(),
            origin: Some(origin),
        }
    }
    fn delete(key: &str, origin: EventOrigin) -> DocumentEvent {
        DocumentEvent::Delete {
            key: key.into(),
            origin: Some(origin),
        }
    }
    fn create(key: &str) -> DocumentEvent {
        DocumentEvent::Create { key: key.into() }
    }

    #[test]
    fn renomme_dans_l_app_une_transaction_origine_vault_le_record_suit() {
        let (mut s, id) = with_note();
        s.m.load();
        s.vault.rename_note("note.md", "renamed.md");
        s.m.on_document_event(&rename("renamed.md", "note.md", EventOrigin::User));
        assert_eq!(s.m.list(Some(&by_path("renamed.md"))).len(), 1);
        assert_eq!(s.m.list(Some(&by_path("note.md"))).len(), 0);
        s.m.flushed();
        let txs = txs_of(&s.adapter, &segments(&s.adapter)[0]);
        let last = txs.last().unwrap();
        assert_eq!(last.o, Origin::Vault);
        assert_eq!(
            last.ops,
            vec![Op::Set(SetOp::new(&id).scope(file("renamed.md")))]
        );
    }

    #[test]
    fn supprime_dans_l_app_des_tombes_restaurables() {
        let (mut s, id) = with_note();
        s.m.load();
        s.m.on_document_event(&delete("note.md", EventOrigin::User));
        assert!(s.m.get(&id).is_none());
        s.m.set(Some(&id), Fields::new().state(json!(1)), Origin::Plugin);
        assert!(s.m.get(&id).is_some());
    }

    #[test]
    fn disparu_hors_de_l_app_rien_n_est_ecrit_le_record_est_garde_orphelin() {
        let (mut s, id) = with_note();
        s.m.load();
        s.m.flushed();
        let seg = segments(&s.adapter)[0].clone();
        let avant = s.adapter.get(&seg);
        let note = s.vault.remove_note("note.md").unwrap();
        s.m.on_document_event(&delete("note.md", EventOrigin::Disk));
        s.m.flushed();
        assert_eq!(s.adapter.get(&seg), avant);
        assert!(s.m.get(&id).is_some());
        // Le fichier revient (git checkout main) : il est rattaché de lui-même.
        s.vault.add_note("note.md", note.size, note.mtime);
        assert_eq!(s.m.list(Some(&by_path("note.md"))).len(), 1);
    }

    #[test]
    fn renomme_hors_de_l_app_meme_taille_et_mtime_les_set_scope_sont_ecrits() {
        let (mut s, id) = with_note();
        s.m.load();
        s.vault.remove_note("note.md");
        s.m.on_document_event(&delete("note.md", EventOrigin::Disk));
        s.vault.add_note("ailleurs/note.md", 321, t0() - 5000);
        s.m.on_document_event(&create("ailleurs/note.md"));
        assert_eq!(ids(&s.m.list(Some(&by_path("ailleurs/note.md")))), vec![id]);
        s.m.flushed();
        let txs = txs_of(&s.adapter, &segments(&s.adapter)[0]);
        assert_eq!(txs.last().map(|t| t.o), Some(Origin::Vault));
    }

    #[test]
    fn un_fichier_d_une_autre_taille_ne_rattache_rien() {
        let (mut s, _id) = with_note();
        s.m.load();
        s.vault.remove_note("note.md");
        s.m.on_document_event(&delete("note.md", EventOrigin::Disk));
        s.vault.add_note("autre.md", 999, t0() - 5000);
        s.m.on_document_event(&create("autre.md"));
        assert_eq!(s.m.list(Some(&by_path("note.md"))).len(), 1);
    }
}

mod un_document_de_plugin_est_renomme_ou_supprime_documents_de_plugin_md {
    use super::*;

    /// Le flux des documents porte aussi les documents de plugin (`chat:agent/c1`) ;
    /// ici le fournisseur `chat` est supposé enregistré (DocumentManager, fragment-core).
    fn with_chat() -> (Setup, String) {
        let mut s = make(MakeOpts::default());
        let id = s.m.set(
            None,
            content("t", file("chat:agent/c1"), json!(1)),
            Origin::Plugin,
        );
        (s, id)
    }

    #[test]
    fn supprime_user_ses_marques_sont_enterrees_origine_plugin() {
        let (mut s, id) = with_chat();
        s.m.load();
        s.m.on_document_event(&DocumentEvent::Delete {
            key: "chat:agent/c1".into(),
            origin: Some(EventOrigin::User),
        });
        assert!(s.m.get(&id).is_none());
        s.m.flushed();
        let txs = txs_of(&s.adapter, &segments(&s.adapter)[0]);
        let last = txs.last().unwrap();
        assert_eq!(
            (last.o, last.ops.clone()),
            (Origin::Plugin, vec![Op::del(&id)])
        );
    }

    #[test]
    fn supprime_disk_rien_n_est_ecrit() {
        let (mut s, id) = with_chat();
        s.m.load();
        s.m.on_document_event(&DocumentEvent::Delete {
            key: "chat:agent/c1".into(),
            origin: Some(EventOrigin::Disk),
        });
        assert!(s.m.get(&id).is_some());
    }

    #[test]
    fn renomme_le_scope_suit() {
        let (mut s, id) = with_chat();
        s.m.load();
        s.m.on_document_event(&DocumentEvent::Rename {
            key: "chat:agent/c2".into(),
            old_key: "chat:agent/c1".into(),
            origin: Some(EventOrigin::User),
        });
        assert_eq!(ids(&s.m.list(Some(&by_path("chat:agent/c2")))), vec![id]);
    }

    #[test]
    fn une_cle_hors_coffre_n_est_jamais_rattachee_par_taille_mtime() {
        let (mut s, _id) = with_chat();
        s.m.load();
        s.vault.add_note("x.md", 100, t0());
        s.m.on_document_event(&DocumentEvent::Create { key: "x.md".into() });
        assert_eq!(s.m.list(Some(&by_path("chat:agent/c1"))).len(), 1);
    }

    // « un événement sans fournisseur est ignoré » : c'est `DocumentManager.notify`
    // (fragment-core) qui filtre ; le manager ne reçoit que des événements fournis.
    // Listé dans parite.toml.
}

mod la_migration_depuis_les_refs_18 {
    use super::*;

    fn legacy(adapter: &SpyFs, order: &[&str]) {
        let refs = [
            json!({"id": "bbb", "type": "annotation:doc", "version": 1, "scope": {"kind": "file", "path": "n.md"}, "head": "h1", "updatedAt": t0()}),
            json!({
                "id": "aaa", "type": "doc-widget:pin", "version": 1, "scope": {"kind": "file", "path": "n.md"},
                "anchor": {"mode": "document", "pos": 3, "dx": 0, "dy": 0, "char": "x"},
                "head": "h2", "updatedAt": t0() + 5
            }),
        ];
        for id in order {
            let r = refs.iter().find(|x| x["id"] == json!(id)).unwrap();
            let plugin = r["type"].as_str().unwrap().split(':').next().unwrap();
            adapter
                .write(
                    &format!("{BASE}/refs/{plugin}/{id}.json"),
                    &r.to_string(),
                    None,
                )
                .unwrap();
        }
        adapter
            .write(
                &format!("{BASE}/objects/h1.json"),
                &json!({"strokes": [1, 2]}).to_string(),
                None,
            )
            .unwrap();
        adapter
            .write(
                &format!("{BASE}/objects/h2.json"),
                &json!({"file": "a.md", "w": 1, "h": 2}).to_string(),
                None,
            )
            .unwrap();
    }

    #[test]
    fn import_deterministe_deux_appareils_produisent_le_meme_fichier_refs_renomme_garde() {
        let a1 = SpyFs::new(Rc::new(t0));
        let a2 = SpyFs::new(Rc::new(t0));
        legacy(&a1, &["aaa", "bbb"]);
        legacy(&a2, &["bbb", "aaa"]);
        let mut m1 = make(MakeOpts {
            adapter: Some(a1.clone()),
            ..Default::default()
        });
        let mut m2 = make(MakeOpts {
            adapter: Some(a2.clone()),
            device_id: Some("p2x8"),
            ..Default::default()
        });
        m1.m.load();
        m2.m.load();
        assert!(a1.get(&import_path(BASE)).is_some());
        assert_eq!(a1.get(&import_path(BASE)), a2.get(&import_path(BASE)));
        assert!(
            !a1.keys()
                .iter()
                .any(|k| k.starts_with(&format!("{BASE}/refs/")))
        );
        assert!(
            a1.keys()
                .iter()
                .any(|k| k.starts_with(&format!("{BASE}/refs.migrated/")))
        );
        assert_eq!(
            m1.m.get("aaa").unwrap().state,
            Some(json!({"file": "a.md", "w": 1, "h": 2}))
        );
        assert_eq!(
            m1.m.get("aaa").unwrap().anchor().cloned(),
            Some(DurableAnchor::from(WidgetAnchor::document(3)))
        );
        assert_eq!(
            m1.m.get("bbb").unwrap().content_t,
            Some(format_stamp(t0(), 0, "import"))
        );
        // Les plugins existants lisent toujours par la façade.
        let f = ScopeFilter {
            kind: Some(ScopeKind::File),
            path: Some("n.md".into()),
            collection: None,
        };
        assert_eq!(m1.m.list_sync(Some(f)).len(), 2);
    }

    #[test]
    fn importfilecontent_trie_par_id_quel_que_soit_l_ordre() {
        let l1 = legacy_line(
            &LegacyRef::new("b", "t", ArtefactScope::Global, "h", 1.0),
            Some(&json!(1)),
        );
        let l2 = legacy_line(
            &LegacyRef::new("a", "t", ArtefactScope::Global, "h", 2.0),
            Some(&json!(2)),
        );
        assert_eq!(
            import_file_content(&[l1.clone(), l2.clone()]),
            import_file_content(&[l2, l1])
        );
    }
}

mod la_facade_de_l_ancienne_api_18 {
    use super::*;

    #[test]
    fn save_update_listsync_delete_au_dessus_de_dispatch() {
        let Setup { mut m, .. } = make(MakeOpts::default());
        let ArtefactRef { id } = m.save(SaveSpec {
            id: None,
            type_: "annotation:doc".into(),
            version: None,
            scope: file("n.md"),
            anchor: None,
            state: json!({"strokes": []}),
        });
        m.update(
            &id,
            SavePatch {
                state: Some(json!({"strokes": [1]})),
                ..Default::default()
            },
        );
        let f = || ScopeFilter {
            kind: Some(ScopeKind::File),
            path: Some("n.md".into()),
            collection: None,
        };
        let states: Vec<Option<Value>> = m
            .list_sync(Some(f()))
            .into_iter()
            .map(|r| r.state)
            .collect();
        assert_eq!(states, vec![Some(json!({"strokes": [1]}))]);
        m.delete(&id);
        assert_eq!(m.list_sync(Some(f())).len(), 0);
    }
}

mod maintenance_15_9_15_10 {
    use super::*;

    #[test]
    fn la_compaction_reecrit_mes_segments_scelles_en_un_seul_l_index_ne_change_pas() {
        let adapter = SpyFs::new(Rc::new(t0));
        let x = "X";
        for (s, session) in ["s00", "s01", "s02"].into_iter().enumerate() {
            let Setup { mut m, .. } = make(MakeOpts {
                adapter: Some(adapter.clone()),
                session: Some(session),
                ..Default::default()
            });
            m.load();
            m.set(
                Some(x),
                content("t", file("n.md"), json!(s)),
                Origin::Plugin,
            );
            m.set(
                Some(x),
                Fields::new().anchor(WidgetAnchor::document(s as i64)),
                Origin::Plugin,
            );
            m.flushed();
            m.close(); // scelle le segment
        }
        let mut autre = make(MakeOpts {
            adapter: Some(adapter.clone()),
            device_id: Some("p2x8"),
            session: Some("e02"),
            ..Default::default()
        });
        autre.m.set(
            Some("Y"),
            content("t", ArtefactScope::Global, json!("y")),
            Origin::Plugin,
        );
        autre.m.flushed();

        let Setup { mut m, .. } = make(MakeOpts {
            adapter: Some(adapter.clone()),
            session: Some("zzz"),
            ..Default::default()
        });
        m.load();
        let avant = m.dump();
        m.maintain(true);
        let mine: Vec<String> = segments(&adapter)
            .into_iter()
            .filter(|p| p.contains("/k3f9/"))
            .collect();
        assert_eq!(mine.len(), 1);
        let name = mine[0].rsplit('/').next().unwrap();
        assert!(
            name.starts_with("c-") && name.ends_with("-zzz.ndjson"),
            "{name}"
        );
        assert!(
            name[2..name.len() - "-zzz.ndjson".len()]
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'T')
        );
        assert!(segments(&adapter).iter().any(|p| p.contains("/p2x8/"))); // jamais ceux des autres

        let mut relu = make(MakeOpts {
            adapter: Some(adapter.clone()),
            session: Some("yyy"),
            ..Default::default()
        });
        relu.m.load();
        assert_eq!(relu.m.dump(), avant);
    }

    #[test]
    fn un_objet_que_plus_rien_ne_cite_part_apres_quatorze_jours_pas_avant() {
        let c = clock(t0());
        let adapter = SpyFs::new(c.now());
        let Setup { mut m, .. } = make(MakeOpts {
            adapter: Some(adapter.clone()),
            now: Some(c.now()),
            ..Default::default()
        });
        m.load();
        adapter
            .write(&format!("{BASE}/objects/orphelin.json"), "{}", None)
            .unwrap();
        let big = json!({"big": "x".repeat(3000)});
        let id = m.set(
            None,
            content("t", ArtefactScope::Global, big.clone()),
            Origin::Plugin,
        );
        m.flushed();
        m.maintain(false);
        assert!(adapter.has(&format!("{BASE}/objects/orphelin.json")));
        c.advance(GRACE_MS + 1000);
        m.maintain(false);
        assert!(!adapter.has(&format!("{BASE}/objects/orphelin.json")));
        assert_eq!(m.state(&id), Some(big)); // celui qui est cité reste
    }

    #[test]
    fn un_segment_illisible_annule_le_balayage_pour_la_session() {
        let c = clock(t0());
        let adapter = SpyFs::new(c.now());
        adapter
            .write(&format!("{BASE}/objects/orphelin.json"), "{}", None)
            .unwrap();
        adapter
            .write(
                &format!("{BASE}/journal/p2x8/x-e02.ndjson"),
                "{\"journal\":1}\n",
                None,
            )
            .unwrap();
        *adapter.fail_read_suffix.borrow_mut() = Some("x-e02.ndjson".into());
        c.advance(GRACE_MS + 1000);
        let Setup { mut m, .. } = make(MakeOpts {
            adapter: Some(adapter.clone()),
            now: Some(c.now()),
            ..Default::default()
        });
        m.load();
        m.maintain(false);
        assert!(adapter.has(&format!("{BASE}/objects/orphelin.json")));
    }
}

mod abonnements {
    use super::*;

    #[test]
    fn recoit_chaque_transaction_qui_touche_un_record_du_filtre_avant_apres() {
        let Setup { mut m, .. } = make(MakeOpts::default());
        let seen: Rc<RefCell<Vec<Vec<Change>>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        let off = m.subscribe(
            Filter::scope(ScopeFilter {
                kind: Some(ScopeKind::Plugin),
                path: None,
                collection: Some("session/1".into()),
            }),
            move |_tx, ch| sink.borrow_mut().push(ch.to_vec()),
        );
        let id = m.set(
            None,
            content("ia:message", ArtefactScope::plugin("session/1"), json!("a")),
            Origin::Plugin,
        );
        m.set(
            None,
            content("ia:message", ArtefactScope::plugin("session/2"), json!("b")),
            Origin::Plugin,
        );
        m.del(&id, Origin::Plugin);
        m.unsubscribe(off);
        m.set(Some(&id), Fields::new().state(json!("c")), Origin::Plugin);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(
            (seen[1][0].id.as_str(), seen[1][0].after.is_none()),
            (id.as_str(), true)
        );
    }
}

/// `poll` sert les échéances : le tic du tampon et le calme des ancres suivies.
#[test]
fn poll_vide_le_tampon_au_tic_et_les_ancres_suivies_apres_le_calme() {
    let c = clock(t0());
    let adapter = SpyFs::new(c.now());
    let Setup { mut m, .. } = make(MakeOpts {
        adapter: Some(adapter.clone()),
        now: Some(c.now()),
        ..Default::default()
    });
    let id = m.set(None, content("t", file("n.md"), json!(1)), Origin::Plugin);
    assert!(segments(&adapter).is_empty());
    m.poll(); // le tic : écrit
    assert_eq!(segments(&adapter).len(), 1);
    m.set(
        Some(&id),
        Fields::new().anchor(WidgetAnchor::document(5)),
        Origin::Follow,
    );
    m.poll();
    assert_eq!(txs_of(&adapter, &segments(&adapter)[0]).len(), 1); // pas encore : 2 s de calme
    c.advance(crate::journal::writer::FOLLOW_CALM_MS);
    m.poll();
    assert_eq!(txs_of(&adapter, &segments(&adapter)[0]).len(), 2);
}
