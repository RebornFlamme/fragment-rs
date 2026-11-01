// Port de app/src/ui/core/artefacts/journal/writer.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'écriture (§15.1, §15.2) — le tampon et le segment actif.
//
//  R2 : un fichier, un écrivain. R3 : on ajoute, on ne réécrit pas.
//
//  Le TAMPON regroupe les transactions et fusionne les ancres suivies :
//
//      origine          vidage                                fusion
//      follow           après 2 s de calme, 10 s au plus      une seule op d'ancre par record
//      toutes autres    au tic suivant                        aucune
//      fermeture, perte du focus : immédiat, puis ligne de fermeture
//
//  Au vidage : les gros states partent d'abord en objets (invariant §19.1-4),
//  puis UN `append` des lignes au segment actif, puis le manager commit le
//  cache (records + curseur, une transaction).
//
//  Le SEGMENT ACTIF est créé paresseusement (une session qui n'écrit rien ne
//  laisse aucun fichier), scellé et remplacé dès qu'il atteint 256 Kio : un outil
//  de synchronisation qui renvoie le fichier entier n'envoie jamais plus.
//
//  ★ Pas de minuteries ici : le TS pose des `setTimeout` ; en Rust le tampon note
//    ses ÉCHÉANCES (`due`) et c'est l'hôte (le manager, par `poll`) qui vide quand
//    elles sont passées. Les tests appellent `flushed()` : même résultat, sans fil.
// ═══════════════════════════════════════════════════════════════════════════

use crate::cache::{JournalKind, PerJournal, SegmentCursor};
use crate::clock::{Now, iso_from_ms};
use crate::fs::SharedFs;
use crate::objects::{ObjectStore, byte_length, fits_inline};
use crate::types::{Op, Tx};
use std::collections::HashSet;
use std::io;
use std::rc::Rc;

use super::reader::{JOURNAL_FORMAT, SEGMENT_EXT, SegmentHeader, cited_objects, cursor_key};

/// Taille à partir de laquelle un segment actif est scellé (§14.1).
pub const SEGMENT_MAX_BYTES: usize = 256 * 1024;

/// Le calme attendu avant d'écrire les ancres suivies, et le délai maximal (§15.1).
pub const FOLLOW_CALM_MS: i64 = 2000;
pub const FOLLOW_MAX_MS: i64 = 10_000;

const SEAL_LINE: &str = "{\"seal\":true}\n";

/// `20260929T172000` — la date de création d'un segment, dans son nom.
pub fn segment_date(ms: i64) -> String {
    iso_from_ms(ms)
        .chars()
        .filter(|c| *c != '-' && *c != ':')
        .take(15)
        .collect()
}

fn tx_line(tx: &Tx) -> String {
    let mut s = serde_json::to_string(tx).unwrap_or_else(|_| "null".to_string());
    s.push('\n');
    s
}

/// Le segment actif d'UN journal (coffre ou appareil) pour cette session. Seul
/// fichier que ce processus modifie dans ce journal.
pub struct SegmentWriter {
    fs: SharedFs,
    dir: String,
    pub journal: JournalKind,
    writer: String,
    session: String,
    app: String,
    now: Now,
    /// Le segment actif, ou `None` tant que rien n'a été écrit.
    path: Option<String>,
    cursor: Option<SegmentCursor>,
    /// Les segments que CETTE session a ouverts (pour les reconnaître au retour du watcher).
    mine: HashSet<String>,
}

pub struct SegmentWriterOptions<'a> {
    pub fs: SharedFs,
    pub base: &'a str,
    pub journal: JournalKind,
    pub device_id: &'a str,
    pub session: &'a str,
    pub app: &'a str,
    pub now: Now,
}

impl SegmentWriter {
    pub fn new(opts: SegmentWriterOptions<'_>) -> Self {
        SegmentWriter {
            fs: opts.fs,
            dir: format!("{}/journal/{}", opts.base, opts.device_id),
            journal: opts.journal,
            writer: format!("{}-{}", opts.device_id, opts.session),
            session: opts.session.to_string(),
            app: opts.app.to_string(),
            now: opts.now,
            path: None,
            cursor: None,
            mine: HashSet::new(),
        }
    }

    /// Le chemin du segment actif (ou `None`).
    pub fn active_path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Ce segment a-t-il été ouvert par cette session ?
    pub fn is_mine(&self, path: &str) -> bool {
        self.mine.contains(path)
    }

    /// Ajoute des lignes ; rend les curseurs touchés (le segment scellé s'il y en a un, l'actif).
    pub fn append(&mut self, txs: &[Tx]) -> io::Result<Vec<SegmentCursor>> {
        let mut touched = Vec::new();
        if txs.is_empty() {
            return Ok(touched);
        }
        let lines: Vec<String> = txs.iter().map(tx_line).collect();
        let data = lines.concat();

        if let Some(c) = &self.cursor
            && c.size + byte_length(&data) > SEGMENT_MAX_BYTES
            && c.lines > 1
        {
            touched.push(self.seal()?);
        }
        if self.path.is_none() || self.cursor.is_none() {
            self.open()?;
        }
        let (Some(path), Some(cursor)) = (self.path.clone(), self.cursor.take()) else {
            return Err(io::Error::other("segment actif absent après ouverture"));
        };

        self.fs.append(&path, &data)?;
        let mut cited: Vec<String> = cursor.objects.clone();
        for tx in txs {
            for sha in cited_objects(&tx.ops) {
                if !cited.contains(&sha) {
                    cited.push(sha);
                }
            }
        }
        let cursor = SegmentCursor {
            size: cursor.size + byte_length(&data),
            lines: cursor.lines + lines.len(),
            objects: cited,
            ..cursor
        };
        self.cursor = Some(cursor.clone());
        touched.push(cursor);
        Ok(touched)
    }

    /// Pose la ligne de fermeture ; le prochain ajout ouvrira un nouveau segment.
    pub fn seal(&mut self) -> io::Result<SegmentCursor> {
        let (Some(path), Some(cursor)) = (self.path.take(), self.cursor.take()) else {
            return Err(io::Error::other("aucun segment actif à sceller"));
        };
        self.fs.append(&path, SEAL_LINE)?;
        Ok(SegmentCursor {
            size: cursor.size + byte_length(SEAL_LINE),
            lines: cursor.lines + 1,
            sealed: true,
            ..cursor
        })
    }

    /// Fermeture (fenêtre qui se ferme) : lignes restantes + ligne de fermeture.
    /// (`sealSync` du TS : ici tout est déjà synchrone, même chemin que `append`.)
    pub fn seal_with(&mut self, txs: &[Tx]) -> io::Result<()> {
        let mut data: String = txs.iter().map(tx_line).collect();
        if self.path.is_none() {
            if data.is_empty() {
                return Ok(()); // rien écrit cette session : aucun fichier
            }
            let path = self.new_path(0);
            self.mine.insert(path.clone());
            self.fs.mkdir(&self.dir)?;
            data = format!("{}{data}", self.header_line());
            self.path = Some(path);
        }
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        self.cursor = None;
        self.fs.append(&path, &format!("{data}{SEAL_LINE}"))
    }

    fn header_line(&self) -> String {
        let header = SegmentHeader {
            journal: JOURNAL_FORMAT,
            writer: self.writer.clone(),
            app: Some(self.app.clone()),
        };
        let mut s = serde_json::to_string(&header).unwrap_or_default();
        s.push('\n');
        s
    }

    fn new_path(&self, offset_s: i64) -> String {
        format!(
            "{}/{}-{}{SEGMENT_EXT}",
            self.dir,
            segment_date((self.now)() + offset_s * 1000),
            self.session
        )
    }

    /// Ouvre un segment neuf : son nom (date de création + session), puis l'en-tête.
    fn open(&mut self) -> io::Result<()> {
        self.fs.mkdir(&self.dir)?;
        let mut path = self.new_path(0);
        // Deux segments de la même session dans la même seconde : on décale le nom.
        let mut k = 1;
        while self.mine.contains(&path) || self.fs.exists(&path) {
            path = self.new_path(k);
            k += 1;
        }
        let header = self.header_line();
        self.fs.append(&path, &header)?;
        self.mine.insert(path.clone());
        self.cursor = Some(SegmentCursor {
            key: cursor_key(self.journal, &path),
            journal: self.journal,
            path: path.clone(),
            size: byte_length(&header),
            lines: 1,
            sealed: false,
            objects: Vec::new(),
        });
        self.path = Some(path);
        Ok(())
    }
}

/// Une transaction en attente, et le journal où elle va.
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub tx: Tx,
    pub journal: JournalKind,
}

/// L'empreinte posée sur une op dont le state est parti en objet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub id: String,
    pub t: String,
    pub head: String,
}

/// Ce qu'un vidage a écrit, par journal — de quoi commit le cache.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlushResult {
    pub written: Vec<Pending>,
    pub cursors: Vec<SegmentCursor>,
    pub heads: Vec<Head>,
}

/// Une op de suivi d'ancre seule : `{ set, anchor }`, rien d'autre.
fn is_anchor_only(op: &Op) -> bool {
    match op {
        Op::Set(s) => {
            s.anchor.is_some()
                && s.type_.is_none()
                && s.v.is_none()
                && s.state.is_none()
                && s.head.is_none()
                && s.scope.is_none()
                && !s.local
        }
        Op::Del { .. } => false,
    }
}

/// Le tampon d'écriture. Il ne connaît ni l'index ni les abonnés : le manager a
/// déjà plié la transaction avant de la lui confier (la mémoire est à jour avant
/// le retour de l'appel, §15.1).
pub struct WriteBuffer {
    pending: Vec<Pending>,
    writers: PerJournal<SegmentWriter>,
    objects: PerJournal<Rc<ObjectStore>>,
    now: Now,
    /// Les échéances (`setTimeout` du TS) : le tic (au prochain `poll`), le calme, le maximum.
    tick_armed: bool,
    calm_at: Option<i64>,
    max_at: Option<i64>,
}

impl WriteBuffer {
    pub fn new(
        writers: PerJournal<SegmentWriter>,
        objects: PerJournal<Rc<ObjectStore>>,
        now: Now,
    ) -> Self {
        WriteBuffer {
            pending: Vec::new(),
            writers,
            objects,
            now,
            tick_armed: false,
            calm_at: None,
            max_at: None,
        }
    }

    pub fn size(&self) -> usize {
        self.pending.len()
    }

    pub fn writer(&self, journal: JournalKind) -> &SegmentWriter {
        self.writers.get(journal)
    }

    /// Met une transaction (déjà pliée) en attente d'écriture.
    pub fn push(&mut self, tx: Tx, journal: JournalKind) {
        if tx.o == crate::types::Origin::Follow {
            self.merge_follow(tx, journal);
            self.arm_follow();
        } else {
            self.pending.push(Pending { tx, journal });
            self.tick_armed = true;
        }
    }

    /// Une échéance est-elle passée ? (L'hôte vide alors par `flushed`.)
    pub fn due(&self, now: i64) -> bool {
        self.tick_armed
            || self.calm_at.is_some_and(|t| now >= t)
            || self.max_at.is_some_and(|t| now >= t)
    }

    /// Vide tout maintenant ; rend ce qui a été écrit (vide si rien n'attendait).
    pub fn flushed(&mut self) -> io::Result<FlushResult> {
        self.clear_timers();
        let batch = std::mem::take(&mut self.pending);
        self.write(batch)
    }

    /// Fermeture : les lignes en attente et la ligne de fermeture, par journal. Les
    /// gros states partent inline (le TS n'a pas le temps de calculer l'empreinte
    /// avant que la fenêtre se ferme — la ligne reste valide).
    pub fn close(&mut self) -> Vec<io::Error> {
        self.clear_timers();
        let batch = std::mem::take(&mut self.pending);
        let mut errors = Vec::new();
        for journal in JournalKind::ALL {
            let txs: Vec<Tx> = batch
                .iter()
                .filter(|p| p.journal == journal)
                .map(|p| p.tx.clone())
                .collect();
            if let Err(e) = self.writers.get_mut(journal).seal_with(&txs) {
                errors.push(e);
            }
        }
        errors
    }

    // ── interne ────────────────────────────────────────────────────────────────

    /// Fusion des ancres suivies : une op d'ancre en attente pour le même record est
    /// REMPLACÉE (la nouvelle porte l'estampille la plus récente). Une transaction
    /// d'attente vidée de ses ops disparaît.
    fn merge_follow(&mut self, tx: Tx, journal: JournalKind) {
        let ids: HashSet<&str> = tx
            .ops
            .iter()
            .filter(|op| is_anchor_only(op))
            .map(|op| op.id())
            .collect();
        if !ids.is_empty() {
            let mut kept = Vec::with_capacity(self.pending.len());
            for mut p in std::mem::take(&mut self.pending) {
                if p.tx.o != crate::types::Origin::Follow || p.journal != journal {
                    kept.push(p);
                    continue;
                }
                p.tx.ops
                    .retain(|op| !(is_anchor_only(op) && ids.contains(op.id())));
                if !p.tx.ops.is_empty() {
                    kept.push(p);
                }
            }
            self.pending = kept;
        }
        self.pending.push(Pending { tx, journal });
    }

    fn arm_follow(&mut self) {
        let now = (self.now)();
        self.calm_at = Some(now + FOLLOW_CALM_MS);
        if self.max_at.is_none() {
            self.max_at = Some(now + FOLLOW_MAX_MS);
        }
    }

    fn clear_timers(&mut self) {
        self.tick_armed = false;
        self.calm_at = None;
        self.max_at = None;
    }

    fn write(&mut self, batch: Vec<Pending>) -> io::Result<FlushResult> {
        let mut result = FlushResult::default();
        if batch.is_empty() {
            return Ok(result);
        }

        // 1. Les gros states d'abord : objects/<sha>, puis la ligne cite l'empreinte.
        let now = (self.now)();
        for p in batch {
            let mut ops = Vec::with_capacity(p.tx.ops.len());
            for op in p.tx.ops {
                match op {
                    Op::Set(mut s)
                        if s.type_.is_some()
                            && s.state.as_ref().is_some_and(|st| !fits_inline(st)) =>
                    {
                        let state = s.state.take().unwrap_or(serde_json::Value::Null);
                        let head = self.objects.get(p.journal).put(&state, now)?;
                        result.heads.push(Head {
                            id: s.set.clone(),
                            t: p.tx.t.clone(),
                            head: head.clone(),
                        });
                        s.head = Some(head);
                        ops.push(Op::Set(s));
                    }
                    other => ops.push(other),
                }
            }
            result.written.push(Pending {
                tx: Tx { ops, ..p.tx },
                journal: p.journal,
            });
        }

        // 2. Un ajout par journal touché.
        for journal in JournalKind::ALL {
            let txs: Vec<Tx> = result
                .written
                .iter()
                .filter(|p| p.journal == journal)
                .map(|p| p.tx.clone())
                .collect();
            if !txs.is_empty() {
                result
                    .cursors
                    .extend(self.writers.get_mut(journal).append(&txs)?);
            }
        }

        // 3. Le manager : records + curseurs, une transaction du cache.
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_date_a_la_forme_du_ts() {
        assert_eq!(
            segment_date(crate::clock::date_utc(2026, 8, 29, 17, 20)),
            "20260929T172000"
        );
    }
}
