// Port de app/src/ui/core/artefacts/journal/compact.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  La compaction (§15.9) — le SEUL code qui supprime des lignes (§19.2).
//
//      Qui      la fenêtre principale, et seulement le dossier de son appareil.
//      Quoi     les segments scellés, et les abandonnés depuis plus de 24 h.
//               Jamais un segment actif.
//      Quand    au repos, après le démarrage, s'il y a plus de 8 segments ou
//               plus de 1 Mio.
//      Comment  écrire le segment compacté sous un nom temporaire, le renommer,
//               puis supprimer les sources.
//      Retiré   une ancre remplacée ; un contenu remplacé depuis plus de 90 jours
//               (par un autre contenu, ou par une tombe : la purge, figure 36).
//      Gardé    la valeur courante de chaque groupe, et toute tombe.
//
//  ★ Une coupure entre l'écriture et la suppression laisse des lignes en double,
//    que le pli absorbe (idempotent). Plier le compacté = plier les sources
//    (associatif) : l'index ne change pas — c'est le test de propriété.
// ═══════════════════════════════════════════════════════════════════════════

use crate::clock::stamp_time;
use crate::fs::SharedFs;
use crate::objects::byte_length;
use crate::types::{Index, Op, SetOp, Stamp, Tx};
use std::collections::BTreeMap;
use std::io;

use super::reader::{JOURNAL_FORMAT, SEGMENT_EXT, SegmentHeader, cited_objects};
use super::writer::segment_date;

/// La rétention d'un contenu remplacé : 90 jours, le reflog de git (§13).
pub const RETENTION_MS: i64 = 90 * 24 * 60 * 60 * 1000;
/// Un segment non scellé est abandonné au-delà de 24 heures sans écriture.
pub const ABANDONED_MS: i64 = 24 * 60 * 60 * 1000;
/// Seuils de déclenchement.
pub const COMPACT_MIN_SEGMENTS: usize = 8;
pub const COMPACT_MIN_BYTES: u64 = 1024 * 1024;

/// Les estampilles qui REMPLACENT un contenu, pour chaque record, triées : un
/// nouveau contenu, ou une tombe (figure 36 : un record supprimé voit son contenu
/// purgé quand la rétention est écoulée). Tous écrivains confondus.
pub fn content_stamps_of<'a>(
    txs: impl IntoIterator<Item = &'a Tx>,
) -> BTreeMap<String, Vec<Stamp>> {
    let mut out: BTreeMap<String, Vec<Stamp>> = BTreeMap::new();
    for tx in txs {
        for op in &tx.ops {
            let id = match op {
                Op::Del { del } => del,
                Op::Set(s) if s.type_.is_some() => &s.set,
                Op::Set(_) => continue,
            };
            out.entry(id.clone()).or_default().push(tx.t.clone());
        }
    }
    for list in out.values_mut() {
        list.sort();
    }
    out
}

/// La première estampille de `stamps` strictement plus grande que `t`.
fn next_after<'a>(stamps: Option<&'a Vec<Stamp>>, t: &str) -> Option<&'a Stamp> {
    stamps?.iter().find(|s| s.as_str() > t)
}

/// Le cœur pur : garde de chaque op ce qui doit survivre. `index` = le pli de TOUT
/// le journal (valeurs courantes) ; `stamps` = les estampilles de contenu connues.
pub fn compact_txs(
    txs: &[Tx],
    index: &Index,
    stamps: &BTreeMap<String, Vec<Stamp>>,
    now: i64,
) -> Vec<Tx> {
    let mut sorted: Vec<&Tx> = txs.iter().collect();
    sorted.sort_by(|a, b| a.t.cmp(&b.t));
    let mut out = Vec::new();
    for tx in sorted {
        let mut ops = Vec::new();
        for op in &tx.ops {
            let Op::Set(op) = op else {
                ops.push(op.clone()); // une tombe ne part jamais
                continue;
            };
            let rec = index.get(&op.set);
            let mut kept = SetOp::new(&op.set);
            let mut any = false;

            if op.type_.is_some() {
                // Remplacé par un contenu plus récent, ou par une tombe : gardé
                // pendant la rétention (l'historique), retiré ensuite. La valeur
                // courante d'un record vivant n'a pas de remplaçant : elle reste.
                let replaced_at = next_after(stamps.get(&op.set), &tx.t);
                let keep = !replaced_at
                    .and_then(|r| stamp_time(r))
                    .is_some_and(|ms| now - ms > RETENTION_MS);
                if keep {
                    kept.type_ = op.type_.clone();
                    kept.v = op.v;
                    kept.state = op.state.clone();
                    kept.head = op.head.clone();
                    any = true;
                }
            }
            if op.scope.is_some() {
                // Un scope remplacé n'est pas dans la liste de ce qui part : il reste.
                kept.scope = op.scope.clone();
                any = true;
            }
            if op.anchor.is_some()
                && rec.is_some_and(|r| r.anchor_t.as_deref() == Some(tx.t.as_str()))
            {
                kept.anchor = op.anchor.clone(); // une ancre remplacée ne sert plus à rien
                any = true;
            }
            if any {
                if op.local {
                    kept.local = true;
                }
                ops.push(Op::Set(kept));
            }
        }
        if !ops.is_empty() {
            out.push(Tx { ops, ..tx.clone() });
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactedSegment {
    pub path: String,
    pub size: usize,
    pub lines: usize,
    pub objects: Vec<String>,
}

pub struct WriteCompacted<'a> {
    pub fs: &'a SharedFs,
    pub device_dir: &'a str,
    pub sources: &'a [String],
    pub kept: &'a [Tx],
    pub writer: &'a str,
    pub session: &'a str,
    pub app: &'a str,
}

/// Écrit le segment compacté puis supprime les sources. Rend le nouveau segment
/// (ou `None` si rien ne survit — les sources partent quand même).
pub fn write_compacted(opts: WriteCompacted<'_>) -> io::Result<Option<CompactedSegment>> {
    let WriteCompacted {
        fs,
        device_dir,
        sources,
        kept,
        ..
    } = opts;
    let mut result = None;

    if let Some(last) = kept.last() {
        let newest = stamp_time(&last.t).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("estampille illisible : {}", last.t),
            )
        })?;
        let date = segment_date(newest);
        let mut path = format!("{device_dir}/c-{date}-{}{SEGMENT_EXT}", opts.session);
        let mut k = 2;
        while sources.contains(&path) || fs.exists(&path) {
            path = format!("{device_dir}/c-{date}-{}-{k}{SEGMENT_EXT}", opts.session);
            k += 1;
        }
        let header = serde_json::to_string(&SegmentHeader {
            journal: JOURNAL_FORMAT,
            writer: opts.writer.to_string(),
            app: Some(opts.app.to_string()),
        })
        .unwrap_or_default();
        let body: Vec<String> = kept
            .iter()
            .map(|tx| serde_json::to_string(tx).unwrap_or_else(|_| "null".to_string()))
            .collect();
        let content = format!("{header}\n{}\n{{\"seal\":true}}\n", body.join("\n"));
        let tmp = format!("{path}.tmp");
        fs.write(&tmp, &content, None)?;
        fs.rename(&tmp, &path)?;
        let mut cited: Vec<String> = Vec::new();
        for tx in kept {
            for sha in cited_objects(&tx.ops) {
                if !cited.contains(&sha) {
                    cited.push(sha);
                }
            }
        }
        result = Some(CompactedSegment {
            path,
            size: byte_length(&content),
            lines: kept.len() + 2,
            objects: cited,
        });
    }

    for src in sources {
        let _ = fs.remove(src); // déjà parti : tant mieux
    }
    Ok(result)
}
