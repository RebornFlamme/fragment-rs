// Port de app/src/ui/core/artefacts/objects.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les objets adressés par contenu (§14.5) — repris de l'ancien FsArtefactStore,
//  réservés désormais aux GROS states (> 2 Kio de JSON canonique) et aux blobs.
//  Les petits voyagent dans la ligne du journal.
//
//    objects/<sha>.json   ← immuable, partagé entre tous les écrivains
//
//  ★ Un objet est écrit AVANT la ligne qui le cite (invariant §19.1-4).
//  ★ Il ne part qu'au balayage (§15.10) : plus aucune ligne présente sur le
//    disque ne le cite, ET son fichier a plus de quatorze jours (gc.pruneExpire
//    de git). Citer de nouveau un objet présent rafraîchit son mtime.
// ═══════════════════════════════════════════════════════════════════════════

use crate::fs::SharedFs;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;

/// Le seuil inline/objet : 2 Kio de JSON canonique (§13).
pub const INLINE_MAX_BYTES: usize = 2 * 1024;

/// Le délai de grâce avant de supprimer un objet que plus rien ne cite (§13).
pub const GRACE_MS: i64 = 14 * 24 * 60 * 60 * 1000;

/// JSON canonique : clefs triées récursivement → un même contenu produit les mêmes
/// octets, donc le même hash, quel que soit l'ordre d'insertion des clefs.
///
/// ★ Trié explicitement, sans compter sur la `Map` de serde_json (qui garde l'ordre
///   d'insertion si une autre crate de l'espace de travail active `preserve_order`).
pub fn canonical_json(value: &Value) -> String {
    serde_json::to_string(&sort_keys(value)).unwrap_or_else(|_| "null".to_string())
}

fn sort_keys(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(sort_keys).collect()),
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                if let Some(x) = o.get(k) {
                    out.insert(k.clone(), sort_keys(x));
                }
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}

/// La taille en octets d'une chaîne UTF-8.
pub fn byte_length(text: &str) -> usize {
    text.len()
}

/// Le state tient-il dans la ligne ?
pub fn fits_inline(state: &Value) -> bool {
    byte_length(&canonical_json(state)) <= INLINE_MAX_BYTES
}

pub fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Un cache borné « le moins récemment utilisé » (le `Lru` de core/lru.ts, réduit
/// à ce dont les objets ont besoin).
struct Lru {
    cap: usize,
    tick: u64,
    map: HashMap<String, (Value, u64)>,
}

impl Lru {
    fn new(cap: usize) -> Self {
        Lru {
            cap,
            tick: 0,
            map: HashMap::new(),
        }
    }

    fn get(&mut self, k: &str) -> Option<Value> {
        self.tick += 1;
        let tick = self.tick;
        let e = self.map.get_mut(k)?;
        e.1 = tick;
        Some(e.0.clone())
    }

    fn set(&mut self, k: &str, v: Value) {
        self.tick += 1;
        self.map.insert(k.to_string(), (v, self.tick));
        if self.map.len() > self.cap
            && let Some(oldest) = self
                .map
                .iter()
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| k.clone())
        {
            self.map.remove(&oldest);
        }
    }

    fn delete(&mut self, k: &str) {
        self.map.remove(k);
    }
}

pub struct ObjectStore {
    fs: SharedFs,
    dir: String,
    /// Les states déjà lus ou écrits cette session — un objet ne change jamais.
    memo: RefCell<Lru>,
}

impl ObjectStore {
    pub fn new(fs: SharedFs, base: &str) -> Self {
        ObjectStore {
            fs,
            dir: format!("{base}/objects"),
            memo: RefCell::new(Lru::new(128)),
        }
    }

    pub fn dir(&self) -> &str {
        &self.dir
    }

    pub fn path(&self, sha: &str) -> String {
        format!("{}/{sha}.json", self.dir)
    }

    /// Écrit le state comme objet (s'il est absent) et rend son empreinte. S'il est
    /// déjà là, son mtime est rafraîchi : le délai de grâce repart de zéro, comme
    /// quand on redonne à git un objet qu'il possède déjà.
    pub fn put(&self, state: &Value, now: i64) -> io::Result<String> {
        let json = canonical_json(state);
        let sha = sha256_hex(&json);
        let path = self.path(&sha);
        self.fs.mkdir(&self.dir)?;
        if self.fs.exists(&path) {
            // Même contenu, mtime neuf : l'objet est immuable, le réécrire à
            // l'identique ne change que sa date.
            self.fs.write(&path, &json, Some(now))?;
        } else {
            // Nom temporaire puis renommage : un objet n'est jamais vu tronqué.
            let tmp = format!("{path}.tmp");
            self.fs.write(&tmp, &json, None)?;
            if self.fs.rename(&tmp, &path).is_err() {
                // Un autre écrivain l'a posé entre-temps : même contenu, on garde le sien.
                let _ = self.fs.remove(&tmp);
            }
        }
        self.remember(&sha, state.clone());
        Ok(sha)
    }

    /// Le state d'un objet, ou `None` s'il n'est pas (encore) là.
    pub fn get(&self, sha: &str) -> Option<Value> {
        if let Some(v) = self.memo.borrow_mut().get(sha) {
            return Some(v);
        }
        let text = self.fs.read(&self.path(sha)).ok()?;
        let state: Value = serde_json::from_str(&text).ok()?;
        self.remember(sha, state.clone());
        Some(state)
    }

    /// Garde un state en mémoire sous son empreinte (cache borné).
    pub fn remember(&self, sha: &str, state: Value) {
        self.memo.borrow_mut().set(sha, state);
    }

    /// Le balayage (§15.10). `cited` : toutes les empreintes citées par une ligne
    /// présente sur le disque. Un objet non cité n'est supprimé que si son fichier a
    /// plus de quatorze jours. Rend le nombre d'objets supprimés.
    pub fn sweep(&self, cited: &HashSet<String>, now: i64) -> usize {
        if !self.fs.exists(&self.dir) {
            return 0;
        }
        let mut removed = 0;
        for f in self.fs.list(&self.dir).files {
            let Some(stem) = f.strip_suffix(".json") else {
                continue;
            };
            let sha = stem.rsplit('/').next().unwrap_or(stem);
            if cited.contains(sha) {
                continue;
            }
            let Some(s) = self.fs.stat(&f) else { continue };
            if now - s.mtime <= GRACE_MS {
                continue;
            }
            if self.fs.remove(&f).is_ok() {
                self.memo.borrow_mut().delete(sha);
                removed += 1;
            }
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Now;
    use crate::fs::{JournalFs, MemoryFs};
    use serde_json::json;
    use std::rc::Rc;

    #[test]
    fn json_canonique_trie_les_clefs_a_tous_les_niveaux() {
        let v = json!({"b": 1, "a": {"z": [ {"y": 1, "x": 2} ], "c": null}});
        assert_eq!(
            canonical_json(&v),
            r#"{"a":{"c":null,"z":[{"x":2,"y":1}]},"b":1}"#
        );
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert!(fits_inline(&json!({"text": "x".repeat(2000)})));
        assert!(!fits_inline(&json!({"text": "x".repeat(3000)})));
    }

    #[test]
    fn put_ecrit_une_fois_puis_rafraichit_le_mtime_et_sweep_respecte_la_grace() {
        let t = Rc::new(std::cell::Cell::new(1_000i64));
        let now: Now = {
            let t = t.clone();
            Rc::new(move || t.get())
        };
        let mem = Rc::new(MemoryFs::new(now));
        let fs: SharedFs = mem.clone();
        let store = ObjectStore::new(fs, ".fragment/artefacts");
        let sha = store.put(&json!({"a": 1}), 1_000).unwrap();
        let path = store.path(&sha);
        assert_eq!(mem.get(&path).as_deref(), Some(r#"{"a":1}"#));
        assert_eq!(mem.mtime(&path), Some(1_000));
        store.put(&json!({"a": 1}), 5_000).unwrap();
        assert_eq!(mem.mtime(&path), Some(5_000));
        assert_eq!(store.get(&sha), Some(json!({"a": 1})));
        assert_eq!(store.get("absent"), None);

        mem.write(".fragment/artefacts/objects/orphelin.json", "{}", Some(0))
            .unwrap();
        let cited: HashSet<String> = [sha.clone()].into_iter().collect();
        assert_eq!(store.sweep(&cited, GRACE_MS), 0);
        assert_eq!(store.sweep(&cited, GRACE_MS + 1), 1);
        assert!(!mem.has(".fragment/artefacts/objects/orphelin.json"));
        assert!(mem.has(&path));
    }
}
