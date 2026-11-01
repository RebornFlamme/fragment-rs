// Port de app/src/ui/core/frontmatter.ts
//! Helpers frontmatter — la moitié « lecture » de l'API Obsidian visible dans la
//! capture (§14). `parse_front_matter_string_array` complète la surface publique
//! aux côtés de `parseFrontMatterEntry` (le jour où il arrive) : un plugin lit un
//! champ de frontmatter comme un tableau de chaînes, quelle que soit sa forme
//! d'origine (chaîne unique, tableau, nombre…).
//!
//! Le frontmatter est représenté par un objet JSON (`serde_json::Map`) : c'est la
//! forme dynamique qu'un YAML parsé prend de toute façon, et celle que l'API des
//! plugins reçoit.

use serde_json::{Map, Value};

/// Le `String(v)` de JavaScript, restreint aux valeurs qu'un frontmatter porte.
///
/// Un tableau ou un objet imbriqué est rendu en JSON compact — le TS donnait
/// `a,b` ou `[object Object]`, dont personne ne dépend.
fn en_chaine(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        autre => autre.to_string(),
    }
}

/// Lit `frontmatter[key]` comme un tableau de chaînes — la sémantique
/// d'`obsidian.parseFrontMatterStringArray`.
///
/// - `null`/absent → `None` (pas `[]` : « rien » et « liste vide » diffèrent) ;
/// - une valeur unique (chaîne, nombre…) → un tableau à un élément ;
/// - un tableau → chaque élément converti en chaîne ;
/// - `nospaces` (pour les tags, p.ex.) → retire tous les espaces de chaque entrée.
pub fn parse_front_matter_string_array(
    frontmatter: Option<&Map<String, Value>>,
    key: &str,
    nospaces: bool,
) -> Option<Vec<String>> {
    let value = frontmatter?.get(key)?;
    if value.is_null() {
        return None;
    }

    let result: Vec<String> = match value {
        Value::Array(items) => items.iter().map(en_chaine).collect(),
        unique => vec![en_chaine(unique)],
    };
    Some(if nospaces {
        result
            .into_iter()
            .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
            .collect()
    } else {
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn objet(v: Value) -> Map<String, Value> {
        match v {
            Value::Object(m) => m,
            _ => unreachable!("fixture : un objet JSON"),
        }
    }

    #[test]
    fn null_absent_null_rien_differe_de_liste_vide() {
        assert_eq!(parse_front_matter_string_array(None, "tags", false), None);
        assert_eq!(
            parse_front_matter_string_array(Some(&objet(json!({}))), "tags", false),
            None
        );
        assert_eq!(
            parse_front_matter_string_array(Some(&objet(json!({ "tags": null }))), "tags", false),
            None
        );
    }

    #[test]
    fn valeur_unique_tableau_a_un_element() {
        assert_eq!(
            parse_front_matter_string_array(Some(&objet(json!({ "tags": "note" }))), "tags", false),
            Some(vec!["note".to_owned()])
        );
        assert_eq!(
            parse_front_matter_string_array(Some(&objet(json!({ "n": 42 }))), "n", false),
            Some(vec!["42".to_owned()])
        );
    }

    #[test]
    fn tableau_chaque_element_en_chaine() {
        assert_eq!(
            parse_front_matter_string_array(
                Some(&objet(json!({ "tags": ["a", "b", 3] }))),
                "tags",
                false
            ),
            Some(vec!["a".to_owned(), "b".to_owned(), "3".to_owned()])
        );
    }

    #[test]
    fn nospaces_retire_tous_les_espaces_de_chaque_entree() {
        assert_eq!(
            parse_front_matter_string_array(
                Some(&objet(json!({ "tags": ["deux mots", "x"] }))),
                "tags",
                true
            ),
            Some(vec!["deuxmots".to_owned(), "x".to_owned()])
        );
    }
}
