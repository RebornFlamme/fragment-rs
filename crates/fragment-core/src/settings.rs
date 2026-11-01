// Port de app/src/ui/core/settings/definition.ts (les TYPES, pas le rendu)
//
// LE SYSTÈME DÉCLARATIF — décrire les réglages comme DONNÉES. Un onglet expose ses
// réglages via `getSettingDefinitions()` ; l'avantage n'est pas le rendu mais que les
// réglages deviennent INTROSPECTABLES (la recherche « Search settings… » filtre ces
// données). Décision du chef (docs/contrats.md, relecture du 01/11/2026) : ces types
// sont des DONNÉES lues par les plugins internes, l'hôte JS et la fenêtre de réglages →
// fragment-core. Le rendu (`renderDefinition` : produit des `Setting`) est dans fragment-ui.
//
// ★ ÉCARTS :
//   - les callbacks (`onChange`, `onClick`, `onAdd`, `format`) sont des `Rc<dyn Fn>` ;
//   - `SettingDefinitionRender` (rendu DOM libre) reste côté fragment-ui : le variant
//     `SettingDefinition::Custom` porte une charge opaque (`Rc<dyn Any>`) que l'UI
//     reconnaît par `downcast` ;
//   - `nodes()` / `children()` d'un arbre peuvent être asynchrones en TS : synchrones ici
//     (l'hôte qui liste un dossier l'a déjà fait avant de construire la définition) ;
//   - docs/contrats.md classe `SettingTreeNode` et les `Setting*Control` individuels dans
//     fragment-ui alors que la décision du chef place `SettingControl*` dans le cœur : ils
//     sont ici, faute de quoi `SettingControl` (enum) ne peut pas les contenir.

use std::any::Any;
use std::rc::Rc;

/// Rappel de changement de valeur d'un contrôle.
pub type OnChange<T> = Rc<dyn Fn(T)>;

/// Le tronc commun d'un contrôle déclaratif : son discriminant `type` (`"toggle"`…).
/// Rendu par `SettingControl::base()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingControlBase {
    pub type_: &'static str,
}

#[derive(Clone)]
pub struct SettingToggleControl {
    pub value: bool,
    pub on_change: Option<OnChange<bool>>,
}

#[derive(Clone)]
pub struct SettingTextControl {
    pub value: String,
    pub placeholder: Option<String>,
    pub on_change: Option<OnChange<String>>,
}

#[derive(Clone)]
pub struct SettingTextAreaControl {
    pub value: String,
    pub placeholder: Option<String>,
    pub on_change: Option<OnChange<String>>,
}

#[derive(Clone)]
pub struct SettingNumberControl {
    pub value: f64,
    pub placeholder: Option<String>,
    pub on_change: Option<OnChange<f64>>,
}

#[derive(Clone)]
pub struct SettingSliderControl {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    /// Défaut du TS : 1.
    pub step: Option<f64>,
    /// `false` ⇒ `on_change` au relâché seulement. Défaut : `true`.
    pub instant: Option<bool>,
    /// Affiche la valeur à côté de la piste, suivie pendant le glisser.
    pub format: Option<Rc<dyn Fn(f64) -> String>>,
    pub on_change: Option<OnChange<f64>>,
}

/// Un bouton-icône discret (le reset ↺ à côté d'une valeur).
#[derive(Clone)]
pub struct SettingExtraButtonControl {
    pub icon: String,
    pub tooltip: Option<String>,
    pub on_click: Rc<dyn Fn()>,
}

#[derive(Clone)]
pub struct SettingDropdownControl {
    pub value: String,
    /// valeur → libellé, dans l'ordre d'affichage.
    pub options: Vec<(String, String)>,
    pub on_change: Option<OnChange<String>>,
}

#[derive(Clone)]
pub struct SettingColorControl {
    pub value: String,
    pub on_change: Option<OnChange<String>>,
}

#[derive(Clone)]
pub struct SettingFileControl {
    pub value: String,
    pub placeholder: Option<String>,
    pub on_change: Option<OnChange<String>>,
}

#[derive(Clone)]
pub struct SettingFolderControl {
    pub value: String,
    pub placeholder: Option<String>,
    pub on_change: Option<OnChange<String>>,
}

#[derive(Clone)]
pub struct SettingSecretControl {
    /// La clef du secret dans le trousseau — pas la valeur en clair.
    pub key: String,
    pub on_change: Option<OnChange<String>>,
}

/// L'union de tous les contrôles déclaratifs.
#[derive(Clone)]
pub enum SettingControl {
    Toggle(SettingToggleControl),
    Text(SettingTextControl),
    TextArea(SettingTextAreaControl),
    Number(SettingNumberControl),
    Slider(SettingSliderControl),
    Dropdown(SettingDropdownControl),
    Color(SettingColorControl),
    File(SettingFileControl),
    Folder(SettingFolderControl),
    Secret(SettingSecretControl),
    ExtraButton(SettingExtraButtonControl),
}

impl SettingControl {
    /// Le `type` du TS.
    pub fn base(&self) -> SettingControlBase {
        SettingControlBase {
            type_: match self {
                Self::Toggle(_) => "toggle",
                Self::Text(_) => "text",
                Self::TextArea(_) => "textarea",
                Self::Number(_) => "number",
                Self::Slider(_) => "slider",
                Self::Dropdown(_) => "dropdown",
                Self::Color(_) => "color",
                Self::File(_) => "file",
                Self::Folder(_) => "folder",
                Self::Secret(_) => "secret",
                Self::ExtraButton(_) => "extra-button",
            },
        }
    }
}

// ── DÉFINITIONS (les nœuds d'un onglet déclaratif) ──────────────────────────

/// Identité optionnelle — pour l'indexation / la recherche. Chaque définition la porte
/// dans son champ `base` (le `extends` du TS).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingDefinitionBase {
    pub id: Option<String>,
}

/// Une ligne de réglage portant UN contrôle.
#[derive(Clone)]
pub struct SettingDefinitionControl {
    pub base: SettingDefinitionBase,
    pub name: String,
    pub description: Option<String>,
    pub control: SettingControl,
}

/// Une ligne portant PLUSIEURS contrôles (ex. reset + valeur).
#[derive(Clone)]
pub struct SettingDefinitionItem {
    pub base: SettingDefinitionBase,
    pub name: String,
    pub description: Option<String>,
    pub controls: Vec<SettingControl>,
}

/// Un en-tête de section suivi d'items.
#[derive(Clone)]
pub struct SettingDefinitionGroup {
    pub base: SettingDefinitionBase,
    pub name: String,
    pub items: Vec<SettingDefinition>,
}

/// Une sous-page (chevron `>` — rendue à plat pour l'instant).
#[derive(Clone)]
pub struct SettingDefinitionPage {
    pub base: SettingDefinitionBase,
    pub name: String,
    pub items: Vec<SettingDefinition>,
}

/// Ce qui peut vivre DANS une liste/un groupe.
#[derive(Clone)]
pub enum SettingGroupItem {
    Control(SettingDefinitionControl),
    Item(SettingDefinitionItem),
}

/// Une liste répétable d'items, avec un bouton d'ajout.
#[derive(Clone)]
pub struct SettingDefinitionList {
    pub base: SettingDefinitionBase,
    pub name: String,
    pub items: Vec<SettingGroupItem>,
    pub add: Option<SettingDefinitionAddItem>,
}

/// L'état vide (« No secrets have been added… »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingDefinitionEmpty {
    pub base: SettingDefinitionBase,
    pub message: String,
}

/// Une ligne « nom + bouton d'action ».
#[derive(Clone)]
pub struct SettingDefinitionAction {
    pub base: SettingDefinitionBase,
    pub name: String,
    pub description: Option<String>,
    pub label: String,
    pub cta: bool,
    pub warning: bool,
    pub on_click: Rc<dyn Fn()>,
}

/// Un bouton « + ajouter » (en tête d'une section ou d'une liste).
#[derive(Clone)]
pub struct SettingDefinitionAddItem {
    pub base: SettingDefinitionBase,
    pub label: String,
    pub on_add: Rc<dyn Fn()>,
}

/// Un nœud d'ARBRE : une ligne (libellé, icône, un contrôle facultatif) et ses enfants,
/// PARESSEUX (demandés au premier dépli).
#[derive(Clone)]
pub struct SettingTreeNode {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub children: Option<Rc<dyn Fn() -> Vec<SettingTreeNode>>>,
    pub control: Option<SettingControl>,
}

/// Un ARBRE de réglages (des dossiers et leur accès…).
#[derive(Clone)]
pub struct SettingDefinitionTree {
    pub base: SettingDefinitionBase,
    pub name: String,
    pub description: Option<String>,
    pub nodes: Rc<dyn Fn() -> Vec<SettingTreeNode>>,
}

/// L'union de toutes les définitions — le nœud d'un onglet déclaratif.
#[derive(Clone)]
pub enum SettingDefinition {
    Control(SettingDefinitionControl),
    Item(SettingDefinitionItem),
    Group(SettingDefinitionGroup),
    Page(SettingDefinitionPage),
    List(SettingDefinitionList),
    Empty(SettingDefinitionEmpty),
    Action(SettingDefinitionAction),
    /// `SettingDefinitionRender` (rendu libre) : la charge est interprétée par fragment-ui.
    Custom(Rc<dyn Any>),
    AddItem(SettingDefinitionAddItem),
    Tree(SettingDefinitionTree),
}

impl SettingDefinition {
    /// Le `type` du TS.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Control(_) => "control",
            Self::Item(_) => "item",
            Self::Group(_) => "group",
            Self::Page(_) => "page",
            Self::List(_) => "list",
            Self::Empty(_) => "empty",
            Self::Action(_) => "action",
            Self::Custom(_) => "render",
            Self::AddItem(_) => "add-item",
            Self::Tree(_) => "tree",
        }
    }

    /// L'identité optionnelle (`None` pour `Custom`, dont la charge est opaque).
    pub fn id(&self) -> Option<&str> {
        let base = match self {
            Self::Control(d) => &d.base,
            Self::Item(d) => &d.base,
            Self::Group(d) => &d.base,
            Self::Page(d) => &d.base,
            Self::List(d) => &d.base,
            Self::Empty(d) => &d.base,
            Self::Action(d) => &d.base,
            Self::AddItem(d) => &d.base,
            Self::Tree(d) => &d.base,
            Self::Custom(_) => return None,
        };
        base.id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    // ─── Ajoutés : définition.ts n'est testé que par le RENDU DOM (ui), non porté ici ───

    fn dropdown(change: OnChange<String>) -> SettingControl {
        SettingControl::Dropdown(SettingDropdownControl {
            value: "write".into(),
            options: vec![
                ("hidden".into(), "Masqué".into()),
                ("write".into(), "Écriture".into()),
            ],
            on_change: Some(change),
        })
    }

    #[test]
    fn un_arbre_charge_ses_enfants_a_la_demande_et_un_controle_par_ligne() {
        let appels = Rc::new(Cell::new(0));
        let a = appels.clone();
        let vu: Rc<RefCell<Vec<String>>> = Rc::default();
        let v = vu.clone();
        let enfants: Rc<dyn Fn() -> Vec<SettingTreeNode>> = Rc::new(move || {
            a.set(a.get() + 1);
            vec![SettingTreeNode {
                id: "a/b".into(),
                label: "b".into(),
                icon: None,
                children: None,
                control: None,
            }]
        });
        let def = SettingDefinition::Tree(SettingDefinitionTree {
            base: SettingDefinitionBase::default(),
            name: "Dossiers".into(),
            description: None,
            nodes: Rc::new(move || {
                let v = v.clone();
                vec![SettingTreeNode {
                    id: "a".into(),
                    label: "a".into(),
                    icon: Some("folder".into()),
                    children: Some(enfants.clone()),
                    control: Some(dropdown(Rc::new(move |x| v.borrow_mut().push(x)))),
                }]
            }),
        });
        let SettingDefinition::Tree(t) = &def else {
            panic!("arbre attendu")
        };
        let racines = (t.nodes)();
        assert_eq!(appels.get(), 0); // paresseux : rien avant le premier dépli
        assert_eq!(
            racines[0].control.as_ref().unwrap().base().type_,
            "dropdown"
        );
        if let Some(SettingControl::Dropdown(d)) = &racines[0].control {
            (d.on_change.as_ref().unwrap())("hidden".into());
        }
        assert_eq!(*vu.borrow(), vec!["hidden"]);
        let fils = (racines[0].children.as_ref().unwrap())();
        assert_eq!(fils[0].id, "a/b");
        assert_eq!(appels.get(), 1);
        assert_eq!(def.type_name(), "tree");
    }

    #[test]
    fn identite_et_discriminant_de_chaque_definition() {
        let d = SettingDefinition::Empty(SettingDefinitionEmpty {
            base: SettingDefinitionBase {
                id: Some("vide".into()),
            },
            message: "rien".into(),
        });
        assert_eq!(d.id(), Some("vide"));
        assert_eq!(d.type_name(), "empty");
        let c = SettingDefinition::Custom(Rc::new(42_u8));
        assert_eq!(c.id(), None);
        assert_eq!(c.type_name(), "render");
    }

    #[test]
    fn les_types_des_controles_sont_ceux_du_ts() {
        let t = SettingControl::Toggle(SettingToggleControl {
            value: true,
            on_change: None,
        });
        assert_eq!(t.base().type_, "toggle");
        let b = SettingControl::ExtraButton(SettingExtraButtonControl {
            icon: "reset".into(),
            tooltip: None,
            on_click: Rc::new(|| {}),
        });
        assert_eq!(b.base().type_, "extra-button");
    }
}
