// Port de app/src/ui/core/workspace/types.ts
//
// Les types du modèle du workspace qui ne sont que des DONNÉES : l'état sérialisable
// d'une feuille, les options d'ouverture, la direction d'un split. Les classes
// (Workspace, WorkspaceLeaf, View…) viennent avec la tâche qui porte le Vault.
//
// Sous-modules : `layout` (la forme sérialisée), `arbre` (l'arène des nœuds), `pile` (les
// onglets et la découpe), `redimension` (les calculs purs du geste de redimensionnement),
// `vue` (View, ItemView, vue vide, coquille différée), `feuille` (WorkspaceLeaf), `espace` (le
// Workspace vivant), `restauration` (la persistance du layout), `commandes` (workspace:*).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// ★ `PaneType` est défini dans `keymap::types` (le Keymap le rend) : on le réexporte.
pub use crate::keymap::PaneType;

pub mod arbre;
pub mod commandes;
pub mod espace;
pub mod feuille;
pub mod layout;
pub mod menu_pane;
pub mod pile;
pub mod redimension;
pub mod restauration;
pub mod vue;
pub mod vue_fichier;

/// L'état SÉRIALISABLE d'une feuille (§8.2). Ce qui finit dans workspace.json.
///
/// Que des données : un type de vue par son nom, un état opaque appartenant à la vue.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ViewState {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
}

impl ViewState {
    /// Un état sans données : `{ type }`.
    pub fn new(type_: impl Into<String>) -> Self {
        Self {
            type_: type_.into(),
            ..Self::default()
        }
    }

    /// Un état avec ses données (objet JSON ; autre valeur → pas d'état).
    pub fn with_state(type_: impl Into<String>, state: Value) -> Self {
        Self {
            type_: type_.into(),
            state: match state {
                Value::Object(m) => Some(m),
                _ => None,
            },
            ..Self::default()
        }
    }
}

/// Les options d'une ouverture. `e_state` porte l'état ÉPHÉMÈRE (§8.2) — défilement,
/// sélection, ligne à atteindre : ce qui survit à la navigation mais n'a rien à faire
/// dans un fichier de layout.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OpenViewState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Map<String, Value>>,
    #[serde(default, rename = "eState", skip_serializing_if = "Option::is_none")]
    pub e_state: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// Rendu par `View::set_state`. `history: true` dit au Workspace que ce changement
/// mérite une entrée dans l'historique de navigation de la feuille. Charger un
/// fichier, oui ; restaurer un layout au démarrage, non.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ViewStateResult {
    pub history: bool,
}

/// Découpe d'un split (`WorkspaceSplit.ts`), sérialisée `"horizontal"` / `"vertical"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── Ajoutés : aucun test TS (types.ts ne porte que des interfaces) ───
    #[test]
    fn view_state_roundtrip_json_fragment() {
        let v: ViewState = serde_json::from_value(json!({
            "type": "markdown", "state": {"file": "a.md"}, "pinned": true
        }))
        .unwrap();
        assert_eq!(v.type_, "markdown");
        assert_eq!(v.pinned, Some(true));
        assert_eq!(
            serde_json::to_value(&v).unwrap(),
            json!({"type": "markdown", "state": {"file": "a.md"}, "pinned": true})
        );
    }

    #[test]
    fn open_view_state_utilise_estate() {
        let o: OpenViewState = serde_json::from_value(json!({"eState": {"line": 3}})).unwrap();
        assert_eq!(o.e_state.unwrap()["line"], json!(3));
    }

    #[test]
    fn split_direction_en_minuscules() {
        assert_eq!(
            serde_json::to_string(&SplitDirection::Vertical).unwrap(),
            "\"vertical\""
        );
    }
}
