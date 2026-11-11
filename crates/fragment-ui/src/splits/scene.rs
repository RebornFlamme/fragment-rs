// Port de la traduction `layoutVide` de pixels/scenes/_t1.ts (côté Fragment) : l'arbre de scène
// des actions `layout`, vers la forme sérialisée d'un layout (version 1).
//
//! L'action de scène `layout` (événement `pixels:layout`) : un arbre d'onglets de vues vides.
//!
//! Forme de l'arbre (la même des deux côtés, `_t1.ts` et les JSON de `pixels/scenes/`) :
//! - `main` : une pile `{ "type": "tabs", "tabs": n, "current": i, "dimension": g }` ou un découpage
//!   `{ "type": "split", "direction": "vertical" | "horizontal", "children": […], "dimension": g }` ;
//! - `left`, `right` (optionnels) : `{ "collapsed": bool, "width": px, "tabs": n }`, repliés et vides
//!   par défaut. `tabs` (ajout de fr-t1-splits) donne au dock une pile de `n` onglets vides.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::{CommandManager, EventRef, Workspace, WorkspaceLayout};
use serde_json::{Map, Value, json};

/// L'événement que publie le lecteur de scènes pour l'action `layout`.
pub const EVENEMENT_LAYOUT: &str = "pixels:layout";

struct Compteur(u32);

impl Compteur {
    fn feuille(&mut self) -> Value {
        self.0 += 1;
        json!({
            "type": "leaf",
            "id": format!("pixels-feuille-{}", self.0),
            "state": { "type": "empty", "state": {} },
        })
    }

    fn pile(&mut self, n: usize, courant: usize, dimension: Option<&Value>) -> Value {
        let feuilles: Vec<Value> = (0..n).map(|_| self.feuille()).collect();
        let mut p = Map::new();
        p.insert("type".into(), json!("tabs"));
        p.insert("currentTab".into(), json!(courant));
        p.insert("children".into(), Value::Array(feuilles));
        if let Some(d) = dimension {
            p.insert("dimension".into(), d.clone());
        }
        Value::Object(p)
    }

    fn noeud(&mut self, n: &Value) -> Value {
        let dimension = n.get("dimension");
        if n.get("type").and_then(Value::as_str) == Some("split") {
            let enfants: Vec<Value> = n
                .get("children")
                .and_then(Value::as_array)
                .map(|c| c.iter().map(|e| self.noeud(e)).collect())
                .unwrap_or_default();
            let mut s = Map::new();
            s.insert("type".into(), json!("split"));
            s.insert(
                "direction".into(),
                n.get("direction").cloned().unwrap_or(json!("vertical")),
            );
            s.insert("children".into(), Value::Array(enfants));
            if let Some(d) = dimension {
                s.insert("dimension".into(), d.clone());
            }
            return Value::Object(s);
        }
        let tabs = n.get("tabs").and_then(Value::as_u64).unwrap_or(1) as usize;
        let courant = n.get("current").and_then(Value::as_u64).unwrap_or(0) as usize;
        self.pile(tabs, courant, dimension)
    }

    fn dock(&mut self, d: Option<&Value>) -> Value {
        let replie = d
            .and_then(|d| d.get("collapsed"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let tabs = d
            .and_then(|d| d.get("tabs"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let enfants = if tabs > 0 {
            vec![self.pile(tabs, 0, None)]
        } else {
            Vec::new()
        };
        let mut s = Map::new();
        s.insert("type".into(), json!("split"));
        s.insert("direction".into(), json!("horizontal"));
        s.insert("children".into(), Value::Array(enfants));
        s.insert("collapsed".into(), json!(replie));
        if let Some(w) = d.and_then(|d| d.get("width")).filter(|w| w.is_number()) {
            s.insert("width".into(), w.clone());
        }
        Value::Object(s)
    }
}

/// Traduit l'arbre de scène en layout sérialisé (le `layoutVide` de `_t1.ts`). `None` si `arbre`
/// n'a pas de `main` ou si le résultat n'est pas un layout lisible.
pub fn layout_de_scene(arbre: &Value) -> Option<WorkspaceLayout> {
    let mut c = Compteur(0);
    let main = c.noeud(arbre.get("main")?);
    // La racine est toujours un split : une pile seule y est enveloppée.
    let main = if main.get("type").and_then(Value::as_str) == Some("split") {
        main
    } else {
        json!({ "type": "split", "direction": "vertical", "children": [main] })
    };
    let layout = json!({
        "version": 1,
        "main": main,
        "left": c.dock(arbre.get("left")),
        "right": c.dock(arbre.get("right")),
        "active": null,
    });
    WorkspaceLayout::from_value(&layout)
}

/// Branche l'action `layout` sur le Workspace : à appeler par l'assemblage (`fr-t1-binaire`), avec le
/// Workspace de la fenêtre. L'abonnement vit autant que l'[`EventRef`] rendu.
pub fn brancher_scene(commandes: &CommandManager, workspace: &Rc<RefCell<Workspace>>) -> EventRef {
    let ws = workspace.clone();
    commandes.on(EVENEMENT_LAYOUT, move |donnees| {
        let Some(arbre) = donnees.downcast_ref::<Value>() else {
            return;
        };
        if let Some(layout) = layout_de_scene(arbre)
            && let Ok(mut w) = ws.try_borrow_mut()
        {
            w.change_layout(&layout);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_pile_seule_est_enveloppee_dans_un_split_vertical() {
        let l = layout_de_scene(&json!({"main": {"type": "tabs", "tabs": 4, "current": 1}}));
        let v = serde_json::to_value(l.unwrap()).unwrap();
        assert_eq!(v["main"]["type"], "split");
        assert_eq!(v["main"]["children"][0]["currentTab"], 1);
        assert_eq!(
            v["main"]["children"][0]["children"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(v["right"]["collapsed"], true);
    }

    #[test]
    fn les_docks_recoivent_une_pile_et_les_dimensions_passent() {
        let l = layout_de_scene(&json!({
            "main": {"type": "split", "direction": "vertical", "children": [
                {"type": "tabs", "tabs": 1, "dimension": 25},
                {"type": "tabs", "tabs": 1, "dimension": 75}
            ]},
            "right": {"collapsed": false, "tabs": 2}
        }));
        let v = serde_json::to_value(l.unwrap()).unwrap();
        assert_eq!(v["main"]["children"][1]["dimension"], 75.0);
        assert_eq!(v["right"]["collapsed"], false);
        assert_eq!(
            v["right"]["children"][0]["children"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
}
