// Port de app/src/ui/core/artefacts/journal/reader.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Lecture du journal (§14.1-14.2, §15.3, §15.4).
//
//  Chemin d'un segment : journal/<appareil>/<date>-<session>.ndjson. Contenu : un
//  en-tête, une transaction par ligne et, après une fermeture propre, `{"seal":true}`.
//  Seules les lignes situées après le curseur sont pliées. Une dernière ligne tronquée
//  (crash, copie en cours) ne fait pas avancer le curseur : on la relira entière plus
//  tard, et les lignes précédentes restent bonnes.
// ═══════════════════════════════════════════════════════════════════════════

use crate::cache::{JournalKind, SegmentCursor};
use crate::fs::SharedFs;
use crate::objects::byte_length;
use crate::types::{Op, Tx};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::Cell;

/// L'extension d'un segment. Tout autre fichier (`.tmp`, copie de conflit renommée…) est ignoré…
pub const SEGMENT_EXT: &str = ".ndjson";

/// La version du format de ligne, portée par l'en-tête.
pub const JOURNAL_FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentHeader {
    pub journal: u32,
    pub writer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
}

/// La clé de curseur d'un segment.
pub fn cursor_key(journal: JournalKind, path: &str) -> String {
    format!("{}:{path}", journal.as_str())
}

/// Un curseur vierge.
pub fn empty_cursor(journal: JournalKind, path: &str) -> SegmentCursor {
    SegmentCursor {
        key: cursor_key(journal, path),
        journal,
        path: path.to_string(),
        size: 0,
        lines: 0,
        sealed: false,
        objects: Vec::new(),
    }
}

/// Les empreintes d'objets citées par des opérations.
pub fn cited_objects(ops: &[Op]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| op.as_set().and_then(|s| s.head.clone()))
        .collect()
}

/// Une ligne est-elle une transaction bien formée ? (`t` chaîne, `o` origine, `ops` tableau.)
pub fn parse_tx(v: &Value) -> Option<Tx> {
    serde_json::from_value(v.clone()).ok()
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSegment {
    /// Les transactions des lignes nouvelles.
    pub txs: Vec<Tx>,
    /// Le curseur après lecture.
    pub cursor: SegmentCursor,
    /// Les lignes illisibles ignorées (le TS les signale en console).
    pub unreadable: usize,
}

/// Découpe le texte d'un segment et rend les transactions situées APRÈS le
/// curseur. Pur (testé) : la lecture disque est à côté.
pub fn parse_segment(text: &str, cursor: &SegmentCursor) -> ParsedSegment {
    let parts: Vec<&str> = text.split('\n').collect();
    // Le dernier morceau est soit '' (fichier terminé par un saut de ligne), soit
    // une ligne incomplète : dans les deux cas, il n'est pas lu.
    let complete = &parts[..parts.len() - 1];
    let size = if complete.is_empty() {
        0
    } else {
        complete.iter().map(|l| byte_length(l) + 1).sum()
    };

    // Le fichier a rétréci ou a été remplacé : on relit tout (le pli est idempotent).
    let mut start = cursor.lines;
    let mut objects = cursor.objects.clone();
    let mut sealed = cursor.sealed;
    if size < cursor.size || complete.len() < cursor.lines {
        start = 0;
        objects = Vec::new();
        sealed = false;
    }

    let mut txs = Vec::new();
    let mut unreadable = 0;
    for line in complete.iter().skip(start) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        if let Some(tx) = parse_tx(&v) {
            for sha in cited_objects(&tx.ops) {
                if !objects.contains(&sha) {
                    objects.push(sha);
                }
            }
            txs.push(tx);
        } else if v.get("seal") == Some(&Value::Bool(true)) {
            sealed = true;
        }
        // l'en-tête (`journal`) n'apporte rien au pli
    }
    ParsedSegment {
        txs,
        cursor: SegmentCursor {
            size,
            lines: complete.len(),
            sealed,
            objects,
            ..cursor.clone()
        },
        unreadable,
    }
}

pub struct JournalReader {
    fs: SharedFs,
    dir: String,
    pub journal: JournalKind,
    /// Combien de lignes de transaction ont été lues du disque (le compteur `linesRead`).
    lines_read: Cell<usize>,
}

impl JournalReader {
    pub fn new(fs: SharedFs, base: &str, journal: JournalKind) -> Self {
        JournalReader {
            fs,
            dir: format!("{base}/journal"),
            journal,
            lines_read: Cell::new(0),
        }
    }

    pub fn journal_dir(&self) -> &str {
        &self.dir
    }

    pub fn lines_read(&self) -> usize {
        self.lines_read.get()
    }

    /// Le chemin est-il un segment de ce journal ?
    pub fn is_segment(&self, path: &str) -> bool {
        path.starts_with(&format!("{}/", self.dir)) && path.ends_with(SEGMENT_EXT)
    }

    /// Les dossiers d'écrivains (un par appareil, plus `import`). Un `list` par appareil.
    pub fn list_segments(&self) -> Vec<String> {
        if !self.fs.exists(&self.dir) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for folder in self.fs.list(&self.dir).folders {
            for f in self.fs.list(&folder).files {
                if f.ends_with(SEGMENT_EXT) {
                    out.push(f);
                }
            }
        }
        out.sort();
        out
    }

    /// La taille d'un segment (stat), ou `None` s'il a disparu.
    pub fn size(&self, path: &str) -> Option<usize> {
        self.fs.stat(path).map(|s| s.size as usize)
    }

    /// Lit un segment et rend les transactions nouvelles. `None` si le fichier est
    /// illisible : l'appelant le note (le GC est alors annulé pour la session).
    pub fn read(&self, path: &str, cursor: Option<&SegmentCursor>) -> Option<ParsedSegment> {
        let text = self.fs.read(path).ok()?;
        let empty;
        let cursor = match cursor {
            Some(c) => c,
            None => {
                empty = empty_cursor(self.journal, path);
                &empty
            }
        };
        let parsed = parse_segment(&text, cursor);
        self.lines_read
            .set(self.lines_read.get() + parsed.txs.len());
        Some(parsed)
    }

    /// Toutes les transactions d'un segment (historique, compaction) — sans toucher au compteur.
    pub fn read_all(&self, path: &str) -> Option<Vec<Tx>> {
        let text = self.fs.read(path).ok()?;
        Some(parse_segment(&text, &empty_cursor(self.journal, path)).txs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_segment_ne_lit_que_les_lignes_nouvelles_et_ignore_la_ligne_incomplete() {
        let head = r#"{"journal":1,"writer":"k3f9-b71"}"#;
        let l1 = r#"{"t":"2026-09-29T17:20:00.000Z-0000-k3f9-b71","o":"user","ops":[{"set":"a","type":"t","v":1,"head":"abc"}]}"#;
        let l2 = r#"{"t":"2026-09-29T17:20:01.000Z-0000-k3f9-b71","o":"user","ops":[{"del":"a"}]}"#;
        let text = format!("{head}\n{l1}\n");
        let p = parse_segment(&text, &empty_cursor(JournalKind::Vault, "p"));
        assert_eq!(p.txs.len(), 1);
        assert_eq!(p.cursor.lines, 2);
        assert_eq!(p.cursor.size, text.len());
        assert_eq!(p.cursor.objects, vec!["abc"]);
        assert!(!p.cursor.sealed);

        let text2 = format!("{text}{l2}\npas du json\n{{\"seal\":true}}\n{{\"t\":\"tronq");
        let p2 = parse_segment(&text2, &p.cursor);
        assert_eq!(p2.txs.len(), 1);
        assert!(p2.txs[0].ops[0].is_del());
        assert_eq!(p2.unreadable, 1);
        assert!(p2.cursor.sealed);
        assert_eq!(p2.cursor.lines, 5);

        // Le fichier a rétréci : tout est relu.
        let p3 = parse_segment(&text, &p2.cursor);
        assert_eq!(p3.txs.len(), 1);
        assert!(!p3.cursor.sealed);
    }
}
