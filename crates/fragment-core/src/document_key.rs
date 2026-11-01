// Port de app/src/ui/core/documents/documentKey.ts
//
// La forme d'une clé de document (`documents/types.ts`) — pur. Une clé est un chemin
// du coffre (`notes/a.md`, sans schéma) OU `schéma:reste` (`chat:agent/<id>`). Sans
// ambiguïté : `:` est interdit dans un nom de fichier du coffre.
//
// ★ SOURCE UNIQUE de `is_vault_key`, `move_key`, `is_under` : fragment-artefacts en
//   porte des copies marquées ★ (à dédoublonner, voir la PR fr-p1-core-b).

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Une adresse de document : chemin du coffre, ou `schéma:reste`.
///
/// Newtype de `String` (docs/contrats.md) ; sérialisé comme la chaîne nue, ce qui
/// garde `workspace.json` et les journaux d'artefacts relisibles par Fragment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentKey(String);

impl DocumentKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl Deref for DocumentKey {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for DocumentKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for DocumentKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocumentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for DocumentKey {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for DocumentKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl PartialEq<str> for DocumentKey {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for DocumentKey {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// `chat:agent/x` → `{scheme:"chat", rest:"agent/x"}` ; un chemin du coffre → schéma vide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDocumentKey<'a> {
    pub scheme: &'a str,
    pub rest: &'a str,
}

/// ★ Coupé au PREMIER deux-points ; un deux-points en tête (`:x`) n'est pas un schéma.
pub fn parse_document_key(key: &str) -> ParsedDocumentKey<'_> {
    match key.find(':') {
        Some(i) if i > 0 => ParsedDocumentKey {
            scheme: &key[..i],
            rest: &key[i + 1..],
        },
        _ => ParsedDocumentKey {
            scheme: "",
            rest: key,
        },
    }
}

pub fn format_document_key(scheme: &str, rest: &str) -> DocumentKey {
    if scheme.is_empty() {
        DocumentKey::new(rest)
    } else {
        DocumentKey::new(format!("{scheme}:{rest}"))
    }
}

/// Un chemin du coffre (sans schéma) ?
pub fn is_vault_key(key: &str) -> bool {
    parse_document_key(key).scheme.is_empty()
}

/// Applique un renommage `old_base → new_base` à une clé (elle-même OU sous `old_base/`), sinon `None`.
pub fn move_key(key: &str, old_base: &str, new_base: &str) -> Option<DocumentKey> {
    if key == old_base {
        return Some(DocumentKey::new(new_base));
    }
    let reste = key.strip_prefix(old_base)?;
    if reste.starts_with('/') {
        Some(DocumentKey::new(format!("{new_base}{reste}")))
    } else {
        None
    }
}

/// `key` est-elle `base` ou sous `base/` ?
pub fn is_under(key: &str, base: &str) -> bool {
    key == base
        || key
            .strip_prefix(base)
            .is_some_and(|reste| reste.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_chemin_du_coffre_na_pas_de_schema() {
        assert_eq!(
            parse_document_key("notes/a.md"),
            ParsedDocumentKey {
                scheme: "",
                rest: "notes/a.md"
            }
        );
        assert!(is_vault_key("notes/a.md"));
    }

    #[test]
    fn schema_reste_coupe_au_premier_deux_points() {
        assert_eq!(
            parse_document_key("chat:agent/x:y"),
            ParsedDocumentKey {
                scheme: "chat",
                rest: "agent/x:y"
            }
        );
        assert!(!is_vault_key("chat:agent/x"));
        assert_eq!(format_document_key("chat", "agent/x"), "chat:agent/x");
        assert_eq!(format_document_key("", "a.md"), "a.md");
    }

    #[test]
    fn move_key_la_cle_elle_meme_ou_ce_qui_est_dessous() {
        assert_eq!(move_key("a/b.md", "a", "z").unwrap(), "z/b.md");
        assert_eq!(move_key("a.md", "a.md", "b.md").unwrap(), "b.md");
        assert_eq!(move_key("ab/c.md", "a", "z"), None);
        assert_eq!(
            move_key("chat:p/c", "chat:p/c", "chat:p/d").unwrap(),
            "chat:p/d"
        );
    }

    #[test]
    fn is_under_test() {
        assert!(is_under("a/b", "a"));
        assert!(!is_under("ab", "a"));
    }

    // ─── Ajoutés ───
    #[test]
    fn deux_points_en_tete_nest_pas_un_schema() {
        assert_eq!(parse_document_key(":x").scheme, "");
    }

    #[test]
    fn la_cle_se_serialise_comme_une_chaine_nue() {
        let k = DocumentKey::new("chat:a/b");
        assert_eq!(serde_json::to_string(&k).unwrap(), "\"chat:a/b\"");
        let r: DocumentKey = serde_json::from_str("\"n.md\"").unwrap();
        assert_eq!(r, "n.md");
    }
}
