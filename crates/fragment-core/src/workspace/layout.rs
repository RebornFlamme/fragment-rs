// Port de app/src/ui/core/workspace/layout.ts
//
// §8.2 — la forme SÉRIALISÉE de l'arbre de fenêtrage, telle qu'elle atterrit dans
// `.fragment/workspace.json`. Trois formes de nœud (split, tabs, leaf). Aucune
// référence d'objet : ce fichier doit se relire après un redémarrage — et par Fragment
// (TS), le fichier est partagé : les noms de champs JSON sont ceux du TS (`eState`,
// `currentTab`…).
//
// ★ Une feuille porte AUSSI l'état éphémère de sa vue (`eState` : défilement, page /
//   zoom d'un PDF…) : la LECTURE survit au redémarrage. Un layout ancien, sans
//   `eState`, se relit tel quel.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{SplitDirection, ViewState};

/// Un split. `collapsed` / `width` : sidedocks seulement (`width` = `--sidebar-width`, px).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitLayout {
    pub direction: SplitDirection,
    pub children: Vec<LayoutNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// La part de flex-grow du nœud dans son split parent. Posée par le PARENT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsLayout {
    pub children: Vec<LayoutNode>,
    pub current_tab: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeafLayout {
    pub id: String,
    pub state: ViewState,
    /// L'état éphémère de la vue (sa lecture), rendu à `set_deferred_state` au
    /// redémarrage. ★ Valeur libre : une lecture illisible (écrite à la main) ne doit
    /// pas faire tomber le layout, elle est ignorée à la reconstruction.
    #[serde(default, rename = "eState", skip_serializing_if = "Option::is_none")]
    pub e_state: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<f64>,
}

/// Trois formes de nœud, discriminées par `type` (comme dans workspace.json).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayoutNode {
    Split(SplitLayout),
    Tabs(TabsLayout),
    Leaf(LeafLayout),
}

/// Le document entier. `main` / `left` / `right` sont les trois racines permanentes du
/// Workspace — jamais créées ni détruites par la restauration, seulement remplies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    /// Toujours 1 ; un autre numéro est refusé à la lecture (`is_workspace_layout`).
    pub version: u8,
    #[serde(with = "racine")]
    pub main: SplitLayout,
    #[serde(with = "racine")]
    pub left: SplitLayout,
    #[serde(with = "racine")]
    pub right: SplitLayout,
    /// L'id de la feuille active, ou `None` (`null` dans le JSON, JAMAIS absent).
    pub active: Option<String>,
}

/// ★ Une racine est un split AVEC son `type: "split"` dans le JSON (le TS la type en
///   `SplitLayout`) : on passe par `LayoutNode` pour écrire/relire ce discriminant.
mod racine {
    use super::{LayoutNode, SplitLayout};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &SplitLayout, s: S) -> Result<S::Ok, S::Error> {
        LayoutNode::Split(v.clone()).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SplitLayout, D::Error> {
        match LayoutNode::deserialize(d)? {
            LayoutNode::Split(v) => Ok(v),
            _ => Err(serde::de::Error::custom("la racine doit être un split")),
        }
    }
}

impl WorkspaceLayout {
    /// Lit un layout depuis du JSON du disque : validation défensive PUIS conversion.
    /// `None` = layout par défaut, comme un coffre neuf.
    pub fn from_value(value: &Value) -> Option<Self> {
        if !is_workspace_layout(value) {
            return None;
        }
        serde_json::from_value(value.clone()).ok()
    }
}

/// Validation défensive : ce JSON vient du disque, où n'importe qui a pu passer (édition
/// à la main, coupure d'écriture, version antérieure du format). Un layout invalide ne
/// doit jamais empêcher l'app de démarrer — elle retombe sur son layout par défaut.
///
/// On valide la STRUCTURE, pas le contenu : qu'un `ViewState` référence un type de vue
/// inconnu ou un fichier disparu est un cas NORMAL (§8.3), traité en aval.
///
/// ★ `active` doit être PRÉSENT (chaîne ou `null`) : l'absence est refusée, comme le TS
///   (`l.active === null || typeof l.active === 'string'`).
pub fn is_workspace_layout(value: &Value) -> bool {
    let Some(l) = value.as_object() else {
        return false;
    };
    if l.get("version").and_then(Value::as_f64) != Some(1.0) {
        return false;
    }
    let active_ok = matches!(l.get("active"), Some(Value::Null | Value::String(_)));
    ["main", "left", "right"]
        .iter()
        .all(|k| l.get(*k).is_some_and(is_split))
        && active_ok
}

fn is_split(node: &Value) -> bool {
    is_node(node) && node.get("type").and_then(Value::as_str) == Some("split")
}

fn is_node(value: &Value) -> bool {
    let Some(n) = value.as_object() else {
        return false;
    };
    match n.get("type").and_then(Value::as_str) {
        Some("leaf") => {
            let id_ok = n.get("id").is_some_and(Value::is_string);
            let state_ok = n
                .get("state")
                .and_then(Value::as_object)
                .is_some_and(|s| s.get("type").is_some_and(Value::is_string));
            id_ok && state_ok
        }
        Some(t @ ("split" | "tabs")) => {
            if t == "split"
                && !matches!(
                    n.get("direction").and_then(Value::as_str),
                    Some("horizontal" | "vertical")
                )
            {
                return false;
            }
            n.get("children")
                .and_then(Value::as_array)
                .is_some_and(|c| c.iter().all(is_node))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn feuille(id: &str, type_: &str) -> Value {
        json!({"type": "leaf", "id": id, "state": {"type": type_, "state": {"file": format!("{id}.md")}}})
    }

    fn valide() -> Value {
        json!({
            "version": 1,
            "main": {
                "type": "split", "direction": "vertical",
                "children": [
                    {"type": "tabs", "currentTab": 0, "children": [feuille("a", "markdown"), feuille("b", "markdown")], "dimension": 2},
                    {"type": "split", "direction": "horizontal", "dimension": 1,
                     "children": [{"type": "tabs", "currentTab": 0, "children": [feuille("c", "markdown")]}]}
                ]
            },
            "left": {"type": "split", "direction": "horizontal", "children": [], "collapsed": true, "width": 280},
            "right": {"type": "split", "direction": "horizontal", "children": []},
            "active": "a"
        })
    }

    /// Remplace les enfants de la première pile de `main`.
    fn avec_feuilles(l: &mut Value, feuilles: Vec<Value>) {
        l["main"]["children"][0]["children"] = Value::Array(feuilles);
    }

    // ─── isWorkspaceLayout — ce qui passe ───

    #[test]
    fn accepte_un_layout_complet_splits_imbriques_compris() {
        assert!(is_workspace_layout(&valide()));
        assert!(WorkspaceLayout::from_value(&valide()).is_some());
    }

    #[test]
    fn accepte_des_racines_vides_un_coffre_sans_onglet_reste_un_layout() {
        let mut l = valide();
        l["main"]["children"] = json!([]);
        l["active"] = Value::Null;
        assert!(is_workspace_layout(&l));
    }

    #[test]
    fn accepte_une_feuille_qui_porte_sa_lecture_estate_et_une_sans_layout_ancien() {
        let mut l = valide();
        let mut a = feuille("a", "markdown");
        a["eState"] = json!({"scroll": {"line": 120, "dy": 4}, "activeLayers": ["agent"]});
        avec_feuilles(&mut l, vec![a, feuille("b", "markdown")]);
        assert!(is_workspace_layout(&l));
    }

    #[test]
    fn une_lecture_illisible_ecrite_a_la_main_ne_fait_pas_tomber_le_layout() {
        let mut l = valide();
        let mut a = feuille("a", "markdown");
        a["eState"] = json!("n’importe quoi");
        avec_feuilles(&mut l, vec![a]);
        assert!(is_workspace_layout(&l));
        // ★ Et la conversion typée la garde telle quelle (valeur libre).
        assert!(WorkspaceLayout::from_value(&l).is_some());
    }

    #[test]
    fn accepte_un_type_de_vue_inconnu_cest_un_etat_normal_du_8_3() {
        let mut l = valide();
        avec_feuilles(&mut l, vec![feuille("a", "kanban")]);
        assert!(is_workspace_layout(&l));
    }

    // ─── isWorkspaceLayout — ce qui tombe sur le layout par défaut (it.each) ───

    #[test]
    fn refuse_null() {
        assert!(!is_workspace_layout(&Value::Null));
    }

    #[test]
    fn refuse_un_tableau() {
        assert!(!is_workspace_layout(&json!([])));
    }

    #[test]
    fn refuse_une_chaine() {
        assert!(!is_workspace_layout(&json!("{\"version\":1}")));
    }

    #[test]
    fn refuse_un_objet_vide() {
        assert!(!is_workspace_layout(&json!({})));
    }

    #[test]
    fn refuse_une_version_inconnue_on_ne_devine_pas_un_format_futur() {
        let mut l = valide();
        l["version"] = json!(2);
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn refuse_une_racine_manquante() {
        let mut l = valide();
        l.as_object_mut().unwrap().remove("right");
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn refuse_une_racine_qui_nest_pas_un_split() {
        let mut l = valide();
        l["left"] = json!({"type": "tabs", "currentTab": 0, "children": []});
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn refuse_une_direction_de_split_absurde() {
        let mut l = valide();
        l["main"]["direction"] = json!("diagonale");
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn refuse_une_feuille_sans_viewstate_cest_ce_quune_ecriture_coupee_produit() {
        let mut l = valide();
        avec_feuilles(&mut l, vec![json!({"type": "leaf", "id": "a"})]);
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn refuse_un_noeud_invalide_en_profondeur_pas_seulement_a_la_racine() {
        let mut l = valide();
        l["main"]["children"][1]["children"] =
            json!([{"type": "tabs", "currentTab": 0, "children": [{"type": "grille"}]}]);
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn refuse_children_qui_nest_pas_un_tableau() {
        let mut l = valide();
        l["main"]["children"] = json!("deux");
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn refuse_un_active_qui_nest_ni_une_chaine_ni_null() {
        let mut l = valide();
        l["active"] = json!(3);
        assert!(!is_workspace_layout(&l));
    }

    // ─── Ajoutés ───

    #[test]
    fn refuse_un_active_absent_comme_le_ts() {
        let mut l = valide();
        l.as_object_mut().unwrap().remove("active");
        assert!(!is_workspace_layout(&l));
    }

    #[test]
    fn le_layout_typed_se_reserialise_a_lidentique() {
        let l = valide();
        let typed = WorkspaceLayout::from_value(&l).unwrap();
        assert_eq!(typed.active.as_deref(), Some("a"));
        assert_eq!(typed.left.width, Some(280.0));
        // ★ Les nombres repassent en flottants (`280.0`) : le JS les relit à l'identique,
        //   on compare donc l'aller-retour typé, pas le texte brut.
        let retour = serde_json::to_value(&typed).unwrap();
        assert_eq!(WorkspaceLayout::from_value(&retour).unwrap(), typed);
    }
}
