// Port de app/src/ui/core/artefacts/journal/import.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  La migration depuis l'existant (§18) — une seule fois, déterministe.
//
//      refs/<plugin>/<id>.json  ─lit─▶  journal/import/0-refs.ndjson
//      objects/<sha>.json       ─────▶  inchangé
//      refs/                    ─────▶  refs.migrated/  (gardé, plus jamais lu)
//
//  1. Lire chaque ref, et son objet quand le state tient sous le seuil.
//  2. Une ligne par artefact ; estampille = updatedAt, compteur à zéro,
//     écrivain `import` ; lignes triées par id.
//  3. Renommer refs/ en refs.migrated/. Ne rien supprimer.
//  4. Laisser objects/ : les objets devenus inutiles partiront au premier
//     balayage, après le délai de grâce.
//
//  ★ Deux appareils qui lancent l'import chacun de leur côté produisent le même
//    fichier, ligne pour ligne : l'outil de synchronisation n'a rien à arbitrer.
//    Si un fichier d'import existe déjà (un ref en retard est arrivé par la
//    synchro), on fusionne par id et on réécrit — toujours de façon déterministe.
// ═══════════════════════════════════════════════════════════════════════════

use crate::cache::JournalKind;
use crate::clock::format_stamp;
use crate::fs::SharedFs;
use crate::objects::{canonical_json, fits_inline};
use crate::types::{ArtefactScope, DurableAnchor, Op, Origin, SetOp, Tx};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;

use super::reader::{JOURNAL_FORMAT, SEGMENT_EXT, SegmentHeader, empty_cursor, parse_segment};

/// L'écrivain des lignes importées.
pub const IMPORT_WRITER: &str = "import";

/// L'ancienne enveloppe, telle qu'elle vivait dans `refs/<plugin>/<id>.json`.
/// (L'ancien indice `char` de l'ancre n'était relu par personne : serde l'ignore.)
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LegacyRef {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub version: Option<u32>,
    pub scope: ArtefactScope,
    #[serde(default)]
    pub anchor: Option<DurableAnchor>,
    pub head: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<f64>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<f64>,
}

impl LegacyRef {
    /// Un ref minimal (les tests).
    pub fn new(id: &str, type_: &str, scope: ArtefactScope, head: &str, updated_at: f64) -> Self {
        LegacyRef {
            id: id.into(),
            type_: type_.into(),
            version: None,
            scope,
            anchor: None,
            head: head.into(),
            updated_at: Some(updated_at),
            created_at: None,
        }
    }
}

/// Le chemin du fichier d'import.
pub fn import_path(base: &str) -> String {
    format!("{base}/journal/{IMPORT_WRITER}/0-refs{SEGMENT_EXT}")
}

/// La ligne d'un ref : un `set` de tous les groupes, à la date de sa dernière écriture.
pub fn legacy_line(r: &LegacyRef, state: Option<&Value>) -> Tx {
    let mut op = SetOp::new(&r.id);
    op.type_ = Some(r.type_.clone());
    op.v = Some(r.version.unwrap_or(1));
    op.scope = Some(r.scope.clone());
    match state {
        Some(s) if fits_inline(s) => op.state = Some(s.clone()),
        _ => op.head = Some(r.head.clone()),
    }
    if let Some(a) = &r.anchor {
        op.anchor = Some(Some(a.clone()));
    }
    let ms = r.updated_at.or(r.created_at).unwrap_or(0.0) as i64;
    Tx {
        t: format_stamp(ms, 0, IMPORT_WRITER),
        o: Origin::Import,
        ops: vec![Op::Set(op)],
    }
}

/// Le contenu du fichier d'import : en-tête, lignes triées par id, fermeture.
pub fn import_file_content(lines: &[Tx]) -> String {
    let mut sorted: Vec<&Tx> = lines.iter().collect();
    let id_of = |tx: &Tx| -> String {
        tx.ops
            .first()
            .and_then(|op| op.as_set())
            .map(|s| s.set.clone())
            .unwrap_or_default()
    };
    sorted.sort_by_key(|tx| id_of(tx));
    let header = serde_json::to_string(&SegmentHeader {
        journal: JOURNAL_FORMAT,
        writer: IMPORT_WRITER.to_string(),
        app: None,
    })
    .unwrap_or_default();
    let body: Vec<String> = sorted
        .iter()
        .map(|tx| canonical_json(&serde_json::to_value(tx).unwrap_or(Value::Null)))
        .collect();
    let nl = if sorted.is_empty() { "" } else { "\n" };
    format!("{header}\n{}{nl}{{\"seal\":true}}\n", body.join("\n"))
}

/// Importe `refs/` s'il existe. Rend `true` si un import a eu lieu (le fichier
/// d'import a été (ré)écrit). Rend aussi les refs illisibles ignorés (diagnostic).
pub fn import_legacy_refs(fs: &SharedFs, base: &str) -> io::Result<bool> {
    let refs_dir = format!("{base}/refs");
    if !fs.exists(&refs_dir) {
        return Ok(false);
    }

    // Les lignes déjà importées (un import précédent, peut-être d'un autre appareil).
    let mut by_id: BTreeMap<String, Tx> = BTreeMap::new();
    let target = import_path(base);
    // (illisible : on le reconstruit)
    if fs.exists(&target)
        && let Ok(text) = fs.read(&target)
    {
        for tx in parse_segment(&text, &empty_cursor(JournalKind::Vault, &target)).txs {
            if let Some(id) = tx
                .ops
                .first()
                .and_then(|op| op.as_set())
                .map(|s| s.set.clone())
            {
                by_id.insert(id, tx);
            }
        }
    }

    for plugin_dir in fs.list(&refs_dir).folders {
        for f in fs.list(&plugin_dir).files {
            if !f.ends_with(".json") {
                continue; // un .tmp laissé par un crash
            }
            let Ok(text) = fs.read(&f) else { continue };
            let Ok(r) = serde_json::from_str::<LegacyRef>(&text) else {
                continue; // ref illisible ignoré
            };
            // objet absent : la ligne citera l'empreinte
            let state: Option<Value> = fs
                .read(&format!("{base}/objects/{}.json", r.head))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());
            let line = legacy_line(&r, state.as_ref());
            // Le plus récent gagne, à égalité la ligne déjà là : le résultat ne dépend pas de l'ordre.
            let newer = by_id.get(&r.id).is_none_or(|prev| line.t > prev.t);
            if newer {
                by_id.insert(r.id.clone(), line);
            }
        }
    }

    fs.mkdir(&format!("{base}/journal/{IMPORT_WRITER}"))?;
    let tmp = format!("{target}.tmp");
    let lines: Vec<Tx> = by_id.into_values().collect();
    fs.write(&tmp, &import_file_content(&lines), None)?;
    let _ = fs.remove(&target);
    fs.rename(&tmp, &target)?;

    // refs/ est renommé et gardé, pas supprimé.
    let mut dest = format!("{base}/refs.migrated");
    let mut k = 2;
    while fs.exists(&dest) {
        dest = format!("{base}/refs.migrated-{k}");
        k += 1;
    }
    fs.rename(&refs_dir, &dest)?;
    Ok(true)
}
