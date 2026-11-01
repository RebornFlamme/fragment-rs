// Port de app/src/ui/views/agent/screenModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Ce que voit l'utilisateur, en STRUCTURE — fonctions PURES.
//
//  Partir de `getLayout()` (la forme sérialisée de l'arbre, §8.2) plutôt que de
//  l'arbre vivant : c'est déjà l'abstraction « une feuille = un id + un état »,
//  valable pour toute vue. L'interface n'apporte que les rectangles, mesurés à
//  part et passés ici dans `leaves`.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

// ── Le layout sérialisé du workspace (minimum local) ──
// TODO(contrat) : remplacer par fragment_core::layout::{LayoutNode, SplitLayout, WorkspaceLayout}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitLayout {
    /// `vertical` (côte à côte) ou `horizontal` (empilé).
    pub direction: String,
    pub children: Vec<LayoutNode>,
    /// Sidedocks seulement.
    pub collapsed: Option<bool>,
    /// Sidedocks seulement — la valeur de `--sidebar-width`, en px.
    pub width: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutNode {
    Split(SplitLayout),
    Tabs {
        children: Vec<LayoutNode>,
        current_tab: i64,
    },
    /// `state_type` : le `type` du `ViewState` de la feuille.
    Leaf {
        id: String,
        state_type: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceLayout {
    pub main: SplitLayout,
    pub left: SplitLayout,
    pub right: SplitLayout,
    /// L'id de la feuille active, ou `None`.
    pub active: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Ce que l'arbre sérialisé ne dit pas d'une feuille, lu sur la feuille vivante.
#[derive(Debug, Clone, PartialEq)]
pub struct LeafFacts {
    pub r#type: String,
    pub title: String,
    pub file: Option<String>,
    /// Le rectangle de la feuille dans la fenêtre — `None` si elle n'est pas affichée.
    pub rect: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenLeaf {
    pub id: String,
    pub visible: bool,
    pub facts: LeafFacts,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScreenNode {
    Split {
        direction: String,
        children: Vec<ScreenNode>,
    },
    Tabs {
        children: Vec<ScreenNode>,
    },
    Leaf(ScreenLeaf),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenDock {
    pub open: bool,
    pub width: Option<f64>,
    pub root: ScreenNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenModel {
    pub window: WindowSize,
    /// La feuille active (souvent le chat lui-même).
    pub active: Option<String>,
    /// La dernière feuille active de la zone principale — ce que l'utilisateur regarde.
    pub main_focus: Option<String>,
    pub main: ScreenNode,
    pub left: ScreenDock,
    pub right: ScreenDock,
}

fn walk(node: &LayoutNode, shown: bool, leaves: &HashMap<String, LeafFacts>) -> ScreenNode {
    match node {
        LayoutNode::Leaf { id, state_type } => {
            let mut facts = leaves.get(id).cloned().unwrap_or_else(|| LeafFacts {
                r#type: state_type.clone(),
                title: String::new(),
                file: None,
                rect: None,
            });
            if !shown {
                facts.rect = None;
            }
            ScreenNode::Leaf(ScreenLeaf {
                id: id.clone(),
                visible: shown,
                facts,
            })
        }
        LayoutNode::Tabs {
            children,
            current_tab,
        } => {
            // Math.min(Math.max(current, 0), len - 1)
            let current = (*current_tab).max(0).min(children.len() as i64 - 1);
            ScreenNode::Tabs {
                children: children
                    .iter()
                    .enumerate()
                    .map(|(i, c)| walk(c, shown && i as i64 == current, leaves))
                    .collect(),
            }
        }
        LayoutNode::Split(s) => walk_split(s, shown, leaves),
    }
}

fn walk_split(s: &SplitLayout, shown: bool, leaves: &HashMap<String, LeafFacts>) -> ScreenNode {
    ScreenNode::Split {
        direction: s.direction.clone(),
        children: s.children.iter().map(|c| walk(c, shown, leaves)).collect(),
    }
}

/// L'arbre annoté. Une feuille est VISIBLE si elle est l'onglet courant de sa
/// pile et que son dock n'est pas replié — la même règle que l'écran.
pub fn build_screen(
    layout: &WorkspaceLayout,
    leaves: &HashMap<String, LeafFacts>,
    window: WindowSize,
    main_focus: Option<String>,
) -> ScreenModel {
    let dock = |s: &SplitLayout| {
        let ouvert = s.collapsed != Some(true);
        ScreenDock {
            open: ouvert,
            width: s.width,
            root: walk_split(s, ouvert, leaves),
        }
    };
    ScreenModel {
        window,
        active: layout.active.clone(),
        main_focus,
        main: walk_split(&layout.main, true, leaves),
        left: dock(&layout.left),
        right: dock(&layout.right),
    }
}

/// Toutes les feuilles du modèle, dans l'ordre de l'écran (gauche, principale, droite).
pub fn screen_leaves(model: &ScreenModel) -> Vec<&ScreenLeaf> {
    fn rec<'a>(n: &'a ScreenNode, out: &mut Vec<&'a ScreenLeaf>) {
        match n {
            ScreenNode::Leaf(l) => out.push(l),
            ScreenNode::Split { children, .. } | ScreenNode::Tabs { children } => {
                children.iter().for_each(|c| rec(c, out))
            }
        }
    }
    let mut out = Vec::new();
    for racine in [&model.left.root, &model.main, &model.right.root] {
        rec(racine, &mut out);
    }
    out
}

/// `Math.round` du JS (arrondi vers +∞ à la moitié).
fn arrondi(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

fn r(rect: &Rect) -> String {
    format!(
        "x={} y={} {}×{}",
        arrondi(rect.x),
        arrondi(rect.y),
        arrondi(rect.width),
        arrondi(rect.height)
    )
}

/// Le modèle en texte : une ligne par nœud, indentée ; ▶ = onglet affiché, * = feuille active.
pub fn format_screen(model: &ScreenModel) -> String {
    fn rec(n: &ScreenNode, depth: usize, model: &ScreenModel, lines: &mut Vec<String>) {
        let pad = "  ".repeat(depth);
        match n {
            ScreenNode::Split {
                direction,
                children,
            } => {
                lines.push(format!(
                    "{pad}split {}",
                    if direction == "vertical" {
                        "vertical (côte à côte)"
                    } else {
                        "horizontal (empilé)"
                    }
                ));
                children
                    .iter()
                    .for_each(|c| rec(c, depth + 1, model, lines));
            }
            ScreenNode::Tabs { children } => {
                lines.push(format!("{pad}onglets"));
                children
                    .iter()
                    .for_each(|c| rec(c, depth + 1, model, lines));
            }
            ScreenNode::Leaf(l) => {
                let marque = format!(
                    "{}{}",
                    if l.visible { '▶' } else { ' ' },
                    if model.active.as_deref() == Some(l.id.as_str()) {
                        '*'
                    } else {
                        ' '
                    }
                );
                let titre = if l.facts.title.is_empty() {
                    String::new()
                } else {
                    format!(" « {} »", l.facts.title)
                };
                let fichier = l
                    .facts
                    .file
                    .as_deref()
                    .filter(|f| !f.is_empty())
                    .map(|f| format!("  {f}"))
                    .unwrap_or_default();
                let ou = match &l.facts.rect {
                    Some(rect) => format!("  [{}]", r(rect)),
                    None if l.visible => String::new(),
                    None => "  (caché)".to_string(),
                };
                lines.push(format!(
                    "{pad}{marque} {}  {}{titre}{fichier}{ou}",
                    l.id, l.facts.r#type
                ));
            }
        }
    }
    let tiret = |o: &Option<String>| o.clone().unwrap_or_else(|| "—".to_string());
    let mut lines = vec![
        format!("Fenêtre {}×{}", model.window.width, model.window.height),
        format!(
            "Feuille active : {} · focus principal : {}",
            tiret(&model.active),
            tiret(&model.main_focus)
        ),
        String::new(),
        "Zone principale".to_string(),
    ];
    rec(&model.main, 1, model, &mut lines);
    for (nom, d) in [("Dock gauche", &model.left), ("Dock droit", &model.right)] {
        lines.push(String::new());
        let largeur = match d.width {
            Some(w) if w != 0.0 => format!(", {w} px"),
            _ => String::new(),
        };
        lines.push(format!(
            "{nom} — {}",
            if d.open {
                format!("ouvert{largeur}")
            } else {
                "replié".to_string()
            }
        ));
        rec(&d.root, 1, model, &mut lines);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str, t: &str) -> LayoutNode {
        LayoutNode::Leaf {
            id: id.into(),
            state_type: t.into(),
        }
    }

    fn layout() -> WorkspaceLayout {
        WorkspaceLayout {
            active: Some("chat".into()),
            main: SplitLayout {
                direction: "vertical".into(),
                collapsed: None,
                width: None,
                children: vec![
                    LayoutNode::Tabs {
                        current_tab: 1,
                        children: vec![leaf("l-a", "markdown"), leaf("l-b", "markdown")],
                    },
                    LayoutNode::Tabs {
                        current_tab: 0,
                        children: vec![leaf("l-pdf", "pdf")],
                    },
                ],
            },
            left: SplitLayout {
                direction: "horizontal".into(),
                collapsed: Some(true),
                width: None,
                children: vec![LayoutNode::Tabs {
                    current_tab: 0,
                    children: vec![leaf("l-files", "file-explorer")],
                }],
            },
            right: SplitLayout {
                direction: "horizontal".into(),
                collapsed: None,
                width: Some(300.0),
                children: vec![LayoutNode::Tabs {
                    current_tab: 0,
                    children: vec![leaf("chat", "chat")],
                }],
            },
        }
    }

    fn modele() -> ScreenModel {
        let rect = Some(Rect {
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 400.0,
        });
        let f = |t: &str, titre: &str, file: Option<&str>| LeafFacts {
            r#type: t.into(),
            title: titre.into(),
            file: file.map(str::to_string),
            rect,
        };
        let facts: HashMap<String, LeafFacts> = HashMap::from([
            ("l-a".to_string(), f("markdown", "a", Some("a.md"))),
            ("l-b".to_string(), f("markdown", "b", Some("b.md"))),
            ("l-pdf".to_string(), f("pdf", "doc", Some("doc.pdf"))),
            ("l-files".to_string(), f("file-explorer", "coffre", None)),
            ("chat".to_string(), f("chat", "Agent", None)),
        ]);
        build_screen(
            &layout(),
            &facts,
            WindowSize {
                width: 1200,
                height: 800,
            },
            Some("l-b".into()),
        )
    }

    #[test]
    fn visible_onglet_courant_dun_dock_ouvert_le_rectangle_nest_garde_que_pour_le_visible() {
        let m = modele();
        let vus: HashMap<&str, bool> = screen_leaves(&m)
            .iter()
            .map(|l| (l.id.as_str(), l.visible))
            .collect();
        assert_eq!(
            vus,
            HashMap::from([
                ("l-files", false),
                ("l-a", false),
                ("l-b", true),
                ("l-pdf", true),
                ("chat", true)
            ])
        );
        assert_eq!(
            screen_leaves(&m)
                .iter()
                .find(|l| l.id == "l-a")
                .map(|l| l.facts.rect),
            Some(None)
        );
    }

    #[test]
    fn le_texte_marque_longlet_affiche_la_feuille_active_les_docks() {
        let txt = format_screen(&modele());
        assert!(txt.contains("Feuille active : chat · focus principal : l-b"));
        assert!(txt.contains("▶  l-b  markdown « b »  b.md  [x=10 y=20 300×400]"));
        assert!(txt.contains("    l-a  markdown « a »  a.md  (caché)"));
        assert!(txt.contains("▶* chat  chat « Agent »"));
        assert!(txt.contains("Dock gauche — replié"));
        assert!(txt.contains("Dock droit — ouvert, 300 px"));
    }
}
